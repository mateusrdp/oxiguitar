// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engine's gates (M4 slice 2), against a stand-in for the audio
//! device: the device's callbacks, of uneven sizes, are this test calling
//! [`Engine::render`].
//!
//! - Every note starts and ends on the frame its time gives, at 25, 100
//!   and 200 % speed, whatever the callbacks' sizes: the frame of time `t`
//!   is the first whose time is at or after `t`.
//! - Loops jump on the frame of their end and stop what sounds; seeking
//!   starts the next note exactly; the count-in comes first.
//! - Ten minutes of music end on the frame they should, and the clock says
//!   exactly which time each device frame played: nothing drifts.
//! - The mixer: volume, pan, mute, solo; the metronome.
//!
//! That it allocates nothing while it plays is `tests/engine_allocations.rs`
//! (a test program of its own, so no other test allocates meanwhile).

use oxiguitar_play::TrackMix;
use oxiguitar_play::events::NoteEvent;

mod common;
use common::{RATE, frame_of, rig, run, timeline};

#[test]
fn every_note_starts_and_ends_on_its_frame_at_every_speed() {
    let timeline = timeline();
    assert!(timeline.notes.len() > 10, "{}", timeline.notes.len());
    for speed in [0.25, 1.0, 2.0] {
        let (mut engine, mut control, heard, schedule) = rig(&timeline, 1024);
        // Nothing is heard before play.
        assert!(heard.lock().unwrap().on.is_empty());
        control.set_speed(speed);
        control.play(0.0, false);
        let length = (schedule.length() / speed * f64::from(RATE)) as usize + 4096;
        let (left, _) = run(&mut engine, length, true);
        // The sound itself starts on the first note's frame (no ramp up).
        let first = timeline
            .notes
            .iter()
            .map(|n| n.start)
            .fold(f64::MAX, f64::min);
        let onset = frame_of(first, 0.0, speed) as usize;
        assert!(left[onset] > 0.0, "speed {speed}: heard on its frame");
        assert!(
            left[..onset].iter().all(|v| *v == 0.0),
            "speed {speed}: silent before"
        );
        let h = heard.lock().unwrap();
        assert_eq!(h.on.len(), schedule.note_count(), "speed {speed}");
        // The recorder's frame 0 is the first frame of the callback that set
        // things up (64 frames before play), so play starts at frame 64.
        for (id, frame) in &h.on {
            let n: &NoteEvent = &timeline.notes[*id as usize];
            assert_eq!(
                *frame,
                64 + frame_of(n.start, 0.0, speed),
                "speed {speed}: note {id} at {} s",
                n.start
            );
        }
        for (id, frame) in &h.off {
            let n = &timeline.notes[*id as usize];
            assert_eq!(
                *frame,
                64 + frame_of(n.end, 0.0, speed),
                "speed {speed}: end of {id}"
            );
        }
        assert!(control.collect().ended, "speed {speed}: the song ended");
    }
}

#[test]
fn a_loop_jumps_on_the_frame_of_its_end_and_a_seek_starts_the_next_note() {
    let timeline = timeline();
    let (mut engine, mut control, heard, _) = rig(&timeline, 512);
    // Bar 2 (the second played bar), twice and a bit.
    let bar = &timeline.visits()[1];
    let (a, b) = (bar.start, bar.end);
    control.set_loop(Some((a, b)));
    control.play(a, false);
    let span = frame_of(b, a, 1.0);
    run(&mut engine, (span * 2 + span / 2) as usize, false);
    {
        let h = heard.lock().unwrap();
        let in_bar: Vec<&NoteEvent> = timeline
            .notes
            .iter()
            .filter(|n| n.start >= a && n.start < b)
            .collect();
        assert!(!in_bar.is_empty());
        // Every note of the bar, three times (twice whole and its first half
        // again), each pass starting `span` frames after the last.
        for (k, n) in in_bar.iter().enumerate() {
            let first = 64 + frame_of(n.start, a, 1.0);
            let times: Vec<u64> =
                h.on.iter()
                    .filter(|(id, _)| {
                        timeline.notes[*id as usize].start == n.start
                            && timeline.notes[*id as usize].track == n.track
                            && timeline.notes[*id as usize].key == n.key
                    })
                    .map(|(_, f)| *f)
                    .collect();
            assert!(
                times.starts_with(&[first, first + span]),
                "note {k}: {times:?}, expected {first} then {}",
                first + span
            );
        }
        // Each jump stops what sounds, on the loop's last frame + 1.
        assert!(h.all_off.contains(&(64 + span)), "{:?}", h.all_off);
        assert!(h.all_off.contains(&(64 + 2 * span)), "{:?}", h.all_off);
    }
    // Seeking: the next note to start is the first at or after the time,
    // on the frame the seek's time gives.
    control.set_loop(None);
    let target = timeline.notes[timeline.notes.len() / 2].start;
    let before = heard.lock().unwrap().on.len();
    // The seek lands at the start of the next callback, the frame after
    // everything played so far.
    control.seek(target);
    let at = 64 + span * 2 + span / 2;
    run(&mut engine, 4096, false);
    let h = heard.lock().unwrap();
    let (id, frame) = h.on[before];
    assert_eq!(timeline.notes[id as usize].start, target);
    assert_eq!(frame, at);
}

