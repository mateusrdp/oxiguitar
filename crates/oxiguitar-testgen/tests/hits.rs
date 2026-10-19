// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engraver's map of where things are (M3, slice 1), checked against
//! what it draws: in every one of the 50 reference scores, every note of
//! every shown staff is found, its fret number on the tablature lies in the
//! box the map gives for its string, and a click there names that note's
//! track, bar, beat and string.

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{FontSet, Item, Rect};
use oxiguitar_layout::engrave;
use oxiguitar_model::{BeatKind, NotePlace};
use oxiguitar_testgen::{references, to_gp};

#[test]
fn every_note_is_where_the_map_says() {
    let fonts = FontSet::bundled();
    let meta = Metadata::bravura().expect("Bravura metadata");
    let (mut checked, mut on_tab) = (0usize, 0usize);
    for r in references() {
        let score = oxiguitar_formats::read(&to_gp(&r.song))
            .expect("the reader opens it")
            .score;
        let engraved = engrave(&score, &fonts, &meta, &r.options);
        let hits = &engraved.hits;
        let shown = hits.tracks();
        assert!(!shown.is_empty(), "{}", r.name);
        for &ti in &shown {
            let track = &score.tracks[ti];
            for (si, staff) in track.staves.iter().enumerate() {
                for (bi, bar) in staff.bars.iter().enumerate() {
                    for (vi, voice) in bar.voices.iter().enumerate() {
                        for (beat, b) in voice.beats.iter().enumerate() {
                            if b.kind != BeatKind::Notes || b.grace.is_some() {
                                continue;
                            }
                            for n in &b.notes {
                                let string = n.string().unwrap_or(1);
                                let Some(found) = hits.find(ti, si, bi, vi, beat, string) else {
                                    panic!("{}: track {ti} bar {bi} voice {vi} beat {beat}", r.name)
                                };
                                checked += 1;
                                let (NotePlace::Fretted { fret, .. }, Some(rect)) =
                                    (n.place, found.string)
                                else {
                                    continue;
                                };
                                if n.tie || n.effects.dead {
                                    continue;
                                }
                                let page = &engraved.pages[found.sheet];
                                let label = fret.to_string();
                                let drawn = page.items.iter().any(|i| match i {
                                    Item::Glyphs(run) => {
                                        run.text.contains(&label) && overlaps(&i.bounds(), &rect)
                                    }
                                    _ => false,
                                });
                                assert!(
                                    drawn,
                                    "{}: fret {fret} of track {ti} bar {} beat {beat} string \
                                     {string} isn't drawn in {rect:?}",
                                    r.name,
                                    bi + 1
                                );
                                on_tab += 1;
                                // A click in the middle of that box names
                                // the same note.
                                let spot = hits
                                    .hit(
                                        found.sheet,
                                        rect.x + rect.width / 2.0,
                                        rect.y + rect.height / 2.0,
                                        vi,
                                    )
                                    .expect("a click on a fret number");
                                assert_eq!(
                                    (spot.track, spot.staff, spot.bar, spot.string),
                                    (ti, si, bi, Some(string)),
                                    "{}",
                                    r.name
                                );
                                assert!(spot.beat.is_some(), "{}", r.name);
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{checked} notes found, {on_tab} fret numbers where the map says");
    assert!(checked > 2000 && on_tab > 1000, "{checked} {on_tab}");
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}
