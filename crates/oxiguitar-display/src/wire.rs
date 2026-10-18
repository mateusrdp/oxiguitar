// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A plugin's canvas (`oxiguitar_plugin_api::display`, interface 1.1) as a
//! display list, which the core's renderers draw like an engraved page.
//!
//! The two lists match item for item, except text: a plugin sends text, and
//! the core places its glyphs with its own fonts here, keeping only the
//! glyphs within the canvas's reach. Items that can't be drawn (a number
//! that isn't finite, a negative size, a point far outside the canvas:
//! `Item::problem`) are left out; the host refuses whole contents holding
//! them before they get this far.

use oxiguitar_plugin_api::display as w;

use crate::{DisplayList, FontId, FontSet, Item, Paint, Path, PathCmd, Point, Rect};

/// The canvas `width` × `height` (in its own units, used as points) holding
/// `items`, in the core's display list.
pub fn canvas(width: f32, height: f32, items: &[w::Item], fonts: &FontSet) -> DisplayList {
    let mut list = DisplayList::new(width, height);
    list.items = items
        .iter()
        .filter(|i| i.problem(width, height).is_none())
        .map(|i| item(i, width, fonts))
        .collect();
    list
}

fn point(p: w::Point) -> Point {
    Point::new(p.x, p.y)
}

fn paint(p: w::Paint) -> Paint {
    match p {
        w::Paint::Ink => Paint::Ink,
        w::Paint::Page => Paint::Page,
        w::Paint::Accent => Paint::Accent,
        w::Paint::Muted => Paint::Muted,
    }
}

fn item(i: &w::Item, canvas_width: f32, fonts: &FontSet) -> Item {
    match i {
        w::Item::Line {
            from,
            to,
            width,
            paint: p,
        } => Item::Line {
            from: point(*from),
            to: point(*to),
            width: *width,
            paint: paint(*p),
        },
        w::Item::Rect {
            at,
            width,
            height,
            paint: p,
        } => Item::Rect {
            rect: Rect::new(at.x, at.y, *width, *height),
            paint: paint(*p),
        },
        w::Item::Path { cmds, fill, stroke } => Item::Path {
            path: Path {
                cmds: cmds
                    .iter()
                    .map(|c| match *c {
                        w::PathCmd::MoveTo(p) => PathCmd::MoveTo(point(p)),
                        w::PathCmd::LineTo(p) => PathCmd::LineTo(point(p)),
                        w::PathCmd::CubicTo(a, b, c) => {
                            PathCmd::CubicTo(point(a), point(b), point(c))
                        }
                        w::PathCmd::Close => PathCmd::Close,
                    })
                    .collect(),
            },
            fill: fill.map(paint),
            stroke: stroke.map(|(width, p)| (width, paint(p))),
        },
        w::Item::Text {
            text,
            at,
            size,
            font,
            anchor,
            paint: p,
        } => {
            let font: FontId = match font {
                w::Font::Sans => FontSet::TEXT,
                w::Font::Music => FontSet::MUSIC,
            };
            let width = match anchor {
                w::Anchor::Start => 0.0,
                w::Anchor::Middle => fonts.text_width(font, *size, text) / 2.0,
                w::Anchor::End => fonts.text_width(font, *size, text),
            };
            let mut run =
                fonts.text_run(font, *size, Point::new(at.x - width, at.y), text, paint(*p));
            // A long text runs on past the canvas: only glyphs within its
            // reach (as for every point) are drawn.
            let reach = -canvas_width..=2.0 * canvas_width;
            run.glyphs.retain(|g| {
                reach.contains(&(run.origin.x + g.x))
                    && reach.contains(&(run.origin.x + g.x + g.advance))
            });
            Item::Glyphs(run)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_carries_over_and_text_is_placed_by_its_anchor() {
        let fonts = FontSet::bundled();
        let text = |anchor| w::Item::Text {
            text: "12".into(),
            at: w::Point::new(50.0, 20.0),
            size: 10.0,
            font: w::Font::Sans,
            anchor,
            paint: w::Paint::Accent,
        };
        let items = vec![
            w::Item::Line {
                from: w::Point::new(0.0, 1.0),
                to: w::Point::new(10.0, 1.0),
                width: 0.5,
                paint: w::Paint::Ink,
            },
            w::Item::Rect {
                at: w::Point::new(f32::NAN, 0.0),
                width: 1.0,
                height: 1.0,
                paint: w::Paint::Ink,
            },
            w::Item::Path {
                cmds: vec![
                    w::PathCmd::MoveTo(w::Point::new(0.0, 0.0)),
                    w::PathCmd::LineTo(w::Point::new(5.0, 5.0)),
                    w::PathCmd::Close,
                ],
                fill: Some(w::Paint::Muted),
                stroke: None,
            },
            text(w::Anchor::Start),
            text(w::Anchor::Middle),
            text(w::Anchor::End),
        ];
        let list = canvas(100.0, 40.0, &items, &fonts);
        assert_eq!((list.width, list.height), (100.0, 40.0));
        // The rectangle at NaN is left out.
        assert_eq!(list.items.len(), 5);
        assert_eq!(
            list.items[0],
            Item::Line {
                from: Point::new(0.0, 1.0),
                to: Point::new(10.0, 1.0),
                width: 0.5,
                paint: Paint::Ink,
            }
        );
        assert!(matches!(
            &list.items[1],
            Item::Path {
                fill: Some(Paint::Muted),
                ..
            }
        ));
        let x = |i: usize| match &list.items[i] {
            Item::Glyphs(run) => {
                assert_eq!((run.text.as_str(), run.paint), ("12", Paint::Accent));
                run.origin.x
            }
            other => panic!("{other:?}"),
        };
        let width = fonts.text_width(FontSet::TEXT, 10.0, "12");
        assert!(width > 0.0);
        assert_eq!(x(2), 50.0);
        assert!((x(3) - (50.0 - width / 2.0)).abs() < 1e-4);
        assert!((x(4) - (50.0 - width)).abs() < 1e-4);
    }

    #[test]
    fn nothing_out_of_reach_is_drawn() {
        let fonts = FontSet::bundled();
        let items = vec![
            // The line the fuzz target plugin_tab found (session 11).
            w::Item::Line {
                from: w::Point::new(2.410_846_6e-38, -3.689_349e19),
                to: w::Point::new(2.275_328e-36, 2.951_479e20),
                width: 4.0,
                paint: w::Paint::Accent,
            },
            w::Item::Text {
                text: "8".repeat(1000),
                at: w::Point::new(0.0, 10.0),
                size: 10.0,
                font: w::Font::Sans,
                anchor: w::Anchor::Start,
                paint: w::Paint::Ink,
            },
        ];
        let list = canvas(100.0, 40.0, &items, &fonts);
        assert_eq!(list.items.len(), 1, "the line is left out");
        let Item::Glyphs(run) = &list.items[0] else {
            panic!("{:?}", list.items[0]);
        };
        let digit = fonts.text_width(FontSet::TEXT, 10.0, "8");
        assert_eq!(run.glyphs.len(), (200.0 / digit) as usize);
        assert!(run.glyphs.iter().all(|g| g.x + g.advance <= 200.0));
    }
}
