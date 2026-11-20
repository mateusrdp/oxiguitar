// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The core's tools as plugins see and run them (interface 1.14,
//! `HostRequest::Tools` and `RunTool`): the Tools menu's entries, each with
//! the JSON Schema of the options that run it as told, without its dialog.
//! No window code here, so it is unit-tested; main.rs runs what [`parse`]
//! gives on the window's thread, as the menu would.

use oxiguitar_edit::{Fit, FitHow, Harmonize, Scope, Transposition};
use oxiguitar_host::api::message::{CORE_TOOLS, ToolEntry};
use serde_json::{Map, Value, json};

/// The ids of the core's tools beside `oxiguitar_tools::all()`'s.
const TRANSPOSE: &str = "transpose";
const FINGERING: &str = "find-fingering";
const FIT: &str = "fit-bars";
const HARMONIZE: &str = "harmonize";

/// A core tool to run, as a plugin asked. `None` inside: as from the menu,
/// with its dialog for the user.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CoreRun {
    /// One of `oxiguitar_tools::all()`, by its place there.
    Tool(usize),
    /// Transpose…; without options its dialog for every track.
    Transpose(Option<Transposition>),
    /// Find Fingering, which has no dialog.
    Fingering(Scope),
    /// Fit Bars to Time Signature…; without options its dialog for the
    /// whole song.
    Fit(Option<Fit>),
    /// Harmonize…; without options its dialog for the whole song.
    Harmonize(Option<Harmonize>),
}

impl CoreRun {
    /// What the tool is called in the Tools menu.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            CoreRun::Tool(i) => oxiguitar_tools::all().get(*i).map_or("Tool", |t| t.name()),
            CoreRun::Transpose(_) => "Transpose",
            CoreRun::Fingering(_) => "Find Fingering",
            CoreRun::Fit(_) => "Fit Bars to Time Signature",
            CoreRun::Harmonize(_) => "Harmonize",
        }
    }
}

fn scope_schema(what: &str) -> Value {
    json!({
        "type": "string",
        "enum": ["selection", "track", "all"],
        "default": "all",
        "description": format!("{what}: the selection (the caret's beat when nothing is selected), the caret's track, or every track")
    })
}

/// The core's tools, as `HostRequest::Tools` lists them.
pub(crate) fn entries() -> Vec<ToolEntry> {
    let entry = |id: &str, name: &str, description: &str, options: Value| ToolEntry {
        owner: CORE_TOOLS.to_owned(),
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        edits: true,
        options: Some(options.to_string()),
    };
    let mut out: Vec<ToolEntry> = oxiguitar_tools::all()
        .iter()
        .map(|t| {
            entry(
                t.id(),
                t.name(),
                t.description(),
                json!({"type": "object", "properties": {}, "additionalProperties": false}),
            )
        })
        .collect();
    out.push(entry(
        TRANSPOSE,
        "Transpose…",
        "Moves notes up or down by semitones or by steps of the key.",
        json!({
            "type": "object",
            "properties": {
                "amount": {"type": "integer", "minimum": -48, "maximum": 48, "not": {"const": 0},
                    "description": "How far: semitones, or steps of the key with diatonic"},
                "diatonic": {"type": "boolean", "default": false,
                    "description": "Move by steps of the key rather than semitones"},
                "keep_fingering": {"type": "boolean", "default": false,
                    "description": "Keep each note on its string where it can be played there"},
                "scope": scope_schema("What to transpose")
            },
            "required": ["amount"],
            "additionalProperties": false
        }),
    ));
    out.push(entry(
        FINGERING,
        "Find Fingering",
        "Puts notes on the strings and frets that are easiest to play.",
        json!({
            "type": "object",
            "properties": {"scope": scope_schema("What to finger")},
            "additionalProperties": false
        }),
    ));
    out.push(entry(
        FIT,
        "Fit Bars to Time Signature…",
        "Makes bars that don't add up fit their time signature.",
        json!({
            "type": "object",
            "properties": {
                "how": {"type": "string", "enum": ["stretch", "rebar"], "default": "rebar",
                    "description": "stretch: scale each bar's durations to fit; rebar: move the notes across bar lines"},
                "whole_song": {"type": "boolean", "default": true,
                    "description": "Every bar, or only the selected bars (the caret's bar when nothing is selected)"}
            },
            "additionalProperties": false
        }),
    ));
    out.push(entry(
        HARMONIZE,
        "Harmonize…",
        "Finds chord names for the notes and puts them above the bars.",
        json!({
            "type": "object",
            "properties": {
                "sevenths": {"type": "boolean", "default": false, "description": "Allow seventh chords"},
                "inside_bars": {"type": "boolean", "default": false, "description": "Allow chord changes inside a bar"},
                "keep": {"type": "boolean", "default": false, "description": "Keep the chords already there"},
                "all_tracks": {"type": "boolean", "default": true, "description": "Read every track's notes, not only the caret's track"},
                "whole_song": {"type": "boolean", "default": true, "description": "Every bar, or only the selected bars"}
            },
            "additionalProperties": false
        }),
    ));
    out
}

