// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What plugins keep per track (their sections of the track dialog,
//! interface 1.3), saved beside the score: `FILE.oxg`, the score's whole
//! file name plus `.oxg` (`song.gp3.oxg`), since no score format holds it
//! (Mateus, 25 Oct 2026).
//!
//! The file is JSON:
//!
//! ```json
//! {
//!   "oxiguitar": 1,
//!   "tracks": [
//!     { "name": "Lead Guitar", "plugins": { "oxiguitar.nam": { "model": "5150.nam" } } }
//!   ]
//! }
//! ```
//!
//! one entry per track in the score's order, each with the track's name and
//! the settings of every plugin that keeps some, by plugin id. When the
//! score was changed elsewhere (tracks added, removed or moved in another
//! program), a track takes the entry at its own place if the names agree,
//! else the first unused entry of its name, else none.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The extension added to the score's file name.
pub const EXTENSION: &str = "oxg";

/// The version this OxiGuitar writes and reads.
const VERSION: u64 = 1;

/// The sidecar of the score at `score`: its file name plus `.oxg`.
pub fn path_for(score: &Path) -> PathBuf {
    let mut name = score.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(EXTENSION);
    score.with_file_name(name)
}

/// One track's entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackEntry {
    /// The track's name when it was saved.
    pub name: String,
    /// Each plugin's settings for the track: a JSON object as text, by
    /// plugin id.
    pub plugins: BTreeMap<String, String>,
}

/// What a sidecar holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sidecar {
    /// The tracks, in the score's order.
    pub tracks: Vec<TrackEntry>,
}

impl Sidecar {
    /// Whether no track keeps anything.
    pub fn is_empty(&self) -> bool {
        self.tracks.iter().all(|t| t.plugins.is_empty())
    }

