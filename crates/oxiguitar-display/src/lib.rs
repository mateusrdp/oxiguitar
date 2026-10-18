// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The display list: what OxiGuitar's layout produces and every renderer draws.
//!
//! Layout turns the score model into positioned glyphs, lines, rectangles and
//! paths. The screen, PDF, PNG and (later) SVG backends all draw the same list,
//! so what you see is what you export (see ADR 0005).
//!
//! - [`list`]: the display list itself, in points (1/72 inch), origin at the
//!   top left of the page, y pointing down.
//! - [`fonts`]: the fonts a list refers to, with the bundled Bravura and Noto
//!   Sans, and simple text placement.
//! - [`smufl`]: the parts of a SMuFL font's metadata file that layout needs:
//!   engraving defaults, glyph bounding boxes and anchors.
//! - [`specimen`]: a hand-placed guitar excerpt used by the render spike, the
//!   GUI spikes and the renderer tests until real layout arrives in M1.
//! - `wire` (feature `wire`): what a plugin draws on its tab's canvas, as a
//!   display list the core's renderers draw.

pub mod fonts;
pub mod list;
pub mod smufl;
pub mod specimen;
#[cfg(feature = "wire")]
pub mod wire;

pub use fonts::{FontId, FontSet};
pub use list::{
    DisplayList, GlyphRun, Item, Paint, Path, PathCmd, PlacedGlyph, Point, Rect, Rgba, Theme,
};
