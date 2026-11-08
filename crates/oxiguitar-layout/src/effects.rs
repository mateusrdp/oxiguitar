// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Effects and marks drawn after the notes of a system (TAB-1, NOT-3):
//! hammer-ons and pull-offs, slides, bends, vibrato, trills, palm mute,
//! let ring, harmonics, techniques, strums, articulations, dynamics and
//! lyrics.
//!
//! Marks that run along the music sit in lanes of their own above the
//! tablature (or above the staff when there is no tablature); see
//! [`crate::engrave::Lane`].

use oxiguitar_display::{DisplayList, Paint, Path, Point};
use oxiguitar_model::{
    BeatTechnique, Dynamic, Fade, Golpe, Hairpin, Harmonic, Ornament, Ottava, Pedal, Slide,
    StrokeDirection, Vibrato, Wah,
};

use crate::draw::{G, Painter, glyphs};
use crate::engrave::{Band, Lane};
use crate::plan::{BeatInfo, NoteInfo, lyric_size};

/// A beat placed in its system.
pub(crate) struct Mark<'a> {
    pub(crate) voice: usize,
    /// Centre of the beat's column.
    pub(crate) x: f32,
    /// Where the next column (or the bar line) is.
    pub(crate) next_x: f32,
    pub(crate) beat: &'a BeatInfo,
}

/// Where a view's staves are, for the effects pass.
pub(crate) struct Geometry<'a> {
    pub(crate) band: &'a Band,
    /// Distance between tab lines.
    pub(crate) tg: f32,
    /// The system's right edge.
    pub(crate) right: f32,
}

impl Geometry<'_> {
    fn string_y(&self, string: u8) -> Option<f32> {
        self.band
            .tab_top
            .map(|t| t + f32::from(string.saturating_sub(1)) * self.tg)
    }

    fn step_y(&self, step: i32, sp: f32) -> Option<f32> {
        self.band
            .std_top
            .map(|t| t + 4.0 * sp - step as f32 * sp / 2.0)
    }
}

/// Draws every effect of one view in one system. `marks` are in time order
/// within each voice.
pub(crate) fn draw(p: &Painter<'_>, list: &mut DisplayList, marks: &[Mark<'_>], g: &Geometry<'_>) {
    let voices: Vec<usize> = {
        let mut v: Vec<usize> = marks.iter().map(|m| m.voice).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    // Marks for the whole staff (ottava lines, barres, hairpins) come from
    // the first voice that has them, so two voices don't draw them twice.
    let first_with = |f: fn(&BeatInfo) -> bool| {
        voices
            .iter()
            .copied()
            .find(|v| marks.iter().any(|m| m.voice == *v && f(m.beat)))
    };
    let ottava_voice = first_with(|b| b.ottava.is_some());
    let barre_voice = first_with(|b| b.barre.is_some());
    let hairpin_voice = first_with(|b| b.hairpin.is_some());
    for voice in voices {
        let mine: Vec<&Mark<'_>> = marks.iter().filter(|m| m.voice == voice).collect();
        connections(p, list, &mine, g);
        slurs(p, list, &mine, g);
        runs(p, list, &mine, g);
        if ottava_voice == Some(voice) {
            ottavas(p, list, &mine, g);
        }
        if barre_voice == Some(voice) {
            barres(p, list, &mine, g);
        }
        if hairpin_voice == Some(voice) {
            hairpins(p, list, &mine, g);
        }
    }
    for m in marks {
        single(p, list, m, g);
    }
}

/// Runs of consecutive marks of one voice that `same` says belong
/// together, each as (first index, last index).
fn spans<T: PartialEq>(
    marks: &[&Mark<'_>],
    of: impl Fn(&BeatInfo) -> Option<T>,
) -> Vec<(usize, usize, T)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < marks.len() {
        let Some(v) = marks.get(i).and_then(|m| of(m.beat)) else {
            i += 1;
            continue;
        };
        let start = i;
        while marks
            .get(i + 1)
            .and_then(|m| of(m.beat))
            .is_some_and(|w| w == v)
        {
            i += 1;
        }
        out.push((start, i, v));
        i += 1;
    }
    out
}

/// Slurs: from a beat to the next beat of its voice, over the noteheads on
/// the side away from the stems, and over the tab's numbers; one that
/// carries on past the system ends at its edge, one that comes in starts
/// before the first beat.
fn slurs(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    let mut i = 0;
    while i < marks.len() {
        let Some(m) = marks.get(i) else { break };
        let comes_in = i == 0 && m.beat.slur_in;
        if !m.beat.slur && !comes_in {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = if m.beat.slur { i + 1 } else { i };
        while marks.get(end).is_some_and(|x| x.beat.slur) {
            end += 1;
        }
        let first = marks.get(start).copied();
        let last = marks.get(end).copied();
        let (Some(first), last) = (first, last) else {
            break;
        };
        // Its ends: the notes it joins, or the system's edges.
        let open_left = comes_in;
        let x0 = if open_left {
            first.x - 2.0 * sp
        } else {
            first.x
        };
        let x1 = last.map_or(g.right - 0.5 * sp, |l| l.x);
        if x1 <= x0 {
            i = end.max(i + 1);
            continue;
        }
        let up = first.beat.up;
        let dir = if up { 1.0 } else { -1.0 };
        let reach = |m: Option<&&Mark<'_>>| {
            m.and_then(|m| {
                let n = if up {
                    m.beat.notes.first()
                } else {
                    m.beat.notes.last()
                }?;
                g.step_y(n.step, sp)
            })
        };
        if g.band.std_top.is_some() {
            let ya = reach(Some(&first)).unwrap_or(0.0) + dir * 0.9 * sp;
            let yb = reach(last.as_ref()).unwrap_or(ya - dir * 0.9 * sp) + dir * 0.9 * sp;
            let (ya, yb) = (ya.max(yb).min(ya), yb);
            let height = dir * (0.5 * sp + 0.06 * (x1 - x0)).min(2.0 * sp);
            p.slur(
                list,
                Point::new(x0 + if open_left { 0.0 } else { 0.1 * hw }, ya),
                Point::new(x1, yb),
                height,
            );
        } else if let Some(t) = g.band.tab_top {
            let y = t - 0.6 * g.tg;
            let height = -(0.5 * sp + 0.05 * (x1 - x0)).min(1.6 * sp);
            p.slur(list, Point::new(x0, y), Point::new(x1, y), height);
        }
        i = end.max(i + 1);
    }
}

