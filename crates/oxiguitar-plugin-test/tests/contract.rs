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
use oxiguitar_host::api::event::{Event, EventKind, Playback, Position, Selection};
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
/// asked to be redrawn, and a score that is open.
#[derive(Default)]
struct Heard(
    Mutex<Vec<(Level, String)>>,
    Mutex<Vec<TabId>>,
    Option<Score>,
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
    // of event.
    assert_eq!(report.passed.len(), 11 + 2 + 10 + 3, "{:#?}", report.passed);
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

    // The plugin's own refusal, in its words; a tab nobody has.
    let e = plugins
        .tab_event(&panel(), "click", TabEvent::Toggled(true))
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(e.message.contains("doesn't take"), "{e}");
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

    // Contents the core can't show are refused, saying why; the plugin and
    // its other tab carry on.
    plugins.call_service("test-echo", "break", b"").unwrap();
    let e = plugins.tab_content(&panel()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(
        e.message
            .starts_with("Test plugin: the tab Test can't be shown (")
            && e.message.contains("used twice"),
        "{e}"
    );
    assert!(plugins.tab_content(&side).is_ok());
    assert!(heard.0.lock().unwrap().is_empty());
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

    let selection = Event::Selection(Some(Selection {
        track: 0,
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
