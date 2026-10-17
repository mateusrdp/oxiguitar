// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! settings.json: the user's settings, shared by the core and its plugins.
//!
//! The file is a JSON object. The core reads the keys it knows and keeps
//! every other key as it is, so plugins and later versions can add their
//! own. Today the core reads one key:
//!
//! - `plugin_folders`: a list of folders to search for plugins, in order.
//!   It replaces the default list (the user's plugin folder, then the
//!   system's).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The contents of settings.json.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    values: Map<String, Value>,
}

impl Settings {
    /// Reads settings.json. A missing file gives empty settings; a file
    /// that isn't a JSON object is an error naming the file and the
    /// problem, and is left untouched.
    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(format!("Cannot read {}: {e}.", path.display())),
        };
        Settings::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Parses settings from text. An empty file counts as `{}`.
    pub fn parse(text: &str) -> Result<Settings, String> {
        if text.trim().is_empty() {
            return Ok(Settings::default());
        }
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(values)) => Ok(Settings { values }),
            Ok(_) => Err("settings must be a JSON object, such as {}.".to_owned()),
            Err(e) => Err(format!("not valid JSON ({e}).")),
        }
    }

    /// The folders settings.json lists under `plugin_folders`, or `None`
    /// if it lists none (then the defaults apply). Entries that aren't
    /// strings are skipped; they are reported by [`Settings::problems`].
    pub fn plugin_folders(&self) -> Option<Vec<PathBuf>> {
        let list = self.values.get("plugin_folders")?.as_array()?;
        Some(
            list.iter()
                .filter_map(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .collect(),
        )
    }

    /// Keys the core reads but whose values it can't use, as sentences.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.values.get("plugin_folders") {
            None => {}
            Some(Value::Array(items)) => {
                if items.iter().any(|v| !v.is_string()) {
                    out.push("settings.json: plugin_folders should list folder names as text; other entries are ignored.".to_owned());
                }
            }
            Some(_) => out.push(
                "settings.json: plugin_folders should be a list of folders, such as [\"/opt/oxiguitar/plugins\"]; the default folders are used."
                    .to_owned(),
            ),
        }
        out
    }

    /// A value by key, for keys the core doesn't read itself.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_settings_mean_the_defaults() {
        for text in ["", "{}", "  {}\n"] {
            let s = Settings::parse(text).unwrap();
            assert_eq!(s.plugin_folders(), None);
            assert!(s.problems().is_empty());
        }
    }

    #[test]
    fn plugin_folders_replace_the_defaults() {
        let s =
            Settings::parse(r#"{"plugin_folders": ["/opt/og", "", 3], "other": true}"#).unwrap();
        assert_eq!(s.plugin_folders(), Some(vec![PathBuf::from("/opt/og")]));
        assert_eq!(s.problems().len(), 1);
        assert_eq!(s.get("other"), Some(&Value::Bool(true)));
        let wrong = Settings::parse(r#"{"plugin_folders": "/opt/og"}"#).unwrap();
        assert_eq!(wrong.plugin_folders(), None);
        assert_eq!(wrong.problems().len(), 1);
    }

    #[test]
    fn a_file_that_is_not_an_object_is_an_error() {
        assert!(Settings::parse("[]").is_err());
        assert!(Settings::parse("{").unwrap_err().contains("not valid JSON"));
        let missing = Settings::load(Path::new("/nonexistent/oxiguitar/settings.json"));
        assert_eq!(missing, Ok(Settings::default()));
    }
}
