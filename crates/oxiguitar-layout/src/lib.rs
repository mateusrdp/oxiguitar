// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Engraving: turns an OxiGuitar [`Score`] into display lists.
//!
//! The engraver plans every bar (where notes sit, which way stems point, how
//! much room each moment needs), breaks the bars into systems, stretches each
//! system to the line width and draws it. Standard notation and tablature of
//! a track share one set of columns, so they line up bar by bar (VIEW-1).
//! Every symbol comes from the SMuFL font, placed by its metadata (VIEW-7).
//!
//! The output is one [`DisplayList`] per page, which `oxiguitar-render` draws
//! to the screen, PNG or PDF, so what is shown is what is exported.
//!
//! Status (M1): the Page layout, one or more tracks, notes, rests, chords,
//! accidentals, beams, flags, dots, ties, tuplets, repeats and alternate
//! endings, sections, tempo, chord names, beat text and the score header.
//! Effects, rhythm on tab, lyrics, grace notes and the other layouts follow.

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet};
use oxiguitar_model::Score;

mod draw;
mod engrave;
mod grid;
mod page;
mod pitch;
mod plan;
mod rhythm;
mod system;

/// How the score is laid out and on what paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Page width in points.
    pub page_width: f32,
    /// Page height in points.
    pub page_height: f32,
    /// Margin on every side, in points.
    pub margin: f32,
    /// Distance between two staff lines, in points. Tablature lines are
    /// one and a half times as far apart.
    pub staff_space: f32,
    /// Tracks to show, by index, in order. Empty shows the first track.
    pub tracks: Vec<usize>,
    /// Draw the score header (title, artist, credits, tuning) on page one.
    pub header: bool,
}

impl Default for Options {
    /// A4 paper with 15 mm margins and a 6.4 mm staff.
    fn default() -> Self {
        Options {
            page_width: 595.28,
            page_height: 841.89,
            margin: 42.5,
            staff_space: 4.5,
            tracks: Vec::new(),
            header: true,
        }
    }
}

/// An engraved score.
#[derive(Clone, Debug, Default)]
pub struct Engraved {
    /// One display list per page.
    pub pages: Vec<DisplayList>,
    /// Number of systems (lines of music).
    pub systems: usize,
}

/// Engraves `score` with the bundled music font's metadata.
///
/// Never panics on a damaged score: bars that cannot be measured get a
/// minimal width, notes outside the instrument are left out.
pub fn engrave(score: &Score, fonts: &FontSet, meta: &Metadata, options: &Options) -> Engraved {
    let views = system::views(score, &options.tracks);
    let room = options.page_height - 2.0 * options.margin - page::FOOTER;
    let mut sp = options.staff_space.clamp(1.0, 50.0);
    // A system taller than the page (many tracks) is engraved again with a
    // smaller staff, so every system fits on a page.
    let mut attempts = 0;
    loop {
        let painter = draw::Painter { fonts, meta, sp };
        let width = (options.page_width - 2.0 * options.margin).max(10.0 * sp);
        let plans = system::plan_all(score, &views, &painter);
        let systems = system::break_systems(score, &views, &plans, &painter, width);
        let drawn: Vec<engrave::DrawnSystem> = systems
            .iter()
            .map(|s| engrave::system(score, &views, &plans, s, &painter))
            .collect();
        let tallest = drawn.iter().map(|d| d.height).fold(0.0, f32::max);
        if tallest > room && room > 0.0 && sp > MIN_STAFF_SPACE && attempts < 3 {
            sp = (sp * room / tallest * 0.98).max(MIN_STAFF_SPACE);
            attempts += 1;
            continue;
        }
        let header = if options.header {
            page::header(score, &views, &painter, options.page_width, options.margin)
        } else {
            page::Header::default()
        };
        return Engraved {
            systems: drawn.len(),
            pages: page::assemble(score, header, drawn, &painter, options),
        };
    }
}

/// The smallest staff space engraving shrinks to, in points (a 4 mm staff).
const MIN_STAFF_SPACE: f32 = 2.8;
