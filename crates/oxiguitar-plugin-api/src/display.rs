// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The display list as plugins draw with it, version 1: what a tab's
//! canvas ([`crate::tab::Widget::Canvas`]) shows.
//!
//! It mirrors the core's own display list (`oxiguitar-display`), which
//! engraves the score, item for item, with one difference: text crosses as
//! text ([`Item::Text`]), and the core places its glyphs with its own fonts.
//! A plugin never needs the core's fonts, and what it draws looks like the
//! rest of the window.
//!
//! Coordinates are in the canvas's own units, from its top-left corner,
//! with y pointing down; the core scales the canvas to the room it has,
//! keeping its proportions, and reports pointer positions in the same
//! units. Everything drawn stays within reach of the canvas
//! ([`Item::problem`]): further out nothing would show, and renderers lose
//! their precision. Colours are roles ([`Paint`]), which the core resolves with the
//! window's theme, so a canvas follows Dark Interface by itself.
//!
//! Like every message, these types only grow at the end (new variants of
//! [`Item`] after the last one) in later minor versions; the encoding is
//! pinned by a test.

use serde::{Deserialize, Serialize};

/// A point, in canvas units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// From the left edge.
    pub x: f32,
    /// From the top edge, downwards.
    pub y: f32,
}

impl Point {
    /// The point at `x`, `y`.
    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// What colour something is drawn in: a role, resolved by the window's
/// theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Paint {
    /// Lines, shapes and text.
    Ink,
    /// The canvas's background: for knocking shapes out.
    Page,
    /// Highlights: the selection, the playing note.
    Accent,
    /// Secondary marks.
    Muted,
}

/// A font the core draws text in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Font {
    /// The window's text font.
    Sans,
    /// The music font (SMuFL; Bravura): the text is SMuFL code points, and
    /// the size is a staff's height, as in engraving.
    Music,
}

/// Where text sits horizontally against its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Anchor {
    /// The text starts at the point.
    Start,
    /// The text is centred on the point.
    Middle,
    /// The text ends at the point.
    End,
}

/// One drawing command of a path.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PathCmd {
    /// Starts a new subpath.
    MoveTo(Point),
    /// A straight line.
    LineTo(Point),
    /// A cubic Bézier curve through two control points.
    CubicTo(Point, Point, Point),
    /// Closes the subpath.
    Close,
}

/// One thing to draw. Later items cover earlier ones.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    /// A straight line with square-cut ends.
    Line {
        /// Its start, at the middle of its width.
        from: Point,
        /// Its end.
        to: Point,
        /// Its thickness.
        width: f32,
        /// Its colour.
        paint: Paint,
    },
    /// A filled rectangle.
    Rect {
        /// Its top-left corner.
        at: Point,
        /// Its width, never negative.
        width: f32,
        /// Its height, never negative.
        height: f32,
        /// Its colour.
        paint: Paint,
    },
    /// A filled and/or stroked shape.
    Path {
        /// The commands, in order.
        cmds: Vec<PathCmd>,
        /// The fill (non-zero winding), if filled.
        fill: Option<Paint>,
        /// The stroke's width and colour, if stroked.
        stroke: Option<(f32, Paint)>,
    },
    /// One line of text, on a baseline.
    Text {
        /// The text.
        text: String,
        /// The point on its baseline that `anchor` places.
        at: Point,
        /// Its size: the font's em, in canvas units.
        size: f32,
        /// Its font.
        font: Font,
        /// Where it sits against `at`.
        anchor: Anchor,
        /// Its colour.
        paint: Paint,
    },
}

/// The largest canvas side the core draws, in canvas units, and the most
/// items: limits that keep a mistaken plugin from costing the window time.
pub const MAX_SIDE: f32 = 100_000.0;

/// The most items a canvas may hold.
pub const MAX_ITEMS: usize = 100_000;

