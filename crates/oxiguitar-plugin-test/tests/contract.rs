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
use oxiguitar_host::api::display::{Item, Paint, Point};
use oxiguitar_host::api::event::{Event, EventKind, Playback, Position, Selection, Selection2};
use oxiguitar_host::api::message::ErrorKind;
use oxiguitar_host::api::score::{Score, ScoreInfo, Track};
use oxiguitar_host::api::tab::{
    Area, Pointer, PointerButton, PointerKind, TabContent, TabEvent, Widget,
};
use oxiguitar_host::{Core, Level, Plugins, Settings, SharedSettings, State, TabId};

/// The test plugin, built once for all the tests here.
fn test_oxp() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        oxiguitar_plugin_test::build(&workspace, "oxiguitar-test-plugin", "test", false)
            .expect("the test plugin builds")
    })
}

/// The second test plugin (`plugins/test-second`), which reads the test
/// plugin's format too, built once.
fn second_oxp() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        oxiguitar_plugin_test::build(
            &workspace,
            "oxiguitar-test-second-plugin",
            "test-second",
            false,
        )
        .expect("the second test plugin builds")
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

/// The rest of the core, as plugins see it: what they said, which tabs
/// asked to be redrawn, a score that is open, and the plugins withdrawn.
#[derive(Default)]
struct Heard(
    Mutex<Vec<(Level, String)>>,
    Mutex<Vec<TabId>>,
    Option<Score>,
    Mutex<Vec<String>>,
);

impl Core for Heard {
    fn notify(&self, level: Level, text: String) {
        self.0.lock().unwrap().push((level, text));
    }

    fn redraw(&self, tab: &TabId) {
        self.1.lock().unwrap().push(tab.clone());
    }

    fn score(&self) -> Option<Score> {
        self.2.clone()
    }

