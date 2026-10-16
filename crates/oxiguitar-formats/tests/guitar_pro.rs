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
    pub(crate) mod song_check;
}

use oxiguitar_formats::{Format, ReadErrorKind, read};
use support::gp_writer::test_song;
use support::song_check::check_song;

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
