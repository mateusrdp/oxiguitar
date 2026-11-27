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
//!   which scales a track by its section's level (half by default) and
//!   follows a new level while it runs (interface 1.5);
//! - its own UI, `ui/test.slint` (interface 2.0): the window `window`,
//!   the panels `slint-panel` and `panel` (the events it heard, and the
//!   open score's title when its button asks the host for it), the track
//!   sections `test` and `slint-section` (a level and a nickname per
//!   track), the window `settings` and the exporter `raw`'s options;
//! - every kind of event (score, selection, playback): each is written down
//!   and shown in `panel`. It hears the editor's selection
//!   (`Event::Selection2`);
//! - the tool `mark` (interface 1.2), "Mark the Title": it asks for the
//!   score (version 2), adds `!` to its title and hands it back as one
//!   change;
//! - interface 1.3: the track sections, for every track but drums, the tool
//!   `link`, "Show a Footer Link" (a warning in the footer with a link to
//!   the plugin guide), and the shared folder `soundfonts`;
//! - the audio device `fake` (interface 1.3, [`fake`]): an output with no
//!   speaker, priority -100, so a real device always wins;
//! - interface 1.9: the tool `settings`, "Test Settings…", which doesn't
//!   edit the score and opens the window `settings` (a toggle and a field,
//!   kept in the plugin's settings with OK, forgotten with Cancel).
//! - interface 1.14: `mark` has
//!   an options schema (`suffix`, what to add to the title), so other
//!   plugins can run it as told; and the service methods `tools`,
//!   `run-tool`, `measure`, `copy` and `select` call the host's new
//!   requests.
//!
//! `echo`'s methods exercise the host's side: `notify` and `settings` call
//! the host's services, `via-host` calls `test-echo` back through the host
//! (a plugin calling a service, here itself, while answering a call), and
//! `panic` and `fail` check that both cross the interface as errors.
//! `events` returns the events received so far, one per line.
//! `link` shows a footer warning linking to the address it is given, and
//! `folders` writes `hello.txt` into the plugin's data folder and returns
//! the data, cache and `soundfonts` folders, one per line.

pub mod fake;
pub mod sine;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin::display::{Anchor, Font, Item, Paint, Point};
use oxiguitar_plugin::score::{Score, ScoreInfo};
use oxiguitar_plugin::{
    Availability, Error, ErrorKind, Event, FormatReader, FormatWriter, Host, Level, LogEntry,
    Plugin, PluginFileState, Read, Selection2, Service, Tool, Written,
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
    mark: Mark,
    link: LinkTool,
    stamp: Stamp,
    ask_tool: AskTool,
    settings_tool: SettingsTool,
    fake: fake::Fake,
    raw: Raw,
    drawer: StampDrawer,
    performer: StampPerformer,
    ui: Arc<TestUi>,
}

/// A JSON object, as the plugin keeps its settings and a track's.
type Object = serde_json::Map<String, serde_json::Value>;