    fn withdrawn(&self, id: &str) {
        self.3.lock().unwrap().push(id.to_owned());
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
    // 11 for formats and services; 2 tabs; 10 uses of the panel's widgets
    // (2 buttons, 5 other widgets, 3 pointer events on its canvas); 3 kinds
    // of event, the selection in both its forms; 2 tools; the settings
    // window and its 2 widgets; the track section and its 2 widgets; 2
    // instruments and an effect, 4 real-time checks each.
    assert_eq!(
        report.passed.len(),
        11 + 2 + 10 + 4 + 2 + 3 + 3 + 3 * 4,
        "{:#?}",
        report.passed
    );
    // The tool `link` says something on purpose.
    assert_eq!(report.messages.len(), 1, "{:?}", report.messages);
    assert!(
        report.messages[0].contains("A footer message with a link"),
        "{:?}",
        report.messages
    );
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
    let score = oxiguitar_host::api::score2::Score2::from_v1(Score {
        info: ScoreInfo {
            title: "Flor de Lis".into(),
            ..ScoreInfo::default()
        },
        tracks: vec![Track::default()],
        ..read.score.to_v1()
    });
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

fn panel() -> TabId {
    TabId {
        plugin: "oxiguitar.test".into(),
        tab: "panel".into(),
    }
}

fn label(content: &TabContent, row: usize, at: usize) -> &str {
    match &content.rows[row].widgets[at] {
        Widget::Label { text, .. } => text,
        other => panic!("not a label: {other:?}"),
    }
}

#[test]
fn tabs_come_from_the_manifest_and_their_widgets_from_the_plugin() {
    let dir = folder("tabs", &["test.oxp"]);
    let (plugins, heard) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    // Listed, with their areas, before the plugin is loaded.
    let tabs: Vec<(String, String, Area)> = plugins
        .tabs()
        .iter()
        .map(|t| (t.spec.id.clone(), t.spec.title.clone(), t.spec.area))
        .collect();
    assert_eq!(
        tabs,
        [
            ("panel".into(), "Test".into(), Area::Bottom),
            ("side".into(), "Test side".into(), Area::Right),
        ]
    );
    assert!(!plugins.found()[0].is_loaded());

    // Shown: the plugin loads and answers with its widgets.
    let content = plugins.tab_content(&panel()).unwrap();
    assert!(plugins.found()[0].is_loaded());
    assert_eq!(label(&content, 0, 0), "Clicks: 0");
    let kinds: Vec<&str> = content
        .rows
        .iter()
        .flat_map(|r| &r.widgets)
        .map(|w| match w {
            Widget::Label { .. } => "label",
            Widget::Button { .. } => "button",
            Widget::Toggle { .. } => "toggle",
            Widget::Slider { .. } => "slider",
            Widget::Field { .. } => "field",
            Widget::Choice { .. } => "choice",
            Widget::List { .. } => "list",
            Widget::Canvas(_) => "canvas",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "label", "button", "button", "toggle", "slider", "field", "choice", "list", "canvas",
            "label", "label"
        ]
    );

    // Every widget's event reaches the plugin, which answers with its new
    // contents.
    let after = |widget: &str, event: TabEvent| plugins.tab_event(&panel(), widget, event).unwrap();
    assert_eq!(label(&after("click", TabEvent::Clicked), 0, 0), "Clicks: 1");
    let c = after("toggle", TabEvent::Toggled(true));
    assert!(matches!(
        c.widget("toggle"),
        Some(Widget::Toggle { on: true, .. })
    ));
    let c = after("slider", TabEvent::Slid(7.5));
    assert!(matches!(c.widget("slider"), Some(Widget::Slider { value, .. }) if *value == 7.5));
    let c = after("field", TabEvent::Edited("Lá".into()));
    assert!(matches!(c.widget("field"), Some(Widget::Field { text, .. }) if text == "Lá"));
    let c = after("choice", TabEvent::Chosen(2));
    assert!(matches!(
        c.widget("choice"),
        Some(Widget::Choice { selected: 2, .. })
    ));
    let c = after("list", TabEvent::Chosen(1));
    assert!(matches!(
        c.widget("list"),
        Some(Widget::List {
            selected: Some(1),
            ..
        })
    ));

    // The canvas: pointer events in its own units, and its display list
    // back.
    for kind in [PointerKind::Down, PointerKind::Drag] {
        let c = after(
            "canvas",
            TabEvent::Pointer(Pointer {
                kind,
                at: Point::new(30.0, 20.0),
                button: PointerButton::Primary,
            }),
        );
        let Some(Widget::Canvas(canvas)) = c.widget("canvas") else {
            panic!("no canvas");
        };
        assert_eq!(
            (canvas.width, canvas.height),
            oxiguitar_test_plugin_canvas()
        );
        assert!(canvas.items.contains(&Item::Rect {
            at: Point::new(28.0, 18.0),
            width: 4.0,
            height: 4.0,
            paint: Paint::Accent,
        }));
        assert!(
            canvas
                .items
                .iter()
                .any(|i| matches!(i, Item::Text { text, .. } if *text == format!("{kind:?}")))
        );
    }

    // The plugin's own refusal, in its words, goes to the footer, and the
    // tab keeps its contents; a tab nobody has.
    let kept = plugins
        .tab_event(&panel(), "click", TabEvent::Toggled(true))
        .unwrap();
    assert_eq!(label(&kept, 0, 0), "Clicks: 1");
    {
        let told = heard.0.lock().unwrap();
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].0, Level::Warning);
        assert!(
            told[0].1.starts_with("Test plugin: ") && told[0].1.contains("doesn't take"),
            "{told:?}"
        );
    }
    let nobody = TabId {
        plugin: "oxiguitar.test".into(),
        tab: "nothing".into(),
    };
    assert_eq!(
        plugins.tab_content(&nobody).unwrap_err().kind,
        ErrorKind::NotFound
    );

    // The other tab, in another area.
    let side = plugins.tabs()[1].id();
    assert_eq!(
        label(&plugins.tab_content(&side).unwrap(), 0, 0),
        "The test plugin's other tab."
    );

    // Contents the core can't show break the contract: the plugin is
    // withdrawn, its tabs with it, as if it weren't installed.
    plugins.call_service("test-echo", "break", b"").unwrap();
    let e = plugins.tab_content(&panel()).unwrap_err();
    assert!(
        e.message
            .starts_with("Test plugin stopped working: its tab Test can't be shown (")
            && e.message.contains("used twice"),
        "{e}"
    );
    assert!(plugins.tabs().is_empty());
    assert_eq!(
        plugins.tab_content(&side).unwrap_err().kind,
        ErrorKind::NotFound
    );
    assert_eq!(*heard.3.lock().unwrap(), ["oxiguitar.test"]);
    let told = heard.0.lock().unwrap();
    assert_eq!(told.last().map(|t| t.0), Some(Level::Error));
}

