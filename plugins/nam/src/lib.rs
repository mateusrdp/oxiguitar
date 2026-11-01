// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's NAM plugin (plugin interface 1.4; M4 slice 6,
//! docs/design/player.md): the effect `nam`, which plays a guitar or bass
//! track through an amp model (a Neural Amp Modeler capture, `.nam`) and a
//! cabinet impulse response, after its instrument; and the track dialog's
//! section `nam`, where each track picks them.
//!
//! - `files` finds the models and responses in the `nam` folder (beside
//!   `plugins` and `soundfonts`); nothing is ever fetched.
//! - `settings` is a track's choice; `section` shows it in the track
//!   dialog.
//! - The model runs through nam-rs 0.4.0 (MIT), used exactly as published,
//!   at the rate it was captured at: a track played at another rate goes
//!   there and back through `resample`.
//! - `convolve` applies the cabinet's response with no latency; four
//!   responses from Guitarix's BestPlugins pack (GPL-2.0-or-later) are
//!   built in (`irs/`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use oxiguitar_plugin::audio::PrepareEffect;
use oxiguitar_plugin::{
    Effect, Error, ErrorKind, Host, Plugin, Processor, SharedFolder, TrackSection,
};

pub(crate) mod convolve;
pub(crate) mod files;
pub(crate) mod resample;
pub(crate) mod section;
pub(crate) mod settings;
pub(crate) mod wav;

use settings::{Cabinet, Track};

/// The effect's id in the manifest.
pub const EFFECT: &str = "nam";

/// The track dialog's section's id in the manifest.
pub const SECTION: &str = "nam";

/// The manual's page on amp models, which the section links to.
pub const MANUAL: &str = "https://mateusrdp.github.io/oxiguitar/amp-models.html";

/// The level a model's sound is brought to, where its file says how loud
/// it plays, in dBFS (as NAM's own plugin does).
const TARGET: f32 = -18.0;

/// The cabinet responses built in, by name, the first the default.
pub const BUILT_IN: [(&str, &[u8]); 4] = [
    (
        "mesa-boogie-mark-v",
        include_bytes!("../irs/mesa-boogie-mark-v.wav"),
    ),
    (
        "marshall-jmp-2203-mod",
        include_bytes!("../irs/marshall-jmp-2203-mod.wav"),
    ),
    ("evh-5150-iii", include_bytes!("../irs/evh-5150-iii.wav")),
    (
        "engl-retro-tube",
        include_bytes!("../irs/engl-retro-tube.wav"),
    ),
];

/// A built-in response's name as people read it.
pub(crate) fn built_in_title(name: &str) -> &'static str {
    match name {
        "mesa-boogie-mark-v" => "Mesa Boogie Mark V",
        "marshall-jmp-2203-mod" => "Marshall JMP 2203 (modded)",
        "evh-5150-iii" => "EVH 5150 III",
        "engl-retro-tube" => "Engl Retro Tube",
        _ => "",
    }
}

/// A model file read once and kept while it is unchanged, so tracks and
/// later preparations share it.
struct Loaded {
    path: PathBuf,
    modified: Option<SystemTime>,
    file: Arc<nam_rs::NamModel>,
}

/// The plugin.
pub struct NamPlugin {
    host: Host,
    section: section::Section,
    loaded: Mutex<Vec<Loaded>>,
}

impl std::fmt::Debug for NamPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamPlugin").finish_non_exhaustive()
    }
}

/// The `nam` folder, as the host hands it over.
pub(crate) fn folder(host: &Host) -> Option<PathBuf> {
    let folders = host.folders().ok()?;
    folders.shared(SharedFolder::Nam).map(Path::to_path_buf)
}

fn failed(message: String) -> Error {
    Error::new(ErrorKind::Failed, message)
}

fn decibels(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl NamPlugin {
    /// The model file at `path`, read or from the ones already read.
    fn model_file(&self, path: &Path) -> Result<Arc<nam_rs::NamModel>, Error> {
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let mut loaded = self
            .loaded
            .lock()
            .map_err(|_| failed("the models in memory are damaged".into()))?;
        if let Some(l) = loaded
            .iter()
            .find(|l| l.path == path && l.modified == modified)
        {
            return Ok(l.file.clone());
        }
        let file = nam_rs::NamModel::from_file(path)
            .map_err(|e| failed(format!("{} can't be read: {e}", path.display())))?;
        let file = Arc::new(file);
        loaded.retain(|l| l.path != path);
        loaded.push(Loaded {
            path: path.to_path_buf(),
            modified,
            file: file.clone(),
        });
        Ok(file)
    }

    /// The taps of the cabinet `track` picked, at `rate`, or none.
    fn cabinet(
        &self,
        track: &Track,
        includes_cab: Option<bool>,
        nam: Option<&Path>,
        rate: u32,
    ) -> Result<Option<Vec<f32>>, Error> {
        let data: Vec<u8>;
        let bytes: &[u8] = match &track.cabinet {
            Cabinet::None => return Ok(None),
            // A model of a whole rig has its cabinet already, and the
            // first built-in response is what a bare amp gets.
            Cabinet::Auto if includes_cab == Some(true) || track.model.is_none() => {
                return Ok(None);
            }
            Cabinet::Auto => BUILT_IN[0].1,
            Cabinet::BuiltIn(name) => BUILT_IN
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, d)| *d)
                .ok_or_else(|| failed(format!("there is no built-in cabinet called {name}")))?,
            Cabinet::File(name) => {
                let path = nam
                    .and_then(|f| files::resolve(f, name))
                    .ok_or_else(|| failed(format!("the cabinet {name} isn't in the nam folder")))?;
                data = std::fs::read(&path)
                    .map_err(|e| failed(format!("{} can't be read: {e}", path.display())))?;
                &data
            }
        };
        let response = wav::read(bytes).map_err(|e| failed(format!("the cabinet's file: {e}")))?;
        let taps = resample::convert(&response.taps, response.rate, rate);
        Ok(Some(convolve::normalise(taps, rate)))
    }
}

