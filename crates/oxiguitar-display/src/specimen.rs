// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A hand-placed guitar excerpt for the render and GUI spikes.
//!
//! Real layout arrives in M1 (`oxiguitar-layout`). Until then this page
//! exercises everything a renderer must draw: SMuFL glyphs positioned from the
//! font's metadata, staff and ledger lines, stems, a beam, a slur, a bend
//! arrow, tab numbers with the staff lines knocked out behind them, text in a
//! second font, and every colour role.

use crate::fonts::FontSet;
use crate::list::{DisplayList, Paint, Path, Point, Rect};
use crate::smufl::{Metadata, glyph, glyph::Glyph};

/// Staff space of the excerpt, in points (a 20 pt, 7 mm staff).
pub const STAFF_SPACE: f32 = 5.0;

/// A five-line staff. Steps count half staff spaces up from the bottom line.
struct Staff {
    top: f32,
    left: f32,
    right: f32,
}

impl Staff {
    fn y(&self, step: i32) -> f32 {
        self.top + 4.0 * STAFF_SPACE - step as f32 * STAFF_SPACE / 2.0
    }
}

/// A six-line tab staff. String 1 (high E) is the top line.
struct Tab {
    top: f32,
    gap: f32,
}

impl Tab {
    fn string_y(&self, string: u8) -> f32 {
        self.top + f32::from(string.saturating_sub(1)) * self.gap
    }

    fn bottom(&self) -> f32 {
        self.string_y(6)
    }
}

/// Builds the excerpt on an A4 page.
struct Builder<'a> {
    fonts: &'a FontSet,
    meta: &'a Metadata,
    list: DisplayList,
}

