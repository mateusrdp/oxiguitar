// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Drawing primitives in staff spaces: SMuFL glyphs placed by the font's
//! metadata, lines, beams, ties and tab numbers.

use oxiguitar_display::smufl::{EngravingDefaults, Metadata};
use oxiguitar_display::{DisplayList, FontId, FontSet, Paint, Path, Point, Rect};

/// A SMuFL glyph: its metadata name and codepoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct G(pub(crate) &'static str, pub(crate) char);

#[allow(dead_code)] // The full set grows with the engraving.
pub(crate) mod glyphs {
    use super::G;
    pub(crate) const NOTEHEAD_BLACK: G = G("noteheadBlack", '\u{E0A4}');
    pub(crate) const NOTEHEAD_HALF: G = G("noteheadHalf", '\u{E0A3}');
    pub(crate) const NOTEHEAD_WHOLE: G = G("noteheadWhole", '\u{E0A2}');
    pub(crate) const NOTEHEAD_DOUBLE_WHOLE: G = G("noteheadDoubleWhole", '\u{E0A0}');
    pub(crate) const NOTEHEAD_X: G = G("noteheadXBlack", '\u{E0A9}');
    pub(crate) const SHARP: G = G("accidentalSharp", '\u{E262}');
    pub(crate) const FLAT: G = G("accidentalFlat", '\u{E260}');
    pub(crate) const NATURAL: G = G("accidentalNatural", '\u{E261}');
    pub(crate) const DOUBLE_SHARP: G = G("accidentalDoubleSharp", '\u{E263}');
    pub(crate) const DOUBLE_FLAT: G = G("accidentalDoubleFlat", '\u{E264}');
    pub(crate) const REST_DOUBLE_WHOLE: G = G("restDoubleWhole", '\u{E4E2}');
    pub(crate) const REST_WHOLE: G = G("restWhole", '\u{E4E3}');
    pub(crate) const REST_HALF: G = G("restHalf", '\u{E4E4}');
    pub(crate) const REST_QUARTER: G = G("restQuarter", '\u{E4E5}');
    pub(crate) const REST_8TH: G = G("rest8th", '\u{E4E6}');
    pub(crate) const REST_16TH: G = G("rest16th", '\u{E4E7}');
    pub(crate) const REST_32ND: G = G("rest32nd", '\u{E4E8}');
    pub(crate) const REST_64TH: G = G("rest64th", '\u{E4E9}');
    pub(crate) const REST_128TH: G = G("rest128th", '\u{E4EA}');
    pub(crate) const G_CLEF: G = G("gClef", '\u{E050}');
    pub(crate) const G_CLEF_8VB: G = G("gClef8vb", '\u{E052}');
    pub(crate) const F_CLEF: G = G("fClef", '\u{E062}');
    pub(crate) const F_CLEF_8VB: G = G("fClef8vb", '\u{E064}');
    pub(crate) const C_CLEF: G = G("cClef", '\u{E05C}');
    pub(crate) const PERCUSSION_CLEF: G = G("unpitchedPercussionClef1", '\u{E069}');
    pub(crate) const TAB_CLEF_6: G = G("6stringTabClef", '\u{E06D}');
    pub(crate) const TAB_CLEF_4: G = G("4stringTabClef", '\u{E06E}');
    pub(crate) const AUGMENTATION_DOT: G = G("augmentationDot", '\u{E1E7}');
    pub(crate) const PAREN_LEFT: G = G("noteheadParenthesisLeft", '\u{E0F5}');
    pub(crate) const PAREN_RIGHT: G = G("noteheadParenthesisRight", '\u{E0F6}');
    pub(crate) const NOTEHEAD_DIAMOND_BLACK: G = G("noteheadDiamondBlack", '\u{E0DB}');
    pub(crate) const NOTEHEAD_DIAMOND_HALF: G = G("noteheadDiamondHalf", '\u{E0D9}');
    pub(crate) const NOTEHEAD_DIAMOND_WHOLE: G = G("noteheadDiamondWhole", '\u{E0D8}');
    pub(crate) const ACCENT_ABOVE: G = G("articAccentAbove", '\u{E4A0}');
    pub(crate) const ACCENT_BELOW: G = G("articAccentBelow", '\u{E4A1}');
    pub(crate) const STACCATO_ABOVE: G = G("articStaccatoAbove", '\u{E4A2}');
    pub(crate) const STACCATO_BELOW: G = G("articStaccatoBelow", '\u{E4A3}');
    pub(crate) const MARCATO_ABOVE: G = G("articMarcatoAbove", '\u{E4AC}');
    pub(crate) const MARCATO_BELOW: G = G("articMarcatoBelow", '\u{E4AD}');
    pub(crate) const VIBRATO: G = G("guitarVibratoStroke", '\u{EAB2}');
    pub(crate) const VIBRATO_WIDE: G = G("guitarWideVibratoStroke", '\u{EAB3}');
    pub(crate) const TRILL: G = G("ornamentTrill", '\u{E566}');
    pub(crate) const WIGGLE_TRILL: G = G("wiggleTrill", '\u{EAA4}');
    pub(crate) const TREMOLO: [G; 3] = [
        G("tremolo1", '\u{E220}'),
        G("tremolo2", '\u{E221}'),
        G("tremolo3", '\u{E222}'),
    ];
    pub(crate) const DOWN_BOW: G = G("stringsDownBow", '\u{E610}');
    pub(crate) const UP_BOW: G = G("stringsUpBow", '\u{E612}');
    pub(crate) const ARROWHEAD_UP: G = G("arrowheadBlackUp", '\u{EB78}');
    pub(crate) const ARROWHEAD_DOWN: G = G("arrowheadBlackDown", '\u{EB7C}');
    pub(crate) const GRACE_SLASH_UP: G = G("graceNoteSlashStemUp", '\u{E564}');
    pub(crate) const REPEAT_DOT: G = G("repeatDot", '\u{E044}');
    pub(crate) const NOTE_QUARTER_UP: G = G("noteQuarterUp", '\u{E1D5}');
    pub(crate) const FLAGS_UP: [G; 4] = [
        G("flag8thUp", '\u{E240}'),
        G("flag16thUp", '\u{E242}'),
        G("flag32ndUp", '\u{E244}'),
        G("flag64thUp", '\u{E246}'),
    ];
    pub(crate) const FLAGS_DOWN: [G; 4] = [
        G("flag8thDown", '\u{E241}'),
        G("flag16thDown", '\u{E243}'),
        G("flag32ndDown", '\u{E245}'),
        G("flag64thDown", '\u{E247}'),
    ];

