// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Button icons drawn by our own renderer (UI-10): the notation buttons,
//! the mixer's toggles and the palette (docs/design/ui.md, "Palette").
//!
//! A face is a SMuFL symbol, a short text in the interface font, or a small
//! drawing made with a [`Pen`]: symbols, lines and curves placed in staff
//! spaces, the way the engraver draws the page. A face's ink says nothing
//! about its font metrics (a clef hangs far below its origin), so each face
//! is drawn once to measure its ink, then drawn again scaled, with the
//! centre of its ink on the centre of the box. Faces of one class (the note
//! values, the dynamics) share one scale so their sizes compare; text faces
//! share one size; any other face is fitted on its own, but never larger
//! than twice the note values' scale, so a lone dot or notehead stays small.
//!
//! Icons are white on transparent; Slint tints them (`colorize`) for the
//! off, on and disabled states. The approach is the GUI spike's
//! (spikes/gui/common).

use oxiguitar_display::smufl::glyph::Glyph;
use oxiguitar_display::{DisplayList, FontSet, Paint, Path, Point, Rect, Rgba, Theme};
use oxiguitar_render::{Raster, View};
use tiny_skia::Pixmap;

/// One staff space in the units faces are drawn in. Symbols are drawn at a
/// staff height of four staff spaces, as SMuFL defines a music font's size.
pub(crate) const S: f32 = 25.0;
/// The music font's size for that staff space.
const STAFF: f32 = 4.0 * S;
/// Text faces are measured at this font size.
const TEXT_SIZE: f32 = 100.0;
/// Noto Sans's cap height, as a share of its font size.
const CAP_HEIGHT: f32 = 0.714;
/// Pixels per unit when a face's ink is measured.
const PROBE_SCALE: f32 = 1.5;
/// The measured area: every face lies inside it, around its origin.
const PROBE_FROM: f32 = -160.0;
const PROBE_SPAN: f32 = 400.0;

/// The treble clef: the standard-notation button.
pub(crate) const G_CLEF: Glyph = glyph("gClef", '\u{E050}');
/// "TAB" set vertically, the six-string tablature clef: the tab button.
pub(crate) const TAB_CLEF: Glyph = glyph("6stringTabClef", '\u{E06D}');

/// A SMuFL glyph by name and codepoint; a test checks that each one's ink
/// matches the bounding box Bravura's metadata gives for the name.
pub(crate) const fn glyph(name: &'static str, cp: char) -> Glyph {
    Glyph { name, cp }
}

/// What a button shows.
#[derive(Clone, Copy)]
pub(crate) enum Face {
    /// One SMuFL symbol.
    Symbol(Glyph),
    /// Short text in the interface font (H, P.M.).
    Text(&'static str),
    /// A small drawing.
    Drawn(fn(&mut Pen<'_>)),
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Face::Symbol(g) => write!(f, "Symbol({})", g.name),
            Face::Text(t) => write!(f, "Text({t:?})"),
            Face::Drawn(_) => f.write_str("Drawn"),
        }
    }
}

/// How a face is scaled to its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    /// Shares one scale with every face of the same class in the set: the
    /// largest of them fills [`Fill::shared`] of the box. Class 0 is the
    /// note values, which also cap [`Fit::Own`] faces.
    Shared(u8),
    /// Fitted on its own to [`Fill::own`] of the box.
    Own,
    /// Text: every text face in the set has one size.
    Text,
}

/// The class of the note values.
pub(crate) const VALUES: u8 = 0;

/// How much of the box faces fill.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fill {
    /// Share of the box the largest face of a class fills.
    pub(crate) shared: f32,
    /// Share of the box a face fitted on its own fills.
    pub(crate) own: f32,
    /// Largest scale of a face fitted on its own, relative to the note
    /// values' scale (when the set has note values).
    pub(crate) own_max: f32,
    /// Cap height of text faces, as a share of the box.
    pub(crate) text: f32,
}

