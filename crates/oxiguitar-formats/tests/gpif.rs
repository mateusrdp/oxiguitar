// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Guitar Pro 6 (.gpx) and 7/8 (.gp) reader tests on the test song written
//! by `support/gpif_writer.rs`, plus the public fixtures in `fixtures/gp/`.
//!
//! To regenerate the fixtures after changing the writer:
//! `OXIGUITAR_BLESS=1 cargo test -p oxiguitar-formats --test gpif`

mod support {
    pub(crate) mod compare;
    pub(crate) mod gp_writer;
    pub(crate) mod gpif_writer;
    pub(crate) mod song_check;
}

use oxiguitar_formats::{Format, ReadErrorKind, read};
use oxiguitar_model::{Clef, Ratio, Score, StrokeDirection};
use support::gpif_writer::{self, Dialect};
use support::song_check::check_song;

fn gp7() -> Vec<u8> {
    gpif_writer::test_song_gp()
}

fn gpx() -> Vec<u8> {
    gpif_writer::test_song_gpx()
}

/// What the GPIF song has beyond the shared test song.
fn check_extras(score: &Score) {
    let mb = &score.master_bars;
    let marker = mb[0].marker.as_ref().unwrap();
    assert_eq!(
        (marker.letter.as_str(), marker.text.as_str()),
        ("A", "Intro")
    );
    assert_eq!(score.initial_tempo, 96.0);
    assert_eq!(score.tempo_label, "Moderato");
    // 160 eighths a minute, half-way through a 3/4 bar.
    assert_eq!(mb[2].tempo_changes.len(), 1);
    assert_eq!(mb[2].tempo_changes[0].bpm, 80.0);
    assert_eq!(mb[2].tempo_changes[0].position, Ratio::new(3, 8).unwrap());
    // "Words and music" fills the empty composer.
    assert_eq!(score.info.music, "OxiGuitar");
    assert_eq!(
        score.info.notices,
        vec!["Made for OxiGuitar's tests".to_owned()]
    );

    let track = &score.tracks[0];
    assert_eq!(track.short_name, "gtr.");
    assert!((track.mix.volume - 0.8).abs() < 1e-6);
    let staff = &track.staves[0];
    assert!(staff.show_standard && staff.show_tablature);
    assert_eq!(staff.transposition, 0);
    assert_eq!(staff.bars[0].clef, Clef::Treble8vb);
    assert_eq!(track.chords.len(), 1);
    assert_eq!(track.chords[0].name, "E5");
    assert_eq!(
        track.chords[0].frets,
        vec![None, None, None, Some(2), Some(2), Some(0)]
    );
    let chord_beat = &staff.bars[2].voices[0].beats[0];
    assert_eq!(chord_beat.chord, Some(0));
    assert_eq!(
        chord_beat.effects.stroke.map(|s| s.direction),
        Some(StrokeDirection::Down)
    );
    assert_eq!(staff.bars[0].voices[0].beats[4].text, "let it ring");
}

