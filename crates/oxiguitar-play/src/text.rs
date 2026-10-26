// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The timeline as text, one line per bar played, note and click: what
//! `oxiguitar-cli events` prints and what the golden files hold
//! (`fixtures/events/`). Times are in seconds with six decimals (a
//! microsecond, a twentieth of a sample at 48 kHz).
//!
//! ```text
//! bar 3 pass 2 4.000000 6.000000
//! note 4.000000 4.500000 t1 s1 v1 b2 string 6 E2 vel 95 palm-mute legato bend 4.250000:100 4.500000:200
//! click 4.000000 strong
//! ```

use std::fmt::Write;

use oxiguitar_model::Pitch;

use crate::{Attack, HarmonicKind, NoteEvent, Sound, Timeline, Vibrato};

/// One note's line.
pub fn note_line(n: &NoteEvent) -> String {
    let mut s = format!(
        "note {:.6} {:.6} t{} s{} v{} b{}",
        n.start,
        n.end,
        n.track + 1,
        n.staff + 1,
        n.voice + 1,
        n.beat + 1
    );
    if let Some(string) = n.string {
        let _ = write!(s, " string {string}");
    }
    let _ = write!(s, " {} vel {}", Pitch(n.key).name(), n.velocity);
    match n.sound {
        Sound::Normal => {}
        Sound::Dead => s.push_str(" dead"),
        Sound::PalmMute => s.push_str(" palm-mute"),
        Sound::Harmonic(h) => s.push_str(match h {
            HarmonicKind::Natural => " harmonic-natural",
            HarmonicKind::Artificial => " harmonic-artificial",
            HarmonicKind::Tapped => " harmonic-tapped",
            HarmonicKind::Pinch => " harmonic-pinch",
            HarmonicKind::Semi => " harmonic-semi",
            HarmonicKind::Feedback => " harmonic-feedback",
        }),
    }
    match n.attack {
        Attack::Picked => {}
        Attack::Legato => s.push_str(" legato"),
        Attack::Tapped => s.push_str(" tapped"),
        Attack::Slapped => s.push_str(" slapped"),
        Attack::Popped => s.push_str(" popped"),
    }
    for (flag, word) in [
        (n.ghost, " ghost"),
        (n.let_ring, " let-ring"),
        (n.fade_in, " fade-in"),
        (n.fade_out, " fade-out"),
        (n.grace, " grace"),
    ] {
        if flag {
            s.push_str(word);
        }
    }
    match n.vibrato {
        Vibrato::None => {}
        Vibrato::Normal => s.push_str(" vibrato"),
        Vibrato::Wide => s.push_str(" vibrato-wide"),
    }
    if !n.pitch.is_empty() {
        s.push_str(" pitch");
        for p in &n.pitch {
            let _ = write!(s, " {:.6}:{}", p.time, p.cents);
        }
    }
    s
}

/// The whole timeline as text.
pub fn timeline(t: &Timeline) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "length {:.6}", t.length());
    let _ = writeln!(
        s,
        "count-in {:.6} clicks {}",
        t.count_in_length,
        t.count_in.len()
    );
    for v in t.visits() {
        let _ = writeln!(
            s,
            "bar {} pass {} {:.6} {:.6}",
            v.visit.bar + 1,
            v.visit.pass,
            v.start,
            v.end
        );
    }
    for n in &t.notes {
        s.push_str(&note_line(n));
        s.push('\n');
    }
    for c in &t.clicks {
        let _ = writeln!(
            s,
            "click {:.6}{}",
            c.time,
            if c.strong { " strong" } else { "" }
        );
    }
    s
}
