// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin manager (Tools → Plugins; docs/design/plugins.md, "The plugin
//! manager"): every plugin file in the plugin folders, the user's and the
//! system's, and a tickbox to enable or disable each from the next start.
//! No window code here, so it is unit-tested.
//!
//! A plugin can't be unloaded while OxiGuitar runs (its code may have
//! threads running or have handed the host pointers into itself; ADR 0008),
//! so every change waits for the next start, and the manager says so.
//!
//! It only reads the folders and writes `disabled` in settings.json: it
//! never copies, downloads, installs or removes a file, and draws no
//! settings forms. It makes no distinction between our plugins and anyone
//! else's (Mateus, 18 Oct 2026).

use oxiguitar_host::{DISABLED_KEY, Found, Plugins, SharedSettings, State};
use serde_json::Value;

/// One plugin file, as the manager lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    /// The plugin's name, or the file's name when its manifest can't be
    /// read.
    pub(crate) name: String,
    /// Its version, or "" when its manifest can't be read.
    pub(crate) version: String,
    /// The file's name, and the folder it is in.
    pub(crate) file: String,
    pub(crate) folder: String,
    /// What became of it, in a sentence.
    pub(crate) state: String,
    /// The plugin's id when it can be enabled or disabled: its manifest
    /// reads, and nothing else keeps it from being used.
    pub(crate) id: Option<String>,
    /// Enabled in settings.json now, which takes effect at the next start.
    pub(crate) enabled: bool,
    /// The tick differs from how this session started: a restart applies it.
    pub(crate) changed: bool,
    /// Loaded now.
    pub(crate) loaded: bool,
    /// The plugin's id when it is in use and has a settings window
    /// (interface 1.3): the row's Settings… button opens it.
    pub(crate) settings: Option<String>,
}

/// Whether any change waits for OxiGuitar to start again.
pub(crate) fn restart_needed(rows: &[Row]) -> bool {
    rows.iter().any(|r| r.changed)
}

