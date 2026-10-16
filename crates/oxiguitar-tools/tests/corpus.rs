// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Runs every tool on every Guitar Pro file of the private corpus, where
//! `corpus/files/` exists (skipped quietly otherwise):
//! `cargo test --release -p oxiguitar-tools --test corpus -- --nocapture`

use std::path::{Path, PathBuf};

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

#[test]
fn tools_run_on_the_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let mut ran = 0;
    for path in all {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_formats::read(&data) else {
            continue;
        };
        let score = imported.score;
        for tool in oxiguitar_tools::all() {
            let outcome = tool.apply(&score);
            if let Some(changed) = &outcome.score {
                assert_eq!(changed.master_bars.len(), score.master_bars.len());
            }
            assert!(!outcome.report.is_empty());
            println!("{}: {}", path.display(), outcome.report);
        }
        ran += 1;
    }
    if ran > 0 {
        assert!(ran >= 100, "{ran}");
    }
}
