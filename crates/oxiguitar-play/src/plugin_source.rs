// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An instrument plugin's prepared instance as one of the engine's
//! sources, with the effects of each of its tracks after it (interface
//! 1.3), and the instruments for a score's tracks, chosen and prepared
//! ([`plan`], [`prepare`]), as the window and `oxiguitar-cli play` share
//! them.

use oxiguitar_host::Plugins;
use oxiguitar_host::realtime::{Instance, Updater};
use oxiguitar_model::{Ratio, Score};
use oxiguitar_plugin_api::class::InstrumentClass;
use oxiguitar_plugin_api::message::{
    Perform, PerformStaff, PlayedBeat, PlayedNote, Prepare, PrepareEffect, PreparedTrack, TrackInfo,
};
use oxiguitar_plugin_api::realtime::{PitchPoint, RtEvent};

use crate::engine::{Buffers, Slot, Source, Sources};
use crate::schedule::Route;
use crate::time::TimeMap;

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
    /// For an effect that follows the score (interface 1.8): the track's
    /// marked beats as they are played ([`played_beats`]), so a change of
    /// the marks makes a new plan; empty for any other effect.
    pub beats: Vec<PlayedBeat>,
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
    // When the bars are played, worked out only if an effect follows the
    // score.
    let mut map: Option<TimeMap> = None;
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
        let mut beats: Option<Vec<PlayedBeat>> = None;
        let effects: Vec<EffectPick> = plugins
            .effects_in_order(class_of(score, t), &track.effects)
            .into_iter()
            .filter_map(|e| {
                let plugin = e.plugin.manifest.as_ref()?.id.clone();
                let beats = if plugins.follows_score(e) {
                    let map = map.get_or_insert_with(|| {
                        TimeMap::new(score, &crate::order::playing_order(&score.master_bars))
                    });
                    beats
                        .get_or_insert_with(|| played_beats(track, map))
                        .clone()
                } else {
                    Vec::new()
                };
                Some(EffectPick {
                    beats,
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

/// Every beat of `track` that carries a mark (an extension of the score,
/// version 2: the notation's marks and what plugins keep on beats), each
/// time it is played, in time order: what an effect that follows the score
/// is prepared with (interface 1.8).
pub fn played_beats(track: &oxiguitar_model::Track, map: &TimeMap) -> Vec<PlayedBeat> {
    let index = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    let mut out = Vec::new();
    for (si, staff) in track.staves.iter().enumerate() {
        for (visit, v) in map.visits.iter().enumerate() {
            let Some(bar) = staff.bars.get(v.visit.bar) else {
                continue;
            };
            for (vi, voice) in bar.voices.iter().enumerate() {
                let mut pos = Ratio::ZERO;
                for (bi, b) in voice.beats.iter().enumerate() {
                    if b.grace.is_some() {
                        continue;
                    }
                    let Some(len) = b.duration.length() else {
                        continue;
                    };
                    let ext = oxiguitar_model::wire::beat_marks(b);
                    if !ext.is_empty() {
                        out.push(PlayedBeat {
                            start: map.time(visit, pos),
                            end: map.time(visit, pos + len),
                            staff: index(si),
                            bar: index(v.visit.bar),
                            voice: index(vi),
                            beat: index(bi),
                            ext,
                        });
                    }
                    pos = pos + len;
                }
            }
        }
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// Whether a track's chord diagrams play (#151): asked for, and a track
/// with strings that shows some; or cross-referenced chords are active
/// (`chords_from` set), which still needs a chord voice for cursor cues.
fn plays_chords(track: &oxiguitar_model::Track) -> bool {
    if track.chords_from != usize::MAX {
        return true;
    }
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
    /// Every effect prepared, to send its track's settings to while the
    /// song plays ([`follow`]).
    pub live: Vec<LiveEffect>,
}

/// An effect as prepared on one route, which can be sent its track's
/// settings while it runs (interface 1.5, [`follow`]).
#[derive(Debug)]
pub struct LiveEffect {
    /// The route ([`Group::tracks`]).
    route: usize,
    /// Its place in the route's chain of the plan ([`Group::effects`]).
    at: usize,
    /// What it was prepared with; its settings are the ones it follows now.
    prepare: PrepareEffect,
    updater: Updater,
}

/// The song plays and `new` is the plan for the score as it now is,
/// `old` the one its instruments and effects were prepared for: sends each
/// effect whose track's settings changed its new ones (interface 1.5),
/// without preparing anything. Returns whether what plays now follows
/// `new`. False when anything else differs (an instrument, its settings,
/// the effects or their order) or an effect can't follow its new settings
/// while it runs: then `new` is to be prepared once the song stops.
pub fn follow(live: &mut [LiveEffect], old: &Plan, new: &Plan) -> bool {
    // Only the effects' settings may differ.
    if shape_of(new) != shape_of(old) {
        return false;
    }
    let mut all = true;
    for g in &new.groups {
        for (k, &route) in g.tracks.iter().enumerate() {
            for (at, pick) in g.effects[k].iter().enumerate() {
                let Some(e) = live.iter_mut().find(|e| e.route == route && e.at == at) else {
                    // It couldn't get ready: it can follow nothing.
                    all &= old_settings(old, route, at) == Some(&pick.settings);
                    continue;
                };
                if e.prepare.track.settings == pick.settings {
                    continue;
                }
                let mut prepare = e.prepare.clone();
                prepare.track.settings.clone_from(&pick.settings);
                match e.updater.update(prepare.clone(), &e.prepare.track.settings) {
                    Ok(true) => e.prepare = prepare,
                    _ => all = false,
                }
            }
        }
    }
    all
}

/// `plan` without its effects' settings.
fn shape_of(plan: &Plan) -> Plan {
    let mut out = plan.clone();
    for pick in out
        .groups
        .iter_mut()
        .flat_map(|g| g.effects.iter_mut().flatten())
    {
        pick.settings.clear();
    }
    out
}

/// The settings `plan` gives the effect `at` of `route`'s chain.
fn old_settings(plan: &Plan, route: usize, at: usize) -> Option<&String> {
    plan.groups.iter().find_map(|g| {
        let k = g.tracks.iter().position(|&r| r == route)?;
        g.effects.get(k)?.get(at).map(|p| &p.settings)
    })
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
    let mut live = Vec::new();
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
            for (at, pick) in g.effects.get(k).into_iter().flatten().enumerate() {
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
                let prepare = PrepareEffect {
                    effect: pick.effect.clone(),
                    rate,
                    frames: capacity as u32,
                    track,
                };
                match plugins.prepare_effect_following(effect, prepare.clone(), &pick.beats) {
                    Ok(i) => {
                        live.push(LiveEffect {
                            route: t,
                            at,
                            prepare,
                            updater: i.updater(),
                        });
                        chain.push(i);
                    }
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
        live,
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
        playing: bool,
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
                    // An effect follows the score only while it plays.
                    effect.process(l, r, time, if playing { step } else { 0.0 });
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

/// The timeline of `score` as the performing plugins play it (interface
/// 1.8): each track's notes given to each plugin whose manifest `performs`,
/// in plugin order, and replaced with what it gives back. A plugin that
/// can't leaves the notes as they were. Chord diagrams' notes aren't given.
pub fn perform(plugins: &Plugins, score: &Score, timeline: &mut crate::Timeline) {
    let performers = plugins.performers();
    if performers.is_empty() {
        return;
    }
    let visits: Vec<u32> = timeline
        .map
        .visits
        .iter()
        .map(|v| u32::try_from(v.visit.bar).unwrap_or(u32::MAX))
        .collect();
    let mut by_track: Vec<Vec<crate::events::NoteEvent>> = vec![Vec::new(); score.tracks.len()];
    for n in timeline.notes.drain(..) {
        if let Some(t) = by_track.get_mut(n.track) {
            t.push(n);
        }
    }
    for (ti, notes) in by_track.iter_mut().enumerate() {
        let Some(track) = score.tracks.get(ti) else {
            continue;
        };
        let beats = played_beats(track, &timeline.map);
        let staves: Vec<PerformStaff> = track
            .staves
            .iter()
            .map(|s| PerformStaff {
                tuning: s.tuning.iter().map(|p| p.0).collect(),
                capo: s.capo,
            })
            .collect();
        for plugin in &performers {
            let request = Perform {
                track: u32::try_from(ti).unwrap_or(u32::MAX),
                class: class_of(score, ti),
                visits: visits.clone(),
                notes: notes.iter().map(to_played).collect(),
                beats: beats.clone(),
                program: track.instrument.midi_program,
                staves: staves.clone(),
                settings: track
                    .plugins
                    .get(plugin)
                    .cloned()
                    .unwrap_or_else(|| "{}".into()),
            };
            if let Some(back) = plugins.perform(plugin, request) {
                *notes = back.iter().map(|n| from_played(ti, n)).collect();
            }
        }
    }
    timeline.notes = by_track.into_iter().flatten().collect();
    timeline.notes.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track.cmp(&b.track))
            .then(a.key.cmp(&b.key))
    });
}

fn to_played(n: &crate::events::NoteEvent) -> PlayedNote {
    use crate::events::{Attack, HarmonicKind, Sound, Vibrato};
    use oxiguitar_plugin_api::realtime::{attack, flags, harmonic, sound};
    let index = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    let (sound, harmonic) = match n.sound {
        Sound::Normal => (sound::NORMAL, 0),
        Sound::Dead => (sound::DEAD, 0),
        Sound::PalmMute => (sound::PALM_MUTE, 0),
        Sound::Harmonic(k) => (
            sound::HARMONIC,
            match k {
                HarmonicKind::Natural => harmonic::NATURAL,
                HarmonicKind::Artificial => harmonic::ARTIFICIAL,
                HarmonicKind::Tapped => harmonic::TAPPED,
                HarmonicKind::Pinch => harmonic::PINCH,
                HarmonicKind::Semi => harmonic::SEMI,
                HarmonicKind::Feedback => harmonic::FEEDBACK,
            },
        ),
    };
    let mut f = 0;
    for (on, bit) in [
        (n.ghost, flags::GHOST),
        (n.let_ring, flags::LET_RING),
        (n.fade_in, flags::FADE_IN),
        (n.fade_out, flags::FADE_OUT),
        (n.grace, flags::GRACE),
        (n.vibrato == Vibrato::Normal, flags::VIBRATO),
        (n.vibrato == Vibrato::Wide, flags::WIDE_VIBRATO),
    ] {
        if on {
            f |= bit;
        }
    }
    PlayedNote {
        staff: index(n.staff),
        voice: index(n.voice),
        visit: index(n.visit),
        beat: index(n.beat),
        string: n.string.unwrap_or(0),
        key: n.key,
        start: n.start,
        end: n.end.max(n.start),
        velocity: n.velocity,
        sound,
        harmonic,
        attack: match n.attack {
            Attack::Picked => attack::PICKED,
            Attack::Legato => attack::LEGATO,
            Attack::Tapped => attack::TAPPED,
            Attack::Slapped => attack::SLAPPED,
            Attack::Popped => attack::POPPED,
        },
        flags: f,
        pitch: n.pitch.iter().map(|p| (p.time, p.cents)).collect(),
    }
}

fn from_played(track: usize, n: &PlayedNote) -> crate::events::NoteEvent {
    use crate::events::{Attack, HarmonicKind, NoteEvent, PitchPoint, Sound, Vibrato};
    use oxiguitar_plugin_api::realtime::{attack, flags, harmonic, sound};
    let on = |bit: u16| n.flags & bit != 0;
    NoteEvent {
        track,
        staff: n.staff as usize,
        voice: n.voice as usize,
        visit: n.visit as usize,
        beat: n.beat as usize,
        string: (n.string != 0).then_some(n.string),
        key: n.key,
        start: n.start,
        end: n.end,
        velocity: n.velocity,
        sound: match n.sound {
            sound::DEAD => Sound::Dead,
            sound::PALM_MUTE => Sound::PalmMute,
            sound::HARMONIC => Sound::Harmonic(match n.harmonic {
                harmonic::ARTIFICIAL => HarmonicKind::Artificial,
                harmonic::TAPPED => HarmonicKind::Tapped,
                harmonic::PINCH => HarmonicKind::Pinch,
                harmonic::SEMI => HarmonicKind::Semi,
                harmonic::FEEDBACK => HarmonicKind::Feedback,
                _ => HarmonicKind::Natural,
            }),
            _ => Sound::Normal,
        },
        attack: match n.attack {
            attack::LEGATO => Attack::Legato,
            attack::TAPPED => Attack::Tapped,
            attack::SLAPPED => Attack::Slapped,
            attack::POPPED => Attack::Popped,
            _ => Attack::Picked,
        },
        ghost: on(flags::GHOST),
        let_ring: on(flags::LET_RING),
        vibrato: if on(flags::WIDE_VIBRATO) {
            Vibrato::Wide
        } else if on(flags::VIBRATO) {
            Vibrato::Normal
        } else {
            Vibrato::None
        },
        fade_in: on(flags::FADE_IN),
        fade_out: on(flags::FADE_OUT),
        grace: on(flags::GRACE),
        pitch: n
            .pitch
            .iter()
            .map(|&(time, cents)| PitchPoint { time, cents })
            .collect(),
    }
}
