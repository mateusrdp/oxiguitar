// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Drawing one system: staves, clefs, signatures, notes, rests, beams,
//! tablature, bar lines and the marks above the staff (NOT-1 to NOT-3).
//!
//! Marks above the staff go on lines of their own (volta brackets, chord
//! names, text, tempo, sections), each line only as tall as needed and only
//! present when the system uses it, so they never overlap the notes or one
//! another (VIEW-7).

use std::collections::HashMap;

use oxiguitar_display::{DisplayList, Paint, Path, Point, Rect};
use oxiguitar_model::{BeatKind, Clef, GraceKind, NoteValue, Ratio, Score, Tuplet};

use crate::draw::{G, Painter, accidental_glyph, glyphs};
use crate::effects;
use crate::grid::BarGrid;
use crate::pitch::key_signature_steps;
use crate::plan::{
    BarPlan, BeatInfo, GRACE_SCALE, Lanes, chord_name_size, grace_step, notehead, rest_glyph,
    voice_stems_up,
};
use crate::rhythm::beam_count;
use crate::system::{
    Plans, SystemSpec, TAB_GAP, View, bar_prefix, bar_suffix, track_name_size, track_names,
};

/// Length of rhythm stems below a tab staff, in staff spaces from its
/// bottom line.
const TAB_RHYTHM: f32 = 3.0;

/// A system drawn at the origin: x from the left margin, y from its top.
pub(crate) struct DrawnSystem {
    pub(crate) list: DisplayList,
    pub(crate) height: f32,
}

/// Where one bar sits in its system.
struct BarX {
    /// Index of the bar in the score.
    index: usize,
    /// The bar's left edge (the previous bar line).
    start: f32,
    /// Where the notes begin, after any signature or repeat sign.
    notes: f32,
    /// x of each column of the bar's grid.
    cols: Vec<f32>,
    /// The bar line.
    end: f32,
}

impl BarX {
    fn x_of(&self, grid: &BarGrid, onset: Ratio) -> f32 {
        self.cols
            .get(grid.index_of(onset))
            .copied()
            .unwrap_or(self.notes)
    }
}

/// Baselines of the lines of marks above a view's top staff.
#[derive(Default)]
struct Lines {
    ending: Option<f32>,
    chord: Option<f32>,
    text: Option<f32>,
    tempo: Option<f32>,
    marker: Option<f32>,
}

/// Where one view sits in its system.
pub(crate) struct Band {
    /// Top line of the standard staff.
    pub(crate) std_top: Option<f32>,
    /// Top line (string 1) of the tablature.
    pub(crate) tab_top: Option<f32>,
    /// Top and bottom lines of the band's staves.
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    lines: Lines,
    /// Effect lanes, nearest the staff first.
    pub(crate) lanes: Vec<LanePos>,
    /// Baseline of the dynamics under the staff.
    pub(crate) dynamics: Option<f32>,
    /// Baselines of the lyric lines, by line number.
    pub(crate) lyrics: Vec<f32>,
}

/// The kinds of effect lane, nearest the staff first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    Hp,
    Bend,
    Vibrato,
    Trill,
    PalmMute,
    LetRing,
    Harmonic,
    Technique,
}

/// One effect lane: its kind, its top and its baseline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LanePos {
    pub(crate) lane: Lane,
    pub(crate) top: f32,
    pub(crate) base: f32,
}

impl Band {
    pub(crate) fn lane(&self, lane: Lane) -> Option<LanePos> {
        self.lanes.iter().copied().find(|l| l.lane == lane)
    }
}

/// The lanes present in a system and their heights, in staff spaces,
/// nearest the staff first. The H/P lane leaves room under its letters for
/// the arcs over the top string's numbers.
fn lane_list(l: Lanes, tab: bool) -> Vec<(Lane, f32)> {
    [
        (Lane::Hp, tab && l.hp, 2.6),
        (Lane::Bend, tab && l.bend, 3.6),
        (Lane::Vibrato, l.vibrato, 1.4),
        (Lane::Trill, l.trill, 2.0),
        (Lane::PalmMute, l.palm_mute, 1.8),
        (Lane::LetRing, l.let_ring, 1.8),
        (Lane::Harmonic, l.harmonic, 1.8),
        (Lane::Technique, l.technique, 2.0),
    ]
    .into_iter()
    .filter(|(_, present, _)| *present)
    .map(|(lane, _, h)| (lane, h))
    .collect()
}

/// Staff geometry: y of a staff step on a standard staff whose top line is
/// at `top`.
fn step_y(top: f32, sp: f32, step: i32) -> f32 {
    top + 4.0 * sp - step as f32 * sp / 2.0
}

