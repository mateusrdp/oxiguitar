// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Where OxiGuitar keeps its settings and looks for plugins, on each system
//! (docs/design/plugins.md, "Folders and settings").

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use oxiguitar_plugin_api::manifest::SharedFolder;

/// The environment variable that replaces the user folder: for tests, and
/// for a portable copy that keeps everything beside itself.
pub const HOME_VARIABLE: &str = "OXIGUITAR_HOME";

/// The name of the settings file in the user folder.
pub const SETTINGS_FILE: &str = "settings.json";

/// The operating systems with their own folder conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// Linux and other Unix-like systems.
    Linux,
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
}

impl System {
    /// The system this program was built for.
    pub const fn this() -> System {
        if cfg!(target_os = "macos") {
            System::MacOs
        } else if cfg!(target_os = "windows") {
            System::Windows
        } else {
            System::Linux
        }
    }
}

/// OxiGuitar's folders on this machine.
///
/// On Linux they follow the freedesktop XDG Base Directory convention
/// (decided 18 Oct 2026): settings under the config home, plugins and
/// their data under the data home, the plugins' caches under the cache
/// home. On macOS and Windows one folder holds settings, plugins and data,
/// and caches go where the system keeps them; with `OXIGUITAR_HOME`, one
/// folder holds everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    /// Where settings.json lives.
    pub config: PathBuf,
    /// Where the user's plugins and the plugins' own data live.
    pub data: PathBuf,
    /// The user's plugin folder, `plugins` inside the data folder.
    pub user_plugins: PathBuf,
    /// Where the plugins' caches live (interface 1.3): what a plugin can
    /// make again, which the system or the user may delete at any time.
    pub cache: PathBuf,
    /// System plugin folders, which packages install into. OxiGuitar only
    /// reads them; it never creates or writes them.
    pub system_plugins: Vec<PathBuf>,
}

impl Folders {
    /// The folders for this process: its environment and where the program
    /// itself is. `None` if the system names no home folder at all.
    pub fn here() -> Option<Folders> {
        let exe = std::env::current_exe().ok();
        Folders::resolve(
            System::this(),
            |name| std::env::var_os(name),
            exe.as_deref(),
        )
    }

    /// The folders for `system`, given its environment variables and the
    /// program's path (for the system plugin folder that ships beside it).
    pub fn resolve(
        system: System,
        env: impl Fn(&str) -> Option<OsString>,
        exe: Option<&Path>,
    ) -> Option<Folders> {
        let set = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        // The XDG specification: a relative path in these variables is
        // invalid and must be ignored. "Absolute" here means starting at
        // the root, `/`, whichever system runs the check (a test resolving
        // Linux folders on Windows, where /cfg has no drive letter).
        let xdg = |name: &str, fallback: &str| -> Option<PathBuf> {
            match set(name).filter(|p| p.has_root()) {
                Some(dir) => Some(dir),
                None => Some(set("HOME")?.join(fallback)),
            }
        };
        let (config, data, cache) = match set(HOME_VARIABLE) {
            Some(dir) => (dir.clone(), dir.clone(), dir.join("cache")),
            None => match system {
                System::Linux => (
                    xdg("XDG_CONFIG_HOME", ".config")?.join("oxiguitar"),
                    xdg("XDG_DATA_HOME", ".local/share")?.join("oxiguitar"),
                    xdg("XDG_CACHE_HOME", ".cache")?.join("oxiguitar"),
                ),
                System::MacOs => {
                    let home = set("HOME")?;
                    let dir = home
                        .join("Library")
                        .join("Application Support")
                        .join("OxiGuitar");
                    let cache = home.join("Library").join("Caches").join("OxiGuitar");
                    (dir.clone(), dir, cache)
                }
                System::Windows => {
                    let dir = set("APPDATA")?.join("OxiGuitar");
                    // Caches stay on this machine (Local), not in the
                    // roaming profile.
                    let cache = set("LOCALAPPDATA")
                        .map(|local| local.join("OxiGuitar"))
                        .unwrap_or_else(|| dir.clone())
                        .join("cache");
                    (dir.clone(), dir, cache)
                }
            },
        };
        let bin = exe.and_then(Path::parent);
        let system_plugins = match system {
            // /usr/bin/oxiguitar → /usr/lib/oxiguitar/plugins; the same
            // rule finds /usr/local, the AppImage's own tree and a build
            // installed anywhere else. When the derived path is missing
            // (a dev build that hasn't been installed), fall back to the
            // standard Linux plugin directories.
            System::Linux => {
                let derived = bin
                    .and_then(Path::parent)
                    .map(|prefix| prefix.join("lib").join("oxiguitar").join("plugins"));
                let fallbacks = [
                    PathBuf::from("/usr/lib/oxiguitar/plugins"),
                    PathBuf::from("/usr/local/lib/oxiguitar/plugins"),
                ];
                // Deduplicate: the derived path may equal a fallback.
                let mut out: Vec<PathBuf> = Vec::with_capacity(3);
                if let Some(p) = derived
                    && !fallbacks.contains(&p)
                {
                    out.push(p);
                }
                out.extend(fallbacks);
                out
            }
            // OxiGuitar.app/Contents/MacOS/oxiguitar → Contents/PlugIns.
            // When the derived path is missing (no exe path), fall back to
            // the system-wide plugin folder.
            System::MacOs => bin
                .and_then(Path::parent)
                .map(|contents| contents.join("PlugIns"))
                .into_iter()
                .chain([PathBuf::from(
                    "/Library/Application Support/OxiGuitar/plugins",
                )])
                .collect(),
            // The portable zip: plugins\ beside oxiguitar.exe. When the
            // derived path is missing (a dev build that hasn't been
            // installed), fall back to the standard Windows plugin
            // directory.
            System::Windows => bin
                .map(|b| b.join("plugins"))
                .into_iter()
                .chain([PathBuf::from("C:/Program Files/OxiGuitar/plugins")])
                .collect(),
        };
        Some(Folders {
            user_plugins: data.join("plugins"),
            config,
            data,
            cache,
            system_plugins,
        })
    }

