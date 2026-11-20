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
//!
//! A drawing may also have a [`Run`] (interface 1.13): a line from its box
//! onwards to a point in a later beat or bar, drawn across systems as an
//! ottava's line is. Its caller says where it ends; the engraver only draws.

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
    /// Whose it is: a number the caller chooses, opaque to the engraver,
    /// given back with the box in the hit map ([`crate::hits::DrawnHit`]).
    pub tag: u32,
    /// How wide the box is.
    pub width: f32,
    /// How tall.
    pub height: f32,
    /// What is in it, in drawing order.
    pub shapes: Vec<Shape>,
    /// A line from the box onwards, at its middle height.
    pub run: Option<Run>,
}

/// A line from a drawing's box to a point further on in its staff
/// ([`Drawing::run`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The bar it ends in: the drawing's own or a later one.
    pub to_bar: usize,
    /// Where in that bar, in whole notes from its start.
    pub to: f64,
    /// Its width, in staff spaces.
    pub width: f32,
    /// Its colour.
    pub paint: Paint,
    /// Dashed rather than solid.
    pub dashed: bool,
    /// A short hook down at its end.
    pub hook: bool,
}

/// A run as one staff's systems draw it: where it starts and ends.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunSpan {
    /// The drawing's place, and its rank among that place's drawings.
    pub(crate) at: DrawnAt,
    pub(crate) rank: usize,
    /// Its box's height, in staff spaces: the line is at its middle.
    pub(crate) height: f32,
    /// The run.
    pub(crate) run: Run,
}

/// Every run of the staff `staff` of `track`, by where it starts.
pub(crate) fn runs_of(drawings: &Drawings, track: usize, staff: usize) -> Vec<RunSpan> {
    drawings
        .iter()
        .filter(|(at, _)| at.track == track && at.staff == staff)
        .flat_map(|(at, list)| {
            list.iter().enumerate().filter_map(|(rank, d)| {
                let run = d.run.clone()?;
                (run.to_bar >= at.bar).then_some(RunSpan {
                    at: *at,
                    rank,
                    height: d.height,
                    run,
                })
            })
        })
        .collect()
}

/// Where a drawing with a run was drawn in a system: its box's right edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RunStart {
    /// The drawing's place and rank ([`RunSpan`]).
    pub(crate) at: DrawnAt,
    pub(crate) rank: usize,
    /// Its box's right edge, in points.
    pub(crate) x: f32,
}

/// Draws a run's piece in one system, from `x0` to `x1` at height `y`,
/// with its hook when it `ends` there.
pub(crate) fn draw_run(
    p: &Painter<'_>,
    list: &mut DisplayList,
    run: &Run,
    (x0, x1, y): (f32, f32, f32),
    ends: bool,
) {
    let sp = p.sp;
    if x1 <= x0 {
        return;
    }
    let w = run.width * sp;
    if run.dashed {
        let (dash, gap) = (0.6 * sp, 0.4 * sp);
        let mut x = x0;
        while x < x1 {
            list.line(
                Point::new(x, y),
                Point::new((x + dash).min(x1), y),
                w,
                run.paint,
            );
            x += dash + gap;
        }
    } else {
        list.line(Point::new(x0, y), Point::new(x1, y), w, run.paint);
    }
    if ends && run.hook {
        list.line(
            Point::new(x1, y - 0.5 * w),
            Point::new(x1, y + 0.8 * sp),
            w,
            run.paint,
        );
    }
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
pub(crate) fn width(drawings: &[(DrawnAt, Drawing)], sp: f32) -> f32 {
    drawings.iter().map(|(_, d)| (d.width + GAP) * sp).sum()
}

/// Staff spaces between two drawings at one beat.
const GAP: f32 = 0.3;

/// Draws `drawings` side by side from `x`, standing on `base`, and gives
/// each one's box. Their runs are drawn later, once the system's bars are
/// all placed.
pub(crate) fn draw(
    p: &Painter<'_>,
    list: &mut DisplayList,
    drawings: &[(DrawnAt, Drawing)],
    x: f32,
    base: f32,
) -> Vec<Rect> {
    let sp = p.sp;
    let mut left = x;
    let mut boxes = Vec::with_capacity(drawings.len());
    for (_, d) in drawings {
        let top = base - d.height * sp;
        boxes.push(Rect::new(left, top, d.width * sp, d.height * sp));
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
    boxes
}

/// How far left of its point a text of width `w` starts.
fn shift(anchor: Anchor, w: f32) -> f32 {
    match anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => w / 2.0,
        Anchor::End => w,
    }
}
