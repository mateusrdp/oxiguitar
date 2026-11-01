// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the plugin folders hold, and what that means for the user.
//!
//! A plugin is one `.oxp` file (docs/design/plugins.md, "Plugin files").
//! The inventory is what the plugins' manifests say the program can do,
//! without loading any plugin. The core reads and writes no format by
//! itself.

use std::path::{Path, PathBuf};

use crate::folders::System;
use crate::plugins::{Plugins, State, file_name};
use oxiguitar_plugin_api::message::{Advice, Link};

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
    /// The audio devices plugins declare (the `audio-device` service),
    /// by name: where the player's sound can go.
    pub outputs: Vec<String>,
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
            outputs: plugins.declared_services("audio-device"),
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
            "The basic plugins come with every OxiGuitar download ({PLUGINS_URL}): install it again, or put plugins in {}.",
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

/// The manual's pages that the core's advice links to.
const MANUAL: &str = "https://mateusrdp.github.io/oxiguitar/";

/// The manual's page listing the official plugins and how to install them.
pub fn plugins_page() -> String {
    format!("{MANUAL}official-plugins.html")
}

/// The manual's install page for `system`. `appimage` is whether the
/// program runs from an AppImage, which gets its own section: the
/// AppImage holds the core only, and its plugins are files to unpack.
pub fn install_page(system: System, appimage: bool) -> String {
    match (system, appimage) {
        (System::Linux, true) => format!("{MANUAL}install-linux.html#appimage"),
        (System::Linux, _) => format!("{MANUAL}install-linux.html"),
        (System::MacOs, _) => format!("{MANUAL}install-macos.html"),
        (System::Windows, _) => format!("{MANUAL}install-windows.html"),
    }
}

/// What the core says it can't do, as advice the window shows once and
/// the user can silence (plugin interface 1.4; the ids are what
/// settings.json keeps under `advice.oxiguitar`).
pub const NO_FORMAT_PLUGIN: &str = "no-format-plugin";
/// See [`NO_FORMAT_PLUGIN`].
pub const NO_PLAYER_PLUGIN: &str = "no-player-plugin";
/// See [`NO_FORMAT_PLUGIN`].
pub const NO_OUTPUT_PLUGIN: &str = "no-output-plugin";

impl Inventory {
    /// The core's startup check as advice, most important first: no plugin
    /// reads files (`no-format-plugin`), no plugin plays a score
    /// (`no-player-plugin`), and, only when one does, no plugin sends the
    /// sound to the speakers (`no-output-plugin`: exporting audio works
    /// without one). Each links to the manual's page of official plugins
    /// and its install page for `system`. `user_plugins` is the folder the
    /// text tells the user to put plugins in.
    pub fn advice(&self, system: System, appimage: bool, user_plugins: &Path) -> Vec<Advice> {
        let links = || {
            vec![
                Link {
                    text: "The official plugins".to_owned(),
                    url: plugins_page(),
                },
                Link {
                    text: format!(
                        "Installing plugins on {}",
                        match system {
                            System::Linux => "Linux",
                            System::MacOs => "macOS",
                            System::Windows => "Windows",
                        }
                    ),
                    url: install_page(system, appimage),
                },
            ]
        };
        let folder = user_plugins.display();
        let mut out = Vec::new();
        if self.readers().next().is_none() {
            out.push(Advice {
                id: NO_FORMAT_PLUGIN.to_owned(),
                title: "OxiGuitar can't open any file".to_owned(),
                text: format!(
                    "OxiGuitar opens and saves files only through plugins, and no plugin that reads files is installed. The Guitar Pro plugins open Guitar Pro files, versions 3 to 8.\nInstall them from the page of official plugins, or put their .oxp files in {folder}."
                ),
                links: links(),
            });
        }
        if self.instruments.is_empty() {
            out.push(Advice {
                id: NO_PLAYER_PLUGIN.to_owned(),
                title: "OxiGuitar can't play".to_owned(),
                text: format!(
                    "No plugin that plays a score is installed, so the play buttons stay off. The MIDI plugin plays scores through a sound set (SoundFont or SFZ), and your system has one.\nInstall it from the page of official plugins, or put its .oxp file in {folder}."
                ),
                links: links(),
            });
        } else if self.outputs.is_empty() {
            out.push(Advice {
                id: NO_OUTPUT_PLUGIN.to_owned(),
                title: "OxiGuitar has nowhere to send the sound".to_owned(),
                text: format!(
                    "A plugin that plays scores is installed, but no plugin sends sound to your speakers (PipeWire, ALSA, WASAPI or Core Audio, one for each system), so the play buttons stay off. Exporting audio to a file still works.\nInstall the one for your system from the page of official plugins, or put its .oxp file in {folder}."
                ),
                links: links(),
            });
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
            State::Unmet(why) => out.push(Notice::new(
                Level::Error,
                format!("{} can't be used: {why}.", f.name()),
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

    fn ids(advice: &[Advice]) -> Vec<&str> {
        advice.iter().map(|a| a.id.as_str()).collect()
    }

    fn with(readers: bool, instruments: bool, outputs: bool) -> Inventory {
        let one = |on: bool, name: &str| if on { vec![name.to_owned()] } else { vec![] };
        Inventory {
            plugin_readers: one(readers, "Guitar Pro 5 (.gp5)"),
            instruments: one(instruments, "MIDI"),
            outputs: one(outputs, "PipeWire"),
            ..Inventory::default()
        }
    }

    #[test]
    fn the_startup_check_says_what_the_core_cant_do() {
        let folder = Path::new("/home/mat/.local/share/oxiguitar/plugins");
        let advise = |i: &Inventory| i.advice(System::Linux, false, folder);
        assert_eq!(
            ids(&advise(&with(false, false, false))),
            [NO_FORMAT_PLUGIN, NO_PLAYER_PLUGIN]
        );
        assert_eq!(ids(&advise(&with(true, false, false))), [NO_PLAYER_PLUGIN]);
        // No output matters only once something plays.
        assert_eq!(ids(&advise(&with(true, true, false))), [NO_OUTPUT_PLUGIN]);
        assert_eq!(
            ids(&advise(&with(false, true, false))),
            [NO_FORMAT_PLUGIN, NO_OUTPUT_PLUGIN]
        );
        assert!(advise(&with(true, true, true)).is_empty());
        // An output without a player says nothing about outputs.
        assert_eq!(ids(&advise(&with(true, false, true))), [NO_PLAYER_PLUGIN]);
    }

    #[test]
    fn every_advice_is_valid_and_links_to_the_manual() {
        let folder = Path::new("/home/mat/.local/share/oxiguitar/plugins");
        for (system, appimage, install) in [
            (System::Linux, false, "install-linux.html"),
            (System::Linux, true, "install-linux.html#appimage"),
            (System::MacOs, false, "install-macos.html"),
            (System::Windows, false, "install-windows.html"),
        ] {
            let all = [
                with(false, false, false).advice(system, appimage, folder),
                with(true, true, false).advice(system, appimage, folder),
            ]
            .concat();
            assert_eq!(all.len(), 3);
            for a in &all {
                assert_eq!(a.problem(), None, "{}", a.id);
                assert_eq!(a.links.len(), 2, "{}", a.id);
                assert_eq!(a.links[0].url, plugins_page());
                assert!(a.links[1].url.ends_with(install), "{}", a.links[1].url);
                assert!(a.text.contains(&folder.display().to_string()), "{}", a.id);
            }
        }
    }
}
