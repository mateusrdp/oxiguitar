// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score model at the centre of OxiGuitar.
//!
//! Every file format is read into, and written from, this model. It holds
//! data only: no file handling, layout or sound.
//!
//! ```text
//! Score
//! ├── ScoreInfo              title, artist, …
//! ├── MasterBar × bars       time and key signature, repeats, tempo, markers
//! └── Track × tracks         name, instrument, mixer, chord diagrams, lyrics
//!     └── Staff              tuning, capo, notation/tab visibility
//!         └── Bar × bars     clef
//!             └── Voice ×1–4
//!                 └── Beat   duration, notes or rest, beat effects
//!                     └── Note   string and fret (or pitch), note effects
//! ```
//!
//! Musical time is exact: durations are [`Ratio`]s of a whole note, so
//! tuplets never drift. The first version follows the structures shared by
//! TuxGuitar, alphaTab and Guitar Pro's GPIF format; see
//! `docs/adr/0002-score-model.md` for the reasoning.

mod bar;
mod duration;
mod note;
mod ratio;
mod score;
mod track;
pub mod tuning;
#[cfg(feature = "wire")]
pub mod wire;

pub use bar::{
    Bar, Beat, BeatEffects, BeatKind, BeatTechnique, Clef, GraceKind, Stroke, StrokeDirection,
    Voice,
};
pub use duration::{Duration, NoteValue, Tuplet};
pub use note::{
    Accent, Bend, BendPoint, Dynamic, Harmonic, Note, NoteEffects, NotePlace, Pitch, Slide, Trill,
};
pub use ratio::Ratio;
pub use score::{
    Color, Direction, KeyMode, KeySignature, Marker, MasterBar, Score, ScoreInfo, TempoChange,
    TimeSignature, TripletFeel,
};
pub use track::{ChordDiagram, Instrument, LyricLine, Mix, Staff, Track};
