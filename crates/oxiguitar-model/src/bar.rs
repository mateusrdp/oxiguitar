// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Bars, voices and beats.

use crate::{Duration, Note, Ratio};

/// One bar of one staff.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    /// Clef in force in this bar.
    pub clef: Clef,
    /// Up to four independent voices (EDIT-3). The first is always present.
    pub voices: Vec<Voice>,
    /// A simile (repeat) sign drawn in place of the bar's music, which is a
    /// copy of the bar or bars it repeats (docs/design/palette.md).
    pub simile: Option<Simile>,
}

impl Default for Bar {
    fn default() -> Self {
        Bar {
            clef: Clef::Treble8vb,
            voices: vec![Voice::default()],
            simile: None,
        }
    }
}

/// A simile sign: the bar repeats the bar before it, or is one of two bars
/// that repeat the two before them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Simile {
    /// Repeats the bar before.
    Simple,
    /// The first of two bars repeating the two before them.
    FirstOfDouble,
    /// The second of two bars repeating the two before them.
    SecondOfDouble,
}

/// A clef.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Clef {
    /// Treble clef.
    Treble,
    /// Treble clef with an 8 below: the usual guitar clef.
    #[default]
    Treble8vb,
    /// Bass clef.
    Bass,
    /// Bass clef with an 8 below: the usual bass guitar clef.
    Bass8vb,
    /// Alto clef.
    Alto,
    /// Tenor clef.
    Tenor,
    /// Neutral (percussion) clef.
    Neutral,
}

/// One voice of a bar: a sequence of beats.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Voice {
    /// The beats, in time order.
    pub beats: Vec<Beat>,
}

impl Voice {
    /// The total length of the beats, in whole notes. `None` if any duration
    /// is invalid.
    pub fn length(&self) -> Option<Ratio> {
        self.beats
            .iter()
            .filter(|b| b.grace.is_none())
            .try_fold(Ratio::ZERO, |acc, b| acc.checked_add(b.duration.length()?))
    }

    /// True if the voice has no beats with notes or rests.
    pub fn is_empty(&self) -> bool {
        self.beats.iter().all(|b| b.kind == BeatKind::Empty)
    }
}

/// What a beat contains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BeatKind {
    /// One or more notes (a single note or a chord).
    #[default]
    Notes,
    /// A rest.
    Rest,
    /// An empty placeholder that takes time but draws nothing, as Guitar Pro
    /// uses for unfilled voices.
    Empty,
}

/// A point in time in one voice: a note, a chord or a rest, with one duration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Beat {
    /// Notes, rest or empty.
    pub kind: BeatKind,
    /// The written duration.
    pub duration: Duration,
    /// The notes sounding together; empty for rests.
    pub notes: Vec<Note>,
    /// If set, this beat is a grace beat placed before the next main beat and
    /// takes no time in the bar.
    pub grace: Option<GraceKind>,
    /// Effects that apply to the whole beat.
    pub effects: BeatEffects,
    /// Index into the track's chord diagrams, if a chord is shown here.
    pub chord: Option<usize>,
    /// Free text above the beat.
    pub text: String,
    /// Slurred to the next beat of its voice: a slur is a run of such beats
    /// and the beat after the last.
    pub slur: bool,
    /// Under a crescendo or decrescendo hairpin: a hairpin is a run of
    /// beats with the same one.
    pub hairpin: Option<Hairpin>,
    /// Under an ottava line: a line is a run of beats with the same one.
    pub ottava: Option<Ottava>,
    /// Drawn with slash noteheads (rhythm only) on the staff.
    pub slash: bool,
    /// How the beat's beam joins the next beat's.
    pub beaming: Beaming,
    /// Force Groups: equal tuplets from this beat on are bracketed as one
    /// group.
    pub tuplet_group: bool,
    /// Under an extended barre: a barre is a run of beats with the same one.
    pub barre: Option<Barre>,
}

/// A hairpin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hairpin {
    /// Getting louder.
    Crescendo,
    /// Getting softer.
    Decrescendo,
}

