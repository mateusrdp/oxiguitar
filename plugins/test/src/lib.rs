// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A plugin for testing the plugin interface (never shipped).
//!
//! The contract suite (`oxiguitar-plugin-test`) builds it as `test.oxp`
//! and drives every part of the interface through it:
//!
//! - the format `oxgtest`: a text file starting `OXGTEST`, whose second
//!   line is the score's title; it reads and writes;
//! - the service interface `test-echo`, with four services: `echo`
//!   (available, priority 1), `second` (available, priority 0, so chosen
//!   only when settings.json pins it), `unavailable` (its probe says no,
//!   though its priority is the highest that can run) and `elsewhere`
//!   (only on Plan 9, so never chosen);
//! - two instrument declarations, one playing only drums and one
//!   everything but drums (the manifest alone: instruments get their
//!   interface with the player, M4).
//!
//! `echo`'s methods exercise the host's side: `notify` and `settings` call
//! the host's services, `via-host` calls `test-echo` back through the host
//! (a plugin calling a service, here itself, while answering a call), and
//! `panic` and `fail` check that both cross the interface as errors.

use oxiguitar_plugin::score::{Score, ScoreInfo};
use oxiguitar_plugin::{
    Availability, Error, ErrorKind, FormatReader, FormatWriter, Host, Level, LogEntry, Plugin,
    Read, Service, Written,
};

/// The first line of every `oxgtest` file.
pub const MAGIC: &str = "OXGTEST\n";

/// The test plugin.
#[derive(Debug)]
pub struct TestPlugin {
    host: Host,
}

/// The service that is never available.
#[derive(Debug)]
struct Unavailable;

impl Plugin for TestPlugin {
    fn start(host: Host) -> Result<TestPlugin, Error> {
        Ok(TestPlugin { host })
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == "oxgtest").then_some(self as &dyn FormatReader)
    }

    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        (format == "oxgtest").then_some(self as &dyn FormatWriter)
    }

    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        match (interface, id) {
            ("test-echo", "echo" | "second" | "elsewhere") => Some(self),
            ("test-echo", "unavailable") => Some(&Unavailable),
            _ => None,
        }
    }
}

impl FormatReader for TestPlugin {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        let rest = data.strip_prefix(MAGIC.as_bytes()).ok_or_else(|| Error {
            offset: Some(0),
            ..Error::new(ErrorKind::Invalid, "not an oxgtest file")
        })?;
        let text = std::str::from_utf8(rest).map_err(|e| Error {
            offset: Some((MAGIC.len() + e.valid_up_to()) as u64),
            ..Error::new(ErrorKind::Invalid, "the title is not UTF-8 text")
        })?;
        let title = text.lines().next().ok_or_else(|| Error {
            offset: Some(MAGIC.len() as u64),
            ..Error::new(ErrorKind::UnexpectedEnd, "the file ends before the title")
        })?;
        let score = Score {
            info: ScoreInfo {
                title: title.to_owned(),
                ..ScoreInfo::default()
            },
            initial_tempo: 120.0,
            ..Score::default()
        };
        let extra = text.lines().count().saturating_sub(1);
        let log = (extra > 0)
            .then(|| LogEntry {
                level: Level::Info,
                message: "lines after the title are ignored".into(),
                place: "line 3".into(),
                count: extra as u32,
            })
            .into_iter()
            .collect();
        Ok(Read { score, log })
    }
}

impl FormatWriter for TestPlugin {
    fn write(&self, score: &Score) -> Result<Written, Error> {
        let data = format!("{MAGIC}{}\n", score.info.title).into_bytes();
        let losses = (!score.tracks.is_empty())
            .then(|| LogEntry {
                level: Level::Warning,
                message: "the oxgtest format holds only the title".into(),
                place: String::new(),
                count: score.tracks.len() as u32,
            })
            .into_iter()
            .collect();
        Ok(Written { data, losses })
    }
}

impl Service for TestPlugin {
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        match method {
            "echo" => Ok(input.to_vec()),
            "notify" => {
                self.host
                    .notify(Level::Info, String::from_utf8_lossy(input).into_owned());
                Ok(Vec::new())
            }
            "settings" => {
                let mut settings = self.host.settings()?;
                let count = settings.get("count").and_then(|c| c.as_u64()).unwrap_or(0) + 1;
                settings.insert("count".into(), count.into());
                self.host.set_settings(&settings)?;
                Ok(count.to_string().into_bytes())
            }
            "via-host" => self.host.call_service("test-echo", "echo", input),
            "fail" => Err(Error {
                offset: Some(7),
                context: "the test".into(),
                ..Error::new(ErrorKind::Invalid, "failed on purpose")
            }),
            "panic" => panic!("panicked on purpose"),
            _ => Err(Error::not_found(format!("no method {method:?}"))),
        }
    }
}

impl Service for Unavailable {
    fn probe(&self) -> Availability {
        Availability::Unavailable("it is never available (a test)".into())
    }

    fn call(&self, _method: &str, _input: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::new(ErrorKind::Failed, "it is never available"))
    }
}

oxiguitar_plugin::export_plugin!(TestPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_its_format() {
        let plugin = TestPlugin::start(Host::detached()).unwrap();
        let read = plugin.read(b"OXGTEST\nHello\nmore\n").unwrap();
        assert_eq!(read.score.info.title, "Hello");
        assert_eq!(read.log.len(), 1);
        let written = plugin.write(&read.score).unwrap();
        assert_eq!(written.data, b"OXGTEST\nHello\n");
        assert!(written.losses.is_empty());
        let e = plugin.read(b"GP5").unwrap_err();
        assert_eq!((e.kind, e.offset), (ErrorKind::Invalid, Some(0)));
        let e = plugin.read(b"OXGTEST\n").unwrap_err();
        assert_eq!(e.kind, ErrorKind::UnexpectedEnd);
    }
}
