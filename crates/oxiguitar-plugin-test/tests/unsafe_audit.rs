// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Where `unsafe` code may be (docs/design/plugins.md, "Where the `unsafe`
//! is"): the host's loader and the SDK's export glue, plus the two
//! attributes `export_plugin!` generates, and the contract suite's watch
//! for system calls (test code, never shipped). Crates that hold these deny
//! `unsafe_code` and allow it in that one place; every other crate forbids
//! it (the workspace's lints), or denies it where generated code needs a
//! local allowance (the app, for Slint's generated code).
//!
//! The compiler enforces the lints; this test makes sure nobody quietly
//! adds an allowance anywhere else.

use std::path::{Path, PathBuf};

/// The only places our own source may allow `unsafe` code.
const ALLOWED: &[&str] = &[
    "crates/oxiguitar-host/src/loader.rs",
    "crates/oxiguitar-plugin/src/glue.rs",
    // export_plugin!: the manifest section and the entry symbol.
    "crates/oxiguitar-plugin/src/lib.rs",
    // The contract suite catching system calls while a plugin renders
    // (Linux: fork, seccomp, a signal handler). Test code, never shipped.
    "crates/oxiguitar-plugin-test/src/syscalls.rs",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            // Build output, throwaway experiments (own workspaces) and the
            // fuzz targets' build folder.
            if !matches!(name.to_str(), Some("target" | "spikes" | ".git")) {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn unsafe_code_is_allowed_only_in_the_loader_and_the_sdk_glue() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(
        files.len() > 50,
        "found only {} files under {}",
        files.len(),
        root.display()
    );
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        if text.contains("allow(unsafe_code)") && !file.ends_with("tests/unsafe_audit.rs") {
            let relative = file.strip_prefix(&root).unwrap();
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    found.sort();
    let mut allowed: Vec<String> = ALLOWED.iter().map(|s| (*s).to_owned()).collect();
    allowed.sort();
    assert_eq!(
        found, allowed,
        "`unsafe` allowed outside the loader and the SDK glue"
    );
}