#[test]
fn a_plugin_that_fails_is_withdrawn_as_if_it_were_not_installed() {
    // The test plugin first, the second test plugin in a later folder:
    // both read the test format, and the first one is chosen.
    let first = folder("withdrawn", &["test.oxp"]);
    let later = Path::new(env!("CARGO_TARGET_TMPDIR")).join("contract-withdrawn-later");
    let _ = std::fs::remove_dir_all(&later);
    std::fs::create_dir_all(&later).unwrap();
    std::fs::copy(second_oxp(), later.join("test-second.oxp")).unwrap();
    let (plugins, heard) = discover(&[first, later], SharedSettings::default());
    let file = b"OXGTEST\nTarde em Itapu\xc3\xa3\n";
    let opened = plugins.open(file, Some("oxgtest")).unwrap();
    assert_eq!(opened.format.plugin.name(), "Test plugin");
    assert_eq!(opened.read.score.info.title, "Tarde em Itapuã");
    assert_eq!(plugins.tabs().len(), 2);
    assert!(plugins.wants(EventKind::Score));

    // An ordinary error is an answer, not a failure.
    let e = plugins.call_service("test-echo", "fail", b"").unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(plugins.found()[0].failure().is_none());

    // A crash while reading: the test plugin is withdrawn, and the file
    // goes to the next plugin that recognises it.
    let opened = plugins.open(b"OXGTEST\npanic\n", Some("oxgtest")).unwrap();
    assert_eq!(opened.format.plugin.name(), "Second test plugin");
    assert_eq!(opened.read.score.info.title, oxiguitar_test_second_title());
    let failed = &plugins.found()[0];
    assert!(failed.is_loaded());
    assert!(
        failed
            .failure()
            .is_some_and(|why| why.contains("panicked on purpose, reading")),
        "{failed:?}"
    );
    // From now on it is as if it weren't installed: every file of the
    // format goes to the second plugin, and its tabs, services and events
    // are gone.
    let opened = plugins.open(file, Some("oxgtest")).unwrap();
    assert_eq!(opened.format.plugin.name(), "Second test plugin");
    assert!(plugins.tabs().is_empty());
    assert!(!plugins.wants(EventKind::Score));
    assert!(plugins.service("test-echo").is_err());
    let e = plugins
        .call_service("test-echo", "echo", b"hi")
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound);
    assert!(plugins.instruments().is_empty());
    assert_eq!(*heard.3.lock().unwrap(), ["oxiguitar.test"]);
    // The footer is told once, as an error, naming the plugin.
    let told = heard.0.lock().unwrap();
    let errors: Vec<&String> = told
        .iter()
        .filter(|(level, _)| *level == Level::Error)
        .map(|(_, text)| text)
        .collect();
    assert_eq!(errors.len(), 1, "{told:?}");
    assert!(
        errors[0].starts_with("Test plugin stopped working and isn't used until OxiGuitar restarts: it crashed (a panic: panicked on purpose, reading)"),
        "{}",
        errors[0]
    );

    // With no other plugin for the format, a file is refused as if the
    // plugin had never been there.
    let dir = folder("withdrawn-alone", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let e = plugins
        .open(b"OXGTEST\npanic\n", Some("oxgtest"))
        .unwrap_err();
    assert!(
        matches!(
            e,
            oxiguitar_host::OpenError::Unrecognised(oxiguitar_host::Unrecognised::NoReaders)
        ),
        "{e:?}"
    );
}

#[test]
fn a_service_whose_plugin_crashes_is_chosen_again_among_the_others() {
    let dir = folder("service-crash", &["test.oxp"]);
    let (plugins, heard) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    // A panic is caught in the plugin and reported; the plugin is withdrawn,
    // and with it every service of test-echo it provided.
    let e = plugins.call_service("test-echo", "panic", b"").unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotFound, "{e}");
    assert!(
        plugins.found()[0]
            .failure()
            .is_some_and(|why| why == "it crashed (a panic: panicked on purpose)"),
        "{:?}",
        plugins.found()[0]
    );
    assert!(plugins.service("test-echo").is_err());
    assert_eq!(
        heard
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(level, _)| *level == Level::Error)
            .count(),
        1
    );
}

