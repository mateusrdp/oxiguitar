// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engine tests' stand-in device and recording source.

#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use oxiguitar_play::realtime::{PitchPoint, RtEvent, RtEventKind};
use oxiguitar_play::{
    Buffers, Control, Engine, Route, Schedule, Slot, Source, Sources, Timeline, engine,
};

// ── A source that writes down what it hears ──────────────────────────────

/// What a recording source saw: each event with the device frame it came
/// on (its frames rendered so far).
#[derive(Clone, Debug, Default)]
pub(crate) struct Heard {
    pub(crate) on: Vec<(u32, u64)>,
    pub(crate) off: Vec<(u32, u64)>,
    pub(crate) all_off: Vec<u64>,
}

/// Writes 1.0 into a track's left and right while any of its notes sounds,
/// and writes down every event's frame (preallocated, so it allocates
/// nothing while playing).
pub(crate) struct Recorder {
    heard: Arc<Mutex<Heard>>,
    frame: u64,
    sounding: Vec<u32>,
}

impl Recorder {
    pub(crate) fn new(heard: Arc<Mutex<Heard>>) -> Recorder {
        Recorder {
            heard,
            frame: 0,
            sounding: Vec::with_capacity(1024),
        }
    }
}

impl Source for Recorder {
    fn render(
        &mut self,
        events: &[RtEvent],
        _pitch: &[PitchPoint],
        _time: f64,
        _step: f64,
        out: &mut Buffers,
    ) {
        {
            let mut h = self.heard.lock().unwrap();
            for e in events {
                match e.kind {
                    RtEventKind::NOTE_ON => {
                        h.on.push((e.note.id, self.frame));
                        if self.sounding.len() < self.sounding.capacity() {
                            self.sounding.push(e.note.track);
                        }
                    }
                    RtEventKind::NOTE_OFF => {
                        h.off.push((e.note.id, self.frame));
                        if let Some(i) = self.sounding.iter().position(|&t| t == e.note.track) {
                            self.sounding.swap_remove(i);
                        }
                    }
                    RtEventKind::ALL_OFF => {
                        h.all_off.push(self.frame);
                        self.sounding.clear();
                    }
                    _ => {}
                }
            }
        }
        for slot in 0..out.tracks() {
            if self.sounding.contains(&(slot as u32))
                && let Some((l, r)) = out.track(slot)
            {
                l.fill(1.0);
                r.fill(1.0);
            }
        }
        self.frame += out.frames() as u64;
    }
}

// ── The stand-in device ──────────────────────────────────────────────────

pub(crate) const RATE: u32 = 48_000;

/// Callback sizes, uneven on purpose (a device's vary).
pub(crate) const SIZES: [usize; 7] = [64, 100, 1, 480, 1024, 256, 333];

pub(crate) fn timeline() -> Timeline {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp5");
    let data = std::fs::read(path).unwrap();
    let score = oxiguitar_formats::read(&data).unwrap().score;
    Timeline::new(&score)
}

/// An engine playing `timeline`'s every track through one recorder.
pub(crate) fn rig(
    timeline: &Timeline,
    capacity: usize,
) -> (Engine, Control, Arc<Mutex<Heard>>, Arc<Schedule>) {
    let tracks: usize = timeline
        .notes
        .iter()
        .map(|n| n.track + 1)
        .max()
        .unwrap_or(1);
    let routes: Vec<Option<Route>> = (0..tracks)
        .map(|t| Some(Route { source: 0, slot: t }))
        .collect();
    let schedule = Arc::new(Schedule::new(timeline, &routes, 1));
    let heard = Arc::new(Mutex::new(Heard {
        on: Vec::with_capacity(100_000),
        off: Vec::with_capacity(100_000),
        all_off: Vec::with_capacity(1000),
    }));
    let (mut engine, mut control) = engine(RATE, capacity);
    let mut sources = Sources::new(vec![Slot::new(
        Box::new(Recorder::new(heard.clone())),
        (0..tracks).collect(),
        capacity,
    )]);
    sources.fit(&schedule);
    control.set_schedule(schedule.clone());
    control.set_sources(sources);
    // The engine takes them on its next callback.
    let mut out = vec![0.0; 2 * 64];
    engine.render(&mut out, 2, 0);
    control.collect();
    (engine, control, heard, schedule)
}

/// Runs the device for `frames` frames in callbacks of uneven sizes,
/// returning every frame's left and right.
pub(crate) fn run(engine: &mut Engine, frames: usize, keep: bool) -> (Vec<f32>, Vec<f32>) {
    let mut left = Vec::with_capacity(if keep { frames } else { 0 });
    let mut right = Vec::with_capacity(if keep { frames } else { 0 });
    let mut out = vec![0.0f32; 2 * 1024];
    let mut done = 0;
    let mut k = 0;
    while done < frames {
        let n = SIZES[k % SIZES.len()].min(frames - done);
        k += 1;
        engine.render(&mut out[..2 * n], 2, 0);
        if keep {
            for f in 0..n {
                left.push(out[2 * f]);
                right.push(out[2 * f + 1]);
            }
        }
        done += n;
    }
    (left, right)
}

/// The frame (counted from the moment play started) of time `t`, playing
/// from `from` at `speed`: the first frame whose time is at or after `t`.
pub(crate) fn frame_of(t: f64, from: f64, speed: f64) -> u64 {
    let step = speed / f64::from(RATE);
    ((t - from) / step - 1e-6).ceil().max(0.0) as u64
}
