// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Advice, as the window holds it: the dialogs plugins (interface 1.4) and
//! the core ask for, waiting for their turn. Plugins ask from any thread,
//! at any time, even before the window exists, so the waiting line is a
//! plain queue behind a lock; the window takes from it on its own thread,
//! one dialog at a time, once it is up.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use oxiguitar_host::advice::Asked;

/// Advice asked for and not yet shown, oldest first.
static WAITING: Mutex<VecDeque<Asked>> = Mutex::new(VecDeque::new());

/// Puts `asked` at the end of the line. The host has already left out what
/// the user silenced and what was shown this session.
pub(crate) fn ask(asked: Asked) {
    WAITING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push_back(asked);
}

/// The next advice to show, if any is waiting.
pub(crate) fn next() -> Option<Asked> {
    WAITING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .pop_front()
}

/// The dialog's text as lines: paragraphs on their own lines, none empty
/// at the ends.
pub(crate) fn lines(text: &str) -> Vec<String> {
    text.trim()
        .lines()
        .map(|l| l.trim_end().to_owned())
        .collect()
}

/// Who is talking, for the dialog's small print.
pub(crate) fn source(asked: &Asked) -> String {
    if asked.plugin == oxiguitar_host::advice::CORE {
        "From OxiGuitar".to_owned()
    } else {
        format!("From the plugin \u{201c}{}\u{201d}", asked.plugin_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_host::api::message::Advice;

    fn asked(plugin: &str, id: &str) -> Asked {
        Asked {
            plugin: plugin.into(),
            plugin_name: "MIDI (SoundFont, SFZ)".into(),
            advice: Advice {
                id: id.into(),
                title: "t".into(),
                text: "x".into(),
                links: vec![],
            },
        }
    }

    #[test]
    fn advice_waits_in_the_order_it_was_asked() {
        // The queue is shared by the whole process: drain what other tests
        // left, then check ours.
        while next().is_some() {}
        ask(asked("a", "one"));
        ask(asked("b", "two"));
        assert_eq!(next().unwrap().advice.id, "one");
        assert_eq!(next().unwrap().advice.id, "two");
        assert!(next().is_none());
    }

    #[test]
    fn text_becomes_lines() {
        assert_eq!(lines("\nOne.\n\nTwo.  \n"), ["One.", "", "Two."]);
        assert!(lines("  ").is_empty());
    }

    #[test]
    fn the_source_names_the_plugin_or_the_core() {
        assert_eq!(
            source(&asked("oxiguitar.midi", "x")),
            "From the plugin \u{201c}MIDI (SoundFont, SFZ)\u{201d}"
        );
        assert_eq!(
            source(&Asked::core(asked("", "x").advice)),
            "From OxiGuitar"
        );
    }
}
