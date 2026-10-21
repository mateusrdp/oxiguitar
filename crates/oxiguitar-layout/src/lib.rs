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
//! The output is a list of sheets ([`DisplayList`]s): one per page in the
//! Page layout, one long sheet in the others (see [`Layout`]).
//! `oxiguitar-render` draws them to the screen, PNG or PDF, so what is shown
//! is what is exported.
//!
//! Status (M1): the four layouts, one or more tracks, notes, rests, chords,
//! accidentals, beams, flags, dots, ties, tuplets, grace notes, repeats and
//! alternate endings, sections, tempo, chord names and diagrams, beat text,
//! the TAB-1 effects, articulations, dynamics, lyrics and the score header.

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet};
use std::collections::{BTreeMap, BTreeSet};

use oxiguitar_model::{Score, Track};

pub use hits::{BarHits, BeatHit, Found, Hits, Spot, StaffHits, SystemHits};
pub use pitch::pitch_at_step;

mod draw;
mod effects;
mod engrave;
mod grid;
mod hands;
mod hits;
mod page;
mod pitch;
mod plan;
mod rhythm;
mod system;

/// How staves are laid out (VIEW-3), as in Guitar Pro. The names follow
/// Guitar Pro 6 and 7; Guitar Pro 8 calls them Page - Vertical, Screen -
/// Vertical, Screen - Horizontal and Page - Scroll (see
/// `docs/design/engraving.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layout {
    /// Systems fill the page width, stack down the page and break into
    /// pages, with page numbers. What PDF export always uses.
    #[default]
    Page,
    /// Systems fill the window width ([`Options::screen_width`]) and stack
    /// down one long sheet, with no pages.
    Vertical,
    /// Every bar on one long line, at natural spacing, on one sheet as wide
    /// as the music. No header: the line starts with the music.
    Horizontal,
    /// Systems fill the page width and stack down one long sheet, with no
    /// page breaks: the Page layout as one scroll.
    Parchment,
}

impl Layout {
    /// All four, in menu order.
    pub const ALL: [Layout; 4] = [
        Layout::Page,
        Layout::Vertical,
        Layout::Horizontal,
        Layout::Parchment,
    ];

    /// The name shown in menus and taken by `--layout`, such as "Page".
    pub fn name(self) -> &'static str {
        match self {
            Layout::Page => "Page",
            Layout::Vertical => "Vertical",
            Layout::Horizontal => "Horizontal",
            Layout::Parchment => "Parchment",
        }
    }

    /// The layout with this name, ignoring case.
    pub fn from_name(name: &str) -> Option<Layout> {
        Layout::ALL
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(name.trim()))
    }

    /// Whether the score is cut into pages.
    pub fn has_pages(self) -> bool {
        self == Layout::Page
    }
}

/// Paper sizes for the Page layout and PDF export (EXP-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Paper {
    /// 210 × 297 mm.
    #[default]
    A4,
    /// US Letter, 8.5 × 11 in.
    Letter,
}

impl Paper {
    /// Both, in menu order.
    pub const ALL: [Paper; 2] = [Paper::A4, Paper::Letter];

    /// Width and height in points, portrait.
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (595.28, 841.89),
            Paper::Letter => (612.0, 792.0),
        }
    }

    /// "A4" or "Letter".
    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::Letter => "Letter",
        }
    }

    /// The paper with this name, ignoring case.
    pub fn from_name(name: &str) -> Option<Paper> {
        Paper::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// How the score is laid out and on what paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// The staff layout.
    pub layout: Layout,
    /// Width of the window's score area in points, for the Vertical layout
    /// (systems fill it, margins included). The app sets it from the
    /// window's width and the score zoom, so zooming in reflows the bars.
    pub screen_width: f32,
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
    /// Draw the score header (title, artist, credits, tuning) at the top.
    /// The Horizontal layout never has one.
    pub header: bool,
    /// Show dynamics markings.
    pub dynamics: bool,
    /// The user's choice of notation for a track, by track index. Tracks
    /// not listed follow the file and [`default_tablature`]. A choice of
    /// neither is shown as standard notation: hiding a track is done by
    /// leaving it out of [`Options::tracks`].
    pub notation: BTreeMap<usize, Notation>,
    /// Bars to mark as needing attention, as (track, staff, bar): the bars
    /// that don't add up, shown on screen in translucent red (M3). Empty
    /// for a PDF, which shows what the file says.
    pub marked: BTreeSet<(usize, usize, usize)>,
    /// The editor's voice (0 to 3): the notes of every other voice are
    /// greyed out, as Guitar Pro's editor shows them (Mateus, 20 Oct
    /// 2026). `None` (prints, PDF) shows every voice in ink.
    pub active_voice: Option<usize>,
}

/// Which notations a track shows (VIEW-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Notation {
    /// Standard notation.
    pub standard: bool,
    /// Tablature (only for tracks with strings; drums never have any).
    pub tablature: bool,
}

