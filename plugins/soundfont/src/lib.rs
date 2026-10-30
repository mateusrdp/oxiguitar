// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's SoundFont plugin (plugin interface 1.3; M4 slice 3,
//! docs/design/player.md): the instrument `soundfont`, which plays every
//! class of track through rustysynth 1.3.8, used exactly as published.
//!
//! - `sets` finds the sound sets: the user's `soundfonts` folder (beside
//!   `plugins`) and the system's own, read only; nothing is ever fetched.
//! - `settings` is the settings window: a set for guitars, basses, drums
//!   and everything else; until the user picks, the best set found (the
//!   user's before the system's, the larger before the smaller).
//! - `play` turns the timeline's notes into rustysynth's: one synthesiser
//!   per track, a channel per string, notes starting within 7 frames.
//! - rustysynth reads only SoundFont 2, so `sf3` (SoundFont 3: Ogg Vorbis
//!   samples, decoded with lewton) and `dls` (Downloadable Sounds, as
//!   Windows and macOS ship their General MIDI set) turn the others into
//!   SoundFont 2 in memory, written by `sf2`. A decoded SoundFont 3 is kept
//!   in the plugin's cache folder, so it is decoded once.
//! - `sfz` reads SFZ instruments and their samples and plays them: our own
//!   sampler (an SFZ instrument plays only where the settings pick it).

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use oxiguitar_plugin::audio::Prepare;
use oxiguitar_plugin::{
    Error, ErrorKind, Host, Instrument, Level, Plugin, Renderer, SharedFolder, Tab,
};
use rustysynth::SoundFont;

pub(crate) mod dls;
pub(crate) mod play;
pub(crate) mod riff;
pub(crate) mod sets;
pub(crate) mod settings;
pub(crate) mod sf2;
pub(crate) mod sf3;
pub(crate) mod sfz;

/// The instrument's id in the manifest.
pub const INSTRUMENT: &str = "soundfont";

/// The manual's page on getting better sets, which the footer's warning
/// links to.
pub const BETTER_SOUNDS: &str = "https://mateusrdp.github.io/oxiguitar/better-sounds.html";

/// A set loaded once and kept while it is in use, so opening another file
/// doesn't load it again.
struct Loaded {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
    font: Arc<SoundFont>,
}

/// An SFZ instrument loaded, as its file was then.
struct LoadedSfz {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
    sfz: Arc<sfz::Loaded>,
}

/// The plugin.
pub struct SoundFontPlugin {
    host: Host,
    /// The sets loaded for the last preparation.
    loaded: Mutex<Vec<Loaded>>,
    /// The SFZ instruments loaded for it, and their samples.
    loaded_sfz: Mutex<Vec<LoadedSfz>>,
    samples: sfz::Samples,
    window: settings::Window,
}

impl std::fmt::Debug for SoundFontPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundFontPlugin").finish_non_exhaustive()
    }
}

/// The user's `soundfonts` folder, as the host names it.
pub(crate) fn user_folder(host: &Host) -> Option<PathBuf> {
    let folders = host.folders().ok()?;
    folders
        .shared(SharedFolder::Soundfonts)
        .map(Path::to_path_buf)
}

/// The sets this plugin can play, the best first.
pub(crate) fn playable(host: &Host) -> Vec<sets::Found> {
    sets::find(user_folder(host).as_deref(), &sets::system_places())
}

/// A short, stable name for a file's contents: its path, size and time
/// (FNV-1a), for the cache of decoded sets.
fn fingerprint(path: &Path, size: u64, modified: Option<SystemTime>) -> u64 {
    let since = modified
        .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = path
        .as_os_str()
        .to_string_lossy()
        .into_owned()
        .into_bytes()
        .into_iter()
        .chain(size.to_le_bytes())
        .chain(since.to_le_bytes());
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn read_error(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Failed,
        format!("cannot read {}: {e}", path.display()),
    )
}

fn unplayable(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Invalid,
        format!("{} isn't a sound set it can play: {e}", path.display()),
    )
}

