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
//!     { "name": "Lead Guitar", "plugins": { "oxiguitar.nam": { "model": "5150.nam" } },
//!       "effects": ["oxiguitar.eq", "oxiguitar.nam"], "chords": true }
//!   ]
//! }
//! ```
//!
//! one entry per track in the score's order, each with the track's name,
//! the settings of every plugin that keeps some, by plugin id, and, when
//! the user ordered them in the track dialog, the order its effects run in
//! by plugin id (#152; absent for the default order, and ignored by an
//! older OxiGuitar), and `"chords": true` when its chord diagrams are
//! played (#151). When the
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
    /// The order of the track's effects, by plugin id: empty for the
    /// default order.
    pub effects: Vec<String>,
    /// Whether the track's chord diagrams are played.
    pub chords: bool,
}

/// What a sidecar holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sidecar {
    /// The tracks, in the score's order.
    pub tracks: Vec<TrackEntry>,
    /// One entry per bar: `None` for no custom colour, or an RGB object.
    pub bar_colors: Vec<Option<serde_json::Value>>,
    /// Track name → one entry per bar. Same structure as `bar_colors`.
    pub track_bar_colors: BTreeMap<String, Vec<Option<serde_json::Value>>>,
}

impl Sidecar {
    /// Whether no track keeps anything and no bar has a custom colour.
    pub fn is_empty(&self) -> bool {
        self.tracks
            .iter()
            .all(|t| t.plugins.is_empty() && t.effects.is_empty() && !t.chords)
            && self.bar_colors.iter().all(Option::is_none)
            && self
                .track_bar_colors
                .values()
                .all(|v| v.iter().all(Option::is_none))
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

    /// Gives each track's settings kept under a replaced plugin's id to the
    /// plugin that replaces it (interface 1.4; `replaced` as
    /// [`crate::Plugins::replaced_ids`] lists them), where the new id has
    /// none yet, and its place in the order of the track's effects. The old
    /// entries stay, for an older OxiGuitar; the next save writes both.
    pub fn adopt(&mut self, replaced: &[(String, String)]) {
        for track in &mut self.tracks {
            for (old, new) in replaced {
                if !track.effects.contains(new)
                    && let Some(at) = track.effects.iter().position(|id| id == old)
                {
                    track.effects.insert(at, new.clone());
                }
                if !track.plugins.contains_key(new)
                    && let Some(value) = track.plugins.get(old).cloned()
                {
                    track.plugins.insert(new.clone(), value);
                }
            }
        }
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
                let mut effects: Vec<String> = Vec::new();
                if let Some(Value::Array(ids)) = t.get("effects") {
                    for id in ids.iter().filter_map(Value::as_str) {
                        if !effects.iter().any(|e| e == id) {
                            effects.push(id.to_owned());
                        }
                    }
                }
                TrackEntry {
                    name: name.to_owned(),
                    plugins,
                    effects,
                    chords: t.get("chords").and_then(Value::as_bool) == Some(true),
                }
            })
            .collect();
        let mut sidecar = Sidecar {
            tracks,
            ..Default::default()
        };
        if let Some(Value::Array(arr)) = top.get("bar_colors") {
            sidecar.bar_colors = arr
                .iter()
                .map(|v| if v.is_null() { None } else { Some(v.clone()) })
                .collect();
        }
        if let Some(Value::Object(obj)) = top.get("track_bar_colors") {
            sidecar.track_bar_colors = obj
                .iter()
                .map(|(k, v)| {
                    let arr: Vec<&Value> = v.as_array().into_iter().flatten().collect();
                    (
                        k.clone(),
                        arr.iter()
                            .map(|item| {
                                let item: &Value = *item;
                                if item.is_null() {
                                    None
                                } else {
                                    Some(item.clone())
                                }
                            })
                            .collect(),
                    )
                })
                .collect();
        }
        Ok(sidecar)
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
                let mut entry = serde_json::json!({ "name": t.name, "plugins": plugins });
                if !t.effects.is_empty() {
                    entry["effects"] = serde_json::json!(t.effects);
                }
                if t.chords {
                    entry["chords"] = serde_json::json!(true);
                }
                entry
            })
            .collect();
        let mut doc = serde_json::json!({ "oxiguitar": VERSION, "tracks": tracks });
        if !self.bar_colors.iter().all(Option::is_none) {
            let arr: Vec<Value> = self
                .bar_colors
                .iter()
                .map(|c| c.clone().unwrap_or(Value::Null))
                .collect();
            doc["bar_colors"] = serde_json::json!(arr);
        }
        if !self.track_bar_colors.is_empty() {
            let obj: Map<String, Value> = self
                .track_bar_colors
                .iter()
                .map(|(name, colors)| {
                    let arr: Vec<Value> = colors
                        .iter()
                        .map(|c| c.clone().unwrap_or(Value::Null))
                        .collect();
                    (name.clone(), serde_json::json!(arr))
                })
                .collect();
            doc["track_bar_colors"] = Value::Object(obj);
        }
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
    /// entry (empty when it has none): at its own place if the names agree,
    /// else the first unused entry with its name.
    pub fn settle(&self, names: &[&str]) -> Vec<TrackEntry> {
        let mut used = vec![false; self.tracks.len()];
        let mut out = vec![None; names.len()];
        for (i, name) in names.iter().enumerate() {
            if let Some(entry) = self.tracks.get(i)
                && entry.name == *name
            {
                used[i] = true;
                out[i] = Some(entry.clone());
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
                out[i] = Some(self.tracks[j].clone());
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
            effects: Vec::new(),
            chords: false,
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
            ..Default::default()
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
            ..Default::default()
        };
        let got = s.settle(&["Rhythm", "Lead", "Rhythm", "Rhythm", "Keys"]);
        let a = |i: usize| got[i].plugins.get("p").cloned().unwrap_or_default();
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
            ..Default::default()
        };
        empty.write_for(&score).unwrap();
        assert!(!path_for(&score).exists());
        assert_eq!(Sidecar::read_for(&score), Ok(None));
        let kept = Sidecar {
            tracks: vec![entry("Lead", &[("p", "{}")])],
            ..Default::default()
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

    #[test]
    fn a_replaced_plugin_s_track_settings_are_adopted_by_its_successor() {
        let mut sidecar = Sidecar::parse(
            r#"{"oxiguitar": 1, "tracks": [
                {"name": "Lead", "plugins": {"oxiguitar.soundfont": {"set": "a"}}},
                {"name": "Bass", "plugins": {"oxiguitar.soundfont": {"set": "b"},
                                             "oxiguitar.midi": {"set": "c"}}}]}"#,
        )
        .unwrap();
        sidecar.adopt(&[("oxiguitar.soundfont".into(), "oxiguitar.midi".into())]);
        let get = |t: usize, id: &str| sidecar.tracks[t].plugins.get(id).cloned();
        assert_eq!(get(0, "oxiguitar.midi").as_deref(), Some(r#"{"set":"a"}"#));
        assert_eq!(
            get(0, "oxiguitar.soundfont").as_deref(),
            Some(r#"{"set":"a"}"#)
        );
        // The successor's own settings win.
        assert_eq!(get(1, "oxiguitar.midi").as_deref(), Some(r#"{"set":"c"}"#));
    }

    #[test]
    fn the_order_of_a_track_s_effects_is_kept_when_set() {
        let mut ordered = entry("Lead", &[("oxiguitar.nam", "{}")]);
        ordered.effects = vec!["oxiguitar.eq".into(), "oxiguitar.nam".into()];
        let s = Sidecar {
            tracks: vec![ordered.clone(), entry("Bass", &[])],
            ..Default::default()
        };
        let text = s.to_text();
        assert_eq!(text.matches("\"effects\"").count(), 1, "{text}");
        assert_eq!(Sidecar::parse(&text).unwrap(), s);
        assert_eq!(s.settle(&["Lead"])[0], ordered);
        // An order alone is worth keeping.
        let only = Sidecar {
            tracks: vec![TrackEntry {
                plugins: BTreeMap::new(),
                ..ordered
            }],
            ..Default::default()
        };
        assert!(!only.is_empty());
        // What isn't a plugin id is left out, and so is a second mention.
        let odd = r#"{"oxiguitar": 1, "tracks": [{"name": "A", "effects": ["b", 3, "a", "b"]}]}"#;
        assert_eq!(Sidecar::parse(odd).unwrap().tracks[0].effects, ["b", "a"]);
    }

    #[test]
    fn a_replaced_plugin_keeps_its_place_in_the_order_of_effects() {
        let mut sidecar = Sidecar::parse(
            r#"{"oxiguitar": 1, "tracks": [
                {"name": "Lead", "effects": ["x.eq", "old.amp"]},
                {"name": "Bass", "effects": ["new.amp", "old.amp"]}]}"#,
        )
        .unwrap();
        sidecar.adopt(&[("old.amp".into(), "new.amp".into())]);
        assert_eq!(sidecar.tracks[0].effects, ["x.eq", "new.amp", "old.amp"]);
        assert_eq!(sidecar.tracks[1].effects, ["new.amp", "old.amp"]);
    }

    #[test]
    fn played_chords_are_kept_only_when_on() {
        let mut on = entry("Rhythm", &[]);
        on.chords = true;
        let s = Sidecar {
            tracks: vec![on.clone(), entry("Lead", &[])],
            ..Default::default()
        };
        let text = s.to_text();
        assert_eq!(text.matches("\"chords\": true").count(), 1, "{text}");
        assert_eq!(Sidecar::parse(&text).unwrap(), s);
        assert!(!s.is_empty());
        assert_eq!(s.settle(&["Rhythm"])[0], on);
        let odd = r#"{"oxiguitar": 1, "tracks": [{"name": "A", "chords": "yes"}]}"#;
        assert!(!Sidecar::parse(odd).unwrap().tracks[0].chords);
    }

    #[test]
    fn bar_colours_round_trip() {
        let s = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![
                None,
                Some(serde_json::json!({"r": 255, "g": 128, "b": 0})),
                None,
            ],
            track_bar_colors: BTreeMap::new(),
        };
        let text = s.to_text();
        assert!(text.contains("\"bar_colors\""), "{text}");
        let parsed = Sidecar::parse(&text).unwrap();
        assert_eq!(parsed, s);
        // All-null should be omitted from output (is_empty-like).
        let empty_colors = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![None, None],
            track_bar_colors: BTreeMap::new(),
        };
        let text2 = empty_colors.to_text();
        assert!(!text2.contains("\"bar_colors\""), "{text2}");
    }

    #[test]
    fn track_bar_colours_round_trip() {
        let mut track_colors = BTreeMap::new();
        track_colors.insert(
            "Lead".into(),
            vec![None, Some(serde_json::json!({"r": 0, "g": 255, "b": 128}))],
        );
        let s = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![None, None], // all-null → omitted from output
            track_bar_colors: track_colors,
        };
        let text = s.to_text();
        assert!(text.contains("\"track_bar_colors\""), "{text}");
        let parsed = Sidecar::parse(&text).unwrap();
        // All-null bar_colors are omitted, so parse returns empty vec.
        assert!(parsed.bar_colors.is_empty());
        assert_eq!(parsed.track_bar_colors, s.track_bar_colors);
    }

    #[test]
    fn is_empty_considers_colours() {
        let empty = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![None, None],
            track_bar_colors: BTreeMap::new(),
        };
        assert!(empty.is_empty());
        let with_colour = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![None, Some(serde_json::json!({"r": 1, "g": 2, "b": 3}))],
            track_bar_colors: BTreeMap::new(),
        };
        assert!(!with_colour.is_empty());
        let with_track_colour = Sidecar {
            tracks: vec![entry("Lead", &[])],
            bar_colors: vec![None],
            track_bar_colors: {
                let mut m = BTreeMap::new();
                m.insert(
                    "Lead".into(),
                    vec![None, Some(serde_json::json!({"r": 1, "g": 2, "b": 3}))],
                );
                m
            },
        };
        assert!(!with_track_colour.is_empty());
    }

    #[test]
    fn missing_colour_fields_parse_as_empty() {
        let text = r#"{"oxiguitar": 1, "tracks": [{"name": "A"}]}"#;
        let s = Sidecar::parse(text).unwrap();
        assert!(s.bar_colors.is_empty());
        assert!(s.track_bar_colors.is_empty());
    }
}
