// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Read errors and the import log.

use std::fmt;

/// Why a file could not be read. Every error says where in the file the
/// problem was found (FMT-1); the caller adds the file name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadError {
    /// What went wrong.
    pub kind: ReadErrorKind,
    /// Byte offset in the file where it was found, if known.
    pub offset: Option<usize>,
    /// What the reader was reading at the time, such as "bar 12, track 2".
    pub context: String,
}

/// The kinds of read error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadErrorKind {
    /// The contents do not match any format we know.
    UnknownFormat,
    /// A format we recognise but don't read (or not in this build).
    Unsupported(String),
    /// The file ends before the data it promises.
    UnexpectedEnd,
    /// A value is outside what the format allows.
    Invalid(String),
}

impl ReadError {
    pub(crate) fn new(kind: ReadErrorKind, offset: Option<usize>) -> ReadError {
        ReadError {
            kind,
            offset,
            context: String::new(),
        }
    }

    pub(crate) fn invalid(msg: impl Into<String>, offset: usize) -> ReadError {
        ReadError::new(ReadErrorKind::Invalid(msg.into()), Some(offset))
    }

    /// Adds what was being read, unless a more specific context is already set.
    pub(crate) fn within(mut self, context: impl FnOnce() -> String) -> ReadError {
        if self.context.is_empty() {
            self.context = context();
        }
        self
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ReadErrorKind::UnknownFormat => write!(f, "not a file format OxiGuitar knows")?,
            ReadErrorKind::Unsupported(what) => write!(f, "{what} is not supported yet")?,
            ReadErrorKind::UnexpectedEnd => write!(f, "the file ends too early")?,
            ReadErrorKind::Invalid(what) => write!(f, "{what}")?,
        }
        if !self.context.is_empty() {
            write!(f, " (in {})", self.context)?;
        }
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadError {}

/// How serious an import-log entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Something worth knowing; nothing was lost.
    Info,
    /// Something in the file could not be represented and was left out or
    /// changed (FMT-2).
    Warning,
}

/// One entry in the import log. Identical messages are counted, not repeated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// How serious it is.
    pub severity: Severity,
    /// What happened.
    pub message: String,
    /// The first place it happened, such as "bar 3, track 1".
    pub first_place: String,
    /// How many times it happened.
    pub count: usize,
}

/// Everything a reader could not carry over faithfully (FMT-2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportLog {
    entries: Vec<LogEntry>,
}

impl ImportLog {
    /// Records a message; a repeated message only increases its count.
    pub fn add(
        &mut self,
        severity: Severity,
        message: impl Into<String>,
        place: impl Into<String>,
    ) {
        let message = message.into();
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.message == message && e.severity == severity)
        {
            e.count += 1;
            return;
        }
        self.entries.push(LogEntry {
            severity,
            message,
            first_place: place.into(),
            count: 1,
        });
    }

    /// Records a warning: something was lost or changed.
    pub fn warn(&mut self, message: impl Into<String>, place: impl Into<String>) {
        self.add(Severity::Warning, message, place);
    }

    /// Records an informational note.
    pub fn info(&mut self, message: impl Into<String>, place: impl Into<String>) {
        self.add(Severity::Info, message, place);
    }

    /// All entries, in the order first seen.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// True if nothing was logged.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of warnings, counting repeats.
    pub fn warning_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.severity == Severity::Warning)
            .map(|e| e.count)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_messages_are_counted() {
        let mut log = ImportLog::default();
        log.warn("wah not supported", "bar 1");
        log.warn("wah not supported", "bar 2");
        log.info("other", "bar 3");
        assert_eq!(log.entries().len(), 2);
        assert_eq!(log.entries()[0].count, 2);
        assert_eq!(log.entries()[0].first_place, "bar 1");
        assert_eq!(log.warning_count(), 2);
    }

    #[test]
    fn error_message_names_position() {
        let e = ReadError::new(ReadErrorKind::UnexpectedEnd, Some(120)).within(|| "bar 3".into());
        assert_eq!(
            e.to_string(),
            "the file ends too early (in bar 3) at byte 120"
        );
    }
}