/// The test plugin's own UI (`ui/test.slint`): what it was told, the
/// events it heard (shown in the panel `panel`), and the drafts of the
/// window `settings` and of the exporter `raw`'s options, kept with OK and
/// forgotten with Cancel.
#[derive(Debug)]
pub struct TestUi {
    host: Host,
    heard: Mutex<Vec<String>>,
    events: Mutex<Vec<String>>,
    settings: Mutex<Option<Object>>,
    raw: Mutex<Option<Object>>,
    /// Each track section's track settings, as shown.
    sections: Mutex<std::collections::HashMap<String, Object>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl TestUi {
    fn new(host: Host) -> TestUi {
        TestUi {
            host,
            heard: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            settings: Mutex::new(None),
            raw: Mutex::new(None),
            sections: Mutex::default(),
        }
    }

    /// A draft, or what is saved when there is none yet.
    fn draft(&self, draft: &Mutex<Option<Object>>) -> Object {
        lock(draft)
            .clone()
            .unwrap_or_else(|| self.host.settings().unwrap_or_default())
    }

    /// What the window `settings` shows.
    fn settings_properties(&self) -> Vec<(String, oxiguitar_plugin::Value)> {
        use oxiguitar_plugin::Value;
        let settings = self.draft(&self.settings);
        let greet = settings
            .get("greet")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let name = settings
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let greeting = if greet && !name.is_empty() {
            format!("Hello, {name}.")
        } else {
            "Nothing to say.".to_owned()
        };
        vec![
            ("greet".into(), Value::Bool(greet)),
            ("name".into(), Value::String(name)),
            ("greeting".into(), Value::String(greeting)),
        ]
    }

    /// What the exporter `raw`'s options show.
    fn raw_properties(&self) -> Vec<(String, oxiguitar_plugin::Value)> {
        use oxiguitar_plugin::Value;
        let rate = raw_rate(&self.draft(&self.raw));
        vec![
            (
                "rates".into(),
                Value::List(
                    RAW_RATES
                        .iter()
                        .map(|r| Value::String(format!("{r} Hz")))
                        .collect(),
                ),
            ),
            (
                "rate".into(),
                Value::Number(RAW_RATES.iter().position(|r| *r == rate).unwrap_or(0) as f64),
            ),
        ]
    }

    /// What the panel `panel` shows of the events.
    fn show_events(&self) -> Result<(), Error> {
        let events = lock(&self.events).join("\n");
        self.host.set_view(
            "panel",
            vec![("events".into(), oxiguitar_plugin::Value::String(events))],
        )
    }

    /// Opens the window `settings` on what is saved.
    fn open_settings(&self) -> Result<(), Error> {
        *lock(&self.settings) = None;
        self.host.set_view("settings", self.settings_properties())?;
        self.host.show_view("settings")
    }

    /// Keeps a draft in the plugin's settings with OK, forgets it otherwise.
    fn finish(&self, draft: &Mutex<Option<Object>>, ok: bool) -> Result<(), Error> {
        match lock(draft).take() {
            Some(settings) if ok => self.host.set_settings(&settings),
            _ => Ok(()),
        }
    }
}

impl oxiguitar_plugin::Ui for TestUi {
    fn shown(&self, view: &str, track: Option<&oxiguitar_plugin::TrackView>) -> Result<(), Error> {
        use oxiguitar_plugin::Value;
        let told = track.map_or(String::new(), |t| {
            format!(" track {} {}", t.track.index, t.settings)
        });
        lock(&self.heard).push(format!("shown {view}{told}"));
        match view {
            "settings" => self.host.set_view(view, self.settings_properties()),
            "raw-options" => {
                *lock(&self.raw) = None;
                self.host.set_view(view, self.raw_properties())
            }
            "panel" => self.show_events(),
            "test" | "slint-section" => {
                let Some(t) = track else { return Ok(()) };
                let settings: Object = serde_json::from_str(&t.settings).unwrap_or_default();
                let number =
                    |k: &str, d: f64| settings.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
                let text = |k: &str| {
                    settings
                        .get(k)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_owned()
                };
                let properties = vec![
                    ("level".into(), Value::Number(number("level", 0.5))),
                    ("nickname".into(), Value::String(text("nickname"))),
                    (
                        "placeholder".into(),
                        Value::String(format!("A nickname for {}", t.track.name)),
                    ),
                ];
                lock(&self.sections).insert(view.to_owned(), settings);
                self.host.set_view(view, properties)
            }
            _ => Ok(()),
        }
    }

