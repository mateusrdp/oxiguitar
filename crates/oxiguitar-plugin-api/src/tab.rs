// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugin tabs (interface 1.1): a tab in one of the window's four areas,
//! whose contents are standard widgets the core draws in its own style,
//! and, if the plugin needs one, a canvas it draws itself
//! ([`crate::display`]) and gets pointer events on.
//!
//! The manifest declares each tab ([`TabSpec`]), so the window shows its
//! name before the plugin is loaded. When the tab is first shown, the host
//! asks the plugin for its contents ([`crate::message::Request::Tab`]);
//! when the user uses a widget, the host sends the event
//! ([`crate::message::Request::TabEvent`]) and the plugin answers with the
//! tab's new contents. A plugin whose tab changes on its own (after a
//! pushed [`crate::event::Event`], say) asks the host to show it again
//! ([`crate::message::HostRequest::Redraw`]). Nothing polls.
//!
//! A tab's contents are rows from top to bottom, each holding widgets from
//! left to right: a simple shape that every area, and later the mobile
//! apps, can show the same way.

use serde::{Deserialize, Serialize};

use crate::display::{Item, MAX_ITEMS, MAX_SIDE};
use crate::manifest::is_id;

/// The window's four areas around the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Area {
    /// Above the page, across the window (the Player's area).
    Top,
    /// Below the page, across the window (Tracks, Mixer).
    Bottom,
    /// Left of the page (Palette).
    Left,
    /// Right of the page (Inspector).
    Right,
}

/// A tab, as the manifest declares it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSpec {
    /// The id requests name it by, unique in the plugin.
    pub id: String,
    /// The tab's name, as its strip shows it: a word or two.
    pub title: String,
    /// The area it sits in.
    pub area: Area,
}

/// What a tab shows: rows from top to bottom.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TabContent {
    /// The rows.
    pub rows: Vec<Row>,
}

/// One row of widgets, from left to right.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The widgets.
    pub widgets: Vec<Widget>,
}

