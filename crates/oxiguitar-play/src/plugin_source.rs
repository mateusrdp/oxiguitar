// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An instrument plugin's prepared instance as one of the engine's
//! sources, with the effects of each of its tracks after it (interface
//! 1.3), and the instruments for a score's tracks, chosen and prepared
//! ([`plan`], [`prepare`]), as the window and `oxiguitar-cli play` share
//! them.

use oxiguitar_host::Plugins;
use oxiguitar_host::realtime::Instance;
use oxiguitar_model::Score;
use oxiguitar_plugin_api::class::InstrumentClass;
use oxiguitar_plugin_api::message::{Prepare, PreparedTrack, TrackInfo};
use oxiguitar_plugin_api::realtime::{PitchPoint, RtEvent};

use crate::engine::{Buffers, Slot, Source, Sources};
use crate::schedule::Route;

/// One instrument and the tracks it plays, as they are prepared.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The plugin's id.
    pub plugin: String,
    /// The instrument's id in the plugin.
    pub instrument: String,
    /// The score's tracks it plays.
    pub tracks: Vec<usize>,
    /// Each of them as the instrument is told about it.
    pub prepared: Vec<PreparedTrack>,
}

/// What a score's instruments are prepared for: each track goes to the
/// first installed instrument for its class, and the tracks of one
/// instrument share one prepared instance. Two plans that are equal need
/// no preparing again (an edit of the notes leaves it as it is).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// The instruments, in the order of their first track.
    pub groups: Vec<Group>,
    /// Tracks no installed instrument plays.
    pub unplayed: Vec<usize>,
}

