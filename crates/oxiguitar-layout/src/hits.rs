// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Where things are on the engraved sheets, for the editor (M3): every
//! shown staff of every system, its bars, the beats of each voice and the
//! tablature's strings, in the sheet's points.
//!
//! The window uses it both ways: a click becomes a place in the score
//! ([`Hits::hit`]), and the caret's place in the score becomes a box to
//! draw over the page ([`Hits::find`]), with no tile redrawn. It is made by
//! the same pass that draws, so it can't disagree with the picture.

use oxiguitar_display::Rect;
use oxiguitar_model::Clef;

/// Every system's map, in sheet order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hits {
    /// The systems, in order.
    pub systems: Vec<SystemHits>,
}

/// One system (line of music).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemHits {
    /// The sheet it is on (a page in the Page layout; always 0 in the
    /// others).
    pub sheet: usize,
    /// Its top and bottom on the sheet.
    pub top: f32,
    /// See `top`.
    pub bottom: f32,
    /// Its staves, top to bottom.
    pub staves: Vec<StaffHits>,
}

/// One staff of one track as shown in a system. A keyboard's grand staff
/// shows one staff of the model as two or three of these.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaffHits {
    /// The track, from 0.
    pub track: usize,
    /// The staff in the track, from 0.
    pub staff: usize,
    /// The standard staff, if shown: its top line and its staff space.
    pub standard: Option<(f32, f32)>,
    /// The tablature, if shown: the y of string 1 (the top line), the
    /// distance between strings, and how many strings.
    pub tablature: Option<(f32, f32, u8)>,
    /// The top and bottom of its staves (the standard staff's top line to
    /// the tablature's bottom line).
    pub top: f32,
    /// See `top`.
    pub bottom: f32,
    /// Its bars, left to right.
    pub bars: Vec<BarHits>,
}

/// One bar of one staff in a system.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarHits {
    /// The bar, from 0.
    pub bar: usize,
    /// The bar's left edge (the bar line before it).
    pub start: f32,
    /// Where its notes start, after a clef, signature or repeat sign.
    pub notes: f32,
    /// Its bar line.
    pub end: f32,
    /// The clef this view of the bar is written in (a grand staff's hands
    /// have their own).
    pub clef: Clef,
    /// Where its time signature is drawn, left to right, if it shows one.
    pub time_signature: Option<(f32, f32)>,
    /// Its beats, every voice, as drawn.
    pub beats: Vec<BeatHit>,
}

/// One beat: where its notes are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BeatHit {
    /// The voice, from 0.
    pub voice: usize,
    /// The beat in the voice, from 0 (the model's index).
    pub beat: usize,
    /// The centre of its notes.
    pub x: f32,
    /// How far its room goes right of `x`: to the next column or the bar
    /// line.
    pub width: f32,
}

/// A place on the page, as the score names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    /// The track.
    pub track: usize,
    /// The staff in the track.
    pub staff: usize,
    /// The bar.
    pub bar: usize,
    /// The beat of the voice asked for, nearest the point; `None` when that
    /// voice has no beat in the bar.
    pub beat: Option<usize>,
    /// The string nearest the point, if it is on the tablature.
    pub string: Option<u8>,
    /// The line or space nearest the point, if it is on (or near) the
    /// standard staff: 0 the bottom line, 8 the top line, with its clef.
    pub step: Option<(i32, Clef)>,
}

/// Where a beat is drawn, for the caret.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    /// The sheet.
    pub sheet: usize,
    /// The beat's column over its staves, a little wider than the notes.
    pub column: Rect,
    /// The string's place on the tablature, a box around where its fret
    /// number goes; `None` without tablature.
    pub string: Option<Rect>,
    /// The line or space asked for on the standard staff, a box where its
    /// note head goes; `None` when none was asked for or none is shown.
    pub step: Option<Rect>,
}

impl BarHits {
    /// The beats of `voice`, in order.
    fn voice(&self, voice: usize) -> impl Iterator<Item = &BeatHit> {
        self.beats.iter().filter(move |b| b.voice == voice)
    }
}

impl StaffHits {
    /// The string at height `y`, if `y` is on (or just beside) the
    /// tablature.
    fn string_at(&self, y: f32) -> Option<u8> {
        let (top, gap, strings) = self.tablature?;
        if strings == 0 || gap <= 0.0 {
            return None;
        }
        let bottom = top + gap * f32::from(strings - 1);
        if y < top - gap || y > bottom + gap {
            return None;
        }
        let n = ((y - top) / gap).round().clamp(0.0, f32::from(strings - 1));
        Some(n as u8 + 1)
    }

