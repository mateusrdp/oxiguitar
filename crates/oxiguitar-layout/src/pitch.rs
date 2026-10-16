// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Where a pitch sits on a five-line staff, and which accidental it needs
//! (NOT-1).

use oxiguitar_model::{Clef, KeySignature, Pitch};

/// A written note: its diatonic position and alteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spelled {
    /// Diatonic steps from C-1 (MIDI 0): octave × 7 + letter (C = 0 … B = 6).
    pub(crate) diatonic: i32,
    /// -2 to 2: double flat to double sharp.
    pub(crate) alter: i8,
}

/// Letter (0 = C) and alteration of each pitch class, spelt with sharps.
const SHARPS: [(i32, i8); 12] = [
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
/// The same, spelt with flats.
const FLATS: [(i32, i8); 12] = [
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

/// Spells a written MIDI pitch: with flats in flat keys, sharps otherwise,
/// except that in flat keys the relative minor's leading tone stays sharp
/// (F♯ in B♭ major / G minor, C♯ in F major / D minor).
pub(crate) fn spell(written: i32, key: KeySignature) -> Spelled {
    let pc = written.rem_euclid(12);
    let octave = written.div_euclid(12);
    let major_tonic = (7 * i32::from(key.fifths)).rem_euclid(12);
    let leading_tone = (major_tonic + 9 + 11) % 12;
    let table = if key.fifths < 0 && pc != leading_tone {
        &FLATS
    } else {
        &SHARPS
    };
    let (letter, alter) = table.get(pc as usize).copied().unwrap_or((0, 0));
    Spelled {
        diatonic: octave * 7 + letter,
        alter,
    }
}

/// The written pitch of a sounding pitch on a staff: the staff's
/// transposition, plus an octave for the 8vb clefs (the guitar's written
/// part sits an octave above the sound).
pub(crate) fn written(sounding: Pitch, transposition: i8, clef: Clef) -> i32 {
    let octave = match clef {
        Clef::Treble8vb | Clef::Bass8vb => 12,
        _ => 0,
    };
    i32::from(sounding.0) + i32::from(transposition) + octave
}

/// The staff step of a diatonic position: 0 is the bottom line, 1 the first
/// space, 8 the top line.
pub(crate) fn staff_step(diatonic: i32, clef: Clef) -> i32 {
    // The diatonic position of each clef's bottom line.
    let bottom = match clef {
        // E4: octave 5 in MIDI octaves (C4 = 60 = octave 5).
        Clef::Treble | Clef::Treble8vb | Clef::Neutral => 5 * 7 + 2,
        // G2.
        Clef::Bass | Clef::Bass8vb => 3 * 7 + 4,
        // F3.
        Clef::Alto => 4 * 7 + 3,
        // D3.
        Clef::Tenor => 4 * 7 + 1,
    };
    diatonic - bottom
}

/// Staff steps of the key signature's accidentals, in order, for a clef.
pub(crate) fn key_signature_steps(key: KeySignature, clef: Clef) -> Vec<(i32, i8)> {
    // Treble clef positions: sharps F C G D A E B, flats B E A D G C F.
    const SHARP_STEPS: [i32; 7] = [8, 5, 9, 6, 3, 7, 4];
    const FLAT_STEPS: [i32; 7] = [4, 7, 3, 6, 2, 5, 1];
    let shift = match clef {
        Clef::Bass | Clef::Bass8vb => -2,
        Clef::Alto => -1,
        Clef::Tenor => 1,
        _ => 0,
    };
    let n = usize::from(key.fifths.unsigned_abs()).min(7);
    if key.fifths > 0 {
        SHARP_STEPS.iter().take(n).map(|s| (s + shift, 1)).collect()
    } else {
        FLAT_STEPS.iter().take(n).map(|s| (s + shift, -1)).collect()
    }
}

/// The alteration the key gives each letter (C = 0 … B = 6).
fn key_alters(key: KeySignature) -> [i8; 7] {
    // Sharps are added on F C G D A E B, flats on B E A D G C F.
    const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    let mut alters = [0i8; 7];
    let n = usize::from(key.fifths.unsigned_abs()).min(7);
    for i in 0..n {
        if key.fifths > 0 {
            if let Some(letter) = SHARP_ORDER.get(i) {
                alters[*letter] = 1;
            }
        } else if let Some(letter) = SHARP_ORDER.get(6 - i) {
            alters[*letter] = -1;
        }
    }
    alters
}

/// The accidentals in force within one bar: the key's, changed by each
/// accidental written so far, by staff position (as the usual rule says,
/// an accidental holds for its line or space to the end of the bar).
pub(crate) struct BarAccidentals {
    key: [i8; 7],
    written: std::collections::HashMap<i32, i8>,
}

impl BarAccidentals {
    pub(crate) fn new(key: KeySignature) -> BarAccidentals {
        BarAccidentals {
            key: key_alters(key),
            written: std::collections::HashMap::new(),
        }
    }

    /// The accidental to print before `note`, if any, and records it. A tied
    /// note never prints one.
    pub(crate) fn accidental(&mut self, note: Spelled, tied: bool) -> Option<i8> {
        let letter = note.diatonic.rem_euclid(7) as usize;
        let current = self
            .written
            .get(&note.diatonic)
            .copied()
            .unwrap_or_else(|| self.key.get(letter).copied().unwrap_or(0));
        self.written.insert(note.diatonic, note.alter);
        (!tied && current != note.alter).then_some(note.alter)
    }
}

/// Where a General MIDI drum sits on a five-line percussion staff (staff
/// step, 0 = bottom line) and whether it takes a cross notehead, following
/// the usual drum-kit layout: kick in the bottom space, snare in the third
/// space, toms between, cymbals and hi-hat above the staff with crosses.
pub(crate) fn drum(p: Pitch) -> (i32, bool) {
    match p.0 {
        35 | 36 => (1, false),     // bass drums
        37 => (5, true),           // side stick
        38 | 40 => (5, false),     // snares
        39 => (5, true),           // hand clap
        41 => (2, false),          // low floor tom
        43 => (3, false),          // high floor tom
        45 | 47 => (6, false),     // low and low-mid toms
        48 => (7, false),          // high-mid tom
        50 => (8, false),          // high tom
        42 => (9, true),           // closed hi-hat
        44 => (-1, true),          // pedal hi-hat
        46 => (9, true),           // open hi-hat
        49 | 57 => (10, true),     // crashes
        51 | 53 | 59 => (8, true), // rides and ride bell
        52 | 55 => (11, true),     // china, splash
        54 | 56 => (7, true),      // tambourine, cowbell
        _ => (4, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::KeyMode;

    fn key(fifths: i8) -> KeySignature {
        KeySignature {
            fifths,
            mode: KeyMode::Major,
        }
    }

    #[test]
    fn guitar_low_e_sits_below_the_staff() {
        // Low E (40) on a guitar staff is written E3, under the third ledger
        // line: step -7 (E4 is the bottom line, step 0).
        let w = written(Pitch(40), 0, Clef::Treble8vb);
        let s = spell(w, key(0));
        assert_eq!(staff_step(s.diatonic, Clef::Treble8vb), -7);
        // High E (64) is written E5: the top space, step 7.
        let s = spell(written(Pitch(64), 0, Clef::Treble8vb), key(0));
        assert_eq!(staff_step(s.diatonic, Clef::Treble8vb), 7);
        // Middle C on a plain treble staff: one ledger line below, step -2.
        let s = spell(60, key(0));
        assert_eq!(staff_step(s.diatonic, Clef::Treble), -2);
        // Bass clef: G2 (43) is the bottom line.
        assert_eq!(staff_step(spell(43, key(0)).diatonic, Clef::Bass), 0);
    }

    #[test]
    fn spelling_follows_the_key() {
        assert_eq!(spell(70, key(-2)).alter, -1); // B flat
        assert_eq!(spell(70, key(2)).alter, 1); // A sharp
        assert_eq!(spell(70, key(-2)).diatonic, spell(71, key(0)).diatonic);
    }

    #[test]
    fn accidentals_within_a_bar() {
        let mut bar = BarAccidentals::new(key(-2)); // B flat, E flat
        let b_flat = spell(70, key(-2));
        let b_natural = spell(71, key(-2));
        assert_eq!(bar.accidental(b_flat, false), None);
        assert_eq!(bar.accidental(b_natural, false), Some(0));
        assert_eq!(bar.accidental(b_natural, false), None);
        assert_eq!(bar.accidental(b_flat, false), Some(-1));
        let f_sharp = spell(66, key(-2));
        assert_eq!(bar.accidental(f_sharp, false), Some(1));
        assert_eq!(bar.accidental(f_sharp, true), None);
    }

    #[test]
    fn drum_kit_positions() {
        assert_eq!(drum(Pitch(36)), (1, false));
        assert_eq!(drum(Pitch(38)), (5, false));
        assert_eq!(drum(Pitch(42)), (9, true));
        assert_eq!(drum(Pitch(49)), (10, true));
    }

    #[test]
    fn key_signature_positions() {
        let flats = key_signature_steps(key(-2), Clef::Treble8vb);
        assert_eq!(flats, vec![(4, -1), (7, -1)]);
        let sharps = key_signature_steps(key(3), Clef::Bass);
        assert_eq!(sharps, vec![(6, 1), (3, 1), (7, 1)]);
    }
}
