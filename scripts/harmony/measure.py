#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""Measures the harmonizer's numbers from the POP909 dataset.

The harmonizer (`crates/oxiguitar-edit/src/harmonize.rs`,
`docs/design/harmonize.md`) picks chords for a melody with numbers that say
how often a chord follows another, which melody notes sound over which
chord, and how often the chord changes at a bar line or inside a bar. This
script counts them in POP909 (909 pop songs with a melody, chords, beats
and keys; MIT licence, https://github.com/music-x-lab/POP909-Dataset),
tunes the few weights on songs it kept aside, and writes
`crates/oxiguitar-edit/src/harmonize/numbers.rs`.

Nothing in a build or a release runs this: the written file is committed.
Run it again only to change the model:

    git clone --depth 1 https://github.com/music-x-lab/POP909-Dataset
    python3 scripts/harmony/measure.py POP909-Dataset/POP909 \
        crates/oxiguitar-edit/src/harmonize/numbers.rs

It needs Python 3.9 or later and numpy.
"""

import math
import os
import struct
import sys
import textwrap

import numpy as np

# ---------------------------------------------------------------- vocabulary

# Chord kinds, in the order of the Rust `Kind` enum. The simple vocabulary
# is the first three.
KINDS = ["maj", "min", "dim", "maj7", "min7", "7", "hdim7", "sus4", "7sus4"]
SIMPLE = 3

# POP909's chord labels onto the rich and the simple vocabulary. `None`
# stands for "by the key": a chord without a third takes the third the key
# gives its root.
RICH = {
    "maj": "maj", "min": "min", "dim": "dim", "aug": "maj", "maj6": "maj",
    "min6": "min", "minmaj7": "min", "dim7": "dim", "maj7": "maj7",
    "min7": "min7", "7": "7", "hdim7": "hdim7", "sus4": "sus4",
    "sus4(b7)": "7sus4", "sus2": None,
}
PLAIN = {
    "maj": "maj", "min": "min", "dim": "dim", "maj7": "maj", "min7": "min",
    "7": "maj", "hdim7": "dim", "sus4": None, "7sus4": None,
}
# The third the major key gives each root: minor on II, III and VI,
# diminished on VII, major elsewhere.
DIATONIC = {2: "min", 4: "min", 9: "min", 11: "dim"}

NOTES = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def pitch_class(name):
    pc = NOTES[name[0]]
    for c in name[1:]:
        pc += 1 if c == "#" else -1
    return pc % 12


def parse_chord(label):
    """A POP909 label as (root pitch class, POP909 quality), or None."""
    if label == "N":
        return None
    root, _, rest = label.partition(":")
    quality = rest.split("/")[0]
    if quality not in RICH:
        return None
    return pitch_class(root), quality


def state(root, quality, tonic, rich):
    """The model's state: 12 * kind + the root's distance above the tonic."""
    degree = (root - tonic) % 12
    kind = RICH[quality]
    if kind is None:
        kind = DIATONIC.get(degree, "maj")
    if not rich:
        kind = PLAIN[kind]
        if kind is None:
            kind = DIATONIC.get(degree, "maj")
    return 12 * KINDS.index(kind) + degree


# ------------------------------------------------------------------ reading


def read_midi(path):
    """The notes of a MIDI file's tracks by name, as (start, end, pitch) in
    seconds."""
    data = open(path, "rb").read()
    assert data[:4] == b"MThd"
    _, _, division = struct.unpack(">HHH", data[8:14])
    at = 14
    tempos = []
    tracks = {}
    while at < len(data):
        assert data[at:at + 4] == b"MTrk"
        (length,) = struct.unpack(">I", data[at + 4:at + 8])
        body = data[at + 8:at + 8 + length]
        at += 8 + length
        i = 0
        tick = 0
        status = 0
        name = ""
        notes = []
        on = {}
        while i < len(body):
            delta = 0
            while True:
                b = body[i]
                i += 1
                delta = (delta << 7) | (b & 0x7F)
                if b < 0x80:
                    break
            tick += delta
            if body[i] >= 0x80:
                status = body[i]
                i += 1
            if status == 0xFF:
                kind = body[i]
                i += 1
                size = 0
                while True:
                    b = body[i]
                    i += 1
                    size = (size << 7) | (b & 0x7F)
                    if b < 0x80:
                        break
                payload = body[i:i + size]
                i += size
                if kind == 0x03:
                    name = payload.decode("latin-1")
                elif kind == 0x51:
                    tempos.append((tick, int.from_bytes(payload, "big")))
            elif status in (0xF0, 0xF7):
                size = 0
                while True:
                    b = body[i]
                    i += 1
                    size = (size << 7) | (b & 0x7F)
                    if b < 0x80:
                        break
                i += size
            else:
                high = status & 0xF0
                if high in (0xC0, 0xD0):
                    i += 1
                else:
                    a, b = body[i], body[i + 1]
                    i += 2
                    if high == 0x90 and b > 0:
                        on[a] = tick
                    elif high in (0x80, 0x90) and a in on:
                        notes.append((on.pop(a), tick, a))
        tracks[name.strip().upper()] = notes
    tempos.sort()
    if not tempos or tempos[0][0] != 0:
        tempos.insert(0, (0, 500000))

    def seconds(tick):
        total = 0.0
        for n, (start, tempo) in enumerate(tempos):
            end = tempos[n + 1][0] if n + 1 < len(tempos) else None
            if end is None or tick < end:
                return total + (tick - start) * tempo / division / 1e6
            total += (end - start) * tempo / division / 1e6
        return total

    return {name: [(seconds(s), seconds(e), p) for s, e, p in sorted(notes)]
            for name, notes in tracks.items()}


def rows(path):
    return [line.split() for line in open(path) if line.strip()]


class Song:
    """One song cut into half bars (two cells for every bar)."""

    def __init__(self, folder):
        number = os.path.basename(folder)
        beats = [(float(r[0]), float(r[2])) for r in rows(f"{folder}/beat_midi.txt")]
        times = np.array([b[0] for b in beats])
        downs = [n for n, b in enumerate(beats) if b[1] == 1.0]
        # Bars of four beats are cut into two cells; any other bar (the
        # beat tracker's slips, a bar of 2/4) breaks the chain.
        spans = []
        for d, e in zip(downs, downs[1:]):
            if e - d == 4:
                spans += [(d, d + 2, True), (d + 2, e, False)]
            else:
                spans += [(d, e, None), (d, e, None)]
        self.ok = sum(1 for s in spans if s[2] is not None) >= 0.8 * len(spans) > 0
        if not self.ok:
            return
        chords = [(float(r[0]), float(r[1]), parse_chord(r[2]))
                  for r in rows(f"{folder}/chord_midi.txt")]
        keys = []
        for r in rows(f"{folder}/key_audio.txt"):
            name, _, mode = r[2].partition(":")
            tonic = pitch_class(name)
            keys.append((float(r[0]), float(r[1]),
                         (tonic + 3) % 12 if mode.startswith("min") else tonic))
        tracks = read_midi(f"{folder}/{number}.mid")
        notes = tracks.get("MELODY", [])

        def position(t):
            """A time in beats."""
            n = int(np.searchsorted(times, t, side="right")) - 1
            n = min(max(n, 0), len(times) - 2)
            return n + (t - times[n]) / (times[n + 1] - times[n])

        melody = [(position(s), position(e), p) for s, e, p in notes]
        piano = [(position(s), position(e), p) for s, e, p in tracks.get("PIANO", [])]
        # A long note first, so the scan below never passes one still sounding.
        self.around = []    # [cell][12]: the accompaniment, as shares of the cell
        self.lowest = []    # [cell]: the accompaniment's lowest pitch class, or -1
        q = 0
        labelled = [(position(s), position(e), c) for s, e, c in chords]
        # Half bars from the first downbeat to the last.
        self.weights = []   # [cell][12]: beats each pitch class sounds
        self.accents = []   # [cell][12]: the note struck at the cell's start
        self.truth = []     # [cell]: (root, quality) or None
        self.tonic = []     # [cell]: the major key's tonic
        self.bar_start = []  # [cell]: whether the cell starts a bar
        m = 0
        c = 0
        for start, end, starts_bar in spans:
            w = np.zeros(12)
            a = np.zeros(12)
            while m < len(melody) and melody[m][1] <= start:
                m += 1
            k = m
            while k < len(melody) and melody[k][0] < end:
                s, e, p = melody[k]
                w[p % 12] += max(0.0, min(e, end) - max(s, start))
                if abs(s - start) < 0.13:
                    a[p % 12] += min(e - s, 1.0)
                k += 1
            best = {}
            if starts_bar is None:
                w[:] = 0.0
                a[:] = 0.0
            while starts_bar is not None and c < len(labelled) and labelled[c][1] <= start:
                c += 1
            k = c
            while starts_bar is not None and k < len(labelled) and labelled[k][0] < end:
                s, e, ch = labelled[k]
                best[ch] = best.get(ch, 0.0) + min(e, end) - max(s, start)
                k += 1
            top = max(best, key=best.get) if best else None
            middle = times[min(start + 1, len(times) - 1)]
            tonic = keys[0][2]
            for s, e, t in keys:
                if s <= middle:
                    tonic = t
            around = np.zeros(12)
            low = None
            if starts_bar is not None:
                while q < len(piano) and piano[q][0] < end:
                    q += 1
                # Notes are sorted by start: look back as far as notes last.
                k = q - 1
                while k >= 0 and piano[k][0] > start - 16:
                    s, e, p = piano[k]
                    shared = min(e, end) - max(s, start)
                    if shared > 0:
                        around[p % 12] += shared
                        if low is None or p < low:
                            low = p
                    k -= 1
            if around.sum() > 0:
                around *= (end - start) / around.sum()
            self.around.append(around)
            self.lowest.append(-1 if low is None else low % 12)
            self.weights.append(w)
            self.accents.append(a)
            self.truth.append(top)
            self.tonic.append(tonic)
            self.bar_start.append(starts_bar is not False)
        self.weights = np.array(self.weights)
        self.accents = np.array(self.accents)
        self.around = np.array(self.around)
        self.ok = len(self.truth) >= 16 and self.weights.sum() > 16

    def cells(self, per_bar):
        """The song's cells, half bars or (when `per_bar`) bars."""
        if not per_bar:
            return Cells(self.weights, self.accents, self.around, self.lowest,
                         self.truth, self.tonic, self.bar_start)
        n = len(self.truth) // 2
        w = self.weights[0:2 * n:2] + self.weights[1:2 * n:2]
        a = self.accents[0:2 * n:2]
        around = self.around[0:2 * n:2] + self.around[1:2 * n:2]
        lowest = [self.lowest[2 * i] if self.lowest[2 * i] >= 0 else self.lowest[2 * i + 1]
                  for i in range(n)]
        truth = [self.truth[2 * i] or self.truth[2 * i + 1] for i in range(n)]
        tonic = [self.tonic[2 * i] for i in range(n)]
        return Cells(w, a, around, lowest, truth, tonic, [True] * n)


class Cells:
    def __init__(self, weights, accents, around, lowest, truth, tonic, bar_start):
        self.weights = weights      # the melody: beats of each pitch class
        self.accents = accents      # the melody's note struck at the start
        self.around = around        # the accompaniment, as shares of the cell
        self.lowest = lowest        # its lowest pitch class, or -1
        self.truth = truth
        self.tonic = tonic
        self.bar_start = bar_start


# ----------------------------------------------------------------- counting


class Model:
    """The counts of one vocabulary. Which chord follows which and what
    sounds over it are counted on half bars, whatever the density; how
    often the chord changes is counted for both densities."""

    def __init__(self, songs, rich):
        self.rich = rich
        kinds = len(KINDS) if rich else SIMPLE
        self.n = n = 12 * kinds
        follow = np.zeros((n, n))
        sound = np.zeros((n, 12))
        around = np.zeros((n, 12))
        lowest = np.zeros((n, 12))
        first = np.zeros(n)
        each = np.zeros(n)
        # [per bar, per half bar][bar line, inside a bar][held, changed]
        rhythm = np.zeros((2, 2, 2))
        for song in songs:
            for per_bar in (True, False):
                c = song.cells(per_bar)
                previous = None
                begun = False
                for i, chord in enumerate(c.truth):
                    if chord is None:
                        previous = None
                        continue
                    s = state(chord[0], chord[1], c.tonic[i], rich)
                    if not per_bar:
                        sound[s] += np.roll(c.weights[i], -c.tonic[i])
                        around[s] += np.roll(c.around[i], -c.tonic[i])
                        if c.lowest[i] >= 0:
                            lowest[s][(c.lowest[i] - c.tonic[i]) % 12] += 1
                        each[s] += 1
                        if not begun:
                            first[s] += 1
                            begun = True
                    if previous is not None:
                        where = 0 if c.bar_start[i] else 1
                        rhythm[0 if per_bar else 1][where][int(previous != s)] += 1
                        if previous != s and not per_bar:
                            follow[previous][s] += 1
                    previous = s
        self.each = each / each.sum()

        def smoothed(counts, floor, trust):
            """Each chord's shares, a chord that seldom came backed by what
            its kind gets on any root (turned to its root)."""
            by_kind = np.zeros((kinds, 12))
            for s in range(n):
                by_kind[s // 12] += np.roll(counts[s], -(s % 12))
            by_kind = (by_kind + floor) / (by_kind + floor).sum(axis=1, keepdims=True)
            out = np.zeros((n, 12))
            for s in range(n):
                back = np.roll(by_kind[s // 12], s % 12)
                out[s] = (counts[s] + trust * back) / (counts[s].sum() + trust)
            return out

        self.sound = smoothed(sound, 0.05, 40.0)
        self.around = smoothed(around, 0.05, 40.0)
        self.lowest = smoothed(lowest, 0.05, 20.0)
        # A chord's followers are backed by how common each chord is.
        spread = (self.each + 1e-4) / (self.each + 1e-4).sum()
        self.follow = np.zeros((n, n))
        for s in range(n):
            others = spread.copy()
            others[s] = 0.0
            others /= others.sum()
            self.follow[s] = (follow[s] + 5.0 * others) / (follow[s].sum() + 5.0)
        self.first = (first + 20.0 * spread) / (first.sum() + 20.0)
        self.spread = spread
        rhythm[0][1] = rhythm[0][0]
        self.rhythms = rhythm / rhythm.sum(axis=2, keepdims=True)
        # Tuned per density, [per bar, per half bar]: the melody's note
        # weight and accent, the accompaniment's weight, the lowest note's.
        self.weights = [[1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]]
        self.density(False)

    def density(self, per_bar):
        self.per_bar = per_bar
        self.rhythm = self.rhythms[0 if per_bar else 1]
        self.w = self.weights[0 if per_bar else 1]

    def costs(self):
        """Everything as costs (minus natural logarithms)."""
        with np.errstate(divide="ignore"):
            return (-np.log(self.follow), -np.log(self.sound), -np.log(self.first),
                    -np.log(self.spread), -np.log(self.rhythm), -np.log(self.around),
                    -np.log(self.lowest))

    def harmonize(self, c, tonic, band):
        """The cheapest chord per cell, as absolute (root, kind index), and
        the chain's cost. `band` says whether the accompaniment is heard."""
        follow, sound, first, _, rhythm, around, lowest = self.costs()
        n = self.n
        cells = len(c.weights)
        note_weight, accent, around_weight, lowest_weight = self.w
        evidence = (c.weights + accent * c.accents) * note_weight
        emit = np.zeros((cells, n))
        for i in range(cells):
            emit[i] = sound @ np.roll(evidence[i], -tonic[i])
            if band:
                emit[i] += around_weight * (around @ np.roll(c.around[i], -tonic[i]))
                if c.lowest[i] >= 0:
                    emit[i] += lowest_weight * lowest[:, (c.lowest[i] - tonic[i]) % 12]
        cost = first + emit[0]
        back = np.zeros((cells, n), dtype=int)
        for i in range(1, cells):
            where = 0 if c.bar_start[i] else 1
            shift = (tonic[i] - tonic[i - 1]) % 12
            if shift:
                # The chords before, named in this cell's key.
                index = np.array([12 * (s // 12) + (s % 12 - shift) % 12 for s in range(n)])
                remap = np.argsort(index)
                cost = cost[remap]
            move = follow + rhythm[where][1]
            np.fill_diagonal(move, rhythm[where][0])
            total = cost[:, None] + move
            best = total.argmin(axis=0)
            cost = total[best, np.arange(n)] + emit[i]
            back[i] = remap[best] if shift else best
        path = [int(cost.argmin())]
        for i in range(cells - 1, 0, -1):
            path.append(int(back[i][path[-1]]))
        path.reverse()
        return ([((s % 12 + tonic[i]) % 12, s // 12) for i, s in enumerate(path)],
                float(cost.min()))

    def score(self, songs, band, tonic_of=None):
        """The share of labelled cells given their chord, and their root."""
        right = roots = total = 0
        for song in songs:
            c = song.cells(self.per_bar)
            tonic = c.tonic if tonic_of is None else tonic_of(self, song)
            found, _ = self.harmonize(c, tonic, band)
            for i, chord in enumerate(c.truth):
                if chord is None:
                    continue
                s = state(chord[0], chord[1], tonic[i], self.rich)
                want = ((s % 12 + tonic[i]) % 12, s // 12)
                total += 1
                right += found[i] == want
                roots += found[i][0] == want[0]
        return right / total, roots / total

    def tune(self, songs):
        """The melody's weights with the melody alone, then the
        accompaniment's with both."""
        best = 0.0
        for note_weight in (0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0):
            for accent in (0.0, 0.5, 1.0, 2.0):
                self.w[0], self.w[1] = note_weight, accent
                got = self.score(songs, False)[0]
                if got > best:
                    best, keep = got, (note_weight, accent)
        self.w[0], self.w[1] = keep
        best = 0.0
        for around_weight in (0.5, 1.0, 2.0, 4.0, 8.0, 16.0):
            for lowest_weight in (0.0, 0.5, 1.0, 2.0, 4.0):
                self.w[2], self.w[3] = around_weight, lowest_weight
                got = self.score(songs, True)[0]
                if got > best:
                    best, keep = got, (around_weight, lowest_weight)
        self.w[2], self.w[3] = keep


def heard_key(model, song):
    """The one key, of twelve, in which the song's chords come cheapest,
    from the melody alone."""
    c = song.cells(model.per_bar)
    costs = [model.harmonize(c, [t] * len(c.truth), False)[1] for t in range(12)]
    return [int(np.argmin(costs))] * len(c.truth)


def margins(model, songs):
    """How much cheaper a cell the heard key is than the song's own where
    they differ, and than the nearest other key where they agree."""
    own, near = [], []
    for song in songs:
        c = song.cells(model.per_bar)
        if len(set(c.tonic)) > 1:
            continue
        costs = [model.harmonize(c, [t] * len(c.truth), False)[1] / len(c.truth)
                 for t in range(12)]
        best = int(np.argmin(costs))
        if best == c.tonic[0]:
            near.append(sorted(costs)[1] - costs[best])
        else:
            own.append(costs[c.tonic[0]] - costs[best])
    return own, near


# ------------------------------------------------------------------ writing


def number(x):
    # Costs in hundredths of a natural-logarithm unit.
    return str(int(round(min(float(x), 650.0) * 100)))


def table(name, kind, array, doc):
    lines = [f"    /// {line}" for line in doc.split("\n")]
    lines += ["    #[rustfmt::skip]", f"    pub(crate) static {name}: {kind} = ["]
    flat = np.asarray(array)
    if flat.ndim == 1:
        lines.append("        " + ", ".join(number(x) for x in flat) + ",")
    elif flat.ndim == 2:
        for row in flat:
            lines.append("        [" + ", ".join(number(x) for x in row) + "],")
    else:
        for block in flat:
            lines.append("        [" + ", ".join(
                "[" + ", ".join(number(x) for x in row) + "]" for row in block) + "],")
    lines.append("    ];")
    return "\n".join(lines)


def main():
    folder, out = sys.argv[1], sys.argv[2]
    songs = []
    for number_ in sorted(os.listdir(folder)):
        if number_.isdigit():
            song = Song(f"{folder}/{number_}")
            if song.ok:
                song.number = int(number_)
                songs.append(song)
    # Every tenth song is kept aside to tune and to score on.
    aside = [s for s in songs if s.number % 10 == 0]
    learn = [s for s in songs if s.number % 10 != 0]
    print(f"{len(songs)} songs in 4/4 ({len(learn)} counted, {len(aside)} kept aside)")
    parts = []
    report = []
    for rich in (False, True):
        model = Model(learn, rich)
        for per_bar in (True, False):
            model.density(per_bar)
            model.tune(aside)
            chords, roots = model.score(aside, False)
            band, band_roots = model.score(aside, True)
            heard, _ = model.score(aside, False, heard_key)
            agree = np.mean([heard_key(model, s)[0] == s.cells(per_bar).tonic[0] for s in aside])
            line = (f"{'Sevenths too' if rich else 'Plain chords'}, "
                    f"{'one chord a bar' if per_bar else 'changes inside bars'}: "
                    f"from the melody alone {chords:.1%} of cells get the song's chord "
                    f"and {roots:.1%} its root ({heard:.1%} in the key the model hears, the "
                    f"song's own in {agree:.0%} of songs); with the accompaniment "
                    f"{band:.1%} and {band_roots:.1%}")
            print(line, model.w)
            report.append(line)
            own, near = margins(model, aside)
            print("   margin a cell where the heard key is wrong:", np.round(sorted(own), 2))
            print("   margin a cell to the nearest key where it is right:",
                  np.round(np.quantile(near, [0.05, 0.25, 0.5, 0.75]), 2))
        follow, sound, first, spread, _, around, lowest = model.costs()
        with np.errstate(divide="ignore"):
            rhythms = -np.log(model.rhythms)
        n = model.n
        weights = [[int(round(x * 100)) for x in row] for row in model.weights]
        parts.append(f"""/// {'Sevenths and suspended chords too' if rich else 'Major, minor and diminished chords'}.
pub(super) mod {'rich' if rich else 'plain'} {{
{table('FOLLOW', f'[[u16; {n}]; {n}]', follow, 'The cost of each chord (column) after each other chord (row), given a change.')}
{table('SOUND', f'[[u16; 12]; {n}]', sound, 'The cost of a quarter note of melody in each pitch class, counted from the\\ntonic, over each chord.')}
{table('AROUND', f'[[u16; 12]; {n}]', around, 'The same for the accompaniment, a quarter note of all it plays together.')}
{table('LOWEST', f'[[u16; 12]; {n}]', lowest, "The cost of each pitch class as the accompaniment's lowest note in a cell.")}
{table('FIRST', f'[u16; {n}]', first, 'The cost of each chord as the first of a song.')}
{table('EACH', f'[u16; {n}]', spread, 'The cost of each chord anywhere.')}
{table('RHYTHM', '[[[u16; 2]; 2]; 2]', rhythms, 'The cost of holding and of changing the chord, at a bar line and inside a\\nbar, with one cell a bar and with two.')}
    /// Weights in hundredths, with one cell a bar and with two: a quarter
    /// note of melody; what the note struck where a cell starts weighs more
    /// (as a share of its own length, a quarter note at most); a quarter
    /// note of accompaniment; its lowest note.
    pub(crate) const WEIGHTS: [[u32; 4]; 2] = {weights};
}}""")
    header = """// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The harmonizer's numbers, measured from the POP909 dataset (Ziyu Wang,
//! Ke Chen, Junyan Jiang, Yiyi Zhang, Maoran Xu, Shuqi Dai, Guxian Bin and
//! Gus Xia, ISMIR 2020; MIT licence) by `scripts/harmony/measure.py`, which
//! wrote this file. Do not edit it by hand.
//!
//! A chord is `12 * kind + degree`: its kind in `Kind`'s order and its
//! root's semitones above the tonic of the major key the key signature
//! shows (so A minor's chords are counted from C). Costs are minus natural
//! logarithms of how often something came, in hundredths.
//!
//! On the songs kept aside (every tenth):
"""
    for line in report:
        wrapped = textwrap.wrap(line, 72)
        header += "".join(f"//! {'- ' if k == 0 else '  '}{part}\n" for k, part in enumerate(wrapped))
    text = header + "\n" + "\n\n".join(parts) + "\n"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "w").write(text)
    print(f"wrote {out} ({len(text) // 1024} KiB)")


if __name__ == "__main__":
    main()
