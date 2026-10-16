// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The core reads Guitar Pro only. MuseScore, MusicXML, ABC and MIDI files
//! are recognised and refused with a message that a format plugin will open
//! them (decided 16 Oct 2026).

use oxiguitar_formats::{ReadErrorKind, read};

#[test]
fn plugin_formats_are_recognised_and_refused_clearly() {
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
            matches!(e.kind, ReadErrorKind::NeedsPlugin(_)),
            "{name}: {e}"
        );
        let text = e.to_string();
        assert!(text.contains("format plugin"), "{name}: {text}");
    }
}