/// An ottava line: the notes under it are written one or two octaves from
/// where they sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ottava {
    /// 8va: sounds an octave higher than written.
    Alta8,
    /// 8vb: sounds an octave lower than written.
    Bassa8,
    /// 15ma: sounds two octaves higher than written.
    Alta15,
    /// 15mb: sounds two octaves lower than written.
    Bassa15,
}

impl Ottava {
    /// Semitones the music sounds above where it is written.
    pub fn shift(self) -> i32 {
        match self {
            Ottava::Alta8 => 12,
            Ottava::Bassa8 => -12,
            Ottava::Alta15 => 24,
            Ottava::Bassa15 => -24,
        }
    }
}

/// How a beat's beam joins the next beat's (Guitar Pro 8's beaming tools).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Beaming {
    /// By the time signature.
    #[default]
    Automatic,
    /// Beamed to the next beat.
    Force,
    /// Not beamed to the next beat.
    Break,
    /// Only the first beam continues to the next beat.
    BreakSecondary,
}

/// An extended barre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Barre {
    /// The fret, counted from the capo.
    pub fret: u8,
    /// A half (partial) barre rather than one across every string.
    pub half: bool,
}

/// How a grace note relates to its main note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraceKind {
    /// Played before the beat (acciaccatura).
    BeforeBeat,
    /// Played on the beat, taking time from the main note.
    OnBeat,
}

/// Effects that act on a whole beat.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeatEffects {
    /// Strum direction and speed.
    pub stroke: Option<Stroke>,
    /// Pick stroke marking (up or down).
    pub pick_stroke: Option<StrokeDirection>,
    /// Whammy bar movement.
    pub whammy: Option<crate::Bend>,
    /// Tapping, slapping or popping.
    pub technique: Option<BeatTechnique>,
    /// A fade in, fade out or volume swell.
    pub fade: Option<Fade>,
    /// Vibrato with the whammy bar, on the whole beat.
    pub bridge_vibrato: Option<Vibrato>,
    /// Tremolo picking: notes per beat value, such as `NoteValue::Sixteenth`.
    pub tremolo_picking: Option<crate::NoteValue>,
    /// Dynamic marking that starts here.
    pub dynamic: Option<crate::Dynamic>,
    /// Let all notes ring until the next rest.
    pub let_ring: bool,
    /// The sustain pedal goes down or up at this beat.
    pub pedal: Option<Pedal>,
    /// A wah-wah mark.
    pub wah: Option<Wah>,
    /// The chord played as an arpeggio, up or down.
    pub arpeggio: Option<StrokeDirection>,
    /// A rasgueado.
    pub rasgueado: Option<Rasgueado>,
    /// A golpe (a tap on the body).
    pub golpe: Option<Golpe>,
    /// A pick scrape along the strings, up (towards the neck) or down.
    pub pick_scrape: Option<StrokeDirection>,
    /// A dead slap.
    pub dead_slap: bool,
}

/// How the volume moves over a beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fade {
    /// From silence up to the beat's volume.
    In,
    /// From the beat's volume down to silence.
    Out,
    /// Up from silence and back down.
    Swell,
}

/// How wide a vibrato is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vibrato {
    /// Slight.
    Slight,
    /// Wide.
    Wide,
}

/// The sustain pedal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pedal {
    /// Pressed.
    Down,
    /// Released.
    Up,
}

/// A wah-wah pedal mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wah {
    /// Open (o).
    Open,
    /// Closed (+).
    Closed,
}

/// Which part of the hand taps the body in a golpe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Golpe {
    /// A finger.
    Finger,
    /// The thumb.
    Thumb,
}

/// Guitar Pro's rasgueado patterns, by their fingers: p thumb, i index,
/// m middle, a ring, e little finger; triplet or anapaest rhythm where
/// Guitar Pro offers both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rasgueado {
    /// i i.
    Ii,
    /// m i.
    Mi,
    /// m i i, triplet.
    MiiTriplet,
    /// m i i, anapaest.
    MiiAnapaest,
    /// p m p, triplet.
    PmpTriplet,
    /// p m p, anapaest.
    PmpAnapaest,
    /// p e i, triplet.
    PeiTriplet,
    /// p e i, anapaest.
    PeiAnapaest,
    /// p a i, triplet.
    PaiTriplet,
    /// p a i, anapaest.
    PaiAnapaest,
    /// a m i, triplet.
    AmiTriplet,
    /// a m i, anapaest.
    AmiAnapaest,
    /// p p p.
    Ppp,
    /// a m i i.
    Amii,
    /// a m i p.
    Amip,
    /// e a m i.
    Eami,
    /// e a m i i.
    Eamii,
    /// p e a m i.
    Peami,
}