/// The ids settings.json lists under `disabled` now.
pub(crate) fn disabled(settings: &SharedSettings) -> Vec<String> {
    match settings.get(DISABLED_KEY) {
        Some(Value::Array(ids)) => ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every plugin file `plugins` found, in folder order and then by name.
/// `disabled` is settings.json's list now, which may differ from the one
/// this session started with.
pub(crate) fn rows(plugins: &Plugins, disabled: &[String]) -> Vec<Row> {
    plugins
        .found()
        .iter()
        .map(|f| {
            let mut r = row(f, plugins.safe_mode(), disabled);
            let has_window = plugins
                .with_settings_window()
                .iter()
                .any(|w| std::ptr::eq(*w, f));
            if has_window {
                r.settings = f.manifest.as_ref().map(|m| m.id.clone());
            }
            r
        })
        .collect()
}

fn row(f: &Found, safe: bool, disabled: &[String]) -> Row {
    let file = f
        .path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let folder = f
        .path
        .parent()
        .map_or_else(String::new, |p| p.display().to_string());
    let id = f.manifest.as_ref().map(|m| m.id.clone());
    let toggles = matches!(f.state, State::Ready | State::Disabled | State::Replaced(_));
    let enabled = id.as_ref().is_some_and(|id| !disabled.contains(id));
    let mut state = match &f.state {
        // A system library it needs isn't installed (`Plugins`' quiet
        // withdrawal): it can't run here, rather than having failed.
        State::Ready if f.failure().is_some_and(|w| w.starts_with("it needs ")) => {
            format!("Can't run here: {}.", f.failure().unwrap_or_default())
        }
        State::Ready if f.failure().is_some() => format!(
            "Stopped working and not used until OxiGuitar restarts: {}.",
            f.failure().unwrap_or_default()
        ),
        State::Ready if safe => "Not loaded: OxiGuitar was started in safe mode.".to_owned(),
        State::Ready if f.is_loaded() => "Loaded.".to_owned(),
        State::Ready => "Not loaded yet: it loads when it is first needed.".to_owned(),
        State::Disabled => "Disabled.".to_owned(),
        State::Replaced(by) => format!(
            "Not used: {} holds the same plugin at a higher or equal version.",
            by.display()
        ),
        State::Unusable(why) => format!("Can't be used: {why}."),
    };
    // A change made in this session waits for the next start: plugins are
    // never unloaded.
    let was_enabled = !matches!(f.state, State::Disabled);
    let changed = toggles && enabled != was_enabled;
    if changed {
        state = match (enabled, f.is_loaded()) {
            (false, true) => {
                "Loaded: it stays in use until OxiGuitar restarts, and is disabled from then on."
                    .to_owned()
            }
            (false, false) => format!("{state} Disabled when OxiGuitar restarts."),
            (true, _) => format!("{state} Enabled when OxiGuitar restarts."),
        };
    }
    Row {
        name: f.name(),
        version: f
            .manifest
            .as_ref()
            .map_or_else(String::new, |m| m.version.to_string()),
        file,
        folder,
        state,
        id: if toggles { id } else { None },
        enabled,
        changed,
        loaded: f.is_loaded(),
        settings: None,
    }
}

/// Enables (`on`) or disables the plugin `id` from the next start, in
/// settings.json's `disabled` list, keeping every other entry as it is.
/// When settings.json couldn't be read at start it is never written, and
/// the error says so: the change would be lost.
pub(crate) fn set_enabled(settings: &SharedSettings, id: &str, on: bool) -> Result<(), String> {
    if settings.path().is_none() {
        return Err(format!(
            "{} couldn't be read when OxiGuitar started, so it isn't changed: correct it (or remove it) and start OxiGuitar again.",
            oxiguitar_host::SETTINGS_FILE
        ));
    }
    let mut list = match settings.get(DISABLED_KEY) {
        Some(Value::Array(ids)) => ids,
        _ => Vec::new(),
    };
    list.retain(|v| v.as_str() != Some(id));
    if !on {
        list.push(Value::String(id.to_owned()));
    }
    settings.set(DISABLED_KEY, Value::Array(list))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use oxiguitar_host::{Settings, Unheard};

    use super::*;

    /// A folder of plugin files whose manifests are read but which are
    /// never loaded: two copies of the test plugin (hard links where the
    /// system allows) and a file that isn't a plugin.
    fn folder(name: &str) -> std::path::PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let test = crate::test_support::test_oxp();
        let dir = workspace.join("target").join("oxp").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for copy in ["a-test.oxp", "b-test.oxp"] {
            if std::fs::hard_link(test, dir.join(copy)).is_err() {
                std::fs::copy(test, dir.join(copy)).unwrap();
            }
        }
        std::fs::write(dir.join("junk.oxp"), b"not a plugin").unwrap();
        dir
    }

    fn settings(json: &str, file: &Path) -> SharedSettings {
        std::fs::write(file, json).unwrap();
        SharedSettings::new(Settings::load(file).unwrap(), Some(file.to_owned()))
    }

    #[test]
    fn every_file_is_listed_with_what_became_of_it() {
        let dir = folder("manager-list");
        let shared = settings("{}", &dir.join("settings.json"));
        let plugins =
            Plugins::discover(std::slice::from_ref(&dir), shared, "x11", Arc::new(Unheard));
        let rows = rows(&plugins, &[]);
        let names: Vec<(&str, &str, &str)> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.version.as_str(), r.file.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("Test plugin", "0.9.0", "a-test.oxp"),
                ("Test plugin", "0.9.0", "b-test.oxp"),
                ("junk.oxp", "", "junk.oxp"),
            ]
        );
        // The host keeps absolute paths (on Windows `..` is resolved too).
        let shown = std::path::absolute(&dir).unwrap().display().to_string();
        assert!(rows.iter().all(|r| r.folder == shown), "{rows:?}");
        assert_eq!(
            rows[0].state,
            "Not loaded yet: it loads when it is first needed."
        );
        assert!(rows[1].state.starts_with("Not used: ") && rows[1].state.contains("a-test.oxp"));
        assert!(
            rows[2].state.starts_with("Can't be used: "),
            "{}",
            rows[2].state
        );
        // The two copies can be enabled and disabled; the junk can't.
        assert_eq!(rows[0].id.as_deref(), Some("oxiguitar.test"));
        assert_eq!(rows[1].id.as_deref(), Some("oxiguitar.test"));
        assert_eq!(rows[2].id, None);
        assert!(rows[0].enabled && rows[1].enabled && !rows[2].enabled);
        // Listing loads nothing.
        assert!(plugins.found().iter().all(|f| !f.is_loaded()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tick_writes_disabled_for_the_next_start_and_keeps_the_rest() {
        let dir = folder("manager-tick");
        let file = dir.join("settings.json");
        let shared = settings(
            r#"{"disabled": ["someone.else"], "view": {"dark_page": true}}"#,
            &file,
        );
        let plugins = Plugins::discover(
            std::slice::from_ref(&dir),
            shared.clone(),
            "x11",
            Arc::new(Unheard),
        );
        set_enabled(&shared, "oxiguitar.test", false).unwrap();
        let saved = Settings::load(&file).unwrap();
        assert_eq!(
            saved.get(DISABLED_KEY),
            Some(&serde_json::json!(["someone.else", "oxiguitar.test"]))
        );
        assert_eq!(
            saved.get("view"),
            Some(&serde_json::json!({"dark_page": true}))
        );
        // This session goes on with the plugin; the row says what changes.
        let now = rows(&plugins, &disabled(&shared));
        assert!(!now[0].enabled);
        assert_eq!(
            now[0].state,
            "Not loaded yet: it loads when it is first needed. Disabled when OxiGuitar restarts."
        );

        // Started again: disabled. Ticked again: enabled from the next start.
        let plugins = Plugins::discover(
            std::slice::from_ref(&dir),
            shared.clone(),
            "x11",
            Arc::new(Unheard),
        );
        assert_eq!(rows(&plugins, &disabled(&shared))[0].state, "Disabled.");
        set_enabled(&shared, "oxiguitar.test", true).unwrap();
        assert_eq!(disabled(&shared), ["someone.else"]);
        assert_eq!(
            rows(&plugins, &disabled(&shared))[0].state,
            "Disabled. Enabled when OxiGuitar restarts."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_loaded_plugin_stays_in_use_until_a_restart_and_the_manager_says_so() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = workspace.join("target").join("oxp").join("manager-loaded");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A copy of its own: this test loads it.
        std::fs::copy(crate::test_support::test_oxp(), dir.join("test.oxp")).unwrap();
        let shared = settings("{}", &dir.join("settings.json"));
        let plugins = Plugins::discover(
            std::slice::from_ref(&dir),
            shared.clone(),
            "x11",
            Arc::new(Unheard),
        );
        assert!(plugins.load_all().is_empty());
        let now = || rows(&plugins, &disabled(&shared));
        assert_eq!(now()[0].state, "Loaded.");
        assert!(!restart_needed(&now()));

        set_enabled(&shared, "oxiguitar.test", false).unwrap();
        let after = now();
        assert!(after[0].loaded && after[0].changed && !after[0].enabled);
        assert_eq!(
            after[0].state,
            "Loaded: it stays in use until OxiGuitar restarts, and is disabled from then on."
        );
        assert!(restart_needed(&after));
        // Ticked again: nothing waits for a restart any more.
        set_enabled(&shared, "oxiguitar.test", true).unwrap();
        assert!(!restart_needed(&now()));
        assert_eq!(now()[0].state, "Loaded.");
    }

    #[test]
    fn safe_mode_lists_everything_and_loads_nothing() {
        let dir = folder("manager-safe");
        let shared = settings("{}", &dir.join("settings.json"));
        let plugins = Plugins::discover_in_safe_mode(
            std::slice::from_ref(&dir),
            shared.clone(),
            "x11",
            Arc::new(Unheard),
        );
        let rows = rows(&plugins, &[]);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].state,
            "Not loaded: OxiGuitar was started in safe mode."
        );
        // Disabling a plugin still works in safe mode: that is what it is for.
        set_enabled(&shared, "oxiguitar.test", false).unwrap();
        assert_eq!(disabled(&shared), ["oxiguitar.test"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_that_could_not_be_read_are_never_written() {
        let unread = SharedSettings::default();
        assert!(set_enabled(&unread, "oxiguitar.test", false).is_err());
    }
}
