// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugins' sections of the Track tab (plugin interface 1.3): which
//! sections the tab shows for a track, and what the track's settings for
//! each plugin become. Each section is its plugin's own view in Slint
//! (`plugin_ui.rs`), told the track and its settings when it is made; the
//! settings it hands back come here. The sections of plugins with effects for the track come
//! last, in the order the effects run, and can be moved up and down, which
//! changes that order (#152). No window code here.

use std::collections::BTreeMap;

use oxiguitar_host::Plugins;
use oxiguitar_host::api::class::InstrumentClass;
use oxiguitar_host::api::message::TrackInfo;
use oxiguitar_model::Track;

/// The settings a plugin starts from for a track that has none.
pub(crate) const NONE: &str = "{}";

/// One section, as the dialog shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shown {
    /// The plugin's id.
    pub(crate) plugin: String,
    /// The section's id.
    pub(crate) section: String,
    /// Its heading.
    pub(crate) title: String,
}

/// The sections for one track, asked once (until the caret moves to
/// another track or the settings change by Undo or Redo).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackSections {
    /// Which opening of the dialog this is: answers for an earlier one are
    /// dropped.
    pub(crate) dialog: u64,
    /// The track, as the plugins are told about it.
    pub(crate) track: TrackInfo,
    /// The sections, in plugin order.
    pub(crate) sections: Vec<Shown>,
    /// The track's settings for each plugin, as they stand in the dialog.
    pub(crate) settings: BTreeMap<String, String>,
    /// The plugins with effects for the track, in the order they run.
    pub(crate) effects: Vec<String>,
    /// The order of the track's effects as it goes into the track: the
    /// track's own (empty for the default) until a section is moved, then
    /// [`TrackSections::effects`].
    pub(crate) order: Vec<String>,
}

impl TrackSections {
    /// The sections the plugins add for the track `index` of a score,
    /// numbered `dialog`; none are asked for yet ([`TrackSections::asks`]).
    pub(crate) fn open(plugins: &Plugins, dialog: u64, index: usize, track: &Track) -> Self {
        let class = InstrumentClass::of(track.instrument.midi_program, track.instrument.percussion);
        let mut effects: Vec<String> = Vec::new();
        for e in plugins.effects_in_order(class, &track.effects) {
            let id = e.plugin.manifest.as_ref().map_or("", |m| m.id.as_str());
            if !effects.iter().any(|x| x == id) {
                effects.push(id.to_owned());
            }
        }
        // A section is shown by its plugin's view of the same id; one with
        // none (a manifest the contract suite refuses) isn't shown.
        let sections = plugins
            .track_sections(class)
            .into_iter()
            .filter(|s| {
                let id = oxiguitar_host::ViewId {
                    plugin: s.plugin_id().to_owned(),
                    view: s.spec.id.clone(),
                };
                plugins
                    .view(&id)
                    .is_some_and(|v| v.spec.place == oxiguitar_host::api::view::Place::Track)
            })
            .map(|s| Shown {
                plugin: s.plugin_id().to_owned(),
                section: s.spec.id.clone(),
                title: s.spec.title.clone(),
            })
            .collect();
        let mut out = TrackSections {
            dialog,
            track: TrackInfo {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                name: track.name.clone(),
                class,
            },
            sections,
            settings: track.plugins.clone(),
            effects,
            order: track.effects.clone(),
        };
        out.arrange();
        out
    }

    /// Puts the sections of plugins with effects last, in the order the
    /// effects run; the others keep the plugins' order.
    fn arrange(&mut self) {
        let effects = &self.effects;
        self.sections.sort_by_key(|s| {
            effects
                .iter()
                .position(|e| *e == s.plugin)
                .map_or((0, 0), |at| (1, at))
        });
    }

    /// The plugins with effects that have sections here, in order.
    fn movable(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.sections {
            if self.effects.contains(&s.plugin) && !out.contains(&s.plugin.as_str()) {
                out.push(&s.plugin);
            }
        }
        out
    }

    /// Whether the section at `index` can move up and down: the first
    /// section of a plugin with effects, with another such plugin's
    /// section before or after it.
    pub(crate) fn moves(&self, index: usize) -> (bool, bool) {
        let Some(s) = self.sections.get(index) else {
            return (false, false);
        };
        let first = self.sections.iter().position(|x| x.plugin == s.plugin) == Some(index);
        let movable = self.movable();
        match movable.iter().position(|p| *p == s.plugin) {
            Some(at) if first => (at > 0, at + 1 < movable.len()),
            _ => (false, false),
        }
    }

