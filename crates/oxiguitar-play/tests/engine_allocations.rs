// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engine allocates nothing while it plays: the real-time rule
//! (docs/design/player.md, "Staying in sync"), counted by an instrumented
//! global allocator. A test program of its own, without the test harness
//! (`harness = false`): the counter sees every thread, and the harness's
//! own thread allocates while a test runs (it failed once on CI's Linux
//! ARM64 that way, 4 allocations). Here only this thread runs.

use std::alloc::System;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

mod common;
use common::{RATE, SIZES, rig, timeline};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn main() {
    the_engine_allocates_nothing_while_it_plays();
    println!("the engine allocates nothing while it plays: ok");
}

fn the_engine_allocates_nothing_while_it_plays() {
    let timeline = timeline();
    let (mut engine, mut control, _, schedule) = rig(&timeline, 1024);
    control.set_metronome(true);
    control.set_loop(Some((timeline.visits()[0].start, timeline.visits()[1].end)));
    control.play(0.0, true);
    let mut out = vec![0.0f32; 2 * 1024];
    // A first callback takes the commands; then nothing may allocate.
    engine.render(&mut out[..2 * 64], 2, 0);
    let region = Region::new(GLOBAL);
    let frames = (schedule.length() * f64::from(RATE)) as usize * 2;
    let mut done = 0;
    let mut k = 0;
    while done < frames {
        let n = SIZES[k % SIZES.len()];
        k += 1;
        engine.render(&mut out[..2 * n], 2, 256);
        done += n;
    }
    let change = region.change();
    assert_eq!(
        (
            change.allocations,
            change.deallocations,
            change.reallocations
        ),
        (0, 0, 0),
        "allocations while playing"
    );
}
