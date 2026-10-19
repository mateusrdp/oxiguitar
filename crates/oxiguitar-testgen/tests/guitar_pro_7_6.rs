// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reference songs re-saved by Guitar Pro 7.6 (Mateus, 19 Oct 2026;
//! `fixtures/guitar-pro-7.6/`, the score XML of each): our reader gets the
//! same song from Guitar Pro's own file as from ours, so Guitar Pro kept
//! everything we wrote, and we read what Guitar Pro writes. These files are
//! also what the .gp writer (M3 slice 2) is compared against.
//!
//! Guitar Pro 7.6 changes three things, all lessons for that writer:
//!
//! - It drops a chord diagram that has no chord spelled out (key note,
//!   bass, intervals): ours have only a name and frets, so the chords go.
//! - It keeps no text with a tempo ("Moderato").
//! - It gives each track its own mixer settings where we wrote none.
//!
//! Everything else (every note, beat, effect, bar and header text) reads
//! back the same.

use std::path::Path;

use oxiguitar_testgen::gpif::{pack, part_configuration};
use oxiguitar_testgen::references;

#[test]
fn guitar_pro_7_6_keeps_our_songs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/guitar-pro-7.6");
    let mut checked = 0;
    for r in references() {
        let Ok(gpif) = std::fs::read_to_string(dir.join(format!("{}.gpif", r.name))) else {
            continue;
        };
        let mut ours = oxiguitar_formats::read(&oxiguitar_testgen::to_gp(&r.song))
            .expect("our file reads")
            .score;
        let theirs = oxiguitar_formats::read(&pack(&gpif, &part_configuration(&r.song)))
            .unwrap_or_else(|e| panic!("{}: {e}", r.name));
        assert!(
            theirs.log.is_empty(),
            "{}: {:?}",
            r.name,
            theirs.log.entries()
        );
        // What Guitar Pro 7.6 changes (above).
        ours.tempo_label.clear();
        for (o, t) in ours.tracks.iter_mut().zip(&theirs.score.tracks) {
            o.mix = t.mix;
            if t.chords.is_empty() {
                o.chords.clear();
                for b in o.staves.iter_mut().flat_map(|s| &mut s.bars) {
                    for beat in b.voices.iter_mut().flat_map(|v| &mut v.beats) {
                        beat.chord = None;
                    }
                }
            }
        }
        assert_eq!(
            format!("{:#?}", theirs.score),
            format!("{ours:#?}"),
            "{}: Guitar Pro 7.6's file reads differently",
            r.name
        );
        checked += 1;
    }
    assert_eq!(checked, 10, "the ten files re-saved");
}