/// The title the second test plugin gives every score.
fn oxiguitar_test_second_title() -> &'static str {
    "Read by the second plugin"
}

/// The test plugin's canvas size, from its source.
fn oxiguitar_test_plugin_canvas() -> (f32, f32) {
    (100.0, 50.0)
}

#[test]
fn events_are_pushed_in_order_and_the_plugin_asks_for_the_score() {
    let dir = folder("events", &["test.oxp"]);
    let heard = Arc::new(Heard(
        Mutex::default(),
        Mutex::default(),
        Some(Score {
            info: ScoreInfo {
                title: "Água de Beber".into(),
                ..ScoreInfo::default()
            },
            ..Score::default()
        }),
        Mutex::default(),
    ));
    let plugins = Plugins::discover(
        std::slice::from_ref(&dir),
        SharedSettings::default(),
        "x11",
        heard.clone(),
    );
    assert!(plugins.wants(EventKind::Score));
    assert!(plugins.wants(EventKind::Selection) && plugins.wants(EventKind::Playback));
    assert!(!plugins.found()[0].is_loaded());

    // The first event loads the plugin; a score being opened makes it ask
    // for its tab to be redrawn.
    plugins.push(Event::ScoreOpened { revision: 1 });
    plugins.wait_for_events();
    assert!(plugins.found()[0].is_loaded());
    assert_eq!(*heard.1.lock().unwrap(), [panel()]);

    // The test plugin is built for interface 1.2: it hears the editor's
    // selection, not 1.1's.
    plugins.push(Event::Selection(Some(Selection {
        track: 0,
        from: Position { bar: 1, beat: 0 },
        to: Position { bar: 2, beat: 3 },
    })));
    let selection = Event::Selection2(Some(Selection2::Beats {
        track: 0,
        staff: 0,
        voice: 1,
        from: Position { bar: 1, beat: 0 },
        to: Position { bar: 2, beat: 3 },
    }));
    plugins.push(selection.clone());
    plugins.push(Event::ScoreChanged { revision: 2 });
    plugins.push(Event::ScoreClosed);
    plugins.wait_for_events();
    let events = |plugins: &Plugins| -> Vec<String> {
        String::from_utf8(plugins.call_service("test-echo", "events", b"").unwrap())
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        events(&plugins),
        [
            format!("{:?}", Event::ScoreOpened { revision: 1 }),
            format!("{selection:?}"),
            format!("{:?}", Event::ScoreChanged { revision: 2 }),
            format!("{:?}", Event::ScoreClosed),
        ]
    );

    // Playback positions: never more than were pushed, in order, the last
    // one always delivered (a slow plugin gets the latest, not a backlog).
    let playing = |bar| {
        Event::Playback(Playback {
            playing: true,
            at: Position { bar, beat: 0 },
            seconds: f64::from(bar) * 2.0,
        })
    };
    for bar in 0..200 {
        plugins.push(playing(bar));
    }
    plugins.wait_for_events();
    let all = events(&plugins);
    let positions = &all[4..];
    assert!(!positions.is_empty() && positions.len() <= 200);
    assert_eq!(positions.last().unwrap(), &format!("{:?}", playing(199)));
    let bars: Vec<u32> = positions
        .iter()
        .map(|p| {
            (0..200)
                .find(|&b| *p == format!("{:?}", playing(b)))
                .unwrap()
        })
        .collect();
    assert!(bars.windows(2).all(|w| w[0] < w[1]), "{bars:?}");

    // The plugin asks for the open score when it needs it.
    let c = plugins
        .tab_event(&panel(), "score", TabEvent::Clicked)
        .unwrap();
    assert_eq!(label(&c, 3, 0), "Score: Água de Beber");
    assert!(heard.0.lock().unwrap().is_empty());
}

