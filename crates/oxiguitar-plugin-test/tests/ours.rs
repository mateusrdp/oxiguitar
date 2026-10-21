// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Our own plugins, built as the `.oxp` files we ship, through the host:
//! the contract, each plugin reading its own format and nothing else, every
//! public Guitar Pro file (and the private corpus, when `corpus/files/` is
//! there) read exactly as the reader reads it in-process, and the refusals
//! when no plugin recognises a file (Mateus, 18 Oct 2026).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use oxiguitar_host::api::Blob;
use oxiguitar_host::api::message::{Answer, ErrorKind, LogEntry, Request};
use oxiguitar_host::{Plugins, SharedSettings, Unheard, Unrecognised};
use oxiguitar_plugin_test::{OURS, build_ours, check};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Our plugins, built once for every test here (debug builds).
fn ours() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| build_ours(&workspace(), false).expect("our plugins build"))
}

/// One host over our plugins for every test that loads them: a system
/// loads a library file once per process.
fn plugins() -> &'static Plugins {
    static PLUGINS: OnceLock<Plugins> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        Plugins::discover(
            &[ours().to_owned()],
            SharedSettings::default(),
            oxiguitar_host::display_system(),
            Arc::new(Unheard),
        )
    })
}

fn fixture(ext: &str) -> PathBuf {
    workspace()
        .join("fixtures/gp")
        .join(format!("oxiguitar-test-song.{ext}"))
}

#[test]
fn our_plugins_pass_the_contract() {
    for (_, name) in OURS {
        let report = check(&ours().join(format!("{name}.oxp")));
        assert!(report.ok(), "{name}.oxp: {:#?}", report.failed);
        assert!(report.passed.len() >= 6, "{name}.oxp: {:#?}", report.passed);
    }
}

