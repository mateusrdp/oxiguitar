// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's editor, without a window (M3).
//!
//! Everything here works on the score model alone, so the window stays a
//! thin shell and every rule is tested without a display:
//!
//! - [`Caret`]: where typing goes, in the model's own terms (track, staff,
//!   voice, bar, beat, string), and how the keys move it, as Guitar Pro 8
//!   moves its cursor.
//! - [`Selection`]: a run of beats in one voice, or whole bars across
//!   tracks.
//! - [`History`]: edits as commands, with unlimited undo and redo (EDIT-5).
//!   A command says which parts of the score it changes ([`Region`]); only
//!   those are copied, before and after, so undoing a note costs a bar, not
//!   the score.
//! - [`apply`]: note entry, each [`Command`] typed at the caret one
//!   undoable change.
//! - [`BarCheck`]: the bars whose beats don't add up to their time
//!   signature. Every bar is checked once when a file opens; after that only
//!   the bars an edit touched ([`Touched`]), nothing more (Mateus, 19 Oct
//!   2026: speed first). Such bars are allowed; the window marks them.
//!
//! See `docs/design/editor.md` for the rules and the parity checklist.

mod bars;
mod caret;
mod check;
mod clip;
mod entry;
mod history;
mod selection;
mod tracks;
mod transpose;

pub use bars::BarState;
pub use caret::{Caret, StaffBar, VOICES, beat_at, onsets};
pub use check::{BarCheck, adds_up};
pub use clip::{Clip, ClipTrack, Paste, copy};
pub use entry::{Command, Edited, apply, new_score};
pub use history::{Change, History, Region, Touched};
pub use selection::{Selection, Span};
pub use tracks::{TrackSettings, notes_lost};
/// How many notes a transposition would leave out.
pub use transpose::notes_lost as notes_lost_transposing;
pub use transpose::{Scope, Transposition};
