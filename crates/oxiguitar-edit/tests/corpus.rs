// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The bar check and the caret on every Guitar Pro file of the private
//! corpus, where `corpus/files/` exists (skipped quietly otherwise), and on
//! the public fixtures always:
//! `cargo test --release -p oxiguitar-edit --test corpus -- --nocapture`

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
            let score = oxiguitar_formats::read(&data).ok()?.score;
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

#[test]
fn the_corpus() {
    let all = scores("corpus/files");
    let (mut bad, mut bars, mut with_bad) = (0, 0, 0);
    let start = Instant::now();
    for (path, score) in &all {
        let check = BarCheck::of(score);
        bad += check.len();
        bars += score
            .tracks
            .iter()
            .flat_map(|t| &t.staves)
            .map(|s| s.bars.len())
            .sum::<usize>();
        if !check.is_empty() {
            with_bad += 1;
            println!("{:4} bars don't add up: {}", check.len(), path.display());
        }
    }
    let took = start.elapsed();
    println!(
        "{bad} of {bars} staff bars don't add up, in {with_bad} of {} files; checked in {took:?}",
        all.len()
    );
    for (_, score) in &all {
        walk(score);
    }
    let known = all
        .iter()
        .find(|(p, _)| p.ends_with("mettalica-master-off-puppets.gp5"));
    if let Some((_, score)) = known {
        assert_eq!(BarCheck::of(score).len(), 107, "counted on 19 Oct 2026");
    }
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
