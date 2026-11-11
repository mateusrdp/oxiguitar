// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Chords found for a melody (#235, `docs/design/harmonize.md`).
//!
//! The bars to harmonize are cut into cells, a bar or half a bar each. A
//! cell's evidence is what sounds in it. The caret's track is the melody:
//! how long each pitch class sounds, and which notes are struck where the
//! cell starts. Every other pitched track is the accompaniment (Mateus, 10
//! Nov 2026: the notes of all tracks count): the share of the cell each
//! pitch class takes of all they play, and their lowest note. Each chord
//! has a cost in each cell (how seldom such notes sound over it), each
//! pair of chords a cost for following one another, and holding or
//! changing the chord a cost at a bar line and inside a bar. The cheapest
//! chain of chords through the cells (Viterbi) is written on the caret's
//! track as chord diagrams, one wherever the chord changes.
//!
//! The costs were counted in the POP909 dataset ([`numbers`]). Chords are
//! counted from the tonic of the major key the bars' key signature shows,
//! so a song in A minor is harmonized with C major's numbers, where the
//! chord on the sixth degree is the commonest: the tool never has to tell
//! a major key from its relative minor.

use std::collections::BTreeMap;

use oxiguitar_model::{BeatKind, ChordDiagram, NotePlace, Ratio, Score, Staff};

#[cfg(test)]
use crate::harmony::NOTE_NAMES;
use crate::harmony::{Chord, DEFAULT_NAME, Extension, Triad, alt};
use crate::{Caret, Command, Edited, History, Region, Selection};

mod numbers;

/// What Harmonize does, and to what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harmonize {
    /// Sevenths and suspended chords too, rather than major, minor and
    /// diminished chords only.
    pub sevenths: bool,
    /// The chord may change in the middle of a bar, rather than at bar
    /// lines only.
    pub inside_bars: bool,
    /// Chords already there stay, and the new ones are fitted around
    /// them, rather than being replaced.
    pub keep: bool,
    /// The caret's whole track (Tools), rather than the selected bars or
    /// the caret's bar (the page's right-click menu).
    pub whole_track: bool,
}

/// A kind of chord, in the order of [`numbers`]: the first three are the
/// plain vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Major,
    Minor,
    Diminished,
    MajorSeventh,
    MinorSeventh,
    Seventh,
    HalfDiminished,
    Sus4,
    SeventhSus4,
}

const KINDS: [Kind; 9] = [
    Kind::Major,
    Kind::Minor,
    Kind::Diminished,
    Kind::MajorSeventh,
    Kind::MinorSeventh,
    Kind::Seventh,
    Kind::HalfDiminished,
    Kind::Sus4,
    Kind::SeventhSus4,
];
/// How many kinds the plain vocabulary has.
const PLAIN: usize = 3;

impl Kind {
    /// The kind without its seventh; `None` for a chord without a third,
    /// which takes the third its key gives its root.
    fn plain(self) -> Option<Kind> {
        match self {
            Kind::Major | Kind::MajorSeventh | Kind::Seventh => Some(Kind::Major),
            Kind::Minor | Kind::MinorSeventh => Some(Kind::Minor),
            Kind::Diminished | Kind::HalfDiminished => Some(Kind::Diminished),
            Kind::Sus4 | Kind::SeventhSus4 => None,
        }
    }

    /// The triad the major key gives the root `degree` semitones above its
    /// tonic.
    fn diatonic(degree: u8) -> Kind {
        match degree {
            2 | 4 | 9 => Kind::Minor,
            11 => Kind::Diminished,
            _ => Kind::Major,
        }
    }

    /// The builder's triad, extension and alterations.
    fn parts(self) -> (Triad, Extension, u8) {
        match self {
            Kind::Major => (Triad::Major, Extension::None, 0),
            Kind::Minor => (Triad::Minor, Extension::None, 0),
            Kind::Diminished => (Triad::Diminished, Extension::None, 0),
            Kind::MajorSeventh => (Triad::Major, Extension::MajorSeventh, 0),
            Kind::MinorSeventh => (Triad::Minor, Extension::Seventh, 0),
            Kind::Seventh => (Triad::Major, Extension::Seventh, 0),
            Kind::HalfDiminished => (Triad::Minor, Extension::Seventh, alt::FLAT_FIVE),
            Kind::Sus4 => (Triad::Sus4, Extension::None, 0),
            Kind::SeventhSus4 => (Triad::Sus4, Extension::Seventh, 0),
        }
    }