impl SoundFontPlugin {
    /// The SFZ instrument `found` with its samples, loaded, or taken from
    /// those already loaded. Samples it can't read are said in the footer.
    fn load_sfz(&self, found: &sets::Found) -> Result<Arc<sfz::Loaded>, Error> {
        let meta = std::fs::metadata(&found.path).map_err(|e| read_error(&found.path, e))?;
        let modified = meta.modified().ok();
        let size = meta.len();
        if let Ok(loaded) = self.loaded_sfz.lock()
            && let Some(l) = loaded
                .iter()
                .find(|l| l.path == found.path && l.modified == modified && l.size == size)
        {
            return Ok(l.sfz.clone());
        }
        let sfz = Arc::new(
            sfz::load(&found.path, &self.samples).map_err(|e| unplayable(&found.path, e))?,
        );
        if let Some(first) = sfz.missing.first() {
            let name = found.path.file_name().map_or_else(
                || found.path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            self.host.notify(
                Level::Warning,
                format!("{name} plays without some of its samples: {first}"),
            );
        }
        if let Ok(mut loaded) = self.loaded_sfz.lock() {
            loaded.push(LoadedSfz {
                path: found.path.clone(),
                modified,
                size,
                sfz: sfz.clone(),
            });
        }
        Ok(sfz)
    }

    /// The set `found`, loaded, or taken from those already loaded.
    fn load(&self, found: &sets::Found) -> Result<Arc<SoundFont>, Error> {
        let meta = std::fs::metadata(&found.path).map_err(|e| read_error(&found.path, e))?;
        let modified = meta.modified().ok();
        let size = meta.len();
        if let Ok(loaded) = self.loaded.lock()
            && let Some(l) = loaded
                .iter()
                .find(|l| l.path == found.path && l.modified == modified && l.size == size)
        {
            return Ok(l.font.clone());
        }
        let font = Arc::new(self.read(&found.path, size, modified)?);
        if let Ok(mut loaded) = self.loaded.lock() {
            loaded.push(Loaded {
                path: found.path.clone(),
                modified,
                size,
                font: font.clone(),
            });
        }
        Ok(font)
    }

    /// Reads the set at `path`, whatever its kind: what its first bytes
    /// say, not its name.
    fn read(
        &self,
        path: &Path,
        size: u64,
        modified: Option<SystemTime>,
    ) -> Result<SoundFont, Error> {
        let mut head = [0u8; 4096];
        let n = std::fs::File::open(path)
            .and_then(|mut f| f.read(&mut head))
            .map_err(|e| read_error(path, e))?;
        let head = &head[..n];
        if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"DLS ") {
            let file = std::fs::read(path).map_err(|e| read_error(path, e))?;
            let (sf2, _left_out) = dls::to_sf2(&file).map_err(|e| unplayable(path, e.message))?;
            return SoundFont::new(&mut std::io::Cursor::new(&sf2))
                .map_err(|e| unplayable(path, e));
        }
        if !sf3::is_sf3(head) {
            // SoundFont 2, read straight from the file.
            let file = std::fs::File::open(path).map_err(|e| read_error(path, e))?;
            return SoundFont::new(&mut std::io::BufReader::new(file))
                .map_err(|e| unplayable(path, e));
        }
        // SoundFont 3: decoded once, then kept in the cache folder.
        let cache = self.host.folders().ok().map(|f| {
            f.cache
                .join(format!("{:016x}.sf2", fingerprint(path, size, modified)))
        });
        if let Some(cached) = &cache
            && let Ok(file) = std::fs::File::open(cached)
            && let Ok(font) = SoundFont::new(&mut std::io::BufReader::new(file))
        {
            return Ok(font);
        }
        let file = std::fs::read(path).map_err(|e| read_error(path, e))?;
        let sf2 = sf3::to_sf2(&file).map_err(|e| unplayable(path, e.message))?;
        drop(file);
        let font =
            SoundFont::new(&mut std::io::Cursor::new(&sf2)).map_err(|e| unplayable(path, e))?;
        if let Some(cached) = &cache {
            // Written whole under another name, then renamed: a set half
            // written is never read. A cache that can't be written only
            // means decoding again next time.
            let part = cached.with_extension("part");
            if std::fs::write(&part, &sf2).is_ok() {
                let _ = std::fs::rename(&part, cached);
            }
            let _ = std::fs::remove_file(&part);
        }
        Ok(font)
    }

