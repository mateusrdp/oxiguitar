// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the three GUI spikes share, so they differ only in the toolkit:
//! the palette's tools, the track list and the score canvas.
//!
//! The canvas is the real renderer from `oxiguitar-render`: each toolkit asks
//! for the visible part of the page at its zoom and pixel density and shows
//! the pixels it gets back.

pub mod icons;

use oxiguitar_display::{DisplayList, FontSet, Theme, smufl::Metadata, specimen};
use oxiguitar_render::{Raster, View};

/// Bravura, so toolkits can draw SMuFL symbols on palette buttons.
pub use oxiguitar_display::fonts::BRAVURA_OTF;
/// Noto Sans, the same interface font in all three spikes.
pub use oxiguitar_display::fonts::NOTO_SANS_REGULAR;

/// One palette tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    /// Stable id.
    pub id: &'static str,
    /// Name shown in tooltips, the command search and to screen readers.
    pub name: &'static str,
    /// Button face: a SMuFL symbol drawn in Bravura, or short text.
    pub face: Face,
    /// Keyboard shortcut (a single key, no modifiers, in this spike).
    pub key: char,
    /// Palette group.
    pub group: &'static str,
    /// Whether tools of this group exclude each other (note values do).
    pub exclusive: bool,
}

/// What a palette button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// SMuFL codepoints, drawn with Bravura (usually one).
    Symbol(&'static str),
    /// Plain text, drawn with the interface font.
    Text(&'static str),
}

/// The palette: two groups, as on the left of Guitar Pro's window.
pub const TOOLS: &[Tool] = &[
    t(
        "whole",
        "Whole note",
        Face::Symbol("\u{E1D2}"),
        '1',
        "Note values",
        true,
    ),
    t(
        "half",
        "Half note",
        Face::Symbol("\u{E1D3}"),
        '2',
        "Note values",
        true,
    ),
    t(
        "quarter",
        "Quarter note",
        Face::Symbol("\u{E1D5}"),
        '3',
        "Note values",
        true,
    ),
    t(
        "eighth",
        "Eighth note",
        Face::Symbol("\u{E1D7}"),
        '4',
        "Note values",
        true,
    ),
    t(
        "16th",
        "Sixteenth note",
        Face::Symbol("\u{E1D9}"),
        '5',
        "Note values",
        true,
    ),
    t(
        "32nd",
        "Thirty-second note",
        Face::Symbol("\u{E1DB}"),
        '6',
        "Note values",
        true,
    ),
    t(
        "dot",
        "Dotted",
        Face::Symbol("\u{E1E7}"),
        '.',
        "Note values",
        false,
    ),
    t(
        "rest",
        "Rest",
        Face::Symbol("\u{E4E5}"),
        'r',
        "Note values",
        false,
    ),
    t(
        "hammer",
        "Hammer-on / pull-off",
        Face::Text("H"),
        'h',
        "Effects",
        false,
    ),
    t("slide", "Slide", Face::Text("/"), 's', "Effects", false),
    t(
        "bend",
        "Bend",
        Face::Symbol("\u{EB61}"),
        'b',
        "Effects",
        false,
    ),
    t(
        "vibrato",
        "Vibrato",
        Face::Symbol("\u{EAB2}\u{EAB2}\u{EAB2}"),
        'v',
        "Effects",
        false,
    ),
    t(
        "palm",
        "Palm mute",
        Face::Text("P.M."),
        'p',
        "Effects",
        false,
    ),
    t(
        "ring",
        "Let ring",
        Face::Text("L.R."),
        'l',
        "Effects",
        false,
    ),
    t(
        "dead",
        "Dead note",
        Face::Symbol("\u{E0A9}"),
        'x',
        "Effects",
        false,
    ),
    t(
        "ghost",
        "Ghost note",
        Face::Text("( )"),
        'g',
        "Effects",
        false,
    ),
    t(
        "accent",
        "Accent",
        Face::Symbol("\u{E4A0}"),
        'a',
        "Effects",
        false,
    ),
    t(
        "staccato",
        "Staccato",
        Face::Symbol("\u{E4A2}"),
        't',
        "Effects",
        false,
    ),
];

const fn t(
    id: &'static str,
    name: &'static str,
    face: Face,
    key: char,
    group: &'static str,
    exclusive: bool,
) -> Tool {
    Tool {
        id,
        name,
        face,
        key,
        group,
        exclusive,
    }
}

/// Palette groups, in order.
pub const GROUPS: &[&str] = &["Note values", "Effects"];

/// Which palette tools are on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteState {
    on: Vec<bool>,
}

impl Default for PaletteState {
    fn default() -> Self {
        let mut s = Self {
            on: vec![false; TOOLS.len()],
        };
        s.toggle(2); // quarter note
        s
    }
}

impl PaletteState {
    /// Whether tool `i` is on.
    pub fn is_on(&self, i: usize) -> bool {
        self.on.get(i).copied().unwrap_or(false)
    }

    /// Toggle tool `i`; an exclusive tool turns the others in its group off.
    pub fn toggle(&mut self, i: usize) {
        let Some(tool) = TOOLS.get(i) else { return };
        if tool.exclusive {
            for (j, other) in TOOLS.iter().enumerate() {
                if other.exclusive && other.group == tool.group {
                    self.on[j] = j == i;
                }
            }
        } else {
            self.on[i] = !self.on[i];
        }
    }