/// A standard widget, drawn by the core in the window's own style.
///
/// Widgets the user can use carry an `id`, unique in the tab, which their
/// [`TabEvent`]s name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Widget {
    /// Text the user reads.
    Label {
        /// The text.
        text: String,
        /// Drawn dimmer, for secondary text.
        muted: bool,
    },
    /// A push button: [`TabEvent::Clicked`].
    Button {
        /// Its id.
        id: String,
        /// Its text.
        text: String,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A tickbox with its text: [`TabEvent::Toggled`].
    Toggle {
        /// Its id.
        id: String,
        /// Its text.
        text: String,
        /// Ticked.
        on: bool,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A slider: [`TabEvent::Slid`] when the user lets go.
    Slider {
        /// Its id.
        id: String,
        /// Its value, between `min` and `max`.
        value: f32,
        /// The left end.
        min: f32,
        /// The right end.
        max: f32,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A line of text to type: [`TabEvent::Edited`] when the user presses
    /// Enter.
    Field {
        /// Its id.
        id: String,
        /// Its text.
        text: String,
        /// Shown, dimmed, while it is empty.
        placeholder: String,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A drop-down of options: [`TabEvent::Chosen`].
    Choice {
        /// Its id.
        id: String,
        /// The options.
        options: Vec<String>,
        /// The option shown.
        selected: u32,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A list of lines, one of which may be selected:
    /// [`TabEvent::Chosen`].
    List {
        /// Its id.
        id: String,
        /// The lines.
        items: Vec<String>,
        /// The selected line, if one is.
        selected: Option<u32>,
    },
    /// A drawing of the plugin's own: [`TabEvent::Pointer`].
    Canvas(Canvas),
}

/// A drawing of the plugin's own, in a tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    /// Its id.
    pub id: String,
    /// Its width, in its own units.
    pub width: f32,
    /// Its height, in its own units. The core shows the canvas as large as
    /// the room allows with these proportions.
    pub height: f32,
    /// What it shows, in its own units ([`crate::display`]).
    pub items: Vec<Item>,
}

impl Widget {
    /// The widget's id, for those the user can use.
    pub fn id(&self) -> Option<&str> {
        match self {
            Widget::Label { .. } => None,
            Widget::Button { id, .. }
            | Widget::Toggle { id, .. }
            | Widget::Slider { id, .. }
            | Widget::Field { id, .. }
            | Widget::Choice { id, .. }
            | Widget::List { id, .. } => Some(id),
            Widget::Canvas(c) => Some(&c.id),
        }
    }
}

impl TabContent {
    /// The widget whose id is `id`.
    pub fn widget(&self, id: &str) -> Option<&Widget> {
        self.rows
            .iter()
            .flat_map(|r| &r.widgets)
            .find(|w| w.id() == Some(id))
    }

    /// Everything that keeps the core from showing these contents: ids that
    /// are missing or repeated, a slider whose range is backwards, a choice
    /// selecting an option it doesn't have, a canvas that is empty, too
    /// large, or holds something that can't be drawn ([`Item::problem`]).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids: Vec<&str> = Vec::new();
        for (r, row) in self.rows.iter().enumerate() {
            for (w, widget) in row.widgets.iter().enumerate() {
                let at = format!("row {}, widget {}", r + 1, w + 1);
                if let Some(id) = widget.id() {
                    if !is_id(id) {
                        out.push(format!("{at}: the id {id:?} is not a valid id"));
                    } else if ids.contains(&id) {
                        out.push(format!("{at}: the id {id:?} is used twice"));
                    }
                    ids.push(id);
                }
                match widget {
                    Widget::Slider {
                        value, min, max, ..
                    } => {
                        let numbers = value.is_finite() && min.is_finite() && max.is_finite();
                        if !numbers || min >= max {
                            out.push(format!("{at}: the slider's range is not min < max"));
                        }
                    }
                    Widget::Choice {
                        options, selected, ..
                    } => {
                        if *selected as usize >= options.len() {
                            out.push(format!(
                                "{at}: the choice selects option {selected} of {}",
                                options.len()
                            ));
                        }
                    }
                    Widget::List {
                        items, selected, ..
                    } => {
                        if selected.is_some_and(|s| s as usize >= items.len()) {
                            out.push(format!("{at}: the list selects a line it doesn't have"));
                        }
                    }
                    Widget::Canvas(c) => {
                        let side = |v: f32| v.is_finite() && v > 0.0 && v <= MAX_SIDE;
                        if !side(c.width) || !side(c.height) {
                            out.push(format!(
                                "{at}: the canvas must be wider and taller than 0, and at most {MAX_SIDE} units"
                            ));
                        }
                        if c.items.len() > MAX_ITEMS {
                            out.push(format!(
                                "{at}: the canvas holds {} items, more than {MAX_ITEMS}",
                                c.items.len()
                            ));
                        }
                        if let Some(p) = c.items.iter().find_map(|i| i.problem(c.width, c.height)) {
                            out.push(format!("{at}: {p}"));
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }
}

/// What the user did to a widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TabEvent {
    /// A button was clicked.
    Clicked,
    /// A toggle was ticked (`true`) or unticked.
    Toggled(bool),
    /// A slider was moved to this value and let go.
    Slid(f32),
    /// A field's text was entered (Enter pressed).
    Edited(String),
    /// A choice's option, or a list's line, was picked.
    Chosen(u32),
    /// The pointer on a canvas.
    Pointer(Pointer),
}

/// The pointer on a canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pointer {
    /// What happened.
    pub kind: PointerKind,
    /// Where, in the canvas's own units.
    pub at: crate::display::Point,
    /// Which button.
    pub button: PointerButton,
}

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerKind {
    /// A button went down on the canvas.
    Down,
    /// It came up again (wherever the pointer is).
    Up,
    /// The pointer moved with a button held: a drag. Moves the plugin
    /// hasn't answered yet are merged, so a slow plugin gets fewer.
    Drag,
}

/// A pointer's button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerButton {
    /// The main button (left, or a touch).
    Primary,
    /// The other button (right).
    Secondary,
    /// The middle button.
    Middle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Paint, Point};
    use crate::{decode, encode};

    fn button(id: &str) -> Widget {
        Widget::Button {
            id: id.into(),
            text: "Go".into(),
            enabled: true,
        }
    }

    #[test]
    fn contents_round_trip_and_positions_are_frozen() {
        let content = TabContent {
            rows: vec![
                Row {
                    widgets: vec![
                        Widget::Label {
                            text: "Tuning".into(),
                            muted: false,
                        },
                        button("go"),
                    ],
                },
                Row {
                    widgets: vec![Widget::Canvas(Canvas {
                        id: "fretboard".into(),
                        width: 600.0,
                        height: 80.0,
                        items: vec![Item::Rect {
                            at: Point::new(0.0, 0.0),
                            width: 600.0,
                            height: 80.0,
                            paint: Paint::Muted,
                        }],
                    })],
                },
            ],
        };
        assert!(content.problems().is_empty());
        let bytes = encode(&content).unwrap();
        assert_eq!(decode::<TabContent>(&bytes).unwrap(), content);
        assert!(matches!(
            content.widget("fretboard"),
            Some(Widget::Canvas(_))
        ));
        assert!(content.widget("nothing").is_none());

        let first = |b: Vec<u8>| b[0];
        assert_eq!(
            first(
                encode(&Widget::List {
                    id: "l".into(),
                    items: vec![],
                    selected: None
                })
                .unwrap()
            ),
            6
        );
        assert_eq!(first(encode(&TabEvent::Chosen(1)).unwrap()), 4);
        let pointer = TabEvent::Pointer(Pointer {
            kind: PointerKind::Drag,
            at: Point::new(1.0, 2.0),
            button: PointerButton::Middle,
        });
        let bytes = encode(&pointer).unwrap();
        assert_eq!((bytes[0], bytes[1], bytes[10]), (5, 2, 2));
        assert_eq!(encode(&Area::Right).unwrap(), [3]);
    }

    #[test]
    fn areas_are_lowercase_in_manifests() {
        let spec: TabSpec =
            serde_json::from_str(r#"{"id": "tuner", "title": "Tuner", "area": "bottom"}"#).unwrap();
        assert_eq!(spec.area, Area::Bottom);
    }

    #[test]
    fn contents_that_cannot_be_shown_say_why() {
        let row = |widgets| TabContent {
            rows: vec![Row { widgets }],
        };
        let twice = row(vec![button("go"), button("go")]);
        assert!(twice.problems()[0].contains("used twice"));
        let bad_id = row(vec![button("Go!")]);
        assert!(bad_id.problems()[0].contains("not a valid id"));
        let slider = row(vec![Widget::Slider {
            id: "s".into(),
            value: 0.0,
            min: 1.0,
            max: 1.0,
            enabled: true,
        }]);
        assert!(slider.problems()[0].contains("min < max"));
        let choice = row(vec![Widget::Choice {
            id: "c".into(),
            options: vec!["a".into()],
            selected: 1,
            enabled: true,
        }]);
        assert!(choice.problems()[0].contains("option 1 of 1"));
        let canvas = row(vec![Widget::Canvas(Canvas {
            id: "c".into(),
            width: 0.0,
            height: 10.0,
            items: vec![],
        })]);
        assert!(canvas.problems()[0].contains("wider"));
    }
}