    /// The kind nearest a chord of the builder; `None` for a chord
    /// without a third and without a kind of its own (sus2, a power
    /// chord), which takes the third its key gives its root.
    fn of(chord: &Chord) -> Option<Kind> {
        let seventh = matches!(
            chord.extension,
            Extension::Seventh | Extension::Ninth | Extension::Eleventh | Extension::Thirteenth
        );
        let major_seventh = matches!(
            chord.extension,
            Extension::MajorSeventh | Extension::MajorNinth
        );
        Some(match chord.triad {
            Triad::Major | Triad::Augmented if major_seventh => Kind::MajorSeventh,
            Triad::Major | Triad::Augmented if seventh => Kind::Seventh,
            Triad::Major | Triad::Augmented => Kind::Major,
            Triad::Minor if seventh && chord.alterations & alt::FLAT_FIVE != 0 => {
                Kind::HalfDiminished
            }
            Triad::Minor if seventh => Kind::MinorSeventh,
            Triad::Minor => Kind::Minor,
            Triad::Diminished => Kind::Diminished,
            Triad::Sus4 if seventh => Kind::SeventhSus4,
            Triad::Sus4 => Kind::Sus4,
            Triad::Sus2 | Triad::Power => return None,
        })
    }
}

/// The numbers of one vocabulary.
struct Numbers {
    kinds: usize,
    follow: &'static [&'static [u16]],
    sound: &'static [[u16; 12]],
    around: &'static [[u16; 12]],
    lowest: &'static [[u16; 12]],
    first: &'static [u16],
    each: &'static [u16],
    rhythm: &'static [[[u16; 2]; 2]; 2],
    weights: [[u32; 4]; 2],
}

impl Numbers {
    fn of(sevenths: bool) -> Numbers {
        // The tables' rows as slices, so both vocabularies have one type.
        static PLAIN_ROWS: std::sync::LazyLock<Vec<&'static [u16]>> =
            std::sync::LazyLock::new(|| numbers::plain::FOLLOW.iter().map(|r| &r[..]).collect());
        static RICH_ROWS: std::sync::LazyLock<Vec<&'static [u16]>> =
            std::sync::LazyLock::new(|| numbers::rich::FOLLOW.iter().map(|r| &r[..]).collect());
        if sevenths {
            Numbers {
                kinds: KINDS.len(),
                follow: &RICH_ROWS,
                sound: &numbers::rich::SOUND,
                around: &numbers::rich::AROUND,
                lowest: &numbers::rich::LOWEST,
                first: &numbers::rich::FIRST,
                each: &numbers::rich::EACH,
                rhythm: &numbers::rich::RHYTHM,
                weights: numbers::rich::WEIGHTS,
            }
        } else {
            Numbers {
                kinds: PLAIN,
                follow: &PLAIN_ROWS,
                sound: &numbers::plain::SOUND,
                around: &numbers::plain::AROUND,
                lowest: &numbers::plain::LOWEST,
                first: &numbers::plain::FIRST,
                each: &numbers::plain::EACH,
                rhythm: &numbers::plain::RHYTHM,
                weights: numbers::plain::WEIGHTS,
            }
        }
    }

    /// How many chords there are: every kind on every root.
    fn states(&self) -> usize {
        12 * self.kinds
    }

    /// The state of the chord of `kind` on the pitch class `root`, in
    /// this vocabulary.
    fn state(&self, root: u8, kind: Option<Kind>, tonic: u8) -> usize {
        let degree = (root + 12 - tonic % 12) % 12;
        let mut kind = kind.unwrap_or_else(|| Kind::diatonic(degree));
        if self.kinds == PLAIN {
            kind = kind.plain().unwrap_or_else(|| Kind::diatonic(degree));
        }
        12 * KINDS.iter().position(|k| *k == kind).unwrap_or(0) + usize::from(root % 12)
    }
}

/// A chord (a kind on a root's pitch class) counted from `tonic`, as the
/// tables index it.
fn relative(state: usize, tonic: u8) -> usize {
    12 * (state / 12) + (state % 12 + 12 - usize::from(tonic % 12)) % 12
}

