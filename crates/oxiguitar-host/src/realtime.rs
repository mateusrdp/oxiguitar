// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Prepared instruments and effects (interface 1.3), as the core's engine
//! holds them: [`Instance`]s, made off the audio thread by
//! [`crate::Plugins::prepare`] and [`crate::Plugins::prepare_effect`], then
//! rendered on it. An instance owns what a render call needs (its event
//! list and its buffer pointers, sized once), so rendering allocates
//! nothing; dropping it releases the plugin's instance, which the engine
//! does off the audio thread.

use oxiguitar_plugin_api::abi::Status;
use oxiguitar_plugin_api::message::Request;
use oxiguitar_plugin_api::realtime::{PitchPoint, ProcessCall, RenderCall, RtEvent};

use crate::Plugins;
use crate::loader::Loaded;

/// A prepared instrument or effect.
pub struct Instance {
    loaded: Loaded,
    plugins: Plugins,
    index: usize,
    id: u64,
    latency: u32,
    resolution: u32,
    /// The addresses of the buffers of the call being made, sized once
    /// (kept as numbers, so an instance can move to the audio thread).
    pointers: Vec<usize>,
    /// Set once the plugin panicked rendering: silent from then on.
    failed: bool,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance")
            .field("plugin", &self.plugins.found()[self.index].name())
            .field("id", &self.id)
            .field("latency", &self.latency)
            .finish_non_exhaustive()
    }
}

impl Instance {
    pub(crate) fn new(
        loaded: Loaded,
        plugins: Plugins,
        index: usize,
        id: u64,
        latency: u32,
        resolution: u32,
        buffers: usize,
    ) -> Instance {
        Instance {
            loaded,
            plugins,
            index,
            id,
            latency,
            resolution,
            pointers: Vec::with_capacity(buffers),
            failed: false,
        }
    }

    /// The frames its sound comes late by.
    pub fn latency(&self) -> u32 {
        self.latency
    }

    /// How many frames late a note may start, at most.
    pub fn resolution(&self) -> u32 {
        self.resolution
    }

    /// Whether it panicked rendering (it is silent from then on).
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// The plugin's allocations while rendering so far, or `u64::MAX` when
    /// it can't tell: for the contract suite.
    pub fn allocations(&self) -> u64 {
        self.loaded.allocations()
    }

    /// Renders `frames` frames into `buffers` (left then right per track,
    /// each at least `frames` long; at most as many as it was prepared
    /// for), with `events` on the first frame. On the audio thread: it
    /// allocates nothing. False once the plugin has panicked.
    pub fn render<'b>(
        &mut self,
        events: &[RtEvent],
        pitch: &[PitchPoint],
        time: f64,
        step: f64,
        frames: usize,
        buffers: impl Iterator<Item = &'b mut [f32]>,
    ) -> bool {
        if self.failed {
            return false;
        }
        self.pointers.clear();
        let room = self.pointers.capacity();
        for b in buffers.take(room) {
            if b.len() < frames {
                return true;
            }
            self.pointers.push(b.as_mut_ptr() as usize);
        }
        let call = RenderCall {
            size: std::mem::size_of::<RenderCall>() as u32,
            frames: frames as u32,
            events: events.as_ptr(),
            event_count: events.len() as u32,
            time,
            step,
            pitch: pitch.as_ptr(),
            pitch_count: pitch.len() as u32,
            // The addresses, as the pointers they are (`usize` and a
            // pointer have the same size and layout).
            outputs: self.pointers.as_ptr().cast::<*mut f32>(),
            output_count: self.pointers.len() as u32,
        };
        let status = self.loaded.render(self.id, &call);
        self.pointers.clear();
        if status == Status::OK {
            true
        } else {
            self.failed = true;
            false
        }
    }

    /// Runs an effect over one track's `left` and `right`, on the audio
    /// thread. False once the plugin has panicked.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        if self.failed {
            return false;
        }
        let frames = left.len().min(right.len());
        let pointers = [left.as_mut_ptr(), right.as_mut_ptr()];
        let call = ProcessCall {
            size: std::mem::size_of::<ProcessCall>() as u32,
            frames: frames as u32,
            buffers: pointers.as_ptr(),
            buffer_count: 2,
        };
        let status = self.loaded.process(self.id, &call);
        if status == Status::OK {
            true
        } else {
            self.failed = true;
            false
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // Off the audio thread (the engine hands its sources back before
        // they are dropped): the plugin frees its instance.
        let _ = self
            .plugins
            .send_index(self.index, &Request::Release { instance: self.id });
    }
}
