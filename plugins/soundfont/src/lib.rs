// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's SoundFont plugin (plugin interface 1.3; M4 slice 3,
//! docs/design/player.md): the instrument `soundfont`, which plays every
//! class of track through rustysynth 1.3.8, used exactly as published.
//!
//! - `sets` finds the sound sets: the user's `soundfonts` folder (beside
//!   `plugins`) and the system's own, read only; nothing is ever fetched.
//!   Until the user picks, the best set plays: the user's before the
//!   system's, the larger before the smaller.
//! - `play` turns the timeline's notes into rustysynth's: one synthesiser
//!   per track, a channel per string, notes starting within 7 frames.
//! - `sf2` writes SoundFont 2 files in memory: what other sets become
//!   before rustysynth reads them, and the contract suite's test set.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use oxiguitar_plugin::audio::Prepare;
use oxiguitar_plugin::{Error, ErrorKind, Host, Instrument, Plugin, Renderer, SharedFolder};
use rustysynth::SoundFont;

pub(crate) mod play;
pub(crate) mod sets;
// Used by the tests and the contract suite's test set for now; the DLS and
// SF3 readers write through it next.
#[cfg(test)]
pub(crate) mod sf2;

/// The instrument's id in the manifest.
pub const INSTRUMENT: &str = "soundfont";

/// A set loaded once and kept while it is the one chosen, so opening
/// another file doesn't load it again.
struct Loaded {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
    font: Arc<SoundFont>,
}

/// The plugin.
pub struct SoundFontPlugin {
    host: Host,
    /// The sets loaded for the last preparation.
    loaded: Mutex<Vec<Loaded>>,
}

impl std::fmt::Debug for SoundFontPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundFontPlugin").finish_non_exhaustive()
    }
}

impl SoundFontPlugin {
    /// The user's `soundfonts` folder, as the host names it.
    fn user_folder(&self) -> Option<PathBuf> {
        let folders = self.host.folders().ok()?;
        folders
            .shared(SharedFolder::Soundfonts)
            .map(Path::to_path_buf)
    }

    /// The sets this plugin can play, the best first.
    fn playable(&self) -> Vec<sets::Found> {
        sets::find(self.user_folder().as_deref(), &sets::system_places())
            .into_iter()
            .filter(|f| f.kind == sets::Kind::Sf2)
            .collect()
    }

    /// The set at `found`, loaded or taken from those already loaded.
    fn load(&self, found: &sets::Found) -> Result<Arc<SoundFont>, Error> {
        let meta = std::fs::metadata(&found.path).ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok());
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let mut loaded = self
            .loaded
            .lock()
            .map_err(|_| Error::new(ErrorKind::Failed, "the loaded sets"))?;
        if let Some(l) = loaded
            .iter()
            .find(|l| l.path == found.path && l.modified == modified && l.size == size)
        {
            return Ok(l.font.clone());
        }
        let file = std::fs::File::open(&found.path).map_err(|e| {
            Error::new(
                ErrorKind::Failed,
                format!("cannot open {}: {e}", found.path.display()),
            )
        })?;
        let font = SoundFont::new(&mut std::io::BufReader::new(file)).map_err(|e| {
            Error::new(
                ErrorKind::Invalid,
                format!(
                    "{} isn't a sound set it can read: {e}",
                    found.path.display()
                ),
            )
        })?;
        let font = Arc::new(font);
        // Only the set in use stays: an older one goes when the renderers
        // that hold it are let go of.
        loaded.clear();
        loaded.push(Loaded {
            path: found.path.clone(),
            modified,
            size,
            font: font.clone(),
        });
        Ok(font)
    }
}

impl Plugin for SoundFontPlugin {
    fn start(host: Host) -> Result<SoundFontPlugin, Error> {
        Ok(SoundFontPlugin {
            host,
            loaded: Mutex::default(),
        })
    }

    fn instrument(&self, id: &str) -> Option<&dyn Instrument> {
        (id == INSTRUMENT).then_some(self as &dyn Instrument)
    }
}

impl Instrument for SoundFontPlugin {
    fn prepare(&self, prepare: &Prepare) -> Result<Box<dyn Renderer>, Error> {
        let playable = self.playable();
        let Some(best) = playable.first() else {
            let place = self.user_folder().map_or_else(
                || "the soundfonts folder".to_owned(),
                |p| p.display().to_string(),
            );
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("no sound set found: put a SoundFont (.sf2) file in {place}"),
            ));
        };
        let font = self.load(best)?;
        Ok(Box::new(play::Player::new(prepare, &font)?))
    }
}

oxiguitar_plugin::export_plugin!(SoundFontPlugin);
