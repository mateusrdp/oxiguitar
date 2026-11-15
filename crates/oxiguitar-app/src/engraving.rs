// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engraved score as plugins read it (plugin interface 1.8): the
//! window's own pages and its map of where everything is drawn, in the
//! interface's types. A page's drawing becomes a canvas's items: text runs
//! in the text font or the music font, lines, rectangles and paths, colours
//! as the four roles a plugin can name (a warning's red and a track's own
//! colour are ink there).

use oxiguitar_display::{DisplayList, FontSet, Item, Paint, PathCmd};
use oxiguitar_host::api::display as wire;
use oxiguitar_host::api::message::{EngravedBeat, EngravedPage, Engraving};
use oxiguitar_layout::Hits;

fn point(p: oxiguitar_display::Point) -> wire::Point {
    wire::Point::new(p.x, p.y)
}

fn paint(p: Paint) -> wire::Paint {
    match p {
        Paint::Page => wire::Paint::Page,
        Paint::Accent => wire::Paint::Accent,
        Paint::Muted => wire::Paint::Muted,
        Paint::Ink | Paint::Warning | Paint::Custom(_) => wire::Paint::Ink,
    }
}

fn item(item: &Item) -> wire::Item {
    match item {
        Item::Glyphs(run) => wire::Item::Text {
            text: run.text.clone(),
            at: point(run.origin),
            size: run.size,
            font: if run.font == FontSet::MUSIC {
                wire::Font::Music
            } else {
                wire::Font::Sans
            },
            anchor: wire::Anchor::Start,
            paint: paint(run.paint),
        },
        Item::Line {
            from,
            to,
            width,
            paint: p,
        } => wire::Item::Line {
            from: point(*from),
            to: point(*to),
            width: *width,
            paint: paint(*p),
        },
        Item::Rect { rect, paint: p } => wire::Item::Rect {
            at: wire::Point::new(rect.x, rect.y),
            width: rect.width,
            height: rect.height,
            paint: paint(*p),
        },
        Item::Path { path, fill, stroke } => wire::Item::Path {
            cmds: path
                .cmds
                .iter()
                .map(|c| match *c {
                    PathCmd::MoveTo(p) => wire::PathCmd::MoveTo(point(p)),
                    PathCmd::LineTo(p) => wire::PathCmd::LineTo(point(p)),
                    PathCmd::CubicTo(a, b, c) => {
                        wire::PathCmd::CubicTo(point(a), point(b), point(c))
                    }
                    PathCmd::Close => wire::PathCmd::Close,
                })
                .collect(),
            fill: fill.map(paint),
            stroke: stroke.map(|(w, p)| (w, paint(p))),
        },
    }
}

/// The engraving of `pages` and `hits`, made of the score at `revision`.
pub(crate) fn of(revision: u64, pages: &[DisplayList], hits: &Hits) -> Engraving {
    let index = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    let mut beats = Vec::new();
    for system in &hits.systems {
        for staff in &system.staves {
            for bar in &staff.bars {
                for beat in &bar.beats {
                    beats.push(EngravedBeat {
                        track: index(staff.track),
                        staff: index(staff.staff),
                        bar: index(bar.bar),
                        voice: index(beat.voice),
                        beat: index(beat.beat),
                        page: index(system.sheet),
                        x: beat.x,
                        y: staff.top,
                        width: beat.width,
                        height: staff.bottom - staff.top,
                    });
                }
            }
        }
    }
    Engraving {
        revision,
        pages: pages
            .iter()
            .map(|page| EngravedPage {
                width: page.width,
                height: page.height,
                items: page.items.iter().map(item).collect(),
            })
            .collect(),
        beats,
    }
}
