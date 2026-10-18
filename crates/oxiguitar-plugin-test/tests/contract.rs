// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin interface end to end: the test plugin (`plugins/test`) built
//! as a real `.oxp` file, found, read, loaded and called by the host.
//!
//! This is where the two `unsafe` modules (the host's loader and the SDK's
//! export glue) meet, on every system CI runs: ELF, PE and Mach-O
//! manifests, symbols, tables, sinks, panics and calls back into the host.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use oxiguitar_host::api::class::InstrumentClass;
use oxiguitar_host::api::message::ErrorKind;
use oxiguitar_host::api::score::{Score, ScoreInfo, Track};
use oxiguitar_host::{Level, Messages, Plugins, Settings, SharedSettings, State};

/// The test plugin, built once for all the tests here.
fn test_oxp() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        oxiguitar_plugin_test::build(&workspace, "oxiguitar-test-plugin", "test", false)
            .expect("the test plugin builds")
    })
}

/// A folder of its own for one test, holding the given copies of the test
/// plugin.
///
/// A system loads a library file once per process (Linux and macOS know it
/// by its file, so hard links count as one), so every test that loads the
/// plugin gets copies of its own. Tests that only read manifests get hard
/// links where the system allows.
fn folder(name: &str, copies: &[&str]) -> PathBuf {
    make_folder(name, copies, false)
}

fn linked(name: &str, copies: &[&str]) -> PathBuf {
    make_folder(name, copies, true)
}

fn make_folder(name: &str, copies: &[&str], link: bool) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("contract-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for copy in copies {
        let to = dir.join(copy);
        if !link || std::fs::hard_link(test_oxp(), &to).is_err() {
            std::fs::copy(test_oxp(), &to).unwrap();
        }
    }
    dir
}

#[derive(Default)]
struct Heard(Mutex<Vec<(Level, String)>>);

impl Messages for Heard {
    fn notify(&self, level: Level, text: String) {
        self.0.lock().unwrap().push((level, text));
    }
}

fn discover(folders: &[PathBuf], settings: SharedSettings) -> (Plugins, Arc<Heard>) {
    let heard = Arc::new(Heard::default());
    let plugins = Plugins::discover(folders, settings, "x11", heard.clone());
    (plugins, heard)
}

#[test]
fn the_test_plugin_passes_the_contract() {
    let report = oxiguitar_plugin_test::check(test_oxp());
    assert!(report.ok(), "failed: {:#?}", report.failed);
    assert_eq!(report.passed.len(), 11, "{:#?}", report.passed);
    assert!(report.messages.is_empty(), "{:?}", report.messages);
}

#[test]
fn the_manifest_is_read_without_loading_and_the_code_on_first_use() {
    let dir = folder("lazy", &["test.oxp"]);
    let (plugins, heard) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let found = &plugins.found()[0];
    assert_eq!(found.state, State::Ready);
    assert_eq!(found.name(), "Test plugin");
    assert!(!found.is_loaded());

    // Recognising a file uses the manifest's signatures only.
    let file = b"OXGTEST\nTarde em Itapu\xc3\xa3\nextra\n";
    let format = plugins.format_for(file).expect("the signature matches");
    assert_eq!(format.spec.id, "oxgtest");
    assert!(plugins.format_for(b"FICHIER GUITAR PRO v5.10").is_none());
    assert_eq!(plugins.readers().len(), 1);
    assert_eq!(plugins.writers().len(), 1);
    assert!(!found.is_loaded());

    // Reading loads it; the score and its log cross the interface.
    let read = plugins.read(format, file).unwrap();
    assert!(found.is_loaded());
    assert_eq!(read.score.info.title, "Tarde em Itapuã");
    assert_eq!(read.log.len(), 1);
    assert_eq!(read.log[0].message, "lines after the title are ignored");

    // Writing, with what the format loses.
    let score = Score {
        info: ScoreInfo {
            title: "Flor de Lis".into(),
            ..ScoreInfo::default()
        },
        tracks: vec![Track::default()],
        ..read.score
    };
    let written = plugins.write(format, &score).unwrap();
    assert_eq!(written.data, b"OXGTEST\nFlor de Lis\n");
    assert_eq!(
        written.losses[0].level,
        oxiguitar_host::api::message::Level::Warning
    );

    // A damaged file is an error that says where, not a crash.
    let e = plugins.read(format, b"OXGTEST\n\xff\xfe").unwrap_err();
    assert_eq!((e.kind, e.offset), (ErrorKind::Invalid, Some(8)));
    assert!(heard.0.lock().unwrap().is_empty());
}