    /// The line or space of the standard staff at height `y`, if `y` is on
    /// it or within three spaces of it: 0 the bottom line, 8 the top line.
    fn step_at(&self, y: f32) -> Option<i32> {
        let (top, sp) = self.standard?;
        if sp <= 0.0 {
            return None;
        }
        let bottom = top + 4.0 * sp;
        if y < top - 3.0 * sp || y > bottom + 3.0 * sp {
            return None;
        }
        Some(((bottom - y) / (sp / 2.0)).round() as i32)
    }

    /// How far `y` is from this staff's lines, 0 inside them.
    fn distance(&self, y: f32) -> f32 {
        if y < self.top {
            self.top - y
        } else if y > self.bottom {
            y - self.bottom
        } else {
            0.0
        }
    }
}

impl Hits {
    /// The place under the point (`x`, `y`) of sheet `sheet`: the nearest
    /// staff of the system there, the bar there, the beat of `voice`
    /// nearest `x` and the string nearest `y`. `None` off the music (in the
    /// header, between pages, in a margin beside the systems).
    pub fn hit(&self, sheet: usize, x: f32, y: f32, voice: usize) -> Option<Spot> {
        // The system the point is in, or the nearest one on the sheet when
        // it falls in the gap between two.
        let system = self
            .systems
            .iter()
            .filter(|s| s.sheet == sheet)
            .min_by(|a, b| gap(a.top, a.bottom, y).total_cmp(&gap(b.top, b.bottom, y)))?;
        if gap(system.top, system.bottom, y) > 40.0 {
            return None;
        }
        let staff = system
            .staves
            .iter()
            .min_by(|a, b| a.distance(y).total_cmp(&b.distance(y)))?;
        let bar = staff.bars.iter().find(|b| x >= b.start && x < b.end)?;
        // Past the last beat's column: the place after it, where typing
        // adds a beat (the caret keeps to the last beat when the bar is
        // full).
        let last = bar.voice(voice).last();
        let beat = match last {
            Some(l) if x > l.x + l.width.max(8.0) * 0.5 => Some(l.beat + 1),
            _ => bar
                .voice(voice)
                .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
                .map(|b| b.beat),
        };
        let string = staff.string_at(y);
        Some(Spot {
            track: staff.track,
            staff: staff.staff,
            bar: bar.bar,
            beat,
            string,
            step: string
                .is_none()
                .then(|| staff.step_at(y))
                .flatten()
                .map(|s| (s, bar.clef)),
        })
    }

    /// The bar whose time signature is drawn at (`x`, `y`) on sheet
    /// `sheet`, with its track and staff: what a double-click there edits.
    pub fn time_signature_at(&self, sheet: usize, x: f32, y: f32) -> Option<Spot> {
        let system = self
            .systems
            .iter()
            .find(|s| s.sheet == sheet && y >= s.top && y <= s.bottom)?;
        system.staves.iter().find_map(|staff| {
            if y < staff.top - 4.0 || y > staff.bottom + 4.0 {
                return None;
            }
            let bar = staff.bars.iter().find(|b| {
                b.time_signature
                    .is_some_and(|(l, r)| x >= l - 3.0 && x <= r + 3.0)
            })?;
            Some(Spot {
                track: staff.track,
                staff: staff.staff,
                bar: bar.bar,
                beat: None,
                string: None,
                step: None,
            })
        })
    }

