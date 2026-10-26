// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Playing an SFZ instrument: one track's voices, each a region's sample
//! from the note's exact frame, as sfizz plays them (docs/design/player.md,
//! "SFZ, as built").
//!
//! - **Which regions** a note starts: its key and velocity in their
//!   ranges, the key switch last pressed, the controllers in their ranges,
//!   one random number for the note against `lorand`..`hirand`, each
//!   region's own round-robin count (`seq_length`, `seq_position`), the
//!   trigger (`attack`, `first`, `legato`; `release` regions on the
//!   note's end). A region whose group another chokes (`off_by`) stops the
//!   voices of that group; `polyphony` and `note_polyphony` limit them.
//! - **A voice** reads its sample through a 32-tap windowed sinc
//!   (`sinc.rs`) at the speed its pitch asks for: the key's distance from
//!   the region's centre, tuning, the timeline's pitch curve (bends,
//!   slides) in cents with no range limit, vibrato, the pitch envelope and
//!   LFOs. Its level: `volume`, `amplitude`, the velocity curve and
//!   crossfades, key tracking, controllers through their curves, the
//!   amplitude envelope (sfizz's: a linear attack, exponential decay and
//!   release) and LFOs. Then its filter, if it has one, and its pan,
//!   width and position (sfizz's law: centre is unchanged).
//! - **Loops**: `loop_continuous` until the release ends, `loop_sustain`
//!   while the note is held, `one_shot` to the end whatever the note does;
//!   a sample's own loop (its `smpl` chunk) loops unless the region says
//!   otherwise.
//! - **Stopping everything** (stop, seek, a loop's jump): every voice
//!   fades out over 2.5 ms, and the next notes start at full level.
//!
//! Nothing allocates, locks or waits once made: voices, held notes and
//! each key's list of regions are made in [`Sampler::new`].

use std::sync::Arc;

use oxiguitar_plugin::audio::{PitchPoint, RtNote, cents_at};

use super::Loaded;
use super::instrument::Curves;
use super::region::{
    Crossfade, EgKind, EgParam, FilterKind, KeyCenter, LfoParam, LoopMode, OffMode, OldLfo, Region,
    Source, Target, Trigger, Wave, cc,
};
use super::sample::{Pcm, Sample};
use super::sinc::{self, BEFORE, TAPS};

/// The most voices a track sounds at once.
pub(crate) const VOICES: usize = 256;
/// The most notes a track holds at once.
const HELD: usize = 128;
/// How fast a voice another stops goes: sfizz's `off_time`.
const QUICK_FADE: f64 = 0.006;
/// How fast everything stops at once.
const STOP_FADE: f64 = 0.0025;
/// sfizz's envelope: the release ends below this, then fades out over
/// `FADEOUT` seconds; a sustain at or below `FREE_RUNNING` releases by
/// itself (drums).
const RELEASE_FLOOR: f32 = 1e-4;
const FADEOUT: f64 = 0.05;
const FREE_RUNNING: f32 = 0.0032;
/// sfizz's compensation for its pan stages: a mono voice is panned once
/// (−3 dB in the middle), a stereo one twice.
const MONO_GAIN: f32 = std::f32::consts::SQRT_2;
const STEREO_GAIN: f32 = 2.0;

/// How a pitch's cents become a speed.
#[inline]
fn ratio(cents: f64) -> f64 {
    (cents / 1200.0).exp2()
}

#[inline]
fn db(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// A key's frequency.
fn hertz(key: f64) -> f64 {
    440.0 * ratio((key - 69.0) * 100.0)
}

// ---------------------------------------------------------------------
// Envelopes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
    Fadeout,
    Done,
}

/// sfizz's ADSR: a delay, a linear attack from `start` to 1, a hold, an
/// exponential decay to the sustain level, and an exponential release
/// (each falling 78 dB over its time), then a short fade to silence.
#[derive(Clone, Copy, Debug)]
struct Envelope {
    stage: Stage,
    value: f32,
    delay: u32,
    attack_step: f32,
    hold: u32,
    decay_rate: f32,
    sustain: f32,
    release_rate: f32,
    fade_step: f32,
    /// Releases by itself when it reaches its sustain (a sustain of
    /// nothing: drums, plucks).
    free_running: bool,
    rate: f32,
}

/// The values of an envelope's stages for one note.
#[derive(Clone, Copy, Debug, Default)]
struct Stages {
    delay: f32,
    start: f32,
    attack: f32,
    hold: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    depth: f32,
}

fn exp_rate(seconds: f32, rate: f32) -> f32 {
    if seconds <= 0.0 {
        0.0
    } else {
        (-9.0 / (seconds.max(QUICK_FADE as f32) * rate)).exp()
    }
}

impl Envelope {
    const IDLE: Envelope = Envelope {
        stage: Stage::Done,
        value: 0.0,
        delay: 0,
        attack_step: 1.0,
        hold: 0,
        decay_rate: 0.0,
        sustain: 1.0,
        release_rate: 0.0,
        fade_step: 0.0,
        free_running: false,
        rate: 48_000.0,
    };

    fn new(s: &Stages, rate: f32) -> Envelope {
        let sustain = (s.sustain / 100.0).clamp(0.0, 1.0);
        Envelope {
            stage: Stage::Delay,
            value: (s.start / 100.0).clamp(0.0, 1.0),
            delay: (s.delay.max(0.0) * rate) as u32,
            attack_step: if s.attack <= 0.0 {
                1.0
            } else {
                1.0 / (s.attack * rate)
            },
            hold: (s.hold.max(0.0) * rate) as u32,
            decay_rate: exp_rate(s.decay, rate),
            sustain,
            release_rate: exp_rate(s.release, rate),
            fade_step: 0.0,
            free_running: sustain <= FREE_RUNNING,
            rate,
        }
    }

    fn release(&mut self) {
        if !matches!(self.stage, Stage::Release | Stage::Fadeout | Stage::Done) {
            self.stage = Stage::Release;
        }
    }

    /// The next frame's value.
    #[inline]
    fn next(&mut self) -> f32 {
        loop {
            match self.stage {
                Stage::Delay => {
                    if self.delay > 0 {
                        self.delay -= 1;
                        return self.value;
                    }
                    self.stage = Stage::Attack;
                }
                Stage::Attack => {
                    self.value += self.attack_step;
                    if self.value < 1.0 {
                        return self.value;
                    }
                    self.value = 1.0;
                    self.stage = Stage::Hold;
                }
                Stage::Hold => {
                    if self.hold > 0 {
                        self.hold -= 1;
                        return self.value;
                    }
                    self.stage = Stage::Decay;
                }
                Stage::Decay => {
                    self.value *= self.decay_rate;
                    // Exponential: it nears the sustain without reaching
                    // it, so close enough counts (sfizz's threshold).
                    if self.value > self.sustain + 0.001 {
                        return self.value;
                    }
                    self.value = self.value.max(self.sustain);
                    self.stage = Stage::Sustain;
                }
                Stage::Sustain => {
                    if self.free_running {
                        self.stage = Stage::Release;
                        continue;
                    }
                    return self.value;
                }
                Stage::Release => {
                    let before = self.value;
                    self.value *= self.release_rate;
                    if self.value > RELEASE_FLOOR {
                        return self.value;
                    }
                    self.value = before.min(RELEASE_FLOOR);
                    self.fade_step = self.value / (FADEOUT as f32 * self.rate);
                    self.stage = Stage::Fadeout;
                }
                Stage::Fadeout => {
                    self.value -= self.fade_step;
                    if self.value > 0.0 {
                        return self.value;
                    }
                    self.value = 0.0;
                    self.stage = Stage::Done;
                }
                Stage::Done => return 0.0,
            }
        }
    }
}

// ---------------------------------------------------------------------
// Filters

/// A biquad's coefficients (RBJ's cookbook), normalised.
#[derive(Clone, Copy, Debug, Default)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    #[inline]
    fn run(&mut self, c: &Coefficients, x: f32) -> f32 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// How many biquads in a row a filter is, and of which kind.
