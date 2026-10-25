// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A plugin for testing the plugin interface (never shipped).
//!
//! The contract suite (`oxiguitar-plugin-test`) builds it as `test.oxp`
//! and drives every part of the interface through it:
//!
//! - the format `oxgtest`: a text file starting `OXGTEST`, whose second
//!   line is the score's title; it reads and writes, and panics on the
//!   title `panic` (a failing plugin; `plugins/test-second` reads the same
//!   format, to take over);
//! - the service interface `test-echo`, with four services: `echo`
//!   (available, priority 1), `second` (available, priority 0, so chosen
//!   only when settings.json pins it), `unavailable` (its probe says no,
//!   though its priority is the highest that can run) and `elsewhere`
//!   (only on Plan 9, so never chosen);
//! - two instruments, one playing only drums and one everything but drums,
//!   both a sine per note (interface 1.3: [`sine`]), and the effect `gain`,
//!   which halves a track;
//! - two tabs (interface 1.1): `panel`, in the bottom area, with one widget
//!   of every kind and a canvas that marks where the pointer last went
//!   down, and `side`, in the right area, with one line of text. `panel`'s
//!   button `score` asks the host for the open score and shows its title;
//! - every kind of event (score, selection, playback): each is written down
//!   and shown in `panel`, and a score being opened asks the host to
//!   redraw `panel`. Built for interface 1.2, it hears the editor's
//!   selection (`Event::Selection2`);
//! - the tool `mark` (interface 1.2), "Mark the Title": it asks for the
//!   score (version 2), adds `!` to its title and hands it back as one
//!   change;
//! - interface 1.3: a settings window (a toggle and a field, kept in the
//!   plugin's settings), the track dialog's section `test` for every track
//!   but drums (a level slider and a name field, kept per track), the tool
//!   `link`, "Show a Footer Link" (a warning in the footer with a link to
//!   the plugin guide), and the shared folder `soundfonts`.
//!
//! `echo`'s methods exercise the host's side: `notify` and `settings` call
//! the host's services, `via-host` calls `test-echo` back through the host
//! (a plugin calling a service, here itself, while answering a call), and
//! `panic` and `fail` check that both cross the interface as errors.
//! `events` returns the events received so far, one per line, and `break`
//! makes `panel`'s contents unshowable from then on (an id used twice).
//! `link` shows a footer warning linking to the address it is given, and
//! `folders` writes `hello.txt` into the plugin's data folder and returns
//! the data, cache and `soundfonts` folders, one per line.

pub mod sine;

use std::sync::{Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin::display::{Anchor, Font, Item, Paint, Point};
use oxiguitar_plugin::score::{Score, ScoreInfo};
use oxiguitar_plugin::{
    Availability, Canvas, Error, ErrorKind, Event, FormatReader, FormatWriter, Host, Level,
    LogEntry, Plugin, PointerKind, Read, Row, Service, SharedFolder, Tab, TabContent, TabEvent,
    Tool, TrackInfo, TrackSection, Widget, Written,
};

use oxiguitar_plugin::serde_json;

/// The plugin guide, where the tool `link` points.
pub const GUIDE: &str = "https://mateusrdp.github.io/oxiguitar/plugin-guide/";

/// The first line of every `oxgtest` file.
pub const MAGIC: &str = "OXGTEST\n";

/// The test plugin.
#[derive(Debug)]
pub struct TestPlugin {
    host: Host,
    panel: Panel,
    side: Side,
    mark: Mark,
    link: LinkTool,
    settings: SettingsWindow,
}

/// The tool `link`.
#[derive(Debug)]
struct LinkTool {
    host: Host,
}

impl Tool for LinkTool {
    fn run(&self) -> Result<String, Error> {
        self.host.notify_with_link(
            Level::Warning,
            "A footer message with a link (a test).",
            "The plugin guide",
            GUIDE,
        );
        Ok(String::new())
    }
}

/// The settings window: a toggle and a field, kept in the plugin's
/// settings (`greet` and `name`).
#[derive(Debug)]
struct SettingsWindow {
    host: Host,
}

impl Tab for SettingsWindow {
    fn content(&self) -> TabContent {
        let settings = self.host.settings().unwrap_or_default();
        let greet = settings
            .get("greet")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let name = settings
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        let greeting = if greet && !name.is_empty() {
            format!("Hello, {name}.")
        } else {
            "Nothing to say.".to_owned()
        };
        TabContent {
            rows: vec![
                Row {
                    widgets: vec![Widget::Toggle {
                        id: "greet".into(),
                        text: "Greet".into(),
                        on: greet,
                        enabled: true,
                    }],
                },
                Row {
                    widgets: vec![Widget::Field {
                        id: "name".into(),
                        text: name,
                        placeholder: "Your name".into(),
                        enabled: true,
                    }],
                },
                Row {
                    widgets: vec![Widget::Label {
                        text: greeting,
                        muted: true,
                    }],
                },
            ],
        }
    }

    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let mut settings = self.host.settings()?;
        match (widget, event) {
            ("greet", TabEvent::Toggled(on)) => settings.insert("greet".into(), on.into()),
            ("name", TabEvent::Edited(text)) => settings.insert("name".into(), text.into()),
            _ => return Err(Error::not_found(format!("no widget {widget:?}"))),
        };
        self.host.set_settings(&settings)
    }
}

