// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The tab "Statistics", in the window's right area: how many notes each
//! string of a track plays in the open score, as a small bar chart. Click a
//! bar to see its count; "Count tied notes" is remembered in settings.json.
//!
//! It shows the parts of a tab: widgets the core draws (labels, a choice, a
//! toggle), a canvas the plugin draws and gets pointer events on, score
//! events, asking the host for the open score, redrawing, and settings.

use std::sync::{Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin::display::{Anchor, Font, Item, Paint, Point};
use oxiguitar_plugin::score::{NotePlace, Score};
use oxiguitar_plugin::{
    Canvas, Error, ErrorKind, Event, Host, Level, Pointer, PointerKind, Row, Tab, TabContent,
    TabEvent, Widget,
};

/// The tab's id in `manifest.json`.
pub const ID: &str = "statistics";

/// The canvas's size, in its own units (the core scales it to fit).
const WIDTH: f32 = 120.0;
const HEIGHT: f32 = 60.0;
/// Room under the bars for the strings' numbers.
const LABELS: f32 = 12.0;

// ANCHOR: state
/// The tab. Calls may come from any thread at once, so what changes lives
/// behind a lock.
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
    /// The tab, with its settings read.
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
    /// host's event thread: note it and ask for a redraw, and leave the work
    /// to `content`, which runs only while the tab is on screen.
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
        self.host.redraw(ID);
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

impl Tab for Statistics {
    // ANCHOR: content
    fn content(&self) -> TabContent {
        self.refresh();
        let s = self.state();
        let label = |text: String, muted: bool| Widget::Label { text, muted };
        if s.tracks.is_empty() {
            return TabContent {
                rows: vec![Row {
                    widgets: vec![label("Open a score to count its notes".into(), true)],
                }],
            };
        }
        let counts = s.per_string();
        let total: u32 = counts.iter().sum();
        let detail = match s.picked.and_then(|i| Some((i, counts.get(i)?))) {
            Some((i, n)) => format!("String {}: {n} notes", i + 1),
            None if counts.is_empty() => "This track has no strings".into(),
            None => format!("{total} notes; click a bar"),
        };
        let mut rows = vec![
            Row {
                widgets: vec![label(s.title.clone(), false)],
            },
            Row {
                widgets: vec![Widget::Choice {
                    id: "track".into(),
                    options: s.tracks.iter().map(|t| t.name.clone()).collect(),
                    selected: s.track as u32,
                    enabled: s.tracks.len() > 1,
                }],
            },
            Row {
                widgets: vec![Widget::Toggle {
                    id: "tied".into(),
                    text: "Count tied notes".into(),
                    on: s.count_tied,
                    enabled: true,
                }],
            },
            Row {
                widgets: vec![label(detail, s.picked.is_none())],
            },
        ];
        if !counts.is_empty() {
            rows.push(Row {
                widgets: vec![Widget::Canvas(Canvas {
                    id: "chart".into(),
                    width: WIDTH,
                    height: HEIGHT,
                    items: draw(&counts, s.picked),
                })],
            });
        }
        TabContent { rows }
    }
    // ANCHOR_END: content

    // ANCHOR: on
    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        match (widget, event) {
            ("chart", TabEvent::Pointer(pointer)) => {
                let mut s = self.state();
                let strings = s.per_string().len();
                if let Some(string) = bar_at(pointer, strings) {
                    s.picked = Some(string);
                }
            }
            ("track", TabEvent::Chosen(i)) => {
                let mut s = self.state();
                if (i as usize) < s.tracks.len() {
                    s.track = i as usize;
                    s.picked = None;
                }
            }
            ("tied", TabEvent::Toggled(on)) => {
                self.state().count_tied = on;
                // Not being able to save it is worth knowing, not worth
                // losing the tab over.
                if let Err(e) = self.save(on) {
                    let text = format!("\"Count tied notes\" can't be remembered: {e}");
                    self.host.notify(Level::Warning, text);
                }
            }
            (widget, event) => {
                return Err(Error::new(
                    ErrorKind::Invalid,
                    format!("the widget {widget:?} doesn't take {event:?}"),
                ));
            }
        }
        // The core asks for the contents again after every event.
        Ok(())
    }
    // ANCHOR_END: on
}

/// The width of each string's slot on the canvas.
fn slot(strings: usize) -> f32 {
    WIDTH / strings.max(1) as f32
}

// ANCHOR: pointer
/// The string whose bar is under the pointer when a button goes down.
fn bar_at(pointer: Pointer, strings: usize) -> Option<usize> {
    let Point { x, y } = pointer.at;
    let inside = (0.0..WIDTH).contains(&x) && (0.0..HEIGHT).contains(&y);
    (pointer.kind == PointerKind::Down && inside && strings > 0)
        .then(|| ((x / slot(strings)) as usize).min(strings - 1))
}
// ANCHOR_END: pointer

// ANCHOR: draw
/// A bar per string, as tall as its share of the busiest string, with the
/// strings' numbers under them. Paints are roles, so the chart follows the
/// window's theme.
fn draw(counts: &[u32], picked: Option<usize>) -> Vec<Item> {
    let most = counts.iter().copied().max().unwrap_or(0).max(1) as f32;
    let slot = slot(counts.len());
    let room = HEIGHT - LABELS;
    let mut items = vec![Item::Line {
        from: Point::new(0.0, room),
        to: Point::new(WIDTH, room),
        width: 0.5,
        paint: Paint::Muted,
    }];
    for (i, &n) in counts.iter().enumerate() {
        let x = i as f32 * slot;
        let height = room * n as f32 / most;
        items.push(Item::Rect {
            at: Point::new(x + slot * 0.15, room - height),
            width: slot * 0.7,
            height,
            paint: if picked == Some(i) {
                Paint::Accent
            } else {
                Paint::Ink
            },
        });
        items.push(Item::Text {
            text: (i + 1).to_string(),
            at: Point::new(x + slot / 2.0, HEIGHT - 2.0),
            size: 8.0,
            font: Font::Sans,
            anchor: Anchor::Middle,
            paint: Paint::Muted,
        });
    }
    items
}
// ANCHOR_END: draw

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::{FormatReader, PointerButton};

    use super::*;

    fn down(x: f32, y: f32) -> TabEvent {
        TabEvent::Pointer(Pointer {
            kind: PointerKind::Down,
            at: Point::new(x, y),
            button: PointerButton::Primary,
        })
    }

    fn label(content: &TabContent, row: usize) -> String {
        match &content.rows[row].widgets[0] {
            Widget::Label { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        }
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
        let tab = Statistics::new(Host::detached());
        let empty = tab.content();
        assert!(empty.problems().is_empty());
        assert!(label(&empty, 0).starts_with("Open a score"));
        // A detached host has no score open.
        tab.event(&Event::ScoreOpened { revision: 1 });
        assert!(label(&tab.content(), 0).starts_with("Open a score"));

        tab.state().title = "Open Strings".into();
        tab.state().tracks = count(&sample());
        let content = tab.content();
        assert!(content.problems().is_empty());
        assert_eq!(label(&content, 3), "29 notes; click a bar");
        // The third bar: string 3.
        tab.on("chart", down(slot(6) * 2.5, 30.0)).unwrap();
        assert_eq!(label(&tab.content(), 3), "String 3: 6 notes");
        tab.on("chart", down(500.0, 30.0)).unwrap();
        assert_eq!(label(&tab.content(), 3), "String 3: 6 notes");
        tab.on("tied", TabEvent::Toggled(true)).unwrap();
        assert!(tab.state().count_tied);
        assert!(tab.on("tied", TabEvent::Clicked).is_err());
    }
}
