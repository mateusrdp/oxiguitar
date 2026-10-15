// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Draws an OxiGuitar [display list](oxiguitar_display::DisplayList).
//!
//! Two backends read the same list and the same font bytes:
//!
//! - [`raster`]: anti-aliased pixels on the CPU with tiny-skia. The GUI shows
//!   these pixels as the score canvas; the same code writes PNG files and the
//!   images visual regression tests compare (ADR 0005).
//! - [`pdf`]: vector PDF with krilla, fonts embedded as subsets (EXP-1).
//!
//! Neither backend shapes text or decides positions: everything comes from
//! the list, so the screen and the PDF agree by construction.

pub mod pdf;
pub mod raster;

pub use pdf::{PdfError, to_pdf};
pub use raster::{Raster, View};