    /// The time-signature digit glyph for 0–9.
    pub(crate) fn time_sig_digit(d: u32) -> G {
        const NAMES: [&str; 10] = [
            "timeSig0", "timeSig1", "timeSig2", "timeSig3", "timeSig4", "timeSig5", "timeSig6",
            "timeSig7", "timeSig8", "timeSig9",
        ];
        let d = d.min(9);
        G(
            NAMES.get(d as usize).copied().unwrap_or("timeSig0"),
            char::from_u32(0xE080 + d).unwrap_or('\u{E080}'),
        )
    }

    /// The tuplet number glyph for 0–9.
    pub(crate) fn tuplet_digit(d: u32) -> G {
        const NAMES: [&str; 10] = [
            "tuplet0", "tuplet1", "tuplet2", "tuplet3", "tuplet4", "tuplet5", "tuplet6", "tuplet7",
            "tuplet8", "tuplet9",
        ];
        let d = d.min(9);
        G(
            NAMES.get(d as usize).copied().unwrap_or("tuplet0"),
            char::from_u32(0xE880 + d).unwrap_or('\u{E880}'),
        )
    }
}

/// Draws into a display list, measuring in staff spaces.
pub(crate) struct Painter<'a> {
    pub(crate) fonts: &'a FontSet,
    pub(crate) meta: &'a Metadata,
    /// One staff space, in points.
    pub(crate) sp: f32,
}

