// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Tests on the private corpus. The files are never in the repository (see
//! `corpus/README.md`); these tests run only when `corpus/files/` holds them
//! and pass quietly otherwise, so CI and contributors without the files are
//! unaffected.
//!
//! Run with the files in place:
//! `cargo test --release -p oxiguitar-formats --test corpus -- --nocapture`

mod support {
    pub(crate) mod compare;
}

use std::path::{Path, PathBuf};

use oxiguitar_formats::{Format, read};

fn corpus_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    dir.join("tabs").is_dir().then_some(dir)
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Every Guitar Pro 6, 7 and 8 file opens, with a full structure.
#[test]
fn every_gpif_file_reads() {
    let Some(dir) = corpus_dir() else {
        eprintln!("corpus/files not present; skipped");
        return;
    };
    let mut count = 0;
    for set in ["tabs", "scores"] {
        for path in files_in(&dir.join(set)) {
            let data = std::fs::read(&path).unwrap();
            let format = oxiguitar_formats::detect(&data);
            if !matches!(format, Some(Format::GuitarPro6 | Format::GuitarPro7Plus)) {
                continue;
            }
            count += 1;
            let imported = read(&data).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let score = &imported.score;
            score
                .check_structure()
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(score.bar_count() > 0 && !score.tracks.is_empty());
            let notes: usize = score
                .tracks
                .iter()
                .flat_map(|t| &t.staves)
                .flat_map(|s| &s.bars)
                .flat_map(|b| &b.voices)
                .flat_map(|v| &v.beats)
                .map(|b| b.notes.len())
                .sum();
            assert!(notes > 0, "{}: no notes", path.display());
            eprintln!(
                "{}: {} bars, {} tracks, {} notes, {} warnings",
                path.file_name().unwrap().to_string_lossy(),
                score.bar_count(),
                score.tracks.len(),
                notes,
                imported.log.warning_count()
            );
        }
    }
    assert_eq!(count, 15, "the corpus has 15 GPX and GP files (3 + 10 + 2)");
}

/// Songs saved in two formats read into the same music.
///
/// The pairs are different editions of each transcription, so they need not
/// match exactly. Notes are compared by sounding pitch, so a passage fingered
/// on another string still matches. The differences left were each checked
/// against the files (with PyGuitarPro for the older ones) on 16 October 2026:
/// - Manhattan and Cirice: the .gp edition adds notes to some chords and
///   replaces some rests with dead notes.
/// - Trust: Guitar Pro 7's drum kit has no electric snare, so the .gp plays
///   38 where the .gp3 has 40; the .gp moves some vibratos by a beat.
///
/// The thresholds are today's figures, rounded down, to catch regressions.
#[test]
fn songs_in_two_formats_agree() {
    let Some(dir) = corpus_dir() else {
        eprintln!("corpus/files not present; skipped");
        return;
    };
    let tabs = dir.join("tabs");
    // (newer format, older format, minimum share of matching beats and notes)
    let pairs = [
        (
            "metallica-metallica-seek-and-destroy_1424280142.gp",
            "metallica-metallica-seek-and-destroy_1424280142.gp5",
            0.999,
        ),
        (
            "garoto-annibal-augusto-sardinha-desvairada.gp",
            "garoto-annibal-augusto-sardinha-desvairada.gp5",
            0.999,
        ),
        (
            "gary-moore-one_day_ver2.gp",
            "gary-moore-one_day_ver2.gp3",
            0.99,
        ),
        ("ghost-cirice.gp", "ghost-cirice.gp5", 0.98),
        (
            "Eric Johnson - Manhattan.gp",
            "Eric Johnson - Manhattan.gp5",
            0.95,
        ),
        ("megadeth-trust.gp", "megadeth-trust.gp3", 0.91),
    ];
    let mut failures = Vec::new();
    for (new, old, min) in pairs {
        let a = read(&std::fs::read(tabs.join(old)).unwrap()).unwrap().score;
        let b = read(&std::fs::read(tabs.join(new)).unwrap()).unwrap().score;
        let report = support::compare::compare(&a, &b);
        eprintln!("{old} → {new}\n{}", report.summary());
        let exact = [
            "bar count",
            "time signatures",
            "key signatures",
            "repeats",
            "alternate endings",
            "markers",
            "track count",
            "tunings",
            "capos",
        ];
        for category in exact {
            if report.ratio(category) < 1.0 {
                failures.push(format!("{new}: {category} {:.3}", report.ratio(category)));
            }
        }
        for category in ["beats", "notes", "note effects", "beat effects"] {
            if report.ratio(category) < min {
                failures.push(format!("{new}: {category} {:.3}", report.ratio(category)));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
