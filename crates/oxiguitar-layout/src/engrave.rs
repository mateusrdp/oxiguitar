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
use oxiguitar_model::{BeatKind, Clef, NoteValue, Ratio, Score, Tuplet};

use crate::draw::{G, Painter, accidental_glyph, glyphs};
use crate::grid::BarGrid;
use crate::pitch::key_signature_steps;
use crate::plan::{BarPlan, BeatInfo, chord_name_size, notehead, rest_glyph, voice_stems_up};
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
struct Band {
    /// Top line of the standard staff.
    std_top: Option<f32>,
    /// Top line (string 1) of the tablature.
    tab_top: Option<f32>,
    /// Top and bottom lines of the band's staves.
    top: f32,
    bottom: f32,
    lines: Lines,
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
        let mut stack = if v.show_std {
            ((high - 8).max(0) as f32) * sp / 2.0 + sp
        } else {
            sp
        };
        if first {
            stack = stack.max(2.5 * sp);
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
        let tab_top = v.show_tab.then_some(if v.show_std {
            top + 4.0 * sp + below_std + 2.0 * sp
        } else {
            top
        });
        let strings = v.strings.max(1);
        let bottom = match tab_top {
            Some(t) => t + (strings - 1) as f32 * tg,
            None => top + 4.0 * sp,
        };
        let after = if tab_top.is_some() {
            if v.show_std {
                1.5 * sp
            } else {
                TAB_RHYTHM * sp + 2.5 * sp
            }
        } else {
            below_std + 1.5 * sp
        };
        bands.push(Band {
            std_top,
            tab_top,
            top,
            bottom,
            lines,
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
            let head = notehead(b.value, n.dead);
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
            if let Some(alter) = n.accidental {
                let g = accidental_glyph(alter);
                let column_w = p.width(glyphs::SHARP) + 0.15 * sp;
                let edge = if shifted_left { left - hw } else { left };
                let ax = edge - 0.2 * sp - n.accidental_column as f32 * column_w - p.width(g);
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
        let digits: Vec<G> = t
            .notes
            .to_string()
            .chars()
            .filter_map(|c| c.to_digit(10))
            .map(glyphs::tuplet_digit)
            .collect();
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

    /// Rhythm on the tablature when standard notation is hidden (VIEW-2):
    /// stems below the staff, flat beams or flags at their ends, dots,
    /// tuplet numbers, and rests in the middle of the staff. Only the first
    /// voice, as Guitar Pro shows it.
    fn tab_rhythm(
        &self,
        list: &mut DisplayList,
        plan: &BarPlan,
        grid: &BarGrid,
        bx: &BarX,
        top: f32,
    ) {
        let p = self.p;
        let sp = self.sp();
        let Some(beats) = plan.voices.first() else {
            return;
        };
        let bottom = self.tab_bottom(top);
        let tip = bottom + TAB_RHYTHM * sp;
        let t = p.ed().stem_thickness * sp;
        let xs: Vec<Option<f32>> = beats
            .iter()
            .map(|b| b.onset.map(|o| bx.x_of(grid, o)))
            .collect();
        for (b, x) in beats.iter().zip(&xs) {
            let Some(x) = *x else { continue };
            match b.kind {
                BeatKind::Notes => {
                    if b.value >= NoteValue::Half {
                        // From just under the lowest fret number.
                        let lowest = b
                            .notes
                            .iter()
                            .filter_map(|n| n.tab.as_ref().map(|(s, _)| *s))
                            .max()
                            .map_or(bottom, |s| top + f32::from(s.saturating_sub(1)) * self.tg);
                        let from = if b.value == NoteValue::Half {
                            tip - 1.5 * sp
                        } else {
                            lowest + 0.6 * self.tg
                        };
                        p.vline(list, x, from, tip, t);
                        let beams = beam_count(b.value);
                        if b.group.is_none()
                            && beams > 0
                            && let Some(f) = glyphs::FLAGS_DOWN.get(beams.min(4) - 1)
                        {
                            p.glyph(list, *f, x - t / 2.0, tip, Paint::Ink);
                        }
                    }
                    for d in 0..b.dots {
                        let dx = x + 0.5 * sp + f32::from(d) * 0.5 * sp;
                        p.glyph(
                            list,
                            glyphs::AUGMENTATION_DOT,
                            dx,
                            tip - 1.0 * sp,
                            Paint::Ink,
                        );
                    }
                }
                BeatKind::Rest => {
                    let g = rest_glyph(b.value);
                    let w = p.width(g);
                    let mid = (top + bottom) / 2.0;
                    let y = if b.value <= NoteValue::Whole {
                        mid - sp
                    } else {
                        mid
                    };
                    p.glyph(list, g, x - w / 2.0, y, Paint::Ink);
                    for d in 0..b.dots {
                        let dx = x + w / 2.0 + 0.3 * sp + f32::from(d) * 0.5 * sp;
                        p.glyph(
                            list,
                            glyphs::AUGMENTATION_DOT,
                            dx,
                            mid - 0.5 * sp,
                            Paint::Ink,
                        );
                    }
                }
                BeatKind::Empty => {}
            }
        }
        // Flat beams at the stem ends, secondary beams above them.
        let h = p.ed().beam_thickness * sp;
        let step = h + 0.25 * sp;
        let hook = p.width(glyphs::NOTEHEAD_BLACK);
        for g in plan.groups.first().map_or(&[][..], Vec::as_slice) {
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
                let y = tip - h / 2.0 - level as f32 * step;
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
            let digits: Vec<G> = tuplet
                .notes
                .to_string()
                .chars()
                .filter_map(|c| c.to_digit(10))
                .map(glyphs::tuplet_digit)
                .collect();
            let w: f32 = digits.iter().map(|g| p.width(*g)).sum();
            let above = digits.first().map_or(sp, |g| p.height(*g).0);
            let mut x = (xa + xb) / 2.0 - w / 2.0;
            for g in &digits {
                p.glyph(list, *g, x, tip + 0.4 * sp + above, Paint::Ink);
                x += p.width(*g);
            }
        }
    }

    /// Fret numbers on the tablature.
    fn tab_bar(&self, list: &mut DisplayList, plan: &BarPlan, grid: &BarGrid, bx: &BarX, top: f32) {
        for beats in &plan.voices {
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
        let mut chord_right = f32::NEG_INFINITY;
        let mut text_right = f32::NEG_INFINITY;
        for beats in &plan.voices {
            for b in beats {
                let Some(onset) = b.onset else { continue };
                let x = bx.x_of(grid, onset) - hw / 2.0;
                if let (Some(name), Some(base)) = (&b.chord, lines.chord) {
                    let x = x.max(chord_right + 0.4 * sp);
                    chord_right = x + p.text(list, name, chord_name_size(sp), x, base, Paint::Ink);
                }
                if let (false, Some(base)) = (b.text.is_empty(), lines.text) {
                    let x = x.max(text_right + 0.4 * sp);
                    text_right = x + p.text(list, &b.text, 1.8 * sp, x, base, Paint::Ink);
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
            for (pos, bpm) in marks {
                if !(bpm.is_finite() && bpm > 0.0) {
                    continue;
                }
                let x = if pos.is_zero() {
                    bx.notes
                } else {
                    bx.x_of(grid, pos) - hw / 2.0
                };
                let scale = 0.7;
                p.glyph_scaled(list, glyphs::NOTE_QUARTER_UP, scale, x, base, Paint::Ink);
                let w = p.width(glyphs::NOTE_QUARTER_UP) * scale;
                let label = format!(" = {}", bpm.round());
                p.text(list, &label, 1.9 * sp, x + w, base, Paint::Ink);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_lengths() {
        let r = |n, d| Ratio::new(n, d).unwrap_or(Ratio::ZERO);
        assert_eq!(dotted(NoteValue::Quarter, 0), r(1, 4));
        assert_eq!(dotted(NoteValue::Quarter, 1), r(3, 8));
        assert_eq!(dotted(NoteValue::Quarter, 2), r(7, 16));
    }
}
