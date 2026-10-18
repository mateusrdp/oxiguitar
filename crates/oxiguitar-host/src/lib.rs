// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar core's plugin host (docs/design/plugins.md).
//!
//! - Where things live: the user folder with settings.json and the plugin
//!   folders ([`Folders`], [`Settings`], [`SharedSettings`]).
//! - The plugins: their manifests, read at start without loading any code;
//!   each plugin loaded the first time one of its contributions is used and
//!   never unloaded; formats picked by their signatures, instruments by the
//!   classes they play, services by conditions, priority and settings
//!   ([`Plugins`]).
//! - What that means for the user: the footer's notices at start
//!   ([`Inventory`], [`start`]).
//!
//! The loader (`loader.rs`) is the only module with `unsafe` code; the SDK's
//! export glue is its counterpart in plugins. Nothing here touches the
//! network, and nothing runs on a timer.

mod folders;
mod inventory;
mod loader;
mod oxp;
mod plugins;
mod settings;

use std::sync::Arc;

pub use folders::{Folders, HOME_VARIABLE, SETTINGS_FILE, System};
pub use inventory::{Inventory, Level, Notice, PLUGIN_EXTENSION, PLUGINS_URL, plugin_notices};
pub use oxiguitar_plugin_api as api;
pub use plugins::{
    DISABLED_KEY, FormatRef, Found, InstrumentRef, Messages, Plugins, ReadScore, SERVICES_KEY,
    ServiceRef, State, Unheard, WrittenFile,
};
pub use settings::{Settings, SharedSettings};

/// What the app found when it started.
#[derive(Clone, Debug)]
pub struct Startup {
    /// OxiGuitar's folders; `None` if the system names no home folder.
    pub folders: Option<Folders>,
    /// settings.json, shared by the app and the plugins (empty, and never
    /// saved, if it couldn't be read).
    pub settings: SharedSettings,
    /// The plugins found, none of them loaded yet.
    pub plugins: Plugins,
    /// What the core can do with them.
    pub inventory: Inventory,
    /// Messages for the footer, least important first.
    pub notices: Vec<Notice>,
}

/// The display system this program draws with, as plugins' conditions name
/// it: `windows`, `macos`, or on Linux `wayland` or `x11`. Until the two
/// Linux builds (docs/design/plugins.md) each fix their own, Linux follows
/// the windowing library's rule: Wayland when `WAYLAND_DISPLAY` is set.
pub fn display_system() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_vendor = "apple") {
        "macos"
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()) {
        "wayland"
    } else {
        "x11"
    }
}

/// Starts the host for the app: makes sure the user folder, its plugin
/// folder and settings.json exist (creating what is missing), reads the
/// settings and every plugin's manifest, and loads no plugin.
/// `built_in_readers` and `built_in_writers` name the formats the core
/// still handles by itself; `display` is [`display_system`]'s answer, and
/// `messages` shows what plugins say later.
pub fn start(
    folders: Option<Folders>,
    built_in_readers: Vec<String>,
    built_in_writers: Vec<String>,
    display: &str,
    messages: Arc<dyn Messages>,
) -> Startup {
    let mut notices = Vec::new();
    let Some(folders) = folders else {
        notices.push(Notice::new(
            Level::Error,
            format!("No home folder: set HOME (or {HOME_VARIABLE}) so OxiGuitar can keep its settings and find plugins."),
        ));
        let settings = SharedSettings::default();
        let plugins = Plugins::discover(&[], settings.clone(), display, messages);
        let inventory = Inventory {
            built_in_readers,
            built_in_writers,
            ..Inventory::default()
        };
        notices.extend(inventory.notices(std::path::Path::new(".")));
        return Startup {
            folders: None,
            settings,
            plugins,
            inventory,
            notices,
        };
    };
    match folders.ensure() {
        Ok(created) if !created.is_empty() => notices.push(Notice::new(
            Level::Info,
            format!(
                "Created {}.",
                created
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        )),
        Ok(_) => {}
        Err(e) => notices.push(Notice::new(
            Level::Error,
            format!("Cannot create {}: {e}.", folders.user_plugins.display()),
        )),
    }
    let settings = match Settings::load(&folders.settings()) {
        Ok(s) => SharedSettings::new(s, Some(folders.settings())),
        Err(e) => {
            notices.push(Notice::new(
                Level::Error,
                format!("{e} The file was left as it is; default settings are used and changes aren't saved."),
            ));
            SharedSettings::default()
        }
    };
    let snapshot = settings.snapshot();
    notices.extend(
        snapshot
            .problems()
            .into_iter()
            .map(|p| Notice::new(Level::Warning, p)),
    );
    let searched = snapshot
        .plugin_folders()
        .unwrap_or_else(|| folders.default_plugin_folders());
    let plugins = Plugins::discover(&searched, settings.clone(), display, messages);
    notices.extend(plugin_notices(&plugins));
    let inventory = Inventory {
        built_in_readers,
        built_in_writers,
        ..Inventory::of(searched, &plugins)
    };
    notices.extend(inventory.notices(&folders.user_plugins));
    Startup {
        folders: Some(folders),
        settings,
        plugins,
        inventory,
        notices,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn a_first_start_creates_the_folders_and_says_what_is_missing() {
        let root =
            std::env::temp_dir().join(format!("oxiguitar-host-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("og");
        let folders = Folders::resolve(
            System::Linux,
            |n| (n == HOME_VARIABLE).then(|| OsString::from(&home)),
            None,
        );
        let s = start(
            folders,
            vec!["Guitar Pro 5".to_owned()],
            Vec::new(),
            "x11",
            Arc::new(Unheard),
        );
        assert!(home.join("plugins").is_dir());
        assert_eq!(
            std::fs::read_to_string(home.join(SETTINGS_FILE)).unwrap(),
            "{}\n"
        );
        assert_eq!(s.notices[0].level, Level::Info);
        assert!(s.notices[0].text.starts_with("Created "));
        // Built-in readers: no read/write warning, only the missing output.
        assert_eq!(s.notices.len(), 2);
        assert!(s.notices[1].text.starts_with("No output plugin"));
        assert_eq!(s.inventory.folders, vec![home.join("plugins")]);

        // A second start creates nothing; a broken settings file is kept.
        std::fs::write(home.join(SETTINGS_FILE), "{ oops").unwrap();
        let again = start(
            Folders::resolve(
                System::Linux,
                |n| (n == HOME_VARIABLE).then(|| OsString::from(&home)),
                None,
            ),
            Vec::new(),
            Vec::new(),
            "x11",
            Arc::new(Unheard),
        );
        assert_eq!(again.notices[0].level, Level::Error);
        assert!(again.notices[0].text.contains("not valid JSON"));
        assert!(again.notices[1].text.starts_with("No read/write plugins"));
        assert_eq!(
            std::fs::read_to_string(home.join(SETTINGS_FILE)).unwrap(),
            "{ oops"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_home_is_an_error_but_not_fatal() {
        let s = start(None, Vec::new(), Vec::new(), "x11", Arc::new(Unheard));
        assert_eq!(s.notices[0].level, Level::Error);
        assert!(s.folders.is_none());
    }
}
