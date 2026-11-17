//! The drum kit map: where each General MIDI percussion key sits on a
//! five-line drum staff and on drum tablature, and what it is called.
//!
//! One map for every drum track (Mateus, 17 Nov 2026; a map the user edits
//! per track may come after v1.0). The engraver draws a drum note where the
//! map puts it, and the editor enters on a line or space what the map puts
//! there, so the two always agree. The layout is the usual drum-kit one:
//! kick in the bottom space, snare in the third space, toms between,
//! cymbals and hi-hats above the staff with cross noteheads. Names are
//! General MIDI's (level 1 for 35 to 81, level 2 for the rest).

use crate::Pitch;

/// The lowest and highest key of the map (General MIDI level 2's
/// percussion range).
pub const KEYS: std::ops::RangeInclusive<u8> = 27..=87;

/// Lines of drum tablature, top to bottom. Each line holds a group of the
/// kit, and a note on it shows its key's number.
pub const TAB_LINES: u8 = 6;

/// One element of the kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element {
    /// Its General MIDI percussion key.
    pub key: u8,
    /// Its General MIDI name.
    pub name: &'static str,
    /// Its place on the staff: 0 the bottom line, 1 the bottom space, 8
    /// the top line, -1 the space below the staff.
    pub step: i32,
    /// Drawn with a cross notehead (cymbals, hi-hats, side stick, clap).
    pub cross: bool,
    /// Its line of drum tablature, 1 (top) to [`TAB_LINES`].
    pub line: u8,
}

const fn e(key: u8, name: &'static str, step: i32, cross: bool, line: u8) -> Element {
    Element {
        key,
        name,
        step,
        cross,
        line,
    }
}

/// The step of every key the map has no place for: the middle line.
pub const OTHER_STEP: i32 = 4;

/// The kit, in the order entry offers the elements of a line or space: on
/// each step, the first listed is the one Enter puts there and the n-th
/// the one digit n does. Keys not listed sit on the middle line, after
/// these, in key order.
pub const KIT: [Element; 24] = [
    // Above the staff: splash and china, crashes, hi-hats.
    e(55, "Splash Cymbal", 11, true, 1),
    e(52, "Chinese Cymbal", 11, true, 1),
    e(49, "Crash Cymbal 1", 10, true, 1),
    e(57, "Crash Cymbal 2", 10, true, 1),
    e(42, "Closed Hi-Hat", 9, true, 2),
    e(46, "Open Hi-Hat", 9, true, 2),
    // The staff, top to bottom.
    e(51, "Ride Cymbal 1", 8, true, 1),
    e(53, "Ride Bell", 8, true, 1),
    e(59, "Ride Cymbal 2", 8, true, 1),
    e(50, "High Tom", 8, false, 3),
    e(48, "Hi-Mid Tom", 7, false, 3),
    e(54, "Tambourine", 7, true, 4),
    e(56, "Cowbell", 7, true, 4),
    e(47, "Low-Mid Tom", 6, false, 3),
    e(45, "Low Tom", 6, false, 3),
    e(38, "Acoustic Snare", 5, false, 4),
    e(37, "Side Stick", 5, true, 4),
    e(40, "Electric Snare", 5, false, 4),
    e(39, "Hand Clap", 5, true, 4),
    e(43, "High Floor Tom", 3, false, 5),
    e(41, "Low Floor Tom", 2, false, 5),
    e(36, "Bass Drum 1", 1, false, 6),
    e(35, "Acoustic Bass Drum", 1, false, 6),
    // Below the staff.
    e(44, "Pedal Hi-Hat", -1, true, 2),
];

/// Names of the keys [`KIT`] doesn't list.
const OTHER_NAMES: [(u8, &str); 37] = [
    (27, "High Q"),
    (28, "Slap"),
    (29, "Scratch Push"),
    (30, "Scratch Pull"),
    (31, "Sticks"),
    (32, "Square Click"),
    (33, "Metronome Click"),
    (34, "Metronome Bell"),
    (58, "Vibraslap"),
    (60, "Hi Bongo"),
    (61, "Low Bongo"),
    (62, "Mute Hi Conga"),
    (63, "Open Hi Conga"),
    (64, "Low Conga"),
    (65, "High Timbale"),
    (66, "Low Timbale"),
    (67, "High Agogo"),
    (68, "Low Agogo"),
    (69, "Cabasa"),
    (70, "Maracas"),
    (71, "Short Whistle"),
    (72, "Long Whistle"),
    (73, "Short Guiro"),
    (74, "Long Guiro"),
    (75, "Claves"),
    (76, "Hi Wood Block"),
    (77, "Low Wood Block"),
    (78, "Mute Cuica"),
    (79, "Open Cuica"),
    (80, "Mute Triangle"),
    (81, "Open Triangle"),
    (82, "Shaker"),
    (83, "Jingle Bell"),
    (84, "Belltree"),
    (85, "Castanets"),
    (86, "Mute Surdo"),
    (87, "Open Surdo"),
];

/// The element of `key`. A key the map doesn't place sits on the middle
/// line with a plain notehead, on the tablature's fourth line; one outside
/// [`KEYS`] is named by its number.
pub fn element(key: Pitch) -> Element {
    let k = key.0;
    if let Some(e) = KIT.iter().find(|e| e.key == k) {
        return *e;
    }
    let name = OTHER_NAMES
        .iter()
        .find(|(n, _)| *n == k)
        .map_or("Percussion", |(_, name)| name);
    e(k, name, OTHER_STEP, false, 4)
}

/// The elements entry offers on staff step `step`, in order: [`KIT`]'s for
/// that step, then on the middle line every other key in [`KEYS`].
pub fn at_step(step: i32) -> Vec<Element> {
    let mut out: Vec<Element> = KIT.iter().filter(|e| e.step == step).copied().collect();
    if step == OTHER_STEP {
        out.extend(
            KEYS.filter(|k| KIT.iter().all(|e| e.key != *k))
                .map(|k| element(Pitch(k))),
        );
    }
    out
}

/// Every key of [`KEYS`] with its element, in key order (for a list to
/// pick from).
pub fn all() -> Vec<Element> {
    KEYS.map(|k| element(Pitch(k))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_one_place_and_a_name() {
        for k in KEYS {
            let e = element(Pitch(k));
            assert_eq!(e.key, k);
            assert_ne!(e.name, "Percussion", "key {k}");
            assert!((1..=TAB_LINES).contains(&e.line));
            assert_eq!(
                at_step(e.step).iter().filter(|x| x.key == k).count(),
                1,
                "key {k} is offered once, on its own step"
            );
        }
        let mut keys: Vec<u8> = KIT.iter().map(|e| e.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), KIT.len(), "no key twice");
        assert_eq!(all().len(), KEYS.count());
    }

    #[test]
    fn the_usual_kit_layout() {
        let first = |step| at_step(step).first().map(|e| e.key);
        assert_eq!(first(1), Some(36), "kick in the bottom space");
        assert_eq!(first(5), Some(38), "snare in the third space");
        assert_eq!(first(9), Some(42), "closed hi-hat above the staff");
        assert_eq!(first(10), Some(49), "crash on the ledger line");
        assert_eq!(first(8), Some(51), "ride on the top line");
        assert_eq!(first(-1), Some(44), "pedal hi-hat below");
        assert_eq!(first(0), None, "nothing on the bottom line");
        assert!(element(Pitch(42)).cross && !element(Pitch(38)).cross);
        assert_eq!(element(Pitch(99)).step, OTHER_STEP);
    }
}
