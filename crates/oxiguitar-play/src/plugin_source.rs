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
use oxiguitar_plugin_api::message::{Prepare, PrepareEffect, PreparedTrack, TrackInfo};
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
    /// The score's tracks it plays, by their routes: `t` for track `t`,
    /// `tracks + t` for its chord voice (#151; [`crate::events::chords`]),
    /// which plays the track's chord diagrams with its own copy of the
    /// track's instrument settings and effects.
    pub tracks: Vec<usize>,
    /// Each of them as the instrument is told about it.
    pub prepared: Vec<PreparedTrack>,
    /// The effects after each of them, in order: every installed effect
    /// offered for the track's class (interface 1.3), in the order the
    /// track sets ([`Plugins::effects_in_order`], #152), each with the track's
    /// settings for its own plugin, so a change in the track dialog's
    /// section makes a new plan. An effect its settings leave off passes
    /// the sound through.
    pub effects: Vec<Vec<EffectPick>>,
}

/// One effect on a track, as it is prepared.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectPick {
    /// The effect's plugin's id.
    pub plugin: String,
    /// The effect's id in the plugin.
    pub effect: String,
    /// The track's settings for that plugin, a JSON object as text.
    pub settings: String,
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
        let effects: Vec<EffectPick> = plugins
            .effects_in_order(class_of(score, t), &track.effects)
            .into_iter()
            .filter_map(|e| {
                let plugin = e.plugin.manifest.as_ref()?.id.clone();
                Some(EffectPick {
                    settings: track
                        .plugins
                        .get(&plugin)
                        .cloned()
                        .unwrap_or_else(|| "{}".into()),
                    plugin,
                    effect: e.spec.id.clone(),
                })
            })
            .collect();
        // The chord voice, when the track's chords are played: the same
        // instrument and effects, prepared as a track of their own.
        let chords = plays_chords(track).then(|| {
            let mut voice = prepared.clone();
            voice.info.name = format!("{} (chords)", track.name);
            (score.tracks.len() + t, voice, effects.clone())
        });
        let at = match out
            .groups
            .iter()
            .position(|g| g.plugin == plugin && g.instrument == id)
        {
            Some(at) => at,
            None => {
                out.groups.push(Group {
                    plugin,
                    instrument: id,
                    tracks: Vec::new(),
                    prepared: Vec::new(),
                    effects: Vec::new(),
                });
                out.groups.len() - 1
            }
        };
        let g = &mut out.groups[at];
        for (route, prepared, effects) in std::iter::once((t, prepared, effects)).chain(chords) {
            g.tracks.push(route);
            g.prepared.push(prepared);
            g.effects.push(effects);
        }
    }
    out
}

/// Whether a track's chord diagrams play (#151): asked for, and a track
/// with strings that shows some.
fn plays_chords(track: &oxiguitar_model::Track) -> bool {
    track.play_chords
        && !track.chords.is_empty()
        && track.staves.first().is_some_and(|s| !s.tuning.is_empty())
}

/// The sources [`prepare`] made, and what they play.
pub struct Prepared {
    /// One source per instrument, for [`crate::Control::set_sources`].
    pub sources: Sources,
    /// How many sources there are (the routes name them from 0).
    pub count: usize,
    /// Each track's source and slot, then each track's chord voice's
    /// (`tracks + t`), or `None` when nothing plays it.
    pub routes: Vec<Option<Route>>,
    /// The instruments' names.
    pub names: Vec<String>,
    /// Sentences about tracks left silent (no instrument plays them, or
    /// theirs couldn't get ready) and about effects that couldn't get
    /// ready (their tracks play without them).
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
    // A route's track: a chord voice's (`tracks + t`) is track `t`.
    let track_of = |route: usize| route % score.tracks.len().max(1);
    let name_of = |route: usize| {
        let t = track_of(route);
        let n = score.tracks[t].name.trim();
        let name = if n.is_empty() {
            format!("track {}", t + 1)
        } else {
            format!("\u{201c}{n}\u{201d}")
        };
        if route >= score.tracks.len() {
            format!("{name}'s chords")
        } else {
            name
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
    let mut routes = vec![None; score.tracks.len() * 2];
    let mut slots = Vec::new();
    let mut names = Vec::new();
    let mut first_error = None;
    for g in &plan.groups {
        let class = class_of(score, track_of(g.tracks[0]));
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
        let mut effects = Vec::with_capacity(g.tracks.len());
        for (k, &t) in g.tracks.iter().enumerate() {
            let mut chain = Vec::new();
            for pick in g.effects.get(k).into_iter().flatten() {
                let Some(effect) = plugins
                    .effects_for(class_of(score, track_of(t)))
                    .into_iter()
                    .find(|e| {
                        e.spec.id == pick.effect
                            && e.plugin
                                .manifest
                                .as_ref()
                                .is_some_and(|m| m.id == pick.plugin)
                    })
                else {
                    continue;
                };
                let ename = effect.spec.name.clone();
                // The effect hears the track with its own plugin's settings.
                let track = PreparedTrack {
                    settings: pick.settings.clone(),
                    ..g.prepared[k].clone()
                };
                match plugins.prepare_effect(
                    effect,
                    PrepareEffect {
                        effect: pick.effect.clone(),
                        rate,
                        frames: capacity as u32,
                        track,
                    },
                ) {
                    Ok(i) => chain.push(i),
                    Err(e) => silent.push(format!(
                        "{ename} couldn't get ready on {}: {e}. It plays without it.",
                        name_of(t)
                    )),
                }
            }
            effects.push(chain);
        }
        // A chord voice is mixed as its track: its volume, pan, mute and
        // solo.
        slots.push(Slot::new(
            Box::new(PluginSource::new(instance, effects)),
            g.tracks.iter().map(|&r| track_of(r)).collect(),
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
        (0..self.effects.len().max(1))
            .map(|k| self.track_latency(k))
            .max()
            .unwrap_or(0)
    }

    fn track_latency(&self, slot: usize) -> u32 {
        let effects = self
            .effects
            .get(slot)
            .map_or(0, |c| c.iter().map(Instance::latency).sum::<u32>());
        self.instrument.latency() + effects
    }
}
