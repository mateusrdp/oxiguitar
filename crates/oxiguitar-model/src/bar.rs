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
}

impl Default for Bar {
    fn default() -> Self {
        Bar {
            clef: Clef::Treble8vb,
            voices: vec![Voice::default()],
        }
    }
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
    /// Fade in (volume swell from silence).
    pub fade_in: bool,
    /// Wide vibrato on the whole beat (with the whammy bar).
    pub vibrato_wide: bool,
    /// Tremolo picking: notes per beat value, such as `NoteValue::Sixteenth`.
    pub tremolo_picking: Option<crate::NoteValue>,
    /// Dynamic marking that starts here.
    pub dynamic: Option<crate::Dynamic>,
    /// Let all notes ring until the next rest.
    pub let_ring: bool,
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
