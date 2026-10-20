# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
"""python3 scripts/interop/musescore.py MSCORE WRITTEN ORIGINALS...

MuseScore 4 (an independent Guitar Pro reader; MSCORE runs it) opens each
.gp file OxiGuitar wrote (WRITTEN/NAME.EXT.gp, made by the formats crate's
write_gp example) and saves it as MusicXML. It must find what our reader
finds in our file (WRITTEN/NAME.EXT.notes): as many tracks and bars, and
in every bar of every track the same notes, by sounding pitch (drums as
hits). MuseScore keeps beats and voices its own way, so notes are compared
bar by bar, not beat by beat; alphatab.mjs compares beats.

Where MuseScore reads Guitar Pro its own way, less is compared:
- a harmonic counts as a harmonic, whatever its pitch (MuseScore gives a
  harmonic other than a natural one a second note, the fretted one);
- grace notes are left out (MuseScore writes a trill's end as two);
- a bar with a bend isn't compared (MuseScore adds a note per bend), nor
  one longer than its time signature (MuseScore cuts what doesn't fit:
  WRITTEN/NAME.EXT.long lists them);
- a transposing instrument's notes are counted, not compared: MuseScore
  shifts them from what Guitar Pro means, in Guitar Pro's own files too.

A file with a slide from a voice's last note to no note is left out
(WRITTEN/NAME.EXT.musescore-skip says so): MuseScore 4 crashes on it, a
known MuseScore bug, and the slide is right (players slide off a note to
nothing; Mateus, 20 Oct 2026).

And a bar MuseScore reads differently from us passes if MuseScore reads the
original file's bar (looked for in ORIGINALS by name) the same way: then
MuseScore, not our file, differs. A file MuseScore can't open, or crashes
on, fails, unless MuseScore can't open the original either. Exits 1 naming
each file that fails, or when no file could be compared.
"""

import os
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from collections import Counter

STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
LINE = re.compile(r"t(\d+) s(\d+) b(\d+): ?(.*)")