#[test]
fn disabled_plugins_hear_no_events_and_add_no_tabs() {
    let dir = folder("events-disabled", &["test.oxp"]);
    let disabled = SharedSettings::new(
        Settings::parse(r#"{"disabled": ["oxiguitar.test"]}"#).unwrap(),
        None,
    );
    let (plugins, _) = discover(std::slice::from_ref(&dir), disabled);
    assert!(plugins.tabs().is_empty());
    assert!(!plugins.wants(EventKind::Score));
    plugins.push(Event::ScoreOpened { revision: 1 });
    plugins.wait_for_events();
    assert!(!plugins.found()[0].is_loaded());
    assert_eq!(
        plugins.tab_content(&panel()).unwrap_err().kind,
        ErrorKind::NotFound
    );
}

/// The rest of the core for interface 1.2: a score at a revision, the
/// changes plugins hand back, and what they say.
#[derive(Default)]
struct Editor {
    score: Mutex<Option<(u64, oxiguitar_host::api::score2::Score2)>>,
    applied: Mutex<Vec<(String, u64, String, String)>>,
    said: Mutex<Vec<String>>,
}

impl Core for Editor {
    fn notify(&self, _level: Level, text: String) {
        self.said.lock().unwrap().push(text);
    }

    fn score2(&self) -> Option<(u64, oxiguitar_host::api::score2::Score2)> {
        self.score.lock().unwrap().clone()
    }

    fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: oxiguitar_host::api::score2::Score2,
    ) -> Result<(), String> {
        let now = self.score.lock().unwrap().as_ref().map_or(0, |(r, _)| *r);
        if revision != now {
            return Err(format!("the score has changed since revision {revision}"));
        }
        self.applied
            .lock()
            .unwrap()
            .push((plugin.to_owned(), revision, name, score.info.title));
        Ok(())
    }
}

#[test]
fn interface_1_2_tools_edits_the_score_v2_and_both_selections() {
    use oxiguitar_host::api::score2::{MasterBarExt, Score2};
    let dir = folder("one-two", &["test.oxp"]);
    std::fs::copy(second_oxp(), dir.join("second.oxp")).unwrap();
    let core = Arc::new(Editor::default());
    let plugins = Plugins::discover(
        std::slice::from_ref(&dir),
        SharedSettings::default(),
        "x11",
        core.clone(),
    );

    // The test plugin's tool, from its manifest; the second plugin (1.1)
    // has none.
    let tools = plugins.tools();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].spec.name, "Mark the Title");
    assert_eq!(plugins.run_tool(tools[0]).unwrap(), "No score is open.");

    // With a score open, the tool hands back the changed score, made from
    // the revision it was given.
    let mut score = Score2::default();
    score.info.title = "Song".into();
    *core.score.lock().unwrap() = Some((5, score.clone()));
    assert_eq!(
        plugins.run_tool(tools[0]).unwrap(),
        "The title is now Song!."
    );
    assert_eq!(
        *core.applied.lock().unwrap(),
        [(
            "Test plugin".to_owned(),
            5,
            "Mark the Title".to_owned(),
            "Song!".to_owned()
        )]
    );

    // A change the core refuses is an error for the plugin, not a crash.
    struct Stale(Arc<Editor>);
    impl Core for Stale {
        fn notify(&self, level: Level, text: String) {
            self.0.notify(level, text);
        }
        fn score2(&self) -> Option<(u64, Score2)> {
            self.0.score2()
        }
        fn apply(&self, plugin: &str, _: u64, name: String, score: Score2) -> Result<(), String> {
            self.0.apply(plugin, 999, name, score)
        }
    }
    let stale_dir = folder("one-two-stale", &["test.oxp"]);
    let stale = Plugins::discover(
        std::slice::from_ref(&stale_dir),
        SharedSettings::default(),
        "x11",
        Arc::new(Stale(core.clone())),
    );
    let e = stale.run_tool(stale.tools()[0]).unwrap_err();
    assert!(
        e.message
            .contains("the score has changed since revision 999"),
        "{e}"
    );
    assert!(
        stale.found()[0].failure().is_none(),
        "an error, not a failure"
    );

    // Writing: the score version 2 reaches a plugin built for 1.2. The test
    // plugin writes with a version-1 writer, so the SDK names free time as
    // lost.
    let format = plugins.writers()[0];
    let mut free = score.clone();
    free.master_bars
        .push(oxiguitar_host::api::score2::MasterBar2 {
            ext: vec![MasterBarExt::FreeTime],
            ..Score2::from_v1(Score {
                master_bars: vec![oxiguitar_host::api::score::MasterBar {
                    time_signature: oxiguitar_host::api::score::TimeSignature {
                        numerator: 4,
                        denominator: 4,
                    },
                    key_signature: oxiguitar_host::api::score::KeySignature {
                        fifths: 0,
                        mode: oxiguitar_host::api::score::KeyMode::Major,
                    },
                    repeat_start: false,
                    repeat_end: None,
                    alternate_endings: 0,
                    marker: None,
                    double_bar: false,
                    triplet_feel: oxiguitar_host::api::score::TripletFeel::None,
                    tempo_changes: vec![],
                    directions: vec![],
                }],
                ..Score::default()
            })
            .master_bars[0]
                .clone()
        });
    free.tracks
        .push(oxiguitar_host::api::score2::Track2::default());
    let written = plugins.write(format, &free).unwrap();
    assert!(
        written
            .losses
            .iter()
            .any(|l| l.message == "free time is left out by this plugin" && l.place == "bar 1"),
        "{:?}",
        written.losses
    );

    // The selection: the test plugin (1.2) hears the editor's, the second
    // plugin (1.1) the one it knows.
    plugins.push(Event::Selection(Some(Selection {
        track: 0,
        from: Position { bar: 0, beat: 1 },
        to: Position { bar: 0, beat: 1 },
    })));
    plugins.push(Event::Selection2(Some(Selection2::Caret {
        track: 0,
        staff: 0,
        voice: 2,
        bar: 0,
        beat: 1,
        string: 3,
    })));
    plugins.wait_for_events();
    let events =
        String::from_utf8(plugins.call_service("test-echo", "events", b"").unwrap()).unwrap();
    assert!(events.contains("Selection2(Some(Caret"), "{events}");
    assert!(!events.contains("Selection(Some"), "{events}");
    let said = core.said.lock().unwrap().clone();
    assert!(
        said.iter()
            .any(|s| s.contains("heard Selection(Some(Selection")),
        "{said:?}"
    );
    assert!(!said.iter().any(|s| s.contains("Selection2")), "{said:?}");
}

