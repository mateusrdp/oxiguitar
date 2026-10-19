// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! File formats for OxiGuitar: recognising a file from its contents (FMT-6),
//! reading it into the score model with an import log (FMT-2), and later
//! writing it back.
//!
//! The format plugins (`plugins/gp3` … `plugins/gp`) are thin crates over
//! this one. Each reader is behind its own Cargo feature (`gp3`, `gp4`,
//! `gp5`, `gpx`, `gp`; all on by default), so each plugin carries only its
//! own reader; the `plugin` feature adds what they share ([`plugin`]). The
//! core itself doesn't use this crate: it opens what the installed plugins
//! recognise (docs/design/plugins.md).
//!
//! Every reader treats its input as untrusted. A damaged file gives a
//! [`ReadError`] naming the byte offset and what was being read; it never
//! panics (FMT-1).
//!
//! ```no_run
//! let bytes = std::fs::read("song.gp5")?;
//! let imported = oxiguitar_formats::read(&bytes)?;
//! println!("{} bars", imported.score.bar_count());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// A build with no reader at all keeps only detection, so the readers'
// helpers in the shared modules go unused.
#![cfg_attr(
    not(any(
        feature = "gp3",
        feature = "gp4",
        feature = "gp5",
        feature = "gpx",
        feature = "gp"
    )),
    allow(dead_code)
)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

#[cfg(any(feature = "gp3", feature = "gp4", feature = "gp5"))]
mod binary;
#[cfg(any(feature = "gpx", feature = "gp"))]
mod container;
mod detect;
mod error;
#[cfg(any(feature = "gp3", feature = "gp4", feature = "gp5"))]
mod gp;
#[cfg(any(feature = "gpx", feature = "gp"))]
mod gpif;
#[cfg(feature = "plugin")]
pub mod plugin;
#[cfg(any(feature = "gpx", feature = "gp"))]
mod xml;

pub use detect::{Format, detect};
pub use error::{ImportLog, LogEntry, ReadError, ReadErrorKind, Severity};

use oxiguitar_model::Score;

/// A file read into the score model.
#[derive(Clone, Debug)]
pub struct Imported {
    /// The format the contents were recognised as.
    pub format: Format,
    /// The score.
    pub score: Score,
    /// What could not be carried over faithfully.
    pub log: ImportLog,
    /// Bytes left over after the reader finished. Usually 0; a large number
    /// suggests the reader misunderstood part of the file.
    pub trailing_bytes: usize,
}

/// Recognises the format of `data` from its contents and reads it.
///
/// Reads Guitar Pro 3, 4 and 5 (binary), Guitar Pro 6 (.gpx) and Guitar Pro
/// 7 and 8 (.gp), each when its feature is on. Any other format, or one
/// whose reader isn't in this build, gives [`ReadErrorKind::Unsupported`].
pub fn read(data: &[u8]) -> Result<Imported, ReadError> {
    let format = detect(data).ok_or_else(|| ReadError::new(ReadErrorKind::UnknownFormat, None))?;
    match &format {
        #[cfg(feature = "gp3")]
        Format::GuitarPro3 { .. } => read_binary(data, format),
        #[cfg(feature = "gp4")]
        Format::GuitarPro4 { .. } => read_binary(data, format),
        #[cfg(feature = "gp5")]
        Format::GuitarPro5 { .. } => read_binary(data, format),
        #[cfg(feature = "gpx")]
        Format::GuitarPro6 => {
            let r = gpif::read_gpx(data)?;
            Ok(Imported {
                format,
                score: r.score,
                log: r.log,
                trailing_bytes: 0,
            })
        }
        #[cfg(feature = "gp")]
        Format::GuitarPro7Plus => {
            let r = gpif::read_gp7(data)?;
            Ok(Imported {
                format,
                score: r.score,
                log: r.log,
                trailing_bytes: 0,
            })
        }
        other => Err(ReadError::new(
            ReadErrorKind::Unsupported(format!("reading {}", other.describe())),
            None,
        )),
    }
}

/// Reads a Guitar Pro 3, 4 or 5 file.
/// A score written as a file, and what the file couldn't hold (FMT-3).
#[derive(Clone, Debug)]
pub struct Written {
    /// The file's contents.
    pub data: Vec<u8>,
    /// What was left out or changed, each with the first place it happens;
    /// empty when the file reads back as the score.
    pub losses: ImportLog,
}

/// Writes `score` as a Guitar Pro 7–8 file (.gp). Fails only for a score
/// that isn't whole: no tracks, no bars, or a staff missing bars.
#[cfg(feature = "gp")]
pub fn write_gp(score: &Score) -> Result<Written, String> {
    let w = gpif::write::write_gp(score)?;
    Ok(Written {
        data: w.data,
        losses: w.losses,
    })
}

#[cfg(any(feature = "gp3", feature = "gp4", feature = "gp5"))]
fn read_binary(data: &[u8], format: Format) -> Result<Imported, ReadError> {
    let mut r = gp::read(data, &format)?;
    // Guitar Pro 3 and 4 files end with four zero bytes; more than that is
    // data we did not understand.
    if r.trailing_bytes > 4 {
        r.log.info(
            format!(
                "{} bytes after the end of the score were ignored",
                r.trailing_bytes
            ),
            "end of file",
        );
    }
    Ok(Imported {
        format,
        score: r.score,
        log: r.log,
        trailing_bytes: r.trailing_bytes,
    })
}
