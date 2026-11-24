// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A plugin's own UI, in Slint (interface 1.15; #346).
//!
//! A plugin ships one `.slint` file ([`crate::message::Request::Ui`]); the
//! core compiles it with Slint's interpreter and creates its components
//! where the manifest's [`ViewSpec`]s say: a window of its own, a panel in
//! one of the window's areas, a section of the track dialog, or an export's
//! options. The core draws nothing of its own in them. The plugin sets the
//! components' properties ([`crate::message::HostRequest::SetView`]) and
//! hears their callbacks ([`crate::message::Request::ViewCallback`]), with
//! their values as [`Value`]s.

use serde::{Deserialize, Serialize};

/// Where a view is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    /// A window of its own, opened and closed by the plugin
    /// ([`crate::message::HostRequest::ShowView`],
    /// [`crate::message::HostRequest::CloseView`]) or closed by the user.
    /// Its component inherits `Window` (or `Dialog`).
    Window,
    /// A tab of the area above the page.
    Top,
    /// A tab of the area below the page.
    Bottom,
    /// A tab of the area left of the page.
    Left,
    /// A tab of the area right of the page.
    Right,
    /// A section of the track dialog, for each track its plugin has a track
    /// section for (the manifest's `track_sections` with the same id).
    Track,
    /// The options of one of the plugin's exporters, in the export dialog
    /// (an exporter's `options` names the view).
    Export,
}

impl Place {
    /// Whether the view is a panel in one of the window's areas.
    pub fn is_area(self) -> bool {
        matches!(
            self,
            Place::Top | Place::Bottom | Place::Left | Place::Right
        )
    }
}

/// A view, as the manifest declares it (`views`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewSpec {
    /// The id requests name it by, unique in the plugin.
    pub id: String,
    /// The exported component of the plugin's `.slint` file that it shows.
    pub component: String,
    /// Where it is shown.
    pub place: Place,
    /// Its name on a tab or a section's heading; a window takes its own
    /// title from its component.
    #[serde(default)]
    pub title: String,
}

/// A value of a property or a callback's argument. A callback's
/// enumeration values and colours come as text (`center`, `#d32f2fff`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// A `bool`.
    Bool(bool),
    /// An `int`, `float`, length or duration, as Slint's interpreter has it.
    Number(f64),
    /// A `string`.
    String(String),
    /// An array, a model to the UI.
    List(Vec<Value>),
    /// A struct, by field name.
    Struct(Vec<(String, Value)>),
    /// An `image`: `columns` × `rows` premultiplied RGBA.
    Image {
        /// Its width in pixels.
        columns: u32,
        /// Its height in pixels.
        rows: u32,
        /// Its pixels, four bytes each, row by row.
        rgba: Vec<u8>,
    },
}

impl Value {
    /// What keeps the core from showing it: an image whose bytes don't
    /// match its size, or larger than [`IMAGE_MAX`] a side.
    pub fn problem(&self) -> Option<String> {
        match self {
            Value::Image {
                columns,
                rows,
                rgba,
            } => {
                if *columns == 0 || *rows == 0 || *columns > IMAGE_MAX || *rows > IMAGE_MAX {
                    Some(format!(
                        "an image must be 1 to {IMAGE_MAX} pixels a side, not {columns} × {rows}"
                    ))
                } else if rgba.len() != *columns as usize * *rows as usize * 4 {
                    Some(format!(
                        "an image of {columns} × {rows} needs {} bytes, not {}",
                        *columns as usize * *rows as usize * 4,
                        rgba.len()
                    ))
                } else {
                    None
                }
            }
            Value::List(items) => items.iter().find_map(Value::problem),
            Value::Struct(fields) => fields.iter().find_map(|(_, v)| v.problem()),
            _ => None,
        }
    }
}

/// The largest side, in pixels, of an image in a [`Value`].
pub const IMAGE_MAX: u32 = 4096;

/// A track whose section a [`Place::Track`] view shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackView {
    /// The track.
    pub track: crate::message::TrackInfo,
    /// The track's settings for the plugin as they stand in the dialog, a
    /// JSON object as text.
    pub settings: String,
}

/// Whether `name` can be a Slint component's name: a letter or `_`, then
/// letters, digits, `_` and `-`.
pub fn is_component_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_reads_from_the_manifest() {
        let v: ViewSpec = serde_json::from_str(
            r#"{"id": "chat", "component": "Chat", "place": "right", "title": "Chat"}"#,
        )
        .unwrap();
        assert_eq!(v.place, Place::Right);
        assert!(v.place.is_area());
        let w: ViewSpec =
            serde_json::from_str(r#"{"id": "m", "component": "M", "place": "window"}"#).unwrap();
        assert_eq!((w.place, w.title.as_str()), (Place::Window, ""));
        assert!(!w.place.is_area());
    }

    #[test]
    fn images_are_checked_inside_lists_and_structs() {
        let bad = Value::Image {
            columns: 2,
            rows: 2,
            rgba: vec![0; 15],
        };
        assert!(bad.problem().is_some());
        assert!(
            Value::List(vec![Value::Struct(vec![("i".into(), bad)])])
                .problem()
                .is_some()
        );
        let good = Value::Image {
            columns: 2,
            rows: 2,
            rgba: vec![0; 16],
        };
        assert_eq!(good.problem(), None);
        assert!(is_component_name("ManagerWindow") && is_component_name("my-view"));
        assert!(!is_component_name("") && !is_component_name("9a") && !is_component_name("a b"));
    }
}
