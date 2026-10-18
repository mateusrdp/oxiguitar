// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin contract suite (docs/design/plugins.md, "Testing").
//!
//! [`build`] builds a plugin crate as an `.oxp` file, and [`check`] loads
//! one through OxiGuitar's own plugin host and checks it against the
//! interface:
//!
//! - its manifest is read without loading the plugin;
//! - it loads and starts;
//! - a request for a contribution it doesn't have is answered with
//!   "not found", not a crash;
//! - every format it reads answers damaged input (nothing, zeros, its own
//!   signature alone) with an error or a score, never a crash;
//! - every format it writes answers an empty score;
//! - every service it declares answers its probe.
//!
//! Checks for instruments (no allocation, locks or waiting on the audio
//! thread) come with the instrument interface (M4).
//!
//! The `oxiguitar-plugin-test` program runs the same checks from the
//! command line, for plugin authors.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use oxiguitar_host::api::Blob;
use oxiguitar_host::api::message::{Answer, Error, Request};
use oxiguitar_host::api::score::Score;
use oxiguitar_host::{Level, Messages, Plugins, SharedSettings, State};

/// Builds the plugin crate `package` of the workspace at `workspace` as
/// `NAME.oxp` (with its `oxp` feature, as a `cdylib`), in the workspace's
/// `target/oxp/` folder (its own, so a build started from a test doesn't
/// wait for the test's own build folder). Returns the file's path.
pub fn build(
    workspace: &Path,
    package: &str,
    name: &str,
    release: bool,
) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = workspace.join("target").join("oxp");
    let mut command = std::process::Command::new(cargo);
    command
        .current_dir(workspace)
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "rustc",
            "--quiet",
            "-p",
            package,
            "--features",
            "oxp",
            "--crate-type",
            "cdylib",
        ]);
    if release {
        command.arg("--release");
    } else {
        // Without debug information a test build is a tenth of the size;
        // contract tests copy it around.
        command
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_DEV_STRIP", "debuginfo");
    }
    let status = command
        .status()
        .map_err(|e| format!("cargo didn't run ({e})"))?;
    if !status.success() {
        return Err(format!("cargo couldn't build {package} ({status})"));
    }
    let library = target
        .join(if release { "release" } else { "debug" })
        .join(format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            package.replace('-', "_"),
            std::env::consts::DLL_SUFFIX
        ));
    let oxp = target.join(format!("{name}.oxp"));
    // A new file, not the old one overwritten: hard links to the old one
    // (tests make them) and a copy a running program has loaded keep theirs.
    let _ = std::fs::remove_file(&oxp);
    std::fs::copy(&library, &oxp).map_err(|e| {
        format!(
            "cannot copy {} to {}: {e}",
            library.display(),
            oxp.display()
        )
    })?;
    Ok(oxp)
}

/// The outcome of [`check`]: each check's description, passed or with
/// what went wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The checks that passed.
    pub passed: Vec<String>,
    /// The checks that failed, with why.
    pub failed: Vec<String>,
    /// Footer messages the plugin sent while being checked.
    pub messages: Vec<String>,
    /// How long reading the manifest took.
    pub manifest_time: Duration,
}

impl Report {
    /// Whether every check passed.
    pub fn ok(&self) -> bool {
        self.failed.is_empty()
    }

    fn check(&mut self, what: impl Into<String>, result: Result<(), String>) {
        let what = what.into();
        match result {
            Ok(()) => self.passed.push(what),
            Err(why) => self.failed.push(format!("{what}: {why}")),
        }
    }
}

/// Footer messages, kept for the report.
#[derive(Default)]
struct Kept(Mutex<Vec<String>>);

impl Messages for Kept {
    fn notify(&self, level: Level, text: String) {
        if let Ok(mut kept) = self.0.lock() {
            kept.push(format!("{level:?}: {text}"));
        }
    }
}

