// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Note values, dots and tuplets.

use crate::Ratio;

/// The written value of a note or rest, before dots and tuplets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoteValue {
    /// Double whole note (breve).
    DoubleWhole,
    /// Whole note.
    Whole,
    /// Half note.
    Half,
    /// Quarter note.
    Quarter,
    /// Eighth note.
    Eighth,
    /// Sixteenth note.
    Sixteenth,
    /// Thirty-second note.
    ThirtySecond,
    /// Sixty-fourth note.
    SixtyFourth,
    /// Hundred-twenty-eighth note.
    OneTwentyEighth,
    /// Two-hundred-fifty-sixth note.
    TwoFiftySixth,
}

impl NoteValue {
    /// The undotted length in whole notes.
    pub fn length(self) -> Ratio {
        let (n, d) = match self {
            NoteValue::DoubleWhole => (2, 1),
            NoteValue::Whole => (1, 1),
            NoteValue::Half => (1, 2),
            NoteValue::Quarter => (1, 4),
            NoteValue::Eighth => (1, 8),
            NoteValue::Sixteenth => (1, 16),
            NoteValue::ThirtySecond => (1, 32),
            NoteValue::SixtyFourth => (1, 64),
            NoteValue::OneTwentyEighth => (1, 128),
            NoteValue::TwoFiftySixth => (1, 256),
        };
        Ratio::new(n, d).unwrap_or(Ratio::ZERO)
    }

    /// The value from its usual denominator: 1 for whole, 4 for quarter, and so on.
    pub fn from_denominator(d: u16) -> Option<NoteValue> {
        Some(match d {
            1 => NoteValue::Whole,
            2 => NoteValue::Half,
            4 => NoteValue::Quarter,
            8 => NoteValue::Eighth,
            16 => NoteValue::Sixteenth,
            32 => NoteValue::ThirtySecond,
            64 => NoteValue::SixtyFourth,
            128 => NoteValue::OneTwentyEighth,
            256 => NoteValue::TwoFiftySixth,
            _ => return None,
        })
    }

    /// The usual denominator (4 for a quarter note); 0 for a double whole note.
    pub fn denominator(self) -> u16 {
        match self {
            NoteValue::DoubleWhole => 0,
            NoteValue::Whole => 1,
            NoteValue::Half => 2,
            NoteValue::Quarter => 4,
            NoteValue::Eighth => 8,
            NoteValue::Sixteenth => 16,
            NoteValue::ThirtySecond => 32,
            NoteValue::SixtyFourth => 64,
            NoteValue::OneTwentyEighth => 128,
            NoteValue::TwoFiftySixth => 256,
        }
    }
}

/// A tuplet ratio: `notes` notes played in the time of `in_time_of`.
///
/// A triplet is `3:2`, a quintuplet usually `5:4`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tuplet {
    /// How many notes are played.
    pub notes: u8,
    /// In the time normally taken by this many.
    pub in_time_of: u8,
}

impl Tuplet {
    /// The triplet, 3 in the time of 2.
    pub const TRIPLET: Tuplet = Tuplet {
        notes: 3,
        in_time_of: 2,
    };

    /// The factor that scales a note's length: `in_time_of / notes`.
    /// `None` if either number is zero.
    pub fn factor(self) -> Option<Ratio> {
        if self.notes == 0 || self.in_time_of == 0 {
            return None;
        }
        Ratio::new(i64::from(self.in_time_of), i64::from(self.notes))
    }
}

/// The full written duration of a beat: value, dots and any tuplets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Duration {
    /// The written note value.
    pub value: NoteValue,
    /// Number of augmentation dots (0 to 3 in practice).
    pub dots: u8,
    /// Tuplets this beat belongs to, outermost first. Empty for none;
    /// two or more entries for nested tuplets.
    pub tuplets: Vec<Tuplet>,
}

impl Duration {
    /// An undotted duration with no tuplet.
    pub fn new(value: NoteValue) -> Duration {
        Duration {
            value,
            dots: 0,
            tuplets: Vec::new(),
        }
    }

    /// The real length in whole notes, or `None` if a tuplet is invalid or the
    /// numbers overflow (possible only with nonsense from a damaged file).
    pub fn length(&self) -> Option<Ratio> {
        let base = self.value.length();
        // Each dot adds half of the previous addition: 1 + 1/2 + 1/4 ...
        // so the total is base * (2 - 1/2^dots).
        let mut len = base;
        let mut add = base;
        for _ in 0..self.dots {
            add = add.checked_mul(Ratio::new(1, 2)?)?;
            len = len.checked_add(add)?;
        }
        for t in &self.tuplets {
            len = len.checked_mul(t.factor()?)?;
        }
        Some(len)
    }
}

impl Default for Duration {
    fn default() -> Self {
        Duration::new(NoteValue::Quarter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    #[test]
    fn dotted_and_tuplets() {
        let mut d = Duration::new(NoteValue::Quarter);
        assert_eq!(d.length(), Some(r(1, 4)));
        d.dots = 1;
        assert_eq!(d.length(), Some(r(3, 8)));
        d.dots = 2;
        assert_eq!(d.length(), Some(r(7, 16)));

        let mut t = Duration::new(NoteValue::Eighth);
        t.tuplets.push(Tuplet::TRIPLET);
        assert_eq!(t.length(), Some(r(1, 12)));
        // A triplet inside a quintuplet.
        t.tuplets.insert(
            0,
            Tuplet {
                notes: 5,
                in_time_of: 4,
            },
        );
        assert_eq!(t.length(), Some(r(1, 15)));
    }

    #[test]
    fn invalid_tuplet() {
        let mut d = Duration::new(NoteValue::Quarter);
        d.tuplets.push(Tuplet {
            notes: 0,
            in_time_of: 2,
        });
        assert_eq!(d.length(), None);
    }

    #[test]
    fn denominators_round_trip() {
        for d in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
            assert_eq!(NoteValue::from_denominator(d).unwrap().denominator(), d);
        }
        assert_eq!(NoteValue::from_denominator(3), None);
    }
}
