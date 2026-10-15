// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Guitar Pro 3–5 reader tests on a test song written byte by byte, plus the
//! public fixtures in `fixtures/gp/` (the same song, checked in so other
//! tools and the no-network test can use it).
//!
//! To regenerate the fixtures after changing the writer:
//! `OXIGUITAR_BLESS=1 cargo test -p oxiguitar-formats --test guitar_pro`

mod support {
    pub(crate) mod gp_writer;
}

use oxiguitar_formats::{Format, ReadErrorKind, read};
use oxiguitar_model::{BeatKind, NotePlace, NoteValue, Ratio, Score, Tuplet};
use support::gp_writer::{self, test_song};

fn r(n: i64, d: i64) -> Ratio {
    Ratio::new(n, d).unwrap()
}

fn check_song(score: &Score, major: u8) {
    assert_eq!(score.info.title, gp_writer::TITLE);
    assert_eq!(score.info.artist, gp_writer::ARTIST);
    assert_eq!(score.initial_tempo, f64::from(gp_writer::TEMPO));
    assert_eq!(score.bar_count(), gp_writer::BARS);
    score.check_structure().unwrap();

    let mb = &score.master_bars;
    assert!(mb[0].repeat_start);
    assert_eq!(mb[0].marker.as_ref().unwrap().text, "Intro");
    assert_eq!(mb[1].repeat_end, Some(2));
    assert_eq!(
        (
            mb[2].time_signature.numerator,
            mb[2].time_signature.denominator
        ),
        (3, 4)
    );
    assert_eq!(
        mb[1].time_signature.numerator, 4,
        "time signature carries over"
    );

    assert_eq!(score.tracks.len(), 1);
    let track = &score.tracks[0];
    assert_eq!(track.name, "Guitar");
    assert_eq!(track.instrument.midi_program, 27);
    let staff = &track.staves[0];
    assert_eq!(staff.tuning.len(), 6);
    assert_eq!(staff.tuning[0].0, 64);

    // Every bar is full.
    for (bar, m) in staff.bars.iter().zip(mb) {
        for v in bar.voices.iter().filter(|v| !v.is_empty()) {
            assert_eq!(v.length(), m.expected_length());
        }
    }

    let bar1 = &staff.bars[0].voices[0].beats;
    assert_eq!(bar1.len(), 6);
    assert!(bar1[0].notes[0].effects.hammer_pull);
    assert_eq!(bar1[1].duration.tuplets, vec![Tuplet::TRIPLET]);
    assert_eq!(bar1[1].duration.length(), Some(r(1, 12)));
    let bend = bar1[3].notes[0].effects.bend.as_ref().unwrap();
    assert_eq!(bend.points.len(), 3);
    assert_eq!(bend.points[1].offset, r(1, 2));
    assert_eq!(bend.points[1].cents, 200);
    let tied = &bar1[5].notes[0];
    assert!(tied.tie);
    assert_eq!(
        tied.place,
        NotePlace::Fretted { string: 3, fret: 2 },
        "a tied note takes the fret of the note it continues"
    );

    let bar2 = &staff.bars[1].voices[0].beats;
    assert!(bar2[0].notes[0].effects.dead);
    assert_eq!(bar2[1].kind, BeatKind::Rest);
    assert_eq!(bar2[1].duration.value, NoteValue::Half);

    let bar3 = &staff.bars[2];
    let chord = &bar3.voices[0].beats[0];
    assert_eq!(chord.notes.len(), 3);
    assert_eq!(chord.duration.dots, 1);
    assert_eq!(chord.notes[0].effects.palm_mute, major >= 4);
    if major == 5 {
        assert_eq!(bar3.voices.len(), 2);
        assert_eq!(bar3.voices[1].beats.len(), 3);
    } else {
        assert_eq!(bar3.voices.len(), 1);
    }
    if major >= 4 {
        assert_eq!(track.lyrics.len(), 1);
        assert_eq!(track.lyrics[0].text, "la la la");
    }
}

#[test]
fn reads_each_version() {
    for major in [3, 4, 5] {
        let data = test_song(major);
        let imported = read(&data).unwrap_or_else(|e| panic!("GP{major}: {e}"));
        assert_eq!(imported.format.usual_extension(), format!("gp{major}"));
        assert_eq!(
            imported.trailing_bytes,
            if major < 5 { 4 } else { 0 },
            "GP{major}"
        );
        check_song(&imported.score, major);
    }
}

#[test]
fn truncated_files_give_errors_not_panics() {
    // FMT-1: cut the file at every length; each must fail cleanly or read.
    for major in [3, 4, 5] {
        let data = test_song(major);
        for len in 0..data.len() - 4 {
            let cut = &data[..len];
            if let Err(e) = read(cut) {
                assert!(
                    matches!(
                        e.kind,
                        ReadErrorKind::UnexpectedEnd
                            | ReadErrorKind::Invalid(_)
                            | ReadErrorKind::UnknownFormat
                    ),
                    "GP{major} cut at {len}: {e}"
                );
            }
        }
    }
}

#[test]
fn damaged_bytes_never_panic() {
    // Flip each byte in turn to a few awkward values.
    for major in [3, 5] {
        let data = test_song(major);
        for i in 31..data.len() {
            for v in [0x00, 0x7f, 0x80, 0xff] {
                let mut d = data.clone();
                d[i] = v;
                let _ = read(&d);
            }
        }
    }
}

#[test]
fn format_comes_from_contents() {
    // FMT-6: the reader ignores names; detection sees GP5 bytes as GP5.
    let data = test_song(5);
    assert_eq!(
        oxiguitar_formats::detect(&data),
        Some(Format::GuitarPro5 {
            version: "5.10".into()
        })
    );
}

#[test]
fn public_fixtures_match_the_writer() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp");
    let bless = std::env::var_os("OXIGUITAR_BLESS").is_some();
    for major in [3, 4, 5] {
        let path = dir.join(format!("oxiguitar-test-song.gp{major}"));
        let expected = test_song(major);
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
        check_song(&read(&on_disk).unwrap().score, major);
    }
}