fn stages_of(kind: FilterKind) -> (usize, Option<Shape>) {
    match kind {
        FilterKind::LowPass1 => (1, Some(Shape::LowPass1)),
        FilterKind::HighPass1 => (1, Some(Shape::HighPass1)),
        FilterKind::LowPass2 => (1, Some(Shape::LowPass)),
        FilterKind::LowPass4 => (2, Some(Shape::LowPass)),
        FilterKind::LowPass6 => (3, Some(Shape::LowPass)),
        FilterKind::HighPass2 => (1, Some(Shape::HighPass)),
        FilterKind::HighPass4 => (2, Some(Shape::HighPass)),
        FilterKind::HighPass6 => (3, Some(Shape::HighPass)),
        FilterKind::BandPass1 | FilterKind::BandPass2 => (1, Some(Shape::BandPass)),
        FilterKind::BandReject2 => (1, Some(Shape::BandReject)),
        // Shelves and peaks need a gain the recommended sets never give.
        FilterKind::LowShelf | FilterKind::HighShelf | FilterKind::Peak => (0, None),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    LowPass1,
    HighPass1,
    LowPass,
    HighPass,
    BandPass,
    BandReject,
}

fn coefficients(shape: Shape, hz: f32, resonance_db: f32, rate: f32) -> Coefficients {
    let hz = hz.clamp(10.0, rate * 0.49);
    let w = std::f32::consts::TAU * hz / rate;
    let (sin, cos) = w.sin_cos();
    match shape {
        Shape::LowPass1 | Shape::HighPass1 => {
            // One pole, one zero (the bilinear transform).
            let k = (w / 2.0).tan();
            let n = 1.0 / (1.0 + k);
            if shape == Shape::LowPass1 {
                Coefficients {
                    b0: k * n,
                    b1: k * n,
                    b2: 0.0,
                    a1: (k - 1.0) * n,
                    a2: 0.0,
                }
            } else {
                Coefficients {
                    b0: n,
                    b1: -n,
                    b2: 0.0,
                    a1: (k - 1.0) * n,
                    a2: 0.0,
                }
            }
        }
        _ => {
            let q = std::f32::consts::FRAC_1_SQRT_2 * db(resonance_db.max(0.0));
            let alpha = sin / (2.0 * q);
            let a0 = 1.0 + alpha;
            let (b0, b1, b2) = match shape {
                Shape::LowPass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
                Shape::HighPass => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
                Shape::BandPass => (alpha, 0.0, -alpha),
                _ => (1.0, -2.0 * cos, 1.0),
            };
            Coefficients {
                b0: b0 / a0,
                b1: b1 / a0,
                b2: b2 / a0,
                a1: -2.0 * cos / a0,
                a2: (1.0 - alpha) / a0,
            }
        }
    }
}

/// A voice's filter: up to three biquads for each channel.
#[derive(Clone, Copy, Debug, Default)]
struct Filter {
    shape: Option<Shape>,
    stages: usize,
    coefficients: Coefficients,
    state: [[Biquad; 3]; 2],
    /// The cutoff before what moves it, in hertz, and how far (cents) the
    /// key, velocity and randomness move it for this note.
    hz: f32,
    cents: f32,
    resonance: f32,
}

impl Filter {
    #[inline]
    fn run(&mut self, channel: usize, x: f32) -> f32 {
        let mut y = x;
        let c = self.coefficients;
        for s in &mut self.state[channel][..self.stages] {
            y = s.run(&c, y);
        }
        y
    }
}

// ---------------------------------------------------------------------
// LFOs

/// The most LFOs a voice runs (SFZ 2's numbered ones).
const LFOS: usize = 4;

#[derive(Clone, Copy, Debug, Default)]
struct LfoState {
    /// The region's LFO this follows, by its place in `Region::lfos`.
    index: Option<usize>,
    phase: f32,
    freq: f32,
    held: f32,
}

fn wave(w: Wave, phase: f32, held: f32) -> f32 {
    let p = phase.rem_euclid(1.0);
    match w {
        Wave::Triangle => {
            if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            }
        }
        Wave::Sine => (std::f32::consts::TAU * p).sin(),
        Wave::Pulse75 => {
            if p < 0.75 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Pulse25 => {
            if p < 0.25 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Pulse12 => {
            if p < 0.125 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Ramp => 2.0 * p - 1.0,
        Wave::Saw => 1.0 - 2.0 * p,
        Wave::Random => held,
    }
}

/// How far into its delay and fade an LFO is, at `seconds` into the note.
fn lfo_level(delay: f32, fade: f32, seconds: f32) -> f32 {
    if seconds < delay {
        0.0
    } else if fade > 0.0 {
        ((seconds - delay) / fade).min(1.0)
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------
// Voices

/// A voice's loop.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Loop {
    start: f64,
    /// The loop's last frame.
    end: f64,
    /// Loops only while the note is held (`loop_sustain`).
    sustain: bool,
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    active: bool,
    /// The note that started it, its key, and the key its region's
    /// sample sounds at unchanged.
    note: RtNote,
    key: u8,
    center: f64,
    /// The region it plays, by index.
    region: u32,
    /// Its sample (an index of the instrument's) or generator.
    source: Source,
    /// Frames left before it sounds (`delay`, sample-exact).
    wait: u32,
    pos: f64,
    /// The frame after its last.
    end: f64,
    looping: Option<Loop>,
    one_shot: bool,
    /// Source frames for each frame played, before the pitch moves.
    base_speed: f64,
    /// The pitch from the region, key and velocity, in cents; and what
    /// moves while it sounds (the timeline's curve, vibrato).
    cents: f64,
    moving: f64,
    gain: f32,
    /// The pan, width and position, as a matrix from the sample's two
    /// channels to the track's.
    matrix: [f32; 4],
    stereo: bool,
    amp: Envelope,
    pitch_eg: Option<(Envelope, f32)>,
    filter_eg: Option<(Envelope, f32)>,
    filter: Filter,
    lfos: [LfoState; LFOS],
    /// Seconds since it started sounding, for LFOs and vibrato.
    seconds: f32,
    /// A fade that stops it: frames left and their count.
    fade: Option<(u32, u32)>,
    /// When it started, in frames of the track: the oldest is taken first.
    born: u64,
    /// Released (its note ended, or it was choked).
    released: bool,
    /// Vibrato's phase and how long it has run.
    vibrato_phase: f64,
    vibrating: f64,
    /// A generator's phase.
    phase: f64,
}

impl Voice {
    const IDLE: Voice = Voice {
        active: false,
        note: RtNote {
            id: 0,
            track: 0,
            key: 0,
            velocity: 0,
            string: 0,
            sound: 0,
            harmonic: 0,
            attack: 0,
            flags: 0,
            start: 0.0,
            end: 0.0,
            pitch_start: 0,
            pitch_len: 0,
        },
        key: 0,
        center: 60.0,
        region: 0,
        source: Source::Silence,
        wait: 0,
        pos: 0.0,
        end: 0.0,
        looping: None,
        one_shot: false,
        base_speed: 1.0,
        cents: 0.0,
        moving: 0.0,
        gain: 0.0,
        matrix: [1.0, 0.0, 0.0, 1.0],
        stereo: false,
        amp: Envelope::IDLE,
        pitch_eg: None,
        filter_eg: None,
        filter: Filter {
            shape: None,
            stages: 0,
            coefficients: Coefficients {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            },
            state: [[Biquad {
                x1: 0.0,
                x2: 0.0,
                y1: 0.0,
                y2: 0.0,
            }; 3]; 2],
            hz: 0.0,
            cents: 0.0,
            resonance: 0.0,
        },
        lfos: [LfoState {
            index: None,
            phase: 0.0,
            freq: 0.0,
            held: 0.0,
        }; LFOS],
        seconds: 0.0,
        fade: None,
        born: 0,
        released: false,
        vibrato_phase: 0.0,
        vibrating: 0.0,
        phase: 0.0,
    };

    /// Stops it over `frames` frames.
    fn fade_out(&mut self, frames: u32) {
        let frames = frames.max(1);
        match self.fade {
            Some((left, _)) if left <= frames => {}
            _ => self.fade = Some((frames, frames)),
        }
        self.released = true;
    }
}

/// A note being held.
#[derive(Clone, Copy, Debug)]
struct Held {
    id: u32,
    key: u8,
    velocity: u8,
    string: u8,
    /// When it started, in frames of the track.
    since: u64,
    note: RtNote,
}

/// Reads frame `i` of a sample's channel `c`, wrapping into its loop past
/// the loop's end, silent outside it.
#[inline]
fn frame(s: &Sample, i: i64, looping: Option<Loop>, end: f64, c: usize) -> f32 {
    let mut i = i;
    if let Some(l) = looping {
        let (start, last) = (l.start as i64, l.end as i64);
        if i > last {
            let len = last - start + 1;
            if len > 0 {
                i = start + (i - last - 1) % len;
            }
        }
    }
    if i < 0 || i as f64 >= end {
        0.0
    } else {
        s.at(i as usize, c)
    }
}

/// The 32 frames around `first` (the first tap's frame), both channels.
#[inline]
fn gather(s: &Sample, first: i64, looping: Option<Loop>, end: f64, out: &mut [[f32; TAPS]; 2]) {
    let last = first + TAPS as i64 - 1;
    let limit = match looping {
        Some(l) => (l.end as i64).min(end as i64 - 1),
        None => end as i64 - 1,
    };
    let channels = s.channels;
    let stereo = channels == 2;
    let [left, right] = out;
    let taps = left.iter_mut().zip(right.iter_mut()).enumerate();
    if first >= 0 && last <= limit {
        // Inside: read straight.
        let base = first as usize;
        match &s.pcm {
            Pcm::I16(d) => {
                for (t, (l, r)) in taps {
                    let i = (base + t) * channels;
                    *l = f32::from(d[i]) / 32_768.0;
                    *r = if stereo {
                        f32::from(d[i + 1]) / 32_768.0
                    } else {
                        *l
                    };
                }
            }
            Pcm::I24(d) => {
                let read = |i: usize| {
                    let b = &d[i * 3..i * 3 + 3];
                    (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0
                };
                for (t, (l, r)) in taps {
                    let i = (base + t) * channels;
                    *l = read(i);
                    *r = if stereo { read(i + 1) } else { *l };
                }
            }
            Pcm::F32(d) => {
                for (t, (l, r)) in taps {
                    let i = (base + t) * channels;
                    *l = d[i];
                    *r = if stereo { d[i + 1] } else { *l };
                }
            }
        }
        return;
    }
    for (t, (l, r)) in taps {
        let i = first + t as i64;
        *l = frame(s, i, looping, end, 0);
        *r = if stereo {
            frame(s, i, looping, end, 1)
        } else {
            *l
        };
    }
}

#[inline]
fn dot(w: &[f32], x: &[f32; TAPS]) -> f32 {
    w.iter().zip(x).map(|(a, b)| a * b).sum()
}

/// The pan law: gains for left and right at `pan` (−1 to 1).
fn pan_gains(pan: f32) -> (f32, f32) {
    let p = ((pan + 1.0) * 0.5).clamp(0.0, 1.0);
    let a = p * std::f32::consts::FRAC_PI_2;
    (a.cos(), a.sin())
}

/// The matrix from a sample's channels to the track's, for `pan`,
/// `width` and `position` in percent (sfizz's stages).
fn matrix(stereo: bool, pan: f32, width: f32, position: f32) -> [f32; 4] {
    let (pl, pr) = pan_gains(pan / 100.0);
    if !stereo {
        return [pl * MONO_GAIN, 0.0, pr * MONO_GAIN, 0.0];
    }
    // Pan each channel, then mix them by the width, then pan by the
    // position: out = position · width · pan · in.
    let w = ((width / 100.0 + 1.0) * 0.5).clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    let (c1, c2) = (w.cos(), w.sin());
    // Width: l' = l·c2 + r·c1, r' = l·c1 + r·c2 (sfizz's `width`), with
    // c1 the cosine of w and c2 the cosine of the rest.
    let (wa, wb) = (c2, c1);
    let (ql, qr) = pan_gains(position / 100.0);
    [
        ql * wa * pl * STEREO_GAIN,
        ql * wb * pr * STEREO_GAIN,
        qr * wb * pl * STEREO_GAIN,
        qr * wa * pr * STEREO_GAIN,
    ]
}

/// A crossfade in: silent at or below `lo`, full from `hi`.
fn fade_in(range: (f32, f32), value: f32, curve: Crossfade) -> f32 {
    let (lo, hi) = range;
    if value < lo {
        return 0.0;
    }
    let length = hi - lo - 1.0;
    if length <= 0.0 || value >= hi {
        return 1.0;
    }
    let p = (value - lo) / length;
    match curve {
        Crossfade::Power => p.min(1.0).sqrt(),
        Crossfade::Gain => p.min(1.0),
    }
}

/// A crossfade out: full up to `lo`, silent from `hi`.
fn fade_out(range: (f32, f32), value: f32, curve: Crossfade) -> f32 {
    let (lo, hi) = range;
    let length = hi - lo - 1.0;
    if length <= 0.0 || value <= lo {
        return 1.0;
    }
    let p = (value - lo) / length;
    if p > 1.0 {
        return 0.0;
    }
    match curve {
        Crossfade::Power => (1.0 - p).sqrt(),
        Crossfade::Gain => 1.0 - p,
    }
}

/// A small, fast random number generator (xorshift): the audio thread
/// can't ask the system for one.
#[derive(Clone, Copy, Debug)]
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// In [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// What a note-on or note-off looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    On,
    Off,
}

/// One track's sampler.
pub(crate) struct Sampler {
    sfz: Arc<Loaded>,
    rate: f64,
    voices: Box<[Voice]>,
    held: Box<[Option<Held>]>,
    /// Each region's round-robin count.
    counts: Box<[u32]>,
    /// Controllers now: the instrument's, then the note's own (velocity,
    /// key, random).
    controllers: Box<[f32]>,
    /// For each key, the regions a key press or let-go may start.
    on_key: Box<[Box<[u32]>]>,
    off_key: Box<[Box<[u32]>]>,
    /// The keys that switch, and the last switched to.
    switches: Option<(u8, u8)>,
    switch: Option<u8>,
    keys_down: [bool; 128],
    previous_key: Option<u8>,
    alternate: bool,
    random: Random,
    /// Frames played so far.
    clock: u64,
    kernel: &'static sinc::Kernel,
}

impl std::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sampler")
            .field("instrument", &self.sfz.instrument.path)
            .finish_non_exhaustive()
    }
}

impl Sampler {
    /// Gets ready to play `sfz` at `rate` frames a second.
    pub(crate) fn new(sfz: Arc<Loaded>, rate: u32, seed: u64) -> Sampler {
        let regions = &sfz.instrument.regions;
        let mut on_key: Vec<Vec<u32>> = vec![Vec::new(); 128];
        let mut off_key: Vec<Vec<u32>> = vec![Vec::new(); 128];
        let mut switches: Option<(u8, u8)> = None;
        for (i, r) in regions.iter().enumerate() {
            if let (Some(lo), Some(hi)) = (r.sw_lokey, r.sw_hikey) {
                let (lo, hi) = (lo.min(hi), lo.max(hi));
                switches = Some(match switches {
                    Some((a, b)) => (a.min(lo), b.max(hi)),
                    None => (lo, hi),
                });
            }
            if r.cc_triggered || r.lokey > r.hikey {
                continue;
            }
            let list = match r.trigger {
                Trigger::Release | Trigger::ReleaseKey => &mut off_key,
                _ => &mut on_key,
            };
            for k in r.lokey..=r.hikey.min(127) {
                list[usize::from(k)].push(i as u32);
            }
        }
        let mut controllers = vec![0.0f32; super::region::CCS];
        controllers.copy_from_slice(&sfz.instrument.controllers);
        let switch = sfz.instrument.default_switch;
        Sampler {
            rate: f64::from(rate.max(1)),
            voices: vec![Voice::IDLE; VOICES].into_boxed_slice(),
            held: vec![None; HELD].into_boxed_slice(),
            counts: vec![0; regions.len()].into_boxed_slice(),
            controllers: controllers.into_boxed_slice(),
            on_key: on_key.into_iter().map(Vec::into_boxed_slice).collect(),
            off_key: off_key.into_iter().map(Vec::into_boxed_slice).collect(),
            switches,
            switch,
            keys_down: [false; 128],
            previous_key: None,
            alternate: false,
            random: Random(seed | 1),
            clock: 0,
            kernel: sinc::kernel(),
            sfz,
        }
    }

    /// Whether a key is one of the instrument's key switches.
    fn is_switch(&self, key: u8) -> bool {
        self.switches
            .is_some_and(|(lo, hi)| (lo..=hi).contains(&key))
    }

    fn curves(&self) -> &Curves {
        &self.sfz.instrument.curves
    }

    /// `target`'s controllers' sum (or, multiplying, product), each at
    /// its depth through its curve.
    fn added(&self, r: &Region, target: Target) -> f32 {
        let curves = self.curves();
        r.mods
            .iter()
            .filter(|m| m.target == target)
            .map(|m| m.depth * curves.at(m.curve, self.controllers[usize::from(m.cc)] / 127.0))
            .sum()
    }

    fn multiplied(&self, r: &Region, target: Target) -> f32 {
        let curves = self.curves();
        r.mods
            .iter()
            .filter(|m| m.target == target)
            .map(|m| {
                m.depth / 100.0 * curves.at(m.curve, self.controllers[usize::from(m.cc)] / 127.0)
            })
            .product()
    }

    /// Whether region `r` plays for this event, its round-robin counted.
    fn plays(
        &mut self,
        i: usize,
        key: u8,
        velocity: u8,
        rand: f32,
        event: Event,
        others: usize,
    ) -> bool {
        let r = &self.sfz.instrument.regions[i];
        if !(r.lokey..=r.hikey).contains(&key) || !(r.lovel..=r.hivel).contains(&velocity) {
            return false;
        }
        let trigger_ok = match (event, r.trigger) {
            (Event::On, Trigger::Attack) => true,
            (Event::On, Trigger::First) => others == 0,
            (Event::On, Trigger::Legato) => others > 0,
            (Event::Off, Trigger::Release | Trigger::ReleaseKey) => true,
            _ => false,
        };
        if !trigger_ok {
            return false;
        }
        // Round robins count every note in the region's keys and
        // velocities, as sfizz counts them.
        let count = self.counts[i];
        self.counts[i] = count.wrapping_add(1);
        let r = &self.sfz.instrument.regions[i];
        if count % r.seq_length.max(1) != r.seq_position.max(1) - 1 {
            return false;
        }
        if !(r.channel.0..=r.channel.1).contains(&1) {
            return false;
        }
        if !(r.lorand <= rand && (rand < r.hirand || r.hirand >= 1.0)) {
            return false;
        }
        if !(r.lobend <= 0.0 && 0.0 <= r.hibend) {
            return false;
        }
        for &(c, lo, hi) in &r.cc_ranges {
            let v = self.controllers[usize::from(c)];
            if v < lo || v > hi {
                return false;
            }
        }
        if let Some((lo, hi)) = r.sw_last
            && !self.switch.is_some_and(|s| (lo..=hi).contains(&s))
        {
            return false;
        }
        if r.sw_down.is_some_and(|k| !self.keys_down[usize::from(k)])
            || r.sw_up.is_some_and(|k| self.keys_down[usize::from(k)])
            || r.sw_previous.is_some_and(|k| self.previous_key != Some(k))
        {
            return false;
        }
        true
    }

    /// Starts note `note`: the regions it triggers.
    pub(crate) fn note_on(&mut self, note: &RtNote, pitch: &[PitchPoint]) {
        let key = note.key.min(127);
        if self.is_switch(key) {
            self.switch = Some(key);
        }
        let others = self.held.iter().flatten().count();
        self.set_note_controllers(key, note.velocity);
        let rand = self.random.unit();
        let born = self.clock;
        // Which regions play.
        let list = std::mem::take(&mut self.on_key[usize::from(key)]);
        let mut choked = false;
        for &i in list.iter() {
            if self.plays(i as usize, key, note.velocity, rand, Event::On, others) {
                if !choked {
                    // Groups this note's regions belong to stop the voices
                    // they choke, before any of its own starts.
                    self.choke_for(&list, key, note.velocity, born);
                    choked = true;
                }
                self.start(i as usize, note, key, note.velocity, 0.0, born, pitch);
            }
        }
        self.on_key[usize::from(key)] = list;
        // Hold it.
        if let Some(slot) = self.held.iter_mut().find(|h| h.is_none()) {
            *slot = Some(Held {
                id: note.id,
                key,
                velocity: note.velocity,
                string: note.string,
                since: born,
                note: *note,
            });
        }
        self.keys_down[usize::from(key)] = true;
        self.previous_key = Some(key);
    }

    /// The per-note controllers: velocity, key, random, alternate.
    fn set_note_controllers(&mut self, key: u8, velocity: u8) {
        let r1 = self.random.unit();
        let r2 = self.random.unit();
        self.alternate = !self.alternate;
        let previous = self.previous_key.map_or(0.0, f32::from);
        let c = &mut self.controllers;
        c[usize::from(cc::VELOCITY)] = f32::from(velocity);
        c[usize::from(cc::KEY)] = f32::from(key);
        c[usize::from(cc::GATE)] = 127.0;
        c[usize::from(cc::RANDOM)] = r1 * 127.0;
        c[usize::from(cc::BIPOLAR_RANDOM)] = r2 * 127.0;
        c[usize::from(cc::ALTERNATE)] = if self.alternate { 127.0 } else { 0.0 };
        c[usize::from(cc::KEY_DELTA)] = (64.0 + f32::from(key) - previous).clamp(0.0, 127.0);
        c[usize::from(cc::ABSOLUTE_KEY_DELTA)] = (f32::from(key) - previous).abs().min(127.0);
    }

    /// Stops the voices the groups of the regions in `list` that will
    /// play choke (`off_by`), and those over their group's `polyphony`.
    fn choke_for(&mut self, list: &[u32], key: u8, velocity: u8, born: u64) {
        let quick = (QUICK_FADE * self.rate) as u32;
        let regions = &self.sfz.instrument.regions;
        for &i in list {
            let r = &regions[i as usize];
            if !(r.lokey..=r.hikey).contains(&key) || !(r.lovel..=r.hivel).contains(&velocity) {
                continue;
            }
            let group = r.group;
            for v in self.voices.iter_mut().filter(|v| v.active && v.born < born) {
                let vr = &regions[v.region as usize];
                if vr.off_by == Some(group) && !v.released {
                    match vr.off_mode {
                        OffMode::Fast => v.fade_out(quick),
                        OffMode::Time => {
                            v.fade_out((vr.off_time.max(0.0) as f64 * self.rate) as u32)
                        }
                        OffMode::Normal => {
                            v.amp.release();
                            v.released = true;
                        }
                    }
                }
            }
        }
    }

    /// Starts a voice of region `i` for `note`.
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        i: usize,
        note: &RtNote,
        key: u8,
        velocity: u8,
        attenuation_db: f32,
        born: u64,
        pitch: &[PitchPoint],
    ) {
        let (source, polyphony, note_polyphony, group) = {
            let r = &self.sfz.instrument.regions[i];
            let source = match r.source {
                Some(Source::File(s)) => match self.sfz.samples.get(s) {
                    Some(Some(_)) => Source::File(s),
                    _ => return,
                },
                Some(Source::Silence) | None => return,
                Some(other) => other,
            };
            (source, r.polyphony, r.note_polyphony, r.group)
        };
        // Polyphony: the group's, then the note's.
        let quick = (QUICK_FADE * self.rate) as u32;
        if let Some(limit) = polyphony {
            self.limit(limit, quick, |v, regions| {
                regions[v.region as usize].group == group
            });
        }
        if let Some(limit) = note_polyphony {
            self.limit(limit, quick, |v, regions| {
                v.key == key && regions[v.region as usize].group == group
            });
        }
        let r = &self.sfz.instrument.regions[i];
        let sample: Option<&Sample> = match source {
            Source::File(s) => self.sfz.samples[s].as_deref(),
            _ => None,
        };
        let v = f32::from(velocity) / 127.0;
        let rate = self.rate as f32;
        // Where it plays in its sample.
        let (frames, sample_rate, stereo, file_loop, unity) = match sample {
            Some(s) => (
                s.frames as f64,
                f64::from(s.rate),
                s.channels == 2,
                s.looped,
                s.unity,
            ),
            None => (f64::INFINITY, self.rate, false, None, None),
        };
        let end = r.end.map_or(frames, |e| ((e + 1) as f64).min(frames));
        let mut offset = r.offset as f64 + self.added(r, Target::Offset) as f64;
        if r.offset_random > 0 {
            offset += (self.random.next() % (r.offset_random as u64 + 1)) as f64;
        }
        let offset = offset.max(0.0);
        if offset >= end {
            return;
        }
        let mode = r.loop_mode.unwrap_or(if file_loop.is_some() {
            LoopMode::Continuous
        } else {
            LoopMode::NoLoop
        });
        let (loop_start, loop_end) = match (r.loop_start, r.loop_end, file_loop) {
            (Some(s), Some(e), _) => (s as f64, e as f64),
            (s, e, Some((fs, fe))) => (
                s.map_or(f64::from(fs), |s| s as f64),
                e.map_or(f64::from(fe), |e| e as f64),
            ),
            (s, e, None) => (
                s.map_or(0.0, |s| s as f64),
                e.map_or(end - 1.0, |e| e as f64),
            ),
        };
        let looping = match mode {
            LoopMode::Continuous | LoopMode::Sustain if loop_end > loop_start => Some(Loop {
                start: loop_start.max(0.0),
                end: loop_end.min(end - 1.0),
                sustain: mode == LoopMode::Sustain,
            }),
            _ => None,
        };
        // Pitch.
        let center = match r.keycenter {
            KeyCenter::Key(k) => f64::from(k),
            KeyCenter::FromSample => f64::from(unity.unwrap_or(60)),
        };
        let random_cents = (self.random.unit() * 2.0 - 1.0) * r.pitch_random;
        let cents = (f64::from(key) - center) * f64::from(r.pitch_keytrack)
            + f64::from(r.transpose) * 100.0
            + f64::from(r.tune)
            + f64::from(random_cents)
            + f64::from(r.pitch_veltrack * v)
            + f64::from(self.added(r, Target::Tune));
        // Level.
        let amp_random = self.random.unit() * r.amp_random;
        let volume = r.volume + r.group_volume + r.master_volume + r.global_volume + amp_random
            - attenuation_db
            + self.added(r, Target::Volume);
        let amplitude = (r.amplitude / 100.0) * self.multiplied(r, Target::Amplitude);
        let curve = if r.velcurve.is_empty() {
            v * v
        } else {
            velocity_curve(&r.velcurve, velocity)
        };
        let veltrack = (r.amp_veltrack + self.added(r, Target::VelTrack)) / 100.0;
        let g = veltrack.abs() * (1.0 - curve);
        let velocity_gain = if veltrack < 0.0 { g } else { 1.0 - g };
        let key_gain = db(r.amp_keytrack * (f32::from(key) - f32::from(r.amp_keycenter)))
            * fade_in(
                (f32::from(r.xfin_key.0), f32::from(r.xfin_key.1)),
                f32::from(key),
                r.xf_keycurve,
            )
            * fade_out(
                (f32::from(r.xfout_key.0), f32::from(r.xfout_key.1)),
                f32::from(key),
                r.xf_keycurve,
            );
        let vel_fade = fade_in(
            (f32::from(r.xfin_vel.0), f32::from(r.xfin_vel.1)),
            f32::from(velocity),
            r.xf_velcurve,
        ) * fade_out(
            (f32::from(r.xfout_vel.0), f32::from(r.xfout_vel.1)),
            f32::from(velocity),
            r.xf_velcurve,
        );
        let cc_fade: f32 = r
            .cc_fades
            .iter()
            .map(|f| {
                let value = self.controllers[usize::from(f.cc)];
                fade_in(f.fade_in, value, r.xf_cccurve) * fade_out(f.fade_out, value, r.xf_cccurve)
            })
            .product();
        let gain = db(volume) * amplitude * velocity_gain * key_gain * vel_fade * cc_fade;
        if gain <= 0.0 || !gain.is_finite() {
            return;
        }
        let pan = r.pan + self.added(r, Target::Pan);
        let width = r.width + self.added(r, Target::Width);
        let position = r.position + self.added(r, Target::Position);
        // Envelopes.
        let amp = Envelope::new(&self.stages(r, EgKind::Amp, v), rate);
        let pitch_eg = {
            let s = self.stages(r, EgKind::Pitch, v);
            (s.depth != 0.0).then(|| (Envelope::new(&s, rate), s.depth))
        };
        let filter_eg = {
            let s = self.stages(r, EgKind::Filter, v);
            (s.depth != 0.0 && r.cutoff.is_some()).then(|| (Envelope::new(&s, rate), s.depth))
        };
        // Filter.
        let mut filter = Filter::default();
        if let (Some(hz), Some(kind)) = (r.cutoff, r.filter.or(Some(FilterKind::LowPass2))) {
            let (stages, shape) = stages_of(kind);
            if let Some(shape) = shape {
                filter.shape = Some(shape);
                filter.stages = stages;
                filter.hz = hz;
                filter.cents = r.fil_keytrack * (f32::from(key) - f32::from(r.fil_keycenter))
                    + r.fil_veltrack * v
                    + (self.random.unit() * 2.0 - 1.0) * r.fil_random
                    + self.added(r, Target::Cutoff);
                filter.resonance = r.resonance + self.added(r, Target::Resonance);
                filter.coefficients = coefficients(
                    shape,
                    hz * ratio(f64::from(filter.cents)) as f32,
                    filter.resonance,
                    rate,
                );
            }
        }
        // LFOs.
        let mut lfos = [LfoState::default(); LFOS];
        for (state, (j, lfo)) in lfos.iter_mut().zip(r.lfos.iter().enumerate()) {
            state.index = Some(j);
            state.freq = lfo.freq + self.added(r, Target::Lfo(lfo.index, LfoParam::Freq));
            state.phase = lfo.phase + self.added(r, Target::Lfo(lfo.index, LfoParam::Phase));
            state.held = self.random.unit() * 2.0 - 1.0;
        }
        let delay = r.delay + self.random.unit() * r.delay_random + self.added(r, Target::Delay);
        let voice = Voice {
            active: true,
            note: *note,
            key,
            center,
            region: i as u32,
            source,
            wait: (f64::from(delay.max(0.0)) * self.rate) as u32,
            pos: offset,
            end,
            looping,
            one_shot: mode == LoopMode::OneShot,
            base_speed: sample_rate / self.rate,
            cents,
            moving: cents_at(super::super::play::run(pitch, note), note.start),
            gain,
            matrix: matrix(stereo, pan, width, position),
            stereo,
            amp,
            pitch_eg,
            filter_eg,
            filter,
            lfos,
            seconds: 0.0,
            fade: None,
            born,
            released: false,
            vibrato_phase: 0.0,
            vibrating: 0.0,
            phase: 0.0,
        };
        let slot = self.free_slot();
        self.voices[slot] = voice;
    }

    /// An envelope's values for a note of velocity `v` (0 to 1).
    fn stages(&self, r: &Region, kind: EgKind, v: f32) -> Stages {
        let eg = match kind {
            EgKind::Amp => &r.ampeg,
            EgKind::Filter => &r.fileg,
            EgKind::Pitch => &r.pitcheg,
        };
        let m = |p: EgParam| self.added(r, Target::Eg(kind, p));
        Stages {
            delay: eg.delay + eg.vel2_delay * v + m(EgParam::Delay),
            start: eg.start + m(EgParam::Start),
            attack: eg.attack + eg.vel2_attack * v + m(EgParam::Attack),
            hold: eg.hold + eg.vel2_hold * v + m(EgParam::Hold),
            decay: eg.decay + eg.vel2_decay * v + m(EgParam::Decay),
            sustain: eg.sustain + eg.vel2_sustain * v + m(EgParam::Sustain),
            release: eg.release + eg.vel2_release * v + m(EgParam::Release),
            depth: eg.depth + eg.vel2_depth * v + m(EgParam::Depth),
        }
    }

    /// Keeps at most `limit` of the voices `which` picks sounding,
    /// stopping the oldest first.
    fn limit(&mut self, limit: u32, quick: u32, which: impl Fn(&Voice, &[Region]) -> bool) {
        let regions = &self.sfz.instrument.regions;
        loop {
            let mut count = 0u32;
            let mut oldest: Option<usize> = None;
            for (n, v) in self.voices.iter().enumerate() {
                if v.active && !v.released && which(v, regions) {
                    count += 1;
                    if oldest.is_none_or(|o| self.voices[o].born > v.born) {
                        oldest = Some(n);
                    }
                }
            }
            match oldest {
                Some(o) if count >= limit.max(1) => self.voices[o].fade_out(quick),
                _ => break,
            }
        }
    }

    /// A slot for a new voice: a free one, else the oldest released, else
    /// the oldest.
    fn free_slot(&self) -> usize {
        if let Some(i) = self.voices.iter().position(|v| !v.active) {
            return i;
        }
        let mut best = 0;
        for (i, v) in self.voices.iter().enumerate() {
            let b = &self.voices[best];
            if (v.released, std::cmp::Reverse(v.born)) > (b.released, std::cmp::Reverse(b.born)) {
                best = i;
            }
        }
        best
    }

    /// Ends note `id`: its voices release (but one-shots), and the
    /// release samples its key has start.
    pub(crate) fn note_off(&mut self, id: u32, pitch: &[PitchPoint]) {
        let Some(slot) = self.held.iter().position(|h| h.is_some_and(|h| h.id == id)) else {
            return;
        };
        let Some(h) = self.held[slot].take() else {
            return;
        };
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note.id == id && !v.released)
        {
            if v.one_shot {
                continue;
            }
            v.amp.release();
            if let Some((e, _)) = &mut v.pitch_eg {
                e.release();
            }
            if let Some((e, _)) = &mut v.filter_eg {
                e.release();
            }
            v.released = true;
        }
        self.keys_down[usize::from(h.key)] = self.held.iter().flatten().any(|o| o.key == h.key);
        // Release samples.
        let others = self.held.iter().flatten().count();
        let held_seconds = (self.clock - h.since) as f32 / self.rate as f32;
        self.set_note_controllers(h.key, h.velocity);
        let rand = self.random.unit();
        let born = self.clock;
        let list = std::mem::take(&mut self.off_key[usize::from(h.key)]);
        for &i in list.iter() {
            if self.plays(i as usize, h.key, h.velocity, rand, Event::Off, others) {
                let decay = self.sfz.instrument.regions[i as usize].rt_decay * held_seconds;
                self.start(i as usize, &h.note, h.key, h.velocity, decay, born, pitch);
            }
        }
        self.off_key[usize::from(h.key)] = list;
    }

    /// Ends the note sounding on `string`, if any: a string sounds one note.
    pub(crate) fn end_string(&mut self, string: u8, pitch: &[PitchPoint]) {
        if string == 0 {
            return;
        }
        while let Some(id) = self
            .held
            .iter()
            .flatten()
            .find(|h| h.string == string)
            .map(|h| h.id)
        {
            self.note_off(id, pitch);
        }
    }

    /// Everything stops at once: every voice fades out quickly.
    pub(crate) fn stop(&mut self) {
        let frames = (STOP_FADE * self.rate) as u32;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.fade_out(frames);
        }
        self.held.iter_mut().for_each(|h| *h = None);
        self.keys_down = [false; 128];
    }

    /// The voices' pitch from the timeline at `t` (seconds of the score)
    /// and their vibrato, `seconds` of sound before the next update.
    pub(crate) fn update(&mut self, pitch: &[PitchPoint], t: f64, seconds: f64) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let curve = super::super::play::run(pitch, &v.note);
            let mut cents = cents_at(curve, t);
            if let Some((depth, hz)) = super::super::play::vibrato_of(&v.note) {
                let rise = (v.vibrating / super::super::play::VIBRATO_RISE).min(1.0);
                cents += depth * rise * v.vibrato_phase.sin();
                v.vibrato_phase = (v.vibrato_phase + std::f64::consts::TAU * hz * seconds)
                    % std::f64::consts::TAU;
                v.vibrating += seconds;
            }
            v.moving = cents;
        }
    }

    /// Adds the next frames of every voice into `left` and `right`.
    pub(crate) fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let rate = self.rate;
        let seconds = frames as f32 / rate as f32;
        let kernel = self.kernel;
        for n in 0..self.voices.len() {
            if !self.voices[n].active {
                continue;
            }
            let v = &mut self.voices[n];
            let r = &self.sfz.instrument.regions[v.region as usize];
            let sample: Option<&Sample> = match v.source {
                Source::File(s) => self.sfz.samples[s].as_deref(),
                _ => None,
            };
            let mut f = 0;
            // Not started yet: its delay runs out first.
            if v.wait > 0 {
                let skip = (v.wait as usize).min(frames);
                v.wait -= skip as u32;
                f = skip;
                if f == frames {
                    continue;
                }
            }
            // The block's pitch, level and filter, from what moves them.
            let mut cents = v.cents + v.moving;
            let mut gain = v.gain;
            let mut cutoff_cents = v.filter.cents;
            let mut pan_shift = 0.0f32;
            for state in &mut v.lfos {
                let Some(j) = state.index else { continue };
                let lfo = &r.lfos[j];
                let level = lfo_level(lfo.delay, lfo.fade, v.seconds);
                let value = wave(lfo.wave, state.phase, state.held) * level;
                let curves = &self.sfz.instrument.curves;
                let ccs = &self.controllers;
                let modded = |p: LfoParam, base: f32| {
                    base + r
                        .mods
                        .iter()
                        .filter(|m| m.target == Target::Lfo(lfo.index, p))
                        .map(|m| m.depth * curves.at(m.curve, ccs[usize::from(m.cc)] / 127.0))
                        .sum::<f32>()
                };
                cents += f64::from(value * modded(LfoParam::Pitch, lfo.pitch));
                gain *= db(value * modded(LfoParam::Volume, lfo.volume));
                gain *= 1.0 + value * modded(LfoParam::Amplitude, lfo.amplitude) / 100.0;
                pan_shift += value * modded(LfoParam::Pan, lfo.pan);
                cutoff_cents += value * modded(LfoParam::Cutoff, lfo.cutoff);
                let before = state.phase;
                state.phase += state.freq * seconds;
                if lfo.wave == Wave::Random && state.phase.floor() != before.floor() {
                    // A new value each cycle, from the voice's own phase
                    // (no shared generator on this path).
                    state.held = ((state.phase * 7919.0).sin() * 43758.547).fract() * 2.0 - 1.0;
                }
            }
            for (lfo, kind) in [
                (&r.pitchlfo, OldLfo::Pitch),
                (&r.amplfo, OldLfo::Amp),
                (&r.fillfo, OldLfo::Filter),
            ] {
                if lfo.depth == 0.0 || lfo.freq <= 0.0 {
                    continue;
                }
                let level = lfo_level(lfo.delay, lfo.fade, v.seconds);
                let value = wave(Wave::Triangle, lfo.freq * v.seconds, 0.0) * level * lfo.depth;
                match kind {
                    OldLfo::Pitch => cents += f64::from(value),
                    OldLfo::Amp => gain *= db(value),
                    OldLfo::Filter => cutoff_cents += value,
                }
            }
            if let Some((e, depth)) = &v.pitch_eg {
                cents += f64::from(e.value * depth);
            }
            if let Some((e, depth)) = &v.filter_eg {
                cutoff_cents += e.value * depth;
            }
            if let Some(shape) = v.filter.shape {
                v.filter.coefficients = coefficients(
                    shape,
                    v.filter.hz * ratio(f64::from(cutoff_cents)) as f32,
                    v.filter.resonance,
                    rate as f32,
                );
            }
            let matrix = if pan_shift == 0.0 {
                v.matrix
            } else {
                matrix(v.stereo, r.pan + pan_shift, r.width, r.position)
            };
            let speed = v.base_speed * ratio(cents);
            let mut taps = [[0.0f32; TAPS]; 2];
            while f < frames {
                if let Some((left_frames, total)) = v.fade {
                    if left_frames == 0 {
                        v.active = false;
                        break;
                    }
                    v.fade = Some((left_frames - 1, total));
                }
                let env = v.amp.next();
                if v.amp.stage == Stage::Done {
                    v.active = false;
                    break;
                }
                if let Some((e, _)) = &mut v.pitch_eg {
                    e.next();
                }
                if let Some((e, _)) = &mut v.filter_eg {
                    e.next();
                }
                let fade = v.fade.map_or(1.0, |(l, t)| l as f32 / t as f32);
                let level = env * gain * fade;
                let (mut a, mut b) = match sample {
                    Some(s) => {
                        let i = v.pos.floor();
                        let frac = v.pos - i;
                        let looping = v.looping.filter(|l| !(l.sustain && v.released));
                        gather(s, i as i64 - BEFORE, looping, v.end, &mut taps);
                        let w = kernel.weights(speed, frac);
                        (dot(w, &taps[0]), dot(w, &taps[1]))
                    }
                    None => {
                        let x = match v.source {
                            Source::Sine => {
                                let hz = hertz(v.center) * ratio(cents);
                                let s = (v.phase * std::f64::consts::TAU).sin() as f32;
                                v.phase = (v.phase + hz / rate).fract();
                                s
                            }
                            _ => {
                                // White noise from the voice's phase.
                                v.phase = (v.phase * 9301.0 + 0.49297).fract();
                                (v.phase * 2.0 - 1.0) as f32
                            }
                        };
                        (x, x)
                    }
                };
                if v.filter.shape.is_some() {
                    a = v.filter.run(0, a);
                    b = v.filter.run(1, b);
                }
                left[f] += (matrix[0] * a + matrix[1] * b) * level;
                right[f] += (matrix[2] * a + matrix[3] * b) * level;
                f += 1;
                // On to the next frame, round the loop or off the end.
                v.pos += speed;
                match v.looping {
                    Some(l) if !(l.sustain && v.released) => {
                        if v.pos > l.end + 1.0 {
                            let len = l.end - l.start + 1.0;
                            v.pos -= len * ((v.pos - l.end - 1.0) / len).ceil().max(1.0);
                        }
                    }
                    _ => {
                        if sample.is_some() && v.pos >= v.end {
                            v.active = false;
                            break;
                        }
                    }
                }
            }
            v.seconds += seconds;
        }
        self.clock += frames as u64;
    }

    /// Voices sounding now, for tests.
    #[cfg(test)]
    fn sounding(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}

