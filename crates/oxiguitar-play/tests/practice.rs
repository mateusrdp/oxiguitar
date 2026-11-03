// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! M4's gate "the practice tools work on every reference song": on every
//! reference song and the test song, through the engine and a stand-in
//! device whose callbacks are of uneven sizes (`common`):
//!
//! - a loop over a bar with notes, at 75 % speed, with the count-in and the
//!   metronome: the count-in delays the first note, every note of the bar
//!   starts on its frame relative to the others, and each pass comes one
//!   bar's length (at that speed) after the last, stopping what sounds;
//! - Play Selection of another bar at 125 % speed: exactly the bar's notes
//!   sound, and the sound stops on the frame of its end.

use std::path::{Path, PathBuf};

use oxiguitar_play::Timeline;

mod common;
use common::{frame_of, rig, run};

fn songs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out: Vec<PathBuf> = ["fixtures/reference", "fixtures/gp"]
        .iter()
        .flat_map(|d| {
            std::fs::read_dir(root.join(d))
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "gp" | "gp5" | "gpx" | "gp4" | "gp3"))
        })
        .collect();
    out.sort();
    out
}

/// The played bars (visits) with at least one note starting in them.
fn bars_with_notes(timeline: &Timeline) -> Vec<(f64, f64)> {
    timeline
        .visits()
        .iter()
        .map(|v| (v.start, v.end))
        .filter(|&(a, b)| timeline.notes.iter().any(|n| n.start >= a && n.start < b))
        .collect()
}

/// The notes starting in `[a, b)`, by id.
fn notes_in(timeline: &Timeline, a: f64, b: f64) -> Vec<u32> {
    timeline
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.start >= a && n.start < b)
        .map(|(i, _)| i as u32)
        .collect()
}

fn loop_at_three_quarters(
    name: &str,
    timeline: &Timeline,
    (a, b): (f64, f64),
) -> Result<(), String> {
    let speed = 0.75;
    let (mut engine, mut control, heard, _) = rig(timeline, 512);
    control.set_speed(speed);
    control.set_metronome(true);
    control.set_loop(Some((a, b)));
    control.play(a, true);
    let span = frame_of(b, a, speed);
    let count_in = (timeline.count_in_length / speed * 48_000.0) as u64;
    run(
        &mut engine,
        (count_in + span * 2 + span / 2) as usize + 4096,
        false,
    );
    let h = heard.lock().unwrap();
    let ids = notes_in(timeline, a, b);
    let first = ids[0];
    let times = |id: u32| -> Vec<u64> {
        h.on.iter()
            .filter(|(i, _)| *i == id)
            .map(|(_, f)| *f)
            .collect()
    };
    let t0 = times(first);
    let Some(&start) = t0.first() else {
        return Err(format!("{name}: the loop's first note never sounded"));
    };
    let base = start - frame_of(timeline.notes[first as usize].start, a, speed);
    if timeline.count_in_length > 0.0 && base < 64 + count_in / 2 {
        return Err(format!(
            "{name}: no count-in before the first note (frame {base})"
        ));
    }
    for id in ids {
        let n = &timeline.notes[id as usize];
        let t = times(id);
        let want = base + frame_of(n.start, a, speed);
        if t.len() < 2 || t[0] != want || t[1] != want + span {
            return Err(format!(
                "{name}: note {id} at {} s sounded on {t:?}, expected {want} then {}",
                n.start,
                want + span
            ));
        }
    }
    if !h.all_off.contains(&(base + span)) {
        return Err(format!(
            "{name}: the loop's jump didn't stop the sound on frame {}",
            base + span
        ));
    }
    Ok(())
}

fn selection_at_five_quarters(
    name: &str,
    timeline: &Timeline,
    (a, b): (f64, f64),
) -> Result<(), String> {
    let speed = 1.25;
    let (mut engine, mut control, heard, _) = rig(timeline, 1024);
    control.set_speed(speed);
    control.play_until(a, Some(b), false);
    let span = frame_of(b, a, speed);
    run(&mut engine, span as usize + 48_000, false);
    let h = heard.lock().unwrap();
    let mut sounded: Vec<u32> = h.on.iter().map(|(id, _)| *id).collect();
    sounded.sort_unstable();
    let ids = notes_in(timeline, a, b);
    if sounded != ids {
        return Err(format!(
            "{name}: the selection played {sounded:?}, not {ids:?}"
        ));
    }
    for (id, frame) in &h.on {
        let want = 64 + frame_of(timeline.notes[*id as usize].start, a, speed);
        if *frame != want {
            return Err(format!("{name}: note {id} on frame {frame}, not {want}"));
        }
    }
    if !h.all_off.contains(&(64 + span)) {
        return Err(format!(
            "{name}: the sound didn't stop on frame {}: {:?}",
            64 + span,
            h.all_off
        ));
    }
    drop(h);
    if !control.collect().ended {
        return Err(format!(
            "{name}: the window wasn't told the selection ended"
        ));
    }
    Ok(())
}

#[test]
fn the_practice_tools_work_on_every_reference_song() {
    let all = songs();
    assert!(all.len() > 40, "the reference songs are missing");
    let mut failed = Vec::new();
    let mut tried = 0;
    for path in all {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        let score = oxiguitar_plugin_test::released::guitar_pro::read(&data)
            .unwrap()
            .score;
        let timeline = Timeline::new(&score);
        let bars = bars_with_notes(&timeline);
        if bars.is_empty() {
            continue;
        }
        tried += 1;
        // A bar in the middle for the loop, the last for the selection.
        if let Err(e) = loop_at_three_quarters(&name, &timeline, bars[bars.len() / 2]) {
            failed.push(e);
        }
        if let Err(e) = selection_at_five_quarters(&name, &timeline, bars[bars.len() - 1]) {
            failed.push(e);
        }
    }
    assert!(tried > 40, "{tried} songs with notes");
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
