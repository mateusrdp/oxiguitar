// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Pixel backend: tiny-skia, on the CPU.
//!
//! A [`View`] says which part of the page to draw and at what scale, so the
//! GUI renders only the visible part of the score at the current zoom and the
//! screen's pixel density. Glyph outlines are read once per glyph with skrifa
//! and cached.

use std::collections::HashMap;

use oxiguitar_display::{
    DisplayList, FontId, FontSet, GlyphRun, Item, Paint, Path, PathCmd, Rect, Rgba, Theme,
};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, MetadataProvider, instance::LocationRef, instance::Size};
use tiny_skia::{FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

/// The part of a page to draw, and how large.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// Pixels per point. 1.0 is 72 dpi; a 100 % zoom on a typical screen is
    /// about 1.33 (96 dpi), times the screen's scale factor.
    pub scale: f32,
    /// Left edge of the visible area, in points.
    pub x: f32,
    /// Top edge of the visible area, in points.
    pub y: f32,
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
}

impl View {
    /// The whole page at `scale` pixels per point.
    pub fn page(list: &DisplayList, scale: f32) -> Self {
        Self {
            scale,
            x: 0.0,
            y: 0.0,
            width: (list.width * scale).ceil().max(1.0) as u32,
            height: (list.height * scale).ceil().max(1.0) as u32,
        }
    }

    /// The visible area, in points.
    pub fn area(&self) -> Rect {
        Rect::new(
            self.x,
            self.y,
            self.width as f32 / self.scale,
            self.height as f32 / self.scale,
        )
    }

    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_translate(-self.x, -self.y)
    }
}

/// Draws display lists into pixel images, caching glyph outlines.
#[derive(Debug, Default)]
pub struct Raster {
    glyphs: HashMap<(FontId, u16), Option<tiny_skia::Path>>,
}

impl Raster {
    /// A renderer with an empty glyph cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of glyph outlines cached so far.
    pub fn cached_glyphs(&self) -> usize {
        self.glyphs.len()
    }

    /// Draw the part of `list` that `view` covers, on a page of the theme's
    /// colour. Returns `None` only if the image size is zero.
    pub fn render(
        &mut self,
        list: &DisplayList,
        fonts: &FontSet,
        theme: &Theme,
        view: &View,
    ) -> Option<Pixmap> {
        let mut pixmap = Pixmap::new(view.width, view.height)?;
        pixmap.fill(color(theme.page));
        self.render_into(&mut pixmap, list, fonts, theme, view);
        Some(pixmap)
    }

    /// Draw onto an existing image without clearing it.
    pub fn render_into(
        &mut self,
        pixmap: &mut Pixmap,
        list: &DisplayList,
        fonts: &FontSet,
        theme: &Theme,
        view: &View,
    ) {
        let area = view.area();
        let base = view.transform();
        for item in &list.items {
            if !item.bounds().intersects(&area) {
                continue;
            }
            match item {
                Item::Glyphs(run) => self.draw_run(pixmap, run, fonts, theme, base),
                Item::Line {
                    from,
                    to,
                    width,
                    paint,
                } => {
                    let mut pb = PathBuilder::new();
                    pb.move_to(from.x, from.y);
                    pb.line_to(to.x, to.y);
                    if let Some(path) = pb.finish() {
                        let stroke = Stroke {
                            width: *width,
                            line_cap: LineCap::Butt,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint_of(theme, *paint), &stroke, base, None);
                    }
                }
                Item::Rect { rect, paint } => {
                    if let Some(r) =
                        tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
                    {
                        pixmap.fill_rect(r, &paint_of(theme, *paint), base, None);
                    }
                }
                Item::Path { path, fill, stroke } => {
                    let Some(p) = to_skia_path(path) else {
                        continue;
                    };
                    if let Some(f) = fill {
                        pixmap.fill_path(&p, &paint_of(theme, *f), FillRule::Winding, base, None);
                    }
                    if let Some((w, s)) = stroke {
                        let st = Stroke {
                            width: *w,
                            line_join: LineJoin::Round,
                            line_cap: LineCap::Round,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&p, &paint_of(theme, *s), &st, base, None);
                    }
                }
            }
        }
    }

    fn draw_run(
        &mut self,
        pixmap: &mut Pixmap,
        run: &GlyphRun,
        fonts: &FontSet,
        theme: &Theme,
        base: Transform,
    ) {
        let Some(font) = fonts.get(run.font) else {
            return;
        };
        let scale = run.size / f32::from(font.units_per_em().max(1));
        let paint = paint_of(theme, run.paint);
        for g in &run.glyphs {
            let outline = self
                .glyphs
                .entry((run.font, g.id))
                .or_insert_with(|| outline(fonts, run.font, g.id));
            let Some(path) = outline else { continue };
            let t = base
                .pre_translate(run.origin.x + g.x, run.origin.y)
                .pre_scale(scale, -scale);
            pixmap.fill_path(path, &paint, FillRule::Winding, t, None);
        }
    }
}

/// A glyph's outline in font units, y up, or `None` for empty glyphs.
fn outline(fonts: &FontSet, font: FontId, glyph: u16) -> Option<tiny_skia::Path> {
    let data = fonts.get(font)?;
    let f = data.font_ref();
    let og = f.outline_glyphs().get(GlyphId::new(u32::from(glyph)))?;
    let mut pen = SkiaPen(PathBuilder::new());
    og.draw(
        DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
        &mut pen,
    )
    .ok()?;
    pen.0.finish()
}