impl Fill {
    /// The palette's buttons.
    pub(crate) const PALETTE: Fill = Fill {
        shared: 0.78,
        own: 0.62,
        own_max: 2.0,
        text: 0.34,
    };
    /// The track rows' toggles: each symbol fills most of its button.
    pub(crate) const TOGGLES: Fill = Fill {
        shared: 0.86,
        own: 0.86,
        own_max: f32::INFINITY,
        text: 0.4,
    };
}

/// One square icon, premultiplied RGBA with white ink.
pub(crate) struct Icon {
    pub(crate) size: u32,
    pub(crate) rgba: Vec<u8>,
}

const WHITE: Theme = Theme {
    ink: Rgba::rgb(255, 255, 255),
    page: Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    },
    accent: Rgba::rgb(255, 255, 255),
    muted: Rgba::rgb(255, 255, 255),
    warning: Rgba::rgb(255, 255, 255),
};

/// Draws a face: symbols, text, lines and curves, in units where a staff
/// space is [`S`] and y points down.
pub(crate) struct Pen<'a> {
    fonts: &'a FontSet,
    list: DisplayList,
    /// Every glyph drawn, for the test that checks names and codepoints.
    glyphs: Vec<Glyph>,
}

impl Pen<'_> {
    /// A symbol with its origin at (`x`, `y`).
    pub(crate) fn sym(&mut self, g: Glyph, x: f32, y: f32) {
        self.sym_scaled(g, x, y, 1.0);
    }

    /// A symbol at `k` times its size (grace notes, small tuplet numbers).
    pub(crate) fn sym_scaled(&mut self, g: Glyph, x: f32, y: f32, k: f32) {
        self.glyphs.push(g);
        self.list.glyphs(
            self.fonts
                .symbol(g.cp, STAFF * k, Point::new(x, y), Paint::Ink),
        );
    }

    /// A symbol's advance width, so symbols can be set side by side.
    pub(crate) fn advance(&self, g: Glyph) -> f32 {
        let id = self.fonts.glyph_for(FontSet::MUSIC, g.cp);
        self.fonts.advance(FontSet::MUSIC, id, STAFF)
    }

    /// Text in the interface font, its baseline starting at (`x`, `y`).
    pub(crate) fn text(&mut self, text: &str, x: f32, y: f32, size: f32) {
        self.list.glyphs(self.fonts.text_run(
            FontSet::TEXT,
            size,
            Point::new(x, y),
            text,
            Paint::Ink,
        ));
    }

    /// Text centred on `x`.
    pub(crate) fn text_centred(&mut self, text: &str, x: f32, y: f32, size: f32) {
        let w = self.fonts.text_width(FontSet::TEXT, size, text);
        self.text(text, x - w / 2.0, y, size);
    }

    pub(crate) fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
        self.list
            .line(Point::new(x0, y0), Point::new(x1, y1), width, Paint::Ink);
    }

    /// A line through several points, with round joins.
    pub(crate) fn polyline(&mut self, points: &[(f32, f32)], width: f32) {
        let mut it = points.iter();
        let Some(&(x, y)) = it.next() else { return };
        let mut path = Path::new().move_to(x, y);
        for &(x, y) in it {
            path = path.line_to(x, y);
        }
        self.list.stroke(path, width, Paint::Ink);
    }

    pub(crate) fn stroke(&mut self, path: Path, width: f32) {
        self.list.stroke(path, width, Paint::Ink);
    }

    pub(crate) fn fill(&mut self, path: Path) {
        self.list.fill(path, Paint::Ink);
    }

    pub(crate) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.list.rect(Rect::new(x, y, w, h), Paint::Ink);
    }

    /// A rectangle's outline.
    pub(crate) fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32) {
        let path = Path::new()
            .move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close();
        self.stroke(path, width);
    }

    /// A circle's outline (`width` > 0) or a filled disc (`width` = 0).
    pub(crate) fn circle(&mut self, cx: f32, cy: f32, r: f32, width: f32) {
        // Four cubic quarter circles.
        const K: f32 = 0.552_284_8;
        let k = K * r;
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
        if width > 0.0 {
            self.stroke(path, width);
        } else {
            self.fill(path);
        }
    }

    /// A filled arrowhead with its tip at (`x`, `y`), pointing along
    /// (`dx`, `dy`).
    pub(crate) fn arrowhead(&mut self, x: f32, y: f32, dx: f32, dy: f32, size: f32) {
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (ux, uy) = (dx / len, dy / len);
        let (bx, by) = (x - ux * size, y - uy * size);
        let (px, py) = (-uy * size * 0.5, ux * size * 0.5);
        let path = Path::new()
            .move_to(x, y)
            .line_to(bx + px, by + py)
            .line_to(bx - px, by - py)
            .close();
        self.fill(path);
    }

    /// A tie or a slur from `a` to `b`, bulging by `h` (down when positive),
    /// thicker in the middle, as the engraver draws them.
    pub(crate) fn arc(&mut self, a: (f32, f32), b: (f32, f32), h: f32) {
        // Bravura's slur thickness in the middle, in staff spaces.
        const MID: f32 = 0.22;
        let outer = h * 4.0 / 3.0;
        let inner = (h.abs() - MID * S).max(0.0).copysign(h) * 4.0 / 3.0;
        let c = |t: f32, off: f32| Point::new(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t + off);
        let path = Path::new()
            .move_to(a.0, a.1)
            .cubic_to(c(0.25, outer), c(0.75, outer), Point::new(b.0, b.1))
            .cubic_to(c(0.75, inner), c(0.25, inner), Point::new(a.0, a.1))
            .close();
        self.fill(path);
    }

    /// The width of `text` in the interface font.
    pub(crate) fn text_width(&self, text: &str, size: f32) -> f32 {
        self.fonts.text_width(FontSet::TEXT, size, text)
    }

    /// A wavy vertical line from `y0` to `y1` around `x`.
    pub(crate) fn wave_down(&mut self, x: f32, y0: f32, y1: f32, n: u32, h: f32, width: f32) {
        let step = (y1 - y0) / (n as f32 * 2.0);
        let mut path = Path::new().move_to(x, y0);
        for i in 0..n * 2 {
            let ya = y0 + step * i as f32;
            let d = if i % 2 == 0 { -h } else { h };
            path = path.cubic_to(
                Point::new(x + d, ya + step * 0.35),
                Point::new(x + d, ya + step * 0.65),
                Point::new(x, ya + step),
            );
        }
        self.stroke(path, width);
    }
}

