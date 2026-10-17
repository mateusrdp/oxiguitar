// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Where OxiGuitar keeps its settings and looks for plugins, on each system
//! (docs/design/plugins.md, "Folders and settings").

use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    /// The user folder: settings.json and the user's plugins live here.
    pub user: PathBuf,
    /// The user's plugin folder, inside the user folder.
    pub user_plugins: PathBuf,
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
        let user = match set(HOME_VARIABLE) {
            Some(dir) => dir,
            None => match system {
                System::Linux => set("HOME")?.join(".oxiguitar"),
                System::MacOs => set("HOME")?
                    .join("Library")
                    .join("Application Support")
                    .join("OxiGuitar"),
                System::Windows => set("APPDATA")?.join("OxiGuitar"),
            },
        };
        let bin = exe.and_then(Path::parent);
        let system_plugins = match system {
            // /usr/bin/oxiguitar → /usr/lib/oxiguitar/plugins; the same
            // rule finds /usr/local, the AppImage's own tree and a build
            // installed anywhere else.
            System::Linux => bin
                .and_then(Path::parent)
                .map(|prefix| prefix.join("lib").join("oxiguitar").join("plugins"))
                .into_iter()
                .collect(),
            // OxiGuitar.app/Contents/MacOS/oxiguitar → Contents/PlugIns.
            System::MacOs => bin
                .and_then(Path::parent)
                .map(|contents| contents.join("PlugIns"))
                .into_iter()
                .chain([PathBuf::from(
                    "/Library/Application Support/OxiGuitar/plugins",
                )])
                .collect(),
            // The portable zip: plugins\ beside oxiguitar.exe.
            System::Windows => bin.map(|b| b.join("plugins")).into_iter().collect(),
        };
        Some(Folders {
            user_plugins: user.join("plugins"),
            user,
            system_plugins,
        })
    }

    /// The settings file.
    pub fn settings(&self) -> PathBuf {
        self.user.join(SETTINGS_FILE)
    }

    /// Where plugins are looked for, in order, unless settings.json lists
    /// its own: the user's folder first, then the system's.
    pub fn default_plugin_folders(&self) -> Vec<PathBuf> {
        std::iter::once(self.user_plugins.clone())
            .chain(self.system_plugins.iter().cloned())
            .collect()
    }

    /// Makes sure the user folder, its plugin folder and settings.json
    /// exist, creating what is missing (settings.json as an empty JSON
    /// object). Returns what was created; existing files are never touched.
    pub fn ensure(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        for dir in [&self.user, &self.user_plugins] {
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
    fn linux_uses_a_dot_folder_and_the_lib_beside_the_program() {
        let f = Folders::resolve(
            System::Linux,
            env(&[("HOME", "/home/mat")]),
            Some(Path::new("/usr/bin/oxiguitar")),
        )
        .unwrap();
        assert_eq!(f.user, PathBuf::from("/home/mat/.oxiguitar"));
        assert_eq!(
            f.user_plugins,
            PathBuf::from("/home/mat/.oxiguitar/plugins")
        );
        assert_eq!(
            f.settings(),
            PathBuf::from("/home/mat/.oxiguitar/settings.json")
        );
        assert_eq!(
            f.system_plugins,
            vec![PathBuf::from("/usr/lib/oxiguitar/plugins")]
        );
        assert_eq!(f.default_plugin_folders()[0], f.user_plugins);
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
        assert_eq!(
            mac.user,
            PathBuf::from("/Users/mat/Library/Application Support/OxiGuitar")
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
            win.user,
            PathBuf::from("C:/Users/mat/AppData/Roaming/OxiGuitar")
        );
        assert_eq!(
            win.system_plugins,
            vec![PathBuf::from("D:/OxiGuitar/plugins")]
        );
    }

    #[test]
    fn the_variable_replaces_the_user_folder() {
        let f = Folders::resolve(
            System::Linux,
            env(&[("HOME", "/home/mat"), (HOME_VARIABLE, "/media/usb/og")]),
            None,
        )
        .unwrap();
        assert_eq!(f.user, PathBuf::from("/media/usb/og"));
        assert!(f.system_plugins.is_empty());
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
