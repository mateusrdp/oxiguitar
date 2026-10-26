// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The playback cursor (M4 slice 4), without window code: where the sound
//! is, drawn on the page as a line moving with it through the bar and the
//! beat being heard lightly boxed (Mateus, 26 Oct 2026), and when the page
//! should turn to keep it in sight.
//!
//! The line goes from a beat's column to the next in proportion to the
//! time between them, so it moves as the music does even where the
//! engraver spaces beats unevenly; the beat box is the column of the
//! latest beat started, across the system's staves.

use oxiguitar_display::Rect;
use oxiguitar_edit::onsets;
use oxiguitar_layout::Hits;
use oxiguitar_model::Score;

/// Where the cursor is drawn, on one sheet, in its points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cursor {
    pub(crate) sheet: usize,
    /// The line: its x, and the system's top and bottom.
    pub(crate) x: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    /// The beat being heard, over the system's staves.
    pub(crate) beat: Rect,
}

/// The cursor at `offset` whole notes into bar `bar` (as played) of a bar
/// `length` whole notes long, on the page `hits` maps. `None` when the bar
/// isn't shown.
pub(crate) fn at(
    hits: &Hits,
    score: &Score,
    bar: usize,
    offset: f64,
    length: f64,
) -> Option<Cursor> {
    let system = hits.systems.iter().find(|s| {
        s.staves
            .iter()
            .any(|st| st.bars.iter().any(|b| b.bar == bar))
    })?;
    let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut start, mut end) = (f32::INFINITY, f32::NEG_INFINITY);
    // Each beat's start in the bar and where its column is, every staff.
    let mut points: Vec<(f64, f32, f32)> = Vec::new();
    for st in &system.staves {
        let Some(b) = st.bars.iter().find(|b| b.bar == bar) else {
            continue;
        };
        top = top.min(st.top - 6.0);
        bottom = bottom.max(st.bottom + 6.0);
        start = start.min(b.notes);
        end = end.max(b.end);
        let bar_of = score
            .tracks
            .get(st.track)
            .and_then(|t| t.staves.get(st.staff))
            .and_then(|s| s.bars.get(bar));
        let Some(model) = bar_of else {
            continue;
        };
        for (v, voice) in model.voices.iter().enumerate() {
            let starts = onsets(voice);
            for h in b.beats.iter().filter(|h| h.voice == v) {
                let grace = voice.beats.get(h.beat).is_some_and(|x| x.grace.is_some());
                if let (false, Some(o)) = (grace, starts.get(h.beat)) {
                    points.push((o.to_f64(), h.x, h.width));
                }
            }
        }
    }
    if !start.is_finite() {
        return None;
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    // One column per start (the staves line up; the leftmost is kept).
    points.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-9);
    // The line, between the column before and the one after (the bar's
    // first note's place at its start, its bar line at its end).
    let before = points
        .iter()
        .rev()
        .find(|p| p.0 <= offset)
        .map_or((0.0, start), |p| (p.0, p.1));
    let after = points
        .iter()
        .find(|p| p.0 > offset)
        .map_or((length.max(before.0), end - 2.0), |p| (p.0, p.1));
    let span = after.0 - before.0;
    let w = if span > 0.0 {
        ((offset - before.0) / span).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let x = before.1 + (after.1 - before.1) * w;
    // The beat heard: the latest started, as the caret's column.
    let beat = match points.iter().rev().find(|p| p.0 <= offset) {
        Some(&(_, bx, bw)) => {
            let half = (bw * 0.5).clamp(4.0, 12.0);
            Rect::new(bx - half, top, 2.0 * half, bottom - top)
        }
        None => Rect::new(x - 4.0, top, 8.0, bottom - top),
    };
    Some(Cursor {
        sheet: system.sheet,
        x,
        top,
        bottom,
        beat,
    })
}