/// Where a face's ink lies, in its own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ink {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

impl Ink {
    fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    fn centre(&self) -> (f32, f32) {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }
}

/// A face as a display list in its own units, and the glyphs it uses.
fn list_of(fonts: &FontSet, face: Face) -> (DisplayList, Vec<Glyph>) {
    let mut pen = Pen {
        fonts,
        list: DisplayList::new(PROBE_SPAN, PROBE_SPAN),
        glyphs: Vec::new(),
    };
    match face {
        Face::Symbol(g) => pen.sym(g, 0.0, 0.0),
        Face::Text(t) => pen.text(t, 0.0, 0.0, TEXT_SIZE),
        Face::Drawn(draw) => draw(&mut pen),
    }
    (pen.list, pen.glyphs)
}

/// Draws `list` with unit (`x`, `y`) at the image's top-left corner.
fn render(raster: &mut Raster, fonts: &FontSet, list: &DisplayList, view: View) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(view.width, view.height)?;
    raster.render_into(&mut pixmap, list, fonts, &WHITE, &view);
    Some(pixmap)
}

/// The bounding box of an image's ink in pixels, ignoring faint
/// anti-aliased fringes.
fn ink_pixels(pixmap: &Pixmap) -> Option<Ink> {
    let (w, h) = (pixmap.width(), pixmap.height());
    let data = pixmap.data();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..h {
        let row = data.get((y * w * 4) as usize..((y + 1) * w * 4) as usize)?;
        for (x, [_, _, _, alpha]) in row.as_chunks::<4>().0.iter().enumerate() {
            if *alpha > 24 {
                let x = x as u32;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then_some(Ink {
        x0: x0 as f32,
        y0: y0 as f32,
        x1: x1 as f32,
        y1: y1 as f32,
    })
}

/// Measures a face's ink in its own units.
fn measure(raster: &mut Raster, fonts: &FontSet, list: &DisplayList) -> Option<Ink> {
    let px = (PROBE_SPAN * PROBE_SCALE) as u32;
    let view = View {
        scale: PROBE_SCALE,
        x: PROBE_FROM,
        y: PROBE_FROM,
        width: px,
        height: px,
    };
    let p = ink_pixels(&render(raster, fonts, list, view)?)?;
    let unit = |v: f32| v / PROBE_SCALE + PROBE_FROM;
    Some(Ink {
        x0: unit(p.x0),
        y0: unit(p.y0),
        x1: unit(p.x1),
        y1: unit(p.y1),
    })
}

/// A set of faces measured once, drawn at any size.
pub(crate) struct IconSet {
    fonts: FontSet,
    faces: Vec<(DisplayList, Fit)>,
    inks: Vec<Option<Ink>>,
    fill: Fill,
}

impl IconSet {
    /// Measures every face.
    pub(crate) fn new(faces: &[(Face, Fit)], fill: Fill) -> IconSet {
        let fonts = FontSet::bundled();
        let mut raster = Raster::new();
        let lists: Vec<(DisplayList, Fit)> = faces
            .iter()
            .map(|&(face, fit)| (list_of(&fonts, face).0, fit))
            .collect();
        let inks = lists
            .iter()
            .map(|(list, _)| measure(&mut raster, &fonts, list))
            .collect();
        IconSet {
            fonts,
            faces: lists,
            inks,
            fill,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.faces.len()
    }

    /// Pixels per unit for each face in a `px` box.
    fn scales(&self, px: u32) -> Vec<f32> {
        let px = px as f32;
        let fit = |ink: &Ink, share: f32| (px * share / ink.width()).min(px * share / ink.height());
        let class_scale = |class: u8| {
            self.faces
                .iter()
                .zip(&self.inks)
                .filter(|((_, fit), _)| *fit == Fit::Shared(class))
                .filter_map(|(_, ink)| ink.as_ref().map(|i| fit(i, self.fill.shared)))
                .fold(f32::INFINITY, f32::min)
        };
        let values = class_scale(VALUES);
        // Text shares one cap height, shrunk for all if the widest would
        // not fit.
        let text = self
            .faces
            .iter()
            .zip(&self.inks)
            .filter(|((_, fit), _)| *fit == Fit::Text)
            .filter_map(|(_, ink)| ink.as_ref().map(|i| fit(i, 0.8)))
            .fold(px * self.fill.text / (TEXT_SIZE * CAP_HEIGHT), f32::min);
        self.faces
            .iter()
            .zip(&self.inks)
            .map(|((_, f), ink)| {
                let Some(ink) = ink else { return 1.0 };
                match *f {
                    Fit::Shared(class) => class_scale(class),
                    Fit::Own => fit(ink, self.fill.own).min(values * self.fill.own_max),
                    Fit::Text => text,
                }
            })
            .collect()
    }

    /// Every icon, `px` pixels square.
    pub(crate) fn draw(&self, px: u32) -> Vec<Icon> {
        let mut raster = Raster::new();
        let half = px as f32 / 2.0;
        self.faces
            .iter()
            .zip(&self.inks)
            .zip(self.scales(px))
            .map(|(((list, _), ink), k)| {
                let blank = || Icon {
                    size: px,
                    rgba: vec![0; (px * px * 4) as usize],
                };
                let Some(ink) = ink else { return blank() };
                let (cx, cy) = ink.centre();
                let view = View {
                    scale: k,
                    x: cx - half / k,
                    y: cy - half / k,
                    width: px,
                    height: px,
                };
                match render(&mut raster, &self.fonts, list, view) {
                    Some(p) => Icon {
                        size: px,
                        rgba: p.take(),
                    },
                    None => blank(),
                }
            })
            .collect()
    }
}

/// The track rows' toggles, in order: standard notation, tablature, mute
/// and solo.
pub(crate) fn toggles() -> [(Face, Fit); 4] {
    [
        (Face::Symbol(G_CLEF), Fit::Own),
        (Face::Symbol(TAB_CLEF), Fit::Own),
        (Face::Drawn(mute), Fit::Own),
        (Face::Drawn(solo), Fit::Own),
    ]
}

/// A loudspeaker, crossed out.
fn mute(p: &mut Pen<'_>) {
    crate::palette::speaker(p, 0.0, 0.0, 1.2);
    let (x0, x1, h) = (2.3 * S, 3.7 * S, 0.7 * S);
    p.line(x0, -h, x1, h, 0.25 * S);
    p.line(x0, h, x1, -h, 0.25 * S);
}

/// Headphones.
fn solo(p: &mut Pen<'_>) {
    const K: f32 = 0.552_284_8;
    let r = 1.5 * S;
    let band = Path::new()
        .move_to(-r, 0.6 * S)
        .line_to(-r, 0.0)
        .cubic_to(
            Point::new(-r, -K * r),
            Point::new(-K * r, -r),
            Point::new(0.0, -r),
        )
        .cubic_to(
            Point::new(K * r, -r),
            Point::new(r, -K * r),
            Point::new(r, 0.0),
        )
        .line_to(r, 0.6 * S);
    p.stroke(band, 0.25 * S);
    let (w, h) = (0.75 * S, 1.3 * S);
    p.rect(-r - 0.3 * S, 0.1 * S, w, h);
    p.rect(r + 0.3 * S - w, 0.1 * S, w, h);
}

/// Every glyph a face draws, for tests.
#[cfg(test)]
pub(crate) fn glyphs_of(face: Face) -> Vec<Glyph> {
    list_of(&FontSet::bundled(), face).1
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every icon has ink, inside its box and centred within a pixel.
    pub(crate) fn check_inside_and_centred(set: &IconSet, names: &[&str], sizes: &[u32]) {
        for &px in sizes {
            for (icon, name) in set.draw(px).iter().zip(names) {
                let pixmap = Pixmap::from_vec(
                    icon.rgba.clone(),
                    tiny_skia::IntSize::from_wh(px, px).unwrap(),
                )
                .unwrap();
                let ink = ink_pixels(&pixmap).unwrap_or_else(|| panic!("{name} has no ink"));
                assert!(
                    ink.x0 >= 1.0 && ink.y0 >= 1.0,
                    "{name} touches the top or left edge at {px} px"
                );
                assert!(
                    ink.x1 <= px as f32 - 1.0 && ink.y1 <= px as f32 - 1.0,
                    "{name} touches the bottom or right edge at {px} px"
                );
                let (cx, cy) = ink.centre();
                let half = px as f32 / 2.0;
                assert!(
                    (cx - half).abs() <= 1.0 && (cy - half).abs() <= 1.0,
                    "{name} centred at ({cx}, {cy}) in a {px} px box"
                );
            }
        }
    }

    /// Each glyph's ink matches the bounding box Bravura's metadata gives
    /// for its name, so a wrong codepoint can't pass for the right symbol.
    pub(crate) fn check_glyphs(glyphs: &[Glyph]) {
        let meta = oxiguitar_display::smufl::Metadata::bravura().unwrap();
        let fonts = FontSet::bundled();
        let mut raster = Raster::new();
        for g in glyphs {
            let bbox = meta
                .bbox(g.name)
                .unwrap_or_else(|| panic!("{} is not a SMuFL name in Bravura", g.name));
            assert_ne!(
                fonts.glyph_for(FontSet::MUSIC, g.cp),
                0,
                "{} not in the font",
                g.name
            );
            let (list, _) = list_of(&fonts, Face::Symbol(*g));
            let ink = measure(&mut raster, &fonts, &list).expect("ink");
            // Units to staff spaces, y up as SMuFL has it.
            let got = [ink.x0 / S, -ink.y1 / S, ink.x1 / S, -ink.y0 / S];
            let want = [bbox.sw[0], bbox.sw[1], bbox.ne[0], bbox.ne[1]];
            for (a, b) in got.iter().zip(want) {
                assert!(
                    (a - b).abs() <= 0.12,
                    "{} (U+{:04X}): ink {got:?}, metadata {want:?}",
                    g.name,
                    g.cp as u32
                );
            }
        }
    }

    #[test]
    fn notation_glyphs_are_what_they_say() {
        check_glyphs(&[G_CLEF, TAB_CLEF]);
    }
}
