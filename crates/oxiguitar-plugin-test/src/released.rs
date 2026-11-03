// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Released plugins for OxiGuitar's own tests (#165).
//!
//! The plugins live in repositories of their own (mateusrdp/oxiguitar-NAME).
//! The tests of the core that need real ones, to read a Guitar Pro file, play
//! with a sound set or export a WAV file, take the files of the latest
//! release of those repositories: [`folder`] puts them in the workspace's
//! `target/released/` (the release's zip for this system, checked against its
//! `SHA256SUMS`, by `scripts/released-plugins.sh`) and says which versions it
//! took. Set `OXIGUITAR_PLUGINS_DIR` to a folder of `.oxp` files to use those
//! instead: a local build, or a machine with no network.
//!
//! [`guitar_pro`] reads and writes Guitar Pro files through the released
//! Guitar Pro plugins, as the app does, and gives the score the core's own
//! model.
//!
//! This is for the tests of OxiGuitar's own crates, which run inside its
//! workspace; enable the feature `released`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace this crate is built in.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The bash that runs the script. On Windows, `bash` on the PATH may be the
/// launcher of the Windows Subsystem for Linux (`C:\Windows\System32\bash.exe`),
/// not Git's, so Git for Windows' is taken first.
fn bash() -> PathBuf {
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramW6432"] {
            if let Some(dir) = std::env::var_os(var) {
                let git = Path::new(&dir).join("Git/bin/bash.exe");
                if git.is_file() {
                    return git;
                }
            }
        }
    }
    PathBuf::from("bash")
}

/// A folder holding the `.oxp` files of the latest release of each plugin
/// repository in `names` (`"guitarpro"`, `"midi"`, `"wav"`, ...), or
/// `$OXIGUITAR_PLUGINS_DIR`. Panics, saying why, when the release can't be
/// had: a test that needs a plugin can't go on without it.
pub fn folder(names: &[&str]) -> PathBuf {
    if let Some(dir) = std::env::var_os("OXIGUITAR_PLUGINS_DIR") {
        return PathBuf::from(dir);
    }
    let script = workspace().join("scripts/released-plugins.sh");
    let mut key = names.to_vec();
    key.sort_unstable();
    let out = workspace().join("target/released").join(key.join("+"));
    let output = Command::new(bash())
        .arg(&script)
        .arg(&out)
        .args(names)
        .env("OXIGUITAR_RELEASES_ARCH", std::env::consts::ARCH)
        .output()
        .unwrap_or_else(|e| panic!("can't run {}: {e}", script.display()));
    // The versions used go to the test's output.
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "the released plugins {names:?} can't be had (set OXIGUITAR_PLUGINS_DIR to a folder \
         of .oxp files to use a local build): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

/// Guitar Pro files through the released Guitar Pro plugins.
pub mod guitar_pro {
    use std::sync::{Arc, OnceLock};

    use oxiguitar_host::api::message::{Level, LogEntry};
    use oxiguitar_host::{Plugins, SharedSettings, Unheard};

    /// A file read: the score and what could not be carried over faithfully.
    #[derive(Debug)]
    pub struct Imported {
        /// The score.
        pub score: oxiguitar_model::Score,
        /// What the plugin said about the file.
        pub log: Vec<LogEntry>,
    }

    /// What a format could not hold when a score was written.
    #[derive(Debug, Default)]
    pub struct Losses(Vec<LogEntry>);

    impl Losses {
        /// How many entries are warnings or worse.
        pub fn warning_count(&self) -> usize {
            self.0.iter().filter(|e| e.level >= Level::Warning).count()
        }

        /// Every entry.
        pub fn entries(&self) -> &[LogEntry] {
            &self.0
        }
    }

    /// A file written.
    #[derive(Debug)]
    pub struct Written {
        /// The file's contents.
        pub data: Vec<u8>,
        /// What the format can't hold and was left out.
        pub losses: Losses,
    }

    /// The five Guitar Pro plugins of the latest release, loaded once for
    /// the process (a system loads a library file once per process).
    fn plugins() -> &'static Plugins {
        static PLUGINS: OnceLock<Plugins> = OnceLock::new();
        PLUGINS.get_or_init(|| {
            let folder = super::folder(&["guitarpro"]);
            Plugins::discover(
                &[folder],
                SharedSettings::default(),
                oxiguitar_host::display_system(),
                Arc::new(Unheard),
            )
        })
    }

    /// Reads a Guitar Pro file (any of .gp3, .gp4, .gp5, .gpx and .gp, told
    /// by its contents) with the plugin that recognises it.
    pub fn read(data: &[u8]) -> Result<Imported, String> {
        let plugins = plugins();
        let format = plugins.recognise(data, None).map_err(|e| e.to_string())?;
        // The host prefixes a plugin's error with the plugin's name.
        let prefix = format!("{}: ", format.plugin.name());
        let read = plugins.read(format, data).map_err(|e| {
            let text = e.to_string();
            text.strip_prefix(&prefix)
                .map_or(text.clone(), str::to_owned)
        })?;
        let score = oxiguitar_model::wire::from_wire2(read.score)?;
        Ok(Imported {
            score,
            log: read.log,
        })
    }

    fn write(format: &str, score: &oxiguitar_model::Score) -> Result<Written, String> {
        let plugins = plugins();
        let writer = plugins
            .writers()
            .into_iter()
            .find(|f| f.spec.id == format)
            .ok_or_else(|| format!("no plugin writes {format}"))?;
        let written = plugins
            .write(writer, &oxiguitar_model::wire::to_wire2(score))
            .map_err(|e| e.to_string())?;
        Ok(Written {
            data: written.data,
            losses: Losses(written.losses),
        })
    }

    /// Writes `score` as a Guitar Pro 7/8 file (.gp).
    pub fn write_gp(score: &oxiguitar_model::Score) -> Result<Written, String> {
        write("gp", score)
    }

    /// Writes `score` as a Guitar Pro 5 file (.gp5).
    pub fn write_gp5(score: &oxiguitar_model::Score) -> Result<Written, String> {
        write("gp5", score)
    }
}