impl Plugin for NamPlugin {
    fn start(host: Host) -> Result<NamPlugin, Error> {
        Ok(NamPlugin {
            section: section::Section::new(host.clone()),
            host,
            loaded: Mutex::default(),
        })
    }

    fn effect(&self, id: &str) -> Option<&dyn Effect> {
        (id == EFFECT).then_some(self as &dyn Effect)
    }

    fn track_section(&self, id: &str) -> Option<&dyn TrackSection> {
        (id == SECTION).then_some(&self.section as &dyn TrackSection)
    }
}

impl Effect for NamPlugin {
    fn prepare(&self, prepare: &PrepareEffect) -> Result<Box<dyn Processor>, Error> {
        let track = Track::from_text(&prepare.track.settings);
        if !track.on {
            return Ok(Box::new(Through));
        }
        let nam = folder(&self.host);
        let frames = (prepare.frames as usize).max(1);
        let mut model = None;
        let mut includes_cab = None;
        let mut level = 1.0;
        if let Some(name) = &track.model {
            let path = nam
                .as_deref()
                .and_then(|f| files::resolve(f, name))
                .ok_or_else(|| failed(format!("the model {name} isn't in the nam folder")))?;
            let file = self.model_file(&path)?;
            includes_cab = file.includes_cab();
            if let Some(loudness) = file.loudness() {
                level = decibels(TARGET - loudness);
            }
            let mut runtime = nam_rs::Model::from_nam(&file)
                .map_err(|e| failed(format!("{name} can't be played: {e}")))?;
            // A fresh model's first sound is a transient against silence:
            // spent now, not on the first note.
            let mut warm = vec![0.0f32; runtime.receptive_field().max(1)];
            runtime.process_buffer(&mut warm);
            let theirs = file.expected_sample_rate().round() as u32;
            let converted = (theirs != prepare.rate && theirs > 0)
                .then(|| resample::Converted::new(prepare.rate, theirs, frames));
            model = Some((runtime, converted));
        }
        let cabinet = self
            .cabinet(&track, includes_cab, nam.as_deref(), prepare.rate)?
            .map(|taps| convolve::Convolver::new(&taps));
        Ok(Box::new(Amp {
            model,
            cabinet,
            input: decibels(track.input),
            output: decibels(track.output) * level,
            mono: vec![0.0; frames],
        }))
    }
}

/// NAM off on a track: its sound passes untouched.
struct Through;

impl Processor for Through {
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32]) {}
}

/// A track through its amp model and cabinet.
struct Amp {
    model: Option<(nam_rs::Model, Option<resample::Converted>)>,
    cabinet: Option<convolve::Convolver>,
    input: f32,
    output: f32,
    /// The track's sound in mono, as long as the longest piece.
    mono: Vec<f32>,
}

impl Processor for Amp {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let most = self.mono.len();
        let mut at = 0;
        while at < left.len() {
            let n = (left.len() - at).min(most);
            let (l, r) = (&mut left[at..at + n], &mut right[at..at + n]);
            let mono = &mut self.mono[..n];
            for ((m, a), b) in mono.iter_mut().zip(l.iter()).zip(r.iter()) {
                *m = (a + b) * 0.5 * self.input;
            }
            if let Some((model, converted)) = &mut self.model {
                match converted {
                    Some(c) => c.process(mono, |x| model.process_buffer(x)),
                    None => model.process_buffer(mono),
                }
            }
            if let Some(c) = &mut self.cabinet {
                c.process(mono);
            }
            for ((m, a), b) in mono.iter().zip(l.iter_mut()).zip(r.iter_mut()) {
                let v = m * self.output;
                *a = v;
                *b = v;
            }
            at += n;
        }
    }

    fn latency(&self) -> u32 {
        self.model
            .as_ref()
            .and_then(|(_, c)| c.as_ref())
            .map_or(0, |c| c.latency() as u32)
    }
}

oxiguitar_plugin::export_plugin!(NamPlugin);
