// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Two files Mateus made in Guitar Pro 7.6 (20 Oct 2026,
//! `fixtures/guitar-pro-7.6/`): a nylon guitar with its two E strings
//! open, and pianos (named piano, violin and voice) with notes at MIDI 0
//! and 127. They show how Guitar Pro itself writes pitches and the strings
//! of an instrument without them, which our first files got wrong: the
//! staff an octave low (Guitar Pro's octave is MIDI ÷ 12) and frets of
//! -2³¹ on pianos (a tuning of zeros, notes without a string or fret).

use std::collections::BTreeSet;
use std::io::Read as _;

use oxiguitar_model::{NotePlace, Pitch, Score};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../fixtures/guitar-pro-7.6/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn gpif(gp: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(gp)).unwrap();
    let mut s = String::new();
    zip.by_name("Content/score.gpif")
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    s
}

/// The text of each `<Property name="NAME">` of every note, in order.
fn note_property(gpif: &str, name: &str) -> Vec<String> {
    let open = format!("<Property name=\"{name}\">");
    gpif.split("<Note id=")
        .skip(1)
        .filter_map(|n| {
            let n = &n[..n.find("</Note>")?];
            let start = n.find(&open)? + open.len();
            let end = start + n[start..].find("</Property>")?;
            Some(n[start..end].split_whitespace().collect::<String>())
        })
        .collect()
}

fn notes(score: &Score) -> Vec<NotePlace> {
    score
        .tracks
        .iter()
        .flat_map(|t| &t.staves)
        .flat_map(|s| &s.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .flat_map(|b| &b.notes)
        .map(|n| n.place)
        .collect()
}

#[test]
fn guitar_pitches_are_written_as_guitar_pro_writes_them() {
    let theirs = gpif(&fixture("nylon-guitar-default.gp"));
    let score = oxiguitar_formats::read(&fixture("nylon-guitar-default.gp"))
        .unwrap()
        .score;
    assert_eq!(score.tracks[0].staves[0].string_count(), 6);
    let ours = gpif(&oxiguitar_formats::write_gp(&score).unwrap().data);
    for name in ["ConcertPitch", "TransposedPitch", "Midi", "String", "Fret"] {
        assert_eq!(
            note_property(&ours, name),
            note_property(&theirs, name),
            "{name}"
        );
    }
}

#[test]
fn instruments_without_strings_read_and_write_as_guitar_pro_does() {
    let file = fixture("pianos-high-low.gp");
    let theirs = gpif(&file);
    let score = oxiguitar_formats::read(&file).unwrap().score;
    assert_eq!(score.tracks.len(), 6);
    assert!(
        score
            .tracks
            .iter()
            .flat_map(|t| &t.staves)
            .all(|s| s.string_count() == 0),
        "Guitar Pro's default tunings are no strings"
    );
    let read = notes(&score);
    assert!(read.iter().all(|n| matches!(n, NotePlace::Pitched(_))));
    assert!(read.contains(&NotePlace::Pitched(Pitch(0))));
    assert!(read.contains(&NotePlace::Pitched(Pitch(127))));

    let written = oxiguitar_formats::write_gp(&score).unwrap();
    assert_eq!(
        written.losses.warning_count(),
        0,
        "{:?}",
        written.losses.entries()
    );
    let ours = gpif(&written.data);
    // The same tunings, staff by staff, and the same pitches, note by note;
    // a string and fret on every note (Guitar Pro picks some strings
    // otherwise, which plays the same).
    let tunings = |x: &str| -> Vec<String> {
        x.split("<Property name=\"Tuning\">")
            .skip(1)
            .filter_map(|t| {
                let t = &t[t.find("<Pitches>")? + 9..];
                Some(t[..t.find("</Pitches>")?].to_owned())
            })
            .collect()
    };
    assert_eq!(tunings(&ours), tunings(&theirs));
    // Guitar Pro shares a note between tracks where it is the same, so
    // the values are compared, not the lists.
    let set =
        |x: &str, name: &str| -> BTreeSet<String> { note_property(x, name).into_iter().collect() };
    for name in ["ConcertPitch", "TransposedPitch", "Midi"] {
        assert_eq!(set(&ours, name), set(&theirs, name), "{name}");
    }
    assert_eq!(
        note_property(&ours, "Fret").len(),
        note_property(&ours, "Midi").len(),
        "every note has a fret"
    );
    assert!(set(&ours, "Fret").contains("<Fret>-40</Fret>"));
    assert!(set(&ours, "Fret").contains("<Fret>63</Fret>"));
    // And it reads back the same.
    let back = oxiguitar_formats::read(&written.data).unwrap().score;
    assert_eq!(notes(&back), notes(&score));
}