/// The track dialog's section `test`: a level (0 to 1, `level`) and a
/// nickname (`nickname`) per track.
#[derive(Debug)]
struct TestSection;

impl TrackSection for TestSection {
    fn content(
        &self,
        track: &TrackInfo,
        settings: &serde_json::Map<String, serde_json::Value>,
    ) -> TabContent {
        let level = settings
            .get("level")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let nickname = settings
            .get("nickname")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        TabContent {
            rows: vec![
                Row {
                    widgets: vec![
                        Widget::Label {
                            text: "Level".into(),
                            muted: false,
                        },
                        Widget::Slider {
                            id: "level".into(),
                            value: level as f32,
                            min: 0.0,
                            max: 1.0,
                            enabled: true,
                        },
                    ],
                },
                Row {
                    widgets: vec![Widget::Field {
                        id: "nickname".into(),
                        text: nickname.into(),
                        placeholder: format!("A nickname for {}", track.name),
                        enabled: true,
                    }],
                },
            ],
        }
    }

    fn on(
        &self,
        _track: &TrackInfo,
        settings: &mut serde_json::Map<String, serde_json::Value>,
        widget: &str,
        event: TabEvent,
    ) -> Result<(), Error> {
        match (widget, event) {
            ("level", TabEvent::Slid(v)) => {
                settings.insert("level".into(), f64::from(v).into());
            }
            ("nickname", TabEvent::Edited(text)) if text == "refuse" => {
                return Err(Error::new(ErrorKind::Invalid, "that nickname is refused"));
            }
            ("nickname", TabEvent::Edited(text)) => {
                settings.insert("nickname".into(), text.into());
            }
            _ => return Err(Error::not_found(format!("no widget {widget:?}"))),
        }
        Ok(())
    }
}

/// The service that is never available.
#[derive(Debug)]
struct Unavailable;

/// The tab `panel`: what its widgets hold, and the events received.
#[derive(Debug)]
struct Panel {
    host: Host,
    state: Mutex<PanelState>,
}

#[derive(Debug, Default)]
struct PanelState {
    clicks: u32,
    on: bool,
    level: f32,
    text: String,
    choice: u32,
    line: Option<u32>,
    /// The last pointer event on the canvas: what, and where.
    pointer: Option<(PointerKind, Point)>,
    events: Vec<String>,
    broken: bool,
    title: Option<String>,
}

/// The tab `side`.
#[derive(Debug)]
struct Side;

/// The tool `mark`.
#[derive(Debug)]
struct Mark {
    host: Host,
}

impl Tool for Mark {
    fn run(&self) -> Result<String, Error> {
        let Some((revision, mut score)) = self.host.score2()? else {
            return Ok("No score is open.".into());
        };
        score.info.title.push('!');
        let title = score.info.title.clone();
        self.host.apply(revision, "Mark the Title", score)?;
        Ok(format!("The title is now {title}."))
    }
}

/// The panel canvas's size, in its own units.
pub const CANVAS: (f32, f32) = (100.0, 50.0);

impl Plugin for TestPlugin {
    fn start(host: Host) -> Result<TestPlugin, Error> {
        Ok(TestPlugin {
            panel: Panel {
                host: host.clone(),
                state: Mutex::default(),
            },
            side: Side,
            mark: Mark { host: host.clone() },
            link: LinkTool { host: host.clone() },
            settings: SettingsWindow { host: host.clone() },
            host,
        })
    }

