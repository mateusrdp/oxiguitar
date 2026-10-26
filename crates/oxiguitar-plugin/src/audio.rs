// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Instruments and effects (interface 1.3; docs/design/player.md).
//!
//! An [`Instrument`] is prepared off the audio thread for the tracks it
//! plays ([`Instrument::prepare`]): load its sounds then, and touch their
//! memory, so nothing waits later. It hands back a [`Renderer`], which the
//! audio thread calls ([`Renderer::render`]) with each piece of a device
//! callback: its events happen on the piece's first frame, and it adds its
//! tracks' sound into the buffers. An [`Effect`] works the same way on one
//! track's buffers, after its instrument ([`Processor::process`]).
//!
//! On the audio thread, never allocate, lock, wait or make a system call:
//! a device callback is a few milliseconds, and a late one is a click the
//! listener hears. The contract suite counts allocations and, on Linux,
//! catches system calls while rendering.

pub use oxiguitar_plugin_api::message::{Prepare, PrepareEffect, PreparedTrack};
pub use oxiguitar_plugin_api::realtime::{
    PitchPoint, RtEvent, RtEventKind, RtNote, attack, flags, harmonic, sound,
};

use crate::Error;

/// A prepared instrument or effect, as the plugin's glue keeps it for the
/// audio thread.
pub enum Instance {
    /// An instrument's renderer.
    Instrument(Box<dyn Renderer>),
    /// An effect's processor.
    Effect(Box<dyn Processor>),
}

impl Instance {
    /// The frames its sound comes late by.
    pub fn latency(&self) -> u32 {
        match self {
            Instance::Instrument(r) => r.latency(),
            Instance::Effect(p) => p.latency(),
        }
    }

    /// How late a note may start, in frames.
    pub fn resolution(&self) -> u32 {
        match self {
            Instance::Instrument(r) => r.resolution(),
            Instance::Effect(_) => 0,
        }
    }
}

/// Where prepared instances are kept until the host releases them: the
/// export glue in an `.oxp` file; a test's own elsewhere.
pub trait Keeper {
    /// Keeps `instance`; returns the number the host names it by.
    fn keep(&self, instance: Instance) -> u64;
    /// Frees the instance `id`; false if there is none.
    fn release(&self, id: u64) -> bool;
}

/// An instrument the manifest declares under `instruments`.
pub trait Instrument: Send + Sync {
    /// Gets ready to play `prepare.tracks` at `prepare.rate`, in pieces of
    /// at most `prepare.frames` frames. Off the audio thread: take the time
    /// it needs.
    fn prepare(&self, prepare: &Prepare) -> Result<Box<dyn Renderer>, Error>;
}

/// An effect the manifest declares under `effects`.
pub trait Effect: Send + Sync {
    /// Gets ready to process the track `prepare.track` at `prepare.rate`.
    fn prepare(&self, prepare: &PrepareEffect) -> Result<Box<dyn Processor>, Error>;
}

/// A prepared instrument, called on the audio thread.
pub trait Renderer: Send {
    /// Renders one piece: `piece.events()` happen on its first frame; add
    /// each track's sound into `piece.track(slot)`.
    fn render(&mut self, piece: &mut Piece<'_, '_>);

    /// The frames its sound comes late by. The core moves the cursor back
    /// by it.
    fn latency(&self) -> u32 {
        0
    }

    /// How many frames late a note may start, at most: a synthesiser that
    /// renders in blocks starts a note on its next block (7 for blocks of
    /// 8). The contract suite holds the instrument to it. 0, the default,
    /// starts every note on its frame.
    fn resolution(&self) -> u32 {
        0
    }
}

/// A prepared effect, called on the audio thread.
pub trait Processor: Send {
    /// Changes the track's left and right buffers in place.
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);

    /// The frames it delays the sound by.
    fn latency(&self) -> u32 {
        0
    }
}

/// One piece of a device callback, as a renderer sees it.
#[derive(Debug)]
pub struct Piece<'a, 'b> {
    pub(crate) events: &'a [RtEvent],
    pub(crate) pitch: &'a [PitchPoint],
    pub(crate) time: f64,
    pub(crate) step: f64,
    pub(crate) frames: usize,
    pub(crate) outputs: &'a mut [&'b mut [f32]],
}

impl<'a, 'b> Piece<'a, 'b> {
    /// A piece over `outputs` (left then right per track, each `frames`
    /// long): for tests, and for the SDK's glue.
    pub fn new(
        events: &'a [RtEvent],
        pitch: &'a [PitchPoint],
        time: f64,
        step: f64,
        frames: usize,
        outputs: &'a mut [&'b mut [f32]],
    ) -> Piece<'a, 'b> {
        Piece {
            events,
            pitch,
            time,
            step,
            frames,
            outputs,
        }
    }

    /// What happens on the first frame.
    pub fn events(&self) -> &'a [RtEvent] {
        self.events
    }

    /// The frames to render (0: events only).
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The score's time at the first frame, seconds at 100 % speed.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The score's seconds per frame (the speed over the sample rate).
    pub fn step(&self) -> f64 {
        self.step
    }

    /// The timeline's pitch points, which every note's curve is a run of
    /// ([`RtNote::pitch_start`], [`RtNote::pitch_len`]). The same array
    /// stays put while anything sounds, so a renderer can keep a note's
    /// run, by index, from one call to the next.
    pub fn pitch(&self) -> &'a [PitchPoint] {
        self.pitch
    }

    /// The note's pitch curve: empty for a steady pitch.
    pub fn pitch_of(&self, note: &RtNote) -> &'a [PitchPoint] {
        let start = note.pitch_start as usize;
        self.pitch
            .get(start..start + note.pitch_len as usize)
            .unwrap_or(&[])
    }

    /// The pitch, in cents from the note's key, `frame` frames into the
    /// piece: its curve followed in straight lines, held before the first
    /// point and after the last.
    pub fn cents_at(&self, note: &RtNote, frame: usize) -> f64 {
        cents_at(self.pitch_of(note), self.time + frame as f64 * self.step)
    }

    /// The track `slot`'s left and right buffers, `frames()` long; `None`
    /// for a track it wasn't prepared for.
    pub fn track(&mut self, slot: usize) -> Option<(&mut [f32], &mut [f32])> {
        let pair = self.outputs.get_mut(slot * 2..slot * 2 + 2)?;
        let (l, r) = pair.split_at_mut(1);
        let n = self.frames;
        Some((&mut l[0][..n], &mut r[0][..n]))
    }
}

/// The pitch of `curve` at `time`, in cents.
pub fn cents_at(curve: &[PitchPoint], time: f64) -> f64 {
    let Some(first) = curve.first() else {
        return 0.0;
    };
    if time <= first.time {
        return f64::from(first.cents);
    }
    for w in curve.windows(2) {
        let (a, b) = (w[0], w[1]);
        if time < b.time {
            let span = b.time - a.time;
            let f = if span > 0.0 {
                (time - a.time) / span
            } else {
                1.0
            };
            return f64::from(a.cents) + (f64::from(b.cents) - f64::from(a.cents)) * f;
        }
    }
    curve.last().map_or(0.0, |p| f64::from(p.cents))
}