/// Where the view goes to keep the cursor in sight, as a page turns: the
/// view at `view` (x, y, width, height), the cursor's line from `(x, top)`
/// to `(x, bottom)`, all in the canvas's pixels. Turns only when the line
/// nears an edge or leaves the view, then puts it near the left (its
/// system near the top), so the music ahead shows.
pub(crate) fn follow(
    view: (f32, f32, f32, f32),
    x: f32,
    top: f32,
    bottom: f32,
) -> Option<(f32, f32)> {
    let (vx, vy, vw, vh) = view;
    let margin = 32.0;
    let nx = if x < vx + margin || x > vx + vw - margin {
        (x - (vw * 0.15).max(margin)).max(0.0)
    } else {
        vx
    };
    let ny = if top < vy || bottom > vy + vh {
        (top - margin).max(0.0)
    } else {
        vy
    };
    ((nx - vx).abs() > 0.5 || (ny - vy).abs() > 0.5).then_some((nx, ny))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_layout::{BarHits, BeatHit, StaffHits, SystemHits};
    use oxiguitar_model::{
        Bar, Beat, Clef, Duration, MasterBar, Note, NoteValue, Staff, Track, Voice,
    };

    /// Two bars: a half note and two quarters, then four quarters.
    fn score() -> Score {
        let beat = |v| Beat {
            duration: Duration::new(v),
            notes: vec![Note::fretted(1, 0)],
            ..Beat::default()
        };
        let bars = vec![
            Bar {
                voices: vec![Voice {
                    beats: vec![
                        beat(NoteValue::Half),
                        beat(NoteValue::Quarter),
                        beat(NoteValue::Quarter),
                    ],
                }],
                ..Bar::default()
            },
            Bar {
                voices: vec![Voice {
                    beats: vec![beat(NoteValue::Quarter); 4],
                }],
                ..Bar::default()
            },
        ];
        Score {
            master_bars: vec![MasterBar::default(); 2],
            tracks: vec![Track {
                staves: vec![Staff {
                    tuning: Staff::standard_guitar_tuning(),
                    bars,
                    ..Staff::default()
                }],
                ..Track::default()
            }],
            ..Score::new()
        }
    }

    /// The half note at x 110, the quarters at 160 and 180 (unevenly
    /// spaced, as an engraver may), the bar from 100 to 200; the second
    /// bar on the next system.
    fn hits() -> Hits {
        let staff = |bar: usize, xs: &[f32], top: f32| StaffHits {
            track: 0,
            staff: 0,
            standard: None,
            tablature: Some((top, 6.0, 6)),
            top,
            bottom: top + 30.0,
            lyrics: (top + 45.0, 12.0),
            bars: vec![BarHits {
                bar,
                start: 100.0,
                notes: 105.0,
                end: 200.0,
                clef: Clef::Treble8vb,
                time_signature: None,
                beats: xs
                    .iter()
                    .enumerate()
                    .map(|(beat, &x)| BeatHit {
                        voice: 0,
                        beat,
                        x,
                        width: 10.0,
                    })
                    .collect(),
            }],
        };
        Hits {
            systems: vec![
                SystemHits {
                    sheet: 0,
                    top: 90.0,
                    bottom: 140.0,
                    staves: vec![staff(0, &[110.0, 160.0, 180.0], 100.0)],
                },
                SystemHits {
                    sheet: 0,
                    top: 190.0,
                    bottom: 240.0,
                    staves: vec![staff(1, &[110.0, 130.0, 150.0, 170.0], 200.0)],
                },
            ],
        }
    }

    #[test]
    fn the_line_moves_with_the_time_between_columns() {
        let (h, s) = (hits(), score());
        let x = |offset: f64| at(&h, &s, 0, offset, 1.0).unwrap().x;
        assert_eq!(x(0.0), 110.0);
        // Half way through the half note: half way to the next column,
        // though the engraver put it far away.
        assert!((x(0.25) - 135.0).abs() < 1e-3, "{}", x(0.25));
        assert_eq!(x(0.5), 160.0);
        assert!((x(0.625) - 170.0).abs() < 1e-3);
        // After the last beat: on to the bar line.
        assert!(x(0.875) > 180.0 && x(0.875) < 200.0);
    }

    #[test]
    fn the_beat_heard_is_boxed_across_its_system() {
        let (h, s) = (hits(), score());
        let c = at(&h, &s, 0, 0.6, 1.0).unwrap();
        assert_eq!(c.beat.x, 155.0, "the quarter at 160 is sounding");
        assert_eq!((c.top, c.bottom), (94.0, 136.0));
        assert_eq!(c.beat.y, c.top);
        // The next bar is on the next system.
        let c = at(&h, &s, 1, 0.0, 1.0).unwrap();
        assert_eq!(c.top, 194.0);
        assert!(at(&h, &s, 5, 0.0, 1.0).is_none(), "a bar not shown");
    }

    #[test]
    fn the_page_turns_only_near_an_edge() {
        let view = (0.0, 0.0, 800.0, 600.0);
        assert_eq!(follow(view, 400.0, 100.0, 200.0), None);
        // Near the right edge: the line goes near the left.
        let (nx, ny) = follow(view, 790.0, 100.0, 200.0).unwrap();
        assert!((nx - 670.0).abs() < 1e-3 && ny == 0.0, "{nx} {ny}");
        // Its system below the view: it goes to the top.
        let (nx, ny) = follow(view, 400.0, 700.0, 800.0).unwrap();
        assert!(nx == 0.0 && (ny - 668.0).abs() < 1e-3, "{nx} {ny}");
    }
}