impl Item {
    /// Why this item can't be drawn on a canvas `width` × `height` units
    /// large, if it can't: a number that isn't finite, a negative size, or
    /// something out of the canvas's reach. Every point must lie within the
    /// canvas extended by its own size on every side (x from −`width` to
    /// 2 × `width`, y from −`height` to 2 × `height`), and no size, line
    /// width or text size may be more than three times the canvas's longer
    /// side.
    pub fn problem(&self, width: f32, height: f32) -> Option<String> {
        let near = |p: &Point| {
            (-width..=2.0 * width).contains(&p.x) && (-height..=2.0 * height).contains(&p.y)
        };
        let reach = 3.0 * width.max(height);
        let size = |v: f32| (0.0..=reach).contains(&v);
        let far = "reaches too far from the canvas, or isn't a number";
        match self {
            Item::Line {
                from, to, width: w, ..
            } => (!near(from) || !near(to) || !size(*w)).then(|| format!("a line {far}")),
            Item::Rect {
                at,
                width: w,
                height: h,
                ..
            } => (!near(at) || !size(*w) || !size(*h)).then(|| format!("a rectangle {far}")),
            Item::Path { cmds, stroke, .. } => {
                let points = cmds.iter().all(|c| match c {
                    PathCmd::MoveTo(p) | PathCmd::LineTo(p) => near(p),
                    PathCmd::CubicTo(a, b, c) => near(a) && near(b) && near(c),
                    PathCmd::Close => true,
                });
                let stroke = stroke.is_none_or(|(w, _)| size(w));
                (!points || !stroke).then(|| format!("a path {far}"))
            }
            Item::Text { at, size: s, .. } => {
                (!near(at) || !size(*s) || *s <= 0.0).then(|| format!("text {far}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode};

    #[test]
    fn items_round_trip_and_their_positions_are_frozen() {
        let items = vec![
            Item::Line {
                from: Point::new(0.0, 1.0),
                to: Point::new(10.0, 1.0),
                width: 0.5,
                paint: Paint::Ink,
            },
            Item::Rect {
                at: Point::new(2.0, 3.0),
                width: 4.0,
                height: 5.0,
                paint: Paint::Accent,
            },
            Item::Path {
                cmds: vec![
                    PathCmd::MoveTo(Point::new(0.0, 0.0)),
                    PathCmd::CubicTo(
                        Point::new(1.0, 1.0),
                        Point::new(2.0, 1.0),
                        Point::new(3.0, 0.0),
                    ),
                    PathCmd::Close,
                ],
                fill: Some(Paint::Muted),
                stroke: Some((1.0, Paint::Page)),
            },
            Item::Text {
                text: "12".into(),
                at: Point::new(5.0, 5.0),
                size: 9.0,
                font: Font::Sans,
                anchor: Anchor::Middle,
                paint: Paint::Ink,
            },
        ];
        let bytes = encode(&items).unwrap();
        assert_eq!(decode::<Vec<Item>>(&bytes).unwrap(), items);
        let first = |i: &Item| encode(i).unwrap()[0];
        assert_eq!(items.iter().map(first).collect::<Vec<_>>(), [0, 1, 2, 3]);
        assert_eq!(encode(&Paint::Muted).unwrap(), [3]);
        assert_eq!(encode(&Font::Music).unwrap(), [1]);
        assert_eq!(encode(&Anchor::End).unwrap(), [2]);
        assert_eq!(encode(&PathCmd::Close).unwrap(), [3]);
        assert!(items.iter().all(|i| i.problem(10.0, 10.0).is_none()));
    }

    #[test]
    fn items_that_cannot_be_drawn_say_why() {
        let bad = Item::Rect {
            at: Point::new(f32::NAN, 0.0),
            width: 1.0,
            height: 1.0,
            paint: Paint::Ink,
        };
        assert!(bad.problem(10.0, 10.0).unwrap().contains("rectangle"));
        let tiny = Item::Text {
            text: "x".into(),
            at: Point::default(),
            size: 0.0,
            font: Font::Sans,
            anchor: Anchor::Start,
            paint: Paint::Ink,
        };
        assert!(tiny.problem(10.0, 10.0).is_some());
        // The line the fuzz target plugin_tab found (session 11): it made
        // the renderer's edge walker fail.
        let far = Item::Line {
            from: Point::new(2.410_846_6e-38, -3.689_349e19),
            to: Point::new(2.275_328e-36, 2.951_479e20),
            width: 4.0,
            paint: Paint::Accent,
        };
        assert!(far.problem(100.0, 50.0).unwrap().contains("too far"));
        // Within reach: the canvas extended by its own size on every side.
        let near = |x, y, w| Item::Line {
            from: Point::new(x, y),
            to: Point::new(50.0, 25.0),
            width: w,
            paint: Paint::Ink,
        };
        assert!(near(-100.0, 100.0, 300.0).problem(100.0, 50.0).is_none());
        assert!(near(-100.1, 0.0, 1.0).problem(100.0, 50.0).is_some());
        assert!(near(0.0, 100.1, 1.0).problem(100.0, 50.0).is_some());
        assert!(near(0.0, 0.0, 300.1).problem(100.0, 50.0).is_some());
    }
}
