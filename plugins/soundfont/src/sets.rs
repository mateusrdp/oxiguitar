// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Finding sound sets (docs/design/player.md, "Sounds"): the files in the
//! user's `soundfonts` folder and the system's own, read only, never
//! fetched. Until the user picks, the best set plays: the user's before the
//! system's, the larger before the smaller.

use std::path::{Path, PathBuf};

/// Where a set was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Origin {
    /// The user's `soundfonts` folder.
    User,
    /// The system's own sets: every one of them counts as basic.
    System,
}

/// A set's format, from its name's extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// SoundFont 2.
    Sf2,
    /// SoundFont 3: SoundFont 2 with Ogg Vorbis samples.
    Sf3,
    /// Downloadable Sounds, as Windows and macOS ship their General MIDI set.
    Dls,
}

impl Kind {
    /// The kind of the file at `path`, by its extension; `None` for any
    /// other file.
    pub(crate) fn of(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "sf2" => Some(Kind::Sf2),
            "sf3" => Some(Kind::Sf3),
            "dls" => Some(Kind::Dls),
            _ => None,
        }
    }
}

/// A set found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    /// The file, with every link followed.
    pub(crate) path: PathBuf,
    /// Its size, in bytes.
    pub(crate) size: u64,
    /// Where it was found.
    pub(crate) origin: Origin,
    /// Its format.
    pub(crate) kind: Kind,
}

/// The system's own places for sets, files or folders, read only.
pub(crate) fn system_places() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let root = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        vec![root.join("System32").join("drivers").join("gm.dls")]
    }
    #[cfg(target_os = "macos")]
    {
        vec![PathBuf::from(
            "/System/Library/Components/CoreAudio.component/Contents/Resources/gs_instruments.dls",
        )]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        vec![
            PathBuf::from("/usr/share/sounds/sf2"),
            PathBuf::from("/usr/share/sounds/sf3"),
            PathBuf::from("/usr/share/soundfonts"),
        ]
    }
}

/// How deep into a folder's subfolders sets are looked for: a set unpacked
/// into a folder of its own, and one more level.
const DEPTH: usize = 3;

/// Every set in the user's folder `user` and the system's `system` (files
/// or folders), the best first: the user's before the system's, then the
/// larger before the smaller (then by path, so the order is always the
/// same). A file reached twice (a link, a folder inside another) counts
/// once, where it is first found.
pub(crate) fn find(user: Option<&Path>, system: &[PathBuf]) -> Vec<Found> {
    let mut found = Vec::new();
    let mut seen = Vec::new();
    if let Some(user) = user {
        look(user, Origin::User, DEPTH, &mut found, &mut seen);
    }
    for place in system {
        look(place, Origin::System, DEPTH, &mut found, &mut seen);
    }
    found.sort_by(|a: &Found, b: &Found| {
        a.origin
            .cmp(&b.origin)
            .then(b.size.cmp(&a.size))
            .then(a.path.cmp(&b.path))
    });
    found
}

fn look(
    place: &Path,
    origin: Origin,
    depth: usize,
    found: &mut Vec<Found>,
    seen: &mut Vec<PathBuf>,
) {
    let Ok(real) = std::fs::canonicalize(place) else {
        return;
    };
    if seen.contains(&real) {
        return;
    }
    seen.push(real.clone());
    let Ok(meta) = std::fs::metadata(&real) else {
        return;
    };
    if meta.is_file() {
        // The kind goes by the name the set was found under: a link named
        // .sf2 to a file without an extension is still a SoundFont.
        if let Some(kind) = Kind::of(place).or_else(|| Kind::of(&real)) {
            found.push(Found {
                path: real,
                size: meta.len(),
                origin,
                kind,
            });
        }
        return;
    }
    if !meta.is_dir() || depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&real) else {
        return;
    };
    let mut names: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    names.sort();
    for name in names {
        look(&name, origin, depth - 1, found, seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oxiguitar-soundfont-sets-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file(path: &Path, size: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; size]).unwrap();
    }

    #[test]
    fn the_users_sets_come_first_then_the_larger() {
        let dir = scratch("order");
        let (user, system) = (dir.join("user"), dir.join("system"));
        file(&user.join("small.sf2"), 10);
        file(&user.join("Big Set/big.SF3"), 30);
        file(&user.join("notes.txt"), 99);
        file(&system.join("huge.sf2"), 1000);
        file(&system.join("gm.dls"), 20);
        let found = find(Some(&user), std::slice::from_ref(&system));
        let names: Vec<_> = found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["big.SF3", "small.sf2", "huge.sf2", "gm.dls"]);
        assert_eq!(found[0].kind, Kind::Sf3);
        assert_eq!(found[0].origin, Origin::User);
        assert_eq!(found[3].kind, Kind::Dls);
        assert_eq!(found[3].origin, Origin::System);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_set_reached_twice_counts_once_and_missing_places_are_skipped() {
        let dir = scratch("twice");
        let system = dir.join("system");
        file(&system.join("FluidR3_GM.sf2"), 100);
        let found = find(
            None,
            &[
                system.clone(),
                system.join("FluidR3_GM.sf2"),
                dir.join("nowhere"),
            ],
        );
        assert_eq!(found.len(), 1);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                system.join("FluidR3_GM.sf2"),
                system.join("default-GM.sf2"),
            )
            .unwrap();
            let found = find(None, std::slice::from_ref(&system));
            assert_eq!(found.len(), 1, "{found:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn subfolders_are_looked_into_only_so_deep() {
        let dir = scratch("deep");
        file(&dir.join("a/b/found.sf2"), 1);
        file(&dir.join("a/b/c/lost.sf2"), 1);
        let found = find(Some(&dir), &[]);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("found.sf2"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
