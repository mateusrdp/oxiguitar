// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the plugin folders hold, and what that means for the user.
//!
//! A plugin is one `.oxp` file (docs/design/plugins.md, "Plugin files").
//! Until the plugin interface exists (M2), the host only finds the files:
//! it reads none of them and loads none. The readers of M1 are still built
//! into the core until they move into plugins, so the caller says which
//! formats the core can read and write by itself.

use std::path::{Path, PathBuf};

/// A plugin file's extension, without the dot.
pub const PLUGIN_EXTENSION: &str = "oxp";

/// Where our own plugins are published.
pub const PLUGINS_URL: &str = "https://github.com/mateusrdp/oxiguitar/releases";

/// How much a notice matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Something worth knowing.
    Info,
    /// Something doesn't work until the user does something.
    Warning,
    /// Something went wrong.
    Error,
}

/// A message for the footer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// How much it matters.
    pub level: Level,
    /// One or two sentences.
    pub text: String,
}

impl Notice {
    /// A notice of the given level.
    pub fn new(level: Level, text: impl Into<String>) -> Notice {
        Notice {
            level,
            text: text.into(),
        }
    }
}

/// What the core can do and what the plugin folders hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    /// The folders searched, in order.
    pub folders: Vec<PathBuf>,
    /// Plugin files found, in folder order and then by name.
    pub files: Vec<PathBuf>,
    /// Formats read without a plugin (M1's readers, until they move out).
    pub built_in_readers: Vec<String>,
    /// Formats written without a plugin.
    pub built_in_writers: Vec<String>,
    /// Formats read through plugins (none until plugins can be loaded).
    pub plugin_readers: Vec<String>,
    /// Formats written through plugins.
    pub plugin_writers: Vec<String>,
    /// Output plugins (instruments) for the player.
    pub instruments: Vec<String>,
}

impl Inventory {
    /// Looks through `folders` for plugin files. Folders that don't exist
    /// are skipped: a system folder is there only once a package installs
    /// it.
    pub fn scan(folders: Vec<PathBuf>) -> Inventory {
        let files = folders.iter().flat_map(|f| plugin_files(f)).collect();
        Inventory {
            folders,
            files,
            ..Inventory::default()
        }
    }

    /// Every format OxiGuitar can open.
    pub fn readers(&self) -> impl Iterator<Item = &str> {
        self.built_in_readers
            .iter()
            .chain(&self.plugin_readers)
            .map(String::as_str)
    }

    /// Every format OxiGuitar can save.
    pub fn writers(&self) -> impl Iterator<Item = &str> {
        self.built_in_writers
            .iter()
            .chain(&self.plugin_writers)
            .map(String::as_str)
    }

    /// The footer's messages about plugins at start, least important first,
    /// so the one that matters most is the line the footer shows. Nothing
    /// is said when everything is there. `user_plugins` is the folder the
    /// messages tell the user to put plugins in.
    pub fn notices(&self, user_plugins: &Path) -> Vec<Notice> {
        let mut out = Vec::new();
        if !self.files.is_empty() {
            let n = self.files.len();
            out.push(Notice::new(
                Level::Info,
                format!(
                    "Found {n} plugin file{} ({}). This version can't load plugins yet; loading arrives with the plugin system (M2).",
                    if n == 1 { "" } else { "s" },
                    self.files
                        .iter()
                        .filter_map(|p| p.file_name())
                        .map(|n| n.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        let get = format!(
            "Download plugins from {PLUGINS_URL} and put them in {}.",
            user_plugins.display()
        );
        let (reads, writes) = (
            self.readers().next().is_some(),
            self.writers().next().is_some(),
        );
        if !reads && !writes {
            out.push(Notice::new(
                Level::Warning,
                format!("No read/write plugins are installed, so OxiGuitar can't open or save any file. {get}"),
            ));
        } else if !reads {
            out.push(Notice::new(
                Level::Warning,
                format!("No plugin that reads files is installed, so OxiGuitar can't open any file. {get}"),
            ));
        }
        if self.instruments.is_empty() {
            out.push(Notice::new(
                Level::Warning,
                format!("No output plugin is installed, so the player is off. {get}"),
            ));
        }
        out
    }
}

/// The plugin files directly in `folder`, by name. Unreadable folders give
/// none: a missing system folder is normal.
fn plugin_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(PLUGIN_EXTENSION))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders::tests::scratch;

    #[test]
    fn scan_finds_plugin_files_only() {
        let root = scratch("scan");
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        for f in ["gp5.oxp", "notes.txt", "MIDI.OXP"] {
            std::fs::write(a.join(f), b"").unwrap();
        }
        std::fs::create_dir_all(a.join("folder.oxp")).unwrap();
        std::fs::write(b.join("pdf.oxp"), b"").unwrap();
        let inv = Inventory::scan(vec![a.clone(), root.join("missing"), b.clone()]);
        assert_eq!(
            inv.files,
            vec![a.join("MIDI.OXP"), a.join("gp5.oxp"), b.join("pdf.oxp")]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_installed_warns_about_files_and_output() {
        let inv = Inventory::default();
        let notices = inv.notices(Path::new("/home/mat/.local/share/oxiguitar/plugins"));
        assert_eq!(notices.len(), 2);
        assert!(notices.iter().all(|n| n.level == Level::Warning));
        assert!(notices[0].text.starts_with("No read/write plugins"));
        assert!(notices[1].text.starts_with("No output plugin"));
        assert!(notices[1].text.contains(PLUGINS_URL));
        assert!(
            notices[1]
                .text
                .contains("/home/mat/.local/share/oxiguitar/plugins")
        );
    }

    #[test]
    fn built_in_readers_count_until_they_move_into_plugins() {
        let inv = Inventory {
            built_in_readers: vec!["Guitar Pro 5".to_owned()],
            instruments: vec!["MIDI".to_owned()],
            ..Inventory::default()
        };
        assert!(inv.notices(Path::new("p")).is_empty());
        let readers_only = Inventory {
            plugin_writers: vec!["Guitar Pro 7–8".to_owned()],
            instruments: vec!["MIDI".to_owned()],
            ..Inventory::default()
        };
        let n = readers_only.notices(Path::new("p"));
        assert_eq!(n.len(), 1);
        assert!(n[0].text.starts_with("No plugin that reads files"));
    }

    #[test]
    fn found_files_are_named() {
        let inv = Inventory {
            files: vec![PathBuf::from("/p/gp5.oxp")],
            built_in_readers: vec!["Guitar Pro 5".to_owned()],
            ..Inventory::default()
        };
        let n = inv.notices(Path::new("/p"));
        assert_eq!(n[0].level, Level::Info);
        assert!(n[0].text.contains("1 plugin file (gp5.oxp)"));
    }
}
