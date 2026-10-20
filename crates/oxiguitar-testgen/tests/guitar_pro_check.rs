// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `fixtures/guitar-pro-check/` holds what the .gp writer makes of every
//! reference song and of the test song in every format, kept in the
//! repository for checking in Guitar Pro (Mateus, 20 Oct 2026; see
//! CONTRIBUTING.md). This test keeps them current: each reads back as the
//! writer's file for its source does today. Scores are compared, not
//! bytes (the compressor's output depends on the CPU).
//!
//! After a change to the writer or the references:
//! `scripts/guitar-pro-check.sh`.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> oxiguitar_model::Score {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    oxiguitar_formats::read(&data)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .score
}

#[test]
fn the_files_for_guitar_pro_are_current() {
    let root = root();
    let mut sources: Vec<(PathBuf, String)> = std::fs::read_dir(root.join("fixtures/reference"))
        .unwrap()
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gp"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (p, name)
        })
        .collect();
    for ext in ["gp", "gp3", "gp4", "gp5", "gpx"] {
        sources.push((
            root.join(format!("fixtures/gp/oxiguitar-test-song.{ext}")),
            format!("test-song-from-{ext}.gp"),
        ));
    }
    sources.sort();
    let check = root.join("fixtures/guitar-pro-check");
    let kept = std::fs::read_dir(&check)
        .unwrap()
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gp"))
        .count();
    assert_eq!(kept, sources.len(), "one file per source, no others");
    let mut stale = Vec::new();
    for (source, name) in &sources {
        let written = oxiguitar_formats::write_gp(&read(source)).unwrap();
        let now = oxiguitar_formats::read(&written.data).unwrap().score;
        if read(&check.join(name)) != now {
            stale.push(name.clone());
        }
    }
    assert!(
        stale.is_empty(),
        "fixtures/guitar-pro-check/ is out of date for {stale:?}: run scripts/guitar-pro-check.sh"
    );
}
