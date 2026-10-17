// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar core's plugin host (docs/design/plugins.md).
//!
//! Today it knows where things live and what is there: the user folder
//! with settings.json, the plugin folders, and the plugin files in them
//! ([`Folders`], [`Settings`], [`Inventory`]). Reading plugin manifests,
//! loading plugins on first use and choosing services come with the plugin
//! interface in M2 (ADR 0008).
//!
//! Nothing here touches the network, and nothing runs on a timer.

mod folders;
mod inventory;
mod settings;

pub use folders::{Folders, HOME_VARIABLE, SETTINGS_FILE, System};
pub use inventory::{Inventory, Level, Notice, PLUGIN_EXTENSION, PLUGINS_URL};
pub use settings::Settings;

/// What the app found when it started.
#[derive(Clone, Debug, PartialEq)]
pub struct Startup {
    /// OxiGuitar's folders; `None` if the system names no home folder.
    pub folders: Option<Folders>,
    /// The settings read from settings.json (empty if it couldn't be read).
    pub settings: Settings,
    /// The plugin folders' contents and what the core can do.
    pub inventory: Inventory,
    /// Messages for the footer, least important first.
    pub notices: Vec<Notice>,
}

/// Starts the host for the app: makes sure the user folder, its plugin
/// folder and settings.json exist (creating what is missing), reads the
/// settings and looks through the plugin folders. `built_in_readers` and
/// `built_in_writers` name the formats the core still handles by itself.
pub fn start(
    folders: Option<Folders>,
    built_in_readers: Vec<String>,
    built_in_writers: Vec<String>,
) -> Startup {
    let mut notices = Vec::new();
    let Some(folders) = folders else {
        notices.push(Notice::new(
            Level::Error,
            format!("No home folder: set HOME (or {HOME_VARIABLE}) so OxiGuitar can keep its settings and find plugins."),
        ));
        let inventory = Inventory {
            built_in_readers,
            built_in_writers,
            ..Inventory::default()
        };
        notices.extend(inventory.notices(std::path::Path::new(".")));
        return Startup {
            folders: None,
            settings: Settings::default(),
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
        Ok(s) => s,
        Err(e) => {
            notices.push(Notice::new(
                Level::Error,
                format!("{e} The file was left as it is; default settings are used."),
            ));
            Settings::default()
        }
    };
    notices.extend(
        settings
            .problems()
            .into_iter()
            .map(|p| Notice::new(Level::Warning, p)),
    );
    let searched = settings
        .plugin_folders()
        .unwrap_or_else(|| folders.default_plugin_folders());
    let inventory = Inventory {
        built_in_readers,
        built_in_writers,
        ..Inventory::scan(searched)
    };
    notices.extend(inventory.notices(&folders.user_plugins));
    Startup {
        folders: Some(folders),
        settings,
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
        let s = start(folders, vec!["Guitar Pro 5".to_owned()], Vec::new());
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
        let s = start(None, Vec::new(), Vec::new());
        assert_eq!(s.notices[0].level, Level::Error);
        assert!(s.folders.is_none());
    }
}
