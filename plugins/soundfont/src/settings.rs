// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin's settings window: a set for electric guitars, acoustic
//! guitars, basses, drums and everything else, for every file (Mateus, 21
//! Oct 2026; Guitars split in two on 26 Oct 2026, since an SFZ guitar is one
//! or the other), kept in the plugin's own settings as each set's path.
//! Until the user picks, the best General MIDI set found plays; an SFZ
//! instrument plays only where picked.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use oxiguitar_plugin::audio::PreparedTrack;
use oxiguitar_plugin::serde_json::{Map, Value};
use oxiguitar_plugin::{Error, Host, InstrumentClass, Row, Tab, TabContent, TabEvent, Widget};

use crate::sets::{self, Found, Kind, Origin};

/// The kinds of track the window picks a set for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    ElectricGuitars,
    AcousticGuitars,
    Basses,
    Drums,
    Other,
}

/// The key 0.9's settings kept one pick for both kinds of guitar under.
const GUITARS: &str = "guitars";

impl Role {
    pub(crate) const ALL: [Role; 5] = [
        Role::ElectricGuitars,
        Role::AcousticGuitars,
        Role::Basses,
        Role::Drums,
        Role::Other,
    ];

    /// Its key in the settings and its widget's id.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Role::ElectricGuitars => "electric-guitars",
            Role::AcousticGuitars => "acoustic-guitars",
            Role::Basses => "basses",
            Role::Drums => "drums",
            Role::Other => "other",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Role::ElectricGuitars => "Electric guitars",
            Role::AcousticGuitars => "Acoustic guitars",
            Role::Basses => "Basses",
            Role::Drums => "Drums",
            Role::Other => "Everything else",
        }
    }

    fn guitar(self) -> bool {
        matches!(self, Role::ElectricGuitars | Role::AcousticGuitars)
    }

    /// The role of a track: drum kits; basses by their General MIDI
    /// program (32 to 39: acoustic, electric, fretless, slap and synth);
    /// guitars, acoustic (24 and 25) or electric (26 to 31); then the rest,
    /// the other fretted instruments (banjo, sitar…) among them, so they
    /// keep their General MIDI sounds whatever the guitars play.
    pub(crate) fn of(track: &PreparedTrack) -> Role {
        if track.percussion {
            Role::Drums
        } else if (32..=39).contains(&track.program) {
            Role::Basses
        } else if track.info.class == InstrumentClass::Fretted && (24..=25).contains(&track.program)
        {
            Role::AcousticGuitars
        } else if track.info.class == InstrumentClass::Fretted && (26..=31).contains(&track.program)
        {
            Role::ElectricGuitars
        } else {
            Role::Other
        }
    }

    /// What `settings` picks for this role: its own key, or for guitars
    /// 0.9's one pick for both.
    fn pick(self, settings: &Map<String, Value>) -> Option<&str> {
        settings
            .get(self.key())
            .or_else(|| self.guitar().then(|| settings.get(GUITARS)).flatten())
            .and_then(Value::as_str)
    }
}

/// The set `settings` picks for `role` among `found`, if it picks one that
/// is still there; else the best General MIDI set (an SFZ instrument plays
/// only where picked: Mateus, 26 Oct 2026).
pub(crate) fn chosen<'a>(
    settings: &Map<String, Value>,
    role: Role,
    found: &'a [Found],
) -> Option<&'a Found> {
    role.pick(settings)
        .and_then(|p| {
            found
                .iter()
                .find(|f| f.path.as_os_str() == std::ffi::OsStr::new(p))
        })
        .or_else(|| found.iter().find(|f| f.kind != Kind::Sfz))
}

/// How the window names an SFZ instrument: its set's folder (the first
/// folder under the `soundfonts` folder) and its own name.
fn sfz_label(f: &Found, user: Option<&Path>) -> String {
    let stem = f
        .path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let set = user
        .and_then(|u| f.path.strip_prefix(u).ok())
        .and_then(|rest| rest.components().next())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .filter(|s| f.path.file_name().is_none_or(|n| n != s.as_str()));
    match set {
        Some(set) => format!("{set}: {stem} (SFZ)"),
        None => format!("{stem} (SFZ)"),
    }
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

/// Whether an SFZ file plays as an instrument, as it was when checked.
#[derive(Debug)]
struct Checked {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
    instrument: bool,
}

/// The settings window.
#[derive(Debug)]
pub(crate) struct Window {
    pub(crate) host: Host,
    /// The SFZ files checked so far.
    checked: Mutex<Vec<Checked>>,
}

impl Window {
    pub(crate) fn new(host: Host) -> Window {
        Window {
            host,
            checked: Mutex::new(Vec::new()),
        }
    }

    /// Every set found, the General MIDI ones first, then the SFZ
    /// instruments that play on their own.
    fn found(&self) -> Vec<Found> {
        let found = crate::playable(&self.host);
        let (sfz, mut sets): (Vec<Found>, Vec<Found>) =
            found.into_iter().partition(|f| f.kind == Kind::Sfz);
        let mut sfz: Vec<Found> = sfz
            .into_iter()
            .filter(|f| self.instrument(&f.path))
            .collect();
        sfz.sort_by(|a, b| a.path.cmp(&b.path));
        sets.extend(sfz);
        sets
    }

    /// Whether the SFZ file at `path` is an instrument, not a part others
    /// include: it has regions, and every `$name` it uses is defined
    /// (Karoryfer's sets keep parts that work only when included). Kept
    /// while the file is unchanged.
    fn instrument(&self, path: &Path) -> bool {
        let meta = std::fs::metadata(path).ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok());
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        if let Ok(checked) = self.checked.lock()
            && let Some(c) = checked
                .iter()
                .find(|c| c.path == path && c.modified == modified && c.size == size)
        {
            return c.instrument;
        }
        let instrument = crate::sfz::text::read(path).is_ok_and(|t| {
            t.items
                .iter()
                .any(|i| matches!(i, crate::sfz::text::Item::Header(h) if h == "region"))
                && !t.problems.iter().any(|p| p.ends_with("isn't defined"))
                && !matches!(t.items.first(), Some(crate::sfz::text::Item::Opcode(..)))
        });
        if let Ok(mut checked) = self.checked.lock() {
            checked.retain(|c| c.path != path);
            checked.push(Checked {
                path: path.to_path_buf(),
                modified,
                size,
                instrument,
            });
        }
        instrument
    }

    fn label(&self, f: &Found) -> String {
        if f.kind == Kind::Sfz {
            sfz_label(f, crate::user_folder(&self.host).as_deref())
        } else {
            label(f)
        }
    }
}

