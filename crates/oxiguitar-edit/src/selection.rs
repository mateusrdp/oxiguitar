// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Selections: what Shift and the arrows, Shift + click and Select All
//! pick.
//!
//! Inside one voice of one staff a selection is a run of beats, which may
//! cross bar lines. Once it reaches another staff or track the beats no
//! longer line up, so it becomes whole bars across those tracks, as in
//! Guitar Pro.

use oxiguitar_model::Score;

use crate::{Caret, StaffBar};

/// What is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selection {
    /// A run of beats in one voice of one staff, `from` and `to` included,
    /// each as (bar, beat), `from` first.
    Beats {
        /// The track.
        track: usize,
        /// The staff in the track.
        staff: usize,
        /// The voice.
        voice: usize,
        /// The first beat.
        from: (usize, usize),
        /// The last beat.
        to: (usize, usize),
    },
    /// Whole bars of every staff of tracks `tracks.0` to `tracks.1`, bars
    /// `bars.0` to `bars.1`, all included.
    Bars {
        /// The first and last track.
        tracks: (usize, usize),
        /// The first and last bar.
        bars: (usize, usize),
    },
}

/// The part of one bar of one staff a selection covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// The bar.
    pub at: StaffBar,
    /// The voice and the first and last beat covered, or `None` for the
    /// whole bar.
    pub beats: Option<(usize, usize, usize)>,
}

impl Selection {
    /// From `anchor` (where Shift was first held) to `head` (the caret).
    pub fn between(anchor: Caret, head: Caret) -> Selection {
        if (anchor.track, anchor.staff, anchor.voice) == (head.track, head.staff, head.voice) {
            let (a, b) = ((anchor.bar, anchor.beat), (head.bar, head.beat));
            Selection::Beats {
                track: head.track,
                staff: head.staff,
                voice: head.voice,
                from: a.min(b),
                to: a.max(b),
            }
        } else {
            Selection::Bars {
                tracks: (anchor.track.min(head.track), anchor.track.max(head.track)),
                bars: (anchor.bar.min(head.bar), anchor.bar.max(head.bar)),
            }
        }
    }

    /// Every bar of every track (Select All). `None` for an empty score.
    pub fn all(score: &Score) -> Option<Selection> {
        let tracks = score.tracks.len().checked_sub(1)?;
        let bars = score.bar_count().checked_sub(1)?;
        Some(Selection::Bars {
            tracks: (0, tracks),
            bars: (0, bars),
        })
    }

    /// The first and last track selected.
    pub fn tracks(&self) -> (usize, usize) {
        match *self {
            Selection::Beats { track, .. } => (track, track),
            Selection::Bars { tracks, .. } => tracks,
        }
    }

    /// The first and last bar selected.
    pub fn bars(&self) -> (usize, usize) {
        match *self {
            Selection::Beats { from, to, .. } => (from.0, to.0),
            Selection::Bars { bars, .. } => bars,
        }
    }

    /// What the selection covers, bar by bar of each staff of `score`.
    pub fn spans(&self, score: &Score) -> Vec<Span> {
        match *self {
            Selection::Beats {
                track,
                staff,
                voice,
                from,
                to,
            } => (from.0..=to.0)
                .map(|bar| {
                    let last = score
                        .tracks
                        .get(track)
                        .and_then(|t| t.staves.get(staff))
                        .and_then(|s| s.bars.get(bar))
                        .and_then(|b| b.voices.get(voice))
                        .map_or(0, |v| v.beats.len().saturating_sub(1));
                    let first = if bar == from.0 { from.1 } else { 0 };
                    let end = if bar == to.0 { to.1.min(last) } else { last };
                    Span {
                        at: StaffBar { track, staff, bar },
                        beats: Some((voice, first, end.max(first))),
                    }
                })
                .collect(),
            Selection::Bars { tracks, bars } => (tracks.0..=tracks.1)
                .flat_map(|track| {
                    let staves = score.tracks.get(track).map_or(0, |t| t.staves.len());
                    (0..staves).flat_map(move |staff| {
                        (bars.0..=bars.1).map(move |bar| Span {
                            at: StaffBar { track, staff, bar },
                            beats: None,
                        })
                    })
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caret::tests::score;

    #[test]
    fn beats_in_one_voice_bars_across_tracks() {
        let s = score(3, 4);
        let a = Caret {
            bar: 2,
            beat: 1,
            ..Caret::at_start(&s, 0).unwrap()
        };
        let h = Caret {
            bar: 0,
            beat: 3,
            ..a
        };
        let sel = Selection::between(a, h);
        assert_eq!(
            sel,
            Selection::Beats {
                track: 0,
                staff: 0,
                voice: 0,
                from: (0, 3),
                to: (2, 1)
            }
        );
        let spans = sel.spans(&s);
        let beats: Vec<_> = spans.iter().map(|s| s.beats).collect();
        assert_eq!(
            beats,
            [Some((0, 3, 3)), Some((0, 0, 3)), Some((0, 0, 1))],
            "the middle bar whole, the ends from and to the caret"
        );
        let other = Caret { track: 2, ..h };
        let sel = Selection::between(a, other);
        assert_eq!(
            sel,
            Selection::Bars {
                tracks: (0, 2),
                bars: (0, 2)
            }
        );
        assert_eq!(sel.spans(&s).len(), 9);
        assert_eq!(sel.bars(), (0, 2));
        assert_eq!(sel.tracks(), (0, 2));
        // Another voice of the same staff is not the same run of beats.
        assert!(matches!(
            Selection::between(a, Caret { voice: 1, ..a }),
            Selection::Bars { .. }
        ));
    }

    #[test]
    fn select_all() {
        let s = score(2, 5);
        assert_eq!(
            Selection::all(&s),
            Some(Selection::Bars {
                tracks: (0, 1),
                bars: (0, 4)
            })
        );
        assert_eq!(Selection::all(&Score::new()), None);
    }
}
