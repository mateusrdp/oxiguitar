// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Automatic fingering (#157) on every Guitar Pro file of the private
//! corpus, where `corpus/files/` exists (skipped quietly otherwise):
//! `cargo test --release -p oxiguitar-edit --test fingering_corpus -- --nocapture`
//!
//! Each file's tab is taken away (every note becomes its pitch), found again
//! as a format without tablature would have it, and checked: every note
//! sounds as it did. Then Find Fingering runs on every track. It prints how
//! many notes landed where the transcriber put them, how many beats are
//! marked hard, and how long it took.

use std::path::{Path, PathBuf};
use std::time::Instant;

use oxiguitar_edit::{Caret, Command, HARD_FINGERS, HARD_SHIFT, HARD_STRETCH, History, Scope};
use oxiguitar_model::{NotePlace, Score};

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

/// Every note's sounding pitch (with its place, if on a string), staff by
/// staff, for staves with strings outside drum tracks.
fn notes(score: &Score) -> Vec<(u8, Option<(u8, u8)>)> {
    let mut out = Vec::new();
    for t in score.tracks.iter().filter(|t| !t.instrument.percussion) {
        for s in t.staves.iter().filter(|s| !s.tuning.is_empty()) {
            for n in s.bars.iter().flat_map(|b| &b.voices).flat_map(|v| &v.beats).flat_map(|b| &b.notes) {
                out.push(match n.place {
                    NotePlace::Fretted { string, fret } => (
                        s.pitch_of(string, fret).map_or(0, |p| p.0),
                        Some((string, fret)),
                    ),
                    NotePlace::Pitched(p) => (p.0, None),
                });
            }
        }
    }
    out
}

#[test]
fn fingering_runs_on_the_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut total, mut same) = (0usize, 0usize);
    for path in all {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_plugin_test::released::guitar_pro::read(&data) else {
            continue;
        };
        let score = imported.score;
        let before = notes(&score);
        // The tab taken away: pitches only, harmonics' sounding pitch kept
        // by leaving those notes where they are.
        let mut bare = score.clone();
        for t in bare.tracks.iter_mut().filter(|t| !t.instrument.percussion) {
            for s in t.staves.iter_mut() {
                let shape = oxiguitar_model::Staff {
                    bars: Vec::new(),
                    ..s.clone()
                };
                for n in s
                    .bars
                    .iter_mut()
                    .flat_map(|b| &mut b.voices)
                    .flat_map(|v| &mut v.beats)
                    .flat_map(|b| &mut b.notes)
                {
                    if n.effects.harmonic.is_some() {
                        continue;
                    }
                    if let NotePlace::Fretted { string, fret } = n.place
                        && let Some(p) = shape.pitch_of(string, fret)
                    {
                        n.place = NotePlace::Pitched(p);
                    }
                }
            }
        }
        let t0 = Instant::now();
        oxiguitar_edit::finger_pitched(&mut bare);
        let open_time = t0.elapsed();
        let after = notes(&bare);
        assert_eq!(before.len(), after.len(), "{}", path.display());
        let mut kept = 0;
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(b.0, a.0, "{}: a note changed pitch", path.display());
            if b.1.is_some() && b.1 == a.1 {
                kept += 1;
            }
        }
        total += before.len();
        same += kept;
        // Find Fingering on every track of the file as it is.
        let mut edited = score.clone();
        let mut history = History::new();
        let t1 = Instant::now();
        let hard = match Caret::at_start(&edited, 0) {
            Some(c) => {
                oxiguitar_edit::apply(&mut edited, &mut history, c, None, Command::Fingering(Scope::All));
                assert_eq!(notes(&edited).iter().map(|n| n.0).collect::<Vec<_>>(), before.iter().map(|n| n.0).collect::<Vec<_>>());
                edited
                    .tracks
                    .iter()
                    .flat_map(|t| &t.staves)
                    .flat_map(|s| &s.bars)
                    .flat_map(|b| &b.voices)
                    .flat_map(|v| &v.beats)
                    .filter(|b| [HARD_STRETCH, HARD_FINGERS, HARD_SHIFT].contains(&b.text.as_str()))
                    .count()
            }
            None => 0,
        };
        let tool_time = t1.elapsed();
        println!(
            "{}: {} notes, {:.0} % where the tab had them, {hard} hard beats, {:.2} s at open, {:.2} s as the tool",
            path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            before.len(),
            100.0 * kept as f64 / before.len().max(1) as f64,
            open_time.as_secs_f64(),
            tool_time.as_secs_f64(),
        );
    }
    if total > 0 {
        println!(
            "All: {total} notes, {:.1} % where the tabs had them",
            100.0 * same as f64 / total as f64
        );
    }
}