/// The core tool `id` to run with `options` (JSON text), or why it can't.
pub(crate) fn parse(id: &str, options: Option<&str>) -> Result<CoreRun, String> {
    let options = match options {
        None => None,
        Some(text) => match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => Some(map),
            _ => return Err(format!("the options of {id:?} must be a JSON object")),
        },
    };
    if let Some(i) = oxiguitar_tools::all().iter().position(|t| t.id() == id) {
        if let Some(map) = &options {
            known(map, &[])?;
        }
        return Ok(CoreRun::Tool(i));
    }
    match id {
        TRANSPOSE => options
            .map(|map| {
                known(&map, &["amount", "diatonic", "keep_fingering", "scope"])?;
                let amount = map
                    .get("amount")
                    .and_then(Value::as_i64)
                    .ok_or("Transpose needs an amount, a whole number")?;
                let amount = i8::try_from(amount)
                    .ok()
                    .filter(|a| (-48..=48).contains(a) && *a != 0)
                    .ok_or("Transpose's amount is -48 to 48, and not 0")?;
                Ok(Transposition {
                    amount,
                    diatonic: flag(&map, "diatonic", false)?,
                    keep_fingering: flag(&map, "keep_fingering", false)?,
                    scope: scope(&map)?,
                })
            })
            .transpose()
            .map(CoreRun::Transpose),
        FINGERING => {
            let scope = match &options {
                None => Scope::All,
                Some(map) => {
                    known(map, &["scope"])?;
                    scope(map)?
                }
            };
            Ok(CoreRun::Fingering(scope))
        }
        FIT => options
            .map(|map| {
                known(&map, &["how", "whole_song"])?;
                let how = match map.get("how").map(|v| v.as_str()) {
                    None | Some(Some("rebar")) => FitHow::Rebar,
                    Some(Some("stretch")) => FitHow::Stretch,
                    _ => return Err("Fit Bars' how is \"stretch\" or \"rebar\"".to_owned()),
                };
                Ok(Fit {
                    how,
                    whole_song: flag(&map, "whole_song", true)?,
                })
            })
            .transpose()
            .map(CoreRun::Fit),
        HARMONIZE => options
            .map(|map| {
                known(
                    &map,
                    &[
                        "sevenths",
                        "inside_bars",
                        "keep",
                        "all_tracks",
                        "whole_song",
                    ],
                )?;
                Ok(Harmonize {
                    sevenths: flag(&map, "sevenths", false)?,
                    inside_bars: flag(&map, "inside_bars", false)?,
                    keep: flag(&map, "keep", false)?,
                    all_tracks: flag(&map, "all_tracks", true)?,
                    whole_song: flag(&map, "whole_song", true)?,
                })
            })
            .transpose()
            .map(CoreRun::Harmonize),
        _ => Err(format!("OxiGuitar has no tool {id:?}")),
    }
}

/// An error naming the first key of `map` that isn't one of `keys`.
fn known(map: &Map<String, Value>, keys: &[&str]) -> Result<(), String> {
    match map.keys().find(|k| !keys.contains(&k.as_str())) {
        Some(k) => Err(format!("unknown option {k:?}")),
        None => Ok(()),
    }
}

fn flag(map: &Map<String, Value>, key: &str, default: bool) -> Result<bool, String> {
    match map.get(key) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{key:?} is true or false")),
    }
}

fn scope(map: &Map<String, Value>) -> Result<Scope, String> {
    match map.get("scope").map(Value::as_str) {
        None | Some(Some("all")) => Ok(Scope::All),
        Some(Some("track")) => Ok(Scope::Track),
        Some(Some("selection")) => Ok(Scope::Selection),
        _ => Err("scope is \"selection\", \"track\" or \"all\"".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_has_an_object_schema_and_parses_without_options() {
        let entries = entries();
        assert!(entries.len() >= 5);
        for e in &entries {
            assert_eq!(e.owner, CORE_TOOLS);
            let schema: Value = serde_json::from_str(e.options.as_deref().unwrap()).unwrap();
            assert_eq!(schema["type"], "object", "{}", e.id);
            assert!(parse(&e.id, None).is_ok(), "{}", e.id);
        }
    }

    #[test]
    fn options_run_a_tool_as_told() {
        assert_eq!(
            parse(TRANSPOSE, Some(r#"{"amount": -2, "scope": "track"}"#)).unwrap(),
            CoreRun::Transpose(Some(Transposition {
                amount: -2,
                diatonic: false,
                keep_fingering: false,
                scope: Scope::Track,
            }))
        );
        assert_eq!(parse(TRANSPOSE, None).unwrap(), CoreRun::Transpose(None));
        assert_eq!(
            parse(FIT, Some(r#"{"how": "stretch", "whole_song": false}"#)).unwrap(),
            CoreRun::Fit(Some(Fit {
                how: FitHow::Stretch,
                whole_song: false
            }))
        );
        assert_eq!(
            parse(FINGERING, Some(r#"{"scope": "selection"}"#)).unwrap(),
            CoreRun::Fingering(Scope::Selection)
        );
        let h = parse(HARMONIZE, Some(r#"{"sevenths": true}"#)).unwrap();
        assert!(matches!(h, CoreRun::Harmonize(Some(h)) if h.sevenths && h.all_tracks));
        assert_eq!(
            parse("infer-key-signatures", Some("{}")).unwrap(),
            CoreRun::Tool(0)
        );
    }

    #[test]
    fn wrong_options_say_why() {
        for (id, options) in [
            (TRANSPOSE, r#"{"amount": 0}"#),
            (TRANSPOSE, r#"{"amount": 99}"#),
            (TRANSPOSE, r#"{}"#),
            (TRANSPOSE, r#"{"amount": 1, "octave": true}"#),
            (FIT, r#"{"how": "squash"}"#),
            (FINGERING, r#"{"scope": "song"}"#),
            (HARMONIZE, r#"{"keep": "yes"}"#),
            (TRANSPOSE, "[1]"),
            ("infer-key-signatures", r#"{"x": 1}"#),
        ] {
            assert!(parse(id, Some(options)).is_err(), "{id} {options}");
        }
        assert!(parse("make-coffee", None).is_err());
    }
}
