// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Bar dialog's fields (M3 slice 4), without window code: what the
//! caret's bar carries as the dialog shows it, and back.

use oxiguitar_edit::BarState;
use oxiguitar_model::{
    Clef, Direction, KeyMode, KeySignature, Marker, TimeSignature, TripletFeel, Tuplet,
};

/// The bottom numbers a time signature offers.
pub(crate) const DENOMINATORS: [u8; 6] = [1, 2, 4, 8, 16, 32];

/// The clefs offered, in order. A staff that sounds an octave lower keeps
/// doing so (`oxiguitar_edit`'s clef command).
pub(crate) const CLEFS: [(&str, Clef); 5] = [
    ("Treble", Clef::Treble),
    ("Bass", Clef::Bass),
    ("Alto", Clef::Alto),
    ("Tenor", Clef::Tenor),
    ("Neutral", Clef::Neutral),
];

/// The feels offered, in order.
pub(crate) const FEELS: [(&str, TripletFeel); 7] = [
    ("None", TripletFeel::None),
    ("Triplet eighths", TripletFeel::Triplet8th),
    ("Triplet sixteenths", TripletFeel::Triplet16th),
    ("Dotted eighths", TripletFeel::Dotted8th),
    ("Dotted sixteenths", TripletFeel::Dotted16th),
    ("Scottish eighths", TripletFeel::Scottish8th),
    ("Scottish sixteenths", TripletFeel::Scottish16th),
];

/// The directions offered, in order: the signs, then the jumps.
pub(crate) const DIRECTIONS: [(&str, Direction); 19] = [
    ("Segno", Direction::Segno),
    ("Segno segno", Direction::SegnoSegno),
    ("Coda", Direction::Coda),
    ("Double coda", Direction::DoubleCoda),
    ("Fine", Direction::Fine),
    ("To coda", Direction::ToCoda),
    ("To double coda", Direction::ToDoubleCoda),
    ("D.C.", Direction::DaCapo),
    ("D.C. al coda", Direction::DaCapoAlCoda),
    ("D.C. al double coda", Direction::DaCapoAlDoubleCoda),
    ("D.C. al fine", Direction::DaCapoAlFine),
    ("D.S.", Direction::DalSegno),
    ("D.S. al coda", Direction::DalSegnoAlCoda),
    ("D.S. al double coda", Direction::DalSegnoAlDoubleCoda),
    ("D.S. al fine", Direction::DalSegnoAlFine),
    ("D.S.S.", Direction::DalSegnoSegno),
    ("D.S.S. al coda", Direction::DalSegnoSegnoAlCoda),
    (
        "D.S.S. al double coda",
        Direction::DalSegnoSegnoAlDoubleCoda,
    ),
    ("D.S.S. al fine", Direction::DalSegnoSegnoAlFine),
];

/// How many alternate endings the dialog offers.
pub(crate) const ENDINGS: usize = 8;

/// The keys offered, from seven flats to seven sharps: the major key and
/// its relative minor.
pub(crate) fn key_names() -> Vec<String> {
    const MAJOR: [&str; 15] = [
        "C♭", "G♭", "D♭", "A♭", "E♭", "B♭", "F", "C", "G", "D", "A", "E", "B", "F♯", "C♯",
    ];
    const MINOR: [&str; 15] = [
        "A♭", "E♭", "B♭", "F", "C", "G", "D", "A", "E", "B", "F♯", "C♯", "G♯", "D♯", "A♯",
    ];
    MAJOR
        .iter()
        .zip(MINOR)
        .zip(-7i8..=7)
        .map(|((major, minor), fifths)| {
            let sign = match fifths {
                0 => String::new(),
                f if f > 0 => format!(" ({f}♯)"),
                f => format!(" ({}♭)", -f),
            };
            format!("{major} major / {minor} minor{sign}")
        })
        .collect()
}

/// The dialog's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BarForm {
    pub(crate) numerator: i32,
    /// Index in [`DENOMINATORS`].
    pub(crate) denominator: i32,
    /// 0 for seven flats to 14 for seven sharps.
    pub(crate) key: i32,
    pub(crate) minor: bool,
    /// Index in [`CLEFS`].
    pub(crate) clef: i32,
    pub(crate) tempo: i32,
    /// Index in [`FEELS`].
    pub(crate) feel: i32,
    pub(crate) repeat_start: bool,
    /// How many times the repeat closing here plays; 0 for none.
    pub(crate) repeat_count: i32,
    pub(crate) letter: String,
    pub(crate) section: String,
    pub(crate) double_bar: bool,
    pub(crate) free_time: bool,
    /// One per ending, from 1.
    pub(crate) endings: Vec<bool>,
    /// One per [`DIRECTIONS`] entry.
    pub(crate) directions: Vec<bool>,
}

