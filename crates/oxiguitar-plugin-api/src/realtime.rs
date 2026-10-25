// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The audio thread's side of the interface (1.3): plain C calls, no
//! messages, nothing allocated, nothing locked (docs/design/player.md).
//!
//! Everything else about instruments, effects and audio devices goes
//! through messages off the audio thread: an instrument is *prepared* for
//! its tracks with [`crate::message::Request::Prepare`] (it loads its
//! sounds then, and answers with an instance), and only then rendered here.
//!
//! ```text
//!  host (audio thread)                        plugin
//!  ───────────────────                        ──────
//!  rt.render(instance, &RenderCall) ────────▶ an instrument renders a
//!                                             piece of a callback: its
//!                                             events start on its first
//!                                             frame
//!  rt.process(instance, &ProcessCall) ──────▶ an effect changes a track's
//!                                             buffer in place
//!  device plugin, in its audio callback:
//!  host.render(stream, out, frames, …) ─────▶ the core fills the device's
//!                                             buffer (HostTableV1::render)
//! ```
//!
//! A note crosses by value, as an [`RtNote`]: plain data an instrument may
//! keep. Its pitch curve is a run of the call's [`RenderCall::pitch`]
//! points, which stay the same from the note's start to its end (the core
//! replaces the timeline only while nothing plays, after
//! [`RtEventKind::ALL_OFF`]). The core splits every device callback at each
//! event, so the events of a render call all happen on its first frame; a
//! call may have no frames, only events.

use std::ffi::c_void;

/// A point of a note's pitch curve: at `time` seconds of the score (100 %
/// speed), `cents` from the note's key. Between points the pitch moves in
/// a straight line; before the first and after the last it holds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PitchPoint {
    /// Seconds from the score's start, at 100 % speed.
    pub time: f64,
    /// Cents from the note's key.
    pub cents: i32,
}

/// How a note sounds beyond its pitch ([`RtNote::sound`]).
pub mod sound {
    /// As fretted.
    pub const NORMAL: u8 = 0;
    /// Muted with the fretting hand: a percussive click with no pitch.
    pub const DEAD: u8 = 1;
    /// Muted with the side of the picking hand.
    pub const PALM_MUTE: u8 = 2;
    /// A harmonic, of the kind in [`super::RtNote::harmonic`]; the key is
    /// the harmonic's.
    pub const HARMONIC: u8 = 3;
}

/// The kinds of harmonic ([`RtNote::harmonic`], when the sound is
/// [`sound::HARMONIC`]).
pub mod harmonic {
    /// Natural.
    pub const NATURAL: u8 = 0;
    /// Artificial.
    pub const ARTIFICIAL: u8 = 1;
    /// Tapped.
    pub const TAPPED: u8 = 2;
    /// Pinch.
    pub const PINCH: u8 = 3;
    /// Semi-harmonic: the note with a little of its harmonic.
    pub const SEMI: u8 = 4;
    /// Feedback.
    pub const FEEDBACK: u8 = 5;
}

/// How a note is started ([`RtNote::attack`]).
pub mod attack {
    /// Picked, plucked or struck.
    pub const PICKED: u8 = 0;
    /// Not picked: hammer-on, pull-off, legato slide, trill.
    pub const LEGATO: u8 = 1;
    /// Tapped.
    pub const TAPPED: u8 = 2;
    /// Slapped.
    pub const SLAPPED: u8 = 3;
    /// Popped.
    pub const POPPED: u8 = 4;
}

/// A note's flags ([`RtNote::flags`]).
pub mod flags {
    /// A ghost note (already softer).
    pub const GHOST: u16 = 1;
    /// The note rings on (already lengthened).
    pub const LET_RING: u16 = 1 << 1;
    /// Swells in from silence.
    pub const FADE_IN: u16 = 1 << 2;
    /// Fades out as it ends.
    pub const FADE_OUT: u16 = 1 << 3;
    /// A grace note.
    pub const GRACE: u16 = 1 << 4;
    /// The fretting hand's vibrato.
    pub const VIBRATO: u16 = 1 << 5;
    /// A wide vibrato (the whammy bar's).
    pub const WIDE_VIBRATO: u16 = 1 << 6;
}

