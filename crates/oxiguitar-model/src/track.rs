// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Tracks, staves, tunings and instruments.

use std::collections::BTreeMap;

use crate::{Bar, Color, Pitch};

/// One instrument part: a guitar, a bass, a drum kit, a piano.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    /// Full name, such as "Lead Guitar".
    pub name: String,
    /// Short name shown at the start of later systems.
    pub short_name: String,
    /// Display colour.
    pub color: Color,
    /// The sound it plays with.
    pub instrument: Instrument,
    /// Mixer settings.
    pub mix: Mix,
    /// One staff for most instruments; two for a piano grand staff.
    pub staves: Vec<Staff>,
    /// Chord diagrams used by this track; beats refer to them by index.
    pub chords: Vec<ChordDiagram>,
    /// Lyrics lines.
    pub lyrics: Vec<LyricLine>,
    /// What plugins keep for this track (the sections they add to the
    /// track dialog, plugin interface 1.3): a JSON object as text, by
    /// plugin id. No score format holds it: it is saved beside the score,
    /// in `FILE.oxg` (`song.gp3.oxg`).
    pub plugins: BTreeMap<String, String>,
}

/// What a track sounds like through General MIDI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Instrument {
    /// General MIDI program, 0 to 127 (25 is steel-string guitar).
    pub midi_program: u8,
    /// True for a drum kit (MIDI channel 10).
    pub percussion: bool,
}

impl Default for Instrument {
    /// A steel-string guitar (program 25), as a new track in Guitar Pro is.
    fn default() -> Self {
        Instrument {
            midi_program: 25,
            percussion: false,
        }
    }
}

impl Instrument {
    /// A fretted string instrument, by its General MIDI program: the
    /// guitars (24–31), the fretted basses (32–34, 36–37; not fretless 35
    /// or the synth basses), sitar (104) and banjo (105). Only these show
    /// tablature unless the user asks for it (Mateus, 16 Oct 2026).
    pub fn is_fretted(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 24..=34 | 36 | 37 | 104 | 105)
    }

    /// A bass: the bass programs (32–39), fretless and synth basses too.
    pub fn is_bass(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 32..=39)
    }

    /// A keyboard played with two hands, shown on a grand staff: pianos and
    /// electric pianos, harpsichord and clavinet (0–7), organs and
    /// accordions (16–21, 23). Not harmonica (22).
    pub fn is_keyboard(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 0..=7 | 16..=21 | 23)
    }
}

/// Mixer settings for a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    /// Volume, 0.0 to 1.0.
    pub volume: f32,
    /// Pan, -1.0 (left) to 1.0 (right).
    pub pan: f32,
    /// Track is muted.
    pub mute: bool,
    /// Track is soloed.
    pub solo: bool,
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            volume: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

/// One staff of a track, with its strings and its bars of music.
#[derive(Clone, Debug, PartialEq)]
pub struct Staff {
    /// Open-string pitches, highest string first (string 1 is the top line of
    /// the tablature). Empty for instruments without strings.
    pub tuning: Vec<Pitch>,
    /// Capo fret, 0 for none. Fret numbers in notes count from the capo, as in
    /// Guitar Pro: fret 0 with a capo on 2 sounds two semitones above the open
    /// string.
    pub capo: u8,
    /// Number of frets.
    pub fret_count: u8,
    /// Semitones from sounding pitch to written pitch in standard notation.
    /// 0 for no transposition. (The guitar's usual octave is shown by the
    /// treble 8vb clef, not here.)
    pub transposition: i8,
    /// Show standard notation.
    pub show_standard: bool,
    /// Show tablature.
    pub show_tablature: bool,
    /// One bar per master bar of the score.
    pub bars: Vec<Bar>,
}

impl Default for Staff {
    fn default() -> Self {
        Staff {
            tuning: Vec::new(),
            capo: 0,
            fret_count: 24,
            transposition: 0,
            show_standard: true,
            show_tablature: true,
            bars: Vec::new(),
        }
    }
}

impl Staff {
    /// Standard six-string guitar tuning, E A D G B E, highest string first.
    pub fn standard_guitar_tuning() -> Vec<Pitch> {
        [64, 59, 55, 50, 45, 40].into_iter().map(Pitch).collect()
    }

    /// Number of strings (0 for instruments without strings).
    pub fn string_count(&self) -> usize {
        self.tuning.len()
    }

    /// The sounding pitch of `fret` on `string` (1 = highest), counting the capo,
    /// or `None` if the string does not exist or the pitch leaves the MIDI range.
    pub fn pitch_of(&self, string: u8, fret: u8) -> Option<Pitch> {
        let open = self.tuning.get(usize::from(string).checked_sub(1)?)?;
        let p = u16::from(open.0) + u16::from(self.capo) + u16::from(fret);
        u8::try_from(p).ok().filter(|p| *p <= 127).map(Pitch)
    }
}

/// A chord diagram: name plus fret positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordDiagram {
    /// Chord name, such as "Am7".
    pub name: String,
    /// Lowest fret shown in the diagram.
    pub first_fret: u8,
    /// Fret per string, highest string first; `None` for a string not played,
    /// `Some(0)` for an open string.
    pub frets: Vec<Option<u8>>,
}

/// One line of lyrics, attached to a track.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricLine {
    /// Index of the bar where the line starts (0-based).
    pub start_bar: usize,
    /// The text, syllables separated by spaces or hyphens as in Guitar Pro.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_families() {
        let gm = |p| Instrument {
            midi_program: p,
            percussion: false,
        };
        assert!(gm(25).is_fretted() && gm(33).is_fretted() && gm(105).is_fretted());
        assert!(!gm(35).is_fretted(), "fretless bass");
        assert!(!gm(40).is_fretted() && !gm(0).is_fretted() && !gm(38).is_fretted());
        assert!(gm(35).is_bass() && gm(38).is_bass() && !gm(30).is_bass());
        assert!(gm(0).is_keyboard() && gm(18).is_keyboard() && !gm(22).is_keyboard());
        let drums = Instrument {
            midi_program: 0,
            percussion: true,
        };
        assert!(
            Instrument::default().is_fretted(),
            "a new track is a guitar"
        );
        assert!(!drums.is_keyboard() && !drums.is_fretted());
    }

    #[test]
    fn pitch_with_capo() {
        let mut staff = Staff {
            tuning: Staff::standard_guitar_tuning(),
            ..Staff::default()
        };
        assert_eq!(staff.pitch_of(6, 0), Some(Pitch(40)));
        assert_eq!(staff.pitch_of(1, 5), Some(Pitch(69)));
        staff.capo = 2;
        assert_eq!(staff.pitch_of(1, 0), Some(Pitch(66)));
        assert_eq!(staff.pitch_of(0, 0), None);
        assert_eq!(staff.pitch_of(7, 0), None);
        assert_eq!(staff.pitch_of(1, 200), None);
    }
}