#[test]
fn services_are_chosen_by_conditions_priority_probe_and_settings() {
    let dir = folder("services", &["test.oxp"]);
    let (plugins, heard) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    // "elsewhere" (Plan 9 only) is never tried; "unavailable" is probed
    // first, says no, and "echo" (priority 1) wins over "second" (0).
    let chosen = plugins.service("test-echo").unwrap();
    assert_eq!(chosen.spec.id, "echo");
    assert_eq!(
        plugins.call_service("test-echo", "echo", b"hi").unwrap(),
        b"hi"
    );
    assert!(plugins.service("audio-device").is_err());

    // Errors cross with their kind, offset and context.
    let e = plugins.call_service("test-echo", "fail", b"").unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert_eq!(e.to_string(), "failed on purpose (in the test) at byte 7");

    // A panic is caught in the plugin and reported; the plugin goes on.
    let e = plugins.call_service("test-echo", "panic", b"").unwrap_err();
    assert_eq!(
        e.message,
        "Test plugin: it crashed (a panic: panicked on purpose)"
    );
    assert_eq!(
        plugins.call_service("test-echo", "echo", b"still").unwrap(),
        b"still"
    );

    // A plugin calling a service through the host while it answers a call
    // (here its own: the host routes it back in).
    assert_eq!(
        plugins
            .call_service("test-echo", "via-host", b"round")
            .unwrap(),
        b"round"
    );

    // Footer messages arrive with the plugin's name.
    plugins
        .call_service("test-echo", "notify", b"hello")
        .unwrap();
    assert_eq!(
        heard.0.lock().unwrap().as_slice(),
        &[(Level::Info, "Test plugin: hello".to_owned())]
    );

    // settings.json pins "second".
    let pinned = SharedSettings::new(
        Settings::parse(r#"{"services": {"test-echo": "oxiguitar.test/second"}}"#).unwrap(),
        None,
    );
    let dir = folder("services-pinned", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), pinned);
    assert_eq!(plugins.service("test-echo").unwrap().spec.id, "second");
    // A pin that can't run here falls back to the usual choice.
    let elsewhere = SharedSettings::new(
        Settings::parse(r#"{"services": {"test-echo": "oxiguitar.test/elsewhere"}}"#).unwrap(),
        None,
    );
    let dir = folder("services-elsewhere", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), elsewhere);
    assert_eq!(plugins.service("test-echo").unwrap().spec.id, "echo");
}

