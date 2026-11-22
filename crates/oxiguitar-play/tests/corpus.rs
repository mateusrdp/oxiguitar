// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The timeline of every Guitar Pro file of the test corpus
//! (`OXIGUITAR_CORPUS`, or `corpus/files/`; see `tests/support/corpus.rs`),
//! file by file, held to what any timeline must be:
//! notes in order, inside the song, with a length; bars back to back; the
//! cursor's lookup finding every bar. Prints what it found:
//! `cargo test --release -p oxiguitar-play --test corpus -- --nocapture`

#[path = "../../../tests/support/corpus.rs"]
mod corpus;
#[path = "../../../tests/support/guitar_pro.rs"]
mod guitar_pro;

use std::path::Path;
use std::time::Instant;

use oxiguitar_play::Timeline;

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
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let (mut count, mut notes, mut slowest) = (0, 0, (0.0f64, String::new()));
    corpus::check("timeline", &fallback, |path, data| {
        let Some(score) = guitar_pro::read(data)? else {
            return Ok(corpus::Outcome::NotTaken);
        };
        let started = Instant::now();
        let t = Timeline::new(&score);
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
        check(&t)?;
        Ok(corpus::Outcome::Passed)
    });
    println!(
        "{count} files, {notes} notes; slowest {:.1} ms ({})",
        slowest.0, slowest.1
    );
}

#[test]
fn the_reference_songs_have_sound_timelines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut all = Vec::new();
    all.extend(corpus::files(&root.join("reference")));
    all.extend(corpus::files(&root.join("gp")));
    for path in all {
        let Ok(imported) =
            oxiguitar_plugin_test::released::guitar_pro::read(&std::fs::read(&path).unwrap())
        else {
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
    let score = oxiguitar_plugin_test::released::guitar_pro::read(&oxiguitar_testgen::to_gp(&song))
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
