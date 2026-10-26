// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The test plugin's instrument (interface 1.3): a sine per note, at the
//! note's pitch (its curve followed), starting on the frame of its event
//! with no fade in and stopping on the frame of its end, so the contract
//! suite can find every note's first and last frame exactly. And the
//! effect `gain`, which halves a track.
//!
//! For the suite's own tests, a track whose settings hold `"misbehave":
//! true` breaks the real-time rules on purpose (it allocates and yields its
//! thread, a system call, on every render), and `"late": N` starts its
//! notes N frames late.

use oxiguitar_plugin::audio::{Piece, Prepare, PrepareEffect, RtEventKind, RtNote};
use oxiguitar_plugin::{Effect, Error, Instrument, Processor, Renderer};

/// The most notes sounding at once; more are dropped.
pub const VOICES: usize = 64;

/// The instrument: every one the manifest declares is this.
#[derive(Debug)]
pub struct Sine;

/// The level of a note of velocity 127.
pub const LEVEL: f32 = 0.25;

#[derive(Clone, Copy, Debug)]
struct Voice {
    note: RtNote,
    phase: f64,
    /// Frames to wait before sounding (`late`).
    wait: usize,
}

/// A prepared sine: its voices, allocated once.
#[derive(Debug)]
struct Sines {
    rate: f64,
    tracks: usize,
    voices: [Option<Voice>; VOICES],
    misbehave: bool,
    late: usize,
}

impl Instrument for Sine {
    fn prepare(&self, prepare: &Prepare) -> Result<Box<dyn Renderer>, Error> {
        if prepare.rate == 0 {
            return Err(Error::new(
                oxiguitar_plugin::ErrorKind::Invalid,
                "a sample rate of 0",
            ));
        }
        let setting = |key: &str| {
            prepare.tracks.iter().find_map(|t| {
                oxiguitar_plugin::serde_json::from_str::<oxiguitar_plugin::serde_json::Value>(
                    &t.settings,
                )
                .ok()?
                .get(key)
                .cloned()
            })
        };
        Ok(Box::new(Sines {
            rate: f64::from(prepare.rate),
            tracks: prepare.tracks.len(),
            voices: [None; VOICES],
            misbehave: setting("misbehave").and_then(|v| v.as_bool()) == Some(true),
            late: setting("late").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        }))
    }
}

impl Renderer for Sines {
    fn render(&mut self, piece: &mut Piece<'_, '_>) {
        if self.misbehave {
            // Breaking the rules on purpose (see the module's documentation).
            std::hint::black_box(vec![0u8; 64]);
            std::thread::yield_now();
        }
        for e in piece.events() {
            match e.kind {
                RtEventKind::NOTE_ON => {
                    if let Some(free) = self.voices.iter_mut().find(|v| v.is_none()) {
                        *free = Some(Voice {
                            note: e.note,
                            phase: 0.0,
                            wait: self.late,
                        });
                    }
                }
                RtEventKind::NOTE_OFF => {
                    for v in &mut self.voices {
                        if v.is_some_and(|v| v.note.id == e.note.id) {
                            *v = None;
                        }
                    }
                }
                RtEventKind::ALL_OFF => self.voices = [None; VOICES],
                _ => {}
            }
        }
        let frames = piece.frames();
        for i in 0..VOICES {
            let Some(mut v) = self.voices[i] else {
                continue;
            };
            let track = v.note.track as usize;
            if track >= self.tracks {
                continue;
            }
            let level = LEVEL * f32::from(v.note.velocity) / 127.0;
            for f in 0..frames {
                if v.wait > 0 {
                    v.wait -= 1;
                    continue;
                }
                let cents = piece.cents_at(&v.note, f);
                let key = f64::from(v.note.key) + cents / 100.0;
                let hz = 440.0 * 2f64.powf((key - 69.0) / 12.0);
                // Starts at its peak so the first frame is never silent.
                let s = (v.phase.cos() as f32) * level;
                v.phase =
                    (v.phase + std::f64::consts::TAU * hz / self.rate) % std::f64::consts::TAU;
                if let Some((l, r)) = piece.track(track) {
                    l[f] += s;
                    r[f] += s;
                }
            }
            self.voices[i] = Some(v);
        }
    }
}

/// The effect `gain`: halves the track.
#[derive(Debug)]
pub struct Gain;

#[derive(Debug)]
struct Halve;

impl Effect for Gain {
    fn prepare(&self, _prepare: &PrepareEffect) -> Result<Box<dyn Processor>, Error> {
        Ok(Box::new(Halve))
    }
}

impl Processor for Halve {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for v in left.iter_mut().chain(right.iter_mut()) {
            *v *= 0.5;
        }
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::api::class::InstrumentClass;
    use oxiguitar_plugin::api::message::{PreparedTrack, TrackInfo};
    use oxiguitar_plugin::audio::RtEvent;

    use super::*;

    #[test]
    fn a_note_sounds_from_its_first_frame_to_its_end() {
        let track = PreparedTrack {
            info: TrackInfo {
                index: 0,
                name: "Lead".into(),
                class: InstrumentClass::Fretted,
            },
            program: 29,
            percussion: false,
            tuning: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
            settings: "{}".into(),
        };
        let mut r = Sine
            .prepare(&Prepare {
                instrument: "sine".into(),
                rate: 48_000,
                frames: 64,
                tracks: vec![track],
            })
            .unwrap();
        let note = RtNote {
            id: 7,
            key: 69,
            velocity: 127,
            ..RtNote::default()
        };
        let on = [RtEvent {
            kind: RtEventKind::NOTE_ON,
            note,
        }];
        let (mut l, mut rr) = (vec![0.0; 16], vec![0.0; 16]);
        {
            let mut out = [&mut l[..], &mut rr[..]];
            r.render(&mut Piece::new(&on, &[], 0.0, 1.0 / 48_000.0, 16, &mut out));
        }
        assert_eq!(l[0], LEVEL);
        assert_eq!(rr[0], LEVEL);
        assert!(l[1..].iter().any(|v| *v != 0.0));
        let off = [RtEvent {
            kind: RtEventKind::NOTE_OFF,
            note,
        }];
        let (mut l2, mut r2) = (vec![0.0; 16], vec![0.0; 16]);
        {
            let mut out = [&mut l2[..], &mut r2[..]];
            r.render(&mut Piece::new(
                &off,
                &[],
                0.0,
                1.0 / 48_000.0,
                16,
                &mut out,
            ));
        }
        assert!(l2.iter().all(|v| *v == 0.0));
    }
}