    /// Reads the sidecar of the score at `score`: `Ok(None)` when it has
    /// none. The error is a sentence for the footer.
    pub fn read_for(score: &Path) -> Result<Option<Sidecar>, String> {
        let path = path_for(score);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Cannot read {name}: {e}.")),
        };
        Sidecar::parse(&text).map(Some).map_err(|why| {
            format!("{name} can't be used ({why}); the tracks' plugin settings start empty.")
        })
    }

    /// Reads a sidecar's text.
    pub fn parse(text: &str) -> Result<Sidecar, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let Value::Object(top) = value else {
            return Err("not a JSON object".into());
        };
        match top.get("oxiguitar").and_then(Value::as_u64) {
            Some(VERSION) => {}
            Some(v) => return Err(format!("version {v}, written by a newer OxiGuitar")),
            None => return Err("no \"oxiguitar\" version".into()),
        }
        let tracks = match top.get("tracks") {
            Some(Value::Array(tracks)) => tracks,
            None => return Ok(Sidecar::default()),
            Some(_) => return Err("\"tracks\" is not a list".into()),
        };
        let tracks = tracks
            .iter()
            .map(|t| {
                let name = t.get("name").and_then(Value::as_str).unwrap_or_default();
                let plugins = match t.get("plugins") {
                    Some(Value::Object(p)) => p
                        .iter()
                        .filter(|(_, v)| v.is_object())
                        .map(|(id, v)| (id.clone(), v.to_string()))
                        .collect(),
                    _ => BTreeMap::new(),
                };
                TrackEntry {
                    name: name.to_owned(),
                    plugins,
                }
            })
            .collect();
        Ok(Sidecar { tracks })
    }

    /// The sidecar's text.
    pub fn to_text(&self) -> String {
        let tracks: Vec<Value> = self
            .tracks
            .iter()
            .map(|t| {
                let plugins: Map<String, Value> = t
                    .plugins
                    .iter()
                    .filter_map(|(id, json)| {
                        serde_json::from_str::<Value>(json)
                            .ok()
                            .filter(Value::is_object)
                            .map(|v| (id.clone(), v))
                    })
                    .collect();
                serde_json::json!({ "name": t.name, "plugins": plugins })
            })
            .collect();
        let doc = serde_json::json!({ "oxiguitar": VERSION, "tracks": tracks });
        let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Writes the sidecar of the score at `score`, beside it first and
    /// then in place, so a failure never leaves half a file. Nothing is
    /// written when it is empty and there is no sidecar yet; an existing
    /// one is emptied rather than deleted. The error is a sentence.
    pub fn write_for(&self, score: &Path) -> Result<(), String> {
        let path = path_for(score);
        if self.is_empty() && !path.exists() {
            return Ok(());
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let temp = path.with_file_name(format!(".{name}.oxiguitar-saving"));
        std::fs::write(&temp, self.to_text())
            .and_then(|()| std::fs::rename(&temp, &path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("Could not save {name}, which holds the tracks' plugin settings: {e}.")
            })
    }

    /// Each of the tracks named `names` (in the score's order) and its
    /// entry: at its own place if the names agree, else the first unused
    /// entry with its name.
    pub fn settle(&self, names: &[&str]) -> Vec<BTreeMap<String, String>> {
        let mut used = vec![false; self.tracks.len()];
        let mut out = vec![None; names.len()];
        for (i, name) in names.iter().enumerate() {
            if let Some(entry) = self.tracks.get(i)
                && entry.name == *name
            {
                used[i] = true;
                out[i] = Some(entry.plugins.clone());
            }
        }
        for (i, name) in names.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            if let Some(j) =
                (0..self.tracks.len()).find(|&j| !used[j] && self.tracks[j].name == *name)
            {
                used[j] = true;
                out[i] = Some(self.tracks[j].plugins.clone());
            }
        }
        out.into_iter().map(Option::unwrap_or_default).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, plugins: &[(&str, &str)]) -> TrackEntry {
        TrackEntry {
            name: name.into(),
            plugins: plugins
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn the_sidecar_is_the_file_name_plus_oxg() {
        assert_eq!(
            path_for(Path::new("/music/song.gp3")),
            PathBuf::from("/music/song.gp3.oxg")
        );
        assert_eq!(path_for(Path::new("song")), PathBuf::from("song.oxg"));
    }

    #[test]
    fn it_round_trips_and_refuses_what_it_can_t_read() {
        let s = Sidecar {
            tracks: vec![
                entry(
                    "Lead",
                    &[("oxiguitar.nam", r#"{"level":0.5,"model":"5150.nam"}"#)],
                ),
                entry("Bass", &[]),
            ],
        };
        let text = s.to_text();
        assert!(text.contains("\"oxiguitar\": 1"), "{text}");
        assert_eq!(Sidecar::parse(&text).unwrap(), s);
        assert!(Sidecar::parse("[]").is_err());
        assert!(Sidecar::parse(r#"{"tracks": []}"#).is_err());
        assert!(
            Sidecar::parse(r#"{"oxiguitar": 2}"#)
                .unwrap_err()
                .contains("newer OxiGuitar")
        );
        // What isn't an object is left out, the rest kept.
        let odd = r#"{"oxiguitar": 1, "tracks": [{"name": "A", "plugins": {"x": 3, "y": {}}}]}"#;
        assert_eq!(
            Sidecar::parse(odd).unwrap().tracks[0],
            entry("A", &[("y", "{}")])
        );
    }

    #[test]
    fn tracks_find_their_entries_by_place_then_by_name() {
        let s = Sidecar {
            tracks: vec![
                entry("Lead", &[("p", "{\"a\":1}")]),
                entry("Rhythm", &[("p", "{\"a\":2}")]),
                entry("Rhythm", &[("p", "{\"a\":3}")]),
            ],
        };
        let got = s.settle(&["Rhythm", "Lead", "Rhythm", "Rhythm", "Keys"]);
        let a = |i: usize| got[i].get("p").cloned().unwrap_or_default();
        // Track 2 sits where its name was; the others are found by name.
        assert_eq!(a(0), "{\"a\":2}");
        assert_eq!(a(1), "{\"a\":1}");
        assert_eq!(a(2), "{\"a\":3}");
        assert_eq!(a(3), "");
        assert_eq!(a(4), "");
    }

    #[test]
    fn writing_keeps_quiet_until_there_is_something_to_keep() {
        let dir = crate::folders::tests::scratch("sidecar");
        let score = dir.join("song.gp");
        let empty = Sidecar {
            tracks: vec![entry("Lead", &[])],
        };
        empty.write_for(&score).unwrap();
        assert!(!path_for(&score).exists());
        assert_eq!(Sidecar::read_for(&score), Ok(None));
        let kept = Sidecar {
            tracks: vec![entry("Lead", &[("p", "{}")])],
        };
        kept.write_for(&score).unwrap();
        assert_eq!(Sidecar::read_for(&score), Ok(Some(kept)));
        // Emptied, not deleted.
        empty.write_for(&score).unwrap();
        assert_eq!(Sidecar::read_for(&score), Ok(Some(empty)));
        std::fs::write(path_for(&score), "not json").unwrap();
        assert!(
            Sidecar::read_for(&score)
                .unwrap_err()
                .contains("song.gp.oxg can't be used")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