/// Checks the plugin file `oxp` against the interface (see the crate's
/// documentation). The file is copied into a folder of its own first, so
/// no other plugin is found beside it; the copy stays loaded until the
/// program ends.
pub fn check(oxp: &Path) -> Report {
    let mut report = Report::default();
    let folder = std::env::temp_dir().join(format!(
        "oxiguitar-plugin-test-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&folder);
    let copy = folder.join(oxp.file_name().unwrap_or("plugin.oxp".as_ref()));
    if let Err(e) =
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::copy(oxp, &copy).map(drop))
    {
        report.check("the plugin file can be read", Err(e.to_string()));
        return report;
    }
    let kept = Arc::new(Kept::default());
    let started = Instant::now();
    let plugins = Plugins::discover(
        std::slice::from_ref(&folder),
        SharedSettings::default(),
        oxiguitar_host::display_system(),
        kept.clone(),
    );
    report.manifest_time = started.elapsed();
    let Some(found) = plugins.found().first() else {
        report.check(
            "the manifest reads without loading the plugin",
            Err("the file wasn't found".into()),
        );
        return report;
    };
    report.check(
        "the manifest reads without loading the plugin",
        match (&found.state, found.is_loaded()) {
            (State::Ready, false) => Ok(()),
            (State::Ready, true) => Err("reading it loaded the plugin".into()),
            (state, _) => Err(format!("{state:?}")),
        },
    );
    if found.state != State::Ready {
        return report;
    }
    let manifest = found
        .manifest
        .clone()
        .unwrap_or_else(|| unreachable!("a ready plugin has a manifest"));
    let loading = plugins.load_all();
    report.check(
        "it loads and starts",
        if loading.is_empty() && found.is_loaded() {
            Ok(())
        } else {
            Err(loading.join("; "))
        },
    );
    if !found.is_loaded() {
        return report;
    }

    let missing = plugins.send(
        found,
        &Request::Read {
            format: "oxiguitar-contract-no-such-format".into(),
            data: Blob::borrowed(b""),
        },
    );
    report.check(
        "a contribution it doesn't have is \"not found\"",
        match missing {
            Err(e) if e.kind == oxiguitar_host::api::message::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("answered {:?}: {}", e.kind, e.message)),
            Ok(_) => Err("it answered as if it had it".into()),
        },
    );

    for format in manifest.formats.iter().filter(|f| f.read) {
        let mut inputs: Vec<(&str, Vec<u8>)> =
            vec![("nothing", Vec::new()), ("1 KiB of zeros", vec![0; 1024])];
        if let Some(sig) = format.signatures.first() {
            let mut bytes = vec![0; usize::try_from(sig.at).unwrap_or(0).min(1 << 16)];
            if let Some(text) = &sig.text {
                bytes.extend_from_slice(text.as_bytes());
            }
            inputs.push(("its signature alone", bytes));
        }
        for (what, data) in inputs {
            let answer = plugins.send(
                found,
                &Request::Read {
                    format: format.id.clone(),
                    data: Blob::borrowed(&data),
                },
            );
            report.check(
                format!("format {}: reading {what} doesn't crash", format.id),
                match answer {
                    Ok(Answer::Read { .. }) => Ok(()),
                    Ok(other) => Err(format!("answered {other:?}")),
                    Err(e) => not_a_crash(&e),
                },
            );
        }
    }
    for format in manifest.formats.iter().filter(|f| f.write) {
        let answer = plugins.send(
            found,
            &Request::Write {
                format: format.id.clone(),
                score: Box::new(Score::default()),
            },
        );
        report.check(
            format!("format {}: writing an empty score answers", format.id),
            match answer {
                Ok(Answer::Written { .. }) => Ok(()),
                Ok(other) => Err(format!("answered {other:?}")),
                Err(e) => not_a_crash(&e),
            },
        );
    }
    for service in &manifest.services {
        let answer = plugins.send(
            found,
            &Request::Probe {
                interface: service.interface.clone(),
                id: service.id.clone(),
            },
        );
        report.check(
            format!(
                "service {} of {}: the probe answers",
                service.id, service.interface
            ),
            match answer {
                Ok(Answer::Probe(_)) => Ok(()),
                Ok(other) => Err(format!("answered {other:?}")),
                Err(e) => Err(e.message),
            },
        );
    }
    report.messages = kept.0.lock().map(|m| m.clone()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&folder);
    report
}

static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// An error from a plugin is fine; a crash or a request it couldn't decode
/// breaks the contract.
fn not_a_crash(e: &Error) -> Result<(), String> {
    if e.message.contains("it crashed") || e.message.contains("couldn't read the request") {
        Err(e.message.clone())
    } else {
        Ok(())
    }
}