/// Ottava lines: "8va" (and the rest) with a dashed line and a hook at the
/// end of the run, above the staff.
fn ottavas(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let Some(base) = g.band.lines.ottava else {
        return;
    };
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    for (a, b, o) in spans(marks, |b| b.ottava) {
        let (Some(first), Some(last)) = (marks.get(a), marks.get(b)) else {
            continue;
        };
        let gl = match o {
            Ottava::Alta8 => glyphs::OTTAVA_ALTA,
            Ottava::Bassa8 => glyphs::OTTAVA_BASSA,
            Ottava::Alta15 => glyphs::QUINDICESIMA_ALTA,
            Ottava::Bassa15 => glyphs::QUINDICESIMA_BASSA,
        };
        let x0 = first.x - hw / 2.0;
        let x1 = (last.next_x - 0.4 * sp).max(x0 + 2.0 * sp);
        p.glyph_scaled(list, gl, 0.8, x0, base, Paint::Ink);
        let w = p.width(gl) * 0.8;
        let y = base - 0.6 * sp;
        dashes(p, list, x0 + w + 0.3 * sp, x1, y);
        let hook = match o {
            Ottava::Alta8 | Ottava::Alta15 => 0.8 * sp,
            Ottava::Bassa8 | Ottava::Bassa15 => -0.8 * sp,
        };
        p.vline(list, x1, y, y + hook, 0.1 * sp);
    }
}

/// Extended barres: "C" (or "½C") and the fret in Roman numerals, with a
/// dashed line over the run.
fn barres(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let Some(base) = g.band.lines.barre else {
        return;
    };
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    for (a, b, barre) in spans(marks, |b| b.barre) {
        let (Some(first), Some(last)) = (marks.get(a), marks.get(b)) else {
            continue;
        };
        let label = format!(
            "{}C{}",
            if barre.half { "½" } else { "" },
            roman(barre.fret)
        );
        let x0 = first.x - hw / 2.0;
        let x1 = (last.next_x - 0.4 * sp).max(x0 + 2.0 * sp);
        dashed_label(p, list, &label, x0, x1, base, a == b);
    }
}

/// A fret in Roman numerals, as barres are marked (V, VII, XII).
pub(crate) fn roman(n: u8) -> String {
    const PARTS: [(u8, &str); 9] = [
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
        (0, ""),
        (0, ""),
        (0, ""),
    ];
    let mut n = n;
    let mut out = String::new();
    for (value, text) in PARTS {
        if value == 0 {
            break;
        }
        while n >= value {
            out.push_str(text);
            n -= value;
        }
    }
    out
}

/// Hairpins: wedges on the dynamics line, from the first beat of each run
/// to the end of its last.
fn hairpins(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let Some(base) = g.band.dynamics else {
        return;
    };
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    for (a, b, h) in spans(marks, |b| b.hairpin) {
        let (Some(first), Some(last)) = (marks.get(a), marks.get(b)) else {
            continue;
        };
        // Clear of a dynamic at its start.
        let start = first.x - hw / 2.0
            + if first.beat.show_dynamic.is_some() {
                2.6 * sp
            } else {
                0.0
            };
        let end = (last.next_x - 0.5 * sp).max(start + 1.5 * sp);
        let y = base - 0.45 * sp;
        let open = 0.55 * sp;
        let (narrow, wide) = match h {
            Hairpin::Crescendo => (start, end),
            Hairpin::Decrescendo => (end, start),
        };
        p.line_between(
            list,
            Point::new(narrow, y),
            Point::new(wide, y - open),
            0.1 * sp,
        );
        p.line_between(
            list,
            Point::new(narrow, y),
            Point::new(wide, y + open),
            0.1 * sp,
        );
    }
}

/// A dashed line from `x0` to `x1` at `y`.
fn dashes(p: &Painter<'_>, list: &mut DisplayList, x0: f32, x1: f32, y: f32) {
    let sp = p.sp;
    let mut x = x0;
    while x < x1 {
        p.hline(list, x, (x + 0.6 * sp).min(x1), y, 0.1 * sp);
        x += 1.0 * sp;
    }
}

/// Half the width of a note's tab number, knock-out included.
fn tab_half(p: &Painter<'_>, g: &Geometry<'_>, n: &NoteInfo) -> f32 {
    n.tab
        .as_ref()
        .map_or(0.0, |(_, label)| p.tab_number_width(g.tg, label) / 2.0)
}