    /// Says once, ever, that only the system's sets play, with a link to
    /// the manual's page on better ones.
    fn warn_once(&self) {
        let Ok(mut s) = self.host.settings() else {
            return;
        };
        if s.get(settings::WARNED).and_then(|v| v.as_bool()) == Some(true) {
            return;
        }
        self.host.notify_with_link(
            Level::Warning,
            "Playing with your system's basic sound set. Free recorded SFZ sets for guitars, basses and drums sound far better.",
            "Better sounds",
            BETTER_SOUNDS,
        );
        s.insert(settings::WARNED.into(), true.into());
        let _ = self.host.set_settings(&s);
    }
}

impl Plugin for SoundFontPlugin {
    fn start(host: Host) -> Result<SoundFontPlugin, Error> {
        Ok(SoundFontPlugin {
            window: settings::Window::new(host.clone()),
            host,
            loaded: Mutex::default(),
            loaded_sfz: Mutex::default(),
            samples: sfz::Samples::default(),
        })
    }

    fn instrument(&self, id: &str) -> Option<&dyn Instrument> {
        (id == INSTRUMENT).then_some(self as &dyn Instrument)
    }

    fn settings_window(&self) -> Option<&dyn Tab> {
        Some(&self.window)
    }
}

impl Instrument for SoundFontPlugin {
    fn prepare(&self, prepare: &Prepare) -> Result<Box<dyn Renderer>, Error> {
        let found = playable(&self.host);
        if found.is_empty() {
            let place = user_folder(&self.host).map_or_else(
                || "the soundfonts folder".to_owned(),
                |p| p.display().to_string(),
            );
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("no sound set found: put a SoundFont (.sf2 or .sf3) file in {place}"),
            ));
        }
        let picks = self.host.settings().unwrap_or_default();
        let mut used: Vec<&sets::Found> = Vec::new();
        let mut sounds = Vec::with_capacity(prepare.tracks.len());
        let mut said: Vec<&std::path::Path> = Vec::new();
        for t in &prepare.tracks {
            let pick = settings::chosen(&picks, settings::Role::of(t), &found);
            // The pick, then every General MIDI set in order, until one
            // loads.
            let mut first_error = None;
            let mut sound = None;
            for f in pick
                .into_iter()
                .chain(found.iter().filter(|f| f.kind != sets::Kind::Sfz))
            {
                let loaded = if f.kind == sets::Kind::Sfz {
                    self.load_sfz(f).map(play::Sound::Sfz)
                } else {
                    self.load(f).map(play::Sound::Font)
                };
                match loaded {
                    Ok(loaded) => {
                        if !used.iter().any(|u| u.path == f.path) {
                            used.push(f);
                        }
                        sound = Some(loaded);
                        break;
                    }
                    Err(e) => {
                        if f.kind == sets::Kind::Sfz && !said.contains(&f.path.as_path()) {
                            said.push(&f.path);
                            self.host.notify(
                                Level::Warning,
                                format!("{}; playing a General MIDI set instead", e.message),
                            );
                        }
                        first_error.get_or_insert(e);
                    }
                }
            }
            match sound {
                Some(s) => sounds.push(s),
                None => {
                    return Err(first_error
                        .unwrap_or_else(|| Error::new(ErrorKind::NotFound, "no sound set loads")));
                }
            }
        }
        // Only the sets in use stay loaded, and only their samples.
        if let Ok(mut loaded) = self.loaded.lock() {
            loaded.retain(|l| used.iter().any(|u| u.path == l.path));
        }
        if let Ok(mut loaded) = self.loaded_sfz.lock() {
            loaded.retain(|l| used.iter().any(|u| u.path == l.path));
        }
        self.samples.trim();
        if settings::only_basic(&used) {
            self.warn_once();
        }
        Ok(Box::new(play::Player::new(prepare, &sounds)?))
    }
}

oxiguitar_plugin::export_plugin!(SoundFontPlugin);
