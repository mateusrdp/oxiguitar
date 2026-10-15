// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A tiny Guitar Pro 3/4/5 writer for tests. It writes one fixed test song,
//! byte by byte, so the readers can be tested in CI without the private
//! corpus. It is not a general writer; that comes in M2b.

pub(crate) struct W {
    pub(crate) major: u8,
    pub(crate) buf: Vec<u8>,
}

impl W {
    pub(crate) fn new(major: u8) -> W {
        W {
            major,
            buf: Vec::new(),
        }
    }
    pub(crate) fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub(crate) fn i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }
    pub(crate) fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn zeros(&mut self, n: usize) {
        self.buf.extend(std::iter::repeat_n(0, n));
    }
    /// Length byte, then the text padded to `size` bytes.
    pub(crate) fn byte_size_string(&mut self, s: &str, size: usize) {
        self.u8(s.len() as u8);
        self.buf.extend_from_slice(s.as_bytes());
        self.zeros(size - s.len());
    }
    /// int (length + 1), length byte, text.
    pub(crate) fn ibs(&mut self, s: &str) {
        self.i32(s.len() as i32 + 1);
        self.u8(s.len() as u8);
        self.buf.extend_from_slice(s.as_bytes());
    }
    pub(crate) fn int_size_string(&mut self, s: &str) {
        self.i32(s.len() as i32);
        self.buf.extend_from_slice(s.as_bytes());
    }
}

/// What the test song contains, for assertions.
pub(crate) const TITLE: &str = "OxiGuitar Test Song";
pub(crate) const ARTIST: &str = "The OxiGuitar contributors";
pub(crate) const TEMPO: i32 = 96;
pub(crate) const BARS: usize = 3;

/// Writes the test song in Guitar Pro `major` (3, 4 or 5) format.
///
/// Bar 1 (4/4, repeat start, marker "Intro"): a quarter note on string 1
/// fret 0 with a hammer-on; an eighth-note triplet on string 2 frets 1, 3, 5
/// (the last with a bend); a quarter on string 3 fret 2, then a tied quarter.
/// Bar 2 (repeat end, plays 2 times): a half note dead note on string 6, a
/// half rest. Bar 3 (3/4): a dotted half chord (strings 4, 5, 6 at frets
/// 2, 2, 0), palm-muted from version 4 on, and in version 5 a second voice
/// with three quarter notes.
pub(crate) fn test_song(major: u8) -> Vec<u8> {
    let mut w = W::new(major);
    let version = match major {
        3 => "FICHIER GUITAR PRO v3.00",
        4 => "FICHIER GUITAR PRO v4.06",
        _ => "FICHIER GUITAR PRO v5.10",
    };
    w.byte_size_string(version, 30);
    // Score information.
    w.ibs(TITLE);
    w.ibs("");
    w.ibs(ARTIST);
    w.ibs("Fixtures");
    w.ibs("OxiGuitar");
    if major == 5 {
        w.ibs("OxiGuitar");
    }
    w.ibs("Public domain");
    w.ibs("OxiGuitar tests");
    w.ibs("");
    w.i32(1);
    w.ibs("Made for OxiGuitar's tests");
    if major < 5 {
        w.u8(0); // triplet feel
    }
    if major >= 4 {
        w.i32(1); // lyrics on track 1
        w.i32(1);
        w.int_size_string("la la la");
        for _ in 0..4 {
            w.i32(0);
            w.int_size_string("");
        }
    }
    if major == 5 {
        w.i32(100); // RSE master volume
        w.i32(0);
        w.zeros(11);
        // Page setup.
        for v in [210, 297, 10, 10, 15, 10, 100] {
            w.i32(v);
        }
        w.i16(0x01ff);
        for _ in 0..10 {
            w.ibs("");
        }
        w.ibs("Moderato");
        w.i32(TEMPO);
        w.u8(0); // tempo not hidden
        w.i8(0); // key
        w.i32(0); // octave
    } else {
        w.i32(TEMPO);
        w.i32(0); // key
        if major == 4 {
            w.i8(0); // octave
        }
    }
    // 64 MIDI channels; channel 1 is a clean electric guitar.
    for i in 0..64 {
        w.i32(if i == 0 { 27 } else { 0 });
        w.u8(100); // volume
        w.u8(64); // balance
        w.zeros(4 + 2);
    }
    if major == 5 {
        for _ in 0..19 {
            w.i16(-1);
        }
        w.i32(0); // master reverb
    }
    w.i32(BARS as i32);
    w.i32(1); // tracks

    // Bar headers.
    // Bar 1: 4/4, repeat start, marker.
    if major == 5 {
        w.u8(0x01 | 0x02 | 0x04 | 0x20);
        w.u8(4);
        w.u8(4);
        w.ibs("Intro");
        w.zeros(4); // marker colour
        w.zeros(4); // beams
        w.u8(0); // alternate endings
        w.u8(0); // triplet feel
    } else {
        w.u8(0x01 | 0x02 | 0x04 | 0x20);
        w.u8(4);
        w.u8(4);
        w.ibs("Intro");
        w.zeros(4);
    }
    // Bar 2: repeat end, played twice.
    if major == 5 {
        w.u8(0); // blank
        w.u8(0x08);
        w.i8(2);
        w.u8(0);
        w.u8(0);
    } else {
        w.u8(0x08);
        w.i8(1);
    }
    // Bar 3: 3/4.
    if major == 5 {
        w.u8(0);
        w.u8(0x01);
        w.u8(3);
        w.zeros(4);
        w.u8(0);
        w.u8(0);
    } else {
        w.u8(0x01);
        w.u8(3);
    }

    // Track.
    if major == 5 {
        w.u8(0);
    }
    w.u8(0); // flags
    w.byte_size_string("Guitar", 40);
    w.i32(6);
    for t in [64, 59, 55, 50, 45, 40, 0] {
        w.i32(t);
    }
    w.i32(1); // port
    w.i32(1); // channel
    w.i32(2); // effect channel
    w.i32(24); // frets
    w.i32(0); // capo
    w.zeros(4); // colour
    if major == 5 {
        w.i16(0x03); // show tablature and standard notation
        w.u8(0);
        w.u8(0);
        w.u8(0); // humanize
        w.zeros(24);
        w.zeros(16); // RSE instrument
        w.zeros(4); // equaliser
        w.ibs("");
        w.ibs("");
        w.u8(0); // after the last track (5.10)
    }

    // Bar 1.
    w.i32(6);
    note_beat(&mut w, 0, None, &[(1, 0, Fx::Hammer)]);
    note_beat(&mut w, 1, Some(3), &[(2, 1, Fx::None)]);
    note_beat(&mut w, 1, Some(3), &[(2, 3, Fx::None)]);
    note_beat(&mut w, 1, Some(3), &[(2, 5, Fx::Bend)]);
    // A quarter and a tied quarter make up the remaining half.
    note_beat(&mut w, 0, None, &[(3, 2, Fx::None)]);
    note_beat(&mut w, 0, None, &[(3, 0, Fx::Tie)]);
    if major == 5 {
        voice_end_v5(&mut w);
    }
    // Bar 2.
    w.i32(2);
    note_beat(&mut w, -1, None, &[(6, 0, Fx::Dead)]);
    rest_beat(&mut w, -1);
    if major == 5 {
        voice_end_v5(&mut w);
    }
    // Bar 3.
    w.i32(1);
    let chord = [
        (4, 2, Fx::PalmMute),
        (5, 2, Fx::PalmMute),
        (6, 0, Fx::PalmMute),
    ];
    beat(&mut w, -1, true, None, &chord);
    if major == 5 {
        w.i32(3);
        for f in [0, 1, 3] {
            note_beat(&mut w, 0, None, &[(1, f, Fx::None)]);
        }
        w.u8(0); // line break
    }
    if major < 5 {
        w.zeros(4);
    }
    w.buf
}

