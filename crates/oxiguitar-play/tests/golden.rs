// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The timeline of every reference song and the test song, against its
//! golden file in `fixtures/events/` (M4's gate: "golden-file tests cover
//! event output"). After a deliberate change, rewrite them with
//! `OXIGUITAR_BLESS=1 cargo test -p oxiguitar-play --test golden` and read
//! the diff.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ["fixtures/reference", "fixtures/gp"]
        .iter()
        .flat_map(|d| {
            std::fs::read_dir(root().join(d))
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "gp" | "gp5" | "gpx" | "gp4" | "gp3"))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn timelines_match_their_golden_files() {
    let bless = std::env::var_os("OXIGUITAR_BLESS").is_some();
    let dir = root().join("fixtures/events");
    std::fs::create_dir_all(&dir).unwrap();
    let mut failed = Vec::new();
    let all = sources();
    assert!(all.len() > 40, "the reference songs are missing");
    for path in all {
        let data = std::fs::read(&path).unwrap();
        let score = oxiguitar_formats::read(&data).unwrap().score;
        let text = oxiguitar_play::text::timeline(&oxiguitar_play::Timeline::new(&score));
        let name = format!("{}.events", path.file_name().unwrap().to_string_lossy());
        let golden = dir.join(&name);
        if bless {
            std::fs::write(&golden, &text).unwrap();
            continue;
        }
        match std::fs::read_to_string(&golden) {
            Ok(want) if want == text => {}
            Ok(want) => {
                let first = want
                    .lines()
                    .zip(text.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(want.lines().count().min(text.lines().count()));
                failed.push(format!(
                    "{name}: line {}:\n  want {}\n  got  {}",
                    first + 1,
                    want.lines().nth(first).unwrap_or("(end)"),
                    text.lines().nth(first).unwrap_or("(end)")
                ));
            }
            Err(_) => failed.push(format!(
                "{name}: no golden file (run with OXIGUITAR_BLESS=1)"
            )),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
