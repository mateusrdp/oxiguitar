// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! End-to-end tests of `oxiguitar-cli` on the public fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// A home of its own (`OXIGUITAR_HOME`) whose plugin folder holds our
/// format plugins, built once: the program reads files only through them.
fn home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let built =
            oxiguitar_plugin_test::build_ours(&workspace, false).expect("our plugins build");
        let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-home");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("plugins")).unwrap();
        for (_, name) in oxiguitar_plugin_test::OURS {
            let file = format!("{name}.oxp");
            std::fs::copy(built.join(&file), home.join("plugins").join(&file)).unwrap();
        }
        home
    })
}

fn cli() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oxiguitar-cli"));
    c.env("OXIGUITAR_HOME", home());
    c
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
        "Format:      Guitar Pro 5, read by the Guitar Pro 5 plugin",
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
fn info_reads_gp7_and_gpx_files() {
    for (file, format) in [
        (
            "oxiguitar-test-song.gp",
            "Format:      Guitar Pro 7–8, read by the Guitar Pro 7–8 plugin",
        ),
        (
            "oxiguitar-test-song.gpx",
            "Format:      Guitar Pro 6, read by the Guitar Pro 6 plugin",
        ),
    ] {
        let out = cli()
            .arg("info")
            .arg("--bars")
            .arg(fixture(file))
            .output()
            .unwrap();
        assert!(out.status.success(), "{file}");
        let text = String::from_utf8(out.stdout).unwrap();
        for expected in [
            format,
            "Title:       OxiGuitar Test Song",
            "Time sigs:   4/4 ×2, 3/4 ×1",
            "1. Guitar, 6 strings E2 A2 D3 G3 B3 E4, program 27, 2 voice(s), 13 notes, 1 chord diagram(s), lyrics",
            "Bar check:   every bar matches its time signature",
            "1  4/4  |: [A] \"Intro\" | t1: 6/6",
        ] {
            assert!(
                text.contains(expected),
                "{file}: missing {expected:?} in:\n{text}"
            );
        }
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
    // A plugin declares .gp5 but doesn't recognise it: a bug report.
    assert!(
        err.contains("not-a-song.gp5: the Guitar Pro 5 plugin opens .gp5 files but doesn't recognise this one"),
        "{err}"
    );
    assert!(err.contains("report a bug"), "{err}");
    // Nothing declares .txt: a feature request.
    let text = dir.join("notes.txt");
    std::fs::write(&text, b"this is not a guitar pro file").unwrap();
    let out = cli().arg("info").arg(&text).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("notes.txt: no installed plugin recognises this file"),
        "{err}"
    );
    assert!(err.contains("feature request"), "{err}");
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
        text.contains("Guitar Pro 5 (named .gp3), read by the Guitar Pro 5 plugin"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Returns the system plugin directories for the target platform.
/// Mirrors `Folders::resolve()` system_plugins logic so tests can
/// check whether system plugins are available without importing the
/// host crate.
fn system_plugin_folders() -> Vec<PathBuf> {
    let exe = std::env::current_exe().ok();
    let bin = exe.as_ref().and_then(|p| p.parent());
    let system = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    match system {
        "linux" => {
            let derived = bin
                .and_then(Path::parent)
                .map(|prefix| prefix.join("lib").join("oxiguitar").join("plugins"));
            let fallbacks = [
                PathBuf::from("/usr/lib/oxiguitar/plugins"),
                PathBuf::from("/usr/local/lib/oxiguitar/plugins"),
            ];
            let mut out: Vec<PathBuf> = Vec::with_capacity(3);
            if let Some(p) = derived
                && !fallbacks.contains(&p)
            {
                out.push(p);
            }
            out.extend(fallbacks);
            out
        }
        "macos" => bin
            .and_then(Path::parent)
            .map(|contents| contents.join("PlugIns"))
            .into_iter()
            .chain([PathBuf::from(
                "/Library/Application Support/OxiGuitar/plugins",
            )])
            .collect(),
        "windows" => bin
            .map(|b| b.join("plugins"))
            .into_iter()
            .chain([PathBuf::from("C:/Program Files/OxiGuitar/plugins")])
            .collect(),
        _ => unreachable!(),
    }
}

/// True when any system plugin directory exists and contains .oxp files.
fn system_plugins_available() -> bool {
    system_plugin_folders().iter().any(|dir| {
        dir.is_dir()
            && dir.read_dir().is_ok_and(|mut r| {
                r.any(|e| {
                    e.map(|e| e.file_name().to_string_lossy().ends_with(".oxp"))
                        .unwrap_or(false)
                })
            })
    })
}

#[test]
fn without_plugins_nothing_opens() {
    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-empty-home");
    std::fs::create_dir_all(&empty).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oxiguitar-cli"))
        .env("OXIGUITAR_HOME", &empty)
        .arg("info")
        .arg(fixture("oxiguitar-test-song.gp5"))
        .output()
        .unwrap();
    // System plugins are discoverable even with an empty home.
    // When they are installed (local development) the file opens
    // successfully; when they are not (CI runners) the CLI reports
    // that no plugin that opens files is installed.
    if system_plugins_available() {
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("Guitar Pro 5"), "{text}");
    } else {
        assert!(
            !out.status.success(),
            "expected failure, got success: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("no plugin that opens files is installed"),
            "{err}"
        );
    }
    // Nothing was created in the home folder: the program only reads it.
    assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
}

#[test]
fn convert_writes_a_gp_file_and_names_its_losses() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("convert");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("song.gp");
    let run = cli()
        .arg("convert")
        .arg(fixture("oxiguitar-test-song.gp5"))
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = String::from_utf8(run.stdout).unwrap();
    assert!(
        text.contains("(Guitar Pro 5) →") && text.contains("(Guitar Pro 7–8)"),
        "{text}"
    );
    // What we wrote reads back through the plugin.
    let info = cli().arg("info").arg(&out).output().unwrap();
    let info = String::from_utf8(info.stdout).unwrap();
    assert!(
        info.contains("Format:      Guitar Pro 7–8") && info.contains("OxiGuitar Test Song"),
        "{info}"
    );

    // No plugin writes .gp4 yet; a file without an extension is bad usage.
    let gp5 = cli()
        .arg("convert")
        .arg(fixture("oxiguitar-test-song.gp"))
        .arg(dir.join("song.gp4"))
        .output()
        .unwrap();
    assert_eq!(gp5.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&gp5.stderr)
            .contains("no installed plugin writes .gp4 files (they write .gp, .gp5)")
    );
    let bare = cli()
        .arg("convert")
        .arg(fixture("oxiguitar-test-song.gp"))
        .arg(dir.join("song"))
        .output()
        .unwrap();
    assert_eq!(bare.status.code(), Some(2));

    // The tracks' plugin settings beside the file go beside the new one.
    let source = dir.join("with-settings.gp5");
    std::fs::copy(fixture("oxiguitar-test-song.gp5"), &source).unwrap();
    let kept = r#"{"oxiguitar": 1, "tracks": [{"name": "A", "plugins": {"me.p": {"x": 1}}}]}"#;
    std::fs::write(dir.join("with-settings.gp5.oxg"), kept).unwrap();
    let run = cli()
        .arg("convert")
        .arg(&source)
        .arg(dir.join("with-settings.gp"))
        .output()
        .unwrap();
    assert!(run.status.success());
    let copied = std::fs::read_to_string(dir.join("with-settings.gp.oxg")).unwrap();
    assert!(copied.contains("\"me.p\""), "{copied}");
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