impl Builder<'_> {
    fn sp(&self) -> f32 {
        STAFF_SPACE
    }

    fn ed(&self) -> crate::smufl::EngravingDefaults {
        self.meta.engraving_defaults
    }

    fn glyph_width(&self, g: Glyph) -> f32 {
        self.meta.bbox(g.name).map_or(1.0, |b| b.width()) * self.sp()
    }

    fn symbol(&mut self, g: Glyph, x: f32, y: f32, paint: Paint) {
        let run = self
            .fonts
            .symbol(g.cp, 4.0 * self.sp(), Point::new(x, y), paint);
        self.list.glyphs(run);
    }

    fn symbol_sized(&mut self, g: Glyph, size: f32, x: f32, y: f32) {
        let run = self.fonts.symbol(g.cp, size, Point::new(x, y), Paint::Ink);
        self.list.glyphs(run);
    }

    fn text(&mut self, text: &str, size: f32, x: f32, y: f32, paint: Paint) -> f32 {
        let run = self
            .fonts
            .text_run(FontSet::TEXT, size, Point::new(x, y), text, paint);
        let w = run.glyphs.last().map_or(0.0, |g| g.x + g.advance);
        self.list.glyphs(run);
        w
    }

    fn text_centred(&mut self, text: &str, size: f32, cx: f32, y: f32, paint: Paint) {
        let w = self.fonts.text_width(FontSet::TEXT, size, text);
        self.text(text, size, cx - w / 2.0, y, paint);
    }

    fn staff_lines(&mut self, s: &Staff) {
        let t = self.ed().staff_line_thickness * self.sp();
        for i in 0..5 {
            let y = s.top + i as f32 * self.sp();
            self.list
                .line(Point::new(s.left, y), Point::new(s.right, y), t, Paint::Ink);
        }
    }

    fn tab_lines(&mut self, tab: &Tab, left: f32, right: f32) {
        let t = self.ed().staff_line_thickness * self.sp();
        for string in 1..=6 {
            let y = tab.string_y(string);
            self.list
                .line(Point::new(left, y), Point::new(right, y), t, Paint::Ink);
        }
    }

    fn barline(&mut self, x: f32, top: f32, bottom: f32) {
        let t = self.ed().thin_barline_thickness * self.sp();
        self.list
            .line(Point::new(x, top), Point::new(x, bottom), t, Paint::Ink);
    }

    fn final_barline(&mut self, right: f32, top: f32, bottom: f32) {
        let ed = self.ed();
        let thick = ed.thick_barline_thickness * self.sp();
        let thin = ed.thin_barline_thickness * self.sp();
        let thick_x = right - thick / 2.0;
        let thin_x = right - thick - ed.thin_thick_barline_separation * self.sp() - thin / 2.0;
        self.list.line(
            Point::new(thick_x, top),
            Point::new(thick_x, bottom),
            thick,
            Paint::Ink,
        );
        self.list.line(
            Point::new(thin_x, top),
            Point::new(thin_x, bottom),
            thin,
            Paint::Ink,
        );
    }

    /// Ledger lines for a note at `step` whose notehead starts at `x`.
    fn ledgers(&mut self, s: &Staff, x: f32, step: i32, head: Glyph) {
        let ed = self.ed();
        let ext = ed.leger_line_extension * self.sp();
        let w = self.glyph_width(head);
        let t = ed.leger_line_thickness * self.sp();
        let mut lines = Vec::new();
        let mut l = -2;
        while l >= step {
            lines.push(l);
            l -= 2;
        }
        let mut l = 10;
        while l <= step {
            lines.push(l);
            l += 2;
        }
        for l in lines {
            let y = s.y(l);
            self.list.line(
                Point::new(x - ext, y),
                Point::new(x + w + ext, y),
                t,
                Paint::Ink,
            );
        }
    }

    /// Where a stem joins a notehead, from the font's anchors, in points.
    fn stem_anchor(&self, head: Glyph, up: bool) -> (f32, f32) {
        let name = if up { "stemUpSE" } else { "stemDownNW" };
        let a = self.meta.anchor(head.name, name).unwrap_or(if up {
            [1.18, 0.168]
        } else {
            [0.0, -0.168]
        });
        (a[0] * self.sp(), -a[1] * self.sp())
    }

    /// A stem from the notehead at (`x`, `y`) to `tip_y`, drawn inside the
    /// notehead's edge as SMuFL's anchors intend.
    fn stem(&mut self, head: Glyph, x: f32, y: f32, up: bool, tip_y: f32) -> f32 {
        let (ax, ay) = self.stem_anchor(head, up);
        let t = self.ed().stem_thickness * self.sp();
        let sx = if up {
            x + ax - t / 2.0
        } else {
            x + ax + t / 2.0
        };
        self.list
            .line(Point::new(sx, y + ay), Point::new(sx, tip_y), t, Paint::Ink);
        sx
    }

    /// A beam whose top edge runs from (`x0`, `y0`) to (`x1`, `y1`).
    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let h = self.ed().beam_thickness * self.sp();
        let p = Path::new()
            .move_to(x0, y0)
            .line_to(x1, y1)
            .line_to(x1, y1 + h)
            .line_to(x0, y0 + h)
            .close();
        self.list.fill(p, Paint::Ink);
    }

    /// A slur or tie between two points, bulging by `height` (negative is up),
    /// thicker in the middle as engravers draw them.
    fn slur(&mut self, a: Point, b: Point, height: f32) {
        let ed = self.ed();
        let mid = ed.slur_midpoint_thickness * self.sp();
        let dx = b.x - a.x;
        let outer = height * 4.0 / 3.0;
        let inner = (height.abs() - mid).max(0.0).copysign(height) * 4.0 / 3.0;
        let c = |t: f32, off: f32| Point::new(a.x + dx * t, a.y + (b.y - a.y) * t + off);
        let p = Path::new()
            .move_to(a.x, a.y)
            .cubic_to(c(0.25, outer), c(0.75, outer), b)
            .cubic_to(c(0.75, inner), c(0.25, inner), a)
            .close();
        self.list.fill(p, Paint::Ink);
    }

    /// A tab number centred on (`cx`, string line), with the line knocked out.
    fn fret(&mut self, tab: &Tab, cx: f32, string: u8, label: &str) {
        let size = 1.4 * tab.gap;
        let w = self.fonts.text_width(FontSet::TEXT, size, label);
        let y = tab.string_y(string);
        let pad = 0.12 * tab.gap;
        self.list.rect(
            Rect::new(
                cx - w / 2.0 - pad,
                y - 0.45 * tab.gap,
                w + 2.0 * pad,
                0.9 * tab.gap,
            ),
            Paint::Page,
        );
        // Noto Sans digits are 0.714 em tall; centre them on the line.
        self.text(label, size, cx - w / 2.0, y + 0.357 * size, Paint::Ink);
    }
}