/// The next note on the same string after mark `i` in the same voice.
fn next_on_string<'m, 'a>(
    marks: &[&'m Mark<'a>],
    i: usize,
    string: u8,
) -> Option<(&'m Mark<'a>, &'a NoteInfo)> {
    marks.iter().skip(i + 1).find_map(|m| {
        m.beat
            .notes
            .iter()
            .find(|n| n.tab.as_ref().is_some_and(|(s, _)| *s == string))
            .map(|n| (*m, n))
    })
}

/// Hammer-ons, pull-offs and slides: from a note to the next note on the
/// same string (to the end of the system if that note is on a later line).
fn connections(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    for (i, m) in marks.iter().enumerate() {
        for n in &m.beat.notes {
            let hp = n.effects.hammer_pull;
            let slide_to = n
                .effects
                .slides
                .iter()
                .find(|s| matches!(s, Slide::Shift | Slide::Legato))
                .copied();
            let string = n.tab.as_ref().map(|(s, _)| *s);
            let target = string.and_then(|s| next_on_string(marks, i, s));
            if hp || slide_to.is_some() {
                let (nx, next) = match target {
                    Some((t, tn)) => (Some(t.x), Some(tn)),
                    None => (None, None),
                };
                // On the tablature.
                if let Some(y) = string.and_then(|s| g.string_y(s)) {
                    let a = m.x + tab_half(p, g, n);
                    let b = nx.map_or(g.right - 0.5 * sp, |x| {
                        x - next.map_or(0.0, |t| tab_half(p, g, t))
                    });
                    if b > a {
                        if let Some(kind) = slide_to {
                            let rising = match (n.fret, next.and_then(|t| t.fret)) {
                                (Some(f), Some(t)) => t >= f,
                                _ => true,
                            };
                            let d = if rising { 1.0 } else { -1.0 };
                            p.line_between(
                                list,
                                Point::new(a + 0.1 * sp, y + d * 0.25 * g.tg),
                                Point::new(b - 0.1 * sp, y - d * 0.25 * g.tg),
                                0.12 * sp,
                            );
                            if kind == Slide::Legato {
                                p.slur(
                                    list,
                                    Point::new(a, y - 0.5 * g.tg),
                                    Point::new(b, y - 0.5 * g.tg),
                                    -0.5 * sp,
                                );
                            }
                        } else {
                            p.slur(
                                list,
                                Point::new(a, y - 0.5 * g.tg),
                                Point::new(b, y - 0.5 * g.tg),
                                -0.5 * sp,
                            );
                            if let Some(lane) = g.band.lane(Lane::Hp) {
                                let up = match (n.fret, next.and_then(|t| t.fret)) {
                                    (Some(f), Some(t)) => t >= f,
                                    _ => true,
                                };
                                let letter = if up { "H" } else { "P" };
                                p.text_centred(
                                    list,
                                    letter,
                                    1.5 * sp,
                                    (a + b) / 2.0,
                                    lane.base,
                                    Paint::Ink,
                                );
                            }
                        }
                    }
                }
                // On the staff: a slur, or a straight line for a slide.
                if let (Some(y), Some(top)) = (g.step_y(n.step, sp), g.band.std_top) {
                    let _ = top;
                    let a = m.x + hw / 2.0 + 0.1 * sp;
                    let target_y = next.and_then(|t| g.step_y(t.step, sp)).unwrap_or(y);
                    let b = nx.map_or(g.right - 0.5 * sp, |x| x - hw / 2.0 - 0.1 * sp);
                    if b > a {
                        if slide_to.is_some() {
                            p.line_between(
                                list,
                                Point::new(a + 0.2 * sp, y),
                                Point::new(b - 0.2 * sp, target_y),
                                0.12 * sp,
                            );
                        }
                        if hp || slide_to == Some(Slide::Legato) {
                            let dir = if m.beat.up { 1.0 } else { -1.0 };
                            let off = dir * 0.7 * sp;
                            p.slur(
                                list,
                                Point::new(a - 0.3 * sp, y + off),
                                Point::new(b + 0.3 * sp, target_y + off),
                                dir * (0.4 * sp + 0.05 * (b - a)).min(1.2 * sp),
                            );
                        }
                    }
                }
            }
            // Slides into and out of the note.
            for s in &n.effects.slides {
                let (before, rising) = match s {
                    Slide::InFromBelow => (true, true),
                    Slide::InFromAbove => (true, false),
                    Slide::OutUp => (false, true),
                    Slide::OutDown => (false, false),
                    Slide::Shift | Slide::Legato => continue,
                };
                let d = if rising { 1.0 } else { -1.0 };
                if let Some(y) = string.and_then(|s| g.string_y(s)) {
                    let half = tab_half(p, g, n);
                    let (x0, x1) = if before {
                        (m.x - half - 1.3 * sp, m.x - half - 0.15 * sp)
                    } else {
                        (m.x + half + 0.15 * sp, m.x + half + 1.3 * sp)
                    };
                    p.line_between(
                        list,
                        Point::new(x0, y + d * 0.3 * g.tg),
                        Point::new(x1, y - d * 0.3 * g.tg),
                        0.12 * sp,
                    );
                }
                if let Some(y) = g.step_y(n.step, sp) {
                    let (x0, x1) = if before {
                        (m.x - hw / 2.0 - 1.4 * sp, m.x - hw / 2.0 - 0.2 * sp)
                    } else {
                        (m.x + hw / 2.0 + 0.2 * sp, m.x + hw / 2.0 + 1.4 * sp)
                    };
                    p.line_between(
                        list,
                        Point::new(x0, y + d * 0.6 * sp),
                        Point::new(x1, y - d * 0.6 * sp),
                        0.12 * sp,
                    );
                }
            }
        }
    }
}

