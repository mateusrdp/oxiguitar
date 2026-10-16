// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score canvas as tiles: where the engraved sheets sit, which tiles a
//! scrolled view needs, and drawing one tile. Window-free, so it is tested
//! without a display.
//!
//! Canvas space is in points. Pages of the Page layout stack down it with a
//! gap; the other layouts give one sheet. A tile is [`TILE`] device pixels
//! square at the current zoom, so its size in points depends on the scale.

use oxiguitar_display::{DisplayList, FontSet, Rect, Theme};
use oxiguitar_render::{Raster, View};
use tiny_skia::{Color, Paint, Pixmap, Transform};

/// Tile edge in device pixels.
pub(crate) const TILE: u32 = 512;
/// Gap between pages, in points.
pub(crate) const PAGE_GAP: f32 = 18.0;

/// The engraved sheets and where each sits in canvas space.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sheets {
    pub(crate) lists: Vec<DisplayList>,
    pub(crate) rects: Vec<Rect>,
}

impl Sheets {
    /// Stacks the sheets down the canvas, left-aligned.
    pub(crate) fn stack(lists: Vec<DisplayList>) -> Sheets {
        let mut y = 0.0;
        let mut rects = Vec::with_capacity(lists.len());
        for l in &lists {
            rects.push(Rect::new(0.0, y, l.width, l.height));
            y += l.height + PAGE_GAP;
        }
        Sheets { lists, rects }
    }

    /// Size of the whole canvas in points.
    pub(crate) fn size(&self) -> (f32, f32) {
        let w = self.rects.iter().map(Rect::right).fold(0.0, f32::max);
        let h = self.rects.iter().map(Rect::bottom).fold(0.0, f32::max);
        (w, h)
    }

    /// The tiles, as (column, row), that cover the canvas area `visible`
    /// (in points) at `scale` device pixels per point, nearest the centre
    /// first, so the middle of the screen fills in first.
    pub(crate) fn tiles_for(&self, visible: Rect, scale: f32) -> Vec<(u32, u32)> {
        let (w, h) = self.size();
        if scale <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Vec::new();
        }
        let tile = TILE as f32 / scale;
        let x0 = visible.x.max(0.0);
        let y0 = visible.y.max(0.0);
        let x1 = visible.right().min(w);
        let y1 = visible.bottom().min(h);
        if x1 <= x0 || y1 <= y0 {
            return Vec::new();
        }
        let (c0, c1) = ((x0 / tile) as u32, ((x1 / tile).ceil() as u32).max(1));
        let (r0, r1) = ((y0 / tile) as u32, ((y1 / tile).ceil() as u32).max(1));
        let (cx, cy) = ((x0 + x1) / 2.0 / tile, (y0 + y1) / 2.0 / tile);
        let mut out: Vec<(u32, u32)> = (r0..r1)
            .flat_map(|r| (c0..c1).map(move |c| (c, r)))
            .filter(|&(c, r)| self.tile_has_sheet(c, r, scale))
            .collect();
        out.sort_by(|a, b| {
            let d = |&(c, r): &(u32, u32)| {
                (c as f32 + 0.5 - cx).powi(2) + (r as f32 + 0.5 - cy).powi(2)
            };
            d(a).total_cmp(&d(b))
        });
        out
    }

    /// The canvas area of tile (`col`, `row`) in points.
    pub(crate) fn tile_rect(col: u32, row: u32, scale: f32) -> Rect {
        let tile = TILE as f32 / scale;
        Rect::new(col as f32 * tile, row as f32 * tile, tile, tile)
    }

    fn tile_has_sheet(&self, col: u32, row: u32, scale: f32) -> bool {
        let area = Self::tile_rect(col, row, scale);
        self.rects.iter().any(|r| r.intersects(&area))
    }

    /// Draws tile (`col`, `row`): the pages it overlaps on a transparent
    /// background, so the canvas colour shows between pages.
    pub(crate) fn draw_tile(
        &self,
        raster: &mut Raster,
        fonts: &FontSet,
        theme: &Theme,
        col: u32,
        row: u32,
        scale: f32,
    ) -> Option<Pixmap> {
        let mut pixmap = Pixmap::new(TILE, TILE)?;
        let area = Self::tile_rect(col, row, scale);
        let page = theme.page;
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(page.r, page.g, page.b, page.a));
        for (list, r) in self.lists.iter().zip(&self.rects) {
            if !r.intersects(&area) {
                continue;
            }
            // The page itself, in the tile's pixels.
            if let Some(px) = tiny_skia::Rect::from_ltrb(
                ((r.x - area.x) * scale).max(0.0),
                ((r.y - area.y) * scale).max(0.0),
                ((r.right() - area.x) * scale).min(TILE as f32),
                ((r.bottom() - area.y) * scale).min(TILE as f32),
            ) {
                pixmap.fill_rect(px, &paint, Transform::identity(), None);
            }
            let view = View {
                scale,
                x: area.x - r.x,
                y: area.y - r.y,
                width: TILE,
                height: TILE,
            };
            raster.render_into(&mut pixmap, list, fonts, theme, &view);
        }
        Some(pixmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(n: usize) -> Sheets {
        Sheets::stack((0..n).map(|_| DisplayList::a4()).collect())
    }

    #[test]
    fn pages_stack_with_a_gap() {
        let s = pages(3);
        let (w, h) = s.size();
        assert!((w - 595.28).abs() < 0.01);
        assert!((h - (3.0 * 841.89 + 2.0 * PAGE_GAP)).abs() < 0.01);
        assert_eq!(s.rects[1].y, 841.89 + PAGE_GAP);
    }

    #[test]
    fn visible_tiles_centre_first_and_skip_gaps() {
        let s = pages(2);
        // At 1 pixel per point a tile is 512 points.
        let all = s.tiles_for(Rect::new(0.0, 0.0, 1e6, 1e6), 1.0);
        assert_eq!(all.len(), 2 * 4, "{all:?}");
        let view = Rect::new(0.0, 600.0, 595.0, 400.0);
        let some = s.tiles_for(view, 1.0);
        assert_eq!(some, vec![(0, 1), (1, 1)]);
        assert!(
            s.tiles_for(Rect::new(-500.0, -500.0, 10.0, 10.0), 1.0)
                .is_empty()
        );
        assert!(Sheets::default().tiles_for(view, 1.0).is_empty());
    }

    #[test]
    fn a_tile_draws_page_colour_and_leaves_gaps_clear() {
        let s = pages(2);
        let fonts = FontSet::bundled();
        let mut raster = Raster::new();
        // Tile (0, 1) at 1 px/pt spans y 512..1024: page one ends at 841.89,
        // the gap runs to 859.89, page two follows.
        let px = s
            .draw_tile(&mut raster, &fonts, &Theme::DARK, 0, 1, 1.0)
            .unwrap();
        let at = |x: u32, y: u32| px.pixel(x, y).unwrap();
        let page = Theme::DARK.page;
        assert_eq!((at(10, 10).red(), at(10, 10).alpha()), (page.r, 255));
        assert_eq!(at(10, 850 - 512).alpha(), 0, "gap between pages");
        assert_eq!(at(10, 1000 - 512).alpha(), 255, "page two");
        // Tile (1, 0) spans x 512..1024: the page ends at 595.28.
        let px = s
            .draw_tile(&mut raster, &fonts, &Theme::DARK, 1, 0, 1.0)
            .unwrap();
        assert_eq!(px.pixel(50, 10).unwrap().alpha(), 255, "page");
        assert_eq!(px.pixel(100, 10).unwrap().alpha(), 0, "right of the page");
    }
}