#[test]
fn the_count_in_comes_first_then_the_music() {
    let timeline = timeline();
    assert!(timeline.count_in_length > 0.0);
    let (mut engine, mut control, heard, _) = rig(&timeline, 1024);
    control.play(0.0, true);
    let count_in = frame_of(timeline.count_in_length, 0.0, 1.0);
    let (left, _) = run(&mut engine, count_in as usize + 48_000, true);
    let h = heard.lock().unwrap();
    let first = timeline
        .notes
        .iter()
        .map(|n| n.start)
        .fold(f64::MAX, f64::min);
    assert_eq!(h.on[0].1, 64 + count_in + frame_of(first, 0.0, 1.0));
    // The count-in's clicks sound before it.
    let clicks = left[..count_in as usize]
        .iter()
        .filter(|v| v.abs() > 0.01)
        .count();
    assert!(clicks > 1000, "the count-in is heard ({clicks} frames)");
}

#[test]
fn ten_minutes_end_on_their_frame_and_the_clock_never_drifts() {
    let mut timeline = timeline();
    // A note every half second for ten minutes, then the end.
    let model = timeline.notes[0].clone();
    timeline.notes = (0..1200)
        .map(|i| NoteEvent {
            start: f64::from(i) * 0.5,
            end: f64::from(i) * 0.5 + 0.25,
            pitch: Vec::new(),
            ..model.clone()
        })
        .collect();
    if let Some(last) = timeline.map.visits.last_mut() {
        last.end = 600.0;
    }
    let (mut engine, mut control, heard, _) = rig(&timeline, 1024);
    control.play(0.0, false);
    let total = 600 * RATE as usize + 10_000;
    run(&mut engine, total, false);
    let h = heard.lock().unwrap();
    assert_eq!(h.on.len(), 1200);
    for (i, (_, frame)) in h.on.iter().enumerate() {
        assert_eq!(*frame, 64 + i as u64 * 24_000, "note {i}");
    }
    assert!(control.collect().ended);
    // The clock: device frame 64 + n played the time n / rate, to the
    // frame, up to the very end.
    for n in [0u64, 1, 24_000, 12_345_678, 600 * u64::from(RATE) - 1] {
        let t = control.played_at(64 + n).unwrap();
        assert!(
            (t - n as f64 / f64::from(RATE)).abs() < 1e-9,
            "frame {n}: {t}"
        );
    }
}

#[test]
fn the_mixer_pans_mutes_and_solos_and_the_metronome_clicks() {
    let timeline = timeline();
    let tracks: usize = timeline.notes.iter().map(|n| n.track + 1).max().unwrap();
    let (mut engine, mut control, _, schedule) = rig(&timeline, 1024);
    let first = timeline
        .notes
        .iter()
        .map(|n| n.start)
        .fold(f64::MAX, f64::min);
    let at = |frame: u64| frame as usize;
    // Hard left at full volume: the right is silent.
    for t in 0..tracks {
        control.set_mix(
            t,
            TrackMix {
                volume: 1.0,
                pan: -1.0,
                ..TrackMix::default()
            },
        );
    }
    control.play(first, false);
    let (l, r) = run(&mut engine, 4800, true);
    assert!((l[at(2400)] - 1.0).abs() < 1e-3, "{}", l[2400]);
    assert!(r[at(2400)].abs() < 1e-3);
    // Muted: silence (after the ramp of one piece).
    for t in 0..tracks {
        control.set_mix(
            t,
            TrackMix {
                mute: true,
                ..TrackMix::default()
            },
        );
    }
    control.seek(first);
    let (l, _) = run(&mut engine, 4800, true);
    assert!(l[2400..].iter().all(|v| v.abs() < 1e-6));
    // Solo one track, mute none: only it sounds; the metronome adds clicks
    // on top of a muted song.
    control.set_mix(
        0,
        TrackMix {
            solo: true,
            mute: true,
            ..TrackMix::default()
        },
    );
    control.set_metronome(true);
    control.play(0.0, false);
    let beat = frame_of(timeline.clicks[1].time, 0.0, 1.0) as usize;
    let (l, _) = run(&mut engine, beat + 2000, true);
    assert!(l[..1900].iter().any(|v| v.abs() > 0.01), "the first click");
    assert!(
        l[beat + 10..beat + 1900].iter().any(|v| v.abs() > 0.01),
        "the second click"
    );
    let _ = schedule;
}