#[test]
fn the_list_of_our_plugins_is_the_plugins_folder() {
    let mut folders: Vec<String> = std::fs::read_dir(workspace().join("plugins"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        // The test plugins, never shipped.
        .filter(|n| n != "test" && !n.starts_with("test-"))
        .collect();
    folders.sort();
    let mut listed: Vec<String> = OURS.iter().map(|(_, n)| (*n).to_owned()).collect();
    listed.sort();
    assert_eq!(folders, listed);
    for (package, name) in OURS {
        assert_eq!(*package, format!("oxiguitar-{name}-plugin"));
    }
}

#[test]
fn each_plugin_recognises_and_reads_its_own_format_and_nothing_else() {
    let plugins = plugins();
    for (_, name) in OURS {
        let data = std::fs::read(fixture(name)).unwrap();
        let format = plugins
            .recognise(&data, Some(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            format.plugin.manifest.as_ref().unwrap().id,
            format!("oxiguitar.{name}")
        );
        let read = plugins.read(format, &data).unwrap();
        assert_eq!(read.score.info.title, "OxiGuitar Test Song", "{name}");
        // Every other plugin, asked directly, refuses the file: it holds
        // only its own reader.
        for (_, other) in OURS.iter().filter(|(_, o)| o != name) {
            let found = plugins
                .found()
                .iter()
                .find(|f| {
                    f.manifest
                        .as_ref()
                        .is_some_and(|m| m.id == format!("oxiguitar.{other}"))
                })
                .unwrap();
            let answer = plugins.send(
                found,
                &Request::Read {
                    format: (*other).to_owned(),
                    data: Blob::borrowed(&data),
                },
            );
            match answer {
                Err(e) => assert_eq!(e.kind, ErrorKind::Unsupported, "{other} on {name}: {e}"),
                Ok(Answer::Read { .. }) => panic!("{other}.oxp read a .{name} file"),
                Ok(other) => panic!("{other:?}"),
            }
        }
    }
    assert_eq!(
        plugins.read_extensions(),
        ["gp", "gp3", "gp4", "gp5", "gpx"],
        "File → Open's filter, in plugin order"
    );
}

/// What a reader made of a file: the score and its log, or the error's
/// sentence.
type Outcome = Result<(oxiguitar_model::Score, Vec<LogEntry>), String>;

fn in_process(data: &[u8]) -> Outcome {
    oxiguitar_formats::read(data)
        .map(|i| {
            let log = oxiguitar_formats::plugin::log(&i.log);
            (i.score, log)
        })
        .map_err(|e| e.to_string())
}

fn through_plugins(data: &[u8]) -> Outcome {
    let plugins = plugins();
    let format = plugins.recognise(data, None).map_err(|e| e.to_string())?;
    // The host prefixes a plugin's error with the plugin's name.
    let prefix = format!("{}: ", format.plugin.name());
    let read = plugins.read(format, data).map_err(|e| {
        let text = e.to_string();
        text.strip_prefix(&prefix)
            .map_or(text.clone(), str::to_owned)
    })?;
    let score = oxiguitar_model::wire::from_wire2(read.score)?;
    Ok((score, read.log))
}

fn guitar_pro_files() -> Vec<PathBuf> {
    let root = workspace();
    let mut files = Vec::new();
    for dir in [
        "fixtures/gp",
        "fixtures/reference",
        "corpus/files/tabs",
        "corpus/files/scores",
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in entries {
            let path = e.unwrap().path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if matches!(ext.as_deref(), Some("gp" | "gpx" | "gp3" | "gp4" | "gp5")) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn the_plugins_read_every_file_exactly_as_the_readers_do() {
    let files = guitar_pro_files();
    assert!(
        files.len() >= 55,
        "the public fixtures and references are there"
    );
    let mut later_formats = 0;
    let mut later_opened = 0;
    for path in &files {
        let data = std::fs::read(path).unwrap();
        let direct = in_process(&data);
        let plugged = through_plugins(&data);
        assert!(
            direct == plugged,
            "{}: the plugin path differs",
            path.display()
        );
        let opened = plugged.is_ok();
        // M1's gate: Guitar Pro 5 and later, from the file's contents.
        if data.starts_with(b"\x18FICHIER GUITAR PRO v5")
            || data.starts_with(b"BCF")
            || data.starts_with(b"PK")
        {
            if path.starts_with(workspace().join("corpus")) {
                later_formats += 1;
                later_opened += usize::from(opened);
            }
        } else if path.starts_with(workspace().join("corpus")) {
            assert!(opened, "{}: {:?}", path.display(), plugged.err());
        }
    }
    if later_formats > 0 {
        println!(
            "corpus, Guitar Pro 5 and later: {later_opened} of {later_formats} open through the plugins"
        );
        assert_eq!((later_opened, later_formats), (42, 42));
    }
}

#[test]
fn a_file_no_plugin_recognises_is_refused_with_the_right_report() {
    let plugins = plugins();
    // Nothing declares .txt: a feature request.
    let e = plugins.recognise(b"hello", Some("txt")).unwrap_err();
    assert_eq!(e, Unrecognised::Unknown);
    assert!(e.to_string().contains("feature request"), "{e}");
    assert!(e.to_string().contains(oxiguitar_host::ISSUES_URL), "{e}");
    // Guitar Pro 1–2, which no plugin reads, the same way.
    let mut gtp = vec![24u8];
    gtp.extend_from_slice(b"FICHIER GUITARE PRO v1.04");
    gtp.resize(64, 0);
    assert_eq!(
        plugins.recognise(&gtp, Some("gtp")).unwrap_err(),
        Unrecognised::Unknown
    );
    // The Guitar Pro 5 plugin declares .gp5 but doesn't recognise this
    // one: a bug report.
    let e = plugins.recognise(b"hello", Some("GP5")).unwrap_err();
    assert_eq!(
        e,
        Unrecognised::NotRecognised {
            plugin: "Guitar Pro 5".into(),
            format: "Guitar Pro 5".into(),
            extension: "gp5".into(),
        }
    );
    assert!(e.to_string().contains("report a bug"), "{e}");
    // The extension never decides: a Guitar Pro 5 file named .gp3 is
    // read as Guitar Pro 5 (four such files are in the corpus).
    let gp5 = std::fs::read(fixture("gp5")).unwrap();
    let format = plugins.recognise(&gp5, Some("gp3")).unwrap();
    assert_eq!(format.spec.id, "gp5");
}

#[test]
fn a_disabled_plugin_and_no_plugin_at_all() {
    // Only manifests are read here, so the files can be shared.
    let mut settings = oxiguitar_host::Settings::default();
    settings.set("disabled", serde_json::json!(["oxiguitar.gp5"]));
    let disabled = Plugins::discover(
        &[ours().to_owned()],
        SharedSettings::new(settings, None),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let e = disabled.recognise(b"hello", Some("gp5")).unwrap_err();
    assert!(
        matches!(&e, Unrecognised::NotUsable { plugin, .. } if plugin == "Guitar Pro 5"),
        "{e:?}"
    );
    assert!(e.to_string().contains("disabled"), "{e}");
    // A real .gp5 file isn't opened by the disabled plugin either.
    let gp5 = std::fs::read(fixture("gp5")).unwrap();
    assert!(disabled.recognise(&gp5, Some("gp5")).is_err());

    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ours-no-plugins");
    std::fs::create_dir_all(&empty).unwrap();
    let none = Plugins::discover(
        &[empty],
        SharedSettings::default(),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let e = none.recognise(&gp5, Some("gp5")).unwrap_err();
    assert_eq!(e, Unrecognised::NoReaders);
    assert!(e.to_string().contains(oxiguitar_host::PLUGINS_URL), "{e}");
    assert!(none.read_extensions().is_empty());
}

#[test]
fn format_plugins_add_no_tabs_and_hear_no_events() {
    use oxiguitar_host::api::event::{Event, EventKind};
    let plugins = plugins();
    assert!(plugins.tabs().is_empty());
    for kind in [EventKind::Score, EventKind::Selection, EventKind::Playback] {
        assert!(!plugins.wants(kind));
    }
    // Pushed anyway: nobody is told, and nothing waits.
    plugins.push(Event::ScoreClosed);
    plugins.wait_for_events();
}