    fn callback(
        &self,
        view: &str,
        callback: &str,
        args: Vec<oxiguitar_plugin::Value>,
    ) -> Result<(), Error> {
        use oxiguitar_plugin::Value;
        lock(&self.heard).push(format!("{view} {callback} {args:?}"));
        match (view, callback, args.first()) {
            // A section's level and nickname go into the track's settings;
            // the nickname "refuse" is refused.
            ("test" | "slint-section", "level-changed" | "nickname-edited", Some(value)) => {
                let (key, value) = match value {
                    Value::Number(level) => ("level", serde_json::Value::from(*level)),
                    Value::String(t) if t == "refuse" => {
                        return Err(Error::new(ErrorKind::Invalid, "that nickname is refused"));
                    }
                    Value::String(t) => ("nickname", serde_json::Value::from(t.clone())),
                    _ => return Ok(()),
                };
                let settings = {
                    let mut sections = lock(&self.sections);
                    let settings = sections.entry(view.to_owned()).or_default();
                    settings.insert(key.into(), value);
                    settings.clone()
                };
                self.host.set_track_settings(view, &settings)
            }
            ("panel", "ask-score", _) => {
                let score = match self.host.score()? {
                    Some(score) => format!("Score: {}", score.info.title),
                    None => "Score: none open".to_owned(),
                };
                self.host
                    .set_view(view, vec![("score".into(), Value::String(score))])
            }
            ("settings", "ok" | "cancel", _) => {
                self.finish(&self.settings, callback == "ok")?;
                self.host.close_view(view)
            }
            ("settings", "greet-toggled" | "name-edited", Some(value)) => {
                let mut settings = self.draft(&self.settings);
                match value {
                    Value::Bool(on) => settings.insert("greet".into(), (*on).into()),
                    Value::String(t) => settings.insert("name".into(), t.clone().into()),
                    _ => return Ok(()),
                };
                *lock(&self.settings) = Some(settings);
                self.host.set_view(view, self.settings_properties())
            }
            ("raw-options", "rate-chosen", Some(Value::Number(i))) => {
                let rate = RAW_RATES
                    .get(*i as usize)
                    .ok_or_else(|| Error::new(ErrorKind::Failed, format!("no rate {i}")))?;
                let mut settings = self.draft(&self.raw);
                settings.insert("raw-rate".into(), (*rate).into());
                *lock(&self.raw) = Some(settings);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn closed(&self, view: &str, accepted: bool) -> Result<(), Error> {
        lock(&self.heard).push(format!("closed {view} {accepted}"));
        match view {
            "settings" => self.finish(&self.settings, false),
            "raw-options" => self.finish(&self.raw, accepted),
            "window" => {
                self.host.notify(
                    Level::Warning,
                    format!(
                        "The test window was closed{}.",
                        if accepted { " with OK" } else { "" }
                    ),
                );
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The first bytes of a file the exporter `raw` writes: then the rate
/// (u32), the frames (u64, written over at the end) and the samples (f32),
/// all little-endian.
pub const RAW_MAGIC: &[u8; 8] = b"OXGRAW1\n";

/// The rate the exporter `raw` asks for: not the player's usual 48 kHz,
/// so tests see the host render at the rate asked.
pub const RAW_RATE: u32 = 44_100;

/// The rates the exporter `raw` offers in its options (interface 1.11),
/// kept in the plugin's settings as `raw-rate`; the first is the default.
pub const RAW_RATES: [u32; 2] = [RAW_RATE, 22_050];

/// The rate the plugin's settings name for `raw`, or the default.
fn raw_rate(settings: &serde_json::Map<String, serde_json::Value>) -> u32 {
    settings
        .get("raw-rate")
        .and_then(serde_json::Value::as_u64)
        .and_then(|r| RAW_RATES.into_iter().find(|x| u64::from(*x) == r))
        .unwrap_or(RAW_RATE)
}

/// The exporter `raw` (interface 1.4): the samples as they come, after a
/// header whose frame count is written over when the export finishes. Its
/// rate is the one its options saved (interface 1.11).
#[derive(Debug)]
struct Raw {
    host: Host,
    /// The rate and the frames of the export under way, or none.
    frames: Mutex<Option<(u32, u64)>>,
}

impl Raw {
    fn header(rate: u32, frames: u64) -> Vec<u8> {
        let mut h = RAW_MAGIC.to_vec();
        h.extend_from_slice(&rate.to_le_bytes());
        h.extend_from_slice(&frames.to_le_bytes());
        h
    }
}

impl oxiguitar_plugin::AudioExporter for Raw {
    fn start(&self) -> Result<oxiguitar_plugin::ExportStart, Error> {
        let rate = raw_rate(&self.host.settings().unwrap_or_default());
        *self.frames.lock().unwrap_or_else(|e| e.into_inner()) = Some((rate, 0));
        Ok(oxiguitar_plugin::ExportStart {
            rate,
            data: Raw::header(rate, 0),
        })
    }

    fn samples(&self, interleaved: &[f32]) -> Result<Vec<u8>, Error> {
        let mut frames = self.frames.lock().unwrap_or_else(|e| e.into_inner());
        let Some((_, n)) = frames.as_mut() else {
            return Err(Error::new(ErrorKind::Failed, "no export was started"));
        };
        *n += (interleaved.len() / 2) as u64;
        Ok(interleaved.iter().flat_map(|v| v.to_le_bytes()).collect())
    }

    fn finish(&self, cancelled: bool) -> Result<oxiguitar_plugin::ExportEnd, Error> {
        let (rate, frames) = self
            .frames
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Failed, "no export was started"))?;
        Ok(oxiguitar_plugin::ExportEnd {
            data: Vec::new(),
            start: (!cancelled).then(|| Raw::header(rate, frames)),
        })
    }
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

/// The tool `settings` (interface 1.9): it doesn't edit the score, so it
/// runs with no score open and while the song plays, and opens the window
/// `settings` with what is saved.
#[derive(Debug)]
struct SettingsTool {
    ui: Arc<TestUi>,
}

impl Tool for SettingsTool {
    fn run(&self) -> Result<String, Error> {
        self.ui.open_settings()?;
        Ok(String::new())
    }
}

/// The service that is never available.
#[derive(Debug)]
struct Unavailable;

/// The tool `mark`.
#[derive(Debug)]
struct Mark {
    host: Host,
}

impl Mark {
    fn suffix(&self, suffix: &str) -> Result<String, Error> {
        let Some((revision, mut score)) = self.host.score2()? else {
            return Ok("No score is open.".into());
        };
        score.info.title.push_str(suffix);
        let title = score.info.title.clone();
        self.host.apply(revision, "Mark the Title", score)?;
        Ok(format!("The title is now {title}."))
    }
}

impl Tool for Mark {
    fn run(&self) -> Result<String, Error> {
        self.suffix("!")
    }

    /// As told (interface 1.14): `{"suffix": "?"}` adds that instead.
    fn run_with(&self, options: &serde_json::Value) -> Result<String, Error> {
        let suffix = options
            .get("suffix")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::new(ErrorKind::Invalid, "the option suffix is text"))?;
        self.suffix(suffix)
    }
}

/// This plugin's id, under which it keeps its data on beats.
pub const ID: &str = "oxiguitar.test";

/// What the tool `stamp` keeps on a beat (interface 1.8).
pub const STAMP: &str = r#"{"stamped":true}"#;

/// The mark the tool `stamp` has the core draw at that beat.
pub const MARK: &str = "T!";

/// The tool `stamp`, a palette button and an entry of a menu of its own
/// (interface 1.8): keeps the plugin's data on the first beat with notes
/// of the first track, as one change. The effect `gain` is silent while
/// that beat plays.
#[derive(Debug)]
struct Stamp {
    host: Host,
}

impl Tool for Stamp {
    fn run(&self) -> Result<String, Error> {
        use oxiguitar_plugin::score2::BeatExt;
        let Some((revision, mut score)) = self.host.score2()? else {
            return Ok("No score is open.".into());
        };
        let beat = score
            .tracks
            .iter_mut()
            .flat_map(|t| &mut t.staves)
            .flat_map(|s| &mut s.bars)
            .flat_map(|b| &mut b.voices)
            .flat_map(|v| &mut v.beats)
            .find(|b| !b.notes.is_empty());
        let Some(beat) = beat else {
            return Ok("The score has no notes.".into());
        };
        beat.ext.retain(|e| {
            !matches!(e, BeatExt::Plugin { plugin, .. } | BeatExt::Mark { plugin, .. } if plugin == ID)
        });
        beat.ext.push(BeatExt::Plugin {
            plugin: ID.into(),
            data: STAMP.into(),
        });
        // Drawn on the page at the beat.
        beat.ext.push(BeatExt::Mark {
            plugin: ID.into(),
            text: MARK.into(),
        });
        // What the page shows now, read before it changes.
        if let Some(engraving) = self.host.engraving()? {
            self.host.notify(
                Level::Info,
                format!(
                    "Engraved: {} pages, {} beats.",
                    engraving.pages.len(),
                    engraving.beats.len()
                ),
            );
        }
        // Where the caret is, asked for rather than heard of.
        if let Some(Selection2::Caret { bar, beat, .. }) = self.host.selection()? {
            self.host.notify(
                Level::Info,
                format!("The caret is at bar {}, beat {}.", bar + 1, beat + 1),
            );
        }
        self.host.apply(revision, "Stamp the First Beat", score)?;
        Ok(String::new())
    }
}

/// The tool `ask`, a palette button with a symbol and an entry of the
/// page's right-click menu: brings the panel `panel` to the front and
/// opens the window `window`.
#[derive(Debug)]
struct AskTool {
    host: Host,
}

impl Tool for AskTool {
    fn run(&self) -> Result<String, Error> {
        self.host.show_view("panel")?;
        self.host.show_view("window")?;
        Ok(String::new())
    }
}

/// Draws a stamp over each stamped beat (interface 1.8): a box with an S in
/// it, 2.4 by 1.6 staff spaces, and from it a dashed run of one second of
/// the music, with a hook, where the beat is played (interface 1.13).
/// Counts the requests, for the contract suite's test that the same places
/// aren't asked about twice.
#[derive(Debug, Default)]
struct StampDrawer {
    asked: std::sync::atomic::AtomicU32,
}

/// Plays the notes of stamped beats an octave up (interface 1.8), as a
/// plugin that plays the notes its own way would; others as they are.
/// Plays a stamped beat's notes an octave up (interface 1.8), and detunes
/// every fretted note by the track's `detune` setting plus the plugin's own
/// `perform-detune`, in cents (interface 1.12: the strings, the track's
/// settings, and the cache dropped when the plugin's settings change).
#[derive(Debug)]
struct StampPerformer {
    host: Host,
    asked: std::sync::atomic::AtomicU32,
}

/// A whole number of cents under `key` in a JSON object's text, or 0.
fn cents_in(json: &str, key: &str) -> i32 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get(key).and_then(serde_json::Value::as_i64))
        .and_then(|c| i32::try_from(c).ok())
        .unwrap_or(0)
}

/// Whether `n` is stopped at a fret (not an open string), by its staff's
/// strings.
fn fretted(track: &oxiguitar_plugin::Perform, n: &oxiguitar_plugin::PlayedNote) -> bool {
    let Some(staff) = track.staves.get(n.staff as usize) else {
        return false;
    };
    let Some(open) = (n.string as usize)
        .checked_sub(1)
        .and_then(|s| staff.tuning.get(s))
    else {
        return false;
    };
    n.key > open.saturating_add(staff.capo)
}

impl oxiguitar_plugin::Performer for StampPerformer {
    fn perform(
        &self,
        track: &oxiguitar_plugin::Perform,
    ) -> Result<Vec<oxiguitar_plugin::PlayedNote>, Error> {
        use oxiguitar_plugin::score2::BeatExt;
        self.asked
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamped: Vec<(u32, u32, u32, u32)> = track
            .beats
            .iter()
            .filter(|b| {
                b.ext.iter().any(
                    |e| matches!(e, BeatExt::Plugin { plugin, data } if plugin == ID && data == STAMP),
                )
            })
            .map(|b| (b.staff, b.bar, b.voice, b.beat))
            .collect();
        let own = self
            .host
            .settings()
            .ok()
            .and_then(|s| s.get("perform-detune").and_then(serde_json::Value::as_i64))
            .and_then(|c| i32::try_from(c).ok())
            .unwrap_or(0);
        let detune = cents_in(&track.settings, "detune").saturating_add(own);
        Ok(track
            .notes
            .iter()
            .cloned()
            .map(|mut n| {
                if detune != 0 && fretted(track, &n) {
                    if n.pitch.is_empty() {
                        n.pitch.push((n.start, 0));
                    }
                    for p in &mut n.pitch {
                        p.1 = p.1.saturating_add(detune);
                    }
                }
                let bar = track
                    .visits
                    .get(n.visit as usize)
                    .copied()
                    .unwrap_or(u32::MAX);
                if stamped.contains(&(n.staff, bar, n.voice, n.beat)) {
                    n.key = n.key.saturating_add(12).min(127);
                }
                n
            })
            .collect())
    }
}

/// How wide and tall the stamp is, in staff spaces.
pub const STAMP_BOX: (f32, f32) = (2.4, 1.6);

impl oxiguitar_plugin::Drawer for StampDrawer {
    fn clicked(
        &self,
        place: &oxiguitar_plugin::DrawPlace,
        at: Point,
        double: bool,
    ) -> Result<String, Error> {
        let at_ = match place {
            oxiguitar_plugin::DrawPlace::Bar { at, .. } => at.to_vec(),
            oxiguitar_plugin::DrawPlace::Beat { at, .. } => at.to_vec(),
        };
        Ok(format!(
            "{} at {at_:?}, {:.1} by {:.1} in",
            if double { "double-clicked" } else { "clicked" },
            at.x,
            at.y
        ))
    }

    fn draw(
        &self,
        places: &[oxiguitar_plugin::DrawPlace],
        times: &oxiguitar_plugin::DrawTimes,
    ) -> Result<Vec<oxiguitar_plugin::Drawn>, Error> {
        use oxiguitar_plugin::display::PathCmd;
        use oxiguitar_plugin::score2::BeatExt;
        self.asked
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (w, h) = STAMP_BOX;
        let p = Point::new;
        Ok(places
            .iter()
            .enumerate()
            .filter(|(_, place)| match place {
                oxiguitar_plugin::DrawPlace::Beat { beat, .. } => beat.ext.iter().any(
                    |e| matches!(e, BeatExt::Plugin { plugin, data } if plugin == ID && data == STAMP),
                ),
                oxiguitar_plugin::DrawPlace::Bar { .. } => false,
            })
            .map(|(i, _)| oxiguitar_plugin::Drawn {
                place: i as u32,
                width: w,
                height: h,
                items: vec![
                    Item::Path {
                        cmds: vec![
                            PathCmd::MoveTo(p(0.1, 0.1)),
                            PathCmd::LineTo(p(w - 0.1, 0.1)),
                            PathCmd::LineTo(p(w - 0.1, h - 0.1)),
                            PathCmd::LineTo(p(0.1, h - 0.1)),
                            PathCmd::Close,
                        ],
                        fill: None,
                        stroke: Some((0.12, Paint::Accent)),
                    },
                    Item::Text {
                        text: "S".into(),
                        at: p(w / 2.0, h - 0.4),
                        size: 1.2,
                        font: Font::Sans,
                        anchor: Anchor::Middle,
                        paint: Paint::Accent,
                    },
                ],
                run: times
                    .starts
                    .get(i)
                    .is_some_and(|s| !s.is_empty())
                    .then_some(oxiguitar_plugin::DrawnRun {
                        seconds: 1.0,
                        width: 0.12,
                        paint: Paint::Accent,
                        dashed: true,
                        hook: true,
                    }),
            })
            .collect())
    }
}

impl Plugin for TestPlugin {
    fn start(host: Host) -> Result<TestPlugin, Error> {
        let ui = Arc::new(TestUi::new(host.clone()));
        Ok(TestPlugin {
            mark: Mark { host: host.clone() },
            link: LinkTool { host: host.clone() },
            stamp: Stamp { host: host.clone() },
            ask_tool: AskTool { host: host.clone() },
            settings_tool: SettingsTool { ui: ui.clone() },
            fake: fake::Fake::new(host.clone()),
            raw: Raw {
                host: host.clone(),
                frames: Mutex::new(None),
            },
            drawer: StampDrawer::default(),
            performer: StampPerformer {
                host: host.clone(),
                asked: std::sync::atomic::AtomicU32::new(0),
            },
            ui,
            host,
        })
    }

    fn ui(&self) -> Option<&dyn oxiguitar_plugin::Ui> {
        Some(&*self.ui)
    }

    fn drawer(&self) -> Option<&dyn oxiguitar_plugin::Drawer> {
        Some(&self.drawer)
    }

    fn performer(&self) -> Option<&dyn oxiguitar_plugin::Performer> {
        Some(&self.performer)
    }

    fn audio_exporter(&self, id: &str) -> Option<&dyn oxiguitar_plugin::AudioExporter> {
        (id == "raw").then_some(&self.raw as &dyn oxiguitar_plugin::AudioExporter)
    }

    fn tool(&self, id: &str) -> Option<&dyn Tool> {
        match id {
            "mark" => Some(&self.mark),
            "link" => Some(&self.link),
            "stamp" => Some(&self.stamp),
            "ask" => Some(&self.ask_tool),
            "settings" => Some(&self.settings_tool),
            _ => None,
        }
    }

    fn instrument(&self, id: &str) -> Option<&dyn oxiguitar_plugin::Instrument> {
        matches!(id, "drums" | "no-drums")
            .then_some(&sine::Sine as &dyn oxiguitar_plugin::Instrument)
    }

    fn effect(&self, id: &str) -> Option<&dyn oxiguitar_plugin::Effect> {
        (id == "gain").then_some(&sine::Gain as &dyn oxiguitar_plugin::Effect)
    }

    fn event(&self, event: &Event) {
        lock(&self.ui.events).push(format!("{event:?}"));
        // A score being opened shows in the panel at once.
        if matches!(event, Event::ScoreOpened { .. }) {
            let _ = self.ui.show_events();
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
            ("audio-device", "fake") => Some(&self.fake),
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
            // Interface 1.14: every tool, one `owner/id` a line, with ` *`
            // after one that has options.
            "tools" => Ok(self
                .host
                .tools()?
                .iter()
                .map(|t| {
                    let star = if t.options.is_some() { " *" } else { "" };
                    format!("{}/{}{star}\n", t.owner, t.id)
                })
                .collect::<String>()
                .into_bytes()),
            // `{"owner", "tool", "options"?}`: runs it, returns its report.
            "run-tool" => {
                let asked: serde_json::Value = serde_json::from_slice(input)
                    .map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
                let text = |k: &str| {
                    asked
                        .get(k)
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                };
                Ok(self
                    .host
                    .run_tool(text("owner"), text("tool"), asked.get("options"))?
                    .into_bytes())
            }
            // The text's edges at 12, as numbers a line.
            "measure" => {
                let text = String::from_utf8_lossy(input);
                let m = self.host.measure_text(&text, 12.0, Font::Sans)?;
                Ok(m.edges
                    .iter()
                    .map(|e| format!("{e}\n"))
                    .collect::<String>()
                    .into_bytes())
            }
            "copy" => {
                self.host.copy(&String::from_utf8_lossy(input))?;
                Ok(Vec::new())
            }
            // Selects bars `first last` of the first track.
            "select" => {
                let text = String::from_utf8_lossy(input);
                let mut n = text
                    .split_whitespace()
                    .filter_map(|w| w.parse::<u32>().ok());
                let (Some(first_bar), Some(last_bar)) = (n.next(), n.next()) else {
                    return Err(Error::new(ErrorKind::Invalid, "two bars expected"));
                };
                self.host.select(Selection2::Bars {
                    first_track: 0,
                    last_track: 0,
                    first_bar,
                    last_bar,
                })?;
                Ok(Vec::new())
            }
            // What its views were told, a line each (interface 1.15).
            "views-heard" => Ok(self.ui.heard.lock().unwrap().join("\n").into_bytes()),
            // `show VIEW`, `close VIEW`, `set VIEW TEXT` (the property
            // `said`): asked of the host (interface 1.15).
            "view" => {
                let text = String::from_utf8_lossy(input).into_owned();
                let mut words = text.splitn(3, ' ');
                match (words.next(), words.next(), words.next()) {
                    (Some("show"), Some(view), None) => self.host.show_view(view)?,
                    (Some("close"), Some(view), None) => self.host.close_view(view)?,
                    (Some("set"), Some(view), Some(said)) => self.host.set_view(
                        view,
                        vec![("said".into(), oxiguitar_plugin::Value::String(said.into()))],
                    )?,
                    _ => return Err(Error::new(ErrorKind::Invalid, "show, close or set a view")),
                }
                Ok(Vec::new())
            }
            // The open score's file, or nothing (interface 1.13).
            "score-file" => Ok(self
                .host
                .score_file()?
                .map(|p| p.to_string_lossy().into_owned().into_bytes())
                .unwrap_or_default()),
            // The folder its file was unpacked in, then what its
            // assets/hello.txt holds (interface 1.16).
            "own-files" => {
                let dir = self.host.own_files()?;
                let hello = std::fs::read_to_string(dir.join("assets").join("hello.txt"))
                    .map_err(|e| Error::new(ErrorKind::Failed, e.to_string()))?;
                Ok(format!("{}\n{hello}", dir.display()).into_bytes())
            }
            // The user's plugin folder, then a line a plugin file: its
            // folder's kind, state, id and path (interface 1.15).
            "plugin-files" => {
                let found = self.host.plugin_files()?;
                let mut out = format!("{}\n", found.user_folder);
                for f in found.files {
                    let state = match f.state {
                        PluginFileState::Used => "used",
                        PluginFileState::Disabled => "disabled",
                        PluginFileState::Replaced(_) => "replaced",
                        PluginFileState::Unusable(_) => "unusable",
                        PluginFileState::Unmet(_) => "unmet",
                        PluginFileState::Withdrawn(_) => "withdrawn",
                    };
                    out.push_str(&format!(
                        "{:?} {state} {} {}\n",
                        f.folder,
                        f.id.as_deref().unwrap_or("-"),
                        f.path
                    ));
                }
                Ok(out.into_bytes())
            }
            // How many times it was asked to draw (interface 1.8).
            // How many times it was asked to play a track (interface 1.8).
            // Sets the plugin's own detune for playing (interface 1.12).
            "perform-detune" => {
                let cents: i64 = std::str::from_utf8(input)
                    .ok()
                    .and_then(|t| t.trim().parse().ok())
                    .ok_or_else(|| Error::new(ErrorKind::Invalid, "cents expected"))?;
                let mut settings = self.host.settings()?;
                settings.insert("perform-detune".into(), cents.into());
                self.host.set_settings(&settings)?;
                Ok(Vec::new())
            }
            "performed" => Ok(self
                .performer
                .asked
                .load(std::sync::atomic::Ordering::Relaxed)
                .to_string()
                .into_bytes()),
            "drawn" => Ok(self
                .drawer
                .asked
                .load(std::sync::atomic::Ordering::Relaxed)
                .to_string()
                .into_bytes()),
            // Crashes OxiGuitar, as a plugin with a bug would: in its code
            // on the calling thread, or on a thread of its own. For the
            // contract suite's test of crash reports, run in a process of
            // its own.
            "crash" => crash(),
            "crash-thread" => {
                let _ = std::thread::spawn(crash).join();
                Ok(Vec::new())
            }
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
            "events" => Ok(lock(&self.ui.events).join("\n").into_bytes()),
            "link" => {
                let url = String::from_utf8_lossy(input).into_owned();
                self.host
                    .notify_with_link(Level::Warning, "A link.", "Open it", url);
                Ok(Vec::new())
            }
            // The input is an `Advice` as JSON; the host's answer (an error
            // for advice it won't show) comes back as this call's.
            "advise" => {
                let advice: oxiguitar_plugin::Advice = serde_json::from_slice(input)
                    .map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
                self.host.advise(advice)?;
                Ok(Vec::new())
            }
            "folders" => {
                let folders = self.host.folders()?;
                std::fs::write(folders.data.join("hello.txt"), "hello\n")
                    .map_err(|e| Error::new(ErrorKind::Failed, e.to_string()))?;
                let soundfonts = self.host.folder("soundfonts")?.user.display().to_string();
                Ok(format!(
                    "{}\n{}\n{soundfonts}",
                    folders.data.display(),
                    folders.cache.display()
                )
                .into_bytes())
            }
            // A folder of the plugin's own, by its name in the manifest
            // (interface 1.8): every place it is, the user's first.
            "folder" => {
                let name = std::str::from_utf8(input)
                    .map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
                let places = self.host.folder(name)?;
                Ok(places
                    .all()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
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

oxiguitar_plugin::export_plugin!(TestPlugin);

/// Writes where no program may: the crash a plugin's bug would be.
#[allow(unsafe_code)]
fn crash() -> ! {
    let nowhere = std::ptr::without_provenance_mut::<u8>(8);
    // SAFETY: none: this is the crash, on purpose (`crash` and
    // `crash-thread`, for the contract suite).
    unsafe { nowhere.write_volatile(1) };
    unreachable!("the write above crashes")
}

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

    /// The fuzz target `plugin_drawing`'s seeds, `fixtures/plugin/*.answer`:
    /// the stamp's drawing as the plugin answers with it, and one whose line
    /// runs far off its box (what the fuzzer found of a canvas on 18 Oct
    /// 2026). `OXIGUITAR_BLESS=1` rewrites them.
    #[test]
    fn the_fuzzing_seeds_are_drawings() {
        use oxiguitar_plugin::api::message::{Answer, DrawPlace, DrawTimes, Request};
        use oxiguitar_plugin::score::{BeatEffects, BeatKind, Duration, NoteValue};
        use oxiguitar_plugin::score2::{Beat2, BeatExt};
        let plugin = TestPlugin::start(Host::detached()).unwrap();
        let beat = Beat2 {
            kind: BeatKind::Rest,
            duration: Duration {
                value: NoteValue::Quarter,
                dots: 0,
                tuplets: Vec::new(),
            },
            notes: Vec::new(),
            grace: None,
            effects: BeatEffects::default(),
            chord: None,
            text: String::new(),
            ext: vec![BeatExt::Plugin {
                plugin: ID.into(),
                data: STAMP.into(),
            }],
        };
        let request = Request::Draw {
            places: vec![DrawPlace::Beat {
                at: [0; 5],
                beat: Box::new(beat),
            }],
            times: DrawTimes::default(),
        };
        let answer = oxiguitar_plugin::dispatch(&plugin, request).unwrap();
        let Answer::Drawn(stamp) = &answer else {
            panic!("{answer:?}")
        };
        let mut far = stamp.clone();
        far[0].items = vec![Item::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(1.0e30, -1.0e30),
            width: 1.0,
            paint: Paint::Accent,
        }];
        for (name, answer) in [("stamp", answer.clone()), ("far-line", Answer::Drawn(far))] {
            let bytes = oxiguitar_plugin::api::encode(&answer).unwrap();
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../fixtures/plugin/{name}.answer"));
            if std::env::var_os("OXIGUITAR_BLESS").is_some() {
                std::fs::write(&file, &bytes).unwrap();
            }
            assert_eq!(
                std::fs::read(&file).expect("run with OXIGUITAR_BLESS=1 to write it"),
                bytes,
                "{name} changed: run with OXIGUITAR_BLESS=1"
            );
        }
    }

    /// The settings window keeps its draft with OK and forgets it with
    /// Cancel; the export's options keep the rate when the export dialog's
    /// OK closes them.
    #[test]
    fn drafts_are_kept_with_ok_and_forgotten_otherwise() {
        use oxiguitar_plugin::{Ui, Value};
        let ui = TestUi::new(Host::detached());
        ui.callback("settings", "greet-toggled", vec![Value::Bool(true)])
            .unwrap();
        ui.callback("settings", "name-edited", vec![Value::String("Ana".into())])
            .unwrap();
        assert_eq!(
            ui.settings_properties()[2].1,
            Value::String("Hello, Ana.".into())
        );
        ui.closed("settings", false).unwrap();
        assert_eq!(
            ui.settings_properties()[2].1,
            Value::String("Nothing to say.".into())
        );
        ui.callback("raw-options", "rate-chosen", vec![Value::Number(1.0)])
            .unwrap();
        assert_eq!(ui.raw_properties()[1].1, Value::Number(1.0));
        assert!(
            ui.callback(
                "test",
                "nickname-edited",
                vec![Value::String("refuse".into())]
            )
            .is_err()
        );
    }
}