/// Draws one system.
pub(crate) fn system(
    score: &Score,
    views: &[View],
    plans: &Plans,
    spec: &SystemSpec,
    p: &Painter<'_>,
) -> DrawnSystem {
    let sp = p.sp;
    let tg = TAB_GAP * sp;

    // Horizontal: bars one after another after the system prefix.
    let mut bars = Vec::new();
    let mut x = spec.prefix;
    for (k, i) in (spec.first..spec.end).enumerate() {
        let pre = bar_prefix(score, p, i, k == 0);
        let extra = spec.extra.get(k).copied().unwrap_or(0.0);
        let (cols, w) = plans
            .grids
            .get(i)
            .map_or((Vec::new(), 4.0 * sp), |g| g.place(extra));
        let notes = x + pre;
        let end = notes + w + bar_suffix(score, p, i);
        bars.push(BarX {
            index: i,
            start: x,
            notes,
            cols: cols.iter().map(|c| notes + c).collect(),
            end,
        });
        x = end;
    }
    let right = x;

    // Vertical: each view's marks, staff and tablature, top to bottom.
    let range = spec.first..spec.end;
    let mbs = score.master_bars.get(range.clone()).unwrap_or_default();
    let has_endings = mbs
        .iter()
        .any(|mb| mb.alternate_endings != 0 || mb.repeat_end.is_some_and(|n| n > 2));
    let has_tempo = spec.first == 0 || mbs.iter().any(|mb| !mb.tempo_changes.is_empty());
    let has_marker = mbs.iter().any(|mb| mb.marker.is_some());
    let mut bands = Vec::new();
    let mut cursor = 0.0;
    for (vi, v) in views.iter().enumerate() {
        let vplans = plans
            .bars
            .get(vi)
            .and_then(|b| b.get(range.clone()))
            .unwrap_or_default();
        let high = vplans.iter().map(|b| b.high).max().unwrap_or(8);
        let low = vplans.iter().map(|b| b.low).min().unwrap_or(0);
        let chords = vplans.iter().any(|b| b.has_chords);
        let text = vplans.iter().any(|b| b.has_text);
        let first = vi == 0;
        // Tab-only views draw rhythm: voices 1 and 3 above the tab, 2 and 4
        // below, so marks above start beyond the upward stems.
        let tab_rhythm = v.show_tab && !v.show_std;
        let voices_up = tab_rhythm
            && vplans
                .iter()
                .any(|b| shown_voices(b).into_iter().any(voice_stems_up));
        let voices_down = tab_rhythm
            && vplans
                .iter()
                .any(|b| shown_voices(b).into_iter().any(|vi| !voice_stems_up(vi)));
        let lanes_used = vplans
            .iter()
            .map(|b| b.lanes)
            .fold(Lanes::default(), Lanes::or);
        let lane_kinds = lane_list(lanes_used, v.show_tab);
        let lanes_height: f32 = lane_kinds.iter().map(|(_, h)| h * sp).sum();
        let has_dynamics = vplans.iter().any(|b| {
            b.voices
                .iter()
                .flatten()
                .any(|bt| bt.show_dynamic.is_some())
        });
        let lyric_lines = vplans
            .iter()
            .flat_map(|b| b.voices.iter().flatten())
            .flat_map(|bt| bt.lyrics.iter().map(|(l, _)| l + 1))
            .max()
            .unwrap_or(0);
        // Lanes above the tab when there is one, otherwise above the staff.
        let lanes_on_top = !v.show_std || !v.show_tab;
        let mut stack = if v.show_std {
            ((high - 8).max(0) as f32) * sp / 2.0 + sp
        } else if voices_up {
            TAB_RHYTHM * sp + 2.0 * sp
        } else {
            sp
        };
        if first {
            stack = stack.max(2.5 * sp);
        }
        let lanes_from = stack;
        if lanes_on_top {
            stack += lanes_height;
        }
        // Baselines measured upwards from the top staff line.
        let mut line = |present: bool, height: f32| -> Option<f32> {
            present.then(|| {
                let base = stack + 0.6 * sp;
                stack += height;
                base
            })
        };
        let ending = line(first && has_endings, 2.6 * sp);
        let chord = line(chords, 2.8 * sp);
        let text = line(text, 2.4 * sp);
        let tempo = line(first && has_tempo, 3.2 * sp);
        let marker = line(first && has_marker, 3.6 * sp);
        let top = cursor + stack;
        let up = |o: Option<f32>| o.map(|d| top - d);
        let lines = Lines {
            ending: up(ending),
            chord: up(chord),
            text: up(text),
            tempo: up(tempo),
            marker: up(marker),
        };
        let below_std = ((-low).max(0) as f32) * sp / 2.0;
        let std_top = v.show_std.then_some(top);
        // Under the staff (or under the tab and its rhythm): dynamics, then
        // lyrics.
        let under_lines = |from: f32| -> (Option<f32>, Vec<f32>, f32) {
            let mut y = from;
            let dynamics = has_dynamics.then(|| {
                y += 2.6 * sp;
                y - 0.6 * sp
            });
            let lyrics = (0..lyric_lines)
                .map(|_| {
                    y += 2.4 * sp;
                    y - 0.5 * sp
                })
                .collect();
            (dynamics, lyrics, y)
        };
        let std_bottom = top + 4.0 * sp + below_std;
        let (mut dynamics, mut lyrics, after_std) = if v.show_std {
            under_lines(std_bottom)
        } else {
            (None, Vec::new(), std_bottom)
        };
        let tab_top = v.show_tab.then_some(if v.show_std {
            after_std + 2.0 * sp + lanes_height
        } else {
            top
        });
        let strings = v.strings.max(1);
        let bottom = match tab_top {
            Some(t) => t + (strings - 1) as f32 * tg,
            None => top + 4.0 * sp,
        };
        // Effect lanes upwards from the tab, or from the top staff line.
        let mut lanes = Vec::new();
        let mut y = match tab_top {
            Some(t) if v.show_std => t,
            _ => top - lanes_from,
        };
        for (lane, h) in &lane_kinds {
            let lift = if *lane == Lane::Hp { 1.3 } else { 0.45 };
            let base = y - lift * sp;
            y -= h * sp;
            lanes.push(LanePos {
                lane: *lane,
                top: y,
                base,
            });
        }
        let mut after = if tab_top.is_some() {
            if voices_down {
                TAB_RHYTHM * sp + 2.5 * sp
            } else {
                1.5 * sp
            }
        } else {
            after_std - bottom + 1.5 * sp
        };
        if !v.show_std {
            let (d, l, end) = under_lines(bottom + after - 1.0 * sp);
            dynamics = d;
            lyrics = l;
            after = end - bottom + 1.0 * sp;
        }
        bands.push(Band {
            std_top,
            tab_top,
            top,
            bottom,
            lines,
            lanes,
            dynamics,
            lyrics,
        });
        cursor = bottom + after + 2.5 * sp;
    }
    let height = (cursor - 2.5 * sp).max(0.0);

    let mut list = DisplayList::new(right, height);
    let mut ties: HashMap<(usize, usize, u8), (f32, f32)> = HashMap::new();
    for (vi, (v, band)) in views.iter().zip(&bands).enumerate() {
        let vplans = plans.bars.get(vi);
        let plan_of = |i: usize| vplans.and_then(|b| b.get(i));
        let clef = plan_of(spec.first).map_or(Clef::Treble8vb, |b| b.clef);
        let fifths = score
            .master_bars
            .get(spec.first)
            .map_or(0, |mb| mb.key_signature.fifths);
        let ctx = Ctx {
            p,
            score,
            band,
            view: v,
            tg,
            right,
        };
        ctx.staff_lines(&mut list, spec.indent, right);
        ctx.clefs_and_key(&mut list, spec.indent, clef, fifths);
        if vi == 0 && spec.index > 0 {
            let number = (spec.first + 1).to_string();
            p.text(
                &mut list,
                &number,
                1.6 * sp,
                spec.indent,
                band.top - 2.0 * sp,
                Paint::Muted,
            );
        }
        for (k, bx) in bars.iter().enumerate() {
            let Some(plan) = plan_of(bx.index) else {
                continue;
            };
            let Some(grid) = plans.grids.get(bx.index) else {
                continue;
            };
            ctx.bar_start(&mut list, bx, k == 0, plan.clef);
            if let Some(top) = band.std_top {
                ctx.std_bar(&mut list, plan, grid, bx, top, vi, &mut ties);
            }
            if let Some(top) = band.tab_top {
                ctx.tab_bar(&mut list, plan, grid, bx, top);
                if band.std_top.is_none() {
                    ctx.tab_rhythm(&mut list, plan, grid, bx, top);
                }
            }
            ctx.bar_end(&mut list, bx);
            ctx.marks(&mut list, plan, grid, bx, vi == 0);
        }
        // Effects, once every bar of the view is placed.
        let mut marks = Vec::new();
        for bx in &bars {
            let (Some(plan), Some(grid)) = (plan_of(bx.index), plans.grids.get(bx.index)) else {
                continue;
            };
            for (voice, beats) in plan.voices.iter().enumerate() {
                for b in beats {
                    let Some(onset) = b.onset else { continue };
                    if b.kind == BeatKind::Empty {
                        continue;
                    }
                    let ci = grid.index_of(onset);
                    let x = bx.cols.get(ci).copied().unwrap_or(bx.notes);
                    let next_x = bx.cols.get(ci + 1).copied().unwrap_or(bx.end);
                    marks.push(effects::Mark {
                        voice,
                        x,
                        next_x,
                        beat: b,
                    });
                }
            }
        }
        let geometry = effects::Geometry { band, tg, right };
        effects::draw(p, &mut list, &marks, &geometry);
    }
    // The line joining every staff at the start of the system.
    if let (Some(first), Some(last)) = (bands.first(), bands.last())
        && (bands.len() > 1 || first.std_top.is_some() && first.tab_top.is_some())
    {
        p.thin_bar(&mut list, spec.indent, first.top, last.bottom);
    }
    // Track names, right-aligned in the indent, centred on each track's
    // staves.
    let size = track_name_size(p);
    let room = spec.indent - sp;
    for (track, name) in track_names(score, views, spec.index == 0) {
        let mine: Vec<&Band> = views
            .iter()
            .zip(&bands)
            .filter(|(v, _)| v.track == track)
            .map(|(_, b)| b)
            .collect();
        let (Some(first), Some(last)) = (mine.first(), mine.last()) else {
            continue;
        };
        if name.is_empty() || room <= 0.0 {
            continue;
        }
        let label = fit_text(p, &name, size, room);
        let y = (first.top + last.bottom) / 2.0 + 0.35 * size;
        p.text_right(&mut list, &label, size, room, y, Paint::Ink);
    }
    DrawnSystem { list, height }
}

/// What every drawing step of one view needs.
struct Ctx<'a> {
    p: &'a Painter<'a>,
    score: &'a Score,
    band: &'a Band,
    view: &'a View,
    tg: f32,
    /// The system's right edge: text never runs past it.
    right: f32,
}