#[test]
fn reads_gp7_and_gpx() {
    for (name, data, format) in [
        ("gp", gp7(), Format::GuitarPro7Plus),
        ("gpx", gpx(), Format::GuitarPro6),
    ] {
        let imported = read(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(imported.format, format);
        check_song(&imported.score, 7);
        check_extras(&imported.score);
        assert!(
            imported.log.is_empty(),
            "{name}: unexpected log {:?}",
            imported.log.entries()
        );
    }
}

#[test]
fn uncompressed_gpx_reads_too() {
    let fs = gpif_writer::bcfs(&[(
        "score.gpif",
        gpif_writer::score_gpif(Dialect::Gp6).into_bytes(),
    )]);
    let imported = read(&fs).unwrap();
    check_song(&imported.score, 7);
}

/// The same song read from GP5, GP6 and GP7 gives the same music.
#[test]
fn agrees_with_the_gp5_reader() {
    let gp5 = read(&support::gp_writer::test_song(5)).unwrap().score;
    for data in [gp7(), gpx()] {
        let other = read(&data).unwrap().score;
        let report = support::compare::compare(&gp5, &other);
        for (category, (matched, total)) in &report.counts {
            // The GPIF song adds one brush stroke that GP5 does not have.
            let expected = if *category == "beat effects" {
                total - 1
            } else {
                *total
            };
            assert_eq!(*matched, expected, "{category}\n{}", report.summary());
        }
        assert_eq!(gp5.tracks[0].lyrics, other.tracks[0].lyrics);
    }
}

#[test]
fn truncated_files_give_errors_not_panics() {
    // FMT-1: cut each container at many lengths; each must fail cleanly or read.
    for data in [gp7(), gpx()] {
        for len in (0..data.len()).step_by(7) {
            if let Err(e) = read(&data[..len]) {
                assert!(
                    matches!(
                        e.kind,
                        ReadErrorKind::UnexpectedEnd
                            | ReadErrorKind::Invalid(_)
                            | ReadErrorKind::UnknownFormat
                    ),
                    "cut at {len}: {e}"
                );
            }
        }
    }
}

#[test]
fn damaged_bytes_never_panic() {
    for data in [gp7(), gpx()] {
        for i in (8..data.len()).step_by(3) {
            for v in [0x00, 0x7f, 0xff] {
                let mut d = data.clone();
                d[i] = v;
                let _ = read(&d);
            }
        }
    }
}

/// Damage inside the XML: every element cut short, numbers made huge or
/// negative, ids pointing nowhere. The reader may log or fail, never panic.
#[test]
fn damaged_gpif_never_panics() {
    let xml = gpif_writer::score_gpif(Dialect::Gp7);
    let read_xml = |s: &str| {
        let fs = gpif_writer::bcfs(&[("score.gpif", s.as_bytes().to_vec())]);
        let _ = read(&fs);
    };
    // Remove each element's text in turn (any text between '>' and '<').
    let mut starts: Vec<usize> = xml.match_indices('>').map(|(i, _)| i + 1).collect();
    starts.retain(|&i| xml[i..].starts_with(|c: char| c != '<' && c != '\n'));
    for &i in &starts {
        let end = i + xml[i..].find('<').unwrap();
        for replacement in [
            "",
            "-1",
            "99999999999",
            "4294967295",
            "0",
            "nan",
            "1/0",
            "x y z",
        ] {
            let mut s = xml.clone();
            s.replace_range(i..end, replacement);
            read_xml(&s);
        }
    }
    // Cut the XML at many places.
    for len in (0..xml.len()).step_by(11) {
        read_xml(&xml[..len]);
    }
    // Every attribute value made extreme.
    for (i, _) in xml.match_indices("=\"") {
        let end = i + 2 + xml[i + 2..].find('"').unwrap();
        for replacement in ["-5", "1000", "", "true"] {
            let mut s = xml.clone();
            s.replace_range(i + 2..end, replacement);
            read_xml(&s);
        }
    }
}

/// Guitar Pro 6 sometimes stores a stale fret on the second note of a tie;
/// a tie cannot change pitch, so the note continues the first one's fret.
#[test]
fn tied_notes_keep_their_fret() {
    let xml = gpif_writer::score_gpif(Dialect::Gp6);
    let at = xml.find("destination=\"true\"").unwrap();
    let fret = at + xml[at..].find("<Fret>2</Fret>").unwrap();
    let mut damaged = xml.clone();
    damaged.replace_range(fret..fret + "<Fret>2</Fret>".len(), "<Fret>7</Fret>");
    let imported = read(&gpif_writer::bcfs(&[("score.gpif", damaged.into_bytes())])).unwrap();
    check_song(&imported.score, 7);
    assert!(
        imported
            .log
            .entries()
            .iter()
            .any(|e| e.message.contains("tied note")),
        "{:?}",
        imported.log.entries()
    );
}

#[test]
fn not_a_score() {
    let fs = gpif_writer::bcfs(&[("misc.xml", b"<Misc/>".to_vec())]);
    assert!(read(&fs).is_err());
    let fs = gpif_writer::bcfs(&[("score.gpif", b"<NotGpif/>".to_vec())]);
    let e = read(&fs).unwrap_err();
    assert!(e.to_string().contains("GPIF"), "{e}");
    let zip = gpif_writer::pack_zip("<GPIF><Tracks/></GPIF>");
    assert!(read(&zip).is_err());
}

#[test]
fn public_fixtures_match_the_writer() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp");
    let bless = std::env::var_os("OXIGUITAR_BLESS").is_some();
    for (ext, expected) in [("gp", gp7()), ("gpx", gpx())] {
        let path = dir.join(format!("oxiguitar-test-song.{ext}"));
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &expected).unwrap();
        }
        let on_disk = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with OXIGUITAR_BLESS=1 to create it",
                path.display()
            )
        });
        assert_eq!(on_disk, expected, "{} is out of date", path.display());
        check_song(&read(&on_disk).unwrap().score, 7);
    }
}
