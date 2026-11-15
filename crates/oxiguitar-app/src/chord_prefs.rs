// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the chord dialog keeps in settings.json (#278). No window code
//! here.
//!
//! - `chord-styles`: the chord notation styles, each a name and its
//!   aliases, `{"Compact": {"m7b5": "ø", "aug": "+"}, "Custom": {}}`. A
//!   key is a kind of chord in any spelling, its value what the style
//!   writes for it (`oxiguitar_edit::chord_names`). The styles are always
//!   read from there, so they can be changed without building OxiGuitar
//!   again (#278); the ones in `chord-styles.json` are
//!   only what a settings.json without the key is given.
//! - `chord`: the style last picked and whether "Add diagram" was ticked,
//!   used again for the next chord.

use oxiguitar_edit::chord_names::{Style, Styles};
use serde_json::{Map, Value, json};

/// The settings key of the dialog's own choices.
pub(crate) const KEY: &str = "chord";
/// The settings key of the styles.
pub(crate) const STYLES_KEY: &str = "chord-styles";
/// The styles a settings.json without [`STYLES_KEY`] is given.
const FIRST_STYLES: &str = include_str!("chord-styles.json");
/// The style a new user starts with, and the one that changes nothing.
const FIRST_STYLE: &str = "Popular";
const PLAIN_STYLE: &str = "Custom";

/// The dialog's choices that last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChordPrefs {
    /// The style's name.
    pub(crate) style: String,
    /// Whether a new chord gets its diagram.
    pub(crate) diagram: bool,
}

impl Default for ChordPrefs {
    fn default() -> ChordPrefs {
        ChordPrefs {
            style: FIRST_STYLE.to_owned(),
            diagram: true,
        }
    }
}

impl ChordPrefs {
    /// Reads the `chord` value; anything missing or of the wrong type
    /// keeps its default.
    pub(crate) fn from_value(value: Option<&Value>) -> ChordPrefs {
        let d = ChordPrefs::default();
        let Some(Value::Object(map)) = value else {
            return d;
        };
        ChordPrefs {
            style: map
                .get("style")
                .and_then(Value::as_str)
                .map_or(d.style, str::to_owned),
            diagram: map
                .get("diagram")
                .and_then(Value::as_bool)
                .unwrap_or(d.diagram),
        }
    }

    /// The `chord` value for these choices. Keys a later version added to
    /// `old` are kept.
    pub(crate) fn to_value(&self, old: Option<&Value>) -> Value {
        let mut map = match old {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        map.insert("style".into(), json!(self.style));
        map.insert("diagram".into(), json!(self.diagram));
        Value::Object(map)
    }

    /// This style among `styles`: the one of its name, else the one that
    /// changes nothing, else the first.
    pub(crate) fn style_in(&self, styles: &Styles) -> usize {
        styles
            .position(&self.style)
            .or_else(|| styles.position(PLAIN_STYLE))
            .unwrap_or(0)
    }
}

/// The styles a settings.json without the key is given, as its value.
pub(crate) fn first_styles() -> Value {
    serde_json::from_str(FIRST_STYLES).unwrap_or_else(|_| json!({ PLAIN_STYLE: {} }))
}

/// The styles of a `chord-styles` value, in its order. A style that is
/// no object, or an alias that is no text, is left out.
pub(crate) fn styles(value: &Value) -> Styles {
    let Value::Object(map) = value else {
        return Styles::default();
    };
    Styles::new(
        map.iter()
            .filter_map(|(name, aliases)| {
                let aliases = aliases.as_object()?;
                Some(Style::new(
                    name,
                    aliases
                        .iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))),
                ))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn the_first_styles_read_back_what_they_write() {
        let value = first_styles();
        let all = styles(&value);
        assert_eq!(all.names(), ["Compact", "Custom", "Jazz", "Popular"]);
        let map = value.as_object().unwrap();
        // Every key is a kind of chord, and each style's text for it is
        // read as that kind again, so a song goes from any style to any.
        for (s, name) in all.names().iter().enumerate() {
            for (key, text) in map[name].as_object().unwrap() {
                let kind = oxiguitar_edit::chord_names::read(&format!("C{key}"))
                    .unwrap_or_else(|| panic!("{name}: {key}"))
                    .kind;
                let written = format!("C{}", text.as_str().unwrap());
                assert_eq!(all.write(s, &format!("C{key}")), written, "{name}: {key}");
                for other in 0..all.names().len() {
                    let read = all.read(other, &written);
                    assert_eq!(read.map(|r| r.kind), Some(kind.clone()), "{name}: {key}");
                }
            }
        }
        // #264's examples.
        let (compact, custom, jazz, popular) = (0, 1, 2, 3);
        assert_eq!(all.write(popular, "Em7(b5)"), "Em7b5");
        assert_eq!(all.write(compact, "Em7-5"), "Eø");
        assert_eq!(all.write(jazz, "Cmaj7"), "C△7");
        assert_eq!(all.write(jazz, "Fmin"), "F-");
        assert_eq!(all.write(compact, "Daug"), "D+");
        assert_eq!(all.write(popular, "G°7"), "Gdim7");
        assert_eq!(all.write(custom, "Em7(b5)"), "Em7(b5)");
    }

    #[test]
    fn choices_round_trip_and_keep_unknown_keys() {
        let d = ChordPrefs::from_value(None);
        assert_eq!((d.style.as_str(), d.diagram), ("Popular", true));
        assert_eq!(ChordPrefs::from_value(Some(&json!({"diagram": "no"}))), d);
        let p = ChordPrefs {
            style: "Jazz".into(),
            diagram: false,
        };
        let v = p.to_value(Some(&json!({"later": 1})));
        assert_eq!(v, json!({"style": "Jazz", "diagram": false, "later": 1}));
        assert_eq!(ChordPrefs::from_value(Some(&v)), p);
    }

    #[test]
    fn a_style_that_is_gone_falls_back() {
        let all = styles(&json!({"Custom": {}, "Mine": {"m": "min"}, "Odd": 3}));
        assert_eq!(all.names(), ["Custom", "Mine"]);
        let pick = |style: &str| {
            ChordPrefs {
                style: style.into(),
                diagram: true,
            }
            .style_in(&all)
        };
        assert_eq!((pick("Mine"), pick("Jazz")), (1, 0));
        assert_eq!(styles(&json!([])).names(), Vec::<String>::new());
    }
}
