// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What plugins draw on the page (plugin interface 1.8): before each
//! engraving, every plugin that draws is asked about the places that carry
//! its data or marks, and only those; the host reuses its last answer when
//! they are the same. Its drawings become the engraver's
//! [`Drawings`], which knows nothing of plugins.

use oxiguitar_display::{Paint, PathCmd, Point};
use oxiguitar_host::Plugins;
use oxiguitar_host::api::display as wire;
use oxiguitar_host::api::message::DrawPlace;
use oxiguitar_layout::{Anchor, Drawing, Drawings, DrawnAt, Shape};
use oxiguitar_model::Score;

/// Every plugin's drawings for `score`.
pub(crate) fn of(plugins: &Plugins, score: &Score) -> Drawings {
    let mut out = Drawings::new();
    for plugin in plugins.drawers() {
        let (places, at) = places(score, &plugin);
        for drawn in plugins.draw(&plugin, places) {
            if let Some(&at) = at.get(drawn.place as usize) {
                out.entry(at).or_default().push(Drawing {
                    width: drawn.width,
                    height: drawn.height,
                    shapes: drawn.items.iter().map(shape).collect(),
                });
            }
        }
    }
    out
}

/// The places of `score` that carry `plugin`'s data or marks, and where
/// each is.
fn places(score: &Score, plugin: &str) -> (Vec<DrawPlace>, Vec<DrawnAt>) {
    let n = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    let mut places = Vec::new();
    let mut at = Vec::new();
    for (ti, track) in score.tracks.iter().enumerate() {
        for (si, staff) in track.staves.iter().enumerate() {
            for (bi, bar) in staff.bars.iter().enumerate() {
                if let Some(data) = bar.plugins.get(plugin) {
                    places.push(DrawPlace::Bar {
                        at: [n(ti), n(si), n(bi)],
                        data: data.clone(),
                    });
                    at.push(DrawnAt {
                        track: ti,
                        staff: si,
                        bar: bi,
                        beat: None,
                    });
                }
                for (vi, voice) in bar.voices.iter().enumerate() {
                    for (ei, beat) in voice.beats.iter().enumerate() {
                        let ours = beat.plugins.contains_key(plugin)
                            || beat.effects.marks.contains_key(plugin)
                            || beat
                                .notes
                                .iter()
                                .any(|note| note.plugins.contains_key(plugin));
                        if ours {
                            places.push(DrawPlace::Beat {
                                at: [n(ti), n(si), n(bi), n(vi), n(ei)],
                                beat: Box::new(oxiguitar_model::wire::beat_to_wire2(beat)),
                            });
                            at.push(DrawnAt {
                                track: ti,
                                staff: si,
                                bar: bi,
                                beat: Some((vi, ei)),
                            });
                        }
                    }
                }
            }
        }
    }
    (places, at)
}

fn point(p: wire::Point) -> Point {
    Point::new(p.x, p.y)
}

fn paint(p: wire::Paint) -> Paint {
    match p {
        wire::Paint::Ink => Paint::Ink,
        wire::Paint::Page => Paint::Page,
        wire::Paint::Accent => Paint::Accent,
        wire::Paint::Muted => Paint::Muted,
    }
}

/// An item of the interface's canvases as the engraver's shape.
fn shape(item: &wire::Item) -> Shape {
    match item {
        wire::Item::Line {
            from,
            to,
            width,
            paint: p,
        } => Shape::Line {
            from: point(*from),
            to: point(*to),
            width: *width,
            paint: paint(*p),
        },
        wire::Item::Rect {
            at,
            width,
            height,
            paint: p,
        } => Shape::Rect {
            at: point(*at),
            width: *width,
            height: *height,
            paint: paint(*p),
        },
        wire::Item::Path { cmds, fill, stroke } => Shape::Path {
            cmds: cmds
                .iter()
                .map(|c| match *c {
                    wire::PathCmd::MoveTo(p) => PathCmd::MoveTo(point(p)),
                    wire::PathCmd::LineTo(p) => PathCmd::LineTo(point(p)),
                    wire::PathCmd::CubicTo(a, b, c) => {
                        PathCmd::CubicTo(point(a), point(b), point(c))
                    }
                    wire::PathCmd::Close => PathCmd::Close,
                })
                .collect(),
            fill: fill.map(paint),
            stroke: stroke.map(|(w, p)| (w, paint(p))),
        },
        wire::Item::Text {
            text,
            at,
            size,
            font,
            anchor,
            paint: p,
        } => Shape::Text {
            text: text.clone(),
            at: point(*at),
            size: *size,
            music: *font == wire::Font::Music,
            anchor: match anchor {
                wire::Anchor::Start => Anchor::Start,
                wire::Anchor::Middle => Anchor::Middle,
                wire::Anchor::End => Anchor::End,
            },
            paint: paint(*p),
        },
    }
}
