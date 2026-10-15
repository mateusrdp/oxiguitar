// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! End-to-end tests of `oxiguitar-cli` on the public fixtures.

use std::path::PathBuf;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oxiguitar-cli"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gp")
        .join(name)
}

#[test]
fn info_prints_the_structure_of_a_gp5_file() {
    let out = cli()
        .arg("info")
        .arg("--bars")
        .arg(fixture("oxiguitar-test-song.gp5"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    for expected in [
        "Format:      Guitar Pro 5.10 (.gp5)",
        "Title:       OxiGuitar Test Song",
        "Bars:        3",
        "Time sigs:   4/4 ×2, 3/4 ×1",
        "1. Guitar, 6 strings E2 A2 D3 G3 B3 E4, program 27, 2 voice(s), 13 notes",
        "Bar check:   every bar matches its time signature",
        "1  4/4  |: \"Intro\" | t1: 6/6",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}

#[test]
fn unreadable_files_fail_with_a_message() {
    let dir = std::env::temp_dir().join(format!("oxiguitar-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let junk = dir.join("not-a-song.gp5");
    std::fs::write(&junk, b"this is not a guitar pro file").unwrap();
    let out = cli().arg("info").arg(&junk).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("not-a-song.gp5: not a file format OxiGuitar knows"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn detect_reports_misleading_extensions() {
    let dir = std::env::temp_dir().join(format!("oxiguitar-cli-detect-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let renamed = dir.join("really-gp5.gp3");
    std::fs::copy(fixture("oxiguitar-test-song.gp5"), &renamed).unwrap();
    let out = cli().arg("detect").arg(&renamed).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("Guitar Pro 5.10 (.gp5) (named .gp3)"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_usage_exits_with_2() {
    let out = cli().arg("frobnicate").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn specimen_writes_a_png_and_a_pdf() {
    let dir = std::env::temp_dir().join(format!("oxiguitar-specimen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (png, pdf) = (dir.join("page.png"), dir.join("page.pdf"));
    let out = cli()
        .args(["specimen", "--scale", "0.5", "--png"])
        .arg(&png)
        .arg("--pdf")
        .arg(&pdf)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn specimen_needs_an_output() {
    let out = cli().arg("specimen").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
