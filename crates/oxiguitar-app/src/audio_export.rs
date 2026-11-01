// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! File → Export's audio exporters (M4 slice 7, PLAY-7; plugin interface
//! 1.4), without window code: what each entry exports, the plan the
//! export dialog makes, and the thread that renders and writes the file
//! (`oxiguitar_play::export`) while the window shows how far it has got.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use oxiguitar_host::{Level, Plugins};
use oxiguitar_model::Score;
use oxiguitar_play::export::{self, Failure, Plan};

/// An entry of File → Export that an exporter plugin adds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    /// Its plugin's id and its own.
    pub(crate) plugin: String,
    pub(crate) id: String,
    /// What it makes ("WAV Audio") and its files' extension.
    pub(crate) name: String,
    pub(crate) extension: String,
}

/// The audio exporters the ready plugins add, in plugin order.
pub(crate) fn targets(plugins: &Plugins) -> Vec<Target> {
    plugins
        .exporters()
        .into_iter()
        .filter(|e| e.spec.kind == oxiguitar_host::api::manifest::ExportKind::Audio)
        .map(|e| Target {
            plugin: e.plugin_id().to_owned(),
            id: e.spec.id.clone(),
            name: e.spec.name.clone(),
            extension: e.spec.extension.clone(),
        })
        .collect()
}

/// The file an export suggests: the score's PDF name (its file's stem) with
/// the exporter's extension.
pub(crate) fn file_name(pdf_name: &str, extension: &str) -> String {
    let stem = Path::new(pdf_name)
        .file_stem()
        .map_or_else(|| "score".to_owned(), |s| s.to_string_lossy().into_owned());
    format!("{stem}.{extension}")
}

/// `seconds` as minutes and seconds: "3:07".
fn clock(seconds: f64) -> String {
    let s = seconds.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What the footer says when an export is over: its level and sentence.
pub(crate) fn outcome(
    target: &Target,
    result: &Result<export::Exported, Failure>,
) -> (Level, String) {
    match result {
        Ok(made) => {
            let name = made
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut text = format!(
                "Exported {name} ({}, {} Hz).",
                clock(made.seconds),
                made.rate
            );
            if made.silent.is_empty() {
                (Level::Info, text)
            } else {
                for s in &made.silent {
                    text.push(' ');
                    text.push_str(s);
                }
                (Level::Warning, text)
            }
        }
        Err(Failure::Cancelled) => (
            Level::Info,
            format!("The {} export was cancelled.", target.name),
        ),
        Err(Failure::Failed(why)) => (
            Level::Error,
            format!(
                "The {} export failed: {}.",
                target.name,
                why.trim_end_matches('.')
            ),
        ),
    }
}

/// An export to start once the file is picked.
pub(crate) struct Job {
    pub(crate) plugins: Plugins,
    pub(crate) target: Target,
    pub(crate) score: Arc<Score>,
    pub(crate) plan: Plan,
    pub(crate) cancel: Arc<AtomicBool>,
}

impl Job {
    /// Renders and writes the file at `path` on this thread. `progress`
    /// hears the share done (0 to 1) now and then; the answer is for the
    /// footer.
    pub(crate) fn run(self, path: &Path, progress: &mut dyn FnMut(f32)) -> (Level, String) {
        let Some(exporter) = self
            .plugins
            .exporters()
            .into_iter()
            .find(|e| e.plugin_id() == self.target.plugin && e.spec.id == self.target.id)
        else {
            return (
                Level::Error,
                format!("{} is no longer available.", self.target.name),
            );
        };
        let mut shown = -1.0f64;
        let cancel = self.cancel.clone();
        let result = export::export(
            &self.plugins,
            exporter,
            &self.score,
            &self.plan,
            path,
            &mut |done| {
                // A hundred steps at most: the window needn't hear more.
                if done - shown >= 0.01 || done >= 1.0 {
                    shown = done;
                    progress(done as f32);
                }
                !cancel.load(Ordering::Relaxed)
            },
        );
        outcome(&self.target, &result)
    }
}

/// `path` with the extension `extension` when it has none.
pub(crate) fn with_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(extension);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            plugin: "oxiguitar.wav".into(),
            id: "wav".into(),
            name: "WAV Audio".into(),
            extension: "wav".into(),
        }
    }

    #[test]
    fn names_and_sentences() {
        assert_eq!(file_name("Song Title.pdf", "wav"), "Song Title.wav");
        assert_eq!(file_name("", "wav"), "score.wav");
        assert_eq!(
            with_extension(PathBuf::from("/m/take"), "wav"),
            PathBuf::from("/m/take.wav")
        );
        assert_eq!(
            with_extension(PathBuf::from("/m/take.flac"), "wav"),
            PathBuf::from("/m/take.flac")
        );
        let made = export::Exported {
            path: PathBuf::from("/m/song.wav"),
            seconds: 187.4,
            rate: 48_000,
            names: Vec::new(),
            silent: Vec::new(),
        };
        assert_eq!(
            outcome(&target(), &Ok(made.clone())),
            (Level::Info, "Exported song.wav (3:07, 48000 Hz).".into())
        );
        let quiet = export::Exported {
            silent: vec!["Nothing plays \u{201c}Choir\u{201d}.".into()],
            ..made
        };
        assert_eq!(outcome(&target(), &Ok(quiet)).0, Level::Warning);
        assert_eq!(
            outcome(&target(), &Err(Failure::Cancelled)),
            (Level::Info, "The WAV Audio export was cancelled.".into())
        );
        assert_eq!(
            outcome(&target(), &Err(Failure::Failed("the disk is full".into()))).1,
            "The WAV Audio export failed: the disk is full."
        );
    }
}
