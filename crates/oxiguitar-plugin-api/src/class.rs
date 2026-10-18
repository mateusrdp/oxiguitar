// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Instrument classes: what an instrument plugin can play
//! (docs/design/plugins.md, "Instrument classes").
//!
//! The host derives each track's class from its General MIDI program and
//! its percussion flag, and offers a track only the instruments that play
//! its class.

use serde::{Deserialize, Serialize};

/// A class of tracks an instrument plugin plays or doesn't.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentClass {
    /// Guitars and basses with frets, banjo, sitar (GM 24–34, 36, 37, 104,
    /// 105).
    Fretted,
    /// Fretless bass and stringed instruments without frets (GM 35, 106,
    /// 107).
    Fretless,
    /// Violin, viola, cello, contrabass, string ensembles, fiddle (GM 40–45,
    /// 48–51, 110).
    Bowed,
    /// Pianos, chromatic percussion, organs, accordions (GM 0–23).
    Keyboard,
    /// Choirs and voices (GM 52–54).
    Voice,
    /// Brass, reeds and pipes, bagpipe, shanai (GM 56–79, 109, 111).
    Winds,
    /// Synth basses, leads, pads and effects (GM 38, 39, 80–103).
    Synth,
    /// Percussion tracks (the drum-kit flag).
    Drums,
    /// Anything else: harp, timpani, orchestra hit, kalimba, percussive
    /// and sound effects (GM 46, 47, 55, 108, 112–127).
    Other,
}

impl InstrumentClass {
    /// Every class, in a fixed order.
    pub const ALL: [InstrumentClass; 9] = [
        InstrumentClass::Fretted,
        InstrumentClass::Fretless,
        InstrumentClass::Bowed,
        InstrumentClass::Keyboard,
        InstrumentClass::Voice,
        InstrumentClass::Winds,
        InstrumentClass::Synth,
        InstrumentClass::Drums,
        InstrumentClass::Other,
    ];

    /// The class of a track with General MIDI program `program` (0–127)
    /// and the drum-kit flag `percussion`.
    pub fn of(program: u8, percussion: bool) -> InstrumentClass {
        if percussion {
            return InstrumentClass::Drums;
        }
        match program {
            0..=23 => InstrumentClass::Keyboard,
            24..=34 | 36 | 37 | 104 | 105 => InstrumentClass::Fretted,
            35 | 106 | 107 => InstrumentClass::Fretless,
            38 | 39 | 80..=103 => InstrumentClass::Synth,
            40..=45 | 48..=51 | 110 => InstrumentClass::Bowed,
            52..=54 => InstrumentClass::Voice,
            56..=79 | 109 | 111 => InstrumentClass::Winds,
            _ => InstrumentClass::Other,
        }
    }

    /// The name used in manifests, such as `fretted`.
    pub fn name(self) -> &'static str {
        match self {
            InstrumentClass::Fretted => "fretted",
            InstrumentClass::Fretless => "fretless",
            InstrumentClass::Bowed => "bowed",
            InstrumentClass::Keyboard => "keyboard",
            InstrumentClass::Voice => "voice",
            InstrumentClass::Winds => "winds",
            InstrumentClass::Synth => "synth",
            InstrumentClass::Drums => "drums",
            InstrumentClass::Other => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_program_has_one_class_and_the_table_matches_the_docs() {
        use InstrumentClass as C;
        let count = |c: C| (0..=127u8).filter(|&p| C::of(p, false) == c).count();
        assert_eq!(count(C::Keyboard), 24);
        assert_eq!(count(C::Fretted), 15);
        assert_eq!(count(C::Fretless), 3);
        assert_eq!(count(C::Synth), 26);
        assert_eq!(count(C::Bowed), 11);
        assert_eq!(count(C::Voice), 3);
        assert_eq!(count(C::Winds), 26);
        assert_eq!(count(C::Other), 20);
        assert_eq!(count(C::Drums), 0);
        assert_eq!(C::of(25, true), C::Drums);
        assert_eq!(C::of(25, false), C::Fretted, "a new track is a guitar");
    }

    #[test]
    fn classes_are_named_in_snake_case() {
        for c in InstrumentClass::ALL {
            assert_eq!(
                serde_json::to_string(&c).unwrap(),
                format!("\"{}\"", c.name())
            );
        }
    }
}
