// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A second plugin for testing the plugin interface (never shipped).
//!
//! It reads the test plugin's format, `oxgtest`, giving every score the
//! title "Read by the second plugin", so the contract tests can see that a
//! file the test plugin fails on (it panics on the title `panic`) goes to
//! the next plugin that recognises it, as if the test plugin had never been
//! installed (docs/design/plugins.md, "How the host loads and chooses").
//!
//! Built for interface 1.1, it hears the selection as 1.1 gives it
//! (`Event::Selection`, where the test plugin, built for 1.2, hears
//! `Event::Selection2`), and says each in the footer as a warning,
//! starting "heard", for the contract tests to see.

use oxiguitar_plugin::score::{Score, ScoreInfo};
use oxiguitar_plugin::{Error, ErrorKind, Event, FormatReader, Host, Level, Plugin, Read};

/// The title of every score it reads.
pub const TITLE: &str = "Read by the second plugin";

/// The plugin.
#[derive(Debug)]
pub struct Second {
    host: Host,
}

impl Plugin for Second {
    fn start(host: Host) -> Result<Second, Error> {
        Ok(Second { host })
    }

    fn event(&self, event: &Event) {
        self.host.notify(Level::Warning, format!("heard {event:?}"));
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == "oxgtest").then_some(self as &dyn FormatReader)
    }
}

impl FormatReader for Second {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        if !data.starts_with(b"OXGTEST\n") {
            return Err(Error {
                offset: Some(0),
                ..Error::new(ErrorKind::Invalid, "not an oxgtest file")
            });
        }
        Ok(Read {
            score: Score {
                info: ScoreInfo {
                    title: TITLE.into(),
                    ..ScoreInfo::default()
                },
                initial_tempo: 120.0,
                ..Score::default()
            },
            log: Vec::new(),
        })
    }
}

oxiguitar_plugin::export_plugin!(Second);