/// Marks that run along consecutive beats: vibrato, trill, palm mute, let
/// ring.
fn runs(p: &Painter<'_>, list: &mut DisplayList, marks: &[&Mark<'_>], g: &Geometry<'_>) {
    let sp = p.sp;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);
    type Test = fn(&BeatInfo) -> bool;
    let kinds: [(Lane, Test); 6] = [
        (Lane::Vibrato, |b| {
            b.notes.iter().any(|n| n.effects.vibrato.is_some())
                && b.effects.bridge_vibrato.is_none()
        }),
        (Lane::Vibrato, |b| b.effects.bridge_vibrato.is_some()),
        (Lane::Trill, |b| {
            b.notes.iter().any(|n| n.effects.trill.is_some())
        }),
        (Lane::PalmMute, |b| {
            b.notes.iter().any(|n| n.effects.palm_mute)
        }),
        (Lane::LetRing, |b| {
            b.effects.let_ring || b.notes.iter().any(|n| n.effects.let_ring)
        }),
        (Lane::Vibrato, |_| false),
    ];
    for (k, (lane, test)) in kinds.iter().enumerate() {
        let Some(pos) = g.band.lane(*lane) else {
            continue;
        };
        let mut i = 0;
        while i < marks.len() {
            if !marks.get(i).is_some_and(|m| test(m.beat)) {
                i += 1;
                continue;
            }
            let start = i;
            while marks.get(i + 1).is_some_and(|m| test(m.beat)) {
                i += 1;
            }
            let (Some(first), Some(last)) = (marks.get(start), marks.get(i)) else {
                break;
            };
            let x0 = first.x - hw / 2.0;
            let x1 = (last.next_x - 0.4 * sp).max(x0 + 1.5 * sp);
            match k {
                0 | 1 => {
                    // Wide if any beat of the run is.
                    let wide = marks.get(start..=i).is_some_and(|run| {
                        run.iter().any(|m| {
                            if k == 0 {
                                m.beat
                                    .notes
                                    .iter()
                                    .any(|n| n.effects.vibrato == Some(Vibrato::Wide))
                            } else {
                                m.beat.effects.bridge_vibrato == Some(Vibrato::Wide)
                            }
                        })
                    });
                    let g_ = if wide {
                        glyphs::VIBRATO_WIDE
                    } else {
                        glyphs::VIBRATO
                    };
                    wiggle(p, list, g_, x0, x1, pos.base);
                }
                2 => {
                    p.glyph(list, glyphs::TRILL, x0, pos.base, Paint::Ink);
                    let w = p.width(glyphs::TRILL) + 0.2 * sp;
                    wiggle(
                        p,
                        list,
                        glyphs::WIGGLE_TRILL,
                        x0 + w,
                        x1,
                        pos.base - 0.2 * sp,
                    );
                }
                3 => dashed_label(p, list, "P.M.", x0, x1, pos.base, start == i),
                4 => dashed_label(p, list, "let ring", x0, x1, pos.base, start == i),
                _ => {}
            }
            i += 1;
        }
    }
    let _ = g.right;
}

/// A wavy line made of repeated `g` glyphs from `x0` to `x1`.
fn wiggle(p: &Painter<'_>, list: &mut DisplayList, g: G, x0: f32, x1: f32, base: f32) {
    let w = p.width(g).max(0.2 * p.sp);
    let count = (((x1 - x0) / w).floor() as usize).clamp(1, 400);
    for k in 0..count {
        p.glyph(list, g, x0 + k as f32 * w, base, Paint::Ink);
    }
}

/// A label, then a dashed line to `x1` with a short hook down at its end.
fn dashed_label(
    p: &Painter<'_>,
    list: &mut DisplayList,
    label: &str,
    x0: f32,
    x1: f32,
    base: f32,
    single: bool,
) {
    let sp = p.sp;
    let size = 1.6 * sp;
    let w = p.text(list, label, size, x0, base, Paint::Ink);
    if single {
        return;
    }
    let y = base - 0.45 * size;
    let mut x = x0 + w + 0.3 * sp;
    let t = 0.1 * sp;
    while x < x1 {
        let e = (x + 0.6 * sp).min(x1);
        p.hline(list, x, e, y, t);
        x += 1.0 * sp;
    }
    p.vline(list, x1, y, y + 0.6 * sp, t);
}