impl Tab for Window {
    fn content(&self) -> TabContent {
        let settings = self.host.settings().unwrap_or_default();
        let found = self.found();
        let best = found
            .iter()
            .find(|f| f.kind != Kind::Sfz)
            .map_or_else(|| "none found".to_owned(), label);
        let labels: Vec<String> = found.iter().map(|f| self.label(f)).collect();
        let mut rows = Vec::new();
        for role in Role::ALL {
            let mut options = vec![format!("The best found: {best}")];
            options.extend(labels.iter().cloned());
            let picked = role
                .pick(&settings)
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
                text: format!(
                    "Put .sf2, .sf3 or .dls files, or SFZ instruments' folders, in {folder}."
                ),
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
        // 0.9's one pick for guitars goes to both guitar roles, so picking
        // one leaves the other as it was.
        if role.guitar()
            && let Some(old) = settings.remove(GUITARS)
        {
            for r in [Role::ElectricGuitars, Role::AcousticGuitars] {
                settings.entry(r.key()).or_insert_with(|| old.clone());
            }
        }
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
    fn tracks_fall_into_the_five_roles() {
        assert_eq!(
            Role::of(&track(29, false, InstrumentClass::Fretted)),
            Role::ElectricGuitars
        );
        assert_eq!(
            Role::of(&track(25, false, InstrumentClass::Fretted)),
            Role::AcousticGuitars
        );
        assert_eq!(
            Role::of(&track(105, false, InstrumentClass::Fretted)),
            Role::Other
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
    fn the_old_guitars_pick_holds_for_both_kinds_and_sfz_only_where_picked() {
        let mut sfz = found("kit/guitar.sfz", Origin::User);
        sfz.kind = Kind::Sfz;
        let sets = [sfz, found("best.sf2", Origin::User)];
        let mut s = Map::new();
        // An SFZ instrument is never the best found.
        assert_eq!(
            chosen(&s, Role::ElectricGuitars, &sets).unwrap().path,
            sets[1].path
        );
        s.insert("guitars".into(), "/sets/kit/guitar.sfz".into());
        assert_eq!(
            chosen(&s, Role::ElectricGuitars, &sets).unwrap().path,
            sets[0].path
        );
        assert_eq!(
            chosen(&s, Role::AcousticGuitars, &sets).unwrap().path,
            sets[0].path
        );
        assert_eq!(chosen(&s, Role::Basses, &sets).unwrap().path, sets[1].path);
        s.insert("acoustic-guitars".into(), "/sets/best.sf2".into());
        assert_eq!(
            chosen(&s, Role::AcousticGuitars, &sets).unwrap().path,
            sets[1].path
        );
        assert_eq!(
            chosen(&s, Role::ElectricGuitars, &sets).unwrap().path,
            sets[0].path
        );
    }

    #[test]
    fn sfz_instruments_are_named_by_their_set_and_their_own_name() {
        let mut f = found("x", Origin::User);
        f.kind = Kind::Sfz;
        f.path = PathBuf::from(
            "/home/u/soundfonts/karoryfer.black-and-green-guitars/Programs/01-green_keyswitch.sfz",
        );
        assert_eq!(
            sfz_label(&f, Some(Path::new("/home/u/soundfonts"))),
            "karoryfer.black-and-green-guitars: 01-green_keyswitch (SFZ)"
        );
        f.path = PathBuf::from("/home/u/soundfonts/loose.sfz");
        assert_eq!(
            sfz_label(&f, Some(Path::new("/home/u/soundfonts"))),
            "loose (SFZ)"
        );
    }

    #[test]
    fn only_the_systems_sets_are_basic() {
        let (mine, theirs) = (found("a.sf2", Origin::User), found("b.dls", Origin::System));
        assert!(only_basic(&[&theirs]));
        assert!(!only_basic(&[&theirs, &mine]));
        assert!(!only_basic(&[]));
    }
}