    /// Where beat `beat` of `voice` in bar `bar` of `track`'s staff `staff`
    /// is drawn, with string `string` marked on the tablature and, when
    /// asked, line or space `step` on the standard staff in that clef (a
    /// grand staff's hand in that clef). In a voice with no beats in the
    /// bar, the column is where its first note would go. `None` if that
    /// staff or bar isn't shown.
    #[allow(clippy::too_many_arguments)]
    pub fn find(
        &self,
        track: usize,
        staff: usize,
        bar: usize,
        voice: usize,
        beat: usize,
        string: u8,
        step: Option<(i32, Clef)>,
    ) -> Option<Found> {
        let mut found: Option<(usize, Rect, &StaffHits, f32, f32)> = None;
        // The standard staff for `step`: its top line and staff space, and
        // whether its clef is the one asked for.
        let mut standard: Option<(f32, f32, bool)> = None;
        for system in &self.systems {
            for s in system
                .staves
                .iter()
                .filter(|s| s.track == track && s.staff == staff)
            {
                let Some(b) = s.bars.iter().find(|b| b.bar == bar) else {
                    continue;
                };
                // A beat the bar doesn't have is the place after the
                // voice's last beat (where typing adds one), or the bar's
                // start in a voice with no beats.
                let (x, w) = b.voice(voice).find(|h| h.beat == beat).map_or_else(
                    || {
                        b.voice(voice)
                            .filter(|h| h.beat < beat)
                            .last()
                            .map_or((b.notes + 4.0, 8.0), |h| {
                                ((h.x + h.width.max(8.0)).min(b.end - 5.0), 8.0)
                            })
                    },
                    |h| (h.x, h.width),
                );
                let half = (w * 0.5).clamp(4.0, 12.0);
                if let (Some((top, sp)), Some((_, clef))) = (s.standard, step) {
                    let fits = b.clef == clef;
                    if standard.is_none_or(|(_, _, f)| !f && fits) {
                        standard = Some((top, sp, fits));
                    }
                }
                // The column covers every view of the staff (a grand
                // staff's hands and its tablature).
                found = Some(match found {
                    Some((sheet, col, first, fx, fw)) => (
                        sheet,
                        Rect::new(col.x, col.y, col.width, s.bottom + 6.0 - col.y),
                        if first.tablature.is_some() { first } else { s },
                        fx,
                        fw,
                    ),
                    None => (
                        system.sheet,
                        Rect::new(x - half, s.top - 6.0, 2.0 * half, s.bottom - s.top + 12.0),
                        s,
                        x,
                        half,
                    ),
                });
            }
            if found.is_some() {
                break;
            }
        }
        let (sheet, column, s, x, half) = found?;
        let string = s.tablature.and_then(|(top, gap, strings)| {
            (strings > 0).then(|| {
                let n = string.clamp(1, strings);
                let y = top + gap * f32::from(n - 1);
                Rect::new(x - half, y - gap * 0.5, 2.0 * half, gap)
            })
        });
        let step = standard.zip(step).map(|((top, sp, _), (n, _))| {
            let y = top + 4.0 * sp - n as f32 * sp / 2.0;
            Rect::new(x - half, y - sp * 0.5, 2.0 * half, sp)
        });
        Some(Found {
            sheet,
            column,
            string,
            step,
        })
    }

    /// The part of bar `bar` of `track`'s staff `staff` that beats `first`
    /// to `last` of `voice` cover, or the whole bar for `None`, on each
    /// sheet it is drawn on: for drawing a selection.
    pub fn span(
        &self,
        track: usize,
        staff: usize,
        bar: usize,
        beats: Option<(usize, usize, usize)>,
    ) -> Option<(usize, Rect)> {
        let mut out: Option<(usize, Rect)> = None;
        for system in &self.systems {
            for s in system
                .staves
                .iter()
                .filter(|s| s.track == track && s.staff == staff)
            {
                let Some(b) = s.bars.iter().find(|b| b.bar == bar) else {
                    continue;
                };
                let (x0, x1) = match beats {
                    None => (b.start, b.end),
                    Some((voice, first, last)) => {
                        let hits: Vec<&BeatHit> = b
                            .voice(voice)
                            .filter(|h| h.beat >= first && h.beat <= last)
                            .collect();
                        let x0 = hits.iter().map(|h| h.x).fold(f32::INFINITY, f32::min);
                        let x1 = hits
                            .iter()
                            .map(|h| h.x + h.width)
                            .fold(f32::NEG_INFINITY, f32::max);
                        if x0.is_finite() && x1.is_finite() {
                            ((x0 - 6.0).max(b.start), x1.min(b.end))
                        } else {
                            (b.notes, b.end)
                        }
                    }
                };
                let r = Rect::new(x0, s.top - 6.0, x1 - x0, s.bottom - s.top + 12.0);
                out = Some(match out {
                    Some((sheet, o)) => (
                        sheet,
                        Rect::new(o.x, o.y, o.width, r.bottom().max(o.bottom()) - o.y),
                    ),
                    None => (system.sheet, r),
                });
            }
            if out.is_some() {
                break;
            }
        }
        out
    }

    /// The tracks shown, in order from the top.
    pub fn tracks(&self) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for s in self.systems.iter().flat_map(|s| &s.staves) {
            if !out.contains(&s.track) {
                out.push(s.track);
            }
        }
        out
    }
}

/// How far `y` is outside `top`..`bottom`, 0 inside.
fn gap(top: f32, bottom: f32, y: f32) -> f32 {
    if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0.0
    }
}

