// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Score tools: actions the user starts from the Tools menu that change a
//! score, such as inferring key signatures the file left out.
//!
//! Nothing here runs on its own. Opening, showing and exporting a score
//! always use what the file says; a tool changes the score only when the
//! user picks it (decided 16 Oct 2026).
//!
//! Every tool has the same small shape, [`Tool`]: an id, a name and a
//! description for the menu, whether it can run on a score, and a pure
//! function from a score to a changed score and a report. Tools depend on
//! the score model only, so in M5 the plugin system can host tools with the
//! same shape, and any tool here can move into a plugin unchanged. In the
//! editor (M2), applying a tool becomes one undoable command.

use oxiguitar_model::Score;

pub mod keys;

/// An action from the Tools menu.
pub trait Tool: Sync {
    /// A stable identifier, such as `infer-key-signatures`, for the command
    /// line, shortcuts and settings.
    fn id(&self) -> &'static str;
    /// The menu entry, such as "Infer Key Signatures".
    fn name(&self) -> &'static str;
    /// One sentence for the tooltip and the manual.
    fn description(&self) -> &'static str;
    /// Whether the tool can run on `score`; the menu greys it out with the
    /// reason otherwise.
    fn availability(&self, score: &Score) -> Availability;
    /// Runs the tool. Never panics on a damaged score; leaves `score` as it
    /// is and returns the changed copy, if any, with a report.
    fn apply(&self, score: &Score) -> Outcome;
}

/// Whether a tool can run on a score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The tool can run.
    Available,
    /// The tool cannot run, for this reason (shown to the user).
    Unavailable(String),
}

/// What a tool did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The changed score, or `None` if the tool changed nothing.
    pub score: Option<Score>,
    /// A short report for the status line, such as "Key signatures set:
    /// 1♯ in bars 1–199."
    pub report: String,
}

/// Every built-in tool, in menu order.
pub fn all() -> &'static [&'static dyn Tool] {
    &[&keys::InferKeySignatures]
}

/// The built-in tool with this id.
pub fn find(id: &str) -> Option<&'static dyn Tool> {
    all().iter().copied().find(|t| t.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_have_unique_ids_and_names() {
        let tools = all();
        for (i, a) in tools.iter().enumerate() {
            assert!(!a.name().is_empty() && !a.description().is_empty());
            assert!(a.id().chars().all(|c| c.is_ascii_lowercase() || c == '-'));
            for b in tools.iter().skip(i + 1) {
                assert_ne!(a.id(), b.id());
                assert_ne!(a.name(), b.name());
            }
        }
        assert!(find("infer-key-signatures").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn tools_do_not_panic_on_an_empty_score() {
        for t in all() {
            let _ = t.availability(&Score::new());
            let _ = t.apply(&Score::new());
        }
    }
}