def ours(path):
    """Bar count, and per (track, bar) the notes of every staff and voice."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    bars = int(lines[0].split()[0])
    notes = {}
    for line in lines[1:]:
        m = LINE.fullmatch(line)
        if not m:
            continue
        key = (int(m[1]), int(m[3]))
        found = notes.setdefault(key, Counter())
        for word in m[4].replace("|", " ").split():
            for n in word.split(","):
                if n == "r" or n.endswith("g"):
                    continue
                if "b" in n:
                    found["bend"] += 1
                pitch = n.rstrip("bha")
                if "h" in n:
                    found["h"] += 1
                elif "a" in n:
                    # MuseScore adds the touched note to the fretted one.
                    found[pitch] += 1
                    found["h"] += 1
                else:
                    found[pitch] += 1
    return bars, notes


def theirs(path):
    """Per part: its bars' notes, by sounding pitch ("d" for a drum), and
    whether it is a transposing instrument."""
    root = ET.parse(path).getroot()
    parts = []
    for part in root.findall("part"):
        bars = []
        transposing = any(
            int(t.findtext("chromatic", "0")) % 12 != 0 for t in part.iter("transpose")
        )
        # Written pitch + transposition = sounding pitch (guitars are
        # written an octave up), per staff.
        transpose = {}
        for measure in part.findall("measure"):
            found = Counter()
            for el in measure:
                if el.tag == "attributes":
                    for t in el.findall("transpose"):
                        staff = t.get("number", "all")
                        shift = int(t.findtext("chromatic", "0"))
                        shift += 12 * int(t.findtext("octave-change", "0"))
                        transpose[staff] = shift
                if el.tag != "note" or el.find("grace") is not None:
                    continue
                if el.find("unpitched") is not None:
                    found["d"] += 1
                    continue
                if el.findtext("notehead") == "diamond":
                    found["h"] += 1
                    continue
                pitch = el.find("pitch")
                if pitch is None:
                    continue  # a rest
                midi = (int(pitch.findtext("octave")) + 1) * 12
                midi += STEPS[pitch.findtext("step")]
                midi += round(float(pitch.findtext("alter", "0")))
                staff = el.findtext("staff", "1")
                midi += transpose.get(staff, transpose.get("all", 0))
                found[str(midi)] += 1
            bars.append(+found)
        parts.append((bars, transposing))
    return parts


def convert(mscore, source, target):
    """MuseScore saves SOURCE as TARGET (MusicXML); whether it could."""
    try:
        run = subprocess.run(
            [mscore, "-o", target, source],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=300,
        )
    except subprocess.TimeoutExpired:
        return False
    return run.returncode == 0 and os.path.exists(target)


def same(ours_bar, their_bar, transposing):
    """Whether MuseScore's bar holds our bar's notes, as far as compared."""
    if ours_bar["bend"]:
        return True
    if transposing:
        return sum(ours_bar.values()) == sum(their_bar.values())
    return ours_bar == their_bar


def show(notes):
    return " ".join(sorted(notes.elements(), key=lambda n: (n == "d", n.zfill(3))))


def main():
    if len(sys.argv) < 4:
        sys.exit("usage: musescore.py MSCORE WRITTEN ORIGINALS...")
    mscore, written, originals = sys.argv[1], sys.argv[2], sys.argv[3:]
    out = tempfile.mkdtemp(prefix="musescore-")
    failed = checked = skipped = bugs = 0
    for name in sorted(os.listdir(written)):
        if not name.endswith(".notes"):
            continue
        base = name[: -len(".notes")]
        skip_file = os.path.join(written, base + ".musescore-skip")
        if os.path.isfile(skip_file):
            with open(skip_file, encoding="utf-8") as f:
                print(f"skip {base}: {f.read().strip()} (a MuseScore bug)", flush=True)
            bugs += 1
            continue
        bars, expected = ours(os.path.join(written, name))
        try:
            with open(os.path.join(written, base + ".long"), encoding="utf-8") as f:
                long = {tuple(int(x[1:]) for x in line.split()) for line in f if line.strip()}
        except FileNotFoundError:
            long = set()
        tracks = 1 + max((t for t, _ in expected), default=-1)

        def fail(why):
            nonlocal failed
            print(f"FAIL {base}: {why}", flush=True)
            failed += 1

        path = os.path.join(out, base + ".musicxml")
        original = next(
            (os.path.join(d, base) for d in originals if os.path.isfile(os.path.join(d, base))),
            None,
        )
        theirs_original = None

        def original_bar(t, b):
            """MuseScore's reading of the original's bar, if it has one."""
            nonlocal theirs_original
            if theirs_original is None:
                target = os.path.join(out, base + ".original.musicxml")
                ok = original and convert(mscore, original, target)
                theirs_original = theirs(target) if ok else []
            try:
                return theirs_original[t][0][b]
            except IndexError:
                return None

        if not convert(mscore, os.path.join(written, base + ".gp"), path):
            if original and not convert(mscore, original, os.path.join(out, "original.musicxml")):
                print(f"skip {base}: MuseScore can't open the original either", flush=True)
                skipped += 1
            else:
                fail("MuseScore can't open what we wrote")
            continue
        parts = theirs(path)
        if len(parts) != tracks:
            fail(f"{tracks} tracks, MuseScore finds {len(parts)}")
            continue
        wrong = next((t for t, (p, _) in enumerate(parts) if len(p) != bars), None)
        if wrong is not None:
            fail(f"{bars} bars, MuseScore finds {len(parts[wrong][0])} in track {wrong + 1}")
            continue
        differ = next(
            (
                (t, b)
                for t, (p, transposing) in enumerate(parts)
                for b, found in enumerate(p)
                if (t, b + 1) not in long
                and not same(expected.get((t, b + 1), Counter()), found, transposing)
                and original_bar(t, b) != found
            ),
            None,
        )
        if differ is not None:
            t, b = differ
            fail(
                f"track {t + 1} bar {b + 1}: ours {show(expected.get((t, b + 1), Counter()))}"
                f"\n     MuseScore: {show(parts[t][0][b])}"
            )
            continue
        checked += 1
    summary = f"{checked} files: MuseScore reads what we wrote as we do; {failed} failed" + (
        f"; {skipped} it can't open at all" if skipped else ""
    ) + (f"; {bugs} left out for a MuseScore bug" if bugs else "")
    print(summary)
    if os.environ.get("GITHUB_ACTIONS"):
        print(f"::notice title=MuseScore::{summary}")
    # MuseScore that opens nothing has checked nothing.
    sys.exit(1 if failed or checked == 0 else 0)


if __name__ == "__main__":
    main()
