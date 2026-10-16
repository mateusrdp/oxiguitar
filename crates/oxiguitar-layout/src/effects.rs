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
use oxiguitar_model::{BeatTechnique, Dynamic, Harmonic, Slide, StrokeDirection};

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
    for voice in voices {
        let mine: Vec<&Mark<'_>> = marks.iter().filter(|m| m.voice == voice).collect();
        connections(p, list, &mine, g);
        runs(p, list, &mine, g);
    }
    for m in marks {
        single(p, list, m, g);
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
            b.notes.iter().any(|n| n.effects.vibrato) && !b.effects.vibrato_wide
        }),
        (Lane::Vibrato, |b| b.effects.vibrato_wide),
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
                    let g_ = if k == 0 {
                        glyphs::VIBRATO
                    } else {
                        glyphs::VIBRATO_WIDE
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
        let mut put = |text: &str, list: &mut DisplayList| {
            x += p.text(list, text, 1.6 * sp, x, lane.base, Paint::Ink) + 0.4 * sp;
        };
        let tap = b.notes.iter().any(|n| n.effects.left_hand_tap);
        match b.effects.technique {
            Some(BeatTechnique::Tap) => put("T", list),
            Some(BeatTechnique::Slap) => put("S", list),
            Some(BeatTechnique::Pop) => put("P", list),
            None if tap => put("T", list),
            None => {}
        }
        if b.effects.whammy.is_some() {
            put("w/bar", list);
        }
        if b.effects.fade_in {
            // A small swell hairpin.
            let w = 1.6 * sp;
            let y = lane.base - 0.45 * sp;
            p.line_between(
                list,
                Point::new(x, y),
                Point::new(x + w, y - 0.4 * sp),
                0.1 * sp,
            );
            p.line_between(
                list,
                Point::new(x, y),
                Point::new(x + w, y + 0.4 * sp),
                0.1 * sp,
            );
            x += w + 0.4 * sp;
        }
        if let Some(dir) = b.effects.pick_stroke {
            let gl = match dir {
                StrokeDirection::Down => glyphs::DOWN_BOW,
                StrokeDirection::Up => glyphs::UP_BOW,
            };
            p.glyph_scaled(list, gl, 0.8, x, lane.base, Paint::Ink);
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

    // Dynamics and lyrics under the staff.
    if let (Some(d), Some(base)) = (b.show_dynamic, g.band.dynamics) {
        let gl = dynamic_glyph(d);
        p.glyph(list, gl, m.x - hw / 2.0, base, Paint::Ink);
    }
    for (line, syllable) in &b.lyrics {
        if let Some(base) = g.band.lyrics.get(*line) {
            p.text_centred(list, syllable, lyric_size(sp), m.x, *base, Paint::Ink);
        }
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