impl Ctx<'_> {
    fn sp(&self) -> f32 {
        self.p.sp
    }

    fn line_t(&self) -> f32 {
        self.p.ed().staff_line_thickness * self.sp()
    }

    fn tab_bottom(&self, top: f32) -> f32 {
        top + (self.view.strings.max(1) - 1) as f32 * self.tg
    }

    /// Staves as (top, bottom, kind): standard notation and/or tablature.
    fn staves(&self) -> Vec<(f32, f32, bool)> {
        let mut v = Vec::new();
        if let Some(t) = self.band.std_top {
            v.push((t, t + 4.0 * self.sp(), true));
        }
        if let Some(t) = self.band.tab_top {
            v.push((t, self.tab_bottom(t), false));
        }
        v
    }

    fn staff_lines(&self, list: &mut DisplayList, left: f32, right: f32) {
        let sp = self.sp();
        let t = self.line_t();
        if let Some(top) = self.band.std_top {
            for i in 0..5 {
                self.p.hline(list, left, right, top + i as f32 * sp, t);
            }
        }
        if let Some(top) = self.band.tab_top {
            for i in 0..self.view.strings {
                self.p.hline(list, left, right, top + i as f32 * self.tg, t);
            }
        }
    }

    fn clefs_and_key(&self, list: &mut DisplayList, left: f32, clef: Clef, fifths: i8) {
        let p = self.p;
        let sp = self.sp();
        let x = left + 0.6 * sp;
        if let Some(top) = self.band.std_top {
            let (g, step) = clef_glyph(clef);
            p.glyph(list, g, x, step_y(top, sp, step), Paint::Ink);
            let mut kx =
                x + p.width(glyphs::G_CLEF_8VB).max(p.width(glyphs::TAB_CLEF_6)) + 0.8 * sp;
            for (step, alter) in key_signature_steps_for(fifths, clef) {
                let g = accidental_glyph(alter);
                p.glyph(list, g, kx, step_y(top, sp, step), Paint::Ink);
                kx += p.width(g) + 0.15 * sp;
            }
        }
        if let Some(top) = self.band.tab_top {
            let g = if self.view.strings == 4 {
                glyphs::TAB_CLEF_4
            } else {
                glyphs::TAB_CLEF_6
            };
            let (above, below) = p.height(g);
            let glyph_h = (above + below).max(sp);
            let want = (self.view.strings.max(2) - 1) as f32 * self.tg * 0.8;
            let scale = (want / glyph_h).clamp(0.3, 3.0);
            let mid = (top + self.tab_bottom(top)) / 2.0;
            let y = mid + (above - below) / 2.0 * scale;
            p.glyph_scaled(list, g, scale, x, y, Paint::Ink);
        }
    }

    /// What opens a bar before its notes. At the start of a system: time
    /// signature, then start-repeat sign (the system prefix shows clef and
    /// key). Elsewhere: start-repeat sign, key change, time signature.
    fn bar_start(&self, list: &mut DisplayList, bx: &BarX, first_in_system: bool, clef: Clef) {
        let p = self.p;
        let sp = self.sp();
        let Some(mb) = self.score.master_bars.get(bx.index) else {
            return;
        };
        let previous = bx
            .index
            .checked_sub(1)
            .and_then(|j| self.score.master_bars.get(j));
        let mut x = bx.start;
        if !first_in_system && mb.repeat_start {
            x = self.repeat_start(list, x);
        }
        if !first_in_system {
            x += 0.6 * sp;
        }
        if !first_in_system
            && let Some(prev) = previous
            && prev.key_signature.fifths != mb.key_signature.fifths
        {
            let (old, new) = (prev.key_signature.fifths, mb.key_signature.fifths);
            if let Some(top) = self.band.std_top {
                let mut kx = x;
                let naturals = cancelled_steps(old, new, clef);
                for step in &naturals {
                    p.glyph(
                        list,
                        glyphs::NATURAL,
                        kx,
                        step_y(top, sp, *step),
                        Paint::Ink,
                    );
                    kx += p.width(glyphs::NATURAL) + 0.15 * sp;
                }
                if !naturals.is_empty() {
                    kx += 0.4 * sp;
                }
                for (step, alter) in key_signature_steps_for(new, clef) {
                    let g = accidental_glyph(alter);
                    p.glyph(list, g, kx, step_y(top, sp, step), Paint::Ink);
                    kx += p.width(g) + 0.15 * sp;
                }
            }
            x += p.key_change_width(old, new) + sp;
        }
        let ts = mb.time_signature;
        if previous.is_none_or(|prev| prev.time_signature != ts) {
            let w = p.time_width(ts.numerator, ts.denominator);
            let cx = x + w / 2.0;
            if let Some(top) = self.band.std_top {
                p.time_signature(
                    list,
                    ts.numerator,
                    ts.denominator,
                    cx,
                    step_y(top, sp, 6),
                    step_y(top, sp, 2),
                );
            }
            if let Some(top) = self.band.tab_top {
                let h = self.tab_bottom(top) - top;
                p.time_signature(
                    list,
                    ts.numerator,
                    ts.denominator,
                    cx,
                    top + 0.27 * h,
                    top + 0.73 * h,
                );
            }
            x += w + sp;
        }
        if first_in_system && mb.repeat_start {
            self.repeat_start(list, x);
        }
    }

    /// A start-repeat sign whose thick line begins at `x`; returns its right
    /// edge.
    fn repeat_start(&self, list: &mut DisplayList, x: f32) -> f32 {
        let p = self.p;
        let sp = self.sp();
        let thick = p.ed().thick_barline_thickness * sp;
        let thin_x = x + thick / 2.0 + p.thin_thick_step();
        for (top, bottom, std) in self.staves() {
            p.thick_bar(list, x + thick / 2.0, top, bottom);
            p.thin_bar(list, thin_x, top, bottom);
            self.repeat_dots(list, thin_x + 0.35 * sp, top, bottom, std);
        }
        thin_x + 0.35 * sp + p.width(glyphs::REPEAT_DOT)
    }

    fn repeat_dots(&self, list: &mut DisplayList, x: f32, top: f32, bottom: f32, std: bool) {
        let sp = self.sp();
        let ys = if std {
            [step_y(top, sp, 5), step_y(top, sp, 3)]
        } else {
            let mid = (top + bottom) / 2.0;
            let off = if self.view.strings.is_multiple_of(2) {
                self.tg
            } else {
                self.tg / 2.0
            };
            [mid - off, mid + off]
        };
        for y in ys {
            self.p.glyph(list, glyphs::REPEAT_DOT, x, y, Paint::Ink);
        }
    }

    /// The bar line closing a bar: plain, double, final or end-repeat.
    fn bar_end(&self, list: &mut DisplayList, bx: &BarX) {
        let p = self.p;
        let sp = self.sp();
        let Some(mb) = self.score.master_bars.get(bx.index) else {
            return;
        };
        let last = bx.index + 1 == self.score.master_bars.len();
        let thick = p.ed().thick_barline_thickness * sp;
        for (top, bottom, std) in self.staves() {
            let x = bx.end;
            if mb.repeat_end.is_some() || last {
                p.thick_bar(list, x - thick / 2.0, top, bottom);
                let thin_x = x - thick / 2.0 - p.thin_thick_step();
                p.thin_bar(list, thin_x, top, bottom);
                if mb.repeat_end.is_some() {
                    let dot = p.width(glyphs::REPEAT_DOT);
                    self.repeat_dots(list, thin_x - 0.35 * sp - dot, top, bottom, std);
                }
            } else if mb.double_bar {
                p.thin_bar(list, x, top, bottom);
                p.thin_bar(list, x - 0.5 * sp, top, bottom);
            } else {
                p.thin_bar(list, x, top, bottom);
            }
        }
    }

    /// Notes, rests, beams and ties of one bar on the standard staff.
    #[allow(clippy::too_many_arguments)]
    fn std_bar(
        &self,
        list: &mut DisplayList,
        plan: &BarPlan,
        grid: &BarGrid,
        bx: &BarX,
        top: f32,
        view_index: usize,
        ties: &mut HashMap<(usize, usize, u8), (f32, f32)>,
    ) {
        let p = self.p;
        let sp = self.sp();
        let y = |step: i32| step_y(top, sp, step);
        let filled: Vec<usize> = plan
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.iter().any(|b| b.kind != BeatKind::Empty))
            .map(|(i, _)| i)
            .collect();
        let multi = filled.len() > 1;
        let hw = p.width(glyphs::NOTEHEAD_BLACK);
        let stem_t = p.ed().stem_thickness * sp;

        for (vi, beats) in plan.voices.iter().enumerate() {
            let groups = plan.groups.get(vi).map_or(&[][..], Vec::as_slice);
            let xs: Vec<Option<f32>> = beats
                .iter()
                .map(|b| b.onset.map(|o| bx.x_of(grid, o)))
                .collect();
            // Stems: where they join the notes and where they end.
            let mut stems: Vec<Option<Stem>> = beats
                .iter()
                .zip(&xs)
                .map(|(b, x)| {
                    let x = (*x)?;
                    self.stem_base(b, x, top)
                })
                .collect();
            for g in groups {
                self.beam_group(list, beats, g, &mut stems, top);
            }
            for (i, b) in beats.iter().enumerate() {
                let Some(cx) = xs.get(i).copied().flatten() else {
                    continue;
                };
                match b.kind {
                    BeatKind::Empty => {}
                    BeatKind::Rest => {
                        let whole_bar = beats.iter().filter(|b| b.onset.is_some()).count() == 1;
                        let cx = if whole_bar && b.value <= NoteValue::Whole {
                            (bx.notes + bx.end) / 2.0
                        } else {
                            cx
                        };
                        let g = rest_glyph(b.value);
                        let mut step = if b.value <= NoteValue::Whole { 6 } else { 4 };
                        if multi {
                            step += if voice_stems_up(vi) { 4 } else { -4 };
                        }
                        let w = p.width(g);
                        p.glyph(list, g, cx - w / 2.0, y(step), Paint::Ink);
                        for d in 0..b.dots {
                            let dx = cx + w / 2.0 + 0.3 * sp + f32::from(d) * 0.5 * sp;
                            p.glyph(list, glyphs::AUGMENTATION_DOT, dx, y(step | 1), Paint::Ink);
                        }
                    }
                    BeatKind::Notes => {
                        let stem = stems.get(i).copied().flatten();
                        self.chord(list, b, cx, top, stem, (view_index, vi), ties);
                        if let Some(s) = stem {
                            p.vline(list, s.x, s.base, s.tip, stem_t);
                            if let Some(g) = tremolo_glyph(b) {
                                let w = p.width(g);
                                let mid = s.base + (s.tip - s.base) * 0.55;
                                p.glyph(list, g, s.x - w / 2.0 + 0.6 * sp, mid, Paint::Ink);
                            }
                            let beams = beam_count(b.value);
                            if b.group.is_none() && beams > 0 {
                                let flags = if s.up {
                                    &glyphs::FLAGS_UP
                                } else {
                                    &glyphs::FLAGS_DOWN
                                };
                                if let Some(f) = flags.get(beams.min(4) - 1) {
                                    p.glyph(list, *f, s.x - stem_t / 2.0, s.tip, Paint::Ink);
                                }
                            }
                        }
                    }
                }
            }
            self.tuplets(list, beats, &xs, &stems, top, hw);
            self.std_graces(list, beats, &xs, top);
        }
    }

    /// Grace notes on the staff: small notes with stems up before their
    /// main note, slashed when played before the beat, slurred to it.
    fn std_graces(&self, list: &mut DisplayList, beats: &[BeatInfo], xs: &[Option<f32>], top: f32) {
        let p = self.p;
        let sp = self.sp();
        let y = |step: i32| step_y(top, sp, step);
        let scale = GRACE_SCALE;
        let step = grace_step(p, self.tg);
        let hw = p.width(glyphs::NOTEHEAD_BLACK);
        let ed = p.ed();
        for (i, b) in beats.iter().enumerate() {
            if b.grace.is_none() || b.notes.is_empty() {
                continue;
            }
            let Some(main_x) = xs.get(i + b.grace_rank).copied().flatten() else {
                continue;
            };
            let main = beats.get(i + b.grace_rank);
            let main_left = hw / 2.0
                + if main.is_some_and(|m| m.notes.iter().any(|n| n.accidental.is_some())) {
                    p.width(glyphs::SHARP) + 0.2 * sp
                } else {
                    0.0
                };
            let gx = main_x - main_left - 0.3 * sp - (b.grace_rank as f32 - 0.5) * step;
            let head = notehead(b.value.max(NoteValue::Quarter), false);
            let w = p.width(head) * scale;
            let left = gx - w / 2.0;
            for n in &b.notes {
                let ny = y(n.step);
                let ext = ed.leger_line_extension * sp * scale;
                let mut l = -2;
                while l >= n.step {
                    p.hline(
                        list,
                        left - ext,
                        left + w + ext,
                        y(l),
                        ed.leger_line_thickness * sp,
                    );
                    l -= 2;
                }
                let mut l = 10;
                while l <= n.step {
                    p.hline(
                        list,
                        left - ext,
                        left + w + ext,
                        y(l),
                        ed.leger_line_thickness * sp,
                    );
                    l += 2;
                }
                p.glyph_scaled(list, head, scale, left, ny, Paint::Ink);
                if let Some(alter) = n.accidental {
                    let g = accidental_glyph(alter);
                    let aw = p.width(g) * scale;
                    p.glyph_scaled(list, g, scale, left - 0.15 * sp - aw, ny, Paint::Ink);
                }
            }
            let (Some(low), Some(high)) = (b.notes.first(), b.notes.last()) else {
                continue;
            };
            let (ax, ay) = p.anchor(head, "stemUpSE", [1.18, 0.168]);
            let t = ed.stem_thickness * sp * scale;
            let sx = left + ax * scale - t / 2.0;
            let tip = y(high.step) - 2.5 * sp;
            p.vline(list, sx, y(low.step) + ay * scale, tip, t);
            let beams = beam_count(b.value).clamp(1, 4);
            if let Some(f) = glyphs::FLAGS_UP.get(beams - 1) {
                p.glyph_scaled(list, *f, scale, sx - t / 2.0, tip, Paint::Ink);
            }
            if b.grace == Some(GraceKind::BeforeBeat) {
                p.line_between(
                    list,
                    Point::new(sx - 0.45 * sp, tip + 1.4 * sp),
                    Point::new(sx + 0.8 * sp, tip + 0.5 * sp),
                    0.1 * sp,
                );
            }
            // Slur from the first grace note to the main note.
            if b.grace_rank == main.map_or(0, |m| m.graces_before) {
                let sy = y(low.step) + 0.7 * sp;
                let to = main_x - hw / 2.0;
                if to > gx {
                    p.slur(list, Point::new(gx, sy), Point::new(to, sy), 0.5 * sp);
                }
            }
        }
    }

    /// Where a beat's stem starts and its unbeamed length, or `None` for
    /// beats without a stem.
    fn stem_base(&self, b: &BeatInfo, cx: f32, top: f32) -> Option<Stem> {
        let p = self.p;
        let sp = self.sp();
        if b.kind != BeatKind::Notes || b.value <= NoteValue::Whole {
            return None;
        }
        let (first, last) = (b.notes.first()?, b.notes.last()?);
        let head = notehead(b.value, false);
        let left = cx - p.width(head) / 2.0;
        let t = p.ed().stem_thickness * sp;
        let beams = beam_count(b.value);
        let length = 3.5 * sp + beams.saturating_sub(2) as f32 * 0.75 * sp;
        let middle = step_y(top, sp, 4);
        if b.up {
            let (ax, ay) = p.anchor(head, "stemUpSE", [1.18, 0.168]);
            let far = step_y(top, sp, last.step);
            Some(Stem {
                x: left + ax - t / 2.0,
                base: step_y(top, sp, first.step) + ay,
                tip: (far - length).min(middle),
                far,
                up: true,
            })
        } else {
            let (ax, ay) = p.anchor(head, "stemDownNW", [0.0, -0.168]);
            let far = step_y(top, sp, first.step);
            Some(Stem {
                x: left + ax + t / 2.0,
                base: step_y(top, sp, last.step) + ay,
                tip: (far + length).max(middle),
                far,
                up: false,
            })
        }
    }

    /// Draws the beams of one group and moves its stems' tips onto them.
    fn beam_group(
        &self,
        list: &mut DisplayList,
        beats: &[BeatInfo],
        members: &[usize],
        stems: &mut [Option<Stem>],
        top: f32,
    ) {
        let p = self.p;
        let sp = self.sp();
        let ms: Vec<(usize, Stem, usize)> = members
            .iter()
            .filter_map(|&i| {
                let s = stems.get(i).copied().flatten()?;
                let b = beats.get(i)?;
                Some((i, s, beam_count(b.value)))
            })
            .collect();
        let (Some(first), Some(last)) = (ms.first(), ms.last()) else {
            return;
        };
        if ms.len() < 2 {
            return;
        }
        let up = first.1.up;
        let levels = ms.iter().map(|m| m.2).max().unwrap_or(1).max(1);
        let x0 = first.1.x;
        let span = (last.1.x - x0).max(f32::EPSILON);
        let rise = (last.1.far - first.1.far).clamp(-sp, sp);
        let slope = rise / span;
        let min_len = 3.0 * sp + (levels - 1) as f32 * 0.75 * sp;
        let middle = step_y(top, sp, 4);
        let a = if up {
            ms.iter()
                .map(|(_, s, _)| (s.far - min_len).min(middle) - slope * (s.x - x0))
                .fold(f32::INFINITY, f32::min)
        } else {
            ms.iter()
                .map(|(_, s, _)| (s.far + min_len).max(middle) - slope * (s.x - x0))
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let line = |x: f32| a + slope * (x - x0);
        for (i, s, _) in &ms {
            if let Some(Some(st)) = stems.get_mut(*i) {
                st.tip = line(s.x);
                st.up = up;
            }
        }
        let h = p.ed().beam_thickness * sp;
        let step = h + 0.25 * sp;
        let t = p.ed().stem_thickness * sp;
        let centre = |x: f32, level: usize| {
            let off = h / 2.0 + level as f32 * step;
            if up { line(x) + off } else { line(x) - off }
        };
        let hook = p.width(glyphs::NOTEHEAD_BLACK);
        for level in 0..levels {
            let mut k = 0;
            while k < ms.len() {
                let has = |j: usize| ms.get(j).is_some_and(|m| m.2 > level);
                if !has(k) {
                    k += 1;
                    continue;
                }
                let start = k;
                while has(k + 1) {
                    k += 1;
                }
                let (xa, xb) = match (ms.get(start), ms.get(k)) {
                    (Some(a), Some(b)) if start < k => (a.1.x - t / 2.0, b.1.x + t / 2.0),
                    (Some(a), _) if start + 1 == ms.len() => (a.1.x - hook, a.1.x + t / 2.0),
                    (Some(a), _) => (a.1.x - t / 2.0, a.1.x + hook),
                    _ => break,
                };
                p.beam(list, xa, centre(xa, level), xb, centre(xb, level));
                k += 1;
            }
        }
    }

    /// The notes of one beat: heads, ledger lines, accidentals, dots, ties.
    #[allow(clippy::too_many_arguments)]
    fn chord(
        &self,
        list: &mut DisplayList,
        b: &BeatInfo,
        cx: f32,
        top: f32,
        stem: Option<Stem>,
        voice: (usize, usize),
        ties: &mut HashMap<(usize, usize, u8), (f32, f32)>,
    ) {
        let p = self.p;
        let sp = self.sp();
        let y = |step: i32| step_y(top, sp, step);
        let base_head = notehead(b.value, false);
        let hw = p.width(base_head);
        let left = cx - hw / 2.0;
        let shifted_left = b.notes.iter().any(|n| n.shifted) && !b.up;
        let shifted_right = b.notes.iter().any(|n| n.shifted) && b.up;
        let ed = p.ed();
        let ext = ed.leger_line_extension * sp;
        let lt = ed.leger_line_thickness * sp;
        let up = stem.map_or(b.up, |s| s.up);
        let count = b.notes.len();
        for (k, n) in b.notes.iter().enumerate() {
            let head = if n.diamond {
                diamond_head(b.value)
            } else {
                notehead(b.value, n.dead)
            };
            let w = p.width(head);
            let hx = if n.shifted {
                if b.up {
                    left + hw - ed.stem_thickness * sp
                } else {
                    left - hw + ed.stem_thickness * sp
                }
            } else {
                left
            };
            let ny = y(n.step);
            // Ledger lines from the staff out to the note.
            let mut l = -2;
            while l >= n.step {
                p.hline(list, hx - ext, hx + w + ext, y(l), lt);
                l -= 2;
            }
            let mut l = 10;
            while l <= n.step {
                p.hline(list, hx - ext, hx + w + ext, y(l), lt);
                l += 2;
            }
            p.glyph(list, head, hx, ny, Paint::Ink);
            if n.ghost {
                let lw = p.width(glyphs::PAREN_LEFT);
                p.glyph(
                    list,
                    glyphs::PAREN_LEFT,
                    hx - lw - 0.05 * sp,
                    ny,
                    Paint::Ink,
                );
                p.glyph(list, glyphs::PAREN_RIGHT, hx + w + 0.2 * sp, ny, Paint::Ink);
            }
            if let Some(alter) = n.accidental {
                let g = accidental_glyph(alter);
                let column_w = p.width(glyphs::SHARP) + 0.15 * sp;
                let edge = if shifted_left { left - hw } else { left };
                let paren = if n.ghost {
                    p.width(glyphs::PAREN_LEFT)
                } else {
                    0.0
                };
                let ax =
                    edge - 0.2 * sp - paren - n.accidental_column as f32 * column_w - p.width(g);
                p.glyph(list, g, ax, ny, Paint::Ink);
            }
            if b.dots > 0 {
                let edge = if shifted_right {
                    left + 2.0 * hw
                } else {
                    left + hw
                };
                let dot_step = if n.step % 2 == 0 { n.step + 1 } else { n.step };
                for d in 0..b.dots {
                    let dx = edge + 0.35 * sp + f32::from(d) * 0.5 * sp;
                    p.glyph(list, glyphs::AUGMENTATION_DOT, dx, y(dot_step), Paint::Ink);
                }
            }
            // Ties: from the same pitch's last note in this voice.
            if let Some(pitch) = n.pitch {
                let key = (voice.0, voice.1, pitch.0);
                if n.tie {
                    let above = if count == 1 { !up } else { k * 2 >= count };
                    let dir = if above { -1.0 } else { 1.0 };
                    let ty = ny + dir * 0.6 * sp;
                    let from = ties
                        .get(&key)
                        .map_or(hx - 2.0 * sp, |(x, _)| *x + 0.15 * sp);
                    let to = hx - 0.15 * sp;
                    if to > from {
                        let height = dir * (0.35 * sp + 0.04 * (to - from)).min(1.2 * sp);
                        p.tie(list, Point::new(from, ty), Point::new(to, ty), height);
                    }
                }
                ties.insert(key, (hx + w, ny));
            }
        }
    }

    /// Tuplet numbers (and brackets where no single beam shows the group).
    fn tuplets(
        &self,
        list: &mut DisplayList,
        beats: &[BeatInfo],
        xs: &[Option<f32>],
        stems: &[Option<Stem>],
        top: f32,
        hw: f32,
    ) {
        for (range, t) in tuplet_groups(beats) {
            self.tuplet_mark(list, beats, xs, stems, range, t, top, hw);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tuplet_mark(
        &self,
        list: &mut DisplayList,
        beats: &[BeatInfo],
        xs: &[Option<f32>],
        stems: &[Option<Stem>],
        range: std::ops::Range<usize>,
        t: Tuplet,
        top: f32,
        hw: f32,
    ) {
        let p = self.p;
        let sp = self.sp();
        let members: Vec<usize> = range.clone().collect();
        let (Some(&a), Some(&b)) = (members.first(), members.last()) else {
            return;
        };
        let (Some(xa), Some(xb)) = (xs.get(a).copied().flatten(), xs.get(b).copied().flatten())
        else {
            return;
        };
        let up = beats.get(a).is_none_or(|x| x.up);
        // The outermost point of the group on the stem side.
        let mut edge = if up { top } else { top + 4.0 * sp };
        for &m in &members {
            if let Some(s) = stems.get(m).copied().flatten() {
                edge = if up { edge.min(s.tip) } else { edge.max(s.tip) };
            }
            if let Some(bi) = beats.get(m) {
                for n in &bi.notes {
                    let ny = step_y(top, sp, n.step);
                    edge = if up {
                        edge.min(ny - sp)
                    } else {
                        edge.max(ny + sp)
                    };
                }
            }
        }
        let beamed = {
            let g = beats.get(a).and_then(|x| x.group);
            g.is_some()
                && members
                    .iter()
                    .all(|&m| beats.get(m).is_some_and(|x| x.group == g))
        };
        let digits = tuplet_label(t);
        let w: f32 = digits.iter().map(|g| p.width(*g)).sum();
        let (above, below) = digits.first().map_or((sp, 0.0), |g| p.height(*g));
        let cx = (xa + xb) / 2.0;
        let base = if up {
            edge - 0.4 * sp - below
        } else {
            edge + 0.4 * sp + above
        };
        let mut x = cx - w / 2.0;
        for g in &digits {
            p.glyph(list, *g, x, base, Paint::Ink);
            x += p.width(*g);
        }
        if !beamed && a != b {
            let by = base - (above - below) / 2.0;
            let hook = if up { 0.6 * sp } else { -0.6 * sp };
            let l = xa - hw / 2.0;
            let r = xb + hw / 2.0;
            let gap = w / 2.0 + 0.3 * sp;
            let t = 0.16 * sp;
            let path = Path::new()
                .move_to(l, by + hook)
                .line_to(l, by)
                .line_to(cx - gap, by);
            list.stroke(path, t, Paint::Ink);
            let path = Path::new()
                .move_to(cx + gap, by)
                .line_to(r, by)
                .line_to(r, by + hook);
            list.stroke(path, t, Paint::Ink);
        }
    }

    /// Rhythm on the tablature when standard notation is hidden (VIEW-2),
    /// for every voice shown: stems by voice number as on the staff
    /// (voices 1 and 3 above the tab, pointing up; voices 2 and 4 below,
    /// pointing down), flat beams or flags at their ends, dots, tuplet
    /// numbers, and rests inside the staff.
    fn tab_rhythm(
        &self,
        list: &mut DisplayList,
        plan: &BarPlan,
        grid: &BarGrid,
        bx: &BarX,
        top: f32,
    ) {
        for vi in shown_voices(plan) {
            let Some(beats) = plan.voices.get(vi) else {
                continue;
            };
            let groups = plan.groups.get(vi).map_or(&[][..], Vec::as_slice);
            self.tab_voice(list, beats, groups, grid, bx, top, voice_stems_up(vi));
        }
    }

    /// One voice of [`Ctx::tab_rhythm`], with its stems up or down.
    #[allow(clippy::too_many_arguments)]
    fn tab_voice(
        &self,
        list: &mut DisplayList,
        beats: &[BeatInfo],
        groups: &[Vec<usize>],
        grid: &BarGrid,
        bx: &BarX,
        top: f32,
        up: bool,
    ) {
        let p = self.p;
        let sp = self.sp();
        let bottom = self.tab_bottom(top);
        // The stem tips; away from the tab is `dir`.
        let (tip, dir) = if up {
            (top - TAB_RHYTHM * sp, -1.0)
        } else {
            (bottom + TAB_RHYTHM * sp, 1.0)
        };
        let t = p.ed().stem_thickness * sp;
        let string_y = |s: u8| top + f32::from(s.saturating_sub(1)) * self.tg;
        let xs: Vec<Option<f32>> = beats
            .iter()
            .map(|b| b.onset.map(|o| bx.x_of(grid, o)))
            .collect();
        for (b, x) in beats.iter().zip(&xs) {
            let Some(x) = *x else { continue };
            match b.kind {
                BeatKind::Notes => {
                    if b.value >= NoteValue::Half {
                        // From just beyond the outermost fret number on the
                        // stem's side; half notes get a short stem.
                        let strings = b
                            .notes
                            .iter()
                            .filter_map(|n| n.tab.as_ref().map(|(s, _)| *s));
                        let from = if b.value == NoteValue::Half {
                            tip - dir * 1.5 * sp
                        } else if up {
                            strings.min().map_or(top, string_y) - 0.6 * self.tg
                        } else {
                            strings.max().map_or(bottom, string_y) + 0.6 * self.tg
                        };
                        p.vline(list, x, from, tip, t);
                        let beams = beam_count(b.value);
                        let flags = if up {
                            &glyphs::FLAGS_UP
                        } else {
                            &glyphs::FLAGS_DOWN
                        };
                        if b.group.is_none()
                            && beams > 0
                            && let Some(f) = flags.get(beams.min(4) - 1)
                        {
                            p.glyph(list, *f, x - t / 2.0, tip, Paint::Ink);
                        }
                    }
                    for d in 0..b.dots {
                        let dx = x + 0.5 * sp + f32::from(d) * 0.5 * sp;
                        let dy = tip - dir * 1.0 * sp;
                        p.glyph(list, glyphs::AUGMENTATION_DOT, dx, dy, Paint::Ink);
                    }
                }
                BeatKind::Rest => {
                    // In the voice's rhythm area, clear of the fret
                    // numbers, a little smaller than on the staff.
                    let scale = 0.8;
                    let g = rest_glyph(b.value);
                    let w = p.width(g) * scale;
                    let edge = if up { top } else { bottom };
                    let centre = edge + dir * 0.55 * TAB_RHYTHM * sp;
                    let y = match b.value {
                        NoteValue::DoubleWhole | NoteValue::Whole => centre - 0.25 * sp,
                        NoteValue::Half => centre + 0.25 * sp,
                        _ => centre,
                    };
                    p.glyph_scaled(list, g, scale, x - w / 2.0, y, Paint::Ink);
                    for d in 0..b.dots {
                        let dx = x + w / 2.0 + 0.3 * sp + f32::from(d) * 0.5 * sp;
                        p.glyph(
                            list,
                            glyphs::AUGMENTATION_DOT,
                            dx,
                            centre - 0.5 * sp,
                            Paint::Ink,
                        );
                    }
                }
                BeatKind::Empty => {}
            }
        }
        // Flat beams at the stem ends, secondary beams towards the tab.
        let h = p.ed().beam_thickness * sp;
        let step = h + 0.25 * sp;
        let hook = p.width(glyphs::NOTEHEAD_BLACK);
        for g in groups {
            let ms: Vec<(f32, usize)> = g
                .iter()
                .filter_map(|&i| {
                    Some((
                        xs.get(i).copied().flatten()?,
                        beam_count(beats.get(i)?.value),
                    ))
                })
                .collect();
            let levels = ms.iter().map(|m| m.1).max().unwrap_or(0);
            for level in 0..levels {
                let y = tip - dir * (h / 2.0 + level as f32 * step);
                let mut k = 0;
                while k < ms.len() {
                    let has = |j: usize| ms.get(j).is_some_and(|m| m.1 > level);
                    if !has(k) {
                        k += 1;
                        continue;
                    }
                    let start = k;
                    while has(k + 1) {
                        k += 1;
                    }
                    let (xa, xb) = match (ms.get(start), ms.get(k)) {
                        (Some(a), Some(b)) if start < k => (a.0 - t / 2.0, b.0 + t / 2.0),
                        (Some(a), _) if start + 1 == ms.len() => (a.0 - hook, a.0 + t / 2.0),
                        (Some(a), _) => (a.0 - t / 2.0, a.0 + hook),
                        _ => break,
                    };
                    p.beam(list, xa, y, xb, y);
                    k += 1;
                }
            }
        }
        // Tuplet numbers beyond the beams.
        for (range, tuplet) in tuplet_groups(beats) {
            let (Some(xa), Some(xb)) = (
                xs.get(range.start).copied().flatten(),
                range
                    .end
                    .checked_sub(1)
                    .and_then(|e| xs.get(e).copied().flatten()),
            ) else {
                continue;
            };
            let digits = tuplet_label(tuplet);
            let w: f32 = digits.iter().map(|g| p.width(*g)).sum();
            let (above, below) = digits.first().map_or((sp, 0.0), |g| p.height(*g));
            let base = if up {
                tip - 0.4 * sp - below
            } else {
                tip + 0.4 * sp + above
            };
            let mut x = (xa + xb) / 2.0 - w / 2.0;
            for g in &digits {
                p.glyph(list, *g, x, base, Paint::Ink);
                x += p.width(*g);
            }
        }
    }

    /// Fret numbers on the tablature.
    fn tab_bar(&self, list: &mut DisplayList, plan: &BarPlan, grid: &BarGrid, bx: &BarX, top: f32) {
        let step = grace_step(self.p, self.tg);
        let sp = self.sp();
        for beats in &plan.voices {
            // Grace notes: small numbers before their main note.
            for (i, b) in beats.iter().enumerate() {
                if b.grace.is_none() {
                    continue;
                }
                let Some(main) = beats.get(i + b.grace_rank) else {
                    continue;
                };
                let Some(onset) = main.onset else { continue };
                let main_half = main
                    .notes
                    .iter()
                    .filter_map(|n| n.tab.as_ref())
                    .map(|(_, l)| self.p.tab_number_width(self.tg, l) / 2.0)
                    .fold(0.0, f32::max);
                let gx = bx.x_of(grid, onset)
                    - main_half
                    - 0.2 * sp
                    - (b.grace_rank as f32 - 0.5) * step;
                for n in &b.notes {
                    if let Some((string, label)) = &n.tab {
                        let y = top + f32::from(string.saturating_sub(1)) * self.tg;
                        self.p.tab_number(list, gx, y, self.tg * GRACE_SCALE, label);
                    }
                }
            }
            for b in beats {
                let Some(onset) = b.onset else { continue };
                if b.kind != BeatKind::Notes {
                    continue;
                }
                let cx = bx.x_of(grid, onset);
                for n in &b.notes {
                    if let Some((string, label)) = &n.tab {
                        let y = top + f32::from(string.saturating_sub(1)) * self.tg;
                        self.p.tab_number(list, cx, y, self.tg, label);
                    }
                }
            }
        }
    }

    /// Chord names and text over the beats; on the top view, also sections,
    /// tempo and volta brackets.
    fn marks(
        &self,
        list: &mut DisplayList,
        plan: &BarPlan,
        grid: &BarGrid,
        bx: &BarX,
        top_view: bool,
    ) {
        let p = self.p;
        let sp = self.sp();
        let lines = &self.band.lines;
        let hw = p.width(glyphs::NOTEHEAD_BLACK);
        let mut text_right = f32::NEG_INFINITY;
        for beats in &plan.voices {
            for b in beats {
                let Some(onset) = b.onset else { continue };
                let x = bx.x_of(grid, onset) - hw / 2.0;
                if let (Some(name), Some(base)) = (&b.chord, lines.chord) {
                    p.text(list, name, chord_name_size(sp), x, base, Paint::Ink);
                }
                if let (false, Some(base)) = (b.text.is_empty(), lines.text) {
                    let size = 1.8 * sp;
                    let w = p.text_width(&b.text, size);
                    let x = x.max(text_right + 0.4 * sp).min(self.right - w).max(0.0);
                    text_right = x + p.text(list, &b.text, size, x, base, Paint::Ink);
                }
            }
        }
        if !top_view {
            return;
        }
        let Some(mb) = self.score.master_bars.get(bx.index) else {
            return;
        };
        if let (Some(m), Some(base)) = (&mb.marker, lines.marker) {
            let size = 2.2 * sp;
            let mut x = bx.start + 0.2 * sp;
            if !m.letter.is_empty() {
                let w = p.text_width(&m.letter, size);
                let pad = 0.35 * sp;
                let rect = Rect::new(
                    x,
                    base - 0.75 * size - pad,
                    w + 2.0 * pad,
                    0.75 * size + 2.0 * pad,
                );
                let path = Path::new()
                    .move_to(rect.x, rect.y)
                    .line_to(rect.right(), rect.y)
                    .line_to(rect.right(), rect.bottom())
                    .line_to(rect.x, rect.bottom())
                    .close();
                list.stroke(path, 0.12 * sp, Paint::Ink);
                p.text(list, &m.letter, size, x + pad, base, Paint::Ink);
                x = rect.right() + 0.5 * sp;
            }
            if !m.text.is_empty() {
                p.text(list, &m.text, size, x, base, Paint::Ink);
            }
        }
        if let Some(base) = lines.tempo {
            let mut marks: Vec<(Ratio, f64)> = mb
                .tempo_changes
                .iter()
                .map(|t| (t.position, t.bpm))
                .collect();
            if bx.index == 0 && !marks.iter().any(|(pos, _)| pos.is_zero()) {
                marks.insert(0, (Ratio::ZERO, self.score.initial_tempo));
            }
            // Only real changes are shown, and a mark that would run into
            // the one before it in the bar is left out.
            let mut current = (bx.index > 0).then(|| tempo_before(self.score, bx.index));
            let mut free_from = f32::NEG_INFINITY;
            for (pos, bpm) in marks {
                if !(bpm.is_finite() && bpm > 0.0)
                    || current.is_some_and(|c| c.round() == bpm.round())
                {
                    continue;
                }
                let x = if pos.is_zero() {
                    bx.notes
                } else {
                    bx.x_of(grid, pos) - hw / 2.0
                };
                if x < free_from {
                    continue;
                }
                let scale = 0.7;
                p.glyph_scaled(list, glyphs::NOTE_QUARTER_UP, scale, x, base, Paint::Ink);
                let w = p.width(glyphs::NOTE_QUARTER_UP) * scale;
                let label = format!(" = {}", bpm.round());
                let size = 1.9 * sp;
                p.text(list, &label, size, x + w, base, Paint::Ink);
                free_from = x + w + p.text_width(&label, size) + sp;
                current = Some(bpm);
            }
        }
        if let Some(base) = lines.ending {
            let line_y = base - 1.6 * sp;
            let t = 0.12 * sp;
            let hook = 1.5 * sp;
            let mask = mb.alternate_endings;
            if mask != 0 {
                let previous = bx
                    .index
                    .checked_sub(1)
                    .and_then(|j| self.score.master_bars.get(j))
                    .map(|m| m.alternate_endings);
                let next = self
                    .score
                    .master_bars
                    .get(bx.index + 1)
                    .map(|m| m.alternate_endings);
                let starts = previous != Some(mask) || bx.cols.is_empty();
                let x0 = bx.start + 0.3 * sp;
                let x1 = bx.end - 0.3 * sp;
                if starts {
                    p.vline(list, x0, line_y, line_y + hook, t);
                    let label: Vec<String> = (0..16)
                        .filter(|k| mask & (1 << k) != 0)
                        .map(|k| format!("{}.", k + 1))
                        .collect();
                    p.text(
                        list,
                        &label.join(" "),
                        1.7 * sp,
                        x0 + 0.4 * sp,
                        line_y + 1.5 * sp,
                        Paint::Ink,
                    );
                }
                p.hline(list, if starts { x0 } else { bx.start }, x1, line_y, t);
                if next != Some(mask) && mb.repeat_end.is_some() {
                    p.vline(list, x1, line_y, line_y + hook, t);
                }
            }
            if let Some(n) = mb.repeat_end.filter(|n| *n > 2) {
                p.text_right(list, &format!("×{n}"), 1.7 * sp, bx.end, base, Paint::Ink);
            }
        }
    }
}

/// A stem: its x, where it joins the notes, where it ends, the y of the
/// note furthest along it, and its direction.
#[derive(Clone, Copy, Debug)]
struct Stem {
    x: f32,
    base: f32,
    tip: f32,
    far: f32,
    up: bool,
}

/// A diamond notehead for a natural harmonic.
fn diamond_head(value: NoteValue) -> G {
    match value {
        NoteValue::DoubleWhole | NoteValue::Whole => glyphs::NOTEHEAD_DIAMOND_WHOLE,
        NoteValue::Half => glyphs::NOTEHEAD_DIAMOND_HALF,
        _ => glyphs::NOTEHEAD_DIAMOND_BLACK,
    }
}

/// Tremolo-picking strokes for a beat: one for eighths, two for sixteenths,
/// three for shorter.
fn tremolo_glyph(b: &BeatInfo) -> Option<G> {
    let value = b
        .effects
        .tremolo_picking
        .or_else(|| b.notes.iter().find_map(|n| n.effects.tremolo_picking))?;
    let strokes: usize = match value {
        NoteValue::DoubleWhole | NoteValue::Whole | NoteValue::Half | NoteValue::Quarter => 0,
        NoteValue::Eighth => 1,
        NoteValue::Sixteenth => 2,
        _ => 3,
    };
    strokes
        .checked_sub(1)
        .and_then(|i| glyphs::TREMOLO.get(i))
        .copied()
}

/// The voices of a bar that have something to draw, by index.
fn shown_voices(plan: &BarPlan) -> Vec<usize> {
    plan.voices
        .iter()
        .enumerate()
        .filter(|(_, v)| v.iter().any(|b| b.kind != BeatKind::Empty))
        .map(|(i, _)| i)
        .collect()
}

/// `text`, shortened with an ellipsis if it is wider than `room` points.
fn fit_text(p: &Painter<'_>, text: &str, size: f32, room: f32) -> String {
    if p.text_width(text, size) <= room {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>().trim_end().to_owned() + "…";
        if p.text_width(&candidate, size) <= room {
            return candidate;
        }
    }
    String::new()
}

/// The glyphs of a tuplet's mark: its number of notes ("3") when it takes
/// the usual time, the full ratio ("5:3") otherwise, so any grouping reads
/// unambiguously.
fn tuplet_label(t: Tuplet) -> Vec<G> {
    let digits = |n: u8| -> Vec<G> {
        n.to_string()
            .chars()
            .filter_map(|c| c.to_digit(10))
            .map(glyphs::tuplet_digit)
            .collect()
    };
    let mut out = digits(t.notes);
    if !usual_tuplet(t) {
        out.push(glyphs::TUPLET_COLON);
        out.extend(digits(t.in_time_of));
    }
    out
}

/// Whether a tuplet takes the time readers assume from its number alone:
/// the power of two just below it (3:2, 5:4, 6:4, 7:4, 9:8 … 15:8), or 3
/// for duplets and quadruplets (2:3, 4:3).
fn usual_tuplet(t: Tuplet) -> bool {
    let n = u32::from(t.notes);
    let m = u32::from(t.in_time_of);
    match n {
        0 | 1 => true,
        2 | 4 => m == 3,
        _ => m.is_power_of_two() && m < n && n < 2 * m,
    }
}

/// Consecutive beats that form one tuplet: they share the ratio and add up
/// to its number of notes of the shortest value among them.
fn tuplet_groups(beats: &[BeatInfo]) -> Vec<(std::ops::Range<usize>, Tuplet)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < beats.len() {
        let Some(t) = beats.get(i).and_then(|b| b.onset.and(b.tuplet)) else {
            i += 1;
            continue;
        };
        let start = i;
        let mut written = Ratio::ZERO;
        let mut shortest = NoteValue::DoubleWhole;
        while let Some(b) = beats.get(i) {
            if b.tuplet != Some(t) || b.onset.is_none() {
                break;
            }
            written = written + dotted(b.value, b.dots);
            shortest = shortest.max(b.value);
            i += 1;
            let full = shortest
                .length()
                .checked_mul(Ratio::from_integer(i64::from(t.notes)))
                .unwrap_or(Ratio::ZERO);
            if written >= full {
                break;
            }
        }
        out.push((start..i, t));
    }
    out
}

/// The written length of a note value with its dots.
fn dotted(value: NoteValue, dots: u8) -> Ratio {
    let base = value.length();
    let mut total = base;
    let mut add = base;
    for _ in 0..dots.min(4) {
        add = add
            .checked_div(Ratio::from_integer(2))
            .unwrap_or(Ratio::ZERO);
        total = total + add;
    }
    total
}

/// The clef glyph and the staff step of its reference line.
fn clef_glyph(clef: Clef) -> (G, i32) {
    match clef {
        Clef::Treble => (glyphs::G_CLEF, 2),
        Clef::Treble8vb => (glyphs::G_CLEF_8VB, 2),
        Clef::Bass => (glyphs::F_CLEF, 6),
        Clef::Bass8vb => (glyphs::F_CLEF_8VB, 6),
        Clef::Alto => (glyphs::C_CLEF, 4),
        Clef::Tenor => (glyphs::C_CLEF, 6),
        Clef::Neutral => (glyphs::PERCUSSION_CLEF, 4),
    }
}

/// Staff steps of the naturals that cancel the old key's accidentals when
/// the key changes from `old` to `new` fifths.
fn cancelled_steps(old: i8, new: i8, clef: Clef) -> Vec<i32> {
    let kept = if old.signum() == new.signum() {
        usize::from(new.unsigned_abs())
    } else {
        0
    };
    key_signature_steps_for(old, clef)
        .into_iter()
        .skip(kept)
        .map(|(step, _)| step)
        .collect()
}

fn key_signature_steps_for(fifths: i8, clef: Clef) -> Vec<(i32, i8)> {
    if clef == Clef::Neutral {
        return Vec::new();
    }
    key_signature_steps(
        oxiguitar_model::KeySignature {
            fifths,
            mode: oxiguitar_model::KeyMode::Major,
        },
        clef,
    )
}

/// The tempo in force when bar `index` starts.
fn tempo_before(score: &Score, index: usize) -> f64 {
    score
        .master_bars
        .iter()
        .take(index)
        .rev()
        .flat_map(|mb| mb.tempo_changes.iter().rev())
        .map(|t| t.bpm)
        .find(|b| b.is_finite() && *b > 0.0)
        .unwrap_or(score.initial_tempo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuplets_show_a_ratio_only_when_unusual() {
        let t = |notes, in_time_of| Tuplet { notes, in_time_of };
        for usual in [
            t(3, 2),
            t(5, 4),
            t(6, 4),
            t(7, 4),
            t(9, 8),
            t(13, 8),
            t(2, 3),
            t(4, 3),
        ] {
            assert!(usual_tuplet(usual), "{usual:?}");
            assert_eq!(tuplet_label(usual).len(), usual.notes.to_string().len());
        }
        for unusual in [t(5, 3), t(3, 4), t(6, 8), t(7, 6), t(4, 2), t(11, 4)] {
            assert!(!usual_tuplet(unusual), "{unusual:?}");
        }
        let label = tuplet_label(t(10, 7));
        assert_eq!(label.len(), 4); // 1 0 : 7
        assert_eq!(label.get(2), Some(&glyphs::TUPLET_COLON));
    }

    #[test]
    fn dotted_lengths() {
        let r = |n, d| Ratio::new(n, d).unwrap_or(Ratio::ZERO);
        assert_eq!(dotted(NoteValue::Quarter, 0), r(1, 4));
        assert_eq!(dotted(NoteValue::Quarter, 1), r(3, 8));
        assert_eq!(dotted(NoteValue::Quarter, 2), r(7, 16));
    }
}
