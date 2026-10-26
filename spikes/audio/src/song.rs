// SPDX-License-Identifier: GPL-3.0-or-later
//! A four-bar test song as timed events: a guitar on six channels (one per
//! string, so a bend moves one string only), a bass and drums, at 120 BPM.
//! The same events as a standard MIDI file, for FluidSynth's reference.

pub const RATE: u32 = 48_000;
pub const BEAT: u64 = RATE as u64 / 2; // 120 BPM
pub const PPQ: u64 = 960;
pub const BARS: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Msg {
    Program(u8),
    /// Bank select (CC 0); 128 = drums.
    Bank(u8),
    NoteOn(u8, u8),
    NoteOff(u8),
    /// Pitch bend, 0..16383, 8192 = none.
    Bend(u16),
    /// RPN 0 (pitch-bend range) in semitones.
    BendRange(u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub at: u64, // sample
    pub track: usize,
    pub channel: u8, // within the track's own synthesiser
    pub msg: Msg,
}

pub const TRACKS: [&str; 3] = ["guitar", "bass", "drums"];

/// The MIDI-file channel of a track's channel (one synthesiser for all).
pub fn file_channel(track: usize, channel: u8) -> u8 {
    match track {
        0 => channel,     // 0..=5
        1 => 6,
        _ => 9,
    }
}

pub fn events() -> Vec<Event> {
    let mut ev = Vec::new();
    let mut add = |at: u64, track: usize, channel: u8, msg: Msg| {
        ev.push(Event { at, track, channel, msg })
    };
    let eighth = BEAT / 2;
    // Set-up at 0.
    for ch in 0..6u8 {
        add(0, 0, ch, Msg::Program(27)); // Electric Guitar (clean)
        add(0, 0, ch, Msg::BendRange(12));
    }
    add(0, 1, 0, Msg::Program(33)); // Electric Bass (finger)
    add(0, 2, 9, Msg::Program(0));
    let start = BEAT; // a beat of silence first

    // Guitar, bars 1–2: E5 power chords in eighths (strings 6, 5, 4).
    for i in 0..16u64 {
        let t = start + i * eighth;
        let chord: [(u8, u8); 3] = if i < 8 { [(5, 40), (4, 47), (3, 52)] } else { [(5, 43), (4, 50), (3, 55)] };
        for (s, k) in chord {
            add(t, 0, s, Msg::NoteOn(k, if i % 2 == 0 { 110 } else { 85 }));
            add(t + eighth - 600, 0, s, Msg::NoteOff(k));
        }
    }
    // Bar 3: a melody on string 2 (B string, channel 1), with a full bend.
    let bar3 = start + 8 * BEAT;
    let melody = [64u8, 66, 67, 69];
    for (i, k) in melody.iter().enumerate() {
        let t = bar3 + i as u64 * BEAT / 2;
        add(t, 0, 1, Msg::NoteOn(*k, 100));
        add(t + BEAT / 2 - 300, 0, 1, Msg::NoteOff(*k));
    }
    let bend_at = bar3 + 2 * BEAT;
    add(bend_at, 0, 1, Msg::NoteOn(69, 105));
    for step in 0..=24u64 {
        // up a whole tone (2 of 12 semitones) over half a beat
        let v = 8192 + (8191 * 2 / 12) * step / 24;
        add(bend_at + step * (BEAT / 2) / 24, 0, 1, Msg::Bend(v as u16));
    }
    add(bend_at + 2 * BEAT - 600, 0, 1, Msg::NoteOff(69));
    add(bend_at + 2 * BEAT, 0, 1, Msg::Bend(8192));
    // Bar 4: an open E chord, let ring.
    let bar4 = start + 12 * BEAT;
    for (s, k) in [(5u8, 40u8), (4, 47), (3, 52), (2, 56), (1, 59), (0, 64)] {
        add(bar4 + (5 - s as u64) * 480, 0, s, Msg::NoteOn(k, 95)); // a strum, 10 ms per string
        add(bar4 + 4 * BEAT - 600, 0, s, Msg::NoteOff(k));
    }
    // Bass: root eighths, bars 1–4.
    for i in 0..32u64 {
        let t = start + i * eighth;
        let k = if (8..16).contains(&i) { 31 } else { 28 };
        add(t, 1, 0, Msg::NoteOn(k, 100));
        add(t + eighth - 600, 1, 0, Msg::NoteOff(k));
    }
    // Drums: kick 1 and 3, snare 2 and 4, closed hi-hat eighths.
    for i in 0..32u64 {
        let t = start + i * eighth;
        add(t, 2, 9, Msg::NoteOn(42, 80));
        add(t + 2400, 2, 9, Msg::NoteOff(42));
        if i % 4 == 0 {
            add(t, 2, 9, Msg::NoteOn(36, 115));
            add(t + 2400, 2, 9, Msg::NoteOff(36));
        }
        if i % 4 == 2 {
            add(t, 2, 9, Msg::NoteOn(38, 110));
            add(t + 2400, 2, 9, Msg::NoteOff(38));
        }
    }
    ev.sort_by_key(|e| e.at);
    ev
}

/// Samples of the whole song, with two seconds for the last notes to ring.
pub fn length() -> usize {
    ((1 + BARS * 4) * BEAT + 2 * RATE as u64) as usize
}

/// The events as a format-1-free (format 0) standard MIDI file at 120 BPM.
pub fn midi_file(events: &[Event]) -> Vec<u8> {
    use midly::num::{u4, u7, u14, u15, u24, u28};
    use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
    let tick = |at: u64| at * PPQ / BEAT; // exact: 25 samples a tick
    let mut track: Vec<TrackEvent> = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
    }];
    let mut last = 0u64;
    for e in events {
        let ch = u4::new(file_channel(e.track, e.channel));
        let mut msgs: Vec<MidiMessage> = Vec::new();
        match e.msg {
            Msg::Program(p) => msgs.push(MidiMessage::ProgramChange { program: u7::new(p) }),
            Msg::Bank(b) => msgs.push(MidiMessage::Controller { controller: u7::new(0), value: u7::new(b.min(127)) }),
            Msg::NoteOn(k, v) => msgs.push(MidiMessage::NoteOn { key: u7::new(k), vel: u7::new(v) }),
            Msg::NoteOff(k) => msgs.push(MidiMessage::NoteOff { key: u7::new(k), vel: u7::new(0) }),
            Msg::Bend(v) => msgs.push(MidiMessage::PitchBend { bend: midly::PitchBend(u14::new(v)) }),
            Msg::BendRange(r) => {
                for (c, v) in [(101u8, 0u8), (100, 0), (6, r), (38, 0)] {
                    msgs.push(MidiMessage::Controller { controller: u7::new(c), value: u7::new(v) });
                }
            }
        }
        for m in msgs {
            let t = tick(e.at);
            track.push(TrackEvent {
                delta: u28::new((t - last) as u32),
                kind: TrackEventKind::Midi { channel: ch, message: m },
            });
            last = t;
        }
    }
    track.push(TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    let smf = Smf { header: Header::new(Format::SingleTrack, Timing::Metrical(u15::new(PPQ as u16))), tracks: vec![track] };
    let mut out = Vec::new();
    smf.write_std(&mut out).unwrap();
    out
}
