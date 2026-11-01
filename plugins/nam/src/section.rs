// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track dialog's section "Amp (NAM)", for guitars and basses: NAM on
//! or off, the model, the cabinet, and the levels before and after.

use oxiguitar_plugin::serde_json::{Map, Value};
use oxiguitar_plugin::{Error, Host, Row, TabContent, TabEvent, TrackInfo, TrackSection, Widget};

use crate::files::{self, Found};
use crate::settings::{Cabinet, LEVELS, Track};

/// The section.
#[derive(Debug)]
pub(crate) struct Section {
    host: Host,
}

impl Section {
    pub(crate) fn new(host: Host) -> Section {
        Section { host }
    }

    fn found(&self) -> (Option<std::path::PathBuf>, Found) {
        let folder = crate::folder(&self.host);
        let found = folder.as_deref().map(files::find).unwrap_or_default();
        (folder, found)
    }
}

/// The model choice's lines: "None", the model the track names when it
/// isn't there any more, then every model found; the line picked; and
/// where the found models start.
fn models(found: &Found, track: &Track) -> (Vec<String>, u32, usize) {
    let mut options = vec!["None".to_owned()];
    let picked = match &track.model {
        None => 0,
        Some(m) => match found.models.iter().position(|o| o == m) {
            Some(i) => i + 1,
            None => {
                options.push(format!("{m} (not in the nam folder)"));
                1
            }
        },
    };
    let start = options.len();
    options.extend(found.models.iter().cloned());
    (options, picked as u32, start)
}

/// The cabinet choice's lines and what each picks.
fn cabinets(found: &Found) -> Vec<(String, Cabinet)> {
    let mut out = vec![
        ("From the model".to_owned(), Cabinet::Auto),
        ("None".to_owned(), Cabinet::None),
    ];
    for (name, _) in crate::BUILT_IN {
        out.push((
            format!("{} (built in)", crate::built_in_title(name)),
            Cabinet::BuiltIn(name.into()),
        ));
    }
    for r in &found.responses {
        out.push((r.clone(), Cabinet::File(r.clone())));
    }
    out
}

fn label(text: impl Into<String>, muted: bool) -> Widget {
    Widget::Label {
        text: text.into(),
        muted,
    }
}

fn level_row(title: &str, id: &str, value: f32) -> Row {
    Row {
        widgets: vec![
            label(title, false),
            Widget::Slider {
                id: id.into(),
                value,
                min: LEVELS.0,
                max: LEVELS.1,
                enabled: true,
            },
            label(format!("{value:+.1} dB"), true),
        ],
    }
}

impl TrackSection for Section {
    fn content(&self, _track: &TrackInfo, settings: &Map<String, Value>) -> TabContent {
        let t = Track::from_map(settings);
        let (folder, found) = self.found();
        let (model_options, model_picked, _) = models(&found, &t);
        let cabs = cabinets(&found);
        // A file no longer there (or a built-in one a later version
        // dropped) is shown as picked, at the end.
        let cab_picked = cabs
            .iter()
            .position(|(_, c)| *c == t.cabinet)
            .unwrap_or(cabs.len());
        let mut cab_options: Vec<String> = cabs.into_iter().map(|(n, _)| n).collect();
        if cab_picked == cab_options.len() {
            cab_options.push(match &t.cabinet {
                Cabinet::File(f) | Cabinet::BuiltIn(f) => format!("{f} (not found)"),
                Cabinet::Auto | Cabinet::None => "?".into(),
            });
        }
        let mut rows = vec![
            Row {
                widgets: vec![Widget::Toggle {
                    id: "on".into(),
                    text: "Play this track through an amp model".into(),
                    on: t.on,
                    enabled: true,
                }],
            },
            Row {
                widgets: vec![
                    label("Model", false),
                    Widget::Choice {
                        id: "model".into(),
                        options: model_options,
                        selected: model_picked,
                        enabled: !found.models.is_empty() || t.model.is_some(),
                    },
                ],
            },
            Row {
                widgets: vec![
                    label("Cabinet", false),
                    Widget::Choice {
                        id: "cabinet".into(),
                        options: cab_options,
                        selected: cab_picked as u32,
                        enabled: true,
                    },
                ],
            },
            level_row("Input", "input", t.input),
            level_row("Output", "output", t.output),
        ];
        if found.models.is_empty() {
            rows.push(Row {
                widgets: vec![label("No amp models found yet.", false)],
            });
        }
        rows.push(Row {
            widgets: vec![label(
                "Put .nam models and .wav cabinet responses in:",
                true,
            )],
        });
        rows.push(Row {
            widgets: vec![label(
                folder.map_or_else(|| "the nam folder".to_owned(), |f| f.display().to_string()),
                true,
            )],
        });
        rows.push(Row {
            widgets: vec![Widget::Link {
                text: "Amp models: where to get them, and a guitar sound for them".into(),
                url: crate::MANUAL.into(),
            }],
        });
        TabContent { rows }
    }

    fn on(
        &self,
        _track: &TrackInfo,
        settings: &mut Map<String, Value>,
        widget: &str,
        event: TabEvent,
    ) -> Result<(), Error> {
        let mut t = Track::from_map(settings);
        let (_, found) = self.found();
        match (widget, event) {
            ("on", TabEvent::Toggled(on)) => t.on = on,
            ("model", TabEvent::Chosen(i)) => {
                let (_, _, start) = models(&found, &t);
                let i = i as usize;
                if i == 0 {
                    t.model = None;
                } else if let Some(m) = i.checked_sub(start).and_then(|k| found.models.get(k)) {
                    t.model = Some(m.clone());
                    // Picking a model is wanting to hear it.
                    t.on = true;
                } else {
                    // The missing file's line: left as it is.
                    return Ok(());
                }
            }
            ("cabinet", TabEvent::Chosen(i)) => {
                if let Some((_, c)) = cabinets(&found).into_iter().nth(i as usize) {
                    t.cabinet = c;
                }
            }
            ("input", TabEvent::Slid(v)) => t.input = (v * 2.0).round() / 2.0,
            ("output", TabEvent::Slid(v)) => t.output = (v * 2.0).round() / 2.0,
            _ => return Ok(()),
        }
        t.input = t.input.clamp(LEVELS.0, LEVELS.1);
        t.output = t.output.clamp(LEVELS.0, LEVELS.1);
        t.write(settings);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_model_stays_shown_and_none_comes_first() {
        let found = Found {
            models: vec!["a.nam".into(), "b.nam".into()],
            responses: vec!["4x12.wav".into()],
        };
        let none = models(&found, &Track::default());
        assert_eq!(
            none,
            (vec!["None".into(), "a.nam".into(), "b.nam".into()], 0, 1)
        );
        let b = Track {
            model: Some("b.nam".into()),
            ..Track::default()
        };
        assert_eq!(models(&found, &b).1, 2);
        let gone = Track {
            model: Some("gone.nam".into()),
            ..Track::default()
        };
        let (lines, picked, start) = models(&found, &gone);
        assert_eq!(lines[1], "gone.nam (not in the nam folder)");
        assert_eq!((picked, start), (1, 2));
        let cabs = cabinets(&found);
        assert_eq!(cabs.len(), 2 + crate::BUILT_IN.len() + 1);
        assert_eq!(cabs.last().unwrap().1, Cabinet::File("4x12.wav".into()));
    }
}
