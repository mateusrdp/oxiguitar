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
    /// How the note is spelled on the staff, when chosen (in a file or with
    /// the accidental tools) rather than worked out from the key. It counts
    /// only while it spells the note's written pitch ([`Spelling::spells`]:
    /// the sounding pitch plus the staff's transposition).
    pub spelling: Option<Spelling>,
}

/// A note's name on the staff, as written: a step and an alteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spelling {
    /// The step: 0 for C, 1 for D … 6 for B.
    pub step: u8,
    /// Semitones from the step's natural: -2 double flat … 2 double sharp.
    pub alter: i8,
}

impl Spelling {
    /// The semitones above C of each natural step.
    const NATURALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

    /// Whether it names `pitch` (a written pitch, in any octave).
    pub fn spells(self, pitch: Pitch) -> bool {
        Spelling::NATURALS
            .get(usize::from(self.step))
            .is_some_and(|n| (n + i32::from(self.alter) - i32::from(pitch.0)).rem_euclid(12) == 0)
            && (-2..=2).contains(&self.alter)
    }

    /// The spelling of `pitch` with alteration `alter`, if one exists (C
    /// with a sharp has none: it would be B sharp, a step below).
    pub fn with_alter(pitch: Pitch, alter: i8) -> Option<Spelling> {
        (0..7u8)
            .map(|step| Spelling { step, alter })
            .find(|s| s.spells(pitch))
    }

    /// The spelling OxiGuitar works out for written `pitch` in `key`: with
    /// flats in flat keys, sharps otherwise, except that in flat keys the
    /// relative minor's leading tone stays sharp (F sharp in B flat major
    /// and G minor, C sharp in F major and D minor).
    pub fn automatic(pitch: Pitch, key: crate::KeySignature) -> Spelling {
        const SHARPS: [(u8, i8); 12] = [
            (0, 0),
            (0, 1),
            (1, 0),
            (1, 1),
            (2, 0),
            (3, 0),
            (3, 1),
            (4, 0),
            (4, 1),
            (5, 0),
            (5, 1),
            (6, 0),
        ];
        const FLATS: [(u8, i8); 12] = [
            (0, 0),
            (1, -1),
            (1, 0),
            (2, -1),
            (2, 0),
            (3, 0),
            (4, -1),
            (4, 0),
            (5, -1),
            (5, 0),
            (6, -1),
            (6, 0),
        ];
        let pc = usize::from(pitch.0 % 12);
        let major_tonic = (7 * i32::from(key.fifths)).rem_euclid(12);
        let leading_tone = ((major_tonic + 9 + 11) % 12) as usize;
        let table = if key.fifths < 0 && pc != leading_tone {
            &FLATS
        } else {
            &SHARPS
        };
        let (step, alter) = table.get(pc).copied().unwrap_or((0, 0));
        Spelling { step, alter }
    }

    /// Every spelling of `pitch`, from double sharp to double flat.
    pub fn all(pitch: Pitch) -> Vec<Spelling> {
        (-2..=2)
            .rev()
            .filter_map(|alter| Spelling::with_alter(pitch, alter))
            .collect()
    }

    /// The octave the written note is in (as in scientific pitch
    /// notation): B sharp 3 sounds as C 4, C flat 4 as B 3.
    pub fn octave(self, pitch: Pitch) -> i32 {
        let natural = Spelling::NATURALS
            .get(usize::from(self.step))
            .copied()
            .unwrap_or(0);
        (i32::from(pitch.0) - i32::from(self.alter) - natural).div_euclid(12) - 1
    }
}

impl Note {
    /// A plain note on a string and fret.
    pub fn fretted(string: u8, fret: u8) -> Note {
        Note {
            place: NotePlace::Fretted { string, fret },
            tie: false,
            dynamic: Dynamic::default(),
            effects: NoteEffects::default(),
            spelling: None,
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
    /// Left-hand vibrato.
    pub vibrato: Option<crate::Vibrato>,
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
    /// An ornament.
    pub ornament: Option<Ornament>,
    /// Show the string's number on the staff.
    pub string_number: bool,
}

/// An ornament on a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ornament {
    /// The note, the step above, the note.
    UpperMordent,
    /// The note, the step below, the note.
    LowerMordent,
    /// The step above, the note, the step below, the note.
    Turn,
    /// The step below, the note, the step above, the note.
    InvertedTurn,
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

    #[test]
    fn spellings() {
        // G sharp 4 / A flat 4.
        let p = Pitch(68);
        assert_eq!(
            Spelling::with_alter(p, 1),
            Some(Spelling { step: 4, alter: 1 })
        );
        assert_eq!(
            Spelling::with_alter(p, -1),
            Some(Spelling { step: 5, alter: -1 })
        );
        assert_eq!(Spelling::with_alter(p, 0), None);
        assert_eq!(Spelling::all(p).len(), 2);
        // C 4 is also B sharp 3 and D double flat 4.
        let c = Pitch(60);
        let b_sharp = Spelling::with_alter(c, 1).unwrap();
        assert_eq!(b_sharp, Spelling { step: 6, alter: 1 });
        assert_eq!(b_sharp.octave(c), 3);
        assert_eq!(
            Spelling::with_alter(c, -2),
            Some(Spelling { step: 1, alter: -2 })
        );
        assert_eq!(Spelling { step: 0, alter: 0 }.octave(c), 4);
        // C flat 4 sounds as B 3.
        assert_eq!(Spelling { step: 0, alter: -1 }.octave(Pitch(59)), 4);
        assert!(!Spelling { step: 0, alter: 0 }.spells(Pitch(61)));
    }

    #[test]
    fn automatic_spellings_follow_the_key() {
        use crate::KeySignature;
        let c_major = KeySignature::default();
        let f_major = KeySignature {
            fifths: -1,
            ..KeySignature::default()
        };
        // G sharp in C, A flat in F; C sharp stays sharp in F (D minor's
        // leading tone).
        assert_eq!(
            Spelling::automatic(Pitch(68), c_major),
            Spelling { step: 4, alter: 1 }
        );
        assert_eq!(
            Spelling::automatic(Pitch(68), f_major),
            Spelling { step: 5, alter: -1 }
        );
        assert_eq!(
            Spelling::automatic(Pitch(61), f_major),
            Spelling { step: 0, alter: 1 }
        );
    }
}