/// Marks of a single beat: bends, harmonics, techniques, strums,
/// articulations, dynamics, lyrics.
fn single(p: &Painter<'_>, list: &mut DisplayList, m: &Mark<'_>, g: &Geometry<'_>) {
    let sp = p.sp;
    let b = m.beat;
    let hw = p.width(glyphs::NOTEHEAD_BLACK);

    // Bends, on the tablature.
    if let Some(lane) = g.band.lane(Lane::Bend) {
        for n in &b.notes {
            let Some(bend) = n.effects.bend.as_ref() else {
                continue;
            };
            let Some(y0) = n.tab.as_ref().and_then(|(s, _)| g.string_y(*s)) else {
                continue;
            };
            bend_arrows(
                p,
                list,
                m,
                n,
                g,
                y0 - 0.5 * g.tg,
                lane.top + 1.6 * sp,
                &bend.points,
            );
        }
    }

    // Harmonic kinds and techniques, as text in their lanes.
    if let Some(lane) = g.band.lane(Lane::Harmonic) {
        let label = b.notes.iter().find_map(|n| match n.effects.harmonic? {
            Harmonic::Natural => None,
            Harmonic::Artificial { .. } => Some("A.H."),
            Harmonic::Tapped { .. } => Some("T.H."),
            Harmonic::Pinch => Some("P.H."),
            Harmonic::Semi => Some("S.H."),
            Harmonic::Feedback => Some("Fdbk."),
        });
        if let Some(label) = label {
            p.text(list, label, 1.6 * sp, m.x - hw / 2.0, lane.base, Paint::Ink);
        }
    }
    if let Some(lane) = g.band.lane(Lane::Technique) {
        let mut x = m.x - hw / 2.0;
        let put = |text: &str, x: &mut f32, list: &mut DisplayList| {
            *x += p.text(list, text, 1.6 * sp, *x, lane.base, Paint::Ink) + 0.4 * sp;
        };
        let tap = b.notes.iter().any(|n| n.effects.left_hand_tap);
        match b.effects.technique {
            Some(BeatTechnique::Tap) => put("T", &mut x, list),
            Some(BeatTechnique::Slap) => put("S", &mut x, list),
            Some(BeatTechnique::Pop) => put("P", &mut x, list),
            None if tap => put("T", &mut x, list),
            None => {}
        }
        if let Some(fade) = b.effects.fade {
            // Small hairpins: opening for a fade in, closing for a fade
            // out, both for a swell.
            let w = 1.6 * sp;
            let y = lane.base - 0.45 * sp;
            let wedge = |list: &mut DisplayList, x: f32, opening: bool| {
                let (narrow, wide) = if opening { (x, x + w) } else { (x + w, x) };
                for d in [-0.4 * sp, 0.4 * sp] {
                    p.line_between(
                        list,
                        Point::new(narrow, y),
                        Point::new(wide, y + d),
                        0.1 * sp,
                    );
                }
            };
            match fade {
                Fade::In => wedge(list, x, true),
                Fade::Out => wedge(list, x, false),
                Fade::Swell => {
                    wedge(list, x, true);
                    x += w;
                    wedge(list, x, false);
                }
            }
            x += w + 0.4 * sp;
        }
        if let Some(r) = b.effects.rasgueado {
            put(&format!("rasg. {}", r.fingers()), &mut x, list);
        }
        if let Some(wah) = b.effects.wah {
            let gl = match wah {
                Wah::Open => glyphs::WAH_OPEN,
                Wah::Closed => glyphs::WAH_CLOSED,
            };
            p.glyph(list, gl, x, lane.base - 0.2 * sp, Paint::Ink);
            x += p.width(gl) + 0.4 * sp;
        }
        if let Some(golpe) = b.effects.golpe {
            p.glyph(list, glyphs::GOLPE, x, lane.base, Paint::Ink);
            x += p.width(glyphs::GOLPE) + 0.2 * sp;
            if golpe == Golpe::Thumb {
                put("p", &mut x, list);
            } else {
                x += 0.2 * sp;
            }
        }
        if let Some(dir) = b.effects.pick_scrape {
            // A wavy line with an arrow: up the neck, or down to the bridge.
            let w = 2.4 * sp;
            wiggle(p, list, glyphs::VIBRATO, x, x + w, lane.base - 0.1 * sp);
            let (gl, y) = match dir {
                StrokeDirection::Up => (glyphs::ARROWHEAD_UP, lane.base - 0.2 * sp),
                StrokeDirection::Down => (glyphs::ARROWHEAD_DOWN, lane.base - 0.9 * sp),
            };
            p.glyph_scaled(list, gl, 0.4, x + w, y, Paint::Ink);
            x += w + 0.8 * sp;
        }
        if b.effects.dead_slap {
            put("dead slap", &mut x, list);
        }
        if g.band.std_top.is_none() {
            for o in b.notes.iter().filter_map(|n| n.effects.ornament).take(1) {
                let gl = ornament_glyph(o);
                p.glyph(list, gl, x, lane.base, Paint::Ink);
                x += p.width(gl) + 0.4 * sp;
            }
        }
        if let Some(dir) = b.effects.pick_stroke {
            let gl = match dir {
                StrokeDirection::Down => glyphs::DOWN_BOW,
                StrokeDirection::Up => glyphs::UP_BOW,
            };
            p.glyph_scaled(list, gl, 0.8, x, lane.base, Paint::Ink);
        }
    }

    // Whammy-bar curves, in their lane above the tab.
    if let (Some(lane), Some(whammy)) = (g.band.lane(Lane::Whammy), &b.effects.whammy) {
        whammy_curve(p, list, m, lane.top, lane.base, &whammy.points);
    }

    // Arpeggios: a wavy line left of the chord, its arrow the way it rolls.
    if let Some(dir) = b.effects.arpeggio {
        let left = b
            .notes
            .iter()
            .map(|n| tab_half(p, g, n))
            .fold(hw / 2.0, f32::max);
        let spans = [
            b.notes
                .iter()
                .filter_map(|n| n.tab.as_ref().and_then(|(s, _)| g.string_y(*s)))
                .fold(None, extend),
            b.notes
                .iter()
                .filter_map(|n| g.step_y(n.step, sp))
                .fold(None, extend),
        ];
        for (top, bottom) in spans.into_iter().flatten() {
            let x = m.x - left - 1.0 * sp;
            arpeggio(p, list, x, top - 0.6 * sp, bottom + 0.6 * sp, dir);
        }
    }

    // Strums: an arrow left of the chord.
    if let Some(stroke) = b.effects.stroke {
        let tab_span = b
            .notes
            .iter()
            .filter_map(|n| n.tab.as_ref().and_then(|(s, _)| g.string_y(*s)))
            .fold(None, |acc: Option<(f32, f32)>, y| {
                Some(acc.map_or((y, y), |(a, z)| (a.min(y), z.max(y))))
            });
        let std_span = b
            .notes
            .iter()
            .filter_map(|n| g.step_y(n.step, sp))
            .fold(None, |acc: Option<(f32, f32)>, y| {
                Some(acc.map_or((y, y), |(a, z)| (a.min(y), z.max(y))))
            });
        let left = b
            .notes
            .iter()
            .map(|n| tab_half(p, g, n))
            .fold(hw / 2.0, f32::max);
        for (span, pad) in [(tab_span, 0.5 * g.tg), (std_span, 0.6 * sp)] {
            if let Some((top, bottom)) = span {
                let x = m.x - left - 0.6 * sp;
                let (y0, y1) = (top - pad, bottom + pad);
                p.vline(list, x, y0, y1, 0.12 * sp);
                let scale = 0.5;
                let (gl, y) = match stroke.direction {
                    // A down strum starts on the low string: the arrow points
                    // up the page, towards the high strings.
                    StrokeDirection::Down => (
                        glyphs::ARROWHEAD_UP,
                        y0 + p.height(glyphs::ARROWHEAD_UP).0 * scale,
                    ),
                    StrokeDirection::Up => (glyphs::ARROWHEAD_DOWN, y1),
                };
                let w = p.width(gl) * scale;
                p.glyph_scaled(list, gl, scale, x - w / 2.0, y, Paint::Ink);
            }
        }
    }

    // Articulations, on the staff, on the notehead side away from the stem.
    if g.band.std_top.is_some() {
        let accent = b.notes.iter().find_map(|n| n.effects.accent);
        let staccato = b.notes.iter().any(|n| n.effects.staccato);
        if (accent.is_some() || staccato)
            && let (Some(low), Some(high)) = (b.notes.first(), b.notes.last())
        {
            let below = b.up;
            let mut y = if below {
                g.step_y(low.step, sp).unwrap_or(0.0) + 0.9 * sp
            } else {
                g.step_y(high.step, sp).unwrap_or(0.0) - 0.9 * sp
            };
            if staccato {
                let gl = if below {
                    glyphs::STACCATO_BELOW
                } else {
                    glyphs::STACCATO_ABOVE
                };
                let w = p.width(gl);
                p.glyph(list, gl, m.x - w / 2.0, y, Paint::Ink);
                y += if below { 0.8 * sp } else { -0.8 * sp };
            }
            if let Some(a) = accent {
                let gl = match (a, below) {
                    (oxiguitar_model::Accent::Normal, true) => glyphs::ACCENT_BELOW,
                    (oxiguitar_model::Accent::Normal, false) => glyphs::ACCENT_ABOVE,
                    (oxiguitar_model::Accent::Heavy, true) => glyphs::MARCATO_BELOW,
                    (oxiguitar_model::Accent::Heavy, false) => glyphs::MARCATO_ABOVE,
                };
                let w = p.width(gl);
                p.glyph(list, gl, m.x - w / 2.0, y, Paint::Ink);
            }
        }
    }

    // Ornaments over the notes, left-hand fingers before each notehead,
    // right-hand fingers over the chord, string numbers after each note.
    if g.band.std_top.is_some()
        && let (Some(low), Some(high)) = (b.notes.first(), b.notes.last())
    {
        let top_y = g.step_y(high.step.max(8), sp).unwrap_or(0.0);
        let mut y = top_y - 1.2 * sp;
        let _ = low;
        if let Some(o) = b.notes.iter().find_map(|n| n.effects.ornament) {
            let gl = ornament_glyph(o);
            let w = p.width(gl);
            p.glyph(list, gl, m.x - w / 2.0, y, Paint::Ink);
            y -= 1.4 * sp;
        }
        let right: Vec<u8> = b
            .notes
            .iter()
            .rev()
            .filter_map(|n| n.effects.right_finger)
            .collect();
        for (k, f) in right.iter().enumerate() {
            if let Some(gl) = glyphs::RIGHT_FINGERS.get(usize::from(*f)) {
                let w = p.width(*gl);
                p.glyph(
                    list,
                    *gl,
                    m.x - w / 2.0,
                    y - k as f32 * 1.2 * sp,
                    Paint::Ink,
                );
            }
        }
        for n in &b.notes {
            let Some(ny) = g.step_y(n.step, sp) else {
                continue;
            };
            if let Some(f) = n.effects.left_finger {
                let gl = if f == 0 {
                    glyphs::FINGER_THUMB
                } else {
                    glyphs::finger(f)
                };
                let w = p.width(gl) * 0.8;
                let acc = if n.accidental.is_some() {
                    1.2 * sp
                } else {
                    0.0
                };
                p.glyph_scaled(
                    list,
                    gl,
                    0.8,
                    m.x - hw / 2.0 - acc - 0.25 * sp - w,
                    ny + 0.35 * sp,
                    Paint::Ink,
                );
            }
            if n.effects.string_number
                && let Some((string, _)) = &n.tab
            {
                // Under the notehead (over it when the stem points down).
                let cx = m.x;
                let ny = if b.up { ny + 1.6 * sp } else { ny - 1.6 * sp };
                let r = 0.55 * sp;
                circle(list, cx, ny, r, 0.08 * sp);
                p.text_centred(
                    list,
                    &string.to_string(),
                    1.0 * sp,
                    cx,
                    ny + 0.35 * sp,
                    Paint::Ink,
                );
            }
        }
    }

    // Dynamics, pedal marks and lyrics under the staff.
    if let (Some(d), Some(base)) = (b.show_dynamic, g.band.dynamics) {
        let gl = dynamic_glyph(d);
        p.glyph(list, gl, m.x - hw / 2.0, base, Paint::Ink);
    }
    if let (Some(pedal), Some(base)) = (b.effects.pedal, g.band.pedal) {
        let gl = match pedal {
            Pedal::Down => glyphs::PEDAL,
            Pedal::Up => glyphs::PEDAL_UP,
        };
        p.glyph_scaled(list, gl, 0.7, m.x - hw / 2.0, base, Paint::Ink);
    }
    for (line, syllable) in &b.lyrics {
        if let Some(base) = g.band.lyrics.get(*line) {
            p.text_centred(list, syllable, lyric_size(sp), m.x, *base, Paint::Ink);
        }
    }
}