    fn tool(&self, id: &str) -> Option<&dyn Tool> {
        match id {
            "mark" => Some(&self.mark),
            "link" => Some(&self.link),
            _ => None,
        }
    }

    fn settings_window(&self) -> Option<&dyn Tab> {
        Some(&self.settings)
    }

    fn track_section(&self, id: &str) -> Option<&dyn TrackSection> {
        (id == "test").then_some(&TestSection as &dyn TrackSection)
    }

    fn instrument(&self, id: &str) -> Option<&dyn oxiguitar_plugin::Instrument> {
        matches!(id, "drums" | "no-drums")
            .then_some(&sine::Sine as &dyn oxiguitar_plugin::Instrument)
    }

    fn effect(&self, id: &str) -> Option<&dyn oxiguitar_plugin::Effect> {
        (id == "gain").then_some(&sine::Gain as &dyn oxiguitar_plugin::Effect)
    }

    fn tab(&self, id: &str) -> Option<&dyn Tab> {
        match id {
            "panel" => Some(&self.panel),
            "side" => Some(&self.side),
            _ => None,
        }
    }

    fn event(&self, event: &Event) {
        self.panel.state().events.push(format!("{event:?}"));
        if matches!(event, Event::ScoreOpened { .. }) {
            self.host.redraw("panel");
        }
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == "oxgtest").then_some(self as &dyn FormatReader)
    }

    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        (format == "oxgtest").then_some(self as &dyn FormatWriter)
    }

    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        match (interface, id) {
            ("test-echo", "echo" | "second" | "elsewhere") => Some(self),
            ("test-echo", "unavailable") => Some(&Unavailable),
            _ => None,
        }
    }
}

impl FormatReader for TestPlugin {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        let rest = data.strip_prefix(MAGIC.as_bytes()).ok_or_else(|| Error {
            offset: Some(0),
            ..Error::new(ErrorKind::Invalid, "not an oxgtest file")
        })?;
        let text = std::str::from_utf8(rest).map_err(|e| Error {
            offset: Some((MAGIC.len() + e.valid_up_to()) as u64),
            ..Error::new(ErrorKind::Invalid, "the title is not UTF-8 text")
        })?;
        let title = text.lines().next().ok_or_else(|| Error {
            offset: Some(MAGIC.len() as u64),
            ..Error::new(ErrorKind::UnexpectedEnd, "the file ends before the title")
        })?;
        // A crash on purpose: the contract tests check that a plugin that
        // fails is withdrawn and the file goes to the next plugin.
        if title == "panic" {
            panic!("panicked on purpose, reading");
        }
        let score = Score {
            info: ScoreInfo {
                title: title.to_owned(),
                ..ScoreInfo::default()
            },
            initial_tempo: 120.0,
            ..Score::default()
        };
        let extra = text.lines().count().saturating_sub(1);
        let log = (extra > 0)
            .then(|| LogEntry {
                level: Level::Info,
                message: "lines after the title are ignored".into(),
                place: "line 3".into(),
                count: extra as u32,
            })
            .into_iter()
            .collect();
        Ok(Read { score, log })
    }
}

impl FormatWriter for TestPlugin {
    fn write(&self, score: &Score) -> Result<Written, Error> {
        let data = format!("{MAGIC}{}\n", score.info.title).into_bytes();
        let losses = (!score.tracks.is_empty())
            .then(|| LogEntry {
                level: Level::Warning,
                message: "the oxgtest format holds only the title".into(),
                place: String::new(),
                count: score.tracks.len() as u32,
            })
            .into_iter()
            .collect();
        Ok(Written { data, losses })
    }
}

