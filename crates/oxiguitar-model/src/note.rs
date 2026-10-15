// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Notes and the effects attached to them.

use crate::{NoteValue, Ratio};

/// A MIDI pitch number: 60 is middle C, 40 the low E of a guitar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pitch(pub u8);

impl Pitch {
    /// The note name with octave in scientific pitch notation, using sharps:
    /// `Pitch(60)` is "C4".
    pub fn name(self) -> String {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let octave = i32::from(self.0 / 12) - 1;
        let name = NAMES.get(usize::from(self.0 % 12)).copied().unwrap_or("?");
        format!("{name}{octave}")
    }
}

/// Where a note is: on a string and fret, or as a plain pitch for instruments
/// without strings and for files that carry no tablature (until tab derivation
/// assigns strings, TAB-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotePlace {
    /// String (1 = highest) and fret, counted from the capo.
    Fretted {
        /// String number, 1 for the highest.
        string: u8,
        /// Fret number counted from the capo; 0 for the open string.
        fret: u8,
    },
    /// A pitch with no string assigned.
    Pitched(Pitch),
}

/// A single note in a beat.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// String and fret, or pitch.
    pub place: NotePlace,
    /// Tied to the previous note on the same string: not struck again.
    pub tie: bool,
    /// How loud.
    pub dynamic: Dynamic,
    /// Effects on this note.
    pub effects: NoteEffects,
}

impl Note {
    /// A plain note on a string and fret.
    pub fn fretted(string: u8, fret: u8) -> Note {
        Note {
            place: NotePlace::Fretted { string, fret },
            tie: false,
            dynamic: Dynamic::default(),
            effects: NoteEffects::default(),
        }
    }

    /// The string number, if the note is on a string.
    pub fn string(&self) -> Option<u8> {
        match self.place {
            NotePlace::Fretted { string, .. } => Some(string),
            NotePlace::Pitched(_) => None,
        }
    }
}

/// Dynamics, from ppp to fff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dynamic {
    /// Pianississimo.
    PPP,
    /// Pianissimo.
    PP,
    /// Piano.
    P,
    /// Mezzo-piano.
    MP,
    /// Mezzo-forte.
    MF,
    /// Forte (Guitar Pro's default).
    #[default]
    F,
    /// Fortissimo.
    FF,
    /// Fortississimo.
    FFF,
}

/// Effects that act on a single note (TAB-1 core set).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteEffects {
    /// Hammer-on or pull-off to the next note on this string.
    pub hammer_pull: bool,
    /// Slides into, out of, or to the next note.
    pub slides: Vec<Slide>,
    /// String bend.
    pub bend: Option<Bend>,
    /// Harmonic.
    pub harmonic: Option<Harmonic>,
    /// Vibrato.
    pub vibrato: bool,
    /// Palm mute.
    pub palm_mute: bool,
    /// Let ring.
    pub let_ring: bool,
    /// Dead (muted, percussive) note.
    pub dead: bool,
    /// Ghost note (in brackets, played softly).
    pub ghost: bool,
    /// Accent.
    pub accent: Option<Accent>,
    /// Staccato.
    pub staccato: bool,
    /// Trill with another fret.
    pub trill: Option<Trill>,
    /// Tremolo picking on this note.
    pub tremolo_picking: Option<NoteValue>,
    /// Left-hand finger (thumb = 0, index = 1 … little = 4).
    pub left_finger: Option<u8>,
    /// Right-hand finger (p = 0, i = 1, m = 2, a = 3, c = 4).
    pub right_finger: Option<u8>,
    /// Left-hand tapping on this note.
    pub left_hand_tap: bool,
}

/// Accent strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Accent {
    /// Normal accent (>).
    Normal,
    /// Heavy accent (marcato, ^).
    Heavy,
}

/// The kinds of slide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slide {
    /// Slide to the next note and strike it again.
    Shift,
    /// Slide to the next note without striking it again.
    Legato,
    /// Slide into the note from below.
    InFromBelow,
    /// Slide into the note from above.
    InFromAbove,
    /// Slide out of the note downwards.
    OutDown,
    /// Slide out of the note upwards.
    OutUp,
}

/// A bend or whammy-bar curve.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bend {
    /// Points of the curve, in time order.
    pub points: Vec<BendPoint>,
}

/// One point of a bend curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BendPoint {
    /// When, as a fraction of the note's duration, from 0 to 1.
    pub offset: Ratio,
    /// How far, in cents (100 = one semitone). Negative for whammy dips.
    pub cents: i32,
}

/// Harmonics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Harmonic {
    /// Natural harmonic.
    Natural,
    /// Artificial harmonic, sounding at the given offset in semitones above the
    /// fretted note.
    Artificial {
        /// Semitones above the fretted note.
        semitones: u8,
    },
    /// Tapped harmonic, touched at a fret above the fretted one.
    Tapped {
        /// The fret touched.
        fret: u8,
    },
    /// Pinch harmonic.
    Pinch,
    /// Semi-harmonic.
    Semi,
    /// Feedback.
    Feedback,
}

/// A trill: alternation with another fret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Trill {
    /// The other fret.
    pub fret: u8,
    /// Speed, as the value of each alternation.
    pub speed: NoteValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_names() {
        assert_eq!(Pitch(60).name(), "C4");
        assert_eq!(Pitch(40).name(), "E2");
        assert_eq!(Pitch(0).name(), "C-1");
        assert_eq!(Pitch(127).name(), "G9");
    }
}
