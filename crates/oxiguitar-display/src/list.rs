// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The display list and its geometry.
//!
//! All coordinates are in points (1/72 inch) from the top-left corner of the
//! page, with y pointing down. Colours are roles ([`Paint`]), not values, so the
//! same list draws on a light page, a dark page or an inverted page (VIEW-8);
//! a [`Theme`] turns roles into colours when a backend draws.

use crate::fonts::FontId;

/// A point on the page, in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Distance from the left edge.
    pub x: f32,
    /// Distance from the top edge.
    pub y: f32,
}

impl Point {
    /// A point at `x`, `y`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle, in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width, never negative.
    pub width: f32,
    /// Height, never negative.
    pub height: f32,
}

impl Rect {
    /// A rectangle from its top-left corner and size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether the two rectangles overlap or touch.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.bottom()
            && other.y <= self.bottom()
    }

    /// The rectangle grown by `by` on every side.
    pub fn inflate(&self, by: f32) -> Rect {
        Rect::new(
            self.x - by,
            self.y - by,
            self.width + 2.0 * by,
            self.height + 2.0 * by,
        )
    }
}

/// What colour something is drawn in, as a role a [`Theme`] resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Paint {
    /// Notation: staff lines, notes, text.
    Ink,
    /// The page itself. Used to knock out staff lines behind tab numbers.
    Page,
    /// Selection, playback cursor and other highlights.
    Accent,
    /// Secondary marks such as bar numbers and ghosted elements.
    Muted,
}

/// An 8-bit sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Opacity; 255 is opaque.
    pub a: u8,
}

impl Rgba {
    /// An opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// The colours a backend uses for each [`Paint`] role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Theme {
    /// Colour of [`Paint::Ink`].
    pub ink: Rgba,
    /// Colour of [`Paint::Page`], and of the background.
    pub page: Rgba,
    /// Colour of [`Paint::Accent`].
    pub accent: Rgba,
    /// Colour of [`Paint::Muted`].
    pub muted: Rgba,
}

impl Theme {
    /// Black on white, as printed.
    pub const LIGHT: Theme = Theme {
        ink: Rgba::rgb(0x1a, 0x1a, 0x1a),
        page: Rgba::rgb(0xff, 0xff, 0xff),
        accent: Rgba::rgb(0x2e, 0x9e, 0x4f),
        muted: Rgba::rgb(0x80, 0x80, 0x80),
    };

    /// Light notation on a dark page (VIEW-8).
    pub const DARK: Theme = Theme {
        ink: Rgba::rgb(0xe6, 0xe6, 0xe6),
        page: Rgba::rgb(0x1e, 0x1f, 0x22),
        accent: Rgba::rgb(0x5c, 0xd6, 0x7f),
        muted: Rgba::rgb(0x8c, 0x8c, 0x8c),
    };

    /// The colour for a role.
    pub fn resolve(&self, paint: Paint) -> Rgba {
        match paint {
            Paint::Ink => self.ink,
            Paint::Page => self.page,
            Paint::Accent => self.accent,
            Paint::Muted => self.muted,
        }
    }
}

/// One glyph of a [`GlyphRun`], already positioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    /// Glyph index in the run's font (not a character).
    pub id: u16,
    /// Horizontal offset from the run's origin, in points.
    pub x: f32,
    /// The glyph's advance width, in points.
    pub advance: f32,
    /// Byte offset in [`GlyphRun::text`] of the character this glyph shows.
    pub cluster: u32,
}

/// Glyphs from one font at one size on one baseline.
///
/// Layout positions every glyph, so backends never shape text themselves and
/// cannot disagree on where a glyph goes.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    /// Which font of the [`FontSet`](crate::FontSet).
    pub font: FontId,
    /// Font size in points: the em square's height. For a SMuFL font this is
    /// the staff height, four staff spaces.
    pub size: f32,
    /// Start of the baseline.
    pub origin: Point,
    /// The glyphs, left to right.
    pub glyphs: Vec<PlacedGlyph>,
    /// The text the glyphs show, so PDF readers can search and copy it.
    pub text: String,
    /// Colour role.
    pub paint: Paint,
}

/// One drawing command of a [`Path`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCmd {
    /// Start a new subpath.
    MoveTo(Point),
    /// Straight line.
    LineTo(Point),
    /// Cubic Bézier curve through two control points.
    CubicTo(Point, Point, Point),
    /// Close the current subpath.
    Close,
}

/// A vector shape: beams, slurs, ties, bend arrows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    /// The commands, in order.
    pub cmds: Vec<PathCmd>,
}

impl Path {
    /// An empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a subpath.
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.cmds.push(PathCmd::MoveTo(Point::new(x, y)));
        self
    }

    /// Add a straight line.
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.cmds.push(PathCmd::LineTo(Point::new(x, y)));
        self
    }

    /// Add a cubic curve.
    pub fn cubic_to(mut self, c1: Point, c2: Point, to: Point) -> Self {
        self.cmds.push(PathCmd::CubicTo(c1, c2, to));
        self
    }

    /// Close the subpath.
    pub fn close(mut self) -> Self {
        self.cmds.push(PathCmd::Close);
        self
    }

    /// The smallest rectangle holding every point, control points included.
    pub fn bounds(&self) -> Rect {
        let mut pts = self.cmds.iter().flat_map(|c| match *c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => vec![p],
            PathCmd::CubicTo(a, b, c) => vec![a, b, c],
            PathCmd::Close => vec![],
        });
        let Some(first) = pts.next() else {
            return Rect::default();
        };
        let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
        for p in pts {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// One thing to draw.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Text or music symbols.
    Glyphs(GlyphRun),
    /// A straight line with square-cut ends: staff lines, stems, barlines.
    Line {
        /// Start point, at the centre of the line's width.
        from: Point,
        /// End point.
        to: Point,
        /// Thickness in points.
        width: f32,
        /// Colour role.
        paint: Paint,
    },
    /// A filled rectangle.
    Rect {
        /// Where.
        rect: Rect,
        /// Colour role.
        paint: Paint,
    },
    /// A filled and/or stroked shape.
    Path {
        /// The shape.
        path: Path,
        /// Fill colour, if filled (non-zero winding).
        fill: Option<Paint>,
        /// Stroke width and colour, if stroked.
        stroke: Option<(f32, Paint)>,
    },
}