struct SkiaPen(PathBuilder);

impl OutlinePen for SkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

fn to_skia_path(path: &Path) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for cmd in &path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => pb.move_to(p.x, p.y),
            PathCmd::LineTo(p) => pb.line_to(p.x, p.y),
            PathCmd::CubicTo(a, b, c) => pb.cubic_to(a.x, a.y, b.x, b.y, c.x, c.y),
            PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

fn color(c: Rgba) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn paint_of(theme: &Theme, paint: Paint) -> tiny_skia::Paint<'static> {
    let mut p = tiny_skia::Paint::default();
    p.set_color(color(theme.resolve(paint)));
    p.anti_alias = true;
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_display::{Item, Point, smufl::Metadata, specimen};

    #[test]
    fn draws_a_glyph_where_the_list_says() {
        let fonts = FontSet::bundled();
        let mut list = DisplayList::new(40.0, 40.0);
        // A black notehead, 20 pt staff: about 5.9 pt wide, 5 pt tall.
        list.glyphs(fonts.symbol('\u{E0A4}', 20.0, Point::new(10.0, 20.0), Paint::Ink));
        let px = Raster::new()
            .render(&list, &fonts, &Theme::LIGHT, &View::page(&list, 1.0))
            .unwrap();
        let at = |x: u32, y: u32| px.pixel(x, y).unwrap().red();
        assert!(at(13, 20) < 64, "notehead centre should be dark");
        assert!(at(30, 30) > 250, "elsewhere is page");
    }

    #[test]
    fn viewport_draws_only_its_part() {
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap();
        let page = specimen::guitar_excerpt(&fonts, &meta);
        let mut r = Raster::new();
        let full = r
            .render(&page, &fonts, &Theme::LIGHT, &View::page(&page, 2.0))
            .unwrap();
        // A 200 x 100 pt window at 2x, starting at (100, 180).
        let view = View {
            scale: 2.0,
            x: 100.0,
            y: 180.0,
            width: 400,
            height: 200,
        };
        let part = r.render(&page, &fonts, &Theme::LIGHT, &view).unwrap();
        let mut differ = 0;
        for y in 0..200 {
            for x in 0..400 {
                let a = part.pixel(x, y).unwrap();
                let b = full.pixel(x + 200, y + 360).unwrap();
                if a.red().abs_diff(b.red()) > 8 {
                    differ += 1;
                }
            }
        }
        // Anti-aliasing may round a few edge pixels differently when the
        // origin moves; anything beyond that means items were dropped or moved.
        assert!(differ < 40, "{differ} of 80000 pixels differ");
    }

    /// Timings for ADR 0005. Run with
    /// `cargo test --release -p oxiguitar-render -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark; prints timings"]
    fn raster_speed() {
        use std::time::Instant;
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap();
        let specimen = specimen::guitar_excerpt(&fonts, &meta);
        // A dense page: the excerpt's system six times, top to bottom.
        let system: Vec<Item> = specimen
            .items
            .iter()
            .filter(|i| (150.0..330.0).contains(&i.bounds().y))
            .cloned()
            .collect();
        let mut page = DisplayList::a4();
        for k in 0..6 {
            page.items.extend(
                system
                    .iter()
                    .map(|i| i.offset(0.0, -110.0 + k as f32 * 125.0)),
            );
        }
        println!("dense page: {} items", page.items.len());
        let mut r = Raster::new();
        let cases = [
            ("A4 page at 200 % (2.67 px/pt)", View::page(&page, 2.67)),
            (
                "1920x1080 window at 100 % (1.33 px/pt)",
                View {
                    scale: 1.33,
                    x: 0.0,
                    y: 0.0,
                    width: 1920,
                    height: 1080,
                },
            ),
            (
                "1920x1080 window at 400 % (5.33 px/pt)",
                View {
                    scale: 5.33,
                    x: 60.0,
                    y: 60.0,
                    width: 1920,
                    height: 1080,
                },
            ),
            (
                "3840x2160 window at 200 %, HiDPI (5.33 px/pt)",
                View {
                    scale: 5.33,
                    x: 0.0,
                    y: 0.0,
                    width: 3840,
                    height: 2160,
                },
            ),
        ];
        for (name, view) in cases {
            let _ = r.render(&page, &fonts, &Theme::LIGHT, &view);
            let n = 20;
            let t = Instant::now();
            for _ in 0..n {
                let _ = r.render(&page, &fonts, &Theme::LIGHT, &view);
            }
            println!(
                "{name}: {:.1} ms per frame",
                t.elapsed().as_secs_f64() * 1000.0 / f64::from(n)
            );
        }
    }

    #[test]
    fn dark_theme_inverts_the_page() {
        let fonts = FontSet::bundled();
        let list = DisplayList::new(10.0, 10.0);
        let px = Raster::new()
            .render(&list, &fonts, &Theme::DARK, &View::page(&list, 1.0))
            .unwrap();
        assert!(px.pixel(5, 5).unwrap().red() < 64);
    }
}
