// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The tab "Fretboard", in the window's bottom area: a fretboard tuned like
//! the open score's first track (standard tuning when no score is open).
//! Click a string to name the note; "Flats" is remembered in settings.json.
//!
//! It shows the parts of a tab: widgets the core draws (labels, a toggle, a
//! button), a canvas the plugin draws and gets pointer events on, score
//! events, asking the host for the open score, redrawing, and settings.

use std::sync::{Mutex, MutexGuard, PoisonError};

use oxiguitar_plugin::display::{Anchor, Font, Item, Paint, PathCmd, Point};
use oxiguitar_plugin::score::{Pitch, Score};
use oxiguitar_plugin::{
    Canvas, Error, ErrorKind, Event, Host, Level, Pointer, PointerKind, Row, Tab, TabContent,
    TabEvent, Widget,
};

use crate::notes;

/// The tab's id in `manifest.json`.
pub const ID: &str = "fretboard";

/// Frets shown, after the open strings.
const FRETS: u8 = 12;
/// The canvas's layout, in its own units (the core scales it to fit).
const NUT: f32 = 40.0;
const FRET_WIDTH: f32 = 50.0;
const STRING_GAP: f32 = 20.0;
const TOP: f32 = 16.0;
const BOTTOM: f32 = 26.0;
const WIDTH: f32 = NUT + FRETS as f32 * FRET_WIDTH + 10.0;

// ANCHOR: state
/// The tab. Calls may come from any thread at once, so what changes lives
/// behind a lock.
pub struct Fretboard {
    host: Host,
    state: Mutex<State>,
}

struct State {
    /// A score was opened or changed since the tuning was last read.
    stale: bool,
    /// The strings' open pitches, from string 1 (the highest).
    tuning: Vec<Pitch>,
    /// Where the tuning comes from.
    source: String,
    /// The string and fret last clicked.
    picked: Option<(usize, u8)>,
    /// Name notes with flats; kept in settings.json.
    flats: bool,
}
// ANCHOR_END: state