impl Rasgueado {
    /// Every pattern, in Guitar Pro's order.
    pub const ALL: [Rasgueado; 18] = [
        Rasgueado::Ii,
        Rasgueado::Mi,
        Rasgueado::MiiTriplet,
        Rasgueado::MiiAnapaest,
        Rasgueado::PmpTriplet,
        Rasgueado::PmpAnapaest,
        Rasgueado::PeiTriplet,
        Rasgueado::PeiAnapaest,
        Rasgueado::PaiTriplet,
        Rasgueado::PaiAnapaest,
        Rasgueado::AmiTriplet,
        Rasgueado::AmiAnapaest,
        Rasgueado::Ppp,
        Rasgueado::Amii,
        Rasgueado::Amip,
        Rasgueado::Eami,
        Rasgueado::Eamii,
        Rasgueado::Peami,
    ];

    /// Its fingers, in the order they strike.
    pub fn fingers(self) -> &'static str {
        match self {
            Rasgueado::Ii => "ii",
            Rasgueado::Mi => "mi",
            Rasgueado::MiiTriplet | Rasgueado::MiiAnapaest => "mii",
            Rasgueado::PmpTriplet | Rasgueado::PmpAnapaest => "pmp",
            Rasgueado::PeiTriplet | Rasgueado::PeiAnapaest => "pei",
            Rasgueado::PaiTriplet | Rasgueado::PaiAnapaest => "pai",
            Rasgueado::AmiTriplet | Rasgueado::AmiAnapaest => "ami",
            Rasgueado::Ppp => "ppp",
            Rasgueado::Amii => "amii",
            Rasgueado::Amip => "amip",
            Rasgueado::Eami => "eami",
            Rasgueado::Eamii => "eamii",
            Rasgueado::Peami => "peami",
        }
    }

    /// Whether its strokes are short-short-long (anapaest) rather than even.
    pub fn anapaest(self) -> bool {
        matches!(
            self,
            Rasgueado::MiiAnapaest
                | Rasgueado::PmpAnapaest
                | Rasgueado::PeiAnapaest
                | Rasgueado::PaiAnapaest
                | Rasgueado::AmiAnapaest
        )
    }

    /// Its name as the Inspector shows it ("m i i, anapaest").
    pub fn name(self) -> String {
        let fingers: Vec<String> = self.fingers().chars().map(String::from).collect();
        let fingers = fingers.join(" ");
        match self {
            Rasgueado::MiiTriplet
            | Rasgueado::PmpTriplet
            | Rasgueado::PeiTriplet
            | Rasgueado::PaiTriplet
            | Rasgueado::AmiTriplet => format!("{fingers}, triplet"),
            _ if self.anapaest() => format!("{fingers}, anapaest"),
            _ => fingers,
        }
    }
}

/// A strum: direction and how long it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stroke {
    /// Up or down.
    pub direction: StrokeDirection,
    /// The time the strum spreads over, as a note value.
    pub speed: crate::NoteValue,
}

/// Up or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StrokeDirection {
    /// Towards the floor (from the low string).
    Down,
    /// Towards the ceiling (from the high string).
    Up,
}

/// Right-hand techniques on a beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BeatTechnique {
    /// Tapping.
    Tap,
    /// Slapping.
    Slap,
    /// Popping.
    Pop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoteValue;

    #[test]
    fn grace_beats_take_no_time() {
        let mut v = Voice::default();
        v.beats.push(Beat {
            duration: Duration::new(NoteValue::Half),
            ..Beat::default()
        });
        v.beats.push(Beat {
            duration: Duration::new(NoteValue::ThirtySecond),
            grace: Some(GraceKind::BeforeBeat),
            ..Beat::default()
        });
        v.beats.push(Beat {
            duration: Duration::new(NoteValue::Half),
            ..Beat::default()
        });
        assert_eq!(v.length(), Some(Ratio::ONE));
    }
}
