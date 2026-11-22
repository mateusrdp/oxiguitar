// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The corpus files the core's corpus tests take: Guitar Pro 3 to 8,
//! recognised by their contents as the Guitar Pro plugins' signatures do,
//! and read through the released Guitar Pro plugins as the app reads them
//! (`oxiguitar_plugin_test::released`; `OXIGUITAR_PLUGINS_DIR` for local
//! builds). Include it beside `corpus.rs`:
//! `#[path = "../../../tests/support/guitar_pro.rs"] mod guitar_pro;`.

#![allow(dead_code)]

use oxiguitar_model::Score;

/// A Guitar Pro 3, 4 or 5 file (its header), 6 (its container) or 7 to 8
/// (a ZIP holding `Content/score.gpif`).
pub(crate) fn is_guitar_pro(data: &[u8]) -> bool {
    let gp3_to_5 = data
        .get(1..)
        .is_some_and(|d| d.starts_with(b"FICHIER GUITAR PRO"));
    let gp6 = data.starts_with(b"BCFZ") || data.starts_with(b"BCFS");
    let gp7 = data.starts_with(b"PK") && data.windows(18).any(|w| w == b"Content/score.gpif");
    gp3_to_5 || gp6 || gp7
}

/// The score of a Guitar Pro file, `None` for any other file, or why a
/// Guitar Pro file doesn't open.
pub(crate) fn read(data: &[u8]) -> Result<Option<Score>, String> {
    if !is_guitar_pro(data) {
        return Ok(None);
    }
    oxiguitar_plugin_test::released::guitar_pro::read(data)
        .map(|imported| Some(imported.score))
        .map_err(|e| format!("doesn't open: {e}"))
}

/// The file our corpus has under `name`, `size` bytes long: for the few
/// checks counted on our own files, which another corpus may not have.
pub(crate) fn ours(path: &std::path::Path, data: &[u8], name: &str, size: usize) -> bool {
    path.file_name().is_some_and(|n| n == name) && data.len() == size
}
