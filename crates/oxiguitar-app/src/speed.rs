// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The speed budget (M1): a 300-bar, 8-track file opens in under a second,
//! from reading the file to the first tiles of the window's view.
//!
//! The test goes the window's way through the worker: open, engrave with
//! the settings the window starts with, then draw the tiles the first view
//! asks for, at a high-resolution screen's scale. The score is the band
//! song from `oxiguitar-testgen`, written as dense and as verbose as Guitar
//! Pro 7 writes real files (about 16 MB of score XML).
//!
//! Only optimised builds are held to the budget: `cargo test --release -p
//! oxiguitar-app speed -- --nocapture` (CI's "Speed budget" job). A debug
//! build runs the same steps and prints its times.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use oxiguitar_display::Rect;

use crate::canvas::Sheets;
use crate::document::Settings;
use crate::tracks::Tracks;
use crate::worker::{Event, Request, Worker};

/// The budget, from reading the file to the last tile of the first view.
const BUDGET: Duration = Duration::from_secs(1);
/// The window at its first size (1280 × 800), less the menu bar and the
/// track list's eight rows: the canvas in logical pixels.
const VIEW: (f32, f32) = (1280.0, 610.0);
/// A high-resolution screen: twice the pixels, four times the tiles.
const SCALE_FACTOR: f32 = 2.0;

/// How long each step took, from the request to open the file.
#[derive(Debug, Clone, Copy)]
struct Times {
    opened: Duration,
    engraved: Duration,
    tiles: Duration,
    tile_count: usize,
}

fn next(rx: &Receiver<Event>) -> Event {
    rx.recv_timeout(Duration::from_secs(120))
        .expect("the worker answers")
}

/// Opens `path` as the window does and returns how long each step took.
/// `all_tracks` shows every track (View → Multitrack) instead of the first.
fn open_like_the_window(path: &Path, all_tracks: bool) -> Times {
    let (tx, rx) = channel();
    let worker = Worker::start(move |e| {
        let _ = tx.send(e);
    });
    let start = Instant::now();
    worker.send(Request::Open(path.to_owned()));
    let tracks = match next(&rx) {
        Event::Opened { tracks, .. } => tracks,
        other => panic!("expected the score, got {other:?}"),
    };
    let opened = start.elapsed();
    assert_eq!(tracks.len(), 8);

    let rows = Tracks::new(
        tracks.iter().map(|t| t.default_notation).collect(),
        tracks.iter().map(|t| t.can_tab).collect(),
    );
    let px = crate::PX_PER_PT;
    let settings = Settings {
        tracks: if all_tracks {
            (0..tracks.len()).collect()
        } else {
            rows.shown()
        },
        notation: rows.notation(),
        screen_width: (VIEW.0 - crate::CANVAS_PAD) / px,
        ..Settings::default()
    };
    worker.send(Request::Engrave(settings));
    let (engraving, rects) = match next(&rx) {
        Event::Engraved { engraving, sheets } => (engraving, sheets),
        other => panic!("expected an engraving, got {other:?}"),
    };
    let engraved = start.elapsed();

    let sheets = Sheets {
        lists: Vec::new(),
        rects,
    };
    let (near, _) = Sheets::around(Rect::new(0.0, 0.0, VIEW.0 / px, VIEW.1 / px));
    let wanted = sheets.tiles_for(near, px * SCALE_FACTOR);
    assert!(!wanted.is_empty());
    worker.send(Request::Tiles {
        engraving,
        paint: 1,
        scale: px * SCALE_FACTOR,
        dark: false,
        tiles: wanted.clone(),
    });
    for _ in &wanted {
        assert!(matches!(next(&rx), Event::Tile { .. }), "expected a tile");
    }
    Times {
        opened,
        engraved,
        tiles: start.elapsed(),
        tile_count: wanted.len(),
    }
}

/// The band song, written once per test run.
fn score_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxiguitar-speed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary folder");
    let path = dir.join(name);
    let song = oxiguitar_testgen::band_song(300);
    std::fs::write(&path, oxiguitar_testgen::to_gp(&song)).expect("write the score");
    path
}

fn check(name: &str, all_tracks: bool) {
    let path = score_file(name);
    // The best of three runs: a busy machine can only make a run slower.
    // A debug build, which is not timed, runs once.
    let count = if cfg!(debug_assertions) { 1 } else { 3 };
    let runs: Vec<Times> = (0..count)
        .map(|_| open_like_the_window(&path, all_tracks))
        .collect();
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
    let best = runs
        .iter()
        .min_by_key(|t| t.tiles)
        .copied()
        .expect("at least one run");
    println!(
        "{name}: read {:?}, engraved {:?}, {} tiles {:?} (best of {})",
        best.opened,
        best.engraved,
        best.tile_count,
        best.tiles,
        runs.len()
    );
    if cfg!(debug_assertions) {
        println!("  debug build: the budget of {BUDGET:?} is checked in release builds only");
    } else {
        assert!(
            best.tiles < BUDGET,
            "{name}: opening took {:?}, over the budget of {BUDGET:?}",
            best.tiles
        );
    }
}

#[test]
fn a_300_bar_8_track_file_opens_within_the_budget() {
    check("speed-budget.gp", false);
}

#[test]
fn all_8_tracks_shown_open_within_the_budget() {
    check("speed-budget-multitrack.gp", true);
}
