// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The timeline as the audio thread reads it: every note in the plugins'
//! own form ([`RtNote`]), sorted by start and by end, each routed to the
//! source (an instrument instance) that plays its track, with the
//! metronome's clicks and the count-in. Built off the audio thread, when a
//! file opens or after edits (never while playing: Mateus, 25 Oct 2026),
//! and handed to the engine whole; the audio thread only reads it.

use oxiguitar_plugin_api::realtime::{PitchPoint, RtNote, attack, flags, harmonic, sound};

use crate::events::{Attack, HarmonicKind, Sound, Vibrato};
use crate::{Click, Timeline};

/// Where a track's notes go: the source that plays it and the track's
/// place among that source's tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    /// The source's index in the engine's sources.
    pub source: usize,
    /// The track's place among the source's tracks: its pair of buffers.
    pub slot: usize,
}

/// A note routed to its source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Routed {
    pub(crate) note: RtNote,
    pub(crate) source: usize,
}

/// The timeline, ready for the audio thread.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    /// Every routed note, by start (then by id).
    pub(crate) notes: Vec<Routed>,
    /// Indices into `notes`, by end (then by id).
    pub(crate) ends: Vec<u32>,
    /// Every note's pitch curve, one after another; a note's points are
    /// `pitch[pitch_start..pitch_start + pitch_len]`.
    pub(crate) pitch: Vec<PitchPoint>,
    /// The metronome's clicks from the song's start.
    pub(crate) clicks: Vec<Click>,
    /// The count-in's clicks from the count-in's start, and its length.
    pub(crate) count_in: Vec<Click>,
    pub(crate) count_in_length: f64,
    /// How long the song plays, in seconds at 100 % speed.
    pub(crate) length: f64,
    /// The most events each source gets at one instant: its event list's
    /// size ([`crate::Sources::fit`]), so the audio thread never grows one.
    pub(crate) most_at_once: Vec<usize>,
}

impl Schedule {
    /// The schedule of `timeline`, each track routed by `routes` (one per
    /// track of the score; `None` for a track nothing plays), for `sources`
    /// sources.
    pub fn new(timeline: &Timeline, routes: &[Option<Route>], sources: usize) -> Schedule {
        let mut pitch = Vec::new();
        let mut notes: Vec<Routed> = timeline
            .notes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let route = routes.get(n.track).copied().flatten()?;
                if route.source >= sources {
                    return None;
                }
                let start = u32::try_from(pitch.len()).unwrap_or(u32::MAX);
                pitch.extend(n.pitch.iter().map(|p| PitchPoint {
                    time: p.time,
                    cents: p.cents,
                }));
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
                Some(Routed {
                    note: RtNote {
                        id: u32::try_from(i).unwrap_or(u32::MAX),
                        track: u32::try_from(route.slot).unwrap_or(u32::MAX),
                        key: n.key,
                        velocity: n.velocity,
                        string: n.string.unwrap_or(0),
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
                        start: n.start,
                        end: n.end.max(n.start),
                        pitch_start: start,
                        pitch_len: u32::try_from(n.pitch.len()).unwrap_or(0),
                    },
                    source: route.source,
                })
            })
            .collect();
        notes.sort_by(|a, b| {
            a.note
                .start
                .total_cmp(&b.note.start)
                .then(a.note.id.cmp(&b.note.id))
        });
        let mut ends: Vec<u32> = (0..notes.len() as u32).collect();
        ends.sort_by(|&a, &b| {
            let (a, b) = (&notes[a as usize].note, &notes[b as usize].note);
            a.end.total_cmp(&b.end).then(a.id.cmp(&b.id))
        });
        let most_at_once = most_at_once(&notes, &ends, sources);
        Schedule {
            notes,
            ends,
            pitch,
            clicks: timeline.clicks.clone(),
            count_in: timeline.count_in.clone(),
            count_in_length: timeline.count_in_length,
            length: timeline.length(),
            most_at_once,
        }
    }

    /// How long the song plays, in seconds at 100 % speed.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// The number of notes routed to a source.
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    /// The first note starting at or after `time`.
    pub(crate) fn first_start_from(&self, time: f64) -> usize {
        self.notes.partition_point(|n| n.note.start < time)
    }

    /// The first note (in end order) ending after `time`.
    pub(crate) fn first_end_after(&self, time: f64) -> usize {
        self.ends
            .partition_point(|&i| self.notes[i as usize].note.end <= time)
    }
}

/// For each source, the most events it can get at one instant: the notes
/// starting then, plus the notes ending then, plus one for "all off".
fn most_at_once(notes: &[Routed], ends: &[u32], sources: usize) -> Vec<usize> {
    let mut most = vec![1; sources];
    let mut count = vec![0usize; sources];
    // Every instant something starts or ends, with the events of each kind.
    let mut instants: Vec<(f64, usize)> = notes
        .iter()
        .map(|n| (n.note.start, n.source))
        .chain(ends.iter().map(|&i| {
            let n = &notes[i as usize];
            (n.note.end, n.source)
        }))
        .collect();
    instants.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut i = 0;
    while i < instants.len() {
        let t = instants[i].0;
        count.iter_mut().for_each(|c| *c = 0);
        while i < instants.len() && instants[i].0 == t {
            count[instants[i].1] += 1;
            i += 1;
        }
        for (m, c) in most.iter_mut().zip(&count) {
            *m = (*m).max(c + 1);
        }
    }
    most
}
