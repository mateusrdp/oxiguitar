// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What others draw over the music: plugins' drawings at the bars and
//! beats that carry their data (plugin interface 1.8, Mateus, 16 Nov 2026).
//!
//! The engraver knows nothing of plugins. It is given [`Drawings`]: boxes
//! of shapes, measured in staff spaces, each at a staff's bar or at a beat.
//! A system with any gets one more lane above the staff, the outermost, as
//! tall as its tallest drawing; each drawing stands on the lane's
//! baseline, from its beat's column rightwards (a bar's at the bar's first
//! beat, before the beat's own), and the column is made wide enough for
//! them. So a drawing is printed, exported and scrolled as the rest of the
//! page is, and clicking it finds its beat.

use std::collections::BTreeMap;

use oxiguitar_display::{DisplayList, FontSet, Paint, Path, PathCmd, Point, Rect};

use crate::draw::Painter;

/// Where a drawing is: a staff's bar, or one of its beats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrawnAt {
    /// The track.
    pub track: usize,
    /// The staff in the track.
    pub staff: usize,
    /// The bar.
    pub bar: usize,
    /// The voice and the beat in it; `None` for the bar itself.
    pub beat: Option<(usize, usize)>,
}

/// Every drawing, by place, in the order each place's are drawn.
pub type Drawings = BTreeMap<DrawnAt, Vec<Drawing>>;

/// A box of shapes. Everything is in staff spaces, from the box's top left
/// corner, y downwards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    /// How wide the box is.
    pub width: f32,
    /// How tall.
    pub height: f32,
    /// What is in it, in drawing order.
    pub shapes: Vec<Shape>,
}

/// Where a text stands on its point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// Its start is at the point.
    #[default]
    Start,
    /// Its middle.
    Middle,
    /// Its end.
    End,
}

/// One shape of a drawing (staff spaces, from the box's top left corner).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A straight line.
    Line {
        /// Its start.
        from: Point,
        /// Its end.
        to: Point,
        /// Its width.
        width: f32,
        /// Its colour.
        paint: Paint,
    },
    /// A filled rectangle.
    Rect {
        /// Its top left corner.
        at: Point,
        /// Its width.
        width: f32,
        /// Its height.
        height: f32,
        /// Its colour.
        paint: Paint,
    },
    /// A path, filled, stroked or both.
    Path {
        /// Its commands.
        cmds: Vec<PathCmd>,
        /// Its fill.
        fill: Option<Paint>,
        /// Its outline's width and colour.
        stroke: Option<(f32, Paint)>,
    },
    /// Text in the text font, or in the music font (SMuFL code points).
    Text {
        /// The text.
        text: String,
        /// Where its baseline meets its anchor.
        at: Point,
        /// Its size: the font's em.
        size: f32,
        /// In the music font.
        music: bool,
        /// Which part of it stands on `at`.
        anchor: Anchor,
        /// Its colour.
        paint: Paint,
    },
}

/// The room a drawing lane needs, in tenths of a staff space, for drawings
/// as tall as `height` staff spaces: their height and a little air.
pub(crate) fn lane_tenths(height: f32) -> u16 {
    if height > 0.0 {
        ((height + 0.6) * 10.0).ceil().min(f32::from(u16::MAX)) as u16
    } else {
        0
    }
}

/// How wide a beat's drawings are laid side by side, in points.
pub(crate) fn width(drawings: &[Drawing], sp: f32) -> f32 {
    drawings.iter().map(|d| (d.width + GAP) * sp).sum()
}

/// Staff spaces between two drawings at one beat.
const GAP: f32 = 0.3;

/// Draws `drawings` side by side from `x`, standing on `base`.
pub(crate) fn draw(
    p: &Painter<'_>,
    list: &mut DisplayList,
    drawings: &[Drawing],
    x: f32,
    base: f32,
) {
    let sp = p.sp;
    let mut left = x;
    for d in drawings {
        let top = base - d.height * sp;
        let at = |q: Point| Point::new(left + q.x * sp, top + q.y * sp);
        for shape in &d.shapes {
            match shape {
                Shape::Line {
                    from,
                    to,
                    width,
                    paint,
                } => list.line(at(*from), at(*to), width * sp, *paint),
                Shape::Rect {
                    at: corner,
                    width,
                    height,
                    paint,
                } => {
                    let c = at(*corner);
                    list.rect(Rect::new(c.x, c.y, width * sp, height * sp), *paint);
                }
                Shape::Path { cmds, fill, stroke } => {
                    let path = Path {
                        cmds: cmds
                            .iter()
                            .map(|c| match *c {
                                PathCmd::MoveTo(q) => PathCmd::MoveTo(at(q)),
                                PathCmd::LineTo(q) => PathCmd::LineTo(at(q)),
                                PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(at(a), at(b), at(c)),
                                PathCmd::Close => PathCmd::Close,
                            })
                            .collect(),
                    };
                    if let Some(paint) = fill {
                        list.fill(path.clone(), *paint);
                    }
                    if let Some((w, paint)) = stroke {
                        list.stroke(path, w * sp, *paint);
                    }
                }
                Shape::Text {
                    text,
                    at: point,
                    size,
                    music,
                    anchor,
                    paint,
                } => {
                    let size = size * sp;
                    let q = at(*point);
                    if *music {
                        let w = p.fonts.text_width(FontSet::MUSIC, size, text);
                        let x = q.x - shift(*anchor, w);
                        list.glyphs(p.fonts.text_run(
                            FontSet::MUSIC,
                            size,
                            Point::new(x, q.y),
                            text,
                            *paint,
                        ));
                    } else {
                        let w = p.text_width(text, size);
                        p.text(list, text, size, q.x - shift(*anchor, w), q.y, *paint);
                    }
                }
            }
        }
        left += (d.width + GAP) * sp;
    }
}

/// How far left of its point a text of width `w` starts.
fn shift(anchor: Anchor, w: f32) -> f32 {
    match anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => w / 2.0,
        Anchor::End => w,
    }
}