/// A bar or half a bar.
#[derive(Clone, Debug)]
struct Cell {
    bar: usize,
    /// From the bar's start, in whole notes.
    start: Ratio,
    end: Ratio,
    /// The tonic of the major key the bar's key signature shows.
    tonic: u8,
    /// Quarter notes each pitch class sounds in the caret's track.
    weights: [f64; 12],
    /// Its notes struck where the cell starts, each for its length (a
    /// quarter note at most).
    accents: [f64; 12],
    /// Quarter notes each pitch class sounds in the other pitched tracks.
    around: [f64; 12],
    /// Their lowest written pitch.
    lowest: Option<i32>,
    /// The voice and beat of the caret's staff that start where the cell
    /// does: where a chord can be written.
    anchor: Option<(usize, usize)>,
    /// With chords kept: the chord already in the cell (`Some(None)` for
    /// one whose name isn't a chord the builder makes).
    kept: Option<Option<usize>>,
}

impl Cell {
    fn sounds(&self) -> bool {
        self.weights.iter().chain(&self.around).any(|w| *w > 0.0)
    }

    /// Its length in quarter notes.
    fn quarters(&self) -> f64 {
        self.end
            .checked_sub(self.start)
            .map_or(0.0, |l| l.to_f64() * 4.0)
    }
}

/// The tonic's pitch class of the major key with `fifths` sharps (flats
/// when negative).
fn tonic(fifths: i8) -> u8 {
    (i32::from(fifths) * 7).rem_euclid(12) as u8
}

/// A note's written pitch on `staff`.
fn written(staff: &Staff, place: NotePlace) -> Option<i32> {
    let pitch = match place {
        NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret)?,
        NotePlace::Pitched(p) => p,
    };
    Some(i32::from(pitch.0) + i32::from(staff.transposition))
}

