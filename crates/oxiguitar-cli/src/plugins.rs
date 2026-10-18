// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Files are opened through the format plugins, as in the app: the same
//! plugin folders and settings.json, read only (nothing is created or
//! saved). The core reads no format itself.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use oxiguitar_host::api::message::{Level as PluginLevel, LogEntry};
use oxiguitar_host::{Level, Messages, Plugins};
use oxiguitar_model::Score;

/// A file read by a format plugin.
pub(crate) struct Opened {
    /// The format's name, from the plugin's manifest ("Guitar Pro 5").
    pub(crate) format: String,
    /// The format's usual extensions, without the dot.
    pub(crate) extensions: Vec<String>,
    /// The plugin's name.
    pub(crate) plugin: String,
    /// The score.
    pub(crate) score: Score,
    /// What the reader couldn't carry over faithfully (FMT-2).
    pub(crate) log: Vec<LogEntry>,
}

/// Plugins' messages go to the standard error.
struct Stderr;

impl Messages for Stderr {
    fn notify(&self, _level: Level, text: String) {
        eprintln!("{text}");
    }
}

/// The plugins, found on first use. What is wrong with them (a plugin file
/// that can't be used, a damaged settings.json) is said once, on the
/// standard error.
pub(crate) fn plugins() -> &'static Plugins {
    static PLUGINS: OnceLock<Plugins> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        let found = oxiguitar_host::find(
            oxiguitar_host::Folders::here(),
            oxiguitar_host::display_system(),
            Arc::new(Stderr),
        );
        for notice in &found.notices {
            eprintln!("oxiguitar-cli: {}", notice.text);
        }
        found.plugins
    })
}

/// The extension of `path`, if any.
fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|e| e.to_str())
}

/// Which format plugin recognises the file at `path` holding `data`: the
/// format's name, its extensions and the plugin's name.
pub(crate) fn recognise(path: &str, data: &[u8]) -> Result<(String, Vec<String>, String), String> {
    let format = plugins()
        .recognise(data, extension(path))
        .map_err(|e| e.to_string())?;
    Ok((
        format.spec.name.clone(),
        format.spec.extensions.clone(),
        format.plugin.name(),
    ))
}

/// Reads the file at `path`, holding `data`. The error is a sentence.
pub(crate) fn open(path: &str, data: &[u8]) -> Result<Opened, String> {
    let plugins = plugins();
    let format = plugins
        .recognise(data, extension(path))
        .map_err(|e| e.to_string())?;
    let plugin = format.plugin.name();
    let (name, extensions) = (format.spec.name.clone(), format.spec.extensions.clone());
    let read = plugins.read(format, data).map_err(|e| e.to_string())?;
    let score = oxiguitar_model::wire::from_wire(read.score)
        .map_err(|e| format!("the {plugin} plugin gave a score this OxiGuitar can't hold ({e})"))?;
    Ok(Opened {
        format: name,
        extensions,
        plugin,
        score,
        log: read.log,
    })
}

/// " (named .gp3)" when the file's extension isn't one of the format's.
pub(crate) fn extension_note(path: &str, extensions: &[String]) -> String {
    match extension(path).map(str::to_ascii_lowercase) {
        Some(e) if !extensions.iter().any(|x| x.eq_ignore_ascii_case(&e)) => {
            format!(" (named .{e})")
        }
        _ => String::new(),
    }
}

/// How a log entry's level reads in `info`.
pub(crate) fn level_word(level: PluginLevel) -> &'static str {
    match level {
        PluginLevel::Info => "note",
        PluginLevel::Warning => "warning",
        PluginLevel::Error => "error",
    }
}
