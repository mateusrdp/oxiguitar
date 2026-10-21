// SPDX-License-Identifier: GPL-3.0-or-later
//! The two Rust SoundFont synthesisers behind one trait, one instance per
//! track (as the player would run them: each track its own buffer).

use crate::song::{Msg, RATE};
use std::sync::Arc;

pub trait Engine {
    fn name(&self) -> String;
    fn send(&mut self, channel: u8, msg: Msg);
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
}

pub struct Rusty {
    synth: rustysynth::Synthesizer,
    block: usize,
}

impl Rusty {
    pub fn new(font: &Arc<rustysynth::SoundFont>, block: usize) -> Rusty {
        let mut settings = rustysynth::SynthesizerSettings::new(RATE as i32);
        settings.block_size = block;
        settings.enable_reverb_and_chorus = false;
        Rusty { synth: rustysynth::Synthesizer::new(font, &settings).unwrap(), block }
    }
}

impl Engine for Rusty {
    fn name(&self) -> String {
        format!("rustysynth/{}", self.block)
    }
    fn send(&mut self, ch: u8, msg: Msg) {
        let ch = ch as i32;
        match msg {
            Msg::Program(p) => self.synth.process_midi_message(ch, 0xC0, p as i32, 0),
            Msg::Bank(b) => self.synth.process_midi_message(ch, 0xB0, 0, b as i32),
            Msg::NoteOn(k, v) => self.synth.note_on(ch, k as i32, v as i32),
            Msg::NoteOff(k) => self.synth.note_off(ch, k as i32),
            Msg::Bend(v) => self.synth.process_midi_message(ch, 0xE0, (v & 0x7f) as i32, (v >> 7) as i32),
            Msg::BendRange(r) => {
                for (c, v) in [(101, 0), (100, 0), (6, r as i32), (38, 0)] {
                    self.synth.process_midi_message(ch, 0xB0, c, v);
                }
            }
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.synth.render(left, right);
    }
}

pub struct Oxi {
    synth: oxisynth::Synth,
}

impl Oxi {
    pub fn new(sf_bytes: &[u8]) -> Oxi {
        let desc = oxisynth::SynthDescriptor {
            reverb_active: false,
            chorus_active: false,
            sample_rate: RATE as f32,
            ..Default::default()
        };
        let mut synth = oxisynth::Synth::new(desc).unwrap();
        let font = oxisynth::SoundFont::load(&mut std::io::Cursor::new(sf_bytes)).unwrap();
        synth.add_font(font, true);
        Oxi { synth }
    }
}

impl Engine for Oxi {
    fn name(&self) -> String {
        "oxisynth/64".into()
    }
    fn send(&mut self, channel: u8, msg: Msg) {
        use oxisynth::MidiEvent as E;
        let s = &mut self.synth;
        let _ = match msg {
            Msg::Program(p) => s.send_event(E::ProgramChange { channel, program_id: p }),
            Msg::Bank(b) => s.send_event(E::ControlChange { channel, ctrl: 0, value: b }),
            Msg::NoteOn(key, vel) => s.send_event(E::NoteOn { channel, key, vel }),
            Msg::NoteOff(key) => s.send_event(E::NoteOff { channel, key }),
            Msg::Bend(value) => s.send_event(E::PitchBend { channel, value }),
            Msg::BendRange(r) => {
                for (ctrl, value) in [(101u8, 0u8), (100, 0), (6, r), (38, 0)] {
                    let _ = s.send_event(E::ControlChange { channel, ctrl, value });
                }
                Ok(())
            }
        };
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        self.synth.write_f32(n, left, 0, 1, right, 0, 1);
    }
}