/// The span `(top, bottom)` grown to take in `y`.
fn extend(acc: Option<(f32, f32)>, y: f32) -> Option<(f32, f32)> {
    Some(acc.map_or((y, y), |(a, z)| (a.min(y), z.max(y))))
}

/// A wavy vertical line from `top` to `bottom` at `x`, with an arrowhead
/// at the end it rolls towards: up the page for a downward arpeggio (from
/// the low string), down for an upward one.
fn arpeggio(
    p: &Painter<'_>,
    list: &mut DisplayList,
    x: f32,
    top: f32,
    bottom: f32,
    dir: StrokeDirection,
) {
    let sp = p.sp;
    let amp = 0.25 * sp;
    let step = 0.5 * sp;
    let mut path = Path::new().move_to(x, bottom);
    let mut y = bottom;
    let mut side = 1.0;
    while y - step > top {
        let next = y - step;
        path = path.cubic_to(
            Point::new(x + side * amp, y - step / 3.0),
            Point::new(x + side * amp, next + step / 3.0),
            Point::new(x, next),
        );
        side = -side;
        y = next;
    }
    list.stroke(path, 0.1 * sp, Paint::Ink);
    let scale = 0.45;
    let (gl, tip) = match dir {
        StrokeDirection::Down => (glyphs::ARROWHEAD_UP, top),
        StrokeDirection::Up => (glyphs::ARROWHEAD_DOWN, bottom),
    };
    let w = p.width(gl) * scale;
    let h = p.height(gl).0 * scale;
    let y = match dir {
        StrokeDirection::Down => tip + h,
        StrokeDirection::Up => tip,
    };
    p.glyph_scaled(list, gl, scale, x - w / 2.0, y, Paint::Ink);
}