impl Item {
    /// A copy moved by `dx`, `dy` points, used to place a laid-out system
    /// on a page.
    pub fn offset(&self, dx: f32, dy: f32) -> Item {
        let mv = |p: Point| Point::new(p.x + dx, p.y + dy);
        match self {
            Item::Glyphs(run) => Item::Glyphs(GlyphRun {
                origin: mv(run.origin),
                ..run.clone()
            }),
            Item::Line {
                from,
                to,
                width,
                paint,
            } => Item::Line {
                from: mv(*from),
                to: mv(*to),
                width: *width,
                paint: *paint,
            },
            Item::Rect { rect, paint } => Item::Rect {
                rect: Rect::new(rect.x + dx, rect.y + dy, rect.width, rect.height),
                paint: *paint,
            },
            Item::Path { path, fill, stroke } => Item::Path {
                path: Path {
                    cmds: path
                        .cmds
                        .iter()
                        .map(|c| match *c {
                            PathCmd::MoveTo(p) => PathCmd::MoveTo(mv(p)),
                            PathCmd::LineTo(p) => PathCmd::LineTo(mv(p)),
                            PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(mv(a), mv(b), mv(c)),
                            PathCmd::Close => PathCmd::Close,
                        })
                        .collect(),
                },
                fill: *fill,
                stroke: *stroke,
            },
        }
    }

    /// A conservative bounding box, used to skip items outside the visible
    /// part of the page. Glyph boxes assume no glyph reaches further than one
    /// em from its origin, which holds for music and text fonts in practice.
    pub fn bounds(&self) -> Rect {
        match self {
            Item::Glyphs(run) => {
                let width = run.glyphs.last().map_or(0.0, |g| g.x + g.advance);
                Rect::new(run.origin.x, run.origin.y - run.size, width, 2.0 * run.size)
                    .inflate(run.size)
            }
            Item::Line {
                from, to, width, ..
            } => {
                let x = from.x.min(to.x);
                let y = from.y.min(to.y);
                Rect::new(x, y, (from.x - to.x).abs(), (from.y - to.y).abs()).inflate(*width)
            }
            Item::Rect { rect, .. } => *rect,
            Item::Path { path, stroke, .. } => path.bounds().inflate(stroke.map_or(0.0, |s| s.0)),
        }
    }
}

/// One page, or one horizontal strip, of positioned drawing items.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayList {
    /// Page width in points.
    pub width: f32,
    /// Page height in points.
    pub height: f32,
    /// Items in painting order: later items cover earlier ones.
    pub items: Vec<Item>,
}

impl DisplayList {
    /// An empty page of the given size in points.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            items: Vec::new(),
        }
    }

    /// An empty A4 page (210 × 297 mm).
    pub fn a4() -> Self {
        Self::new(595.28, 841.89)
    }

    /// Append an item.
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Append a line.
    pub fn line(&mut self, from: Point, to: Point, width: f32, paint: Paint) {
        self.push(Item::Line {
            from,
            to,
            width,
            paint,
        });
    }

    /// Append a filled rectangle.
    pub fn rect(&mut self, rect: Rect, paint: Paint) {
        self.push(Item::Rect { rect, paint });
    }

    /// Append a filled path.
    pub fn fill(&mut self, path: Path, paint: Paint) {
        self.push(Item::Path {
            path,
            fill: Some(paint),
            stroke: None,
        });
    }

    /// Append a stroked path.
    pub fn stroke(&mut self, path: Path, width: f32, paint: Paint) {
        self.push(Item::Path {
            path,
            fill: None,
            stroke: Some((width, paint)),
        });
    }

    /// Append a glyph run.
    pub fn glyphs(&mut self, run: GlyphRun) {
        self.push(Item::Glyphs(run));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_bounds_include_control_points() {
        let p = Path::new().move_to(0.0, 0.0).cubic_to(
            Point::new(5.0, -10.0),
            Point::new(15.0, -10.0),
            Point::new(20.0, 0.0),
        );
        assert_eq!(p.bounds(), Rect::new(0.0, -10.0, 20.0, 10.0));
    }

    #[test]
    fn offset_moves_every_point() {
        let item = Item::Path {
            path: Path::new().move_to(1.0, 2.0).line_to(3.0, 4.0),
            fill: None,
            stroke: Some((1.0, Paint::Ink)),
        };
        assert_eq!(
            item.offset(10.0, 20.0).bounds(),
            item.bounds().inflate(0.0).offset_for_test(10.0, 20.0)
        );
    }

    impl Rect {
        fn offset_for_test(self, dx: f32, dy: f32) -> Rect {
            Rect::new(self.x + dx, self.y + dy, self.width, self.height)
        }
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(&Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(!a.intersects(&Rect::new(11.0, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn themes_resolve_every_role() {
        for t in [Theme::LIGHT, Theme::DARK] {
            assert_ne!(t.resolve(Paint::Ink), t.resolve(Paint::Page));
        }
    }
}
