// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The footer's messages: one line at the bottom of the window shows the
//! latest; clicking it expands the whole list for this session (errors,
//! warnings, tool reports, plugin notes). No window code here.

use oxiguitar_host::{Level, Notice};

/// Most messages kept: a long session can't grow the list without bound.
const KEEP: usize = 500;

/// This session's messages, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Footer {
    messages: Vec<Notice>,
}

impl Footer {
    pub(crate) fn push(&mut self, level: Level, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.messages.push(Notice::new(level, text));
        let extra = self.messages.len().saturating_sub(KEEP);
        self.messages.drain(..extra);
    }

    pub(crate) fn extend(&mut self, notices: impl IntoIterator<Item = Notice>) {
        for n in notices {
            self.push(n.level, n.text);
        }
    }

    /// The line the footer shows.
    pub(crate) fn latest(&self) -> Option<&Notice> {
        self.messages.last()
    }

    /// Every message, newest first, as the expanded footer lists them.
    pub(crate) fn newest_first(&self) -> impl Iterator<Item = &Notice> {
        self.messages.iter().rev()
    }

    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }
}

/// The level as the window's colour code: 0 info, 1 warning, 2 error.
pub(crate) fn level_code(level: Level) -> i32 {
    match level {
        Level::Info => 0,
        Level::Warning => 1,
        Level::Error => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_message_shows_and_the_rest_are_kept() {
        let mut f = Footer::default();
        assert!(f.latest().is_none());
        f.push(Level::Info, "one");
        f.push(Level::Warning, "two");
        f.push(Level::Error, "  ");
        assert_eq!(f.len(), 2);
        assert_eq!(f.latest().map(|n| n.text.as_str()), Some("two"));
        let order: Vec<&str> = f.newest_first().map(|n| n.text.as_str()).collect();
        assert_eq!(order, ["two", "one"]);
        for i in 0..KEEP {
            f.push(Level::Info, format!("{i}"));
        }
        assert_eq!(f.len(), KEEP);
        assert_eq!(f.newest_first().last().map(|n| n.text.as_str()), Some("0"));
        assert_eq!(level_code(Level::Error), 2);
    }
}