impl BarForm {
    /// The fields for a bar.
    pub(crate) fn of(b: &BarState) -> BarForm {
        let index = |found: Option<usize>| found.map_or(0, |i| i as i32);
        let clef = match b.clef {
            Clef::Treble8vb => Clef::Treble,
            Clef::Bass8vb => Clef::Bass,
            c => c,
        };
        BarForm {
            numerator: i32::from(b.time_signature.numerator),
            denominator: index(
                DENOMINATORS
                    .iter()
                    .position(|d| *d == b.time_signature.denominator),
            ),
            key: i32::from(b.key_signature.fifths.clamp(-7, 7)) + 7,
            minor: b.key_signature.mode == KeyMode::Minor,
            clef: index(CLEFS.iter().position(|(_, c)| *c == clef)),
            tempo: b.tempo.round().clamp(1.0, 2000.0) as i32,
            feel: index(FEELS.iter().position(|(_, f)| *f == b.triplet_feel)),
            repeat_start: b.repeat_start,
            repeat_count: b.repeat_end.map_or(0, i32::from),
            letter: b
                .marker
                .as_ref()
                .map(|m| m.letter.clone())
                .unwrap_or_default(),
            section: b
                .marker
                .as_ref()
                .map(|m| m.text.clone())
                .unwrap_or_default(),
            double_bar: b.double_bar,
            free_time: b.free_time,
            endings: (0..ENDINGS)
                .map(|i| b.alternate_endings & (1 << i) != 0)
                .collect(),
            directions: DIRECTIONS
                .iter()
                .map(|(_, d)| b.directions.contains(d))
                .collect(),
        }
    }

    /// The bar the fields ask for, from `was` (what the bar had, so what
    /// the dialog doesn't show, a section's colour or endings past the
    /// eighth, is kept).
    pub(crate) fn to_state(&self, was: &BarState) -> BarState {
        let pick = |i: i32| usize::try_from(i).unwrap_or(0);
        let clef = CLEFS.get(pick(self.clef)).map_or(was.clef, |(_, c)| *c);
        // The same clef as before, whatever its octave, is no change.
        let clef = match (clef, was.clef) {
            (Clef::Treble, Clef::Treble8vb) | (Clef::Bass, Clef::Bass8vb) => was.clef,
            (c, _) => c,
        };
        let mut endings = was.alternate_endings & !((1u16 << ENDINGS) - 1);
        for (i, on) in self.endings.iter().enumerate().take(ENDINGS) {
            if *on {
                endings |= 1 << i;
            }
        }
        let marker =
            (!(self.letter.trim().is_empty() && self.section.trim().is_empty())).then(|| Marker {
                letter: self.letter.trim().to_owned(),
                text: self.section.trim().to_owned(),
                color: was.marker.as_ref().map(|m| m.color).unwrap_or_default(),
            });
        // Directions the dialog lists keep its order; any others stay.
        let mut directions: Vec<Direction> = was
            .directions
            .iter()
            .copied()
            .filter(|d| !DIRECTIONS.iter().any(|(_, x)| x == d))
            .collect();
        directions.extend(
            DIRECTIONS
                .iter()
                .zip(&self.directions)
                .filter(|(_, on)| **on)
                .map(|((_, d), _)| *d),
        );
        // Kept in the bar's own order where it had them, so an untouched
        // list is no change.
        if directions.len() == was.directions.len()
            && directions.iter().all(|d| was.directions.contains(d))
        {
            directions.clone_from(&was.directions);
        }
        BarState {
            time_signature: TimeSignature {
                numerator: u8::try_from(self.numerator.clamp(1, 32)).unwrap_or(4),
                denominator: DENOMINATORS
                    .get(pick(self.denominator))
                    .copied()
                    .unwrap_or(4),
            },
            key_signature: KeySignature {
                fifths: i8::try_from(self.key.clamp(0, 14) - 7).unwrap_or(0),
                mode: if self.minor {
                    KeyMode::Minor
                } else {
                    KeyMode::Major
                },
            },
            clef,
            tempo: if self.tempo.clamp(1, 2000) == was.tempo.round().clamp(1.0, 2000.0) as i32 {
                was.tempo
            } else {
                f64::from(self.tempo.clamp(1, 2000))
            },
            triplet_feel: FEELS
                .get(pick(self.feel))
                .map_or(was.triplet_feel, |(_, f)| *f),
            repeat_start: self.repeat_start,
            repeat_end: (self.repeat_count >= 2)
                .then(|| u8::try_from(self.repeat_count.min(99)).unwrap_or(2)),
            alternate_endings: endings,
            directions,
            marker,
            double_bar: self.double_bar,
            free_time: self.free_time,
        }
    }
}

