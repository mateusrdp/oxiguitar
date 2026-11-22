// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The test corpus, for the corpus tests: any folder of files, never
//! committed, given as `OXIGUITAR_CORPUS` (`scripts/test-all.py --corpus DIR`
//! sets it) or found in the repository's `corpus/files/`. Searched
//! recursively; each test takes the files it understands and skips the rest,
//! so any corpus gives a complete run.
//!
//! With no corpus a test checks nothing and says so, unless
//! `OXIGUITAR_CORPUS_REQUIRED` is set (the runner sets it): then it fails.
//! Every file is checked even after one fails; the test fails at the end
//! naming every file that failed, and when `OXIGUITAR_TEST_REPORT_DIR` is
//! set it writes `corpus-NAME.json` there for the runner's summary.
//!
//! The same file is in every OxiGuitar repository (`tests/support/corpus.rs`);
//! include it with `#[path = "support/corpus.rs"] mod corpus;` (from a
//! workspace's crate, `#[path = "../../../tests/support/corpus.rs"]`).

#![allow(dead_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

/// The corpus folder, or `None` when there is none (and none is required).
///
/// `fallback` is the repository's `corpus/files/`, used when
/// `OXIGUITAR_CORPUS` isn't set.
pub(crate) fn folder(fallback: &Path) -> Option<PathBuf> {
    let required = std::env::var_os("OXIGUITAR_CORPUS_REQUIRED").is_some_and(|v| !v.is_empty());
    let dir = match std::env::var_os("OXIGUITAR_CORPUS") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => fallback.to_owned(),
    };
    if dir.is_dir() {
        return Some(dir);
    }
    assert!(
        !required,
        "OXIGUITAR_CORPUS_REQUIRED is set but there is no corpus at {} (set OXIGUITAR_CORPUS)",
        dir.display()
    );
    println!("no corpus at {}: nothing checked", dir.display());
    None
}

/// Every file under `dir`, sorted, hidden files and folders left out.
pub(crate) fn files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'))
            {
                continue;
            }
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

/// What a check made of one file.
pub(crate) enum Outcome {
    /// Checked, and right.
    Passed,
    /// Not a file this test is about (another format, say).
    NotTaken,
}

/// Runs `check` on every corpus file, under `catch_unwind`, and fails at
/// the end if any file failed (an `Err`, or a panic). `name` names the test
/// in the report, such as `"musicxml round trip"`.
pub(crate) fn check(
    name: &str,
    fallback: &Path,
    mut check: impl FnMut(&Path, &[u8]) -> Result<Outcome, String>,
) {
    let Some(dir) = folder(fallback) else {
        return;
    };
    let mut passed = 0usize;
    let mut not_taken = 0usize;
    let mut failures: Vec<(String, u64, String)> = Vec::new();
    for path in files(&dir) {
        let rel = path
            .strip_prefix(&dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                failures.push((rel, 0, format!("can't be read: {e}")));
                continue;
            }
        };
        let size = data.len() as u64;
        match catch_unwind(AssertUnwindSafe(|| check(&path, &data))) {
            Ok(Ok(Outcome::Passed)) => passed += 1,
            Ok(Ok(Outcome::NotTaken)) => not_taken += 1,
            Ok(Err(why)) => failures.push((rel, size, why)),
            Err(panic) => {
                let why = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "a panic".to_owned());
                failures.push((rel, size, format!("panicked: {why}")));
            }
        }
    }
    for (file, size, why) in &failures {
        println!("CORPUS-FAIL {file} ({size} bytes): {why}");
    }
    let checked = passed + failures.len();
    println!(
        "{name}: {checked} corpus files checked, {} failed, {not_taken} not for this test",
        failures.len()
    );
    report(name, &dir, checked, passed, not_taken, &failures);
    assert!(
        failures.is_empty(),
        "{name}: {} of {checked} corpus files failed:\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(f, _, why)| format!("  {f}: {why}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    if checked == 0 {
        println!("{name}: no file of the corpus is for this test");
    }
}

/// `corpus-NAME.json` in `OXIGUITAR_TEST_REPORT_DIR`, when it is set.
fn report(
    name: &str,
    dir: &Path,
    checked: usize,
    passed: usize,
    not_taken: usize,
    failures: &[(String, u64, String)],
) {
    let Some(out) = std::env::var_os("OXIGUITAR_TEST_REPORT_DIR") else {
        return;
    };
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let fails: Vec<String> = failures
        .iter()
        .map(|(file, size, why)| {
            format!(
                "{{\"file\": {}, \"size\": {size}, \"error\": {}}}",
                json(file),
                json(why)
            )
        })
        .collect();
    let text = format!(
        "{{\"test\": {}, \"corpus\": {}, \"checked\": {checked}, \"passed\": {passed}, \
         \"not_taken\": {not_taken}, \"failures\": [{}]}}\n",
        json(name),
        json(&dir.to_string_lossy()),
        fails.join(", ")
    );
    let path = Path::new(&out).join(format!("corpus-{slug}.json"));
    if let Err(e) = std::fs::write(&path, text) {
        println!("couldn't write {}: {e}", path.display());
    }
}

/// `s` as a JSON string.
fn json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
