// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Generated test scores.
//!
//! Songs are written in a short text notation ([`song`]) and saved as Guitar
//! Pro 7 files ([`gpif`]), so the tests that use them go through the real
//! file readers. Two sets are built from it:
//!
//! - [`band_song`]: a long eight-track score for the speed budget (M1: a
//!   300-bar, 8-track file opens in under a second).
//! - [`references()`]: the visual-regression references, kept in
//!   `fixtures/reference/` with the pictures they must engrave to.
//!
//! Test code only: no OxiGuitar program depends on this crate.

pub mod big;
pub mod gpif;
pub mod references;
pub mod song;

pub use big::band_song;
pub use gpif::{to_gp, to_gpif};
pub use references::{Reference, references};
pub use song::{Diagram, Kind, MasterBar, Song, Track};
