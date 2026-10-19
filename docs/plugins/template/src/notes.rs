// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Notes: their names, and a guitar's standard tuning.

use oxiguitar_plugin::score::Pitch;

/// Standard guitar tuning, from string 1 (the highest) down, as the score
/// numbers strings: E4 B3 G3 D3 A2 E2.
pub const STANDARD: [Pitch; 6] = [
    Pitch(64),
    Pitch(59),
    Pitch(55),
    Pitch(50),
    Pitch(45),
    Pitch(40),
];

/// The name of `pitch` (a MIDI note number, 60 = middle C), with its
/// octave: "C4", "F♯2", or "G♭2" with `flats`.
pub fn name(pitch: Pitch, flats: bool) -> String {
    const SHARPS: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    const FLATS: [&str; 12] = [
        "C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B",
    ];
    let names = if flats { FLATS } else { SHARPS };
    let octave = i32::from(pitch.0) / 12 - 1;
    format!("{}{octave}", names[usize::from(pitch.0 % 12)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(name(Pitch(60), false), "C4");
        assert_eq!(name(Pitch(42), false), "F♯2");
        assert_eq!(name(Pitch(42), true), "G♭2");
        assert_eq!(name(Pitch(0), false), "C-1");
    }
}
