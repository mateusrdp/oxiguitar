// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Formats this crate recognises but doesn't read (MuseScore, MusicXML,
//! ABC, MIDI; their plugins come in M6) are refused as unsupported, never
//! misread.

use oxiguitar_formats::{ReadErrorKind, read};

#[test]
fn formats_without_a_reader_are_refused_clearly() {
    let musicxml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0"><part-list/></score-partwise>"#;
    let abc = b"X:1\nT:Test\nM:4/4\nK:C\nCDEF|GABc|\n";
    let mut midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60".to_vec();
    midi.extend_from_slice(b"MTrk\0\0\0\x04\0\xff\x2f\0");
    for (name, data) in [
        ("MusicXML", &musicxml[..]),
        ("ABC", &abc[..]),
        ("MIDI", &midi[..]),
    ] {
        let e = read(data).expect_err(name);
        assert!(
            matches!(e.kind, ReadErrorKind::Unsupported(_)),
            "{name}: {e}"
        );
        assert!(e.to_string().contains("not supported"), "{name}: {e}");
    }
}
