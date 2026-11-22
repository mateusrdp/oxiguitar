// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The bar check and the caret on every Guitar Pro file of the private
//! test corpus (`OXIGUITAR_CORPUS`, or `corpus/files/`; see
//! `tests/support/corpus.rs`), file by file, and on the public fixtures
//! always:
//! `cargo test --release -p oxiguitar-edit --test corpus -- --nocapture`

#[path = "../../../tests/support/corpus.rs"]
mod corpus;
#[path = "../../../tests/support/guitar_pro.rs"]
mod guitar_pro;

use std::path::{Path, PathBuf};
use std::time::Instant;

use oxiguitar_edit::{BarCheck, Caret};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn scores(dir: &str) -> Vec<(PathBuf, oxiguitar_model::Score)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(dir);
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort();
    all.into_iter()
        .filter_map(|p| {
            let data = std::fs::read(&p).ok()?;
            let score = oxiguitar_plugin_test::released::guitar_pro::read(&data)
                .ok()?
                .score;
            Some((p, score))
        })
        .collect()
}

/// Walks the caret through every beat of every staff and voice 1: it ends
/// on the last bar and never leaves the score.
fn walk(score: &oxiguitar_model::Score) {
    for track in 0..score.tracks.len() {
        let Some(mut c) = Caret::at_start(score, track) else {
            continue;
        };
        loop {
            let next = c.next_beat(score);
            assert_eq!(next.clamp(score), Some(next));
            if next == c {
                break;
            }
            c = next;
        }
        assert_eq!(c.bar + 1, score.bar_count());
    }
}

/// Every Guitar Pro file of the test corpus: the bar check runs (bars that
/// don't add up are counted, not failed: files have them), and the caret
/// walks every beat and stays in the score. Our corpus's Master of Puppets,
/// where it is, has the 107 bars counted on 19 Oct 2026.
#[test]
fn the_corpus() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let start = Instant::now();
    corpus::check("bar check and caret", &fallback, |path, data| {
        let Some(score) = guitar_pro::read(data)? else {
            return Ok(corpus::Outcome::NotTaken);
        };
        let check = BarCheck::of(&score);
        if !check.is_empty() {
            println!("{:4} bars don't add up: {}", check.len(), path.display());
        }
        walk(&score);
        if guitar_pro::ours(path, data, "mettalica-master-off-puppets.gp5", 170_975)
            && check.len() != 107
        {
            return Err(format!(
                "{} bars don't add up, not the 107 counted on 19 Oct 2026",
                check.len()
            ));
        }
        Ok(corpus::Outcome::Passed)
    });
    println!("checked in {:?}", start.elapsed());
}

#[test]
fn the_fixtures_add_up() {
    // Our own files are written whole: every bar of every reference and
    // fixture adds up, which also shows the check has no false alarms.
    // Files made by hand in Guitar Pro (`fixtures/guitar-pro-7.6/`) are
    // Mateus's, a note or two in a bar.
    let all = scores("fixtures");
    assert!(all.len() >= 50, "{}", all.len());
    for (path, score) in all
        .iter()
        .filter(|(p, _)| !p.to_string_lossy().contains("guitar-pro-7.6"))
        // The .gp5 files there are made with named losses (conversions.txt:
        // Guitar Pro 5 holds one dot, three tuplets of a few kinds), and
        // a bar can come out short. Their sources are checked.
        .filter(|(p, _)| {
            !(p.to_string_lossy().contains("guitar-pro-check")
                && p.extension().is_some_and(|x| x == "gp5"))
        })
    {
        let check = BarCheck::of(score);
        assert!(
            check.is_empty(),
            "{}: {:?}",
            path.display(),
            check.iter().collect::<Vec<_>>()
        );
        walk(score);
    }
}
