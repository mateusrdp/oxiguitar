// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! SFZ instruments (M4 slice 5, docs/design/player.md): our own sampler,
//! since no Rust SFZ player fits. The best free recorded guitars, basses
//! and drum kits are SFZ (Karoryfer's, SM Drums, Naked Drums…).
//!
//! - `text` reads a `.sfz` file into headers and opcodes, with `#define`
//!   and `#include`, as sfizz and ARIA read them.
//! - `region` turns opcodes into values, with sfizz's defaults.
//! - `instrument` gathers regions under their headers, with the
//!   instrument's controllers, curves and what it holds that this player
//!   doesn't know.
//! - `sample` reads WAV, FLAC and Ogg Vorbis samples whole into memory, at
//!   their own bit depth.
//!
//! [`load`] reads an instrument and all its samples, in parallel, before
//! anything plays, so the audio thread never waits for the disk.

pub(crate) mod instrument;
pub(crate) mod region;
pub(crate) mod sample;
pub(crate) mod text;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub(crate) use instrument::Instrument;
pub(crate) use sample::Sample;

/// An instrument with its samples in memory.
#[derive(Debug)]
pub(crate) struct Loaded {
    pub(crate) instrument: Instrument,
    /// The instrument's samples, by index; `None` where one couldn't be
    /// read (its regions stay silent).
    pub(crate) samples: Vec<Option<Arc<Sample>>>,
    /// Samples that couldn't be read, and why.
    pub(crate) missing: Vec<String>,
}

impl Loaded {
    /// The memory its samples take.
    pub(crate) fn bytes(&self) -> usize {
        self.samples.iter().flatten().map(|s| s.bytes()).sum()
    }
}

/// A sample file as it was when read: a changed file is read again.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    fn of(path: &Path) -> Option<Stamp> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Stamp {
            path: path.to_path_buf(),
            size: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

/// Samples already in memory, shared by the instruments that play them,
/// so opening another file or another instrument of the same set reads
/// nothing twice.
#[derive(Debug, Default)]
pub(crate) struct Samples {
    kept: Mutex<Vec<(Stamp, Arc<Sample>)>>,
}

impl Samples {
    fn get(&self, stamp: &Stamp) -> Option<Arc<Sample>> {
        let kept = self.kept.lock().ok()?;
        kept.iter()
            .find(|(s, _)| s == stamp)
            .map(|(_, a)| a.clone())
    }

    fn put(&self, stamp: Stamp, sample: Arc<Sample>) {
        if let Ok(mut kept) = self.kept.lock() {
            kept.retain(|(s, _)| s.path != stamp.path);
            kept.push((stamp, sample));
        }
    }

    /// Lets go of every sample that only this cache holds: those no
    /// loaded instrument plays any more.
    pub(crate) fn trim(&self) {
        if let Ok(mut kept) = self.kept.lock() {
            kept.retain(|(_, a)| Arc::strong_count(a) > 1);
        }
    }
}

/// A sample being read: nothing yet, then the sample or why not.
type Slot = Mutex<Option<Result<Arc<Sample>, String>>>;

/// Reads the instrument at `path` and every sample it plays, the samples
/// in parallel on every core.
pub(crate) fn load(path: &Path, cache: &Samples) -> Result<Loaded, String> {
    let instrument = instrument::read(path)?;
    if instrument.regions.is_empty() {
        return Err(format!("{} has no regions", path.display()));
    }
    let n = instrument.samples.len();
    let slots: Vec<Slot> = (0..n).map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(n.max(1));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(file) = instrument.samples.get(i) else {
                        break;
                    };
                    let result = match Stamp::of(file) {
                        None => Err(format!("{} is missing", file.display())),
                        Some(stamp) => match cache.get(&stamp) {
                            Some(s) => Ok(s),
                            None => sample::read(file).map(|s| {
                                let s = Arc::new(s);
                                cache.put(stamp, s.clone());
                                s
                            }),
                        },
                    };
                    if let Ok(mut slot) = slots[i].lock() {
                        *slot = Some(result);
                    }
                }
            });
        }
    });
    let mut samples = Vec::with_capacity(n);
    let mut missing = Vec::new();
    for slot in slots {
        match slot.into_inner().ok().flatten() {
            Some(Ok(s)) => samples.push(Some(s)),
            Some(Err(e)) => {
                if missing.len() < 20 {
                    missing.push(e);
                }
                samples.push(None);
            }
            None => samples.push(None),
        }
    }
    if samples.iter().all(Option::is_none) {
        return Err(format!(
            "none of the samples of {} could be read{}",
            path.display(),
            missing.first().map_or(String::new(), |e| format!(": {e}"))
        ));
    }
    Ok(Loaded {
        instrument,
        samples,
        missing,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::sfz::sample::tests::wav_file;

    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oxiguitar-sfz-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_instrument_loads_with_its_samples_shared_and_the_missing_said() {
        let dir = scratch("load");
        std::fs::create_dir_all(dir.join("Samples")).unwrap();
        std::fs::create_dir_all(dir.join("Programs")).unwrap();
        std::fs::write(
            dir.join("Samples/a.wav"),
            wav_file(48_000, 1, 16, &[1, 2, 3], None),
        )
        .unwrap();
        std::fs::write(
            dir.join("Samples/b.wav"),
            wav_file(48_000, 2, 24, &[1, 2], None),
        )
        .unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sfz");
        std::fs::copy(fixtures.join("sine-24.flac"), dir.join("Samples/c.flac")).unwrap();
        std::fs::write(
            dir.join("Programs/kit.sfz"),
            "<control> default_path=../Samples/\n\
             <region> sample=a.wav key=36\n<region> sample=b.wav key=38\n\
             <region> sample=c.flac key=40\n<region> sample=gone.wav key=42\n<region> sample=a.wav key=43",
        )
        .unwrap();
        let cache = Samples::default();
        let loaded = load(&dir.join("Programs/kit.sfz"), &cache).unwrap();
        assert_eq!(loaded.instrument.regions.len(), 5);
        assert_eq!(loaded.samples.len(), 4);
        assert!(loaded.samples[3].is_none());
        assert_eq!(loaded.missing.len(), 1);
        assert!(loaded.missing[0].contains("gone.wav"));
        assert_eq!(loaded.samples[0].as_ref().unwrap().frames, 3);
        assert!(loaded.bytes() > 4_000);
        // A second load takes the samples already in memory.
        let again = load(&dir.join("Programs/kit.sfz"), &cache).unwrap();
        assert!(Arc::ptr_eq(
            loaded.samples[2].as_ref().unwrap(),
            again.samples[2].as_ref().unwrap()
        ));
        drop((loaded, again));
        cache.trim();
        assert!(cache.kept.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_instrument_without_regions_or_samples_is_refused() {
        let dir = scratch("refused");
        std::fs::write(dir.join("empty.sfz"), "<control> default_path=x/").unwrap();
        std::fs::write(dir.join("gone.sfz"), "<region> sample=gone.wav").unwrap();
        let cache = Samples::default();
        assert!(
            load(&dir.join("empty.sfz"), &cache)
                .unwrap_err()
                .contains("no regions")
        );
        assert!(
            load(&dir.join("gone.sfz"), &cache)
                .unwrap_err()
                .contains("none of the samples")
        );
        assert!(load(&dir.join("nothing.sfz"), &cache).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