#[test]
fn plugin_settings_live_under_the_plugin_id_beside_everyone_else_s() {
    let dir = folder("settings", &["test.oxp"]);
    let file = dir.join("settings.json");
    std::fs::write(&file, r#"{"view": {"dark_page": true}}"#).unwrap();
    let shared = SharedSettings::new(Settings::load(&file).unwrap(), Some(file.clone()));
    let (plugins, _) = discover(std::slice::from_ref(&dir), shared.clone());
    assert_eq!(
        plugins.call_service("test-echo", "settings", b"").unwrap(),
        b"1"
    );
    assert_eq!(
        plugins.call_service("test-echo", "settings", b"").unwrap(),
        b"2"
    );
    let saved = Settings::load(&file).unwrap();
    assert_eq!(
        saved.get("oxiguitar.test"),
        Some(&serde_json::json!({"count": 2}))
    );
    assert_eq!(
        saved.get("view"),
        Some(&serde_json::json!({"dark_page": true}))
    );
    // The app's own keys go through the same copy and keep the plugin's.
    shared
        .set("view", serde_json::json!({"dark_page": false}))
        .unwrap();
    assert_eq!(
        Settings::load(&file).unwrap().get("oxiguitar.test"),
        Some(&serde_json::json!({"count": 2}))
    );
}

#[test]
fn instruments_are_offered_by_class() {
    let dir = linked("instruments", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let names = |class| -> Vec<String> {
        plugins
            .instruments_for(class)
            .iter()
            .map(|i| i.spec.name.clone())
            .collect()
    };
    assert_eq!(names(InstrumentClass::Drums), ["Test drums"]);
    assert_eq!(
        names(InstrumentClass::Fretted),
        ["Test everything but drums"]
    );
    assert_eq!(
        names(InstrumentClass::of(25, false)),
        ["Test everything but drums"]
    );
    assert!(!plugins.found()[0].is_loaded());
}

#[test]
fn one_file_per_plugin_disabled_ones_are_never_loaded() {
    let a = linked("copies-a", &["test.oxp", "test-again.oxp"]);
    let b = linked("copies-b", &["test.oxp"]);
    let (plugins, _) = discover(&[a.clone(), b.clone()], SharedSettings::default());
    let states: Vec<&State> = plugins.found().iter().map(|f| &f.state).collect();
    // Same version everywhere: the earliest folder, then the earliest name.
    assert_eq!(states[0], &State::Ready);
    assert!(plugins.found()[0].path.ends_with("test-again.oxp"));
    assert!(matches!(states[1], State::Replaced(p) if p.ends_with("test-again.oxp")));
    assert!(matches!(states[2], State::Replaced(_)));
    assert_eq!(plugins.readers().len(), 1);

    let disabled = SharedSettings::new(
        Settings::parse(r#"{"disabled": ["oxiguitar.test"]}"#).unwrap(),
        None,
    );
    let (plugins, _) = discover(std::slice::from_ref(&b), disabled);
    assert_eq!(plugins.found()[0].state, State::Disabled);
    assert!(plugins.readers().is_empty());
    assert!(plugins.service("test-echo").is_err());
    assert!(plugins.load_all().is_empty());
    assert!(!plugins.found()[0].is_loaded());
}

#[test]
fn a_damaged_plugin_file_is_reported_and_never_loaded() {
    let dir = folder("damaged", &[]);
    let bytes = std::fs::read(test_oxp()).unwrap();
    std::fs::write(dir.join("half.oxp"), &bytes[..bytes.len() / 2]).unwrap();
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    // ELF puts its section table last, Mach-O and PE their headers first:
    // either way the host sees the file is cut short.
    assert!(
        matches!(&plugins.found()[0].state, State::Unusable(why) if why.contains("cut short") || why.contains("damaged")),
        "{:?}",
        plugins.found()[0]
    );
    assert!(plugins.load_all().is_empty());
    assert!(!plugins.found()[0].is_loaded());
}

#[test]
fn speed_budget_thirty_manifests() {
    let names: Vec<String> = (0..30).map(|i| format!("p{i:02}.oxp")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let dir = linked("thirty", &names);
    // Once to warm the system's file cache, as a real start finds it.
    let _ = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let started = Instant::now();
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let took = started.elapsed();
    assert_eq!(plugins.found().len(), 30);
    // The budget (docs/design/plugins.md) is for an optimised build, which
    // CI's speed-budget job runs; a debug build gets ten times as long.
    let budget = if cfg!(debug_assertions) { 100.0 } else { 10.0 };
    println!(
        "speed-budget: 30 plugin manifests read in {:.2} ms (budget {budget} ms)",
        took.as_secs_f64() * 1000.0
    );
    assert!(
        took.as_secs_f64() * 1000.0 < budget,
        "30 manifests took {took:?}, over {budget} ms"
    );
}
