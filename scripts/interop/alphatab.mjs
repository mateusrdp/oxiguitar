// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
//
// node scripts/interop/alphatab.mjs ORIGINALS... WRITTEN
//
// alphaTab (an independent Guitar Pro reader) reads the .gp file OxiGuitar
// wrote from each original (WRITTEN/NAME.EXT.gp, made by the formats
// crate's write_gp example) and must find the notes our reader found in
// the original (WRITTEN/NAME.EXT.notes): every track's every bar, beat by
// beat, its notes' pitches, the same bars and tracks. So it checks that
// what we write means to another program what it means to us. Our drums
// must each name a General MIDI sound, through the kit we write.
//
// With --originals, alphaTab's reading of each original must match too:
// true of our own fixtures, where alphaTab and our readers agree. Exits 1
// naming each file that differs or that alphaTab can't read.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import * as alphaTab from "@coderline/alphatab";

const args = process.argv.slice(2);
const originalsToo = args[0] === "--originals";
if (originalsToo) args.shift();
const written = args.pop();
if (!written || args.length === 0) {
  console.error("usage: alphatab.mjs ORIGINALS... WRITTEN");
  process.exit(2);
}

const settings = new alphaTab.Settings();

function load(path) {
  return alphaTab.importer.ScoreLoader.loadScoreFromBytes(
    new Uint8Array(readFileSync(path)),
    settings,
  );
}

// A drum note's General MIDI sound, from its track's drum kit. A file
// without a kit (our test generator's originals name drums by MIDI number
// only, which alphaTab doesn't read) gives "d": a hit, sound unknown.
function drum(track, note) {
  const kit = track.percussionArticulations;
  if (!kit || kit.length === 0) return "d";
  const midi = kit[note.percussionArticulation]?.outputMidiNumber;
  if (midi === undefined || midi < 27 || midi > 87) return `d?${midi}`;
  return `d${midi}`;
}

// Whether a note is bent, or tied to a bent note.
function bent(note) {
  for (let n = note; n; n = n.isTieDestination ? n.tieOrigin : null) {
    if (n.hasBend) return true;
  }
  return false;
}

// What a score sounds like, bar by bar: per track, per staff, per bar, the
// beats of each voice with their notes' pitches (drums by their sound,
// bent notes, and notes tied to them, marked b, natural harmonics h,
// other harmonics a, grace notes g), and the bar count.
function notes(score, kitless) {
  const out = [`${score.masterBars.length} bars`];
  for (const track of score.tracks) {
    for (const staff of track.staves) {
      staff.bars.forEach((bar, b) => {
        const voices = bar.voices.map((voice) =>
          voice.beats
            .filter((beat) => !beat.isEmpty)
            .map((beat) =>
              beat.notes
                .map((n) => {
                  let x = n.isPercussion ? (kitless ? "d" : drum(track, n)) : `${n.realValueWithoutHarmonic}`;
                  if (bent(n)) x += "b";
                  if (n.harmonicType === alphaTab.model.HarmonicType.Natural) x += "h";
                  else if (n.harmonicType !== alphaTab.model.HarmonicType.None) x += "a";
                  if (beat.graceType !== alphaTab.model.GraceType.None) x += "g";
                  return x;
                })
                .sort((x, y) => parseInt(x) - parseInt(y) || (x < y ? -1 : x > y ? 1 : 0))
                .join(",") || "r",
            )
            .join(" "),
        );
        out.push(`t${track.index} s${staff.index} b${b + 1}: ${voices.filter((v) => v).join(" | ")}`);
      });
    }
  }
  return out;
}

let failed = 0;
let checked = 0;
for (const dir of args) {
  for (const name of readdirSync(dir).sort()) {
    const original = join(dir, name);
    const ours = join(written, `${name}.gp`);
    if (!statSync(original).isFile()) continue;
    let ourScore;
    try {
      statSync(ours);
    } catch {
      continue; // not a file our readers take
    }
    try {
      ourScore = load(ours);
    } catch (e) {
      console.log(`FAIL ${name}: alphaTab can't read what we wrote: ${e.message ?? e}`);
      failed++;
      continue;
    }
    const differ = (a, b, what) => {
      const first = a.findIndex((line, i) => line !== b[i]);
      if (first < 0 && a.length === b.length) return false;
      const at = first >= 0 ? first : Math.min(a.length, b.length);
      console.log(`FAIL ${name}: ${what} at ${a[at] ?? "(end)"}\n     alphaTab: ${b[at] ?? "(end)"}`);
      failed++;
      return true;
    };
    // Ours against our reader's notes, drums as hits.
    const expected = readFileSync(join(written, `${name}.notes`), "utf8").split("\n");
    if (expected.at(-1) === "") expected.pop();
    if (differ(expected, notes(ourScore, true), "alphaTab reads our file differently from our reader")) {
      continue;
    }
    // Our drums each name a sound in General MIDI's range.
    const odd = notes(ourScore, false).find((line) => line.includes("d?"));
    if (odd) {
      console.log(`FAIL ${name}: a drum without a General MIDI sound: ${odd}`);
      failed++;
      continue;
    }
    if (originalsToo) {
      let theirScore;
      try {
        theirScore = load(original);
      } catch (e) {
        console.log(`FAIL ${name}: alphaTab can't read the original: ${e.message ?? e}`);
        failed++;
        continue;
      }
      // Drums are compared by sound where the original has a drum kit.
      const kitless = theirScore.tracks.some(
        (t) => t.staves.some((s) => s.isPercussion) && t.percussionArticulations.length === 0,
      );
      const theirs = notes(theirScore, kitless);
      if (differ(theirs, notes(ourScore, kitless), "alphaTab reads our file differently from the original")) {
        continue;
      }
    }
    checked++;
  }
}
console.log(`${checked} files: alphaTab reads what we wrote as we do; ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
