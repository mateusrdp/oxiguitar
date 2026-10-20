// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Tunings as players name them (Mateus, 19 Oct 2026; `docs/design/editor.md`,
//! "Tunings"): a key and a pattern. Standard ("E Standard", "E♭ Standard",
//! "D Standard"), Drop (the lowest string a tone down: "Drop D"), Open with
//! the root on the lowest string ("Open D", "Open E"), Open with the fifth on
//! the lowest string ("Open G", "Open A"), or Other: any note on each string,
//! named by its notes from the lowest ("D A D G A D").
//!
//! Standard and Drop are for guitars of 3 to 10 strings and basses of 3 to 10
//! strings; the open tunings for six-string guitars.

use crate::Pitch;

/// A tuning's pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// Fourths, with a major third between the G and B strings on guitar
    /// (E A D G B E), moved to the key.
    Standard,
    /// Standard with the lowest string a whole tone lower; the key names
    /// the lowest string (Drop D: D A D G B E).
    Drop,
    /// Root, fifth, root, third, fifth, root from the lowest (Open D:
    /// D A D F♯ A D; Open E).
    OpenRoot,
    /// Fifth, root, fifth, root, third, fifth from the lowest (Open G:
    /// D G D G B D; Open A).
    OpenFifth,
    /// Anything else.
    Other,
}

impl Pattern {
    /// Every pattern, as the track dialog lists them.
    pub const ALL: [Pattern; 5] = [
        Pattern::Standard,
        Pattern::Drop,
        Pattern::OpenRoot,
        Pattern::OpenFifth,
        Pattern::Other,
    ];

    /// The pattern's name in the track dialog.
    pub fn label(self) -> &'static str {
        match self {
            Pattern::Standard => "Standard",
            Pattern::Drop => "Drop",
            Pattern::OpenRoot => "Open, root on the lowest string",
            Pattern::OpenFifth => "Open, fifth on the lowest string",
            Pattern::Other => "Other",
        }
    }
}

/// A key's name, from its pitch class (0 = C), as guitarists spell tuning
/// keys: E♭ and B♭ rather than D♯ and A♯, C♯ and F♯ rather than D♭ and G♭.
pub fn key_name(pitch_class: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B",
    ];
    NAMES
        .get(usize::from(pitch_class % 12))
        .copied()
        .unwrap_or("C")
}

/// Standard tuning in E, lowest string first, for `strings` strings: a
/// guitar's (E A D G B E; fourths below for seven to ten strings, B, F♯,
/// C♯, G♯; the top strings for three to five) or a bass's (E A D G; a C
/// string above for six or more, fourths below from five: B, F♯ …; E A D
/// for three).
fn standard_in_e(strings: usize, bass: bool) -> Option<Vec<i32>> {
    if !(3..=10).contains(&strings) {
        return None;
    }
    let mut v: Vec<i32> = if bass {
        vec![28, 33, 38, 43]
    } else {
        vec![40, 45, 50, 55, 59, 64]
    };
    if bass && strings >= 6 {
        v.push(48);
    }
    while v.len() < strings {
        let lowest = *v.first()?;
        v.insert(0, lowest - 5);
    }
    if v.len() > strings {
        if bass {
            v.truncate(strings);
        } else {
            v.drain(..v.len() - strings);
        }
    }
    Some(v)
}

/// How far a tuning in key `key` (pitch class) sits from the one in E: a
/// guitar's or bass's keys run from G♯ (8 semitones below) to G (3 above).
fn shift_from_e(key: u8) -> i32 {
    let d = (i32::from(key % 12) - 4).rem_euclid(12);
    if d > 3 { d - 12 } else { d }
}

/// The tuning a key and pattern give for `strings` strings, highest string
/// first (as [`crate::Staff::tuning`] holds it); `None` for Other, or a
/// pattern that has no form for that many strings (the open tunings are
/// six-string guitar tunings).
pub fn tuning(pattern: Pattern, key: u8, strings: usize, bass: bool) -> Option<Vec<Pitch>> {
    let low_first: Vec<i32> = match pattern {
        Pattern::Standard => {
            let d = shift_from_e(key);
            standard_in_e(strings, bass)?
                .iter()
                .map(|p| p + d)
                .collect()
        }
        Pattern::Drop => {
            // Standard a tone above the key, its lowest string dropped.
            let d = shift_from_e((key % 12 + 2) % 12);
            let mut v: Vec<i32> = standard_in_e(strings, bass)?
                .iter()
                .map(|p| p + d)
                .collect();
            *v.first_mut()? -= 2;
            v
        }
        Pattern::OpenRoot | Pattern::OpenFifth if !bass && strings == 6 => {
            // The root in the guitar's lowest octave, C2 to B2.
            let root = 36 + i32::from(key % 12);
            let shape: [i32; 6] = if pattern == Pattern::OpenRoot {
                [0, 7, 12, 16, 19, 24]
            } else {
                // The fifth below the root, then root, fifth, root, third,
                // fifth; the root a fourth above C2 at least (G2, A2).
                let root = if root < 41 { root + 12 } else { root };
                return pitches(root, [-5, 0, 7, 12, 16, 19]);
            };
            shape.iter().map(|s| root + s).collect()
        }
        _ => return None,
    };
    to_pitches(&low_first)
}