impl SystemHits {
    /// The same map moved by (`dx`, `dy`) onto sheet `sheet`.
    pub(crate) fn placed(mut self, sheet: usize, dx: f32, dy: f32) -> SystemHits {
        self.sheet = sheet;
        self.top += dy;
        self.bottom += dy;
        for s in &mut self.staves {
            s.top += dy;
            s.bottom += dy;
            if let Some((t, _)) = s.standard.as_mut() {
                *t += dy;
            }
            if let Some((t, _, _)) = s.tablature.as_mut() {
                *t += dy;
            }
            for b in &mut s.bars {
                b.start += dx;
                b.notes += dx;
                b.end += dx;
                if let Some((l, r)) = b.time_signature.as_mut() {
                    *l += dx;
                    *r += dx;
                }
                for h in &mut b.beats {
                    h.x += dx;
                }
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Hits {
        let bar = |bar, start: f32| BarHits {
            bar,
            start,
            notes: start + 5.0,
            end: start + 100.0,
            clef: Clef::Treble8vb,
            time_signature: (bar == 1).then_some((start + 1.0, start + 4.0)),
            beats: (0..4)
                .map(|i| BeatHit {
                    voice: 0,
                    beat: i,
                    x: start + 10.0 + 25.0 * i as f32,
                    width: 25.0,
                })
                .collect(),
        };
        Hits {
            systems: vec![SystemHits {
                sheet: 0,
                top: 100.0,
                bottom: 200.0,
                staves: vec![StaffHits {
                    track: 2,
                    staff: 0,
                    standard: Some((100.0, 5.0)),
                    tablature: Some((150.0, 6.0, 6)),
                    top: 100.0,
                    bottom: 180.0,
                    bars: vec![bar(0, 0.0), bar(1, 100.0)],
                }],
            }],
        }
    }

    #[test]
    fn a_click_names_track_bar_beat_and_string() {
        let m = map();
        let s = m.hit(0, 136.0, 162.5, 0).unwrap();
        assert_eq!(
            s,
            Spot {
                track: 2,
                staff: 0,
                bar: 1,
                beat: Some(1),
                string: Some(3),
                step: None,
            }
        );
        let on_staff = m.hit(0, 136.0, 110.0, 0).unwrap();
        assert_eq!(on_staff.string, None);
        // 110 is two spaces down from the top line (100, spaces of 5): the
        // middle line.
        assert_eq!(on_staff.step, Some((4, Clef::Treble8vb)));
        assert_eq!(
            m.hit(0, 136.0, 117.5, 0).unwrap().step,
            Some((1, Clef::Treble8vb))
        );
        // Past the last beat's column: the place after it.
        assert_eq!(m.hit(0, 199.0, 110.0, 0).unwrap().beat, Some(4));
        assert_eq!(m.hit(0, 136.0, 110.0, 1).unwrap().beat, None);
        assert!(m.hit(1, 136.0, 110.0, 0).is_none(), "another sheet");
        assert!(m.hit(0, 136.0, 400.0, 0).is_none(), "far below");
        assert!(m.hit(0, 250.0, 150.0, 0).is_none(), "past the last bar");
        assert_eq!(m.tracks(), [2]);
    }

    #[test]
    fn the_caret_box_and_a_selection() {
        let m = map();
        let f = m
            .find(2, 0, 1, 0, 2, 4, Some((8, Clef::Treble8vb)))
            .unwrap();
        let top = f.step.unwrap();
        assert!(
            (top.y + top.height / 2.0 - 100.0).abs() < 0.01,
            "the top line"
        );
        assert_eq!(f.sheet, 0);
        assert!(f.column.x < 160.0 && f.column.right() > 160.0);
        let s = f.string.unwrap();
        assert!((s.y + s.height / 2.0 - 168.0).abs() < 0.01);
        // An empty voice: where its first note would go.
        assert!(m.find(2, 0, 0, 3, 0, 1, None).is_some());
        assert!(m.find(1, 0, 0, 0, 0, 1, None).is_none(), "a hidden track");
        let (_, whole) = m.span(2, 0, 1, None).unwrap();
        assert_eq!((whole.x, whole.right()), (100.0, 200.0));
        let (_, part) = m.span(2, 0, 0, Some((0, 1, 2))).unwrap();
        assert!(part.x < 35.0 && part.right() > 60.0 && part.right() <= 85.0);
        let ts = m.time_signature_at(0, 102.5, 140.0).unwrap();
        assert_eq!((ts.track, ts.bar), (2, 1));
        assert!(
            m.time_signature_at(0, 30.0, 140.0).is_none(),
            "bar 0 shows none"
        );
        let moved = m.systems[0].clone().placed(3, 10.0, 20.0);
        assert_eq!(moved.sheet, 3);
        assert_eq!(moved.staves[0].bars[0].beats[0].x, 20.0);
        assert_eq!(moved.staves[0].tablature.unwrap().0, 170.0);
    }
}