impl Service for TestPlugin {
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        match method {
            "echo" => Ok(input.to_vec()),
            "notify" => {
                self.host
                    .notify(Level::Info, String::from_utf8_lossy(input).into_owned());
                Ok(Vec::new())
            }
            "settings" => {
                let mut settings = self.host.settings()?;
                let count = settings.get("count").and_then(|c| c.as_u64()).unwrap_or(0) + 1;
                settings.insert("count".into(), count.into());
                self.host.set_settings(&settings)?;
                Ok(count.to_string().into_bytes())
            }
            "via-host" => self.host.call_service("test-echo", "echo", input),
            "fail" => Err(Error {
                offset: Some(7),
                context: "the test".into(),
                ..Error::new(ErrorKind::Invalid, "failed on purpose")
            }),
            "panic" => panic!("panicked on purpose"),
            "events" => Ok(self.panel.state().events.join("\n").into_bytes()),
            "break" => {
                self.panel.state().broken = true;
                Ok(Vec::new())
            }
            "link" => {
                let url = String::from_utf8_lossy(input).into_owned();
                self.host
                    .notify_with_link(Level::Warning, "A link.", "Open it", url);
                Ok(Vec::new())
            }
            "folders" => {
                let folders = self.host.folders()?;
                std::fs::write(folders.data.join("hello.txt"), "hello\n")
                    .map_err(|e| Error::new(ErrorKind::Failed, e.to_string()))?;
                let soundfonts = folders
                    .shared(SharedFolder::Soundfonts)
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                Ok(format!(
                    "{}\n{}\n{soundfonts}",
                    folders.data.display(),
                    folders.cache.display()
                )
                .into_bytes())
            }
            _ => Err(Error::not_found(format!("no method {method:?}"))),
        }
    }
}

impl Service for Unavailable {
    fn probe(&self) -> Availability {
        Availability::Unavailable("it is never available (a test)".into())
    }

    fn call(&self, _method: &str, _input: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::new(ErrorKind::Failed, "it is never available"))
    }
}

impl Panel {
    fn state(&self) -> MutexGuard<'_, PanelState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Tab for Panel {
    fn content(&self) -> TabContent {
        let s = self.state();
        if s.broken {
            let button = Widget::Button {
                id: "same".into(),
                text: "Same".into(),
                enabled: true,
            };
            return TabContent {
                rows: vec![Row {
                    widgets: vec![button.clone(), button],
                }],
            };
        }
        let (w, h) = CANVAS;
        let mut items = vec![Item::Rect {
            at: Point::new(0.0, 0.0),
            width: w,
            height: h,
            paint: Paint::Muted,
        }];
        if let Some((kind, at)) = s.pointer {
            items.push(Item::Rect {
                at: Point::new(at.x - 2.0, at.y - 2.0),
                width: 4.0,
                height: 4.0,
                paint: Paint::Accent,
            });
            items.push(Item::Text {
                text: format!("{kind:?}"),
                at: Point::new(w / 2.0, h - 4.0),
                size: 10.0,
                font: Font::Sans,
                anchor: Anchor::Middle,
                paint: Paint::Ink,
            });
        }
        let label = |text: String| Widget::Label { text, muted: false };
        let button = |id: &str, text: &str| Widget::Button {
            id: id.into(),
            text: text.into(),
            enabled: true,
        };
        TabContent {
            rows: vec![
                Row {
                    widgets: vec![
                        label(format!("Clicks: {}", s.clicks)),
                        button("click", "Click"),
                        button("score", "Score"),
                    ],
                },
                Row {
                    widgets: vec![
                        Widget::Toggle {
                            id: "toggle".into(),
                            text: "On".into(),
                            on: s.on,
                            enabled: true,
                        },
                        Widget::Slider {
                            id: "slider".into(),
                            value: s.level,
                            min: 0.0,
                            max: 10.0,
                            enabled: true,
                        },
                        Widget::Field {
                            id: "field".into(),
                            text: s.text.clone(),
                            placeholder: "Type here".into(),
                            enabled: true,
                        },
                        Widget::Choice {
                            id: "choice".into(),
                            options: vec!["One".into(), "Two".into(), "Three".into()],
                            selected: s.choice,
                            enabled: true,
                        },
                        Widget::List {
                            id: "list".into(),
                            items: vec!["First".into(), "Second".into()],
                            selected: s.line,
                        },
                    ],
                },
                Row {
                    widgets: vec![Widget::Canvas(Canvas {
                        id: "canvas".into(),
                        width: w,
                        height: h,
                        items,
                    })],
                },
                Row {
                    widgets: vec![
                        Widget::Label {
                            text: format!(
                                "Score: {}",
                                s.title.as_deref().unwrap_or("not asked for")
                            ),
                            muted: true,
                        },
                        label(format!("Events: {}", s.events.len())),
                    ],
                },
            ],
        }
    }

    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let mut s = self.state();
        match (widget, event) {
            ("click", TabEvent::Clicked) => s.clicks += 1,
            ("score", TabEvent::Clicked) => {
                // Not with the state locked: the host may take a while.
                drop(s);
                let title = match self.host.score()? {
                    Some(score) => score.info.title,
                    None => "none open".into(),
                };
                self.state().title = Some(title);
            }
            ("toggle", TabEvent::Toggled(on)) => s.on = on,
            ("slider", TabEvent::Slid(v)) => s.level = v.clamp(0.0, 10.0),
            ("field", TabEvent::Edited(text)) => s.text = text,
            ("choice", TabEvent::Chosen(i)) if i < 3 => s.choice = i,
            ("list", TabEvent::Chosen(i)) if i < 2 => s.line = Some(i),
            ("canvas", TabEvent::Pointer(p)) => s.pointer = Some((p.kind, p.at)),
            (widget, event) => {
                return Err(Error::new(
                    ErrorKind::Invalid,
                    format!("the widget {widget:?} doesn't take {event:?}"),
                ));
            }
        }
        Ok(())
    }
}

