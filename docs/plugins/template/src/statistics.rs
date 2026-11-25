// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The panel "Statistics", in the window's right area: how many notes each
//! string of a track plays in the open score, as a small bar chart. Click a
//! bar to see its count; "Count tied notes" is remembered in settings.json.
//!
//! It shows the parts of a plugin's UI: a view of the plugin's own
//! `ui/example.slint` (the component `Statistics`), its properties set
//! from Rust and its callbacks heard, score events, asking the host for the
//! open score, and settings.

use std::sync::{Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin::score::{NotePlace, Score};
use oxiguitar_plugin::{Error, ErrorKind, Event, Host, Level, TrackView, Ui, Value};

/// The view's id in `manifest.json`.
pub const ID: &str = "statistics";

// ANCHOR: state
/// The panel. Calls may come from any thread at once, so what changes
/// lives behind a lock.
pub struct Statistics {
    host: Host,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// A score was opened or changed since it was last counted.
    stale: bool,
    /// The open score's title, and each track's counts; none when no
    /// score is open.
    title: String,
    tracks: Vec<Counts>,
    /// The track shown.
    track: usize,
    /// The string whose bar was clicked last, from 0 (string 1).
    picked: Option<usize>,
    /// Count notes tied to the one before; kept in settings.json.
    count_tied: bool,
}

/// One track's notes, string by string.
#[derive(Clone, Debug, Default, PartialEq)]
struct Counts {
    name: String,
    /// Notes on each string, from string 1; empty for a track without
    /// strings.
    struck: Vec<u32>,
    /// Of those, the notes tied to the one before.
    tied: Vec<u32>,
}
// ANCHOR_END: state

impl Statistics {
    /// The panel, with its settings read.
    pub fn new(host: Host) -> Statistics {
        // ANCHOR: settings_read
        let count_tied = host
            .settings()
            .ok()
            .and_then(|s| s.get("count_tied").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        // ANCHOR_END: settings_read
        Statistics {
            host,
            state: Mutex::new(State {
                count_tied,
                ..State::default()
            }),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // ANCHOR: event
    /// A score event (the manifest subscribes to `score`). It comes on the
    /// host's event thread: note it, count the score again and show it.
    pub fn event(&self, event: &Event) {
        match event {
            Event::ScoreOpened { .. } | Event::ScoreChanged { .. } => self.state().stale = true,
            Event::ScoreClosed => {
                let count_tied = self.state().count_tied;
                *self.state() = State {
                    count_tied,
                    ..State::default()
                };
            }
            _ => return,
        }
        self.show();
    }
    // ANCHOR_END: event

    // ANCHOR: settings_write
    /// Keeps `count_tied` in the plugin's settings, beside whatever else is
    /// there.
    fn save(&self, count_tied: bool) -> Result<(), Error> {
        let mut settings = self.host.settings()?;
        settings.insert("count_tied".into(), count_tied.into());
        self.host.set_settings(&settings)
    }
    // ANCHOR_END: settings_write

    // ANCHOR: score
    /// Counts the open score's notes, if a score event came since.
    fn refresh(&self) {
        if !std::mem::take(&mut self.state().stale) {
            return;
        }
        // Not with the state locked: converting a long score takes a while.
        let score = self.host.score();
        let mut s = self.state();
        match score {
            Ok(Some(score)) => {
                s.title = score.info.title.clone();
                s.tracks = count(&score);
            }
            Ok(None) | Err(_) => s.tracks.clear(),
        }
        s.track = s.track.min(s.tracks.len().saturating_sub(1));
        s.picked = None;
    }
    // ANCHOR_END: score
}

/// Each track's notes, string by string.
fn count(score: &Score) -> Vec<Counts> {
    score
        .tracks
        .iter()
        .map(|track| {
            let strings = track
                .staves
                .iter()
                .map(|s| s.tuning.len())
                .max()
                .unwrap_or(0);
            let mut counts = Counts {
                name: track.name.clone(),
                struck: vec![0; strings],
                tied: vec![0; strings],
            };
            let notes = track
                .staves
                .iter()
                .flat_map(|s| &s.bars)
                .flat_map(|b| &b.voices)
                .flat_map(|v| &v.beats)
                .flat_map(|b| &b.notes);
            for note in notes {
                if let NotePlace::Fretted { string, .. } = note.place
                    && let Some(i) = usize::from(string).checked_sub(1).filter(|&i| i < strings)
                {
                    counts.struck[i] += 1;
                    counts.tied[i] += u32::from(note.tie);
                }
            }
            counts
        })
        .collect()
}

impl State {
    /// The shown track's notes on each string, as the setting counts them.
    fn per_string(&self) -> Vec<u32> {
        let Some(t) = self.tracks.get(self.track) else {
            return Vec::new();
        };
        if self.count_tied {
            t.struck.clone()
        } else {
            t.struck.iter().zip(&t.tied).map(|(s, t)| s - t).collect()
        }
    }
}

impl Statistics {
    // ANCHOR: properties
    /// What the panel shows: the view's properties, by the names
    /// `ui/example.slint` gives them.
    fn properties(&self) -> Vec<(String, Value)> {
        let s = self.state();
        let text = |t: &str| Value::String(t.to_owned());
        if s.tracks.is_empty() {
            return vec![
                ("title".into(), text("Open a score to count its notes")),
                ("has-score".into(), Value::Bool(false)),
                ("bars".into(), Value::List(Vec::new())),
            ];
        }
        let counts = s.per_string();
        let total: u32 = counts.iter().sum();
        let most = counts.iter().copied().max().unwrap_or(0).max(1);
        let detail = match s.picked.and_then(|i| Some((i, counts.get(i)?))) {
            Some((i, n)) => format!("String {}: {n} notes", i + 1),
            None if counts.is_empty() => "This track has no strings".into(),
            None => format!("{total} notes; click a bar"),
        };
        let bars = counts
            .iter()
            .enumerate()
            .map(|(i, n)| {
                Value::Struct(vec![
                    ("label".into(), Value::String((i + 1).to_string())),
                    (
                        "height".into(),
                        Value::Number(f64::from(*n) / f64::from(most)),
                    ),
                    ("picked".into(), Value::Bool(s.picked == Some(i))),
                ])
            })
            .collect();
        vec![
            ("title".into(), text(&s.title)),
            ("has-score".into(), Value::Bool(true)),
            (
                "tracks".into(),
                Value::List(s.tracks.iter().map(|t| text(&t.name)).collect()),
            ),
            ("track".into(), Value::Number(s.track as f64)),
            ("count-tied".into(), Value::Bool(s.count_tied)),
            ("detail".into(), Value::String(detail)),
            ("bars".into(), Value::List(bars)),
        ]
    }
    // ANCHOR_END: properties

    /// Counts the score again if it changed, and sets the panel's
    /// properties. A panel not made yet takes them when it is.
    fn show(&self) {
        self.refresh();
        if let Err(e) = self.host.set_view(ID, self.properties()) {
            self.host.notify(Level::Warning, format!("Statistics: {e}"));
        }
    }
}

// ANCHOR: ui
impl Ui for Statistics {
    /// The panel was made: it shows what is counted.
    fn shown(&self, _view: &str, _track: Option<&TrackView>) -> Result<(), Error> {
        self.show();
        Ok(())
    }

    /// What the user did in the panel.
    fn callback(&self, _view: &str, callback: &str, args: Vec<Value>) -> Result<(), Error> {
        let index = match args.first() {
            Some(Value::Number(n)) if *n >= 0.0 => Some(*n as usize),
            _ => None,
        };
        match (callback, args.first()) {
            ("bar-clicked", _) => {
                let mut s = self.state();
                if let Some(i) = index.filter(|i| *i < s.per_string().len()) {
                    s.picked = Some(i);
                }
            }
            ("track-chosen", _) => {
                let mut s = self.state();
                if let Some(i) = index.filter(|i| *i < s.tracks.len()) {
                    s.track = i;
                    s.picked = None;
                }
            }
            ("tied-toggled", Some(Value::Bool(on))) => {
                self.state().count_tied = *on;
                // Not being able to save it is worth knowing, not worth
                // losing the panel over.
                if let Err(e) = self.save(*on) {
                    let text = format!("\"Count tied notes\" can't be remembered: {e}");
                    self.host.notify(Level::Warning, text);
                }
            }
            (callback, _) => {
                return Err(Error::new(
                    ErrorKind::Invalid,
                    format!("the panel has no callback {callback:?} with {args:?}"),
                ));
            }
        }
        self.show();
        Ok(())
    }
}
// ANCHOR_END: ui

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::FormatReader;

    use super::*;

    fn get(p: &[(String, Value)], name: &str) -> Value {
        p.iter().find(|(n, _)| n == name).unwrap().1.clone()
    }

    fn sample() -> Score {
        let file = include_bytes!("../samples/open-strings.extab");
        crate::format::ExampleTab.read(file).unwrap().score
    }

    #[test]
    fn counts_each_string() {
        let mut score = sample();
        let counts = count(&score);
        assert_eq!(counts[0].struck, [4, 5, 6, 6, 5, 3]);
        score.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0].tie = true;
        assert_eq!(count(&score)[0].tied, [0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn shows_nothing_without_a_score_and_follows_clicks() {
        let panel = Statistics::new(Host::detached());
        let title = |p: &[(String, Value)]| get(p, "title");
        assert_eq!(
            title(&panel.properties()),
            Value::String("Open a score to count its notes".into())
        );
        // A detached host has no score open.
        panel.event(&Event::ScoreOpened { revision: 1 });
        assert_eq!(get(&panel.properties(), "has-score"), Value::Bool(false));

        panel.state().title = "Open Strings".into();
        panel.state().tracks = count(&sample());
        let detail = |panel: &Statistics| get(&panel.properties(), "detail");
        assert_eq!(
            detail(&panel),
            Value::String("29 notes; click a bar".into())
        );
        // The third bar: string 3.
        panel
            .callback(ID, "bar-clicked", vec![Value::Number(2.0)])
            .unwrap();
        assert_eq!(detail(&panel), Value::String("String 3: 6 notes".into()));
        panel
            .callback(ID, "bar-clicked", vec![Value::Number(9.0)])
            .unwrap();
        assert_eq!(detail(&panel), Value::String("String 3: 6 notes".into()));
        panel
            .callback(ID, "tied-toggled", vec![Value::Bool(true)])
            .unwrap();
        assert!(panel.state().count_tied);
        assert!(panel.callback(ID, "other", vec![]).is_err());
    }

    /// The file compiles as OxiGuitar compiles it, with every property the
    /// panel sets.
    #[test]
    fn the_slint_file_has_what_the_panel_sets() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/example.slint");
        let compiled = oxiguitar_plugin_test::compile_ui_file(&file).unwrap();
        let panel = compiled.component("Statistics").unwrap();
        let shown = Statistics::new(Host::detached());
        shown.state().tracks = count(&sample());
        for (name, _) in shown.properties() {
            assert!(panel.properties.contains(&name), "{name}");
        }
        for name in ["track-chosen", "tied-toggled", "bar-clicked"] {
            assert!(panel.callbacks.iter().any(|c| c == name), "{name}");
        }
    }
}