#[test]
fn render_engraves_pages_and_a_pdf() {
    let dir = std::env::temp_dir().join(format!("oxiguitar-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (file, dark) in [
        ("oxiguitar-test-song.gp", false),
        ("oxiguitar-test-song.gp5", true),
    ] {
        let prefix = dir.join("song");
        let pdf = dir.join("song.pdf");
        let mut cmd = cli();
        cmd.arg("render")
            .arg(fixture(file))
            .args(["--scale", "0.5", "--tracks", "all", "--png"])
            .arg(&prefix)
            .arg("--pdf")
            .arg(&pdf);
        if dark {
            cmd.args(["--dark", "--no-dynamics", "--tool", "infer-key-signatures"]);
        }
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "{file}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("1 page(s), 1 system(s)"), "{text}");
        let png = dir.join("song-1.png");
        assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_rejects_bad_usage() {
    for args in [
        &["render"][..],
        &["render", "x.gp"],
        &["render", "x.gp", "--png", "out", "--tracks", "0"],
        &["render", "x.gp", "--png", "out", "--frobnicate"],
        &["render", "x.gp", "--png", "out", "--tool", "frobnicate"],
    ] {
        let out = cli().args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
    let out = cli()
        .args(["render", "does-not-exist.gp", "--quiet"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn tools_are_listed() {
    let out = cli().arg("tools").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("infer-key-signatures"), "{text}");
    assert!(text.contains("Infer Key Signatures"), "{text}");
}

#[test]
fn events_prints_the_score_as_it_plays() {
    let out = cli()
        .arg("events")
        .arg(fixture("oxiguitar-test-song.gp"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    // The golden file was made from the same file by the play crate.
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/events/oxiguitar-test-song.gp.events"),
    )
    .unwrap();
    assert_eq!(text, golden);
    assert!(
        text.starts_with("length ")
            && text.contains("\nbar 1 pass 1 0.000000 ")
            && text.contains("\nnote ")
    );
}

/// A home whose plugin folder holds our format and instrument plugins and
/// whose `soundfonts` folder holds the contract suite's test set, which
/// the SoundFont plugin plays before any set of the system's.
fn sound_home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let built =
            oxiguitar_plugin_test::build_ours(&workspace, false).expect("our plugins build");
        let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-sound-home");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("plugins")).unwrap();
        std::fs::create_dir_all(home.join("soundfonts")).unwrap();
        for (_, name) in oxiguitar_plugin_test::OURS
            .iter()
            .chain(oxiguitar_plugin_test::OUR_INSTRUMENTS)
        {
            let file = format!("{name}.oxp");
            std::fs::copy(built.join(&file), home.join("plugins").join(&file)).unwrap();
        }
        std::fs::write(
            home.join("soundfonts").join("test-sine.sf2"),
            oxiguitar_plugin_test::TEST_SET,
        )
        .unwrap();
        home
    })
}

#[test]
fn play_into_a_wav_file_needs_no_sound_card() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("played.wav");
    let _ = std::fs::remove_file(&out);
    let o = Command::new(env!("CARGO_BIN_EXE_oxiguitar-cli"))
        .env("OXIGUITAR_HOME", sound_home())
        .arg("play")
        .arg(fixture("oxiguitar-test-song.gp5"))
        .arg("--wav")
        .arg(&out)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let said = String::from_utf8_lossy(&o.stdout);
    assert!(said.contains("with SoundFont"), "{said}");
    let wav = std::fs::read(&out).unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..16], b"WAVEfmt ");
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48_000);
    let samples: Vec<i16> = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    assert!(samples.iter().any(|v| v.unsigned_abs() > 100), "silent");
    // The song and two seconds after it.
    let seconds = samples.len() as f64 / 2.0 / 48_000.0;
    let events = cli()
        .arg("events")
        .arg(fixture("oxiguitar-test-song.gp5"))
        .output()
        .unwrap();
    let length: f64 = String::from_utf8_lossy(&events.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("length ")?.trim().parse().ok())
        .unwrap();
    assert!(
        (seconds - length - 2.0).abs() < 0.1,
        "{seconds} s for a song of {length} s"
    );
    // Nothing was made in the user's folder: the plugins' own folders are
    // named to them, not created.
    let mut left: Vec<String> = std::fs::read_dir(sound_home())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["plugins", "soundfonts"]);
}
