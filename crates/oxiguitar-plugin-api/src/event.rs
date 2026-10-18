// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Events the core pushes to plugins (interface 1.1), as they happen.
//!
//! A plugin lists the kinds it wants under `events` in its manifest
//! ([`EventKind`]); the host sends it each such [`Event`]
//! ([`crate::message::Request::Event`]), loading the plugin the first time
//! one fires. Nothing polls: a plugin that wants to follow the score, the
//! selection or the playback cursor is told, and asks for nothing until
//! then.
//!
//! Score events carry no score: a plugin that needs it asks the host
//! ([`crate::message::HostRequest::Score`]), so the score is converted and
//! sent only to plugins that use it, when they use it.

use serde::{Deserialize, Serialize};

/// What a plugin can subscribe to, by its manifest's `events` list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// A score was opened, changed or closed.
    Score,
    /// The selection changed.
    Selection,
    /// Playback started, stopped or moved.
    Playback,
}

/// Something that happened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A score was opened; it replaces the one before, if any.
    ScoreOpened {
        /// Its revision: a number that changes with every new or changed
        /// score in this session.
        revision: u64,
    },
    /// The open score changed (an edit, an undo, a tool).
    ScoreChanged {
        /// Its new revision.
        revision: u64,
    },
    /// No score is open any more.
    ScoreClosed,
    /// The selection changed; `None` when nothing is selected.
    Selection(Option<Selection>),
    /// Playback started, stopped, or the cursor reached another beat.
    Playback(Playback),
}

impl Event {
    /// The kind a plugin subscribes to for this event.
    pub fn kind(&self) -> EventKind {
        match self {
            Event::ScoreOpened { .. } | Event::ScoreChanged { .. } | Event::ScoreClosed => {
                EventKind::Score
            }
            Event::Selection(_) => EventKind::Selection,
            Event::Playback(_) => EventKind::Playback,
        }
    }
}

/// A place in a track: a beat of a bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    /// The bar, from 0.
    pub bar: u32,
    /// The beat in the bar's first voice, from 0.
    pub beat: u32,
}

/// The selection: a run of beats in one track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selection {
    /// The track, from 0.
    pub track: u32,
    /// The first beat selected.
    pub from: Position,
    /// The last beat selected (the same as `from` for one beat).
    pub to: Position,
}

/// Where playback is. Sent when playback starts or stops, when the cursor
/// reaches another beat, and when it jumps; a plugin that draws in between
/// works from `seconds` and `playing`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    /// Whether it is playing.
    pub playing: bool,
    /// The beat the cursor is on.
    pub at: Position,
    /// Time from the start of the score, in seconds, as played (repeats and
    /// tempo included).
    pub seconds: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode};

    #[test]
    fn events_round_trip_and_positions_are_frozen() {
        let events = [
            Event::ScoreOpened { revision: 1 },
            Event::ScoreChanged { revision: 2 },
            Event::ScoreClosed,
            Event::Selection(Some(Selection {
                track: 1,
                from: Position { bar: 2, beat: 0 },
                to: Position { bar: 3, beat: 4 },
            })),
            Event::Playback(Playback {
                playing: true,
                at: Position { bar: 7, beat: 1 },
                seconds: 12.5,
            }),
        ];
        for (i, e) in events.iter().enumerate() {
            let bytes = encode(e).unwrap();
            assert_eq!(bytes[0] as usize, i);
            assert_eq!(&decode::<Event>(&bytes).unwrap(), e);
        }
        let kinds: Vec<EventKind> = events.iter().map(Event::kind).collect();
        assert_eq!(
            kinds,
            [
                EventKind::Score,
                EventKind::Score,
                EventKind::Score,
                EventKind::Selection,
                EventKind::Playback
            ]
        );
        assert_eq!(
            serde_json::from_str::<Vec<EventKind>>(r#"["score", "playback"]"#).unwrap(),
            [EventKind::Score, EventKind::Playback]
        );
    }
}