/// Bars `first..=last` cut into cells, with what sounds in each and where
/// the caret's staff can take a chord.
fn cells(score: &Score, c: Caret, first: usize, last: usize, h: &Harmonize) -> Vec<Cell> {
    let mut out = Vec::new();
    let Some(own) = score.tracks.get(c.track) else {
        return out;
    };
    for bar in first..=last {
        let Some(master) = score.master_bars.get(bar) else {
            break;
        };
        let Some(length) = master.time_signature.bar_length() else {
            continue;
        };
        if length <= Ratio::ZERO {
            continue;
        }
        let half = length.checked_div(Ratio::from_integer(2));
        // Two cells where the bar has two halves of a dotted quarter or
        // more (4/4, 2/2, 6/8, 12/8; not 2/4 or 3/4).
        let halves = h.inside_bars
            && master.time_signature.numerator % 2 == 0
            && half.is_some_and(|x| Ratio::new(3, 8).is_some_and(|least| x >= least));
        let cuts = match half {
            Some(half) if halves => vec![(Ratio::ZERO, half), (half, length)],
            _ => vec![(Ratio::ZERO, length)],
        };
        let key = tonic(master.key_signature.fifths);
        let from = out.len();
        for (start, end) in cuts {
            out.push(Cell {
                bar,
                start,
                end,
                tonic: key,
                weights: [0.0; 12],
                accents: [0.0; 12],
                around: [0.0; 12],
                lowest: None,
                anchor: None,
                kept: None,
            });
        }
        let Some(these) = out.get_mut(from..) else {
            continue;
        };
        // What sounds, in every pitched track.
        for (t, track) in score.tracks.iter().enumerate() {
            if track.instrument.percussion {
                continue;
            }
            let melody = t == c.track;
            for staff in &track.staves {
                let Some(b) = staff.bars.get(bar) else {
                    continue;
                };
                for voice in &b.voices {
                    let starts = crate::onsets(voice);
                    for (beat, at) in voice.beats.iter().zip(starts) {
                        if beat.kind != BeatKind::Notes || beat.grace.is_some() {
                            continue;
                        }
                        let Some(until) = beat.duration.length().and_then(|l| at.checked_add(l))
                        else {
                            continue;
                        };
                        for cell in these.iter_mut() {
                            let (from, to) = (at.max(cell.start), until.min(cell.end));
                            let Some(shared) = to.checked_sub(from).filter(|s| *s > Ratio::ZERO)
                            else {
                                continue;
                            };
                            let quarters = shared.to_f64() * 4.0;
                            for note in beat.notes.iter().filter(|n| !n.effects.dead) {
                                let Some(pitch) = written(staff, note.place) else {
                                    continue;
                                };
                                let pc = pitch.rem_euclid(12) as usize;
                                if !melody {
                                    if let Some(w) = cell.around.get_mut(pc) {
                                        *w += quarters;
                                    }
                                    if cell.lowest.is_none_or(|l| pitch < l) {
                                        cell.lowest = Some(pitch);
                                    }
                                    continue;
                                }
                                if let Some(w) = cell.weights.get_mut(pc) {
                                    *w += quarters;
                                }
                                if at == cell.start
                                    && !note.tie
                                    && let Some(a) = cell.accents.get_mut(pc)
                                {
                                    *a += quarters.min(1.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Where a chord can go, and the chords already there.
        for (st, staff) in own.staves.iter().enumerate() {
            let Some(b) = staff.bars.get(bar) else {
                continue;
            };
            for (v, voice) in b.voices.iter().enumerate() {
                let starts = crate::onsets(voice);
                for (i, (beat, at)) in voice.beats.iter().zip(starts).enumerate() {
                    if beat.grace.is_some() {
                        continue;
                    }
                    for cell in these.iter_mut() {
                        if st == c.staff
                            && at == cell.start
                            && cell.anchor.is_none()
                            && beat.kind != BeatKind::Empty
                        {
                            cell.anchor = Some((v, i));
                        }
                        if h.keep
                            && at >= cell.start
                            && at < cell.end
                            && let Some(d) = beat.chord.and_then(|i| own.chords.get(i))
                        {
                            cell.kept = Some(Chord::parse(&d.name).map(|chord| {
                                Numbers::of(h.sevenths).state(
                                    chord.root_class(),
                                    Kind::of(&chord),
                                    cell.tonic,
                                )
                            }));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The cheapest chord for each cell of `cells` (a chord's kind and its
/// root's pitch class as `12 * kind + root`), with the chain's cost.
/// `density` is 0 with one cell a bar and 1 with two; `start` says the
/// first cell starts the song. `key` puts every cell in that key and
/// ignores kept chords: the chain as it would be in another key.
fn chain(
    cells: &[Cell],
    n: &Numbers,
    density: usize,
    start: bool,
    key: Option<u8>,
) -> (Vec<usize>, f64) {
    let states = n.states();
    let [note_weight, accent, around_weight, lowest_weight] = n
        .weights
        .get(density)
        .copied()
        .unwrap_or([100, 0, 100, 100])
        .map(|w| f64::from(w) / 100.0);
    let mut cost = vec![0.0f64; states];
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(cells.len());
    for (i, cell) in cells.iter().enumerate() {
        let tonic = key.unwrap_or(cell.tonic);
        let kept = if key.is_some() { None } else { cell.kept };
        // What the cell's notes cost over each chord.
        let emit = |s: usize| -> f64 {
            if let Some(kept) = kept {
                return if kept.is_none_or(|k| k == s) {
                    0.0
                } else {
                    f64::INFINITY
                };
            }
            let chord = relative(s, tonic);
            let price = |table: &[[u16; 12]], from_tonic: usize| {
                let p = table.get(chord).and_then(|r| r.get(from_tonic));
                f64::from(p.copied().unwrap_or(0))
            };
            // All the accompaniment plays together counts as one voice as
            // long as the cell, however many tracks and notes it is.
            let band: f64 = cell.around.iter().sum();
            let share = if band > 0.0 {
                cell.quarters() / band
            } else {
                0.0
            };
            let mut total = 0.0;
            for pc in 0..12usize {
                let from_tonic = (pc + 12 - usize::from(tonic % 12)) % 12;
                let melody = cell.weights.get(pc).copied().unwrap_or(0.0)
                    + accent * cell.accents.get(pc).copied().unwrap_or(0.0);
                total += melody * note_weight * price(n.sound, from_tonic);
                let around = cell.around.get(pc).copied().unwrap_or(0.0) * share;
                total += around * around_weight * price(n.around, from_tonic);
            }
            if let Some(low) = cell.lowest {
                let from_tonic = (low - i32::from(tonic)).rem_euclid(12) as usize;
                total += lowest_weight * price(n.lowest, from_tonic);
            }
            total
        };
        if i == 0 {
            let table = if start { n.first } else { n.each };
            for (s, slot) in cost.iter_mut().enumerate() {
                let prior = table.get(relative(s, tonic)).copied().unwrap_or(0);
                *slot = f64::from(prior) + emit(s);
            }
            back.push(Vec::new());
            continue;
        }
        let before = cells.get(i - 1);
        let at_bar_line = before.is_none_or(|b| b.bar != cell.bar);
        let [hold, change] = n
            .rhythm
            .get(density)
            .and_then(|d| d.get(usize::from(!at_bar_line)))
            .copied()
            .unwrap_or([0, 0]);
        // A chord can change only where it can be written and something
        // sounds, or where a chord is kept.
        let free = kept.is_some() || (cell.anchor.is_some() && cell.sounds());
        // A chord nobody can name breaks the chain.
        let broken =
            kept == Some(None) || before.is_some_and(|b| key.is_none() && b.kept == Some(None));
        let mut new = vec![f64::INFINITY; states];
        let mut from = vec![0usize; states];
        for s in 0..states {
            let mut best = cost.get(s).copied().unwrap_or(f64::INFINITY) + f64::from(hold);
            let mut source = s;
            if broken {
                for (p, c) in cost.iter().enumerate() {
                    if *c < best {
                        best = *c;
                        source = p;
                    }
                }
            } else if free {
                let column = relative(s, tonic);
                for (p, c) in cost.iter().enumerate() {
                    if p == s {
                        continue;
                    }
                    let step = n
                        .follow
                        .get(relative(p, tonic))
                        .and_then(|r| r.get(column))
                        .copied()
                        .unwrap_or(u16::MAX);
                    let total = c + f64::from(change) + f64::from(step);
                    if total < best {
                        best = total;
                        source = p;
                    }
                }
            }
            if let (Some(slot), Some(f)) = (new.get_mut(s), from.get_mut(s)) {
                *slot = best + emit(s);
                *f = source;
            }
        }
        cost = new;
        back.push(from);
    }
    let mut at = 0;
    let mut least = f64::INFINITY;
    for (s, c) in cost.iter().enumerate() {
        if *c < least {
            least = *c;
            at = s;
        }
    }
    let mut path = vec![at; cells.len()];
    for i in (1..cells.len()).rev() {
        at = back.get(i).and_then(|b| b.get(at)).copied().unwrap_or(at);
        if let Some(slot) = path.get_mut(i - 1) {
            *slot = at;
        }
    }
    (path, least)
}

/// The name of the root `degree` semitones above the tonic of the major
/// key with `fifths` sharps, for a chord of `kind`: as the key spells it,
/// the notes between its degrees as flats (♭III, ♭VI, ♭VII; ♯IV), or as
/// sharps under a diminished chord (a leading tone).
fn root_name(root: u8, kind: Kind, fifths: i8) -> usize {
    let degree = (root + 12 - tonic(fifths)) % 12;
    let leading = matches!(kind, Kind::Diminished | Kind::HalfDiminished);
    // Steps along the line of fifths from the tonic.
    let steps: i32 = match degree {
        0 => 0,
        1 if leading => 7,
        1 => -5,
        2 => 2,
        3 if leading => 9,
        3 => -3,
        4 => 4,
        5 => -1,
        6 => 6,
        7 => 1,
        8 if leading => 8,
        8 => -4,
        9 => 3,
        10 => -2,
        _ => 5,
    };
    // The line of fifths from G flat to A sharp, as indices of NOTE_NAMES.
    const LINE: [usize; 17] = [9, 2, 12, 5, 15, 7, 0, 10, 3, 13, 6, 16, 8, 1, 11, 4, 14];
    usize::try_from(i32::from(fifths) + steps + 6)
        .ok()
        .and_then(|i| LINE.get(i))
        .copied()
        .unwrap_or_else(|| {
            DEFAULT_NAME
                .get(usize::from(root % 12))
                .copied()
                .unwrap_or(0)
        })
}

/// The chord of a state, spelled for the key with `fifths` sharps.
fn chord_of(state: usize, fifths: i8) -> Chord {
    let kind = KINDS.get(state / 12).copied().unwrap_or(Kind::Major);
    let (triad, extension, alterations) = kind.parts();
    Chord {
        root: root_name((state % 12) as u8, kind, fifths),
        triad,
        extension,
        alterations,
        bass: None,
    }
}

/// The chord's diagram on `staff`: its name, and its easiest shape where
/// the staff has strings.
fn diagram(chord: &Chord, staff: &Staff) -> ChordDiagram {
    let tuning: Vec<u8> = staff
        .tuning
        .iter()
        .map(|p| p.0.saturating_add(staff.capo))
        .collect();
    let reach = staff.fret_count.saturating_sub(staff.capo);
    let frets = chord
        .voicings(&tuning, reach, 1)
        .into_iter()
        .next()
        .unwrap_or_default();
    let high = frets.iter().flatten().max().copied().unwrap_or(0);
    let low = frets.iter().flatten().filter(|f| **f > 0).min().copied();
    ChordDiagram {
        name: chord.name(),
        // The grid starts at the nut when the shape fits the first five
        // frets, as the chord builder's does.
        first_fret: if high <= 5 { 1 } else { low.unwrap_or(1) },
        frets,
    }
}

/// What the harmonizer would do.
struct Plan {
    first: usize,
    last: usize,
    /// The chords to write: bar, voice, beat and diagram.
    chords: Vec<(usize, usize, usize, ChordDiagram)>,
    /// The key signatures the bars have (sharps; flats negative).
    keys: Vec<i8>,
    /// A key (sharps) the notes fit clearly better than the one key
    /// signature the bars have.
    better: Option<i8>,
}

/// The bars a command covers.
fn bars(
    score: &Score,
    c: Caret,
    selection: Option<Selection>,
    h: &Harmonize,
) -> Option<(usize, usize)> {
    let count = score.bar_count();
    let (first, last) = if h.whole_track {
        (0, count.checked_sub(1)?)
    } else {
        selection.map_or((c.bar, c.bar), |s| s.bars())
    };
    (first <= last && last < count).then_some((first, last))
}

/// How much cheaper a cell (in hundredths) another key must make the
/// chain before the report says so. In POP909, where the cheapest key was
/// not the song's, it was cheaper by 0.03 to 0.05 a cell; where it was the
/// song's, the next key was dearer by 0.2 to 0.6.
const CLEARLY: f64 = 15.0;

fn plan(score: &Score, c: Caret, selection: Option<Selection>, h: &Harmonize) -> Option<Plan> {
    let (first, last) = bars(score, c, selection, h)?;
    let track = score.tracks.get(c.track)?;
    if track.instrument.percussion {
        return None;
    }
    let staff = track.staves.get(c.staff)?;
    let all = cells(score, c, first, last, h);
    // From the first cell where something sounds to the last.
    let from = all.iter().position(Cell::sounds)?;
    let to = all.iter().rposition(Cell::sounds)?;
    let cells = all.get(from..=to)?;
    let n = Numbers::of(h.sevenths);
    let density = usize::from(h.inside_bars);
    let start = cells.first().is_some_and(|c| c.bar == 0);
    let (path, _) = chain(cells, &n, density, start, None);
    let fifths = |bar: usize| {
        score
            .master_bars
            .get(bar)
            .map_or(0, |m| m.key_signature.fifths)
    };
    let mut chords = Vec::new();
    let mut shown: Option<usize> = None;
    let mut made: BTreeMap<(usize, i8), ChordDiagram> = BTreeMap::new();
    for (cell, state) in cells.iter().zip(&path) {
        match cell.kept {
            Some(kept) => shown = kept,
            None => {
                if let Some((voice, beat)) = cell.anchor
                    && shown != Some(*state)
                {
                    let key = fifths(cell.bar);
                    let d = made
                        .entry((*state, key))
                        .or_insert_with(|| diagram(&chord_of(*state, key), staff));
                    chords.push((cell.bar, voice, beat, d.clone()));
                    shown = Some(*state);
                }
            }
        }
    }
    let mut keys: Vec<i8> = cells.iter().map(|c| fifths(c.bar)).collect();
    keys.dedup();
    // Whether another key fits the notes clearly better, heard with the
    // plain chords (enough to tell, and quick).
    let mut better = None;
    if let [only] = keys.as_slice() {
        let plain = Numbers::of(false);
        let cost = |key: u8| chain(cells, &plain, density, start, Some(key)).1;
        let own = cost(tonic(*only));
        let (best, least) = (0..12u8)
            .map(|k| (k, cost(k)))
            .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
        if own - least > CLEARLY * cells.len() as f64 {
            // From six flats to five sharps, as Infer Key Signatures.
            let sharps = (i32::from(best) * 7).rem_euclid(12);
            better = i8::try_from(if sharps > 5 { sharps - 12 } else { sharps }).ok();
        }
    }
    Some(Plan {
        first,
        last,
        chords,
        keys,
        better,
    })
}

/// A key signature's two keys, such as "G major or E minor".
fn key_name(fifths: i8) -> String {
    const MAJOR: [&str; 15] = [
        "Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#",
    ];
    const MINOR: [&str; 15] = [
        "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#",
    ];
    let at = usize::try_from(i32::from(fifths) + 7).unwrap_or(7);
    format!(
        "{} major or {} minor",
        MAJOR.get(at).copied().unwrap_or("C"),
        MINOR.get(at).copied().unwrap_or("A")
    )
}

/// What [`Command::Harmonize`] would say it did at `c`, for the footer;
/// `None` when it would do nothing (no notes in the bars, a drum track).
pub fn harmonize_report(
    score: &Score,
    c: Caret,
    selection: Option<Selection>,
    h: &Harmonize,
) -> Option<String> {
    let plan = plan(score, c, selection, h)?;
    let track = score.tracks.get(c.track)?;
    let bars = if plan.first == plan.last {
        format!("bar {}", plan.first + 1)
    } else {
        format!("bars {}–{}", plan.first + 1, plan.last + 1)
    };
    let count = match plan.chords.len() {
        0 => "No new chords".to_owned(),
        1 => "1 chord found".to_owned(),
        n => format!("{n} chords found"),
    };
    let key = match plan.keys.as_slice() {
        [only] => format!("in {} (the key signature)", key_name(*only)),
        _ => "in the keys of the bars' key signatures".to_owned(),
    };
    let mut text = format!("{count} for {bars} of {}, {key}.", track.name);
    if let Some(better) = plan.better {
        text.push_str(&format!(
            " The notes fit {} better: if the key signature is wrong, run Infer Key Signatures and harmonize again.",
            key_name(better)
        ));
    }
    Some(text)
}

/// Runs [`Command::Harmonize`]; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let Command::Harmonize(h) = command else {
        return None;
    };
    let Some(plan) = plan(score, c, selection, h) else {
        return Some(None);
    };
    let mut work = score.clone();
    let Some(track) = work.tracks.get_mut(c.track) else {
        return Some(None);
    };
    if !h.keep {
        for staff in &mut track.staves {
            for bar in staff.bars.iter_mut().take(plan.last + 1).skip(plan.first) {
                for beat in bar.voices.iter_mut().flat_map(|v| &mut v.beats) {
                    beat.chord = None;
                }
            }
        }
    }
    for (bar, voice, beat, diagram) in plan.chords {
        let index = match track.chords.iter().position(|d| *d == diagram) {
            Some(i) => i,
            None => {
                track.chords.push(diagram);
                track.chords.len() - 1
            }
        };
        if let Some(b) = track
            .staves
            .get_mut(c.staff)
            .and_then(|s| s.bars.get_mut(bar))
            .and_then(|b| b.voices.get_mut(voice))
            .and_then(|v| v.beats.get_mut(beat))
        {
            b.chord = Some(index);
        }
    }
    let staves = track.staves.len();
    if work == *score {
        return Some(None);
    }
    let mut regions = vec![Region::Track(c.track)];
    regions.extend((0..staves).map(|staff| Region::Bars {
        track: c.track,
        staff,
        bars: plan.first..plan.last + 1,
    }));
    let touched = history.record(score, command.label(), regions, |s| *s = work);
    Some(Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    }))
}

#[cfg(test)]
mod tests;