impl Notation {
    /// Neither: the track is hidden.
    pub const NONE: Notation = Notation {
        standard: false,
        tablature: false,
    };

    /// Whether anything is shown.
    pub fn any(self) -> bool {
        self.standard || self.tablature
    }
}

/// The notation a track shows unless the user decides otherwise: tablature
/// as [`default_tablature`] says, standard notation where the file shows it
/// or where there is no tablature.
pub fn default_notation(track: &Track) -> Notation {
    let tablature = default_tablature(track);
    let standard = !tablature || track.staves.iter().any(|s| s.show_standard);
    Notation {
        standard,
        tablature,
    }
}

/// Whether a track shows tablature unless the user decides otherwise: only
/// fretted string instruments do (Mateus, 16 Oct 2026), and only where the
/// file shows it. Keyboards, voices, winds, bowed strings and fretless bass
/// are shown in standard notation; the user can still ask for their tab.
pub fn default_tablature(track: &Track) -> bool {
    track.instrument.is_fretted()
        && track
            .staves
            .iter()
            .any(|s| s.show_tablature && s.string_count() > 0)
}

impl Options {
    /// These options on `paper`, with the same margins.
    pub fn with_paper(mut self, paper: Paper) -> Self {
        (self.page_width, self.page_height) = paper.size();
        self
    }
}

impl Default for Options {
    /// A4 paper with 15 mm margins and a 6.4 mm staff.
    fn default() -> Self {
        Options {
            layout: Layout::Page,
            screen_width: 595.28,
            page_width: 595.28,
            page_height: 841.89,
            margin: 42.5,
            staff_space: 4.5,
            tracks: Vec::new(),
            header: true,
            dynamics: true,
            notation: BTreeMap::new(),
            marked: BTreeSet::new(),
            active_voice: None,
        }
    }
}

/// An engraved score.
#[derive(Clone, Debug, Default)]
pub struct Engraved {
    /// The sheets: one per page in the Page layout, exactly one in the
    /// others.
    pub pages: Vec<DisplayList>,
    /// Number of systems (lines of music).
    pub systems: usize,
    /// Where every staff, bar and beat is drawn, for the editor.
    pub hits: Hits,
}

/// Engraves `score` with the bundled music font's metadata.
///
/// Never panics on a damaged score: bars that cannot be measured get a
/// minimal width, notes outside the instrument are left out.
pub fn engrave(score: &Score, fonts: &FontSet, meta: &Metadata, options: &Options) -> Engraved {
    let views = system::views(score, &options.tracks, &options.notation);
    let layout = options.layout;
    let sheet_width = match layout {
        Layout::Vertical => options.screen_width,
        _ => options.page_width,
    };
    let room = options.page_height - 2.0 * options.margin - page::FOOTER;
    let mut sp = options.staff_space.clamp(1.0, 50.0);
    // A system taller than the page (many tracks) is engraved again with a
    // smaller staff, so every system fits on a page. Only pages need this.
    let mut attempts = 0;
    loop {
        let painter = draw::Painter { fonts, meta, sp };
        // The Horizontal layout has no line width: every bar goes on one
        // line at its natural width.
        let width = match layout {
            Layout::Horizontal => f32::INFINITY,
            _ => (sheet_width - 2.0 * options.margin).max(10.0 * sp),
        };
        let plans = system::plan_all(score, &views, &painter, options.dynamics);
        let systems = system::break_systems(score, &views, &plans, &painter, width);
        let drawn: Vec<engrave::DrawnSystem> = systems
            .iter()
            .map(|s| {
                engrave::system(
                    score,
                    &views,
                    &plans,
                    s,
                    &painter,
                    &options.marked,
                    options.active_voice,
                )
            })
            .collect();
        let tallest = drawn.iter().map(|d| d.height).fold(0.0, f32::max);
        if layout.has_pages()
            && tallest > room
            && room > 0.0
            && sp > MIN_STAFF_SPACE
            && attempts < 3
        {
            sp = (sp * room / tallest * 0.98).max(MIN_STAFF_SPACE);
            attempts += 1;
            continue;
        }
        let header = if options.header && layout != Layout::Horizontal {
            page::header(score, &views, &painter, sheet_width, options.margin)
        } else {
            page::Header::default()
        };
        let systems = drawn.len();
        let (pages, hits) = match layout {
            Layout::Page => page::assemble(score, header, drawn, &painter, options),
            _ => {
                let (sheet, hits) =
                    page::sheet(score, header, drawn, &painter, options, sheet_width);
                (vec![sheet], hits)
            }
        };
        return Engraved {
            systems,
            pages,
            hits: Hits { systems: hits },
        };
    }
}

/// The smallest staff space engraving shrinks to, in points (a 4 mm staff).
const MIN_STAFF_SPACE: f32 = 2.8;
