// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Files are opened through the format plugins, as in the app: the same
//! plugin folders and settings.json, read only (nothing is created or
//! saved). The core reads no format itself.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use oxiguitar_host::api::message::{Level as PluginLevel, LogEntry};
use oxiguitar_host::{Core, Level, Plugins};
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

/// The score file the command plays or exports, for plugins that keep
/// paths relative to it (interface 1.13).
static SCORE_FILE: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// The command works on the score file `path`.
pub(crate) fn set_score_file(path: &Path) {
    *SCORE_FILE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(std::path::absolute(path).unwrap_or_else(|_| path.to_owned()));
}

impl Core for Stderr {
    fn notify(&self, _level: Level, text: String) {
        eprintln!("{text}");
    }

    fn score_file(&self) -> Option<std::path::PathBuf> {
        SCORE_FILE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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
            oxiguitar_host::NO_DISPLAY,
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
    // A plugin that fails while reading is withdrawn and the next one
    // that recognises the file reads it.
    let opened = plugins()
        .open(data, extension(path))
        .map_err(|e| e.to_string())?;
    let format = opened.format;
    let plugin = format.plugin.name();
    let (name, extensions) = (format.spec.name.clone(), format.spec.extensions.clone());
    let read = opened.read;
    let mut score = oxiguitar_model::wire::from_wire2(read.score)
        .map_err(|e| format!("the {plugin} plugin gave a score this OxiGuitar can't hold ({e})"))?;
    // Pitches on strings get a string and fret, as the app does (#157).
    oxiguitar_edit::finger_pitched(&mut score);
    oxiguitar_edit::plain_spellings(&mut score);
    // The whole score, when the user had what the format can't hold kept
    // beside the file (FILE.oxg) and the file is still the one saved: as
    // the app opens it, with nothing left out.
    let mut log = read.log;
    if let Ok(Some(sidecar)) = oxiguitar_host::sidecar::Sidecar::read_for(Path::new(path))
        && let oxiguitar_host::sidecar::Kept::Score(kept) = sidecar.kept_score(data)
        && let Ok(whole) = oxiguitar_model::wire::from_wire2(*kept)
    {
        score = whole;
        log.clear();
    }
    Ok(Opened {
        format: name,
        extensions,
        plugin,
        score,
        log,
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

/// Gives `score` what the file beside `path` keeps (FILE.oxg): the tracks'
/// plugin settings and effect order, played chords, and what plugins keep
/// on the score, its bars, beats and notes (interface 1.8), as the window
/// reads them. The error is a sentence.
pub(crate) fn apply_sidecar(path: &str, score: &mut Score) -> Result<(), String> {
    let Some(mut sidecar) = oxiguitar_host::sidecar::Sidecar::read_for(Path::new(path))? else {
        return Ok(());
    };
    sidecar.adopt(&plugins().replaced_ids());
    let names: Vec<String> = score.tracks.iter().map(|t| t.name.clone()).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    for (track, entry) in score.tracks.iter_mut().zip(sidecar.settle(&names)) {
        track.plugins = entry.plugins;
        track.effects = entry.effects;
        track.play_chords = entry.chords;
        for kept in entry.kept {
            track.keep(
                &kept.at,
                oxiguitar_model::Kept {
                    plugins: kept.plugins,
                    marks: kept.marks,
                },
            );
        }
    }
    if !sidecar.plugins.is_empty() {
        score.plugins = sidecar.plugins;
    }
    for kept in sidecar.bars {
        if let [bar] = kept.at[..]
            && let Some(m) = score.master_bars.get_mut(bar)
        {
            m.plugins = kept.plugins;
        }
    }
    Ok(())
}