    /// The plugin `id`'s own data folder (interface 1.3).
    pub fn plugin_data(&self, id: &str) -> PathBuf {
        self.data.join("data").join(id)
    }

    /// The plugin `id`'s own cache folder (interface 1.3).
    pub fn plugin_cache(&self, id: &str) -> PathBuf {
        self.cache.join(id)
    }

    /// A shared folder, beside `plugins` in the data folder: `soundfonts`,
    /// `nam` (interface 1.3).
    pub fn shared(&self, folder: SharedFolder) -> PathBuf {
        self.data.join(folder.name())
    }

    /// Makes sure the shared folders `wanted` exist, creating what is
    /// missing; returns what was created.
    pub fn ensure_shared(&self, wanted: &[SharedFolder]) -> std::io::Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        for &folder in wanted {
            let dir = self.shared(folder);
            if !dir.is_dir() {
                std::fs::create_dir_all(&dir)?;
                created.push(dir);
            }
        }
        Ok(created)
    }

    /// The settings file.
    pub fn settings(&self) -> PathBuf {
        self.config.join(SETTINGS_FILE)
    }

    /// Where plugins are looked for, in order, unless settings.json lists
    /// its own: the system's folders first, then the user's. Later folders
    /// override earlier ones for the same plugin ID.
    pub fn default_plugin_folders(&self) -> Vec<PathBuf> {
        self.system_plugins
            .iter()
            .cloned()
            .chain(std::iter::once(self.user_plugins.clone()))
            .collect()
    }

    /// Makes sure the settings folder, the user's plugin folder and
    /// settings.json exist, creating what is missing (settings.json as an
    /// empty JSON object). Returns what was created; existing files are
    /// never touched.
    pub fn ensure(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        for dir in [&self.config, &self.user_plugins] {
            if !dir.is_dir() {
                std::fs::create_dir_all(dir)?;
                created.push(dir.clone());
            }
        }
        let settings = self.settings();
        // create_new: never overwrite a file that appeared in between.
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&settings)
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(b"{}\n")?;
                created.push(settings);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        Ok(created)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn linux_follows_xdg_and_finds_the_lib_beside_the_program() {
        let f = Folders::resolve(
            System::Linux,
            env(&[("HOME", "/home/mat")]),
            Some(Path::new("/usr/bin/oxiguitar")),
        )
        .unwrap();
        assert_eq!(f.config, PathBuf::from("/home/mat/.config/oxiguitar"));
        assert_eq!(
            f.settings(),
            PathBuf::from("/home/mat/.config/oxiguitar/settings.json")
        );
        assert_eq!(f.data, PathBuf::from("/home/mat/.local/share/oxiguitar"));
        assert_eq!(
            f.user_plugins,
            PathBuf::from("/home/mat/.local/share/oxiguitar/plugins")
        );
        assert_eq!(
            f.system_plugins,
            vec![
                PathBuf::from("/usr/lib/oxiguitar/plugins"),
                PathBuf::from("/usr/local/lib/oxiguitar/plugins"),
            ]
        );
        assert_eq!(f.default_plugin_folders()[0], f.system_plugins[0]);
        // The second fallback comes before the user folder.
        assert_eq!(
            f.default_plugin_folders()[1],
            PathBuf::from("/usr/local/lib/oxiguitar/plugins")
        );
        assert_eq!(f.default_plugin_folders()[2], f.user_plugins);
        assert_eq!(f.cache, PathBuf::from("/home/mat/.cache/oxiguitar"));
        assert_eq!(
            f.plugin_data("me.nam"),
            PathBuf::from("/home/mat/.local/share/oxiguitar/data/me.nam")
        );
        assert_eq!(
            f.plugin_cache("me.nam"),
            PathBuf::from("/home/mat/.cache/oxiguitar/me.nam")
        );
        assert_eq!(
            f.shared(SharedFolder::Soundfonts),
            PathBuf::from("/home/mat/.local/share/oxiguitar/soundfonts")
        );
    }

    #[test]
    fn xdg_variables_are_used_when_absolute() {
        let f = Folders::resolve(
            System::Linux,
            env(&[
                ("HOME", "/home/mat"),
                ("XDG_CONFIG_HOME", "/cfg"),
                ("XDG_DATA_HOME", "relative/share"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(f.config, PathBuf::from("/cfg/oxiguitar"));
        // A relative value is invalid by the specification: ignored.
        assert_eq!(f.data, PathBuf::from("/home/mat/.local/share/oxiguitar"));
    }

    #[test]
    fn macos_and_windows_follow_their_conventions() {
        let mac = Folders::resolve(
            System::MacOs,
            env(&[("HOME", "/Users/mat")]),
            Some(Path::new(
                "/Applications/OxiGuitar.app/Contents/MacOS/oxiguitar",
            )),
        )
        .unwrap();
        let support = PathBuf::from("/Users/mat/Library/Application Support/OxiGuitar");
        assert_eq!((&mac.config, &mac.data), (&support, &support));
        assert_eq!(
            mac.cache,
            PathBuf::from("/Users/mat/Library/Caches/OxiGuitar")
        );
        assert_eq!(
            mac.system_plugins[0],
            PathBuf::from("/Applications/OxiGuitar.app/Contents/PlugIns")
        );
        let win = Folders::resolve(
            System::Windows,
            env(&[("APPDATA", "C:/Users/mat/AppData/Roaming")]),
            Some(Path::new("D:/OxiGuitar/oxiguitar.exe")),
        )
        .unwrap();
        assert_eq!(
            win.config,
            PathBuf::from("C:/Users/mat/AppData/Roaming/OxiGuitar")
        );
        assert_eq!(
            win.system_plugins,
            vec![
                PathBuf::from("D:/OxiGuitar/plugins"),
                PathBuf::from("C:/Program Files/OxiGuitar/plugins"),
            ]
        );
        assert_eq!(
            win.cache,
            PathBuf::from("C:/Users/mat/AppData/Roaming/OxiGuitar/cache")
        );
        let local = Folders::resolve(
            System::Windows,
            env(&[
                ("APPDATA", "C:/Users/mat/AppData/Roaming"),
                ("LOCALAPPDATA", "C:/Users/mat/AppData/Local"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(
            local.cache,
            PathBuf::from("C:/Users/mat/AppData/Local/OxiGuitar/cache")
        );
    }

    #[test]
    fn dev_build_falls_back_to_standard_system_paths() {
        let f = Folders::resolve(
            System::Linux,
            env(&[("HOME", "/home/mat")]),
            Some(Path::new("/home/mat/src/oxiguitar/target/debug/oxiguitar")),
        )
        .unwrap();
        // The derived path does not exist; fallbacks cover standard installs.
        assert_eq!(
            f.system_plugins,
            vec![
                PathBuf::from("/home/mat/src/oxiguitar/target/lib/oxiguitar/plugins"),
                PathBuf::from("/usr/lib/oxiguitar/plugins"),
                PathBuf::from("/usr/local/lib/oxiguitar/plugins"),
            ]
        );
        // System folders still come before user folders.
        let folders = f.default_plugin_folders();
        assert_eq!(folders.len(), 4);
        assert_eq!(
            folders[0],
            PathBuf::from("/home/mat/src/oxiguitar/target/lib/oxiguitar/plugins")
        );
        assert_eq!(folders[1], PathBuf::from("/usr/lib/oxiguitar/plugins"));
        assert_eq!(
            folders[2],
            PathBuf::from("/usr/local/lib/oxiguitar/plugins")
        );
        assert_eq!(
            folders[3],
            PathBuf::from("/home/mat/.local/share/oxiguitar/plugins")
        );
    }

    #[test]
    fn system_folders_come_before_user_folders() {
        let f = Folders::resolve(
            System::Linux,
            env(&[("HOME", "/home/mat"), (HOME_VARIABLE, "/tmp/og-test-var")]),
            Some(Path::new("/usr/bin/oxiguitar")),
        )
        .unwrap();
        let folders = f.default_plugin_folders();
        // System folders come first, then user folder.
        assert_eq!(folders.len(), 3);
        assert_eq!(folders[0], PathBuf::from("/usr/lib/oxiguitar/plugins"));
        assert_eq!(
            folders[1],
            PathBuf::from("/usr/local/lib/oxiguitar/plugins")
        );
        assert_eq!(folders[2], PathBuf::from("/tmp/og-test-var/plugins"));
        // The user folder overrides the system folder for the same plugin ID
        // because deduplication prefers the earlier folder, and the system
        // folder comes first.
        assert!(
            folders
                .iter()
                .position(|p| p.as_path() == f.system_plugins[0].as_path())
                .unwrap()
                < folders
                    .iter()
                    .position(|p| p.as_path() == f.user_plugins)
                    .unwrap()
        );
    }

    #[test]
    fn the_variable_puts_everything_in_one_folder() {
        let f = Folders::resolve(
            System::Linux,
            env(&[
                ("HOME", "/home/mat"),
                ("XDG_CONFIG_HOME", "/cfg"),
                (HOME_VARIABLE, "/media/usb/og"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(f.config, PathBuf::from("/media/usb/og"));
        assert_eq!(f.user_plugins, PathBuf::from("/media/usb/og/plugins"));
        assert_eq!(f.cache, PathBuf::from("/media/usb/og/cache"));
        assert_eq!(
            f.shared(SharedFolder::Nam),
            PathBuf::from("/media/usb/og/nam")
        );
        assert_eq!(
            f.system_plugins,
            vec![
                PathBuf::from("/usr/lib/oxiguitar/plugins"),
                PathBuf::from("/usr/local/lib/oxiguitar/plugins"),
            ]
        );
        assert!(Folders::resolve(System::Linux, env(&[]), None).is_none());
    }

    #[test]
    fn ensure_creates_what_is_missing_and_keeps_what_exists() {
        let root = scratch("ensure");
        let f = Folders::resolve(
            System::Linux,
            env(&[(HOME_VARIABLE, root.to_str().unwrap())]),
            None,
        )
        .unwrap();
        let created = f.ensure().unwrap();
        assert_eq!(created, vec![f.user_plugins.clone(), f.settings()]);
        assert_eq!(std::fs::read_to_string(f.settings()).unwrap(), "{}\n");
        std::fs::write(f.settings(), "{\"mine\": 1}").unwrap();
        assert!(f.ensure().unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(f.settings()).unwrap(),
            "{\"mine\": 1}"
        );
        let wanted = [SharedFolder::Soundfonts];
        assert_eq!(
            f.ensure_shared(&wanted).unwrap(),
            vec![root.join("soundfonts")]
        );
        assert!(f.ensure_shared(&wanted).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A fresh folder under the system's temporary folder (the user folder
    /// itself exists; ensure() creates what's inside it).
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("oxiguitar-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