/// A circle of radius `r` centred on (`cx`, `cy`).
fn circle(list: &mut DisplayList, cx: f32, cy: f32, r: f32, width: f32) {
    // Four cubic arcs: k is the usual control distance for a quarter.
    let k = 0.552_284_8 * r;
    let path = Path::new()
        .move_to(cx + r, cy)
        .cubic_to(
            Point::new(cx + r, cy + k),
            Point::new(cx + k, cy + r),
            Point::new(cx, cy + r),
        )
        .cubic_to(
            Point::new(cx - k, cy + r),
            Point::new(cx - r, cy + k),
            Point::new(cx - r, cy),
        )
        .cubic_to(
            Point::new(cx - r, cy - k),
            Point::new(cx - k, cy - r),
            Point::new(cx, cy - r),
        )
        .cubic_to(
            Point::new(cx + k, cy - r),
            Point::new(cx + r, cy - k),
            Point::new(cx + r, cy),
        )
        .close();
    list.stroke(path, width, Paint::Ink);
}

/// An ornament's symbol.
fn ornament_glyph(o: Ornament) -> G {
    match o {
        Ornament::UpperMordent => glyphs::UPPER_MORDENT,
        Ornament::LowerMordent => glyphs::LOWER_MORDENT,
        Ornament::Turn => glyphs::TURN,
        Ornament::InvertedTurn => glyphs::TURN_INVERTED,
    }
}

/// The whammy bar's curve over a beat: "w/bar", then the curve from its
/// zero line (dips go down, pulls up), with its deepest amount.
fn whammy_curve(
    p: &Painter<'_>,
    list: &mut DisplayList,
    m: &Mark<'_>,
    top: f32,
    base: f32,
    points: &[oxiguitar_model::BendPoint],
) {
    let sp = p.sp;
    let size = 1.3 * sp;
    let x0 = m.x;
    let label_w = p.text(list, "w/bar", size, x0, top + size, Paint::Ink);
    let start = x0 + label_w + 0.3 * sp;
    let span = ((m.next_x - start) * 0.9).max(2.0 * sp);
    let zero = (top + base) / 2.0 + 0.3 * sp;
    let deepest = points
        .iter()
        .map(|pt| pt.cents)
        .max_by_key(|c| c.unsigned_abs())
        .unwrap_or(0);
    let range = deepest.unsigned_abs().max(1) as f32;
    let reach = 1.0 * sp;
    let at = |pt: &oxiguitar_model::BendPoint| {
        let t = pt.offset.to_f64().clamp(0.0, 1.0) as f32;
        Point::new(start + t * span, zero - pt.cents as f32 / range * reach)
    };
    let mut path: Option<Path> = None;
    for pt in points {
        let q = at(pt);
        path = Some(match path {
            None => Path::new().move_to(q.x, q.y),
            Some(path) => path.line_to(q.x, q.y),
        });
    }
    if let Some(path) = path {
        list.stroke(path, 0.1 * sp, Paint::Ink);
    }
    if deepest != 0 {
        let amount = bend_amount(deepest);
        let label = if deepest < 0 {
            format!("-{amount}")
        } else {
            amount
        };
        p.text(
            list,
            &label,
            size,
            start + span + 0.2 * sp,
            zero + 0.4 * sp,
            Paint::Ink,
        );
    }
}