impl Plan {
    /// Nothing plays any track.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// The class of a track's instrument.
pub fn class_of(score: &Score, track: usize) -> InstrumentClass {
    let i = score.tracks[track].instrument;
    InstrumentClass::of(i.midi_program, i.percussion)
}

/// The plan for `score`'s tracks with the installed instruments.
pub fn plan(plugins: &Plugins, score: &Score) -> Plan {
    let mut out = Plan::default();
    for t in 0..score.tracks.len() {
        let Some(i) = plugins.playable(class_of(score, t)).into_iter().next() else {
            out.unplayed.push(t);
            continue;
        };
        let plugin = i
            .plugin
            .manifest
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_default();
        let id = i.spec.id.clone();
        let track = &score.tracks[t];
        let staff = track.staves.first();
        let prepared = PreparedTrack {
            info: TrackInfo {
                index: t as u32,
                name: track.name.clone(),
                class: class_of(score, t),
            },
            program: track.instrument.midi_program,
            percussion: track.instrument.percussion,
            tuning: staff
                .map(|s| s.tuning.iter().map(|p| p.0).collect())
                .unwrap_or_default(),
            capo: staff.map_or(0, |s| s.capo),
            settings: track
                .plugins
                .get(&plugin)
                .cloned()
                .unwrap_or_else(|| "{}".into()),
        };
        match out
            .groups
            .iter_mut()
            .find(|g| g.plugin == plugin && g.instrument == id)
        {
            Some(g) => {
                g.tracks.push(t);
                g.prepared.push(prepared);
            }
            None => out.groups.push(Group {
                plugin,
                instrument: id,
                tracks: vec![t],
                prepared: vec![prepared],
            }),
        }
    }
    out
}

/// The sources [`prepare`] made, and what they play.
pub struct Prepared {
    /// One source per instrument, for [`crate::Control::set_sources`].
    pub sources: Sources,
    /// How many sources there are (the routes name them from 0).
    pub count: usize,
    /// Each track's source and slot, or `None` when nothing plays it.
    pub routes: Vec<Option<Route>>,
    /// The instruments' names.
    pub names: Vec<String>,
    /// Sentences about tracks left silent: no instrument plays them, or
    /// theirs couldn't get ready.
    pub silent: Vec<String>,
}

/// Prepares `plan`'s instruments for `score` at `rate`, in pieces of at
/// most `capacity` frames. An instrument that can't get ready leaves its
/// tracks silent, said in [`Prepared::silent`]; when none can, the error
/// says why (the first instrument's reason).
pub fn prepare(
    plugins: &Plugins,
    score: &Score,
    plan: &Plan,
    rate: u32,
    capacity: usize,
) -> Result<Prepared, String> {
    let name_of = |t: usize| {
        let n = score.tracks[t].name.trim();
        if n.is_empty() {
            format!("track {}", t + 1)
        } else {
            format!("\u{201c}{n}\u{201d}")
        }
    };
    let mut silent: Vec<String> = plan
        .unplayed
        .iter()
        .map(|&t| {
            format!(
                "No installed instrument plays {}, so it is silent.",
                name_of(t)
            )
        })
        .collect();
    let mut routes = vec![None; score.tracks.len()];
    let mut slots = Vec::new();
    let mut names = Vec::new();
    let mut first_error = None;
    for g in &plan.groups {
        let class = class_of(score, g.tracks[0]);
        let Some(instrument) = plugins.playable(class).into_iter().find(|i| {
            i.spec.id == g.instrument
                && i.plugin.manifest.as_ref().is_some_and(|m| m.id == g.plugin)
        }) else {
            continue;
        };
        let iname = instrument.spec.name.clone();
        let instance = plugins.prepare(
            instrument,
            Prepare {
                instrument: g.instrument.clone(),
                rate,
                frames: capacity as u32,
                tracks: g.prepared.clone(),
            },
        );
        let instance = match instance {
            Ok(i) => i,
            Err(e) => {
                let why = format!("{iname} couldn't get ready: {e}");
                let tracks: Vec<String> = g.tracks.iter().map(|&t| name_of(t)).collect();
                silent.push(format!("{why}. Silent: {}.", tracks.join(", ")));
                first_error.get_or_insert(why);
                continue;
            }
        };
        let source = slots.len();
        for (slot, &t) in g.tracks.iter().enumerate() {
            routes[t] = Some(Route { source, slot });
        }
        names.push(iname);
        let effects = g.tracks.iter().map(|_| Vec::new()).collect();
        slots.push(Slot::new(
            Box::new(PluginSource::new(instance, effects)),
            g.tracks.clone(),
            capacity,
        ));
    }
    if slots.is_empty() {
        return Err(
            first_error.unwrap_or_else(|| "no installed instrument plays its tracks".into())
        );
    }
    Ok(Prepared {
        count: slots.len(),
        sources: Sources::new(slots),
        routes,
        names,
        silent,
    })
}

/// An instrument instance and, per track it plays, the effects that run
/// on the track after it, in order.
#[derive(Debug)]
pub struct PluginSource {
    instrument: Instance,
    effects: Vec<Vec<Instance>>,
}

impl PluginSource {
    /// `instrument`, with `effects[slot]` after each of its tracks.
    pub fn new(instrument: Instance, effects: Vec<Vec<Instance>>) -> PluginSource {
        PluginSource {
            instrument,
            effects,
        }
    }
}

impl Source for PluginSource {
    fn render(
        &mut self,
        events: &[RtEvent],
        pitch: &[PitchPoint],
        time: f64,
        step: f64,
        out: &mut Buffers,
    ) {
        let frames = out.frames();
        if !self
            .instrument
            .render(events, pitch, time, step, frames, out.all())
        {
            return;
        }
        if frames == 0 {
            return;
        }
        for (slot, chain) in self.effects.iter_mut().enumerate() {
            for effect in chain {
                if let Some((l, r)) = out.track(slot) {
                    effect.process(l, r);
                }
            }
        }
    }

    fn latency(&self) -> u32 {
        let effects = self
            .effects
            .iter()
            .map(|c| c.iter().map(Instance::latency).sum::<u32>())
            .max()
            .unwrap_or(0);
        self.instrument.latency() + effects
    }
}
