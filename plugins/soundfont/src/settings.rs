// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin's settings window: one set for guitars, one for basses, one
//! for drums and one for everything else, for every file (Mateus, 21 Oct
//! 2026), kept in the plugin's own settings as each set's path. Until the
//! user picks, the best set found plays.

use oxiguitar_plugin::audio::PreparedTrack;
use oxiguitar_plugin::serde_json::{Map, Value};
use oxiguitar_plugin::{Error, Host, InstrumentClass, Row, Tab, TabContent, TabEvent, Widget};

use crate::sets::{self, Found, Origin};

/// The four kinds of track the window picks a set for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Guitars,
    Basses,
    Drums,
    Other,
}

impl Role {
    pub(crate) const ALL: [Role; 4] = [Role::Guitars, Role::Basses, Role::Drums, Role::Other];

    /// Its key in the settings and its widget's id.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Role::Guitars => "guitars",
            Role::Basses => "basses",
            Role::Drums => "drums",
            Role::Other => "other",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Role::Guitars => "Guitars",
            Role::Basses => "Basses",
            Role::Drums => "Drums",
            Role::Other => "Everything else",
        }
    }

    /// The role of a track: drum kits; basses by their General MIDI
    /// program (32 to 39: acoustic, electric, fretless, slap and synth);
    /// guitars and the other fretted instruments; then the rest.
    pub(crate) fn of(track: &PreparedTrack) -> Role {
        if track.percussion {
            Role::Drums
        } else if (32..=39).contains(&track.program) {
            Role::Basses
        } else if track.info.class == InstrumentClass::Fretted {
            Role::Guitars
        } else {
            Role::Other
        }
    }
}

/// The set `settings` picks for `role` among `found`, if it picks one that
/// is still there; else the best.
pub(crate) fn chosen<'a>(
    settings: &Map<String, Value>,
    role: Role,
    found: &'a [Found],
) -> Option<&'a Found> {
    settings
        .get(role.key())
        .and_then(Value::as_str)
        .and_then(|p| {
            found
                .iter()
                .find(|f| f.path.as_os_str() == std::ffi::OsStr::new(p))
        })
        .or_else(|| found.first())
}

/// How the window names a set.
fn label(f: &Found) -> String {
    let name = f.path.file_name().map_or_else(
        || f.path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let whose = match f.origin {
        Origin::User => "yours",
        Origin::System => "the system's",
    };
    format!("{name} ({whose}, {} MB)", f.size.div_ceil(1_000_000))
}

/// The settings window.
#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) host: Host,
}

impl Window {
    fn found(&self) -> Vec<Found> {
        crate::playable(&self.host)
    }
}

impl Tab for Window {
    fn content(&self) -> TabContent {
        let settings = self.host.settings().unwrap_or_default();
        let found = self.found();
        let best = found.first().map_or_else(|| "none found".to_owned(), label);
        let mut rows = Vec::new();
        for role in Role::ALL {
            let mut options = vec![format!("The best found: {best}")];
            options.extend(found.iter().map(label));
            let picked = settings
                .get(role.key())
                .and_then(Value::as_str)
                .and_then(|p| {
                    found
                        .iter()
                        .position(|f| f.path.as_os_str() == std::ffi::OsStr::new(p))
                })
                .map_or(0, |i| i + 1);
            rows.push(Row {
                widgets: vec![
                    Widget::Label {
                        text: role.title().into(),
                        muted: false,
                    },
                    Widget::Choice {
                        id: role.key().into(),
                        options,
                        selected: picked as u32,
                        enabled: !found.is_empty(),
                    },
                ],
            });
        }
        let folder = crate::user_folder(&self.host).map_or_else(
            || "the soundfonts folder".to_owned(),
            |p| p.display().to_string(),
        );
        rows.push(Row {
            widgets: vec![Widget::Label {
                text: format!("Put .sf2, .sf3 or .dls files in {folder}."),
                muted: true,
            }],
        });
        TabContent { rows }
    }

    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let Some(role) = Role::ALL.into_iter().find(|r| r.key() == widget) else {
            return Err(Error::not_found(format!("no widget {widget:?}")));
        };
        let TabEvent::Chosen(i) = event else {
            return Ok(());
        };
        let found = self.found();
        let mut settings = self.host.settings()?;
        match (i as usize).checked_sub(1).and_then(|i| found.get(i)) {
            Some(f) => {
                settings.insert(role.key().into(), f.path.display().to_string().into());
            }
            None => {
                settings.remove(role.key());
            }
        }
        self.host.set_settings(&settings)
    }
}

/// The settings key that remembers the warning about basic sets was shown.
pub(crate) const WARNED: &str = "warned_basic_sets";

/// Whether every set in `used` is the system's (all basic, Mateus, 25 Oct
/// 2026: every system set counts as basic, any set of the user's doesn't).
pub(crate) fn only_basic(used: &[&Found]) -> bool {
    !used.is_empty() && used.iter().all(|f| f.origin == sets::Origin::System)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use oxiguitar_plugin::api::message::TrackInfo;

    use super::*;
    use crate::sets::Kind;

    fn track(program: u8, percussion: bool, class: InstrumentClass) -> PreparedTrack {
        PreparedTrack {
            info: TrackInfo {
                index: 0,
                name: String::new(),
                class,
            },
            program,
            percussion,
            tuning: Vec::new(),
            capo: 0,
            settings: "{}".into(),
        }
    }

    #[test]
    fn tracks_fall_into_the_four_roles() {
        assert_eq!(
            Role::of(&track(29, false, InstrumentClass::Fretted)),
            Role::Guitars
        );
        assert_eq!(
            Role::of(&track(105, false, InstrumentClass::Fretted)),
            Role::Guitars
        );
        assert_eq!(
            Role::of(&track(33, false, InstrumentClass::Fretted)),
            Role::Basses
        );
        assert_eq!(
            Role::of(&track(35, false, InstrumentClass::Fretless)),
            Role::Basses
        );
        assert_eq!(
            Role::of(&track(38, false, InstrumentClass::Synth)),
            Role::Basses
        );
        assert_eq!(
            Role::of(&track(0, true, InstrumentClass::Drums)),
            Role::Drums
        );
        assert_eq!(
            Role::of(&track(0, false, InstrumentClass::Keyboard)),
            Role::Other
        );
    }

    fn found(name: &str, origin: Origin) -> Found {
        Found {
            path: PathBuf::from(format!("/sets/{name}")),
            size: 1,
            origin,
            kind: Kind::Sf2,
        }
    }

    #[test]
    fn a_pick_holds_while_its_set_is_there() {
        let sets = [
            found("best.sf2", Origin::User),
            found("other.sf2", Origin::System),
        ];
        let mut s = Map::new();
        assert_eq!(chosen(&s, Role::Drums, &sets).unwrap().path, sets[0].path);
        s.insert("drums".into(), "/sets/other.sf2".into());
        assert_eq!(chosen(&s, Role::Drums, &sets).unwrap().path, sets[1].path);
        assert_eq!(chosen(&s, Role::Basses, &sets).unwrap().path, sets[0].path);
        s.insert("drums".into(), "/sets/gone.sf2".into());
        assert_eq!(chosen(&s, Role::Drums, &sets).unwrap().path, sets[0].path);
        assert!(chosen(&s, Role::Drums, &[]).is_none());
    }

    #[test]
    fn only_the_systems_sets_are_basic() {
        let (mine, theirs) = (found("a.sf2", Origin::User), found("b.dls", Origin::System));
        assert!(only_basic(&[&theirs]));
        assert!(!only_basic(&[&theirs, &mine]));
        assert!(!only_basic(&[]));
    }
}
