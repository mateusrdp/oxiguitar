// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Runs every tool on every Guitar Pro file of the test corpus
//! (`OXIGUITAR_CORPUS`, or `corpus/files/`; see `tests/support/corpus.rs`):
//! `cargo test --release -p oxiguitar-tools --test corpus -- --nocapture`

#[path = "../../../tests/support/corpus.rs"]
mod corpus;
#[path = "../../../tests/support/guitar_pro.rs"]
mod guitar_pro;

use std::path::Path;

/// Every tool on every Guitar Pro file of the test corpus, file by file:
/// each says what it did, and what it changes keeps the bars.
#[test]
fn tools_run_on_the_corpus() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    corpus::check("tools", &fallback, |path, data| {
        let Some(score) = guitar_pro::read(data)? else {
            return Ok(corpus::Outcome::NotTaken);
        };
        for tool in oxiguitar_tools::all() {
            let outcome = tool.apply(&score);
            if let Some(changed) = &outcome.score
                && changed.master_bars.len() != score.master_bars.len()
            {
                return Err(format!("{} changed the number of bars", tool.name()));
            }
            if outcome.report.is_empty() {
                return Err(format!("{} said nothing", tool.name()));
            }
            println!("{}: {}", path.display(), outcome.report);
        }
        Ok(corpus::Outcome::Passed)
    });
}
