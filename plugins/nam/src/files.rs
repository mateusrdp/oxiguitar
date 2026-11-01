// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The `nam` folder (beside `plugins` and `soundfonts`): amp models
//! (`.nam`) and cabinet impulse responses (`.wav`) the user puts there, in
//! folders inside it too, five levels deep. Never written to. Tracks name
//! a file by its path inside the folder, with `/` between folders, so a
//! score's settings work on another computer with the same files.

use std::path::{Component, Path, PathBuf};

/// How deep inside the folder files are looked for.
const DEPTH: usize = 5;

/// What the folder holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Found {
    /// The models' paths inside the folder, sorted.
    pub(crate) models: Vec<String>,
    /// The impulse responses' paths inside the folder, sorted.
    pub(crate) responses: Vec<String>,
}

/// The models and impulse responses in `folder`.
pub(crate) fn find(folder: &Path) -> Found {
    let mut found = Found::default();
    walk(folder, folder, 0, &mut found);
    found.models.sort_by_key(|p| p.to_lowercase());
    found.responses.sort_by_key(|p| p.to_lowercase());
    found
}

fn walk(root: &Path, dir: &Path, depth: usize, found: &mut Found) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() || (kind.is_symlink() && path.is_dir()) {
            if depth + 1 < DEPTH {
                walk(root, &path, depth + 1, found);
            }
            continue;
        }
        let Some(name) = inside(root, &path) else {
            continue;
        };
        let lower = name.to_lowercase();
        if lower.ends_with(".nam") {
            found.models.push(name);
        } else if lower.ends_with(".wav") {
            found.responses.push(name);
        }
    }
}

/// `path`'s path inside `root`, with `/` between folders.
fn inside(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rest.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

/// The file a track names, inside `folder`: refused if the name would
/// leave the folder (a score's settings are untrusted).
pub(crate) fn resolve(folder: &Path, name: &str) -> Option<PathBuf> {
    let rel = Path::new(name);
    if name.is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(folder.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_and_responses_are_found_inside_folders() {
        let dir = std::env::temp_dir().join(format!("oxiguitar-nam-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Amps/Clean")).unwrap();
        std::fs::write(dir.join("Amps/Clean/Twin.nam"), "{}").unwrap();
        std::fs::write(dir.join("lead.NAM"), "{}").unwrap();
        std::fs::write(dir.join("4x12.wav"), "").unwrap();
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        let f = find(&dir);
        assert_eq!(f.models, ["Amps/Clean/Twin.nam", "lead.NAM"]);
        assert_eq!(f.responses, ["4x12.wav"]);
        assert_eq!(
            resolve(&dir, "Amps/Clean/Twin.nam"),
            Some(dir.join("Amps/Clean/Twin.nam"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_leaving_the_folder_are_refused() {
        let root = Path::new("/home/u/nam");
        for bad in ["", "../x.nam", "/etc/x.nam", "a/../../x.nam", "./x.nam"] {
            assert_eq!(resolve(root, bad), None, "{bad}");
        }
    }
}
