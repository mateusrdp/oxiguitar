// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The timeline of every file of the private corpus, where `corpus/files/`
//! exists (skipped quietly otherwise), held to what any timeline must be:
//! notes in order, inside the song, with a length; bars back to back; the
//! cursor's lookup finding every bar. Prints what it found:
//! `cargo test --release -p oxiguitar-play --test corpus -- --nocapture`

use std::path::{Path, PathBuf};
use std::time::Instant;

use oxiguitar_play::Timeline;

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

/// What every timeline must be; the first problem, if any.
fn check(t: &Timeline) -> Result<(), String> {
    let len = t.length();
    if !len.is_finite() || len < 0.0 {
        return Err(format!("length {len}"));
    }
    let mut at = 0.0;
    for (k, v) in t.visits().iter().enumerate() {
        if (v.start - at).abs() > 1e-9 || v.end < v.start {
            return Err(format!("bar {k} runs {}..{} after {at}", v.start, v.end));
        }
        at = v.end;
        if v.end - v.start > 1e-6 {
            let mid = 0.5 * (v.start + v.end);
            match t.find(mid) {
                Some((found, _)) if found == k => {}
                other => return Err(format!("find({mid}) gave {other:?}, not bar {k}")),
            }
        }
    }
    let mut last = f64::NEG_INFINITY;
    for n in &t.notes {
        if !(n.start.is_finite() && n.end.is_finite()) || n.start < 0.0 || n.end <= n.start {
            return Err(format!("note {}..{}", n.start, n.end));
        }
        if n.start < last {
            return Err("notes out of order".into());
        }
        last = n.start;
        if n.start > len + 1e-6 || n.velocity == 0 || n.key > 127 {
            return Err(format!(
                "note at {} (song {len}), key {}, velocity {}",
                n.start, n.key, n.velocity
            ));
        }
        if n.pitch.windows(2).any(|w| w[1].time < w[0].time) {
            return Err(format!("pitch curve out of order at {}", n.start));
        }
    }
    if t.clicks.iter().any(|c| c.time < 0.0 || c.time > len + 1e-6) {
        return Err("a click outside the song".into());
    }
    Ok(())
}

#[test]
fn every_corpus_file_has_a_sound_timeline() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut count, mut notes, mut slowest) = (0, 0, (0.0f64, String::new()));
    let mut problems = Vec::new();
    for path in all {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_formats::read(&data) else {
            continue;
        };
        let started = Instant::now();
        let t = Timeline::new(&imported.score);
        let ms = started.elapsed().as_secs_f64() * 1e3;
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if ms > slowest.0 {
            slowest = (ms, name.clone());
        }
        count += 1;
        notes += t.notes.len();
        println!(
            "{name}: {} bars played, {:.1} s, {} notes, {:.1} ms",
            t.visits().len(),
            t.length(),
            t.notes.len(),
            ms
        );
        if let Err(e) = check(&t) {
            problems.push(format!("{name}: {e}"));
        }
    }
    println!(
        "{count} files, {notes} notes; slowest {:.1} ms ({})",
        slowest.0, slowest.1
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_reference_songs_have_sound_timelines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut all = Vec::new();
    files(&root.join("reference"), &mut all);
    files(&root.join("gp"), &mut all);
    for path in all {
        let Ok(imported) = oxiguitar_formats::read(&std::fs::read(&path).unwrap()) else {
            continue;
        };
        let t = Timeline::new(&imported.score);
        if let Err(e) = check(&t) {
            panic!("{}: {e}", path.display());
        }
    }
}

/// M4's start budget: playback starts in under 100 ms, so the timeline of
/// the 300-bar, 8-track band song (the speed tests' song) must take a
/// small part of it. Measured in release builds; debug builds only print.
#[test]
fn the_band_song_timeline_is_quick() {
    let song = oxiguitar_testgen::band_song(300);
    let score = oxiguitar_formats::read(&oxiguitar_testgen::to_gp(&song))
        .unwrap()
        .score;
    let started = Instant::now();
    let t = Timeline::new(&score);
    let ms = started.elapsed().as_secs_f64() * 1e3;
    println!(
        "band song: {} notes, {:.1} s, timeline in {ms:.1} ms",
        t.notes.len(),
        t.length()
    );
    assert!(t.notes.len() > 10_000);
    if !cfg!(debug_assertions) {
        assert!(
            ms < 30.0,
            "the timeline took {ms:.1} ms (budget 30 ms of the 100 ms start)"
        );
    }
}