/// The usual "in the time of" for `n` notes: the power of two just below
/// it (3 in the time of 2, 5 to 7 in the time of 4, 9 to 15 of 8, as
/// TuxGuitar offers them); for a power of two, the compound-time grouping
/// (2 in the time of 3, 4 of 3, 8 of 6).
pub(crate) fn usual_in_time_of(n: u8) -> u8 {
    if n == 2 {
        return 3;
    }
    if n.is_power_of_two() {
        return n / 4 * 3;
    }
    let mut m: u8 = 1;
    while m.saturating_mul(2) < n {
        m *= 2;
    }
    m
}

/// The tuplet a Custom N-tuplet dialog asks for, if it is one.
pub(crate) fn tuplet(n: i32, m: i32) -> Option<Tuplet> {
    let notes = u8::try_from(n).ok().filter(|n| (2..=32).contains(n))?;
    let in_time_of = u8::try_from(m).ok().filter(|m| (1..=32).contains(m))?;
    (notes != in_time_of).then_some(Tuplet { notes, in_time_of })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> BarState {
        BarState {
            time_signature: TimeSignature {
                numerator: 6,
                denominator: 8,
            },
            key_signature: KeySignature {
                fifths: -3,
                mode: KeyMode::Minor,
            },
            clef: Clef::Treble8vb,
            tempo: 92.5,
            triplet_feel: TripletFeel::Dotted8th,
            repeat_start: true,
            repeat_end: Some(3),
            alternate_endings: 0b1_0000_0101,
            directions: vec![Direction::Fine, Direction::Segno],
            marker: Some(Marker {
                letter: "A".into(),
                text: "Verse".into(),
                color: oxiguitar_model::Color { r: 9, g: 9, b: 9 },
            }),
            double_bar: true,
            free_time: false,
        }
    }

    #[test]
    fn an_untouched_dialog_changes_nothing() {
        let s = state();
        let f = BarForm::of(&s);
        assert_eq!((f.numerator, f.denominator, f.key, f.clef), (6, 3, 4, 0));
        assert_eq!(
            f.endings,
            [true, false, true, false, false, false, false, false]
        );
        assert_eq!(f.to_state(&s), s);
    }

    #[test]
    fn fields_become_the_bar() {
        let s = state();
        let mut f = BarForm::of(&s);
        f.numerator = 7;
        f.denominator = 2;
        f.key = 9;
        f.minor = false;
        f.clef = 1;
        f.tempo = 140;
        f.repeat_count = 0;
        f.letter = String::new();
        f.section = "  ".into();
        f.endings = vec![false; ENDINGS];
        f.directions = vec![false; DIRECTIONS.len()];
        f.directions[2] = true;
        let t = f.to_state(&s);
        assert_eq!(t.time_signature.numerator, 7);
        assert_eq!(t.time_signature.denominator, 4);
        assert_eq!(t.key_signature.fifths, 2);
        assert_eq!(t.clef, Clef::Bass);
        assert_eq!(t.tempo, 140.0);
        assert_eq!(t.repeat_end, None);
        assert_eq!(t.marker, None);
        // The ninth ending, which the dialog doesn't show, stays.
        assert_eq!(t.alternate_endings, 0b1_0000_0000);
        assert_eq!(t.directions, [Direction::Coda]);
        assert_eq!(key_names()[7], "C major / A minor");
        assert_eq!(key_names()[0], "C♭ major / A♭ minor (7♭)");
    }

    #[test]
    fn tuplets() {
        assert_eq!(
            (2..=13).map(usual_in_time_of).collect::<Vec<_>>(),
            [3, 2, 3, 4, 4, 4, 6, 8, 8, 8, 8, 8]
        );
        assert_eq!(tuplet(3, 2), Some(Tuplet::TRIPLET));
        assert_eq!(tuplet(4, 4), None);
        assert_eq!(tuplet(1, 1), None);
    }
}
