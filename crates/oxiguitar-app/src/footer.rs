// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The footer's messages: one line at the bottom of the window shows the
//! latest; clicking it expands the whole list for this session. It holds
//! errors and warnings only: it isn't a log (Mateus, 19 Oct 2026), so
//! messages that are only worth knowing (`Level::Info`: the plugins found,
//! a tool's report, a file exported) aren't kept. A message may end with a
//! link (plugin interface 1.3), which opens in the system's browser when
//! clicked. No window code here.

use oxiguitar_host::{Level, Notice};

/// One message, with its link if it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) level: Level,
    pub(crate) text: String,
    /// What the link says, and its `https:` address.
    pub(crate) link: Option<(String, String)>,
}

/// Most messages kept: a long session can't grow the list without bound.
const KEEP: usize = 500;

/// This session's messages, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Footer {
    messages: Vec<Message>,
}

impl Footer {
    pub(crate) fn push(&mut self, level: Level, text: impl Into<String>) {
        self.push_message(level, text.into(), None);
    }

    /// A message ending with a link to `url` (an `https:` address; anything
    /// else is shown without its link).
    pub(crate) fn push_with_link(
        &mut self,
        level: Level,
        text: impl Into<String>,
        link_text: impl Into<String>,
        url: impl Into<String>,
    ) {
        let url = url.into();
        let link = oxiguitar_host::is_https(&url).then(|| (link_text.into(), url));
        self.push_message(level, text.into(), link);
    }

    fn push_message(&mut self, level: Level, text: String, link: Option<(String, String)>) {
        if level == Level::Info || text.trim().is_empty() {
            return;
        }
        self.messages.push(Message { level, text, link });
        let extra = self.messages.len().saturating_sub(KEEP);
        self.messages.drain(..extra);
    }

    pub(crate) fn extend(&mut self, notices: impl IntoIterator<Item = Notice>) {
        for n in notices {
            self.push(n.level, n.text);
        }
    }

    /// The line the footer shows.
    pub(crate) fn latest(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Every message, newest first, as the expanded footer lists them.
    pub(crate) fn newest_first(&self) -> impl Iterator<Item = &Message> {
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
        f.push(Level::Error, "one");
        f.push(Level::Warning, "two");
        f.push(Level::Error, "  ");
        // Only worth knowing: not kept.
        f.push(Level::Info, "Plugins: Guitar Pro 5 0.1.0.");
        assert_eq!(f.len(), 2);
        assert_eq!(f.latest().map(|n| n.text.as_str()), Some("two"));
        let order: Vec<&str> = f.newest_first().map(|n| n.text.as_str()).collect();
        assert_eq!(order, ["two", "one"]);
        for i in 0..KEEP {
            f.push(Level::Warning, format!("{i}"));
        }
        assert_eq!(f.len(), KEEP);
        assert_eq!(f.newest_first().last().map(|n| n.text.as_str()), Some("0"));
        assert_eq!(level_code(Level::Error), 2);
    }

    #[test]
    fn a_message_may_end_with_an_https_link() {
        let mut f = Footer::default();
        f.push_with_link(
            Level::Warning,
            "Only a basic sound set plays.",
            "Better sounds",
            "https://example.org/sounds",
        );
        let m = f.latest().unwrap();
        assert_eq!(
            m.link,
            Some(("Better sounds".into(), "https://example.org/sounds".into()))
        );
        f.push_with_link(Level::Warning, "Not a link.", "Here", "file:///etc/passwd");
        assert_eq!(f.latest().unwrap().link, None);
        f.push_with_link(Level::Info, "Not kept.", "Here", "https://example.org");
        assert_eq!(f.len(), 2);
    }
}
