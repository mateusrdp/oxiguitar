// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Fit Bars to Time Signature (#222) on every Guitar Pro file of the
//! private corpus, where `corpus/files/` exists (skipped quietly otherwise):
//! `cargo test --release -p oxiguitar-edit --test fit_corpus -- --nocapture`
//!
//! Each file's time signatures are changed (n/d to (n-1)/d, 2/d to 3/d), as
//! a user changing their mind would, and every bar is fitted again:
//!
//! - re-barred, every note of every voice is struck at the same moment as
//!   before, and no bar is longer than its time signature. Files whose odd
//!   tuplets leave lengths no note value can show are the exception: such
//!   a sliver of a note stays whole, and the notes after it in its bar
//!   move by less than a 64th note (the test counts them);
//! - stretched, every voice strikes the same notes in the same order, the
//!   bar count stays, and every bar fitted adds up.

use std::path::{Path, PathBuf};
use std::time::Instant;

use oxiguitar_edit::{BarCheck, Caret, Command, Fit, FitHow, History};
use oxiguitar_model::{BeatKind, NotePlace, Ratio, Score, TimeSignature};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Each bar's place on the time line, as Fit reads it: as long as its
/// longest voice holding music, or its time signature if it holds none.
fn bar_starts(score: &Score) -> Vec<Ratio> {
    let mut out = Vec::new();
    let mut at = Ratio::ZERO;
    for (k, m) in score.master_bars.iter().enumerate() {
        out.push(at);
        let longest = score
            .tracks
            .iter()
            .flat_map(|t| &t.staves)
            .filter_map(|s| s.bars.get(k))
            .flat_map(|b| &b.voices)
            .filter(|v| {
                v.beats
                    .iter()
                    .any(|b| matches!(b.kind, BeatKind::Notes | BeatKind::Rest))
            })
            .filter_map(|v| v.length())
            .max()
            .unwrap_or(Ratio::ZERO);
        let len = if longest == Ratio::ZERO {
            m.expected_length().unwrap_or(Ratio::ZERO)
        } else {
            longest
        };
        at = at + len;
    }
    out
}

/// A struck beat: its moment, and its notes' places.
type Struck = (Ratio, Vec<(u8, u8)>);

/// Every struck beat of each staff and voice, with its moment on the time
/// line (`timed`) or without it.
fn struck(score: &Score, timed: bool) -> Vec<Vec<Struck>> {
    let starts = bar_starts(score);
    let mut out = Vec::new();
    for s in score.tracks.iter().flat_map(|t| &t.staves) {
        let voices = s.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
        for v in 0..voices {
            let mut line = Vec::new();
            for (k, bar) in s.bars.iter().enumerate() {
                let Some(voice) = bar.voices.get(v) else {
                    continue;
                };
                let mut at = starts[k];
                for b in &voice.beats {
                    if b.grace.is_some() {
                        continue;
                    }
                    if b.kind == BeatKind::Notes && !b.notes.iter().all(|n| n.tie) {
                        let places = b
                            .notes
                            .iter()
                            .filter(|n| !n.tie)
                            .map(|n| match n.place {
                                NotePlace::Fretted { string, fret } => (string, fret),
                                NotePlace::Pitched(p) => (0, p.0),
                            })
                            .collect();
                        line.push((if timed { at } else { Ratio::ZERO }, places));
                    }
                    at = at + b.duration.length().unwrap_or(Ratio::ZERO);
                }
            }
            out.push(line);
        }
    }
    out
}

/// The time signatures a user changing their mind might pick.
fn changed(ts: TimeSignature) -> TimeSignature {
    TimeSignature {
        numerator: if ts.numerator > 2 {
            ts.numerator - 1
        } else {
            ts.numerator + 1
        },
        ..ts
    }
}

#[test]
fn fitting_runs_on_the_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let mut done = 0;
    let mut moved_total = 0;
    for path in all {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_plugin_test::released::guitar_pro::read(&data) else {
            continue;
        };
        let mut score = imported.score;
        if score.bar_count() == 0 || score.check_structure().is_err() {
            continue;
        }
        for m in &mut score.master_bars {
            m.time_signature = changed(m.time_signature);
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let red = BarCheck::of(&score).len();
        let Some(c) = Caret::at_start(&score, 0) else {
            continue;
        };

        // Re-bar.
        let mut rebarred = score.clone();
        let mut h = History::new();
        let t0 = Instant::now();
        let fit = Fit {
            how: FitHow::Rebar,
            whole_song: true,
        };
        oxiguitar_edit::apply(&mut rebarred, &mut h, c, None, Command::FitBars(fit));
        let rebar_time = t0.elapsed();
        assert!(rebarred.check_structure().is_ok(), "{name}");
        let (before, after) = (struck(&score, true), struck(&rebarred, true));
        assert_eq!(before.len(), after.len(), "{name}: staves and voices");
        let tiny = Ratio::new(1, 64).unwrap();
        let mut moved = 0;
        for (i, (b, a)) in before.iter().zip(&after).enumerate() {
            assert_eq!(b.len(), a.len(), "{name}: line {i} strikes as many beats");
            for (j, (x, y)) in b.iter().zip(a).enumerate() {
                assert_eq!(x.1, y.1, "{name}: line {i}, struck beat {j}");
                let d = if x.0 > y.0 { x.0 - y.0 } else { y.0 - x.0 };
                assert!(d < tiny, "{name}: line {i}, struck beat {j} moved by {d}");
                if d > Ratio::ZERO {
                    moved += 1;
                }
            }
        }
        moved_total += moved;
        let left = BarCheck::of(&rebarred);
        // A bar left red must be short: the end of a passage, or a pickup.
        for at in left.iter() {
            let m = &rebarred.master_bars[at.bar];
            let want = m.expected_length().unwrap();
            let bar = &rebarred.tracks[at.track].staves[at.staff].bars[at.bar];
            for v in &bar.voices {
                if let Some(len) = v.length()
                    && v.beats.iter().any(|b| b.kind != BeatKind::Empty)
                {
                    assert!(
                        len < want + tiny,
                        "{name}: bar {} too long after",
                        at.bar + 1
                    );
                }
            }
        }

        // Stretch.
        let mut stretched = score.clone();
        let fit = Fit {
            how: FitHow::Stretch,
            whole_song: true,
        };
        let t0 = Instant::now();
        oxiguitar_edit::apply(&mut stretched, &mut h, c, None, Command::FitBars(fit));
        let stretch_time = t0.elapsed();
        assert!(stretched.check_structure().is_ok(), "{name}");
        assert_eq!(stretched.bar_count(), score.bar_count(), "{name}");
        assert_eq!(
            struck(&score, false),
            struck(&stretched, false),
            "{name}: stretching strikes the same notes"
        );
        let still = BarCheck::of(&stretched).len();
        println!(
            "{name}: {red} red of {} bars; re-barred {} bars, {} red, {moved} beats moved, {:.3} s; stretched, {still} red, {:.3} s",
            score.bar_count(),
            rebarred.bar_count(),
            left.len(),
            rebar_time.as_secs_f64(),
            stretch_time.as_secs_f64(),
        );
        done += 1;
    }
    println!("{done} files fitted; {moved_total} struck beats moved by a sliver");
}
