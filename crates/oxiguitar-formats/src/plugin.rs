// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the format plugins share: [`read`] reads a file with the readers in
//! this build and hands back the score and its import log as they cross
//! the plugin interface, and a [`ReadError`] as the interface's error, with
//! the same words, place and offset. [`write_gp`] writes a .gp file from a
//! score that crossed it, with its losses.
//!
//! A plugin built with only its own reader (say `gp5`) refuses every other
//! format as unsupported, even if the host asked it to read one.

use oxiguitar_model::wire;
use oxiguitar_plugin::{Error, ErrorKind, Level, LogEntry, Read};

use crate::{ImportLog, ReadError, ReadErrorKind, Severity};

/// Reads `data`, a whole file, for a format plugin.
pub fn read(data: &[u8]) -> Result<Read, Error> {
    let imported = crate::read(data).map_err(error)?;
    Ok(Read {
        score: wire::to_wire(&imported.score),
        log: log(&imported.log),
    })
}

/// Writes `score`, as it crossed the plugin interface, as a Guitar Pro 7–8
/// file (.gp), with what the file couldn't hold.
#[cfg(feature = "gp")]
pub fn write_gp(
    score: &oxiguitar_plugin::score::Score,
) -> Result<oxiguitar_plugin::Written, Error> {
    let invalid = |message: String| Error {
        kind: ErrorKind::Invalid,
        message,
        offset: None,
        context: String::new(),
    };
    let score = wire::from_wire(score.clone()).map_err(invalid)?;
    let written = crate::write_gp(&score).map_err(invalid)?;
    Ok(oxiguitar_plugin::Written {
        data: written.data,
        losses: log(&written.losses),
    })
}

/// The import log as plugins report it.
pub fn log(log: &ImportLog) -> Vec<LogEntry> {
    log.entries()
        .iter()
        .map(|e| LogEntry {
            level: match e.severity {
                Severity::Info => Level::Info,
                Severity::Warning => Level::Warning,
            },
            message: e.message.clone(),
            place: e.first_place.clone(),
            count: u32::try_from(e.count).unwrap_or(u32::MAX),
        })
        .collect()
}

/// A read error as the interface carries it. Its text, shown by the core,
/// is the same as the reader's own.
pub fn error(e: ReadError) -> Error {
    let kind = match &e.kind {
        ReadErrorKind::UnknownFormat | ReadErrorKind::Unsupported(_) => ErrorKind::Unsupported,
        ReadErrorKind::UnexpectedEnd => ErrorKind::UnexpectedEnd,
        ReadErrorKind::Invalid(_) => ErrorKind::Invalid,
    };
    // The message is the error without its place, which the interface
    // carries apart and adds back in the same words.
    let message = ReadError {
        context: String::new(),
        offset: None,
        ..e.clone()
    }
    .to_string();
    Error {
        kind,
        message,
        offset: e.offset.map(|o| o as u64),
        context: e.context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_the_same_across_the_interface() {
        let e = ReadError {
            kind: ReadErrorKind::UnexpectedEnd,
            offset: Some(42),
            context: "bar 3, track 1".into(),
        };
        let crossed = error(e.clone());
        assert_eq!(crossed.kind, ErrorKind::UnexpectedEnd);
        assert_eq!(crossed.to_string(), e.to_string());
        let unknown = error(crate::read(b"hello").unwrap_err());
        assert_eq!(unknown.kind, ErrorKind::Unsupported);
        assert_eq!(unknown.to_string(), "not a file format OxiGuitar knows");
    }

    #[test]
    fn a_fixture_crosses_with_its_log() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp");
        for name in ["oxiguitar-test-song.gp3", "oxiguitar-test-song.gp"] {
            let data = std::fs::read(dir.join(name)).unwrap();
            let direct = crate::read(&data).unwrap();
            let crossed = read(&data).unwrap();
            assert_eq!(
                wire::from_wire(crossed.score).unwrap(),
                direct.score,
                "{name}"
            );
            assert_eq!(crossed.log, log(&direct.log), "{name}");
        }
    }
}
