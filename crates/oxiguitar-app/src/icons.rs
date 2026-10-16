// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Button icons drawn by our own renderer from the music font (UI-10).
//!
//! A music symbol's ink says nothing about its font metrics (a clef hangs
//! far below its origin), so each symbol is drawn once large to measure its
//! ink, then drawn again scaled, with the centre of its ink on the centre of
//! the box. Each symbol is fitted to the box on its own, so a tall clef and
//! the narrower TAB sign read equally well.
//! Icons are white on transparent; Slint tints them (`colorize`) for the off
//! and on states. The approach is the GUI spike's (spikes/gui/common).

use oxiguitar_display::{DisplayList, FontSet, Paint, Point, Rgba, Theme};
use oxiguitar_render::{Raster, View};
use tiny_skia::Pixmap;

/// The treble clef: the standard-notation button.
pub(crate) const G_CLEF: char = '\u{E050}';
/// "TAB" set vertically, the six-string tablature clef: the tab button.
pub(crate) const TAB_CLEF: char = '\u{E06D}';

/// Share of the box each symbol fills.
const FILL: f32 = 0.86;
/// Size symbols are measured at, in pixels.
const PROBE: f32 = 200.0;

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
};

/// Where a symbol's ink lies, relative to its origin.
#[derive(Clone, Copy, Debug)]
struct Ink {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

fn draw(
    fonts: &FontSet,
    raster: &mut Raster,
    symbol: char,
    size: f32,
    at: Point,
    px: u32,
) -> Option<Pixmap> {
    let mut list = DisplayList::new(px as f32, px as f32);
    list.glyphs(fonts.symbol(symbol, size, at, Paint::Ink));
    let mut pixmap = Pixmap::new(px, px)?;
    let view = View {
        scale: 1.0,
        x: 0.0,
        y: 0.0,
        width: px,
        height: px,
    };
    raster.render_into(&mut pixmap, &list, fonts, &WHITE, &view);
    Some(pixmap)
}

/// The bounding box of the ink, ignoring faint anti-aliased fringes.
fn ink_of(pixmap: &Pixmap, origin: Point) -> Option<Ink> {
    let (w, h) = (pixmap.width(), pixmap.height());
    let data = pixmap.data();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let alpha = data
                .get(((y * w + x) * 4 + 3) as usize)
                .copied()
                .unwrap_or(0);
            if alpha > 24 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then_some(Ink {
        x0: x0 as f32 - origin.x,
        y0: y0 as f32 - origin.y,
        x1: x1 as f32 - origin.x,
        y1: y1 as f32 - origin.y,
    })
}

/// Icons for `symbols`, `px` pixels square, each fitted to the box.
pub(crate) fn symbol_icons(symbols: &[char], px: u32) -> Vec<Icon> {
    let fonts = FontSet::bundled();
    let mut raster = Raster::new();
    let probe_px = (PROBE * 3.0) as u32;
    let probe_at = Point::new(PROBE, PROBE * 2.0);
    let inks: Vec<Option<Ink>> = symbols
        .iter()
        .map(|&s| {
            draw(&fonts, &mut raster, s, PROBE, probe_at, probe_px)
                .and_then(|p| ink_of(&p, probe_at))
        })
        .collect();
    let room = px as f32 * FILL;
    symbols
        .iter()
        .zip(&inks)
        .map(|(&s, ink)| {
            let blank = || Icon {
                size: px,
                rgba: vec![0; (px * px * 4) as usize],
            };
            let Some(ink) = ink else { return blank() };
            let scale = (room / (ink.x1 - ink.x0)).min(room / (ink.y1 - ink.y0));
            let (cx, cy) = ((ink.x0 + ink.x1) / 2.0, (ink.y0 + ink.y1) / 2.0);
            let half = px as f32 / 2.0;
            let at = Point::new(half - cx * scale, half - cy * scale);
            match draw(&fonts, &mut raster, s, PROBE * scale, at, px) {
                Some(p) => Icon {
                    size: px,
                    rgba: p.take(),
                },
                None => blank(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both icons have ink, inside the box and centred within a pixel.
    #[test]
    fn icons_are_inside_and_centred() {
        for px in [20, 28, 40, 56] {
            for (i, icon) in symbol_icons(&[G_CLEF, TAB_CLEF], px).iter().enumerate() {
                let pixmap = Pixmap::from_vec(
                    icon.rgba.clone(),
                    tiny_skia::IntSize::from_wh(px, px).unwrap(),
                )
                .unwrap();
                let ink = ink_of(&pixmap, Point::new(0.0, 0.0)).expect("ink");
                assert!(ink.x0 >= 1.0 && ink.y0 >= 1.0, "icon {i} at {px}px");
                assert!(ink.x1 <= px as f32 - 1.0 && ink.y1 <= px as f32 - 1.0);
                let (cx, cy) = ((ink.x0 + ink.x1) / 2.0, (ink.y0 + ink.y1) / 2.0);
                let half = px as f32 / 2.0;
                assert!((cx - half).abs() <= 1.0 && (cy - half).abs() <= 1.0);
            }
        }
    }
}
