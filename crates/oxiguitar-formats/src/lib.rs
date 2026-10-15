// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! File formats for OxiGuitar: recognising a file from its contents (FMT-6),
//! reading it into the score model with an import log (FMT-2), and later
//! writing it back.
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

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod binary;
mod detect;
mod error;
mod gp;

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
pub fn read(data: &[u8]) -> Result<Imported, ReadError> {
    let format = detect(data).ok_or_else(|| ReadError::new(ReadErrorKind::UnknownFormat, None))?;
    match &format {
        Format::GuitarPro3 { .. } | Format::GuitarPro4 { .. } | Format::GuitarPro5 { .. } => {
            let mut r = gp::read(data, &format)?;
            // Guitar Pro 3 and 4 files end with four zero bytes; more than that
            // is data we did not understand.
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
        other => Err(ReadError::new(
            ReadErrorKind::Unsupported(format!("reading {}", other.describe())),
            None,
        )),
    }
}