    /// Toggle the tool bound to `key`, if any. Returns its index.
    pub fn press(&mut self, key: char) -> Option<usize> {
        let i = TOOLS
            .iter()
            .position(|t| t.key == key.to_ascii_lowercase())?;
        self.toggle(i);
        Some(i)
    }

    /// A one-line summary for the status bar.
    pub fn summary(&self) -> String {
        let names: Vec<&str> = TOOLS
            .iter()
            .enumerate()
            .filter(|(i, _)| self.is_on(*i))
            .map(|(_, t)| t.name)
            .collect();
        if names.is_empty() {
            "No tool".into()
        } else {
            names.join(" + ")
        }
    }
}

/// Indices of tools whose name matches a command-search query.
pub fn search(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    TOOLS
        .iter()
        .enumerate()
        .filter(|(_, t)| q.is_empty() || t.name.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// One row of the track list.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Track name, editable.
    pub name: String,
    /// Instrument.
    pub instrument: &'static str,
    /// Muted.
    pub mute: bool,
    /// Soloed.
    pub solo: bool,
    /// Volume, 0 to 1.
    pub volume: f32,
}

/// The tracks every spike starts with. Names are in Portuguese, Japanese and
/// Arabic on purpose: the spikes must show them, right-to-left included,
/// and let you edit them.
pub fn tracks() -> Vec<Track> {
    vec![
        track("Violão — Água de Beber", "Nylon guitar"),
        track("Lead guitar", "Distortion guitar"),
        track("ギター・ソロ", "Steel guitar"),
        track("عود عربي", "Oud"),
        track("Drums", "Drum kit"),
    ]
}

fn track(name: &str, instrument: &'static str) -> Track {
    Track {
        name: name.to_owned(),
        instrument,
        mute: false,
        solo: false,
        volume: 0.8,
    }
}

/// The score canvas: the render spike's specimen page and a renderer.
pub struct Canvas {
    fonts: FontSet,
    page: DisplayList,
    raster: Raster,
}

/// Pixels for one frame of the canvas.
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Premultiplied RGBA, row by row. Opaque here, so equal to straight RGBA.
    pub rgba: Vec<u8>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    /// The specimen page with the bundled fonts.
    pub fn new() -> Self {
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().expect("bundled Bravura metadata parses");
        let page = specimen::guitar_excerpt(&fonts, &meta);
        Self {
            fonts,
            page,
            raster: Raster::new(),
        }
    }

    /// Page size in points.
    pub fn page_size(&self) -> (f32, f32) {
        (self.page.width, self.page.height)
    }

    /// Draw the part of the page at (`x`, `y`) points, `width` × `height`
    /// pixels, at `zoom` (1.0 = 100 %, 96 dpi) on a screen with `dpr`
    /// physical pixels per logical pixel.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        zoom: f32,
        dpr: f32,
        dark: bool,
    ) -> Frame {
        let theme = if dark { Theme::DARK } else { Theme::LIGHT };
        self.frame_themed(x, y, width, height, zoom, dpr, &theme)
    }

    /// Like [`Canvas::frame`], with any page theme.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_themed(
        &mut self,
        x: f32,
        y: f32,
        width: u32,
        height: u32,
        zoom: f32,
        dpr: f32,
        theme: &Theme,
    ) -> Frame {
        let scale = zoom * 96.0 / 72.0 * dpr;
        let view = View {
            scale,
            x,
            y,
            width: width.max(1),
            height: height.max(1),
        };
        let rgba = self
            .raster
            .render(&self.page, &self.fonts, theme, &view)
            .map(|p| p.take())
            .unwrap_or_else(|| vec![0; (view.width * view.height * 4) as usize]);
        Frame {
            width: view.width,
            height: view.height,
            rgba,
        }
    }

    /// The page's size in logical pixels at `zoom`.
    pub fn page_pixels(&self, zoom: f32) -> (f32, f32) {
        let s = zoom * 96.0 / 72.0;
        (self.page.width * s, self.page.height * s)
    }
}

/// Zoom steps, as in Guitar Pro's zoom menu.
pub const ZOOMS: &[f32] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

/// The next zoom step up (`dir` > 0) or down.
pub fn step_zoom(zoom: f32, dir: i32) -> f32 {
    let i = ZOOMS
        .iter()
        .position(|z| (*z - zoom).abs() < 1e-3)
        .unwrap_or(3) as i32;
    let j = (i + dir).clamp(0, ZOOMS.len() as i32 - 1) as usize;
    ZOOMS[j]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_values_exclude_each_other() {
        let mut p = PaletteState::default();
        p.press('4');
        assert!(p.is_on(3) && !p.is_on(2));
        p.press('h');
        p.press('H');
        assert!(!p.is_on(8));
    }

    #[test]
    fn search_is_case_insensitive() {
        assert_eq!(search("BEND").len(), 1);
        assert_eq!(search("").len(), TOOLS.len());
    }

    #[test]
    fn canvas_renders() {
        let mut c = Canvas::new();
        let f = c.frame(0.0, 0.0, 100, 50, 1.0, 1.0, false);
        assert_eq!(f.rgba.len(), 100 * 50 * 4);
    }
}