fn voice_end_v5(w: &mut W) {
    // Empty second voice, then the line-break byte.
    w.i32(0);
    w.u8(0);
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Fx {
    None,
    Hammer,
    Bend,
    Dead,
    Tie,
    PalmMute,
}

fn note_beat(w: &mut W, duration: i8, tuplet: Option<i32>, notes: &[(u8, i8, Fx)]) {
    beat(w, duration, false, tuplet, notes);
}

fn rest_beat(w: &mut W, duration: i8) {
    w.u8(0x40);
    w.u8(0x02);
    w.i8(duration);
    w.u8(0); // no strings
    if w.major == 5 {
        w.i16(0);
    }
}

fn beat(w: &mut W, duration: i8, dotted: bool, tuplet: Option<i32>, notes: &[(u8, i8, Fx)]) {
    let mut flags = 0u8;
    if dotted {
        flags |= 0x01;
    }
    if tuplet.is_some() {
        flags |= 0x20;
    }
    w.u8(flags);
    w.i8(duration);
    if let Some(t) = tuplet {
        w.i32(t);
    }
    let mut strings = 0u8;
    for (s, _, _) in notes {
        strings |= 1 << (7 - s);
    }
    w.u8(strings);
    // Notes are written from string 1 down.
    let mut sorted: Vec<_> = notes.to_vec();
    sorted.sort_by_key(|n| n.0);
    for (_, fret, fx) in sorted {
        note(w, fret, fx);
    }
    if w.major == 5 {
        w.i16(0);
    }
}

fn note(w: &mut W, fret: i8, fx: Fx) {
    let mut flags = 0x20; // type and fret present
    let has_effects = matches!(fx, Fx::Hammer | Fx::Bend) || (fx == Fx::PalmMute && w.major >= 4);
    if has_effects {
        flags |= 0x08;
    }
    w.u8(flags);
    w.u8(match fx {
        Fx::Dead => 3,
        Fx::Tie => 2,
        _ => 1,
    });
    w.i8(fret);
    if w.major == 5 {
        w.u8(0); // flags2
    }
    if flags & 0x08 != 0 {
        let (f1, f2) = match fx {
            Fx::Hammer => (0x02, 0),
            Fx::Bend => (0x01, 0),
            Fx::PalmMute => (0, 0x02),
            _ => (0, 0),
        };
        w.u8(f1);
        if w.major >= 4 {
            w.u8(f2);
        }
        if f1 & 0x01 != 0 {
            // A full bend up and back.
            w.i8(1);
            w.i32(100);
            w.i32(3);
            for (pos, val) in [(0, 0), (30, 100), (60, 0)] {
                w.i32(pos);
                w.i32(val);
                w.u8(0);
            }
        }
    }
}