impl Painter<'_> {
    pub(crate) fn ed(&self) -> EngravingDefaults {
        self.meta.engraving_defaults
    }

    /// Glyph width in points.
    pub(crate) fn width(&self, g: G) -> f32 {
        self.meta.bbox(g.0).map_or(1.0, |b| b.width()) * self.sp
    }

    /// Glyph extent above and below its origin, in points (both positive
    /// away from the baseline).
    pub(crate) fn height(&self, g: G) -> (f32, f32) {
        self.meta
            .bbox(g.0)
            .map_or((1.0, 1.0), |b| (b.ne[1] * self.sp, -b.sw[1] * self.sp))
    }

    /// A glyph anchor `[x, y]` in points, y down, or `default` (in staff
    /// spaces, y up) if the font has none.
    pub(crate) fn anchor(&self, g: G, name: &str, default: [f32; 2]) -> (f32, f32) {
        let a = self.meta.anchor(g.0, name).unwrap_or(default);
        (a[0] * self.sp, -a[1] * self.sp)
    }

    /// A glyph at full staff size with its origin at (`x`, `y`).
    pub(crate) fn glyph(&self, list: &mut DisplayList, g: G, x: f32, y: f32, paint: Paint) {
        self.glyph_scaled(list, g, 1.0, x, y, paint);
    }

    /// A glyph scaled from the staff size.
    pub(crate) fn glyph_scaled(
        &self,
        list: &mut DisplayList,
        g: G,
        scale: f32,
        x: f32,
        y: f32,
        paint: Paint,
    ) {
        list.glyphs(
            self.fonts
                .symbol(g.1, 4.0 * self.sp * scale, Point::new(x, y), paint),
        );
    }

    /// Text in the text font, left-aligned at `x` on baseline `y`. Returns
    /// its width.
    pub(crate) fn text(
        &self,
        list: &mut DisplayList,
        text: &str,
        size: f32,
        x: f32,
        y: f32,
        paint: Paint,
    ) -> f32 {
        let mut cx = x;
        for (font, segment) in self.segments(text) {
            let dy = if font == FontSet::MUSIC {
                -FALLBACK_RAISE * size
            } else {
                0.0
            };
            let run = self
                .fonts
                .text_run(font, size, Point::new(cx, y + dy), segment, paint);
            cx += run.glyphs.last().map_or(0.0, |g| g.x + g.advance);
            list.glyphs(run);
        }
        cx - x
    }

    pub(crate) fn text_width(&self, text: &str, size: f32) -> f32 {
        self.segments(text)
            .into_iter()
            .map(|(font, segment)| self.fonts.text_width(font, size, segment))
            .sum()
    }

    /// Splits text into runs by font: the text font, or the music font for
    /// characters only it has (♯ ♭ ♮ in chord names).
    fn segments<'t>(&self, text: &'t str) -> Vec<(FontId, &'t str)> {
        let font_of = |c: char| {
            if c.is_whitespace() || self.fonts.glyph_for(FontSet::TEXT, c) != 0 {
                FontSet::TEXT
            } else if self.fonts.glyph_for(FontSet::MUSIC, c) != 0 {
                FontSet::MUSIC
            } else {
                FontSet::TEXT
            }
        };
        let mut out: Vec<(FontId, &str)> = Vec::new();
        let mut start = 0;
        let mut current: Option<FontId> = None;
        for (i, c) in text.char_indices() {
            let f = font_of(c);
            match current {
                Some(cur) if cur != f => {
                    if let Some(seg) = text.get(start..i) {
                        out.push((cur, seg));
                    }
                    start = i;
                    current = Some(f);
                }
                None => current = Some(f),
                _ => {}
            }
        }
        if let (Some(cur), Some(seg)) = (current, text.get(start..)) {
            out.push((cur, seg));
        }
        out
    }

    /// Text centred on `cx`.
    pub(crate) fn text_centred(
        &self,
        list: &mut DisplayList,
        text: &str,
        size: f32,
        cx: f32,
        y: f32,
        paint: Paint,
    ) {
        let w = self.text_width(text, size);
        self.text(list, text, size, cx - w / 2.0, y, paint);
    }

    /// Text ending at `right`.
    pub(crate) fn text_right(
        &self,
        list: &mut DisplayList,
        text: &str,
        size: f32,
        right: f32,
        y: f32,
        paint: Paint,
    ) {
        let w = self.text_width(text, size);
        self.text(list, text, size, right - w, y, paint);
    }

    pub(crate) fn hline(&self, list: &mut DisplayList, x0: f32, x1: f32, y: f32, width: f32) {
        list.line(Point::new(x0, y), Point::new(x1, y), width, Paint::Ink);
    }

    pub(crate) fn vline(&self, list: &mut DisplayList, x: f32, y0: f32, y1: f32, width: f32) {
        list.line(Point::new(x, y0), Point::new(x, y1), width, Paint::Ink);
    }

    /// A beam of `thickness` whose top edge runs from (`x0`, `y0`) to
    /// (`x1`, `y1`) (or bottom edge, for `downward == false`).
    pub(crate) fn beam(&self, list: &mut DisplayList, x0: f32, y0: f32, x1: f32, y1: f32) {
        let h = self.ed().beam_thickness * self.sp;
        let p = Path::new()
            .move_to(x0, y0 - h / 2.0)
            .line_to(x1, y1 - h / 2.0)
            .line_to(x1, y1 + h / 2.0)
            .line_to(x0, y0 + h / 2.0)
            .close();
        list.fill(p, Paint::Ink);
    }

    /// A tie or slur from `a` to `b`, bulging by `height` (negative is up),
    /// thicker in the middle.
    pub(crate) fn tie(&self, list: &mut DisplayList, a: Point, b: Point, height: f32) {
        let mid = self.ed().slur_midpoint_thickness * self.sp;
        let dx = b.x - a.x;
        let outer = height * 4.0 / 3.0;
        let inner = (height.abs() - mid).max(0.0).copysign(height) * 4.0 / 3.0;
        let c = |t: f32, off: f32| Point::new(a.x + dx * t, a.y + (b.y - a.y) * t + off);
        let p = Path::new()
            .move_to(a.x, a.y)
            .cubic_to(c(0.25, outer), c(0.75, outer), b)
            .cubic_to(c(0.75, inner), c(0.25, inner), a)
            .close();
        list.fill(p, Paint::Ink);
    }

    /// A straight line between two points, `width` thick.
    pub(crate) fn line_between(&self, list: &mut DisplayList, a: Point, b: Point, width: f32) {
        list.line(a, b, width, Paint::Ink);
    }

    /// A slur: like a tie, but between any two points.
    pub(crate) fn slur(&self, list: &mut DisplayList, a: Point, b: Point, height: f32) {
        self.tie(list, a, b, height);
    }

    /// A tab number centred on (`cx`, `y`) with the string line knocked out
    /// behind it. `gap` is the distance between tab lines.
    pub(crate) fn tab_number(
        &self,
        list: &mut DisplayList,
        cx: f32,
        y: f32,
        gap: f32,
        label: &str,
    ) {
        let size = tab_number_size(gap);
        let w = self.text_width(label, size);
        let pad = 0.12 * gap;
        list.rect(
            Rect::new(cx - w / 2.0 - pad, y - 0.45 * gap, w + 2.0 * pad, 0.9 * gap),
            Paint::Page,
        );
        // Noto Sans digits are 0.714 em tall; centre them on the line.
        self.text(
            list,
            label,
            size,
            cx - w / 2.0,
            y + 0.357 * size,
            Paint::Ink,
        );
    }

    /// Width of a tab number, knock-out included.
    pub(crate) fn tab_number_width(&self, gap: f32, label: &str) -> f32 {
        self.text_width(label, tab_number_size(gap)) + 0.24 * gap
    }

    /// Width of a key signature.
    pub(crate) fn key_width(&self, fifths: i8) -> f32 {
        let n = f32::from(fifths.unsigned_abs().min(7));
        let g = if fifths < 0 {
            glyphs::FLAT
        } else {
            glyphs::SHARP
        };
        n * (self.width(g) + 0.15 * self.sp)
    }

    /// Width of a key change: naturals cancelling the old key, then the new.
    pub(crate) fn key_change_width(&self, old: i8, new: i8) -> f32 {
        let kept = if old.signum() == new.signum() {
            new.unsigned_abs()
        } else {
            0
        };
        let naturals = f32::from(old.unsigned_abs().min(7).saturating_sub(kept));
        let cancel = naturals * (self.width(glyphs::NATURAL) + 0.15 * self.sp);
        let gap = if naturals > 0.0 && new != 0 {
            0.4 * self.sp
        } else {
            0.0
        };
        cancel + gap + self.key_width(new)
    }

    /// The digit glyphs of a time-signature number.
    fn digits(n: u8) -> Vec<G> {
        n.to_string()
            .chars()
            .filter_map(|c| c.to_digit(10))
            .map(glyphs::time_sig_digit)
            .collect()
    }

    /// Width of a time signature.
    pub(crate) fn time_width(&self, numerator: u8, denominator: u8) -> f32 {
        let w = |n: u8| Self::digits(n).iter().map(|g| self.width(*g)).sum::<f32>();
        w(numerator).max(w(denominator))
    }

    /// A time signature whose number centres are at `top_y` and `bottom_y`,
    /// centred on `cx`.
    pub(crate) fn time_signature(
        &self,
        list: &mut DisplayList,
        numerator: u8,
        denominator: u8,
        cx: f32,
        top_y: f32,
        bottom_y: f32,
    ) {
        for (n, y) in [(numerator, top_y), (denominator, bottom_y)] {
            let digits = Self::digits(n);
            let total: f32 = digits.iter().map(|g| self.width(*g)).sum();
            let mut x = cx - total / 2.0;
            for g in digits {
                self.glyph(list, g, x, y, Paint::Ink);
                x += self.width(g);
            }
        }
    }

    /// A thin vertical line of the bar-line thickness.
    pub(crate) fn thin_bar(&self, list: &mut DisplayList, x: f32, top: f32, bottom: f32) {
        let t = self.ed().thin_barline_thickness * self.sp;
        self.vline(list, x, top, bottom, t);
    }

    /// A thick vertical line of the bar-line thickness.
    pub(crate) fn thick_bar(&self, list: &mut DisplayList, x: f32, top: f32, bottom: f32) {
        let t = self.ed().thick_barline_thickness * self.sp;
        self.vline(list, x, top, bottom, t);
    }

    /// Distance between a thin and a thick bar line, centre to centre.
    pub(crate) fn thin_thick_step(&self) -> f32 {
        let ed = self.ed();
        (ed.thin_barline_thickness / 2.0
            + ed.thin_thick_barline_separation
            + ed.thick_barline_thickness / 2.0)
            * self.sp
    }
}

/// The accidental glyph for an alteration.
pub(crate) fn accidental_glyph(alter: i8) -> G {
    match alter {
        i8::MIN..=-2 => glyphs::DOUBLE_FLAT,
        -1 => glyphs::FLAT,
        0 => glyphs::NATURAL,
        1 => glyphs::SHARP,
        _ => glyphs::DOUBLE_SHARP,
    }
}

/// How far music-font characters inside text are raised, as a fraction of
/// the text size, so a ♯ sits with the letters instead of on the baseline.
const FALLBACK_RAISE: f32 = 0.3;

/// Tab numbers: their digits fill most of the space between tab lines.
fn tab_number_size(gap: f32) -> f32 {
    1.2 * gap
}