/// One note, as an instrument plays it: what it means, not MIDI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RtNote {
    /// Unique among the notes of a timeline.
    pub id: u32,
    /// The track's place among the tracks the instrument was prepared for.
    pub track: u32,
    /// The sounding MIDI key at its start.
    pub key: u8,
    /// MIDI velocity, 1 to 127.
    pub velocity: u8,
    /// The string, 1 for the highest; 0 for none (drums, keyboards).
    pub string: u8,
    /// [`sound`].
    pub sound: u8,
    /// [`harmonic`], for a harmonic.
    pub harmonic: u8,
    /// [`attack`].
    pub attack: u8,
    /// [`flags`].
    pub flags: u16,
    /// When it starts and ends, in seconds of the score at 100 % speed.
    pub start: f64,
    /// The end.
    pub end: f64,
    /// Its pitch curve (bends, the whammy bar, slides): `pitch_len` points
    /// of [`RenderCall::pitch`] from `pitch_start`; none for a steady
    /// pitch.
    pub pitch_start: u32,
    /// The number of points.
    pub pitch_len: u32,
}

/// The kinds of [`RtEvent`].
pub struct RtEventKind;

impl RtEventKind {
    /// A note starts.
    pub const NOTE_ON: u32 = 1;
    /// A note ends (the same note as its start; its `id` names it).
    pub const NOTE_OFF: u32 = 2;
    /// Every note stops at once (stop, seek, loop, speed).
    pub const ALL_OFF: u32 = 3;
}

/// Something that happens on the first frame of a render call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RtEvent {
    /// [`RtEventKind`]; an instrument ignores kinds it doesn't know.
    pub kind: u32,
    /// The note, for `NOTE_ON` and `NOTE_OFF`.
    pub note: RtNote,
}

/// One render call to an instrument: `frames` frames of every track it
/// plays, into the host's buffers.
#[repr(C)]
#[derive(Debug)]
pub struct RenderCall {
    /// `size_of::<RenderCall>()` in the host's build.
    pub size: u32,
    /// The frames to render, at most the largest the instrument was
    /// prepared for.
    pub frames: u32,
    /// The events on the first frame: `event_count` from `events`.
    pub events: *const RtEvent,
    /// The number of events.
    pub event_count: u32,
    /// The score's time at the first frame, in seconds at 100 % speed.
    pub time: f64,
    /// The score's seconds per frame: the speed over the sample rate. A
    /// pitch curve's times map to frames with it.
    pub step: f64,
    /// The timeline's pitch points, which notes' curves are runs of:
    /// `pitch_count` from `pitch`.
    pub pitch: *const PitchPoint,
    /// The number of points.
    pub pitch_count: u32,
    /// Two buffers per track, left then right, in the order the tracks were
    /// prepared: `outputs[2 * t]` and `outputs[2 * t + 1]`, each `frames`
    /// floats, zeroed by the host. The instrument adds into them.
    pub outputs: *const *mut f32,
    /// The number of buffers (twice the tracks).
    pub output_count: u32,
}

/// One call to an effect: change a track's buffers in place.
#[repr(C)]
#[derive(Debug)]
pub struct ProcessCall {
    /// `size_of::<ProcessCall>()` in the host's build.
    pub size: u32,
    /// The frames.
    pub frames: u32,
    /// Left then right, each `frames` floats.
    pub buffers: *const *mut f32,
    /// 2.
    pub buffer_count: u32,
}

/// The plugin's real-time functions, which its [`crate::abi::PluginTableV1`]
/// points to (interface 1.3). Each may be called only on the audio thread
/// with an instance the plugin made, never for one instance from two
/// threads at once, and must return without allocating, locking, waiting
/// or making a system call.
#[repr(C)]
#[derive(Debug)]
pub struct RealtimeTableV1 {
    /// `size_of::<RealtimeTableV1>()` in the plugin's build.
    pub size: u32,
    /// Renders an instrument instance. [`crate::abi::Status::OK`], or
    /// [`crate::abi::Status::PANICKED`] once it panicked (the host then
    /// silences it and withdraws the plugin off the audio thread).
    pub render: unsafe extern "C" fn(instance: u64, call: *const RenderCall) -> crate::abi::Status,
    /// Runs an effect instance.
    pub process:
        unsafe extern "C" fn(instance: u64, call: *const ProcessCall) -> crate::abi::Status,
    /// How many times the plugin's memory allocator was used while one of
    /// these functions ran, since the plugin started; `u64::MAX` when the
    /// plugin can't tell (built without the SDK's allocator). For the
    /// contract suite.
    pub allocations: unsafe extern "C" fn() -> u64,
}

/// The audio device's request to the core from its callback
/// ([`crate::abi::HostTableV1`]'s `render`, interface 1.3): fill `out`
/// with `frames` interleaved frames of `channels` channels, which reach
/// the speaker `delay_ns` nanoseconds after the call.
pub type HostRender = unsafe extern "C" fn(
    ctx: *const c_void,
    stream: u64,
    out: *mut f32,
    frames: u32,
    channels: u32,
    delay_ns: u64,
);
