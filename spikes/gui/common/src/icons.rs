// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Palette button icons, drawn by our own renderer.
//!
//! Font line metrics say nothing about where a music symbol's ink is: a
//! quarter note's stem rises far above its notehead. So each face is drawn
//! once at a large size to measure its ink, scaled, and drawn again with the
//! centre of its ink's bounding box on the centre of the box.
//!
//! Scaling: note values (a family that must keep its relative sizes) share
//! one scale, set by the largest of them. Other symbols are fitted one by one
//! to a smaller share of the box, so an arrow and an accent look about as
//! heavy as each other, but never larger than 1.5 times the note-value scale,
//! so a staccato dot stays a dot.
//!
//! Icons are white on transparent; the toolkit tints them (Slint's
//! `colorize`) for the normal and selected states.

use oxiguitar_display::{DisplayList, FontId, FontSet, Paint, Point, Rgba, Theme};
use oxiguitar_render::{Raster, View};
use tiny_skia::Pixmap;

use crate::{Face, TOOLS};

/// Share of the icon box the tallest note value fills.
const FILL: f32 = 0.78;
/// Share of the box other symbols are fitted to.
const FILL_OTHER: f32 = 0.56;
/// Largest scale for other symbols, relative to the note-value scale.
const OTHER_MAX: f32 = 1.5;
/// The palette group whose symbols share one scale.
const FAMILY: &str = "Note values";
/// Text faces (H, P.M.) are set at this share of the box height at most.
const TEXT_HEIGHT: f32 = 0.34;
/// Size the faces are measured at, in pixels.
const PROBE_SIZE: f32 = 200.0;

/// One square icon.
pub struct Icon {
    /// Width and height in physical pixels.
    pub size: u32,
    /// Premultiplied RGBA, white ink.
    pub rgba: Vec<u8>,
}

/// Where a face's ink lies relative to its origin, at `PROBE_SIZE`.
#[derive(Debug, Clone, Copy)]
struct Ink {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
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

fn font_and_text(face: Face) -> (FontId, String) {
    match face {
        Face::Symbol(s) => (FontSet::MUSIC, s.to_owned()),
        Face::Text(s) => (FontSet::TEXT, s.to_owned()),
    }
}

fn draw(
    fonts: &FontSet,
    raster: &mut Raster,
    face: Face,
    size: f32,
    origin: Point,
    px: u32,
) -> Option<Pixmap> {
    let (font, text) = font_and_text(face);
    let mut list = DisplayList::new(px as f32, px as f32);
    list.glyphs(fonts.text_run(font, size, origin, &text, Paint::Ink));
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

/// The ink's bounding box of a pixmap drawn with its origin at `origin`.
fn ink_of(pixmap: &Pixmap, origin: Point) -> Option<Ink> {
    let (w, h) = (pixmap.width(), pixmap.height());
    let data = pixmap.data();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            // Alpha of the pixel; faint anti-aliasing fringes don't count.
            if data[((y * w + x) * 4 + 3) as usize] > 24 {
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

/// Icons for every tool in `TOOLS`, `px` physical pixels square.
pub fn tool_icons(px: u32) -> Vec<Icon> {
    let fonts = FontSet::bundled();
    let mut raster = Raster::new();
    let probe_px = (PROBE_SIZE * 3.0) as u32;
    let probe_origin = Point::new(PROBE_SIZE, PROBE_SIZE * 2.0);
    let inks: Vec<Option<Ink>> = TOOLS
        .iter()
        .map(|t| {
            draw(
                &fonts,
                &mut raster,
                t.face,
                PROBE_SIZE,
                probe_origin,
                probe_px,
            )
            .and_then(|p| ink_of(&p, probe_origin))
        })
        .collect();

    // Scale that makes a face fill `share` of the box, in pixels per probe pixel.
    let fit = |ink: &Ink, share: f32| {
        let room = px as f32 * share;
        (room / ink.width()).min(room / ink.height())
    };
    let family_scale = TOOLS
        .iter()
        .zip(&inks)
        .filter(|(t, _)| t.group == FAMILY && matches!(t.face, Face::Symbol(_)))
        .filter_map(|(_, ink)| ink.as_ref().map(|i| fit(i, FILL)))
        .fold(f32::INFINITY, f32::min);
    // Text faces share one size too: a fixed cap height, shrunk for all of
    // them if the widest would not fit.
    let text_scale = TOOLS
        .iter()
        .zip(&inks)
        .filter(|(t, _)| matches!(t.face, Face::Text(_)))
        .filter_map(|(_, ink)| ink.as_ref().map(|i| fit(i, 0.7)))
        .fold(px as f32 * TEXT_HEIGHT / (PROBE_SIZE * 0.714), f32::min);

    TOOLS
        .iter()
        .zip(&inks)
        .map(|(tool, ink)| {
            let blank = Icon {
                size: px,
                rgba: vec![0; (px * px * 4) as usize],
            };
            let Some(ink) = ink else { return blank };
            let k = match tool.face {
                Face::Symbol(_) if tool.group == FAMILY => family_scale.min(fit(ink, FILL)),
                Face::Symbol(_) => fit(ink, FILL_OTHER).min(family_scale * OTHER_MAX),
                Face::Text(_) => text_scale,
            };
            let (cx, cy) = ink.centre();
            let half = px as f32 / 2.0;
            let origin = Point::new(half - cx * k, half - cy * k);
            match draw(&fonts, &mut raster, tool.face, PROBE_SIZE * k, origin, px) {
                Some(p) => Icon {
                    size: px,
                    rgba: p.take(),
                },
                None => blank,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon's ink is inside the box and centred on it within a pixel.
    #[test]
    fn icons_are_inside_and_centred() {
        for px in [36, 54, 72] {
            let icons = tool_icons(px);
            for (tool, icon) in TOOLS.iter().zip(&icons) {
                let pixmap = Pixmap::from_vec(
                    icon.rgba.clone(),
                    tiny_skia::IntSize::from_wh(px, px).unwrap(),
                )
                .unwrap();
                let ink = ink_of(&pixmap, Point::new(0.0, 0.0))
                    .unwrap_or_else(|| panic!("{} has no ink", tool.id));
                assert!(
                    ink.x0 >= 1.0 && ink.y0 >= 1.0,
                    "{} touches the top or left edge at {px}px",
                    tool.id
                );
                assert!(
                    ink.x1 <= px as f32 - 1.0 && ink.y1 <= px as f32 - 1.0,
                    "{} touches the bottom or right edge at {px}px",
                    tool.id
                );
                let (cx, cy) = ink.centre();
                let half = px as f32 / 2.0;
                assert!(
                    (cx - half).abs() <= 1.0 && (cy - half).abs() <= 1.0,
                    "{} centred at ({cx}, {cy}) in a {px}px box",
                    tool.id
                );
            }
        }
    }
}