fn pitches(root: i32, shape: [i32; 6]) -> Option<Vec<Pitch>> {
    let v: Vec<i32> = shape.iter().map(|s| root + s).collect();
    to_pitches(&v)
}

/// Lowest first to highest first, each in MIDI's range.
fn to_pitches(low_first: &[i32]) -> Option<Vec<Pitch>> {
    low_first
        .iter()
        .rev()
        .map(|p| u8::try_from(*p).ok().filter(|p| *p <= 127).map(Pitch))
        .collect()
}

/// A tuning's key and pattern: the first pattern that gives exactly these
/// strings, else Other with the lowest string's pitch class as the key.
pub fn recognise(tuning: &[Pitch], bass: bool) -> (Pattern, u8) {
    let strings = tuning.len();
    for pattern in [
        Pattern::Standard,
        Pattern::Drop,
        Pattern::OpenRoot,
        Pattern::OpenFifth,
    ] {
        for key in 0..12 {
            if self::tuning(pattern, key, strings, bass).as_deref() == Some(tuning) {
                return (pattern, key);
            }
        }
    }
    (Pattern::Other, tuning.last().map_or(4, |p| p.0 % 12))
}

/// A tuning's name, compact as wherever it shows: "E Standard", "Drop D",
/// "Open G", or the notes from the lowest string ("D A D G A D").
pub fn name(tuning: &[Pitch], bass: bool) -> String {
    let (pattern, key) = recognise(tuning, bass);
    let k = key_name(key);
    match pattern {
        Pattern::Standard => format!("{k} Standard"),
        Pattern::Drop => format!("Drop {k}"),
        // The key is the open chord's root.
        Pattern::OpenRoot | Pattern::OpenFifth => format!("Open {k}"),
        Pattern::Other => tuning
            .iter()
            .rev()
            .map(|p| key_name(p.0 % 12))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn low(t: &[Pitch]) -> Vec<u8> {
        t.iter().rev().map(|p| p.0).collect()
    }

    #[test]
    fn guitars() {
        let e = tuning(Pattern::Standard, 4, 6, false).unwrap();
        assert_eq!(low(&e), [40, 45, 50, 55, 59, 64]);
        assert_eq!(e, crate::Staff::standard_guitar_tuning());
        assert_eq!(name(&e, false), "E Standard");
        let eb = tuning(Pattern::Standard, 3, 6, false).unwrap();
        assert_eq!(low(&eb), [39, 44, 49, 54, 58, 63]);
        assert_eq!(name(&eb, false), "E♭ Standard");
        let d = tuning(Pattern::Drop, 2, 6, false).unwrap();
        assert_eq!(low(&d), [38, 45, 50, 55, 59, 64]);
        assert_eq!(name(&d, false), "Drop D");
        let c = tuning(Pattern::Drop, 0, 6, false).unwrap();
        assert_eq!(low(&c), [36, 43, 48, 53, 57, 62]);
        assert_eq!(name(&c, false), "Drop C");
        let open_d = tuning(Pattern::OpenRoot, 2, 6, false).unwrap();
        assert_eq!(low(&open_d), [38, 45, 50, 54, 57, 62]);
        assert_eq!(name(&open_d, false), "Open D");
        let open_e = tuning(Pattern::OpenRoot, 4, 6, false).unwrap();
        assert_eq!(low(&open_e), [40, 47, 52, 56, 59, 64]);
        let open_g = tuning(Pattern::OpenFifth, 7, 6, false).unwrap();
        assert_eq!(low(&open_g), [38, 43, 50, 55, 59, 62]);
        assert_eq!(name(&open_g, false), "Open G");
        let open_a = tuning(Pattern::OpenFifth, 9, 6, false).unwrap();
        assert_eq!(low(&open_a), [40, 45, 52, 57, 61, 64]);
        let seven = tuning(Pattern::Standard, 4, 7, false).unwrap();
        assert_eq!(low(&seven), [35, 40, 45, 50, 55, 59, 64]);
        let eight = tuning(Pattern::Standard, 4, 8, false).unwrap();
        assert_eq!(low(&eight)[0], 30);
        let dadgad: Vec<Pitch> = [62, 57, 55, 50, 45, 38].into_iter().map(Pitch).collect();
        assert_eq!(recognise(&dadgad, false), (Pattern::Other, 2));
        assert_eq!(name(&dadgad, false), "D A D G A D");
        assert!(tuning(Pattern::OpenRoot, 2, 7, false).is_none());
        assert!(tuning(Pattern::Other, 2, 6, false).is_none());
    }

    #[test]
    fn basses() {
        let four = tuning(Pattern::Standard, 4, 4, true).unwrap();
        assert_eq!(low(&four), [28, 33, 38, 43]);
        assert_eq!(name(&four, true), "E Standard");
        let five = tuning(Pattern::Standard, 4, 5, true).unwrap();
        assert_eq!(low(&five), [23, 28, 33, 38, 43]);
        let six = tuning(Pattern::Standard, 4, 6, true).unwrap();
        assert_eq!(low(&six), [23, 28, 33, 38, 43, 48]);
        let drop_d = tuning(Pattern::Drop, 2, 4, true).unwrap();
        assert_eq!(low(&drop_d), [26, 33, 38, 43]);
        assert_eq!(name(&drop_d, true), "Drop D");
    }
}
