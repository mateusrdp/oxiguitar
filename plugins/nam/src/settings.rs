// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A track's NAM settings, kept per track beside the score (`FILE.oxg`,
//! the track dialog's section):
//!
//! ```json
//! {"on": true, "model": "Amps/Twin.nam", "cabinet": "built-in:mesa-boogie-mark-v",
//!  "input": 0.0, "output": 0.0}
//! ```
//!
//! `cabinet` is `none`, `built-in:NAME`, a `.wav` file's path inside the
//! `nam` folder, or missing: then a model that includes a cabinet plays
//! without one and any other plays through the first built-in response.
//! `input` and `output` are levels in dB, from −24 to +24.

use oxiguitar_plugin::serde_json::{Map, Value, json};

/// The lowest and highest level, in dB.
pub(crate) const LEVELS: (f32, f32) = (-24.0, 24.0);

/// The cabinet a track picked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cabinet {
    /// None picked: chosen from the model (see the module's text).
    Auto,
    /// No cabinet.
    None,
    /// A response built into the plugin, by its file's name.
    BuiltIn(String),
    /// A `.wav` file in the `nam` folder.
    File(String),
}

/// One track's settings.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    pub(crate) on: bool,
    pub(crate) model: Option<String>,
    pub(crate) cabinet: Cabinet,
    /// Before the model, in dB.
    pub(crate) input: f32,
    /// After everything, in dB, on top of the model's own level brought to
    /// −18 dBFS where its file says how loud it is.
    pub(crate) output: f32,
}

impl Default for Track {
    fn default() -> Track {
        Track {
            on: false,
            model: None,
            cabinet: Cabinet::Auto,
            input: 0.0,
            output: 0.0,
        }
    }
}

const BUILT_IN: &str = "built-in:";

fn level(v: Option<&Value>) -> f32 {
    v.and_then(Value::as_f64)
        .map_or(0.0, |d| (d as f32).clamp(LEVELS.0, LEVELS.1))
}

impl Track {
    /// Reads a track's settings; anything missing or of the wrong type
    /// keeps its default.
    pub(crate) fn from_map(map: &Map<String, Value>) -> Track {
        let cabinet = match map.get("cabinet").and_then(Value::as_str) {
            None => Cabinet::Auto,
            Some("none") => Cabinet::None,
            Some(s) if s.starts_with(BUILT_IN) => Cabinet::BuiltIn(s[BUILT_IN.len()..].into()),
            Some(s) => Cabinet::File(s.into()),
        };
        Track {
            on: map.get("on").and_then(Value::as_bool).unwrap_or(false),
            model: map
                .get("model")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(Into::into),
            cabinet,
            input: level(map.get("input")),
            output: level(map.get("output")),
        }
    }

    /// Reads them from the text the player prepares with.
    pub(crate) fn from_text(text: &str) -> Track {
        match oxiguitar_plugin::serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => Track::from_map(&map),
            _ => Track::default(),
        }
    }

    /// Writes them into `map`, keeping keys a later version added.
    pub(crate) fn write(&self, map: &mut Map<String, Value>) {
        map.insert("on".into(), json!(self.on));
        match &self.model {
            Some(m) => map.insert("model".into(), json!(m)),
            None => map.remove("model"),
        };
        match &self.cabinet {
            Cabinet::Auto => map.remove("cabinet"),
            Cabinet::None => map.insert("cabinet".into(), json!("none")),
            Cabinet::BuiltIn(n) => map.insert("cabinet".into(), json!(format!("{BUILT_IN}{n}"))),
            Cabinet::File(f) => map.insert("cabinet".into(), json!(f)),
        };
        map.insert("input".into(), json!(self.input));
        map.insert("output".into(), json!(self.output));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_keep_unknown_keys() {
        let t = Track {
            on: true,
            model: Some("Amps/Twin.nam".into()),
            cabinet: Cabinet::BuiltIn("mesa-boogie-mark-v".into()),
            input: -3.0,
            output: 6.0,
        };
        let mut map = Map::new();
        map.insert("later".into(), json!(1));
        t.write(&mut map);
        assert_eq!(map["cabinet"], json!("built-in:mesa-boogie-mark-v"));
        assert_eq!(map["later"], json!(1));
        assert_eq!(Track::from_map(&map), t);
        let none = Track {
            cabinet: Cabinet::None,
            ..t.clone()
        };
        none.write(&mut map);
        assert_eq!(Track::from_map(&map).cabinet, Cabinet::None);
        Track {
            cabinet: Cabinet::Auto,
            ..t
        }
        .write(&mut map);
        assert!(!map.contains_key("cabinet"));
    }

    #[test]
    fn wrong_values_keep_their_defaults() {
        let t = Track::from_text(r#"{"on": "yes", "model": "", "input": 99, "output": "loud"}"#);
        assert_eq!(
            t,
            Track {
                input: 24.0,
                ..Track::default()
            }
        );
        assert_eq!(Track::from_text("not json"), Track::default());
    }
}