    /// Moves the section at `index`, with its plugin's other sections and
    /// effects, before the plugin shown above it (`up`) or after the one
    /// shown below it. Returns whether it moved.
    pub(crate) fn move_section(&mut self, index: usize, up: bool) -> bool {
        let (can_up, can_down) = self.moves(index);
        if !(if up { can_up } else { can_down }) {
            return false;
        }
        let plugin = self.sections[index].plugin.clone();
        let movable: Vec<String> = self.movable().into_iter().map(str::to_owned).collect();
        let Some(at) = movable.iter().position(|p| *p == plugin) else {
            return false;
        };
        // Moving down is moving the one below up.
        let (moved, before) = if up {
            (plugin, movable[at - 1].clone())
        } else {
            (movable[at + 1].clone(), plugin)
        };
        self.effects.retain(|e| *e != moved);
        let to = self
            .effects
            .iter()
            .position(|e| *e == before)
            .unwrap_or(self.effects.len());
        self.effects.insert(to, moved);
        self.order.clone_from(&self.effects);
        self.arrange();
        true
    }

    /// The track's settings for `plugin` as they stand.
    pub(crate) fn settings_of(&self, plugin: &str) -> &str {
        self.settings.get(plugin).map_or(NONE, String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_host::{SharedSettings, Unheard};
    use oxiguitar_model::Instrument;

    use super::*;

    fn sections() -> TrackSections {
        let plugins = Plugins::discover(
            &[],
            SharedSettings::default(),
            "x11",
            std::sync::Arc::new(Unheard),
        );
        let track = Track {
            name: "Lead".into(),
            instrument: Instrument {
                midi_program: 29,
                percussion: false,
            },
            plugins: [("me.amp".to_owned(), r#"{"gain":2}"#.to_owned())].into(),
            ..Track::default()
        };
        let mut s = TrackSections::open(&plugins, 7, 3, &track);
        // No plugins here: the sections are added as a plugin would give
        // them.
        for (plugin, section) in [("me.amp", "amp"), ("me.cab", "cab")] {
            s.sections.push(Shown {
                plugin: plugin.into(),
                section: section.into(),
                title: section.into(),
            });
        }
        s
    }

    #[test]
    fn sections_start_from_the_track_s_settings() {
        let s = sections();
        assert_eq!(s.track.index, 3);
        assert_eq!(s.track.class, InstrumentClass::Fretted);
        assert_eq!(s.settings_of("me.amp"), r#"{"gain":2}"#);
        assert_eq!(s.settings_of("me.cab"), NONE);
    }

    #[test]
    fn effect_sections_come_last_in_their_order_and_move_up_and_down() {
        let mut s = sections();
        let shown = |plugin: &str, section: &str| Shown {
            plugin: plugin.into(),
            section: section.into(),
            title: section.into(),
        };
        s.sections = vec![
            shown("me.amp", "amp"),
            shown("me.sound", "sound"),
            shown("me.amp", "amp2"),
            shown("me.cab", "cab"),
            shown("me.eq", "eq"),
        ];
        // `me.hidden` has an effect and no section.
        s.effects = ["me.cab", "me.hidden", "me.amp", "me.eq"]
            .map(String::from)
            .to_vec();
        s.arrange();
        let order = |s: &TrackSections| {
            s.sections
                .iter()
                .map(|x| x.section.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&s), ["sound", "cab", "amp", "amp2", "eq"]);
        let moves: Vec<_> = (0..6).map(|i| s.moves(i)).collect();
        assert_eq!(
            moves,
            [
                (false, false),
                (false, true),
                (true, true),
                (false, false),
                (true, false),
                (false, false)
            ]
        );
        // Nothing moved: the track keeps its own order (here the default).
        assert!(s.order.is_empty());
        assert!(!s.move_section(0, true));
        assert!(!s.move_section(1, true));
        assert!(!s.move_section(4, false));
        assert!(s.order.is_empty());

        // The amp up: before the cabinet, with its second section.
        assert!(s.move_section(2, true));
        assert_eq!(order(&s), ["sound", "amp", "amp2", "cab", "eq"]);
        assert_eq!(s.effects, ["me.amp", "me.cab", "me.hidden", "me.eq"]);
        assert_eq!(s.order, s.effects);
        // The cabinet down: after the equalizer.
        assert!(s.move_section(3, false));
        assert_eq!(order(&s), ["sound", "amp", "amp2", "eq", "cab"]);
        assert_eq!(s.order, ["me.amp", "me.eq", "me.cab", "me.hidden"]);
    }
}