/// Bend and release arrows from a note's tab number: up to `top` for a
/// bend, back down for a release, with the amount above each peak.
#[allow(clippy::too_many_arguments)]
fn bend_arrows(
    p: &Painter<'_>,
    list: &mut DisplayList,
    m: &Mark<'_>,
    n: &NoteInfo,
    g: &Geometry<'_>,
    from_y: f32,
    top: f32,
    points: &[oxiguitar_model::BendPoint],
) {
    let sp = p.sp;
    if points.iter().all(|pt| pt.cents == 0) {
        return;
    }
    let x0 = m.x + tab_half(p, g, n) * 0.6;
    let span = ((m.next_x - m.x) * 0.85).max(2.0 * sp);
    let at = |pt: &oxiguitar_model::BendPoint| {
        let t = pt.offset.to_f64().clamp(0.0, 1.0) as f32;
        let y = if pt.cents == 0 { from_y } else { top };
        Point::new(x0 + t * span, y)
    };
    let t = 0.12 * sp;
    let scale = 0.5;
    let arrow_up = |list: &mut DisplayList, tip: Point| {
        let gl = glyphs::ARROWHEAD_UP;
        let w = p.width(gl) * scale;
        let h = p.height(gl).0 * scale;
        p.glyph_scaled(list, gl, scale, tip.x - w / 2.0, tip.y + h, Paint::Ink);
        h
    };
    let arrow_down = |list: &mut DisplayList, tip: Point| {
        let gl = glyphs::ARROWHEAD_DOWN;
        let w = p.width(gl) * scale;
        let h = p.height(gl).0 * scale;
        p.glyph_scaled(list, gl, scale, tip.x - w / 2.0, tip.y, Paint::Ink);
        h
    };
    let label = |list: &mut DisplayList, x: f32, cents: i32| {
        p.text_centred(
            list,
            &bend_amount(cents),
            1.5 * sp,
            x,
            top - 0.4 * sp,
            Paint::Ink,
        );
    };
    let mut prev: Option<(Point, i32)> = None;
    for pt in points {
        let here = at(pt);
        match prev {
            None if pt.cents != 0 => {
                // A pre-bend: straight up from the number.
                let tip = Point::new(x0, top);
                let h = arrow_up(list, tip);
                p.vline(list, x0, from_y, top + h, t);
                label(list, x0, pt.cents);
                prev = Some((tip, pt.cents));
                continue;
            }
            None => {}
            Some((a, c)) if pt.cents > c => {
                let h = arrow_up(list, here);
                let end = Point::new(here.x, here.y + h);
                let path = Path::new().move_to(a.x, a.y).cubic_to(
                    Point::new(end.x, a.y),
                    Point::new(end.x, a.y),
                    end,
                );
                list.stroke(path, t, Paint::Ink);
                label(list, here.x, pt.cents);
            }
            Some((a, c)) if pt.cents < c => {
                let h = arrow_down(list, here);
                let end = Point::new(here.x, here.y - h);
                let path = Path::new().move_to(a.x, a.y).cubic_to(
                    Point::new(end.x, a.y),
                    Point::new(end.x, a.y),
                    end,
                );
                list.stroke(path, t, Paint::Ink);
            }
            Some((a, c)) if c != 0 && here.x > a.x + 0.5 * sp => {
                // Held at the top: a dashed line.
                let mut x = a.x;
                while x < here.x {
                    p.hline(list, x, (x + 0.4 * sp).min(here.x), a.y, 0.08 * sp);
                    x += 0.7 * sp;
                }
            }
            Some(_) => {}
        }
        prev = Some((here, pt.cents));
    }
}

/// A bend amount as Guitar Pro writes it: "full" for a whole tone, "1/2",
/// "1/4", "1 1/2", "2" and so on.
pub(crate) fn bend_amount(cents: i32) -> String {
    let quarters = (cents.unsigned_abs() + 25) / 50; // quarter tones
    let whole = quarters / 4;
    let frac = match quarters % 4 {
        1 => "1/4",
        2 => "1/2",
        3 => "3/4",
        _ => "",
    };
    match (whole, frac) {
        (0, "") => "0".to_owned(),
        (0, f) => f.to_owned(),
        (1, "") => "full".to_owned(),
        (w, "") => w.to_string(),
        (w, f) => format!("{w} {f}"),
    }
}

pub(crate) fn dynamic_glyph(d: Dynamic) -> G {
    match d {
        Dynamic::PPP => G("dynamicPPP", '\u{E52A}'),
        Dynamic::PP => G("dynamicPP", '\u{E52B}'),
        Dynamic::P => G("dynamicPiano", '\u{E520}'),
        Dynamic::MP => G("dynamicMP", '\u{E52C}'),
        Dynamic::MF => G("dynamicMF", '\u{E52D}'),
        Dynamic::F => G("dynamicForte", '\u{E522}'),
        Dynamic::FF => G("dynamicFF", '\u{E52F}'),
        Dynamic::FFF => G("dynamicFFF", '\u{E530}'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_amounts_read_like_guitar_pro() {
        assert_eq!(bend_amount(200), "full");
        assert_eq!(bend_amount(100), "1/2");
        assert_eq!(bend_amount(50), "1/4");
        assert_eq!(bend_amount(300), "1 1/2");
        assert_eq!(bend_amount(400), "2");
        assert_eq!(bend_amount(-100), "1/2");
    }
}
