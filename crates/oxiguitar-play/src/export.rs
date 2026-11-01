// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Audio export (PLAY-7, M4 slice 7): the score played through the engine
//! as a device would play it, as fast as it can, its sound handed to an
//! exporter plugin (plugin interface 1.4, `exporters` of kind `audio`), and
//! the bytes the plugin makes written to the file.
//!
//! The file is written under a temporary name beside the one asked for
//! (`.NAME.part`) and renamed at the end, so a cancelled or failed export
//! leaves nothing, and an existing file is replaced only by a whole one.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxiguitar_host::{ExporterRef, Plugins};
use oxiguitar_model::Score;

use crate::{Schedule, Timeline, TrackMix, engine, plugin_source};

/// How long the sound goes on after the end, so the last notes' release
/// is heard: two seconds, as `oxiguitar-cli play --wav` does.
pub const TAIL_SECONDS: f64 = 2.0;

/// The frames rendered at once.
const BLOCK: usize = 1024;

/// What to export: the user's choices in the export dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Where to start, in score seconds at 100 % speed.
    pub from: f64,
    /// Where to stop (the selection's end), or the song's end with `None`.
    pub until: Option<f64>,
    /// The speed, 0.25 to 2.0 (1.0: as written).
    pub speed: f64,
    /// The metronome's clicks in the sound.
    pub metronome: bool,
    /// Each track's mixer settings, by track.
    pub mixes: Vec<TrackMix>,
    /// The master level ([`engine::MASTER_USUAL`] as the window starts).
    pub master: f32,
}

impl Plan {
    /// The whole song as written, with the score's own mixer settings.
    pub fn whole(score: &Score) -> Plan {
        Plan {
            from: 0.0,
            until: None,
            speed: 1.0,
            metronome: false,
            mixes: score
                .tracks
                .iter()
                .map(|t| TrackMix {
                    volume: t.mix.volume,
                    pan: t.mix.pan,
                    mute: t.mix.mute,
                    solo: t.mix.solo,
                })
                .collect(),
            master: engine::MASTER_USUAL,
        }
    }
}

/// What an export made.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    /// The file written.
    pub path: PathBuf,
    /// How long its sound lasts, in seconds.
    pub seconds: f64,
    /// The rate it is at.
    pub rate: u32,
    /// The instruments that played.
    pub names: Vec<String>,
    /// Sentences about tracks left silent or effects left out.
    pub silent: Vec<String>,
}

/// Why an export didn't make a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The user stopped it.
    Cancelled,
    /// Something went wrong; the sentence says what.
    Failed(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Cancelled => f.write_str("the export was cancelled"),
            Failure::Failed(why) => f.write_str(why),
        }
    }
}

impl From<String> for Failure {
    fn from(why: String) -> Failure {
        Failure::Failed(why)
    }
}

/// How long the export's sound will be, in seconds: the part played at the
/// plan's speed, and the tail.
pub fn length(timeline: &Timeline, plan: &Plan) -> f64 {
    let end = plan.until.unwrap_or_else(|| timeline.length());
    (end - plan.from).max(0.0) / plan.speed + TAIL_SECONDS
}

/// The temporary file an export to `path` writes first.
pub fn partial(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.part"))
}

/// Exports `score` (its tracks' plugin settings in place) as `plan` says,
/// through `exporter`, to the file `path`. `progress` hears the share done,
/// 0 to 1, after each piece, and stops the export by answering `false`.
pub fn export(
    plugins: &Plugins,
    exporter: ExporterRef<'_>,
    score: &Score,
    plan: &Plan,
    path: &Path,
    progress: &mut dyn FnMut(f64) -> bool,
) -> Result<Exported, Failure> {
    let part = partial(path);
    let result = write(plugins, exporter, score, plan, path, &part, progress);
    match &result {
        Ok(_) => {}
        Err(_) => {
            let _ = std::fs::remove_file(&part);
        }
    }
    result
}

fn write(
    plugins: &Plugins,
    exporter: ExporterRef<'_>,
    score: &Score,
    plan: &Plan,
    path: &Path,
    part: &Path,
    progress: &mut dyn FnMut(f64) -> bool,
) -> Result<Exported, Failure> {
    let name = &exporter.spec.name;
    let mut export = plugins
        .start_audio_export(exporter)
        .map_err(|e| format!("{name} couldn't start: {}", e.message))?;
    let rate = export.rate;
    let timeline = Timeline::new(score);
    let found = plugin_source::plan(plugins, score);
    let prepared = plugin_source::prepare(plugins, score, &found, rate, BLOCK)?;

    let (mut engine, mut control) = engine(rate, BLOCK);
    let mut sources = prepared.sources;
    let schedule = Arc::new(Schedule::new(&timeline, &prepared.routes, prepared.count));
    sources.fit(&schedule);
    control.set_schedule(schedule);
    control.set_sources(sources);
    control.set_master(plan.master);
    for (t, mix) in plan.mixes.iter().enumerate() {
        control.set_mix(t, *mix);
    }
    control.set_speed(plan.speed);
    control.set_metronome(plan.metronome);
    control.play_until(plan.from, plan.until, false);

    let mut file = std::fs::File::create(part)
        .map_err(|e| format!("{} can't be written: {e}", part.display()))?;
    let written = |e: std::io::Error| format!("{} can't be written: {e}", part.display());
    file.write_all(&export.first).map_err(written)?;
    let mut size = export.first.len() as u64;

    // At most the part played and the tail, with ten seconds to spare: a
    // song that never says it ended still stops.
    let expected = length(&timeline, plan);
    let most = ((expected + 10.0) * f64::from(rate)) as u64;
    let tail = (TAIL_SECONDS * f64::from(rate)) as u64;
    let mut out = vec![0f32; BLOCK * 2];
    let (mut frames, mut left) = (0u64, None::<u64>);
    while frames < most && left.is_none_or(|l| l > 0) {
        engine.render(&mut out, 2, 0);
        // The tail's last block is cut to the tail's length.
        let n = left.map_or(BLOCK, |l| (l as usize).min(BLOCK));
        let bytes = export
            .samples(&out[..n * 2])
            .map_err(|e| format!("{name} failed: {}", e.message))?;
        file.write_all(&bytes).map_err(written)?;
        size += bytes.len() as u64;
        frames += n as u64;
        if let Some(l) = &mut left {
            *l = l.saturating_sub(n as u64);
        } else if control.collect().ended {
            left = Some(tail);
        }
        if !progress((frames as f64 / f64::from(rate) / expected).min(1.0)) {
            export.cancel();
            return Err(Failure::Cancelled);
        }
    }
    let end = export
        .finish()
        .map_err(|e| format!("{name} failed: {}", e.message))?;
    file.write_all(&end.last).map_err(written)?;
    size += end.last.len() as u64;
    if let Some(start) = end.start {
        if start.len() as u64 > size {
            return Err(Failure::Failed(format!(
                "{name} asked to rewrite {} bytes of a {size}-byte file",
                start.len()
            )));
        }
        file.seek(SeekFrom::Start(0)).map_err(written)?;
        file.write_all(&start).map_err(written)?;
    }
    file.sync_all().map_err(written)?;
    drop(file);
    std::fs::rename(part, path).map_err(|e| format!("{} can't be written: {e}", path.display()))?;
    Ok(Exported {
        path: path.to_path_buf(),
        seconds: frames as f64 / f64::from(rate),
        rate,
        names: prepared.names,
        silent: prepared.silent,
    })
}
