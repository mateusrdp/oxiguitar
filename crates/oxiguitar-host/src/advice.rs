// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Advice: the dialogs a plugin (or the core itself) asks the window to
//! show, once, with a "Don't show this again" box (plugin interface 1.4,
//! [`HostRequest::Advise`](oxiguitar_plugin_api::message::HostRequest::Advise)).
//!
//! What the user silenced is kept in settings.json, one entry per plugin
//! and advice: `"advice": {"<plugin id>": {"<advice id>": "hidden"}}`. The
//! core's own advice has the plugin id `oxiguitar`. Nothing here opens a
//! link or shows a dialog: the window does, when the user clicks.

use oxiguitar_plugin_api::message::Advice;
use serde_json::{Map, Value};

use crate::settings::SharedSettings;

/// The settings.json key holding what the user silenced.
pub const KEY: &str = "advice";

/// The plugin id the core's own advice is kept under.
pub const CORE: &str = "oxiguitar";

/// The value kept for a silenced advice.
const HIDDEN: &str = "hidden";

/// An advice somebody asked the window to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    /// The id of the plugin that asked ([`CORE`] for the core).
    pub plugin: String,
    /// The plugin's name, for the dialog to say who is talking.
    pub plugin_name: String,
    /// What it says.
    pub advice: Advice,
}

impl Asked {
    /// The core's own advice.
    pub fn core(advice: Advice) -> Asked {
        Asked {
            plugin: CORE.to_owned(),
            plugin_name: "OxiGuitar".to_owned(),
            advice,
        }
    }
}

/// Whether the user silenced the advice `advice` of `plugin`.
pub fn is_hidden(settings: &SharedSettings, plugin: &str, advice: &str) -> bool {
    settings
        .get(KEY)
        .as_ref()
        .and_then(|v| v.get(plugin))
        .and_then(|v| v.get(advice))
        .and_then(Value::as_str)
        == Some(HIDDEN)
}

/// Silences the advice `advice` of `plugin` for good (until the user edits
/// settings.json), and saves settings.json. Everything else under
/// `advice` stays as it is. The error is one sentence for the footer.
pub fn hide(settings: &SharedSettings, plugin: &str, advice: &str) -> Result<(), String> {
    settings.update(KEY, |old| {
        let mut all = match old {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        let mut mine = match all.get(plugin) {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        mine.insert(advice.to_owned(), Value::String(HIDDEN.to_owned()));
        all.insert(plugin.to_owned(), Value::Object(mine));
        Value::Object(all)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn hiding_one_advice_keeps_the_others() {
        let settings = SharedSettings::new(Settings::default(), None);
        assert!(!is_hidden(&settings, "oxiguitar.midi", "no-sound-sets"));
        hide(&settings, "oxiguitar.midi", "no-sound-sets").unwrap();
        hide(&settings, "oxiguitar.nam", "no-model").unwrap();
        hide(&settings, "oxiguitar.nam", "no-cabinet").unwrap();
        assert!(is_hidden(&settings, "oxiguitar.midi", "no-sound-sets"));
        assert!(is_hidden(&settings, "oxiguitar.nam", "no-model"));
        assert!(is_hidden(&settings, "oxiguitar.nam", "no-cabinet"));
        assert!(!is_hidden(&settings, "oxiguitar.midi", "no-model"));
        assert!(!is_hidden(&settings, "oxiguitar", "no-model"));
        assert_eq!(
            settings.get(KEY),
            Some(serde_json::json!({
                "oxiguitar.midi": {"no-sound-sets": "hidden"},
                "oxiguitar.nam": {"no-model": "hidden", "no-cabinet": "hidden"},
            }))
        );
    }

    #[test]
    fn a_damaged_entry_is_replaced_and_other_keys_stay() {
        let settings = SharedSettings::new(
            Settings::parse(r#"{"view": {"x": 1}, "advice": {"a": 5, "b": {"c": "hidden"}}}"#)
                .unwrap(),
            None,
        );
        assert!(!is_hidden(&settings, "a", "c"));
        assert!(is_hidden(&settings, "b", "c"));
        hide(&settings, "a", "c").unwrap();
        assert!(is_hidden(&settings, "a", "c") && is_hidden(&settings, "b", "c"));
        assert_eq!(settings.get("view"), Some(serde_json::json!({"x": 1})));
        let broken = SharedSettings::new(Settings::parse(r#"{"advice": []}"#).unwrap(), None);
        assert_eq!(broken.snapshot().problems().len(), 1);
        hide(&broken, "oxiguitar", "no-format-plugin").unwrap();
        assert!(is_hidden(&broken, "oxiguitar", "no-format-plugin"));
        assert!(broken.snapshot().problems().is_empty());
    }

    #[test]
    fn it_is_saved_to_settings_json() {
        let dir = std::env::temp_dir().join(format!("oxg-advice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let settings = SharedSettings::new(Settings::default(), Some(path.clone()));
        hide(&settings, "oxiguitar", "no-player-plugin").unwrap();
        let back = Settings::load(&path).unwrap();
        assert_eq!(
            back.get(KEY),
            Some(&serde_json::json!({"oxiguitar": {"no-player-plugin": "hidden"}}))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
