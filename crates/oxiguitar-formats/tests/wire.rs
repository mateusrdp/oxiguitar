// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Every score we can read crosses the plugin interface without loss: the
//! model, converted to the wire score (version 2, interface 1.2), encoded,
//! decoded and converted back, equals what it was; and so does version 1,
//! for a score without what version 1 lacks (free time). This is what the
//! format plugins rely on.
//!
//! Covers the public fixtures and the 50 visual references, and the
//! private corpus when `corpus/files/` holds it:
//! `cargo test --release -p oxiguitar-formats --test wire -- --nocapture`

use std::path::{Path, PathBuf};

use oxiguitar_model::wire::{from_wire, from_wire2, to_wire, to_wire2};
use oxiguitar_plugin_api::score::Score as WireScore;
use oxiguitar_plugin_api::score2::Score2;
use oxiguitar_plugin_api::{decode, encode};

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.is_file());
    v.sort();
    v
}

/// Reads every Guitar Pro file in `dirs` and checks each round trip.
/// Returns how many files crossed.
fn round_trip_all(dirs: &[PathBuf]) -> usize {
    let mut count = 0;
    let mut bytes_total = 0;
    for path in dirs.iter().flat_map(|d| files(d)) {
        let data = std::fs::read(&path).unwrap();
        let Ok(imported) = oxiguitar_formats::read(&data) else {
            continue;
        };
        let mut score = imported.score;
        let encoded = encode(&to_wire(&score)).unwrap();
        bytes_total += encoded.len();
        let decoded: WireScore = decode(&encoded).unwrap();
        let back = from_wire(decoded).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            back == score,
            "{} changed crossing the interface",
            path.display()
        );
        // Version 2, with a bar in free time.
        score.master_bars[0].free_time = true;
        let encoded = encode(&to_wire2(&score)).unwrap();
        let decoded: Score2 = decode(&encoded).unwrap();
        let back = from_wire2(decoded).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            back == score,
            "{} changed crossing as version 2",
            path.display()
        );
        count += 1;
    }
    eprintln!("{count} scores crossed, {bytes_total} bytes encoded in all");
    count
}

#[test]
fn fixtures_and_references_cross_without_loss() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let n = round_trip_all(&[root.join("gp"), root.join("reference")]);
    assert!(n >= 55, "only {n} fixtures read");
}

#[test]
fn the_corpus_crosses_without_loss() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    if !dir.join("tabs").is_dir() {
        eprintln!("corpus/files not present; skipped");
        return;
    }
    let n = round_trip_all(&[dir.join("tabs"), dir.join("scores")]);
    assert!(n >= 100, "only {n} corpus files read");
}