impl Tab for Side {
    fn content(&self) -> TabContent {
        TabContent {
            rows: vec![Row {
                widgets: vec![Widget::Label {
                    text: "The test plugin's other tab.".into(),
                    muted: false,
                }],
            }],
        }
    }
}

oxiguitar_plugin::export_plugin!(TestPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_its_format() {
        let plugin = TestPlugin::start(Host::detached()).unwrap();
        let read = plugin.read(b"OXGTEST\nHello\nmore\n").unwrap();
        assert_eq!(read.score.info.title, "Hello");
        assert_eq!(read.log.len(), 1);
        let written = plugin.write(&read.score).unwrap();
        assert_eq!(written.data, b"OXGTEST\nHello\n");
        assert!(written.losses.is_empty());
        let e = plugin.read(b"GP5").unwrap_err();
        assert_eq!((e.kind, e.offset), (ErrorKind::Invalid, Some(0)));
        let e = plugin.read(b"OXGTEST\n").unwrap_err();
        assert_eq!(e.kind, ErrorKind::UnexpectedEnd);
    }

    #[test]
    fn the_manifest_matches_the_code() {
        let problems = oxiguitar_plugin::check::<TestPlugin>(
            include_str!("../manifest.json"),
            env!("CARGO_PKG_VERSION"),
        );
        assert_eq!(problems, Vec::<String>::new());
    }

    /// `fixtures/plugin/panel.answer`, the fuzz target `plugin_tab`'s seed:
    /// the panel's contents after a pointer went down, as the plugin
    /// answers with them. `OXIGUITAR_BLESS=1` rewrites it.
    #[test]
    fn the_fuzzing_seed_is_the_panel_s_answer() {
        use oxiguitar_plugin::api::message::{Answer, Request};
        let plugin = TestPlugin::start(Host::detached()).unwrap();
        let pointer = TabEvent::Pointer(oxiguitar_plugin::Pointer {
            kind: PointerKind::Down,
            at: Point::new(30.0, 20.0),
            button: oxiguitar_plugin::PointerButton::Primary,
        });
        let request = Request::TabEvent {
            tab: "panel".into(),
            widget: "canvas".into(),
            event: pointer,
        };
        let answer = oxiguitar_plugin::dispatch(&plugin, request).unwrap();
        assert!(matches!(&answer, Answer::Tab(_)));
        let bytes = oxiguitar_plugin::api::encode(&answer).unwrap();
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/plugin/panel.answer");
        if std::env::var_os("OXIGUITAR_BLESS").is_some() {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, &bytes).unwrap();
        }
        assert_eq!(
            std::fs::read(&file).expect("run with OXIGUITAR_BLESS=1 to write it"),
            bytes,
            "the panel changed: run with OXIGUITAR_BLESS=1"
        );
    }

    #[test]
    fn the_panel_follows_its_widgets_and_breaks_on_purpose() {
        let plugin = TestPlugin::start(Host::detached()).unwrap();
        let panel = plugin.tab("panel").unwrap();
        panel.on("click", TabEvent::Clicked).unwrap();
        panel.on("score", TabEvent::Clicked).unwrap();
        let content = panel.content();
        assert!(content.problems().is_empty());
        assert!(
            matches!(&content.rows[0].widgets[0], Widget::Label { text, .. } if text == "Clicks: 1")
        );
        assert!(
            matches!(&content.rows[3].widgets[0], Widget::Label { text, .. } if text == "Score: none open")
        );
        assert!(panel.on("click", TabEvent::Toggled(true)).is_err());
        plugin.call("break", b"").unwrap();
        assert!(panel.content().problems()[0].contains("used twice"));
    }
}