/// The rest of the core for interface 1.3: footer messages with their
/// links.
#[derive(Default)]
struct Linked(Mutex<Vec<Said>>);

/// A footer message, and its link's words and address if it has one.
type Said = (Level, String, Option<(String, String)>);

impl Core for Linked {
    fn notify(&self, level: Level, text: String) {
        self.0.lock().unwrap().push((level, text, None));
    }

    fn notify_with_link(
        &self,
        level: Level,
        text: String,
        link: oxiguitar_host::api::message::Link,
    ) {
        self.0
            .lock()
            .unwrap()
            .push((level, text, Some((link.text, link.url))));
    }
}

#[test]
fn interface_1_3_settings_windows_track_sections_links_and_folders() {
    use oxiguitar_host::api::manifest::SharedFolder;
    use oxiguitar_host::api::message::TrackInfo;
    use oxiguitar_host::{Folders, System};

    let dir = folder("one-three", &["test.oxp"]);
    let home = dir.join("home");
    let core = Arc::new(Linked::default());
    let plugins = Plugins::discover(
        std::slice::from_ref(&dir),
        SharedSettings::default(),
        "x11",
        core.clone(),
    );

    // The shared folders come from the manifest alone.
    assert_eq!(plugins.shared_folders(), [SharedFolder::Soundfonts]);
    assert!(!plugins.found()[0].is_loaded());

    // The settings window: its widgets change the plugin's own settings.
    let found = plugins.with_settings_window()[0];
    let content = plugins.settings_window(found).unwrap();
    assert!(matches!(
        &content.rows[0].widgets[0],
        Widget::Toggle { on: false, .. }
    ));
    plugins
        .settings_event(found, "greet", TabEvent::Toggled(true))
        .unwrap();
    let content = plugins
        .settings_event(found, "name", TabEvent::Edited("Mateus".into()))
        .unwrap();
    assert!(matches!(
        &content.rows[2].widgets[0],
        Widget::Label { text, .. } if text == "Hello, Mateus."
    ));
    assert_eq!(
        plugins.settings().get("oxiguitar.test"),
        Some(serde_json::json!({"greet": true, "name": "Mateus"}))
    );

    // The track section: every class but drums, and the settings travel
    // with every request; the plugin keeps none.
    assert!(plugins.track_sections(InstrumentClass::Drums).is_empty());
    let section = plugins.track_sections(InstrumentClass::Fretted)[0];
    assert_eq!(section.spec.title, "Test section");
    assert_eq!(section.plugin_id(), "oxiguitar.test");
    let track = TrackInfo {
        index: 1,
        name: "Lead".into(),
        class: InstrumentClass::Fretted,
    };
    let (content, settings) = plugins.track_section(section, &track, "{}").unwrap();
    assert_eq!(settings, "{}");
    assert!(matches!(
        &content.rows[1].widgets[0],
        Widget::Field { placeholder, .. } if placeholder == "A nickname for Lead"
    ));
    let (_, settings) = plugins
        .track_section_event(section, &track, &settings, "level", TabEvent::Slid(0.25))
        .unwrap();
    assert_eq!(settings, r#"{"level":0.25}"#);
    // A refused value: said in the footer, and the settings stay.
    let (_, kept) = plugins
        .track_section_event(
            section,
            &track,
            &settings,
            "nickname",
            TabEvent::Edited("refuse".into()),
        )
        .unwrap();
    assert_eq!(kept, settings);
    assert!(
        core.0
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .1
            .contains("that nickname is refused"),
        "{:?}",
        core.0.lock().unwrap()
    );

    // Footer links: https only.
    let tool = plugins
        .tools()
        .into_iter()
        .find(|t| t.spec.id == "link")
        .unwrap();
    assert_eq!(plugins.run_tool(tool).unwrap(), "");
    assert_eq!(
        core.0.lock().unwrap().last().unwrap(),
        &(
            Level::Warning,
            "Test plugin: A footer message with a link (a test).".to_owned(),
            Some((
                "The plugin guide".to_owned(),
                "https://mateusrdp.github.io/oxiguitar/plugin-guide/".to_owned()
            ))
        )
    );
    // Anything else is refused (the SDK drops a message that can't be
    // shown, so the footer hears nothing).
    let said = core.0.lock().unwrap().len();
    for refused in [
        "http://example.org",
        "file:///etc/passwd",
        "https://",
        "javascript:x",
    ] {
        plugins
            .call_service("test-echo", "link", refused.as_bytes())
            .unwrap();
        assert_eq!(core.0.lock().unwrap().len(), said, "{refused}");
    }
    assert!(oxiguitar_host::is_https("https://example.org/a?b=c"));

    // Folders: none until the host is given them; then the plugin's own,
    // made on first ask, and the shared ones.
    let e = plugins
        .call_service("test-echo", "folders", b"")
        .unwrap_err();
    assert!(e.message.contains("keeps no folders"), "{e}");
    let folders = Folders::resolve(
        System::this(),
        |name| (name == oxiguitar_host::HOME_VARIABLE).then(|| home.clone().into()),
        None,
    )
    .unwrap();
    plugins.use_folders(folders.clone());
    let answer = plugins.call_service("test-echo", "folders", b"").unwrap();
    let answer = String::from_utf8(answer).unwrap();
    let lines: Vec<&str> = answer.lines().collect();
    assert_eq!(
        lines,
        [
            folders.plugin_data("oxiguitar.test").to_str().unwrap(),
            folders.plugin_cache("oxiguitar.test").to_str().unwrap(),
            folders.shared(SharedFolder::Soundfonts).to_str().unwrap(),
        ]
    );
    assert_eq!(
        std::fs::read_to_string(folders.plugin_data("oxiguitar.test").join("hello.txt")).unwrap(),
        "hello\n"
    );
    assert!(folders.plugin_cache("oxiguitar.test").is_dir());
}

#[test]
fn the_real_time_checks_catch_allocations_system_calls_and_late_notes() {
    let dir = folder("realtime", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    // As it should be: every check passes.
    let fine = oxiguitar_plugin_test::check_realtime(&plugins, "{}");
    assert!(fine.ok(), "{:#?}", fine.failed);
    assert_eq!(fine.passed.len(), 3 * 4, "{:#?}", fine.passed);

    // Allocating and yielding its thread on every render: both are caught.
    let bad = oxiguitar_plugin_test::check_realtime(&plugins, r#"{"misbehave": true}"#);
    let failed = bad.failed.join("\n");
    assert!(
        failed.contains("allocates nothing while rendering"),
        "{failed}"
    );
    if cfg!(target_os = "linux") {
        assert!(
            failed.contains("makes no system call while rendering"),
            "{failed}"
        );
    }
    // A note 20 frames late, past a resolution of 0.
    let late = oxiguitar_plugin_test::check_realtime(&plugins, r#"{"late": 20}"#);
    let failed = late.failed.join("\n");
    assert!(
        failed.contains("started on frame 20, later than its resolution of 0"),
        "{failed}"
    );
}

#[test]
fn the_engine_plays_a_plugin_instrument_on_its_frames() {
    use oxiguitar_host::api::message::{Prepare, PreparedTrack, TrackInfo};
    use oxiguitar_play::plugin_source::PluginSource;
    use oxiguitar_play::{Route, Schedule, Slot, Sources, Timeline, engine};

    let dir = folder("engine", &["test.oxp"]);
    let (plugins, _) = discover(std::slice::from_ref(&dir), SharedSettings::default());
    let song =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp5");
    let score = oxiguitar_formats::read(&std::fs::read(song).unwrap())
        .unwrap()
        .score;
    let timeline = Timeline::new(&score);
    let tracks = score.tracks.len();
    let class = InstrumentClass::of(
        score.tracks[0].instrument.midi_program,
        score.tracks[0].instrument.percussion,
    );
    let instrument = plugins.playable(class)[0];
    let prepared: Vec<PreparedTrack> = (0..tracks)
        .map(|i| PreparedTrack {
            info: TrackInfo {
                index: i as u32,
                name: score.tracks[i].name.clone(),
                class,
            },
            program: score.tracks[i].instrument.midi_program,
            percussion: false,
            tuning: Vec::new(),
            capo: 0,
            settings: "{}".into(),
        })
        .collect();
    let instance = plugins
        .prepare(
            instrument,
            Prepare {
                instrument: String::new(),
                rate: 48_000,
                frames: 512,
                tracks: prepared,
            },
        )
        .unwrap();
    let routes: Vec<Option<Route>> = (0..tracks)
        .map(|t| Some(Route { source: 0, slot: t }))
        .collect();
    let schedule = Arc::new(Schedule::new(&timeline, &routes, 1));
    let (mut engine, mut control) = engine(48_000, 512);
    let mut sources = Sources::new(vec![Slot::new(
        Box::new(PluginSource::new(
            instance,
            (0..tracks).map(|_| Vec::new()).collect(),
        )),
        (0..tracks).collect(),
        512,
    )]);
    sources.fit(&schedule);
    control.set_schedule(schedule.clone());
    control.set_sources(sources);
    let first = timeline
        .notes
        .iter()
        .map(|n| n.start)
        .fold(f64::MAX, f64::min);
    // Play from a little before the first note; the device's callbacks
    // are 100 frames.
    let from = (first - 0.01).max(0.0);
    control.play(from, false);
    let mut out = vec![0.0f32; 200];
    let mut left: Vec<f32> = Vec::new();
    for _ in 0..48 {
        engine.render(&mut out, 2, 0);
        left.extend(out.iter().step_by(2));
    }
    let expected = ((first - from) * 48_000.0 - 1e-6).ceil() as usize;
    let heard = left.iter().position(|v| v.abs() > 1e-4).unwrap();
    assert_eq!(heard, expected, "the first note starts on its frame");
    assert!(left[..expected].iter().all(|v| *v == 0.0));
    control.pause();
    engine.render(&mut out, 2, 0);
    // The sources go back to the window, which drops them: the plugin
    // releases its instance.
    control.set_sources(Sources::new(Vec::new()));
    engine.render(&mut out, 2, 0);
    control.collect();
}
