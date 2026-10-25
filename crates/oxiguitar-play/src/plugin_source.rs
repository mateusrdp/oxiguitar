// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An instrument plugin's prepared instance as one of the engine's
//! sources, with the effects of each of its tracks after it (interface
//! 1.3).

use oxiguitar_host::realtime::Instance;
use oxiguitar_plugin_api::realtime::{PitchPoint, RtEvent};

use crate::engine::{Buffers, Source};

/// An instrument instance and, per track it plays, the effects that run
/// on the track after it, in order.
#[derive(Debug)]
pub struct PluginSource {
    instrument: Instance,
    effects: Vec<Vec<Instance>>,
}

impl PluginSource {
    /// `instrument`, with `effects[slot]` after each of its tracks.
    pub fn new(instrument: Instance, effects: Vec<Vec<Instance>>) -> PluginSource {
        PluginSource {
            instrument,
            effects,
        }
    }
}

impl Source for PluginSource {
    fn render(
        &mut self,
        events: &[RtEvent],
        pitch: &[PitchPoint],
        time: f64,
        step: f64,
        out: &mut Buffers,
    ) {
        let frames = out.frames();
        if !self
            .instrument
            .render(events, pitch, time, step, frames, out.all())
        {
            return;
        }
        if frames == 0 {
            return;
        }
        for (slot, chain) in self.effects.iter_mut().enumerate() {
            for effect in chain {
                if let Some((l, r)) = out.track(slot) {
                    effect.process(l, r);
                }
            }
        }
    }

    fn latency(&self) -> u32 {
        let effects = self
            .effects
            .iter()
            .map(|c| c.iter().map(Instance::latency).sum::<u32>())
            .max()
            .unwrap_or(0);
        self.instrument.latency() + effects
    }
}