impl Fretboard {
    /// The tab, with its settings read.
    pub fn new(host: Host) -> Fretboard {
        // ANCHOR: settings_read
        let flats = host
            .settings()
            .ok()
            .and_then(|s| s.get("flats").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        // ANCHOR_END: settings_read
        Fretboard {
            host,
            state: Mutex::new(State {
                stale: false,
                tuning: notes::STANDARD.to_vec(),
                source: "Standard tuning: no score is open".into(),
                picked: None,
                flats,
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
                let mut s = self.state();
                s.stale = false;
                s.tuning = notes::STANDARD.to_vec();
                s.source = "Standard tuning: no score is open".into();
                s.picked = None;
            }
            _ => return,
        }
        self.host.redraw(ID);
    }
    // ANCHOR_END: event

    // ANCHOR: settings_write
    /// Keeps `flats` in the plugin's settings, beside whatever else is there.
    fn save_flats(&self, flats: bool) -> Result<(), Error> {
        let mut settings = self.host.settings()?;
        settings.insert("flats".into(), flats.into());
        self.host.set_settings(&settings)
    }
    // ANCHOR_END: settings_write

    // ANCHOR: score
    /// Reads the tuning from the open score, if a score event came since.
    fn refresh(&self) {
        if !std::mem::take(&mut self.state().stale) {
            return;
        }
        // Not with the state locked: converting a long score takes a while.
        let (tuning, source) = match self.host.score() {
            Ok(Some(score)) => tuning_of(&score),
            Ok(None) => (
                notes::STANDARD.to_vec(),
                "Standard tuning: no score is open".into(),
            ),
            Err(e) => (notes::STANDARD.to_vec(), format!("Standard tuning: {e}")),
        };
        let mut s = self.state();
        if s.tuning != tuning {
            s.picked = None;
        }
        s.tuning = tuning;
        s.source = source;
    }
    // ANCHOR_END: score
}

/// The tuning of the score's first track with strings, and a line saying so.
fn tuning_of(score: &Score) -> (Vec<Pitch>, String) {
    let title = match score.info.title.as_str() {
        "" => "the open score",
        t => t,
    };
    let track = score.tracks.iter().find_map(|t| {
        t.staves
            .first()
            .filter(|s| !s.tuning.is_empty())
            .map(|s| (t, s))
    });
    match track {
        Some((track, staff)) => (
            staff.tuning.clone(),
            format!("Tuning of {} in {title}", track.name),
        ),
        None => (
            notes::STANDARD.to_vec(),
            format!("Standard tuning: no track in {title} has strings"),
        ),
    }
}

impl Tab for Fretboard {
    // ANCHOR: content
    fn content(&self) -> TabContent {
        self.refresh();
        let s = self.state();
        let picked = s.picked.and_then(|(string, fret)| {
            let pitch = notes::fretted(*s.tuning.get(string)?, fret)?;
            Some(format!(
                "String {}, fret {fret}: {}",
                string + 1,
                notes::name(pitch, s.flats)
            ))
        });
        TabContent {
            rows: vec![
                Row {
                    widgets: vec![Widget::Label {
                        text: s.source.clone(),
                        muted: false,
                    }],
                },
                Row {
                    widgets: vec![Widget::Canvas(Canvas {
                        id: "board".into(),
                        width: WIDTH,
                        height: height(s.tuning.len()),
                        items: draw(&s),
                    })],
                },
                Row {
                    widgets: vec![
                        Widget::Label {
                            muted: picked.is_none(),
                            text: picked
                                .unwrap_or_else(|| "Click a string to name the note".into()),
                        },
                        Widget::Toggle {
                            id: "flats".into(),
                            text: "Flats".into(),
                            on: s.flats,
                            enabled: true,
                        },
                        Widget::Button {
                            id: "clear".into(),
                            text: "Clear".into(),
                            enabled: s.picked.is_some(),
                        },
                    ],
                },
            ],
        }
    }
    // ANCHOR_END: content

    // ANCHOR: on
    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        match (widget, event) {
            ("board", TabEvent::Pointer(pointer)) => {
                let mut s = self.state();
                if let Some(place) = place(pointer, s.tuning.len()) {
                    s.picked = Some(place);
                }
            }
            ("clear", TabEvent::Clicked) => self.state().picked = None,
            ("flats", TabEvent::Toggled(on)) => {
                self.state().flats = on;
                // Not being able to save it is worth knowing, not worth
                // losing the tab over: an error returned here would show
                // in the tab's place.
                if let Err(e) = self.save_flats(on) {
                    let text = format!("Flats can't be remembered: {e}");
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

/// The canvas's height for `strings` strings.
fn height(strings: usize) -> f32 {
    TOP + strings.saturating_sub(1) as f32 * STRING_GAP + BOTTOM
}

fn string_y(string: usize) -> f32 {
    TOP + string as f32 * STRING_GAP
}

/// The middle of the space for `fret` (0: left of the nut).
fn fret_x(fret: u8) -> f32 {
    match fret {
        0 => NUT / 2.0,
        f => NUT + (f32::from(f) - 0.5) * FRET_WIDTH,
    }
}

// ANCHOR: pointer
/// The string and fret under the pointer when a button goes down there.
fn place(pointer: Pointer, strings: usize) -> Option<(usize, u8)> {
    if pointer.kind != PointerKind::Down || strings == 0 {
        return None;
    }
    let Point { x, y } = pointer.at;
    let string = ((y - TOP) / STRING_GAP).round();
    if !(0.0..strings as f32).contains(&string) || !(0.0..WIDTH).contains(&x) {
        return None;
    }
    let fret = if x < NUT {
        0
    } else {
        (((x - NUT) / FRET_WIDTH).floor() as u8 + 1).min(FRETS)
    };
    Some((string as usize, fret))
}
// ANCHOR_END: pointer

// ANCHOR: draw
/// What the canvas shows: strings, frets, inlays, fret numbers, the open
/// strings' names and the note clicked. Paints are roles, so the drawing
/// follows the window's theme.
fn draw(s: &State) -> Vec<Item> {
    let strings = s.tuning.len();
    let (top, bottom) = (string_y(0), string_y(strings.saturating_sub(1)));
    let mut items = Vec::new();
    for fret in 0..=FRETS {
        let x = NUT + f32::from(fret) * FRET_WIDTH;
        items.push(Item::Line {
            from: Point::new(x, top),
            to: Point::new(x, bottom),
            width: if fret == 0 { 4.0 } else { 1.5 },
            paint: if fret == 0 { Paint::Ink } else { Paint::Muted },
        });
        if fret > 0 {
            items.push(text(
                &fret.to_string(),
                fret_x(fret),
                height(strings) - 6.0,
                Paint::Muted,
            ));
        }
    }
    let middle = (top + bottom) / 2.0;
    for fret in [3, 5, 7, 9] {
        items.push(circle(fret_x(fret), middle, 4.0, Some(Paint::Muted), None));
    }
    for y in [middle - STRING_GAP, middle + STRING_GAP] {
        items.push(circle(fret_x(12), y, 4.0, Some(Paint::Muted), None));
    }
    for (i, open) in s.tuning.iter().enumerate() {
        let y = string_y(i);
        items.push(Item::Line {
            from: Point::new(NUT, y),
            to: Point::new(WIDTH - 10.0, y),
            // Lower strings are drawn thicker.
            width: 1.0 + i as f32 * 0.25,
            paint: Paint::Ink,
        });
        let name = notes::name(*open, s.flats);
        let letter = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
        items.push(text(letter, fret_x(0), y + 4.0, Paint::Ink));
    }
    if let Some((string, fret)) = s.picked {
        let (x, y) = (fret_x(fret), string_y(string));
        // An open string is ringed; a fretted one gets a dot.
        let fill = (fret > 0).then_some(Paint::Accent);
        let stroke = (fret == 0).then_some((2.0, Paint::Accent));
        items.push(circle(x, y, 8.0, fill, stroke));
    }
    items
}
// ANCHOR_END: draw

fn text(text: &str, x: f32, y: f32, paint: Paint) -> Item {
    Item::Text {
        text: text.into(),
        at: Point::new(x, y),
        size: 11.0,
        font: Font::Sans,
        anchor: Anchor::Middle,
        paint,
    }
}

/// A circle, as four Bézier curves.
fn circle(x: f32, y: f32, r: f32, fill: Option<Paint>, stroke: Option<(f32, Paint)>) -> Item {
    let k = 0.552_284_8 * r;
    let p = Point::new;
    Item::Path {
        cmds: vec![
            PathCmd::MoveTo(p(x + r, y)),
            PathCmd::CubicTo(p(x + r, y + k), p(x + k, y + r), p(x, y + r)),
            PathCmd::CubicTo(p(x - k, y + r), p(x - r, y + k), p(x - r, y)),
            PathCmd::CubicTo(p(x - r, y - k), p(x - k, y - r), p(x, y - r)),
            PathCmd::CubicTo(p(x + k, y - r), p(x + r, y - k), p(x + r, y)),
            PathCmd::Close,
        ],
        fill,
        stroke,
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::PointerButton;

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

    #[test]
    fn clicking_a_string_names_the_note() {
        let tab = Fretboard::new(Host::detached());
        assert!(tab.content().problems().is_empty());
        // String 2 (B3), fret 3: D4.
        tab.on("board", down(fret_x(3), string_y(1))).unwrap();
        assert_eq!(label(&tab.content(), 2), "String 2, fret 3: D4");
        // String 6 open: E2.
        tab.on("board", down(5.0, string_y(5) + 3.0)).unwrap();
        assert_eq!(label(&tab.content(), 2), "String 6, fret 0: E2");
        tab.on("flats", TabEvent::Toggled(true)).unwrap();
        tab.on("board", down(fret_x(2), string_y(5))).unwrap();
        assert_eq!(label(&tab.content(), 2), "String 6, fret 2: G♭2");
        // Outside the strings, nothing changes.
        tab.on("board", down(fret_x(2), 500.0)).unwrap();
        assert_eq!(label(&tab.content(), 2), "String 6, fret 2: G♭2");
        tab.on("clear", TabEvent::Clicked).unwrap();
        assert!(label(&tab.content(), 2).starts_with("Click"));
        assert!(tab.on("clear", TabEvent::Toggled(true)).is_err());
    }

    #[test]
    fn the_tuning_follows_the_score() {
        use oxiguitar_plugin::FormatReader;
        let sample = include_bytes!("../samples/open-strings.extab");
        let mut score = crate::format::ExampleTab.read(sample).unwrap().score;
        score.tracks[0].name = "Bass".into();
        score.tracks[0].staves[0].tuning = vec![Pitch(43), Pitch(38), Pitch(33), Pitch(28)];
        let (tuning, source) = tuning_of(&score);
        assert_eq!(tuning.len(), 4);
        assert_eq!(source, "Tuning of Bass in Open Strings");
        let tab = Fretboard::new(Host::detached());
        tab.state().tuning = tuning;
        let content = tab.content();
        assert!(content.problems().is_empty());
        // A detached host has no score open.
        tab.event(&Event::ScoreOpened { revision: 1 });
        assert_eq!(
            label(&tab.content(), 0),
            "Standard tuning: no score is open"
        );
    }
}
