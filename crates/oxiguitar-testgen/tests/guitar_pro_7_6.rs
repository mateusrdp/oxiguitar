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
//!
//! The .gp writer is held to these files too: what it writes for each song
//! reads as Guitar Pro's file does (apart from the three changes above),
//! and holds every element Guitar Pro wrote for the song, but for a few that
//! only describe Guitar Pro's own display and sound settings.

use std::collections::BTreeSet;
use std::io::Read as _;
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

/// Every element path in a GPIF document, a property named by its name:
/// `/GPIF/Notes/Note/Properties/Property[Fret]/Fret`.
fn element_paths(xml: &str) -> BTreeSet<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut out = BTreeSet::new();
    let name = |e: &quick_xml::events::BytesStart<'_>| {
        let mut n = e.name().as_ref().to_owned();
        if n == "Property"
            && let Some(Ok(a)) = e
                .attributes()
                .find(|a| a.as_ref().is_ok_and(|a| a.key.as_ref() == "name"))
        {
            n = format!("Property[{}]", a.value);
        }
        n
    };
    loop {
        match reader.read_event().expect("well-formed XML") {
            Event::Start(e) => {
                stack.push(name(&e));
                out.insert(format!("/{}", stack.join("/")));
            }
            Event::Empty(e) => {
                out.insert(format!("/{}/{}", stack.join("/"), name(&e)));
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

#[test]
fn our_writer_writes_what_guitar_pro_7_6_does() {
    // Guitar Pro's display and sound settings, not the song.
    let theirs_only = |p: &str| {
        p.starts_with("/GPIF/Score/")
            || p.starts_with("/GPIF/MasterTrack/RSE")
            || p.ends_with("/IconId")
            || p.ends_with("/SystemsLayout")
            || p.contains("/EffectChain/Effect/")
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/guitar-pro-7.6");
    let mut checked = 0;
    for r in references() {
        let Ok(gpif) = std::fs::read_to_string(dir.join(format!("{}.gpif", r.name))) else {
            continue;
        };
        let song = oxiguitar_formats::read(&oxiguitar_testgen::to_gp(&r.song))
            .unwrap()
            .score;
        let written = oxiguitar_formats::write_gp(&song).unwrap();
        assert_eq!(written.losses.warning_count(), 0, "{}", r.name);
        let mut ours_xml = String::new();
        zip::ZipArchive::new(std::io::Cursor::new(&written.data))
            .unwrap()
            .by_name("Content/score.gpif")
            .unwrap()
            .read_to_string(&mut ours_xml)
            .unwrap();

        let ours = element_paths(&ours_xml);
        let theirs_paths = element_paths(&gpif);
        let missing: Vec<&String> = theirs_paths
            .iter()
            .filter(|p| !theirs_only(p) && !ours.contains(*p))
            .collect();
        assert!(
            missing.is_empty(),
            "{}: we don't write {missing:#?}",
            r.name
        );

        let mut ours = oxiguitar_formats::read(&written.data).unwrap().score;
        let theirs = oxiguitar_formats::read(&pack(&gpif, &part_configuration(&r.song)))
            .unwrap()
            .score;
        ours.tempo_label.clear();
        for (o, t) in ours.tracks.iter_mut().zip(&theirs.tracks) {
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
            format!("{theirs:#?}"),
            format!("{ours:#?}"),
            "{}: our file reads differently from Guitar Pro 7.6's",
            r.name
        );
        checked += 1;
    }
    assert_eq!(checked, 10, "the ten files re-saved");
}
