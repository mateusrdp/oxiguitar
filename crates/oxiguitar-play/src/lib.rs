// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's player core (M4; docs/design/player.md).
//!
//! This crate turns a score into its **timeline**: the bars in the order
//! they are played (repeats, endings, D.C., D.S., codas; [`order`]), when
//! each starts and ends in seconds (tempo, gradual tempo changes, feel;
//! [`time`]), every note as an event with its exact start and end and what
//! it means for the sound (every TAB-1 effect; [`events`]), and the
//! metronome's clicks and the count-in ([`click`]). The timeline also maps
//! a time back to the bar and the position being heard, for the cursor
//! ([`Timeline::find`]).
//!
//! Times are in seconds at 100 % speed; the transport plays faster or
//! slower by scaling them, never by changing pitch. The sound itself comes
//! from instrument plugins and the audio device from a device plugin: this
//! crate has no audio code and no window code.

pub mod click;
pub mod events;
pub mod order;
pub mod text;
pub mod time;

use oxiguitar_model::Score;

pub use click::Click;
pub use events::{Attack, HarmonicKind, NoteEvent, PitchPoint, Sound, Vibrato};
pub use order::{Visit, playing_order};
pub use time::{TimeMap, VisitTime};

/// A score as it plays: the bars in order with their times, the notes, the
/// metronome and the count-in.
#[derive(Clone, Debug)]
pub struct Timeline {
    /// When each bar plays, in playing order.
    pub map: TimeMap,
    /// Every note, by start time.
    pub notes: Vec<NoteEvent>,
    /// The metronome's clicks, from the song's start.
    pub clicks: Vec<Click>,
    /// The count-in's clicks, from the count-in's start.
    pub count_in: Vec<Click>,
    /// How long the count-in lasts, in seconds.
    pub count_in_length: f64,
}

impl Timeline {
    /// The timeline of `score`.
    pub fn new(score: &Score) -> Timeline {
        let order = playing_order(&score.master_bars);
        let map = TimeMap::new(score, &order);
        let notes = events::notes(score, &map);
        let clicks = click::metronome(score, &map);
        let (count_in, count_in_length) = click::count_in(score, &map);
        Timeline {
            map,
            notes,
            clicks,
            count_in,
            count_in_length,
        }
    }

    /// How long the song plays, in seconds at 100 % speed.
    pub fn length(&self) -> f64 {
        self.map.length()
    }

    /// The played bars.
    pub fn visits(&self) -> &[VisitTime] {
        &self.map.visits
    }

    /// Where the music is at `time` seconds: the index of the played bar
    /// and the position in it, in whole notes as played. `None` outside
    /// the song.
    pub fn find(&self, time: f64) -> Option<(usize, f64)> {
        self.map.find(time)
    }
}