/// The velocity curve `points` (`amp_velcurve_N`) at `velocity`, linear
/// between them; 0 at 0 and 1 at 127 unless given.
fn velocity_curve(points: &[(u8, f32)], velocity: u8) -> f32 {
    let mut lo = (0u8, 0.0f32);
    let mut hi = (127u8, 1.0f32);
    for &(k, v) in points {
        if k == 0 {
            lo = (0, v);
        }
        if k == 127 {
            hi = (127, v);
        }
    }
    for &(k, v) in points {
        if k <= velocity && k >= lo.0 {
            lo = (k, v);
        }
        if k >= velocity && k <= hi.0 {
            hi = (k, v);
        }
    }
    if hi.0 == lo.0 {
        return lo.1;
    }
    let t = f32::from(velocity - lo.0) / f32::from(hi.0 - lo.0);
    lo.1 + (hi.1 - lo.1) * t
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::sfz::instrument::build;
    use crate::sfz::text::read_with;

    const RATE: u32 = 48_000;

    /// A sample of `values`, mono, at `rate`.
    fn sample(rate: u32, values: Vec<f32>) -> Sample {
        Sample {
            rate,
            channels: 1,
            frames: values.len(),
            pcm: Pcm::F32(values.into()),
            looped: None,
            unity: None,
        }
    }

    fn constant(level: f32, frames: usize) -> Sample {
        sample(RATE, vec![level; frames])
    }

    fn sine(rate: u32, hz: f64, seconds: f64) -> Sample {
        let n = (f64::from(rate) * seconds) as usize;
        sample(
            rate,
            (0..n)
                .map(|i| {
                    (std::f64::consts::TAU * hz * i as f64 / f64::from(rate)).sin() as f32 * 0.5
                })
                .collect(),
        )
    }

    /// An instrument from `text`, its samples by name.
    fn instrument(text: &str, samples: Vec<(&str, Sample)>) -> Sampler {
        let path = PathBuf::from("/k/i.sfz");
        let bytes = text.as_bytes().to_vec();
        let load = move |p: &Path| {
            if p == Path::new("/k/i.sfz") {
                Ok(bytes.clone())
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            }
        };
        let instrument = build(&path, read_with(&path, &load).unwrap());
        let by_name: Vec<Option<Arc<Sample>>> = instrument
            .samples
            .iter()
            .map(|p| {
                samples
                    .iter()
                    .find(|(n, _)| Path::new("/k").join(n) == *p)
                    .map(|(_, s)| Arc::new(s.clone()))
            })
            .collect();
        let loaded = Loaded {
            instrument,
            samples: by_name,
            missing: Vec::new(),
        };
        Sampler::new(Arc::new(loaded), RATE, 1)
    }

    fn note(id: u32, key: u8, velocity: u8, string: u8) -> RtNote {
        RtNote {
            id,
            key,
            velocity,
            string,
            start: 0.0,
            end: 1.0,
            ..RtNote::default()
        }
    }

    fn render(s: &mut Sampler, frames: usize) -> Vec<f32> {
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        // As the renderer does: blocks of 8.
        let mut f = 0;
        while f < frames {
            let n = (frames - f).min(8);
            s.render(&mut l[f..f + n], &mut r[f..f + n]);
            f += n;
        }
        l
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    fn hz(x: &[f32]) -> f64 {
        let crossings = x
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        crossings as f64 / 2.0 / (x.len() as f64 / f64::from(RATE))
    }

    /// Volume's implicit controller (7 at 100, through the square curve).
    fn volume7() -> f32 {
        (100.0f32 / 127.0).powi(2)
    }

    #[test]
    fn a_note_starts_on_its_own_frame_at_its_level() {
        let mut s = instrument(
            "<region> sample=a.wav key=60",
            vec![("a.wav", constant(0.5, 4_800))],
        );
        assert_eq!(peak(&render(&mut s, 64)), 0.0);
        s.note_on(&note(1, 60, 127, 0), &[]);
        let out = render(&mut s, 64);
        // Centre pan keeps the level; velocity 127 is full.
        let want = 0.5 * volume7();
        assert!((out[0] - want).abs() < 0.02, "{} {want}", out[0]);
        assert!((out[40] - want).abs() < 1e-3, "{}", out[40]);
        // Another key plays nothing.
        s.note_on(&note(2, 61, 127, 0), &[]);
        assert_eq!(s.sounding(), 1);
    }

    #[test]
    fn pitch_follows_the_key_and_the_samples_own_rate() {
        // A 440 Hz sine recorded at 44.1 kHz, centred on A4 (69): A5 plays
        // 880 Hz at 48 kHz.
        let mut s = instrument(
            "<region> sample=a.wav pitch_keycenter=69",
            vec![("a.wav", sine(44_100, 440.0, 1.0))],
        );
        s.note_on(&note(1, 81, 127, 0), &[]);
        let out = render(&mut s, RATE as usize / 4);
        let f = hz(&out[480..]);
        assert!((f - 880.0).abs() < 5.0, "{f}");
        // The timeline's bend moves it: two semitones down, 784 Hz.
        s.stop();
        render(&mut s, 480);
        s.note_on(&note(2, 81, 127, 0), &[]);
        s.update(&[], 0.0, 0.0);
        for v in s.voices.iter_mut().filter(|v| v.active) {
            v.moving = -200.0;
        }
        let out = render(&mut s, RATE as usize / 4);
        let f = hz(&out[480..]);
        assert!((f - 784.0).abs() < 5.0, "{f}");
    }

    #[test]
    fn velocity_layers_and_round_robins_take_turns() {
        let mut s = instrument(
            "<region> sample=soft.wav key=60 hivel=63\n\
             <region> sample=a.wav key=60 lovel=64 seq_length=2 seq_position=1\n\
             <region> sample=b.wav key=60 lovel=64 seq_length=2 seq_position=2",
            vec![
                ("soft.wav", constant(0.1, 48_000)),
                ("a.wav", constant(0.3, 48_000)),
                ("b.wav", constant(0.6, 48_000)),
            ],
        );
        let mut levels = Vec::new();
        for (i, v) in [127u8, 127, 127, 40].into_iter().enumerate() {
            s.stop();
            render(&mut s, 480);
            s.note_on(&note(i as u32, 60, v, 0), &[]);
            let out = render(&mut s, 64);
            let gain = volume7() * (f32::from(v) / 127.0).powi(2);
            levels.push(((out[32] / gain) * 10.0).round() / 10.0);
        }
        assert_eq!(levels, [0.3, 0.6, 0.3, 0.1]);
    }

    #[test]
    fn key_switches_choose_the_regions() {
        let mut s = instrument(
            "<global> sw_lokey=36 sw_hikey=38 sw_default=36\n\
             <group> sw_last=36 <region> sample=a.wav lokey=40 hikey=80\n\
             <group> sw_last=38 <region> sample=b.wav lokey=40 hikey=80",
            vec![
                ("a.wav", constant(0.3, 48_000)),
                ("b.wav", constant(0.6, 48_000)),
            ],
        );
        let level = |s: &mut Sampler, id| {
            s.stop();
            render(s, 480);
            s.note_on(&note(id, 60, 127, 0), &[]);
            let out = render(s, 64);
            ((out[32] / volume7()) * 10.0).round() / 10.0
        };
        assert_eq!(level(&mut s, 1), 0.3);
        // The switch key sounds nothing, and switches.
        s.stop();
        render(&mut s, 480);
        s.note_on(&note(2, 38, 127, 0), &[]);
        assert_eq!(s.sounding(), 0);
        assert_eq!(level(&mut s, 3), 0.6);
    }

    #[test]
    fn a_closed_hi_hat_chokes_the_open_one() {
        let mut s = instrument(
            "<region> sample=open.wav key=46 group=1 off_by=2\n\
             <region> sample=closed.wav key=42 group=2",
            vec![
                ("open.wav", constant(0.5, 48_000)),
                ("closed.wav", constant(0.0001, 48_000)),
            ],
        );
        s.note_on(&note(1, 46, 127, 0), &[]);
        render(&mut s, 480);
        s.note_on(&note(2, 42, 127, 0), &[]);
        let out = render(&mut s, 960);
        // Gone within off_time's 6 ms (288 frames).
        assert!(peak(&out[300..]) < 0.01, "{}", peak(&out[300..]));
        assert!(out[0] > 0.2);
    }

    #[test]
    fn release_samples_play_at_the_end_softer_the_longer_the_note() {
        let text = "<region> sample=hit.wav key=60 loop_mode=one_shot\n\
                    <region> sample=rel.wav key=60 trigger=release rt_decay=6";
        let level_after = |held: usize| {
            let mut s = instrument(
                text,
                vec![
                    ("hit.wav", constant(0.0, 100)),
                    ("rel.wav", constant(0.5, 48_000)),
                ],
            );
            s.note_on(&note(1, 60, 127, 0), &[]);
            render(&mut s, held);
            assert_eq!(peak(&render(&mut s, 64)), 0.0);
            s.note_off(1, &[]);
            render(&mut s, 64)[32]
        };
        let short = level_after(8);
        let long = level_after(RATE as usize);
        assert!((short - 0.5 * volume7()).abs() < 0.01, "{short}");
        // Held a second: 6 dB softer.
        assert!((long / short - 0.501).abs() < 0.01, "{}", long / short);
    }

    #[test]
    fn loops_hold_one_shots_ring_and_releases_fade() {
        // A 100-frame sample: looped, it sounds as long as the note.
        let mut s = instrument(
            "<region> sample=a.wav key=60 loop_mode=loop_continuous ampeg_release=0.05",
            vec![("a.wav", constant(0.5, 100))],
        );
        s.note_on(&note(1, 60, 127, 0), &[]);
        let held = render(&mut s, 4_800);
        assert!(held[4_000] > 0.25, "{}", held[4_000]);
        s.note_off(1, &[]);
        let after = render(&mut s, 9_600);
        assert!(after[100] > 0.1 && after[100] < held[4_000]);
        assert!(peak(&after[7_000..]) < 1e-3);
        assert_eq!(s.sounding(), 0);
        // Unlooped, it ends with its sample.
        let mut s = instrument(
            "<region> sample=a.wav key=60",
            vec![("a.wav", constant(0.5, 100))],
        );
        s.note_on(&note(1, 60, 127, 0), &[]);
        let out = render(&mut s, 400);
        assert!(peak(&out[200..]) < 1e-6);
        // A one-shot ignores the note's end.
        let mut s = instrument(
            "<region> sample=a.wav key=60 loop_mode=one_shot",
            vec![("a.wav", constant(0.5, 4_800))],
        );
        s.note_on(&note(1, 60, 127, 0), &[]);
        render(&mut s, 64);
        s.note_off(1, &[]);
        let out = render(&mut s, 2_000);
        assert!(out[1_900] > 0.25);
    }

    #[test]
    fn stopping_fades_everything_quickly_and_the_next_note_is_full() {
        let mut s = instrument(
            "<region> sample=a.wav key=60 ampeg_release=2",
            vec![("a.wav", constant(0.5, 48_000))],
        );
        s.note_on(&note(1, 60, 127, 0), &[]);
        render(&mut s, 480);
        s.stop();
        s.note_on(&note(2, 60, 127, 0), &[]);
        let out = render(&mut s, 480);
        // Old and new together for 2.5 ms (120 frames), then the new alone.
        assert!((out[200] - 0.5 * volume7()).abs() < 1e-3, "{}", out[200]);
    }

    #[test]
    fn a_new_note_on_a_string_ends_the_one_before() {
        let mut s = instrument(
            "<region> sample=a.wav lokey=40 hikey=80 ampeg_release=0.01",
            vec![("a.wav", constant(0.5, 48_000))],
        );
        s.note_on(&note(1, 60, 127, 1), &[]);
        s.end_string(1, &[]);
        s.note_on(&note(2, 62, 127, 1), &[]);
        s.note_on(&note(3, 64, 127, 2), &[]);
        render(&mut s, 4_800);
        assert_eq!(s.sounding(), 2);
    }

    #[test]
    fn envelopes_are_sfizzs() {
        let mut e = Envelope::new(
            &Stages {
                attack: 0.01,
                decay: 0.1,
                sustain: 50.0,
                release: 0.1,
                ..Stages::default()
            },
            48_000.0,
        );
        let attack: Vec<f32> = (0..480).map(|_| e.next()).collect();
        assert!((attack[239] - 0.5).abs() < 0.01, "{}", attack[239]);
        // The decay falls 78 dB a decay time towards the sustain.
        for _ in 0..48_000 {
            e.next();
        }
        assert!((e.value - 0.5).abs() < 0.002, "{}", e.value);
        e.release();
        for _ in 0..4_800 {
            e.next();
        }
        // After one release time: 78 dB down.
        assert!(e.value < 0.5 * 2e-4, "{}", e.value);
        for _ in 0..4_800 {
            e.next();
        }
        assert_eq!(e.stage, Stage::Done);
    }

    #[test]
    fn velocity_curves_interpolate_their_points() {
        assert_eq!(velocity_curve(&[(42, 1.0)], 21), 0.5);
        assert_eq!(velocity_curve(&[(42, 1.0)], 100), 1.0);
        assert_eq!(velocity_curve(&[], 127), 1.0);
        assert!((velocity_curve(&[(1, 0.2), (3, 0.3)], 2) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn pan_is_equal_power_and_the_middle_unchanged() {
        let m = matrix(false, 0.0, 100.0, 0.0);
        assert!(
            (m[0] - 1.0).abs() < 1e-6 && (m[2] - 1.0).abs() < 1e-6,
            "{m:?}"
        );
        let m = matrix(false, 100.0, 100.0, 0.0);
        assert!(m[0].abs() < 1e-6 && (m[2] - std::f32::consts::SQRT_2).abs() < 1e-6);
        // A stereo sample in the middle at full width keeps its channels.
        let m = matrix(true, 0.0, 100.0, 0.0);
        assert!(
            (m[0] - 1.0).abs() < 1e-5
                && m[1].abs() < 1e-5
                && m[2].abs() < 1e-5
                && (m[3] - 1.0).abs() < 1e-5,
            "{m:?}"
        );
    }
}
