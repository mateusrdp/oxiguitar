// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the plugin folders hold, and what that means for the user.
//!
//! A plugin is one `.oxp` file (docs/design/plugins.md, "Plugin files").
//! The inventory is what the plugins' manifests say the program can do,
//! without loading any plugin. The core reads and writes no format by
//! itself.

use std::path::{Path, PathBuf};

use crate::plugins::{Plugins, State, file_name};

/// A plugin file's extension, without the dot.
pub const PLUGIN_EXTENSION: &str = "oxp";

/// Where our own plugins are published.
pub const PLUGINS_URL: &str = "https://github.com/mateusrdp/oxiguitar/releases";

/// Where to report a bug or ask for a feature, such as a plugin for a
/// format nobody opens yet.
pub const ISSUES_URL: &str = "https://github.com/mateusrdp/oxiguitar/issues/new";

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
    /// Formats read through plugins.
    pub plugin_readers: Vec<String>,
    /// Formats written through plugins.
    pub plugin_writers: Vec<String>,
    /// Output plugins (instruments) for the player.
    pub instruments: Vec<String>,
}

impl Inventory {
    /// What `plugins`, found in `folders`, can do: the names of the
    /// formats they read and write ("Guitar Pro 5 (.gp5)") and of their
    /// instruments.
    pub fn of(folders: Vec<PathBuf>, plugins: &Plugins) -> Inventory {
        let format = |f: crate::plugins::FormatRef<'_>| match f.spec.extensions.first() {
            Some(ext) => format!("{} (.{ext})", f.spec.name),
            None => f.spec.name.clone(),
        };
        Inventory {
            folders,
            files: plugins.found().iter().map(|f| f.path.clone()).collect(),
            plugin_readers: plugins.readers().into_iter().map(format).collect(),
            plugin_writers: plugins.writers().into_iter().map(format).collect(),
            instruments: plugins
                .instruments()
                .into_iter()
                .map(|i| i.spec.name.clone())
                .collect(),
        }
    }

    /// Every format OxiGuitar can open.
    pub fn readers(&self) -> impl Iterator<Item = &str> {
        self.plugin_readers.iter().map(String::as_str)
    }

    /// Every format OxiGuitar can save.
    pub fn writers(&self) -> impl Iterator<Item = &str> {
        self.plugin_writers.iter().map(String::as_str)
    }

    /// The footer's messages about plugins at start, least important first,
    /// so the one that matters most is the line the footer shows. Nothing
    /// is said when everything is there. `user_plugins` is the folder the
    /// messages tell the user to put plugins in.
    pub fn notices(&self, user_plugins: &Path) -> Vec<Notice> {
        let mut out = Vec::new();
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

/// The footer's messages about the plugin files themselves: which plugins
/// are there, and which files aren't used and why.
pub fn plugin_notices(plugins: &Plugins) -> Vec<Notice> {
    let mut out = Vec::new();
    let ready: Vec<String> = plugins
        .found()
        .iter()
        .filter(|f| f.state == State::Ready)
        .filter_map(|f| f.manifest.as_ref())
        .map(|m| format!("{} {}", m.name, m.version))
        .collect();
    if !ready.is_empty() {
        out.push(Notice::new(
            Level::Info,
            format!("Plugins: {}.", ready.join(", ")),
        ));
    }
    for f in plugins.found() {
        let file = file_name(&f.path);
        match &f.state {
            State::Ready => {}
            State::Disabled => out.push(Notice::new(
                Level::Info,
                format!("{} is disabled in settings.json.", f.name()),
            )),
            State::Replaced(by) => out.push(Notice::new(
                Level::Info,
                format!(
                    "{} isn't used: {} holds the same plugin, {}, at a higher or equal version.",
                    f.path.display(),
                    by.display(),
                    f.name()
                ),
            )),
            State::Unusable(why) => out.push(Notice::new(
                Level::Warning,
                format!(
                    "The plugin file {file} can't be used: {why}. It is in {}.",
                    f.path
                        .parent()
                        .map_or_else(String::new, |p| p.display().to_string())
                ),
            )),
        }
    }
    out
}

/// The plugin files directly in `folder`, by name. Unreadable folders give
/// none: a missing system folder is normal.
pub(crate) fn plugin_files(folder: &Path) -> Vec<PathBuf> {
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
    fn only_plugin_files_are_found() {
        let root = scratch("scan");
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        for f in ["gp5.oxp", "notes.txt", "MIDI.OXP"] {
            std::fs::write(a.join(f), b"").unwrap();
        }
        std::fs::create_dir_all(a.join("folder.oxp")).unwrap();
        std::fs::write(b.join("pdf.oxp"), b"").unwrap();
        assert_eq!(
            plugin_files(&a),
            vec![a.join("MIDI.OXP"), a.join("gp5.oxp")]
        );
        assert!(plugin_files(&root.join("missing")).is_empty());
        let plugins = Plugins::discover(
            &[a.clone(), root.join("missing"), b.clone()],
            crate::SharedSettings::default(),
            "x11",
            std::sync::Arc::new(crate::Unheard),
        );
        let inv = Inventory::of(vec![a.clone(), b.clone()], &plugins);
        let abs = |p: PathBuf| std::path::absolute(&p).unwrap();
        assert_eq!(
            inv.files,
            vec![
                abs(a.join("MIDI.OXP")),
                abs(a.join("gp5.oxp")),
                abs(b.join("pdf.oxp"))
            ]
        );
        // Empty files aren't plugins: each is named, none is used.
        let notices = plugin_notices(&plugins);
        assert_eq!(notices.len(), 3);
        assert!(notices.iter().all(|n| n.level == Level::Warning));
        assert!(
            notices[0]
                .text
                .starts_with("The plugin file MIDI.OXP can't be used: ")
        );
        assert!(inv.plugin_readers.is_empty());
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
    fn readers_and_an_output_need_no_notice() {
        let inv = Inventory {
            plugin_readers: vec!["Guitar Pro 5 (.gp5)".to_owned()],
            instruments: vec!["MIDI".to_owned()],
            ..Inventory::default()
        };
        assert!(inv.notices(Path::new("p")).is_empty());
        let writers_only = Inventory {
            plugin_writers: vec!["Guitar Pro 7–8".to_owned()],
            instruments: vec!["MIDI".to_owned()],
            ..Inventory::default()
        };
        let n = writers_only.notices(Path::new("p"));
        assert_eq!(n.len(), 1);
        assert!(n[0].text.starts_with("No plugin that reads files"));
    }
}