/// One note of the excerpt: staff step (0 = bottom line), string and fret.
#[derive(Clone, Copy)]
struct Note {
    step: i32,
    string: u8,
    fret: u8,
}

const fn n(step: i32, string: u8, fret: u8) -> Note {
    Note { step, string, fret }
}

/// The render spike's specimen page: a two-bar guitar excerpt in standard
/// notation and tablature, a sampler of SMuFL glyphs and a row of tab numbers.
pub fn guitar_excerpt(fonts: &FontSet, meta: &Metadata) -> DisplayList {
    let mut b = Builder {
        fonts,
        meta,
        list: DisplayList::a4(),
    };
    let sp = STAFF_SPACE;
    let left = 56.7;
    let right = b.list.width - 56.7;
    let centre = b.list.width / 2.0;

    // Header.
    b.text_centred("Render spike", 22.0, centre, 86.0, Paint::Ink);
    let sub = format!(
        "{} {} and Noto Sans, drawn from one display list",
        meta.font_name, meta.font_version
    );
    b.text_centred(&sub, 10.0, centre, 106.0, Paint::Muted);
    b.text("Tuning: E A D G B E", 8.0, left, 140.0, Paint::Ink);

    // System: standard staff over tab.
    let staff = Staff {
        top: 190.0,
        left,
        right,
    };
    let tab = Tab {
        top: staff.top + 4.0 * sp + 9.0 * sp,
        gap: 1.5 * sp,
    };
    b.staff_lines(&staff);
    b.tab_lines(&tab, left, right);
    b.barline(left, staff.top, tab.bottom());

    // Tempo and a section marker.
    let tempo_y = staff.top - 18.0;
    // The tempo note is drawn at 80 % of the staff size.
    b.symbol_sized(glyph::NOTE_QUARTER_UP, 3.2 * sp, left + 2.0, tempo_y);
    let note_w = 0.8 * b.glyph_width(glyph::NOTE_QUARTER_UP);
    b.text("= 96", 9.0, left + 2.0 + note_w + 3.0, tempo_y, Paint::Ink);
    let marker = "Intro";
    let mw = b.fonts.text_width(FontSet::TEXT, 9.0, marker);
    let (mx, my) = (150.0, tempo_y);
    b.list.stroke(
        Path::new()
            .move_to(mx - 3.0, my - 9.5)
            .line_to(mx + mw + 3.0, my - 9.5)
            .line_to(mx + mw + 3.0, my + 3.0)
            .line_to(mx - 3.0, my + 3.0)
            .close(),
        0.6,
        Paint::Ink,
    );
    b.text(marker, 9.0, mx, my, Paint::Ink);

    // Clefs and time signatures.
    b.symbol(glyph::G_CLEF_8VB, left + 0.5 * sp, staff.y(2), Paint::Ink);
    let tab_mid = (tab.top + tab.bottom()) / 2.0;
    // The tab clef is drawn for a five-line staff; scale it to the taller tab.
    b.symbol_sized(
        glyph::SIX_STRING_TAB_CLEF,
        4.0 * tab.gap * 0.7,
        left + 0.5 * sp,
        tab_mid,
    );
    let ts_x = left + 0.5 * sp + b.glyph_width(glyph::G_CLEF_8VB) + sp;
    b.symbol(glyph::TIME_SIG_4, ts_x, staff.y(6), Paint::Ink);
    b.symbol(glyph::TIME_SIG_4, ts_x, staff.y(2), Paint::Ink);
    let tab_ts = 4.0 * tab.gap * 0.7;
    b.symbol_sized(glyph::TIME_SIG_4, tab_ts, ts_x, tab.top + 1.25 * tab.gap);
    b.symbol_sized(
        glyph::TIME_SIG_4,
        tab_ts,
        ts_x,
        tab.bottom() - 1.25 * tab.gap,
    );

    let bar2 = 319.0;
    b.barline(bar2, staff.top, staff.y(0));
    b.barline(bar2, tab.top, tab.bottom());
    b.final_barline(right, staff.top, staff.y(0));
    b.final_barline(right, tab.top, tab.bottom());
    b.text("1", 7.0, 100.0, staff.top - 4.0, Paint::Muted);
    b.text("2", 7.0, bar2 + 2.0, staff.top - 4.0, Paint::Muted);

    let head_w = b.glyph_width(glyph::NOTEHEAD_BLACK);
    let stem_len = 3.5 * sp;

    // Bar 1: four beamed eighths (G arpeggio), then a G major chord.
    let eighths = [
        (110.0, n(-5, 6, 3)),
        (138.0, n(-1, 4, 0)),
        (166.0, n(2, 3, 0)),
        (194.0, n(4, 2, 0)),
    ];
    for pair in eighths.chunks(2) {
        let [(x0, a), (x1, c)] = [pair[0], pair[1]];
        // Beam through the higher note's stem tip, slope at most half a space.
        let (ya, yc) = (staff.y(a.step) - stem_len, staff.y(c.step) - stem_len);
        let top = ya.min(yc);
        let slope = (yc - ya).clamp(-sp / 2.0, sp / 2.0);
        let (ba, bc) = if yc < ya {
            (top - slope, top)
        } else {
            (top, top + slope)
        };
        for &(x, note, tip) in &[(x0, a, ba), (x1, c, bc)] {
            b.ledgers(&staff, x, note.step, glyph::NOTEHEAD_BLACK);
            b.symbol(glyph::NOTEHEAD_BLACK, x, staff.y(note.step), Paint::Ink);
            b.stem(glyph::NOTEHEAD_BLACK, x, staff.y(note.step), true, tip);
            b.fret(&tab, x + head_w / 2.0, note.string, &note.fret.to_string());
        }
        let t = b.ed().stem_thickness * sp;
        let (sx0, sx1) = (x0 + head_w - t, x1 + head_w);
        b.beam(sx0, ba, sx1, bc);
    }
    let chord = [
        n(-5, 6, 3),
        n(-3, 5, 2),
        n(-1, 4, 0),
        n(2, 3, 0),
        n(4, 2, 0),
        n(9, 1, 3),
    ];
    let cx = 230.0;
    for note in chord {
        b.ledgers(&staff, cx, note.step, glyph::NOTEHEAD_HALF);
        b.symbol(glyph::NOTEHEAD_HALF, cx, staff.y(note.step), Paint::Ink);
        b.fret(&tab, cx + head_w / 2.0, note.string, &note.fret.to_string());
    }
    b.stem(
        glyph::NOTEHEAD_HALF,
        cx,
        staff.y(-5),
        true,
        staff.y(9) - stem_len,
    );

    // Bar 2: hammer-on E -> F#, a bent G, a quarter rest.
    let (x_e, x_fs, x_g, x_rest) = (332.0, 382.0, 432.0, 482.0);
    let e = n(0, 4, 2);
    let fs = n(1, 4, 4);
    for &(x, note) in &[(x_e, e), (x_fs, fs)] {
        b.symbol(glyph::NOTEHEAD_BLACK, x, staff.y(note.step), Paint::Ink);
        b.stem(
            glyph::NOTEHEAD_BLACK,
            x,
            staff.y(note.step),
            true,
            staff.y(note.step) - stem_len,
        );
        b.fret(&tab, x + head_w / 2.0, note.string, &note.fret.to_string());
    }
    let sharp_w = b.glyph_width(glyph::ACCIDENTAL_SHARP);
    b.symbol(
        glyph::ACCIDENTAL_SHARP,
        x_fs - sharp_w - 0.25 * sp,
        staff.y(fs.step),
        Paint::Ink,
    );
    // Slur under the noteheads, and the hammer-on arc and "H" over the tab.
    b.slur(
        Point::new(x_e + head_w / 2.0, staff.y(e.step) + 0.9 * sp),
        Point::new(x_fs + head_w / 2.0, staff.y(fs.step) + 0.9 * sp),
        1.3 * sp,
    );
    let arc_y = tab.string_y(4) - 0.7 * tab.gap;
    b.slur(
        Point::new(x_e + head_w / 2.0 + 2.0, arc_y),
        Point::new(x_fs + head_w / 2.0 - 2.0, arc_y),
        -1.1 * sp,
    );
    b.text_centred(
        "H",
        7.0,
        (x_e + x_fs + head_w) / 2.0,
        tab.top - 3.0,
        Paint::Ink,
    );

    // A note bent a whole tone: stem down from G5, then the bend arrow.
    let g5 = n(9, 2, 8);
    b.symbol(glyph::NOTEHEAD_BLACK, x_g, staff.y(g5.step), Paint::Ink);
    b.stem(
        glyph::NOTEHEAD_BLACK,
        x_g,
        staff.y(g5.step),
        false,
        staff.y(g5.step) + stem_len,
    );
    let fret_cx = x_g + head_w / 2.0;
    b.fret(&tab, fret_cx, g5.string, "8");
    let from = Point::new(fret_cx + 5.0, tab.string_y(2));
    let peak = Point::new(fret_cx + 16.0, tab.top - 2.2 * tab.gap);
    b.list.stroke(
        Path::new().move_to(from.x, from.y).cubic_to(
            Point::new(peak.x, from.y),
            Point::new(peak.x, from.y),
            Point::new(peak.x, peak.y + 4.0),
        ),
        0.8,
        Paint::Ink,
    );
    b.list.fill(
        Path::new()
            .move_to(peak.x, peak.y)
            .line_to(peak.x + 2.6, peak.y + 5.0)
            .line_to(peak.x - 2.6, peak.y + 5.0)
            .close(),
        Paint::Ink,
    );
    b.text_centred("full", 7.0, peak.x, peak.y - 2.5, Paint::Ink);
    b.symbol(glyph::REST_QUARTER, x_rest, staff.y(4), Paint::Ink);

    // Playback cursor on bar 2, beat 1.
    b.list.rect(
        Rect::new(
            x_e - 4.0,
            staff.top - 6.0,
            2.0,
            tab.bottom() - staff.top + 12.0,
        ),
        Paint::Accent,
    );

    // Glyph sampler.
    let sampler_top = 360.0;
    b.text(
        "SMuFL glyphs from Bravura",
        10.0,
        left,
        sampler_top,
        Paint::Muted,
    );
    let sample = [
        glyph::G_CLEF_8VB,
        glyph::SIX_STRING_TAB_CLEF,
        glyph::NOTEHEAD_BLACK,
        glyph::NOTEHEAD_HALF,
        glyph::NOTEHEAD_X,
        glyph::ACCIDENTAL_SHARP,
        glyph::ACCIDENTAL_FLAT,
        glyph::ACCIDENTAL_NATURAL,
        glyph::REST_QUARTER,
        glyph::REST_EIGHTH,
        glyph::FLAG_8TH_UP,
        glyph::DYNAMIC_MF,
        glyph::FERMATA_ABOVE,
        glyph::ARTICULATION_ACCENT,
        glyph::ARTICULATION_STACCATO,
        glyph::SEGNO,
        glyph::CODA,
        glyph::VIBRATO_SEGMENT,
    ];
    let per_row = 9;
    let cell = (right - left) / per_row as f32;
    for (i, g) in sample.iter().enumerate() {
        let row = (i / per_row) as f32;
        let col = (i % per_row) as f32;
        let base = sampler_top + 45.0 + row * 62.0;
        let x = left + col * cell + cell / 2.0 - b.glyph_width(*g) / 2.0;
        if g.name == glyph::VIBRATO_SEGMENT.name {
            // Repeat the segment by its repeatOffset anchor to make a wavy line.
            let step = meta.anchor(g.name, "repeatOffset").map_or(0.6, |a| a[0]) * sp;
            for k in 0..4 {
                b.symbol(*g, x - 1.5 * step + k as f32 * step, base, Paint::Ink);
            }
        } else {
            b.symbol(*g, x, base, Paint::Ink);
        }
        let label = format!("U+{:04X}", u32::from(g.cp));
        b.text_centred(
            &label,
            6.5,
            left + col * cell + cell / 2.0,
            base + 28.0,
            Paint::Muted,
        );
    }

    // Tab numbers 0 to 24, a dead note and a ghost note.
    let row_top = 540.0;
    b.text(
        "Tab numbers in Noto Sans, staff lines knocked out behind them",
        10.0,
        left,
        row_top,
        Paint::Muted,
    );
    let t2 = Tab {
        top: row_top + 22.0,
        gap: 1.5 * sp,
    };
    b.tab_lines(&t2, left, right);
    b.barline(left, t2.top, t2.bottom());
    b.barline(right, t2.top, t2.bottom());
    let labels: Vec<String> = (0..=24)
        .map(|f| f.to_string())
        .chain(["X".to_owned(), "(5)".to_owned()])
        .collect();
    let step_x = (right - left - 20.0) / labels.len() as f32;
    for (i, label) in labels.iter().enumerate() {
        let string = u8::try_from(i % 6).unwrap_or(0) + 1;
        b.fret(&t2, left + 14.0 + i as f32 * step_x, string, label);
    }

    // Colour roles.
    let roles_y = 650.0;
    b.text(
        "Colour roles: ink, muted, accent, page (knock-outs)",
        10.0,
        left,
        roles_y,
        Paint::Muted,
    );
    for (i, paint) in [Paint::Ink, Paint::Muted, Paint::Accent]
        .into_iter()
        .enumerate()
    {
        b.list.rect(
            Rect::new(left + i as f32 * 40.0, roles_y + 10.0, 30.0, 14.0),
            paint,
        );
    }
    b.list.stroke(
        Path::new()
            .move_to(left + 120.0, roles_y + 10.0)
            .line_to(left + 150.0, roles_y + 10.0)
            .line_to(left + 150.0, roles_y + 24.0)
            .line_to(left + 120.0, roles_y + 24.0)
            .close(),
        0.6,
        Paint::Ink,
    );

    b.text_centred(
        "oxiguitar-cli specimen · OxiGuitar render spike (M0)",
        7.0,
        centre,
        b.list.height - 40.0,
        Paint::Muted,
    );
    b.list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::Item;

    #[test]
    fn specimen_has_every_item_kind() {
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap();
        let page = guitar_excerpt(&fonts, &meta);
        assert!(page.items.len() > 150);
        let has = |f: fn(&Item) -> bool| page.items.iter().any(f);
        assert!(has(|i| matches!(i, Item::Glyphs(_))));
        assert!(has(|i| matches!(i, Item::Line { .. })));
        assert!(has(|i| matches!(
            i,
            Item::Rect {
                paint: Paint::Page,
                ..
            }
        )));
        assert!(has(|i| matches!(i, Item::Path { fill: Some(_), .. })));
        assert!(has(|i| matches!(
            i,
            Item::Path {
                stroke: Some(_),
                ..
            }
        )));
        // No glyph falls back to .notdef.
        for item in &page.items {
            if let Item::Glyphs(run) = item {
                assert!(
                    run.glyphs.iter().all(|g| g.id != 0),
                    "missing glyph in {:?}",
                    run.text
                );
            }
        }
    }

    #[test]
    fn everything_is_on_the_page() {
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap();
        let page = guitar_excerpt(&fonts, &meta);
        let sheet = Rect::new(0.0, 0.0, page.width, page.height);
        for item in &page.items {
            assert!(item.bounds().intersects(&sheet));
        }
    }
}
