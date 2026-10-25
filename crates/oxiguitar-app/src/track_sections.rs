// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugins' sections of the track dialog (plugin interface 1.3): which
//! sections the dialog shows for a track, what each shows, and what the
//! track's settings for each plugin become while the dialog is open. On
//! OK they go into the track as part of the dialog's one change; on Cancel
//! they are dropped. The plugins keep nothing: the settings travel with
//! every request. No window code here.

use std::collections::BTreeMap;

use oxiguitar_host::Plugins;
use oxiguitar_host::api::class::InstrumentClass;
use oxiguitar_host::api::message::TrackInfo;
use oxiguitar_host::api::tab::{TabContent, TabEvent};
use oxiguitar_model::Track;

use crate::plugin_tabs::Request;

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
    /// What it shows: `None` until the plugin answers, then its contents
    /// or why they can't be shown.
    pub(crate) content: Option<Result<TabContent, String>>,
}

/// The sections of one opening of the track dialog.
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
}

impl TrackSections {
    /// The sections the plugins add for the track `index` of a score,
    /// numbered `dialog`; none are asked for yet ([`TrackSections::asks`]).
    pub(crate) fn open(plugins: &Plugins, dialog: u64, index: usize, track: &Track) -> Self {
        let class = InstrumentClass::of(track.instrument.midi_program, track.instrument.percussion);
        let sections = plugins
            .track_sections(class)
            .into_iter()
            .map(|s| Shown {
                plugin: s.plugin_id().to_owned(),
                section: s.spec.id.clone(),
                title: s.spec.title.clone(),
                content: None,
            })
            .collect();
        TrackSections {
            dialog,
            track: TrackInfo {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                name: track.name.clone(),
                class,
            },
            sections,
            settings: track.plugins.clone(),
        }
    }

    /// The track's settings for `plugin` as they stand.
    pub(crate) fn settings_of(&self, plugin: &str) -> &str {
        self.settings.get(plugin).map_or(NONE, String::as_str)
    }

    /// What to ask the tab thread for every section's contents.
    pub(crate) fn asks(&self) -> Vec<Request> {
        (0..self.sections.len())
            .filter_map(|i| self.ask(i, None))
            .collect()
    }

    /// What to ask the tab thread when the user used `widget` of the
    /// section at `index` (`None` for its contents alone).
    pub(crate) fn ask(&self, index: usize, event: Option<(String, TabEvent)>) -> Option<Request> {
        let s = self.sections.get(index)?;
        Some(Request::Section {
            dialog: self.dialog,
            track: self.track.clone(),
            plugin: s.plugin.clone(),
            section: s.section.clone(),
            settings: self.settings_of(&s.plugin).to_owned(),
            event,
        })
    }

    /// The tab thread answered for a section: its contents, and the track's
    /// settings for its plugin. Returns whether anything shown changed.
    /// Answers for another opening of the dialog change nothing.
    pub(crate) fn answered(
        &mut self,
        dialog: u64,
        plugin: &str,
        section: &str,
        answer: Result<(TabContent, String), String>,
    ) -> bool {
        if dialog != self.dialog {
            return false;
        }
        let Some(shown) = self
            .sections
            .iter_mut()
            .find(|s| s.plugin == plugin && s.section == section)
        else {
            return false;
        };
        match answer {
            Ok((content, settings)) => {
                shown.content = Some(Ok(content));
                // A plugin's untouched settings aren't written into the
                // track: `{}` stays absent.
                if settings != self.settings_of(plugin) {
                    self.settings.insert(plugin.to_owned(), settings);
                }
            }
            Err(why) => shown.content = Some(Err(why)),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_host::api::tab::{Row, Widget};
    use oxiguitar_host::{SharedSettings, Unheard};
    use oxiguitar_model::Instrument;

    use super::*;

    fn content(text: &str) -> TabContent {
        TabContent {
            rows: vec![Row {
                widgets: vec![Widget::Label {
                    text: text.into(),
                    muted: false,
                }],
            }],
        }
    }

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
                content: None,
            });
        }
        s
    }

    #[test]
    fn sections_start_from_the_track_s_settings_and_keep_the_plugins_answers() {
        let mut s = sections();
        assert_eq!(s.track.index, 3);
        assert_eq!(s.track.class, InstrumentClass::Fretted);
        let asks = s.asks();
        assert_eq!(asks.len(), 2);
        assert!(matches!(
            &asks[0],
            Request::Section { dialog: 7, settings, event: None, .. } if settings == r#"{"gain":2}"#
        ));
        assert!(matches!(&asks[1], Request::Section { settings, .. } if settings == NONE));

        // An untouched plugin's `{}` isn't written into the track.
        assert!(s.answered(7, "me.cab", "cab", Ok((content("Cab"), NONE.into()))));
        assert!(!s.settings.contains_key("me.cab"));
        assert!(s.answered(
            7,
            "me.amp",
            "amp",
            Ok((content("Amp"), r#"{"gain":3}"#.into()))
        ));
        assert_eq!(s.settings_of("me.amp"), r#"{"gain":3}"#);
        assert_eq!(s.sections[0].content, Some(Ok(content("Amp"))));

        // A failure is shown in the section; the settings stay.
        assert!(s.answered(7, "me.amp", "amp", Err("it crashed".into())));
        assert_eq!(s.sections[0].content, Some(Err("it crashed".into())));
        assert_eq!(s.settings_of("me.amp"), r#"{"gain":3}"#);

        // An answer for an earlier opening, or a section it doesn't have,
        // changes nothing.
        assert!(!s.answered(6, "me.amp", "amp", Ok((content("Old"), "{}".into()))));
        assert!(!s.answered(7, "me.amp", "other", Ok((content("No"), "{}".into()))));
        assert_eq!(s.settings_of("me.amp"), r#"{"gain":3}"#);

        let event = s
            .ask(0, Some(("gain".into(), TabEvent::Slid(4.0))))
            .unwrap();
        assert!(matches!(
            event,
            Request::Section { settings, event: Some(_), .. } if settings == r#"{"gain":3}"#
        ));
        assert!(s.ask(5, None).is_none());
    }
}
