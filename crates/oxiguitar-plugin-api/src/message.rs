// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The messages that cross the C tables, version 1.0.
//!
//! The host sends a plugin [`Request`]s and gets [`Answer`]s (or an
//! [`Error`]); a plugin sends the host [`HostRequest`]s and gets
//! [`HostAnswer`]s. Everything is encoded with postcard ([`encode`],
//! [`decode`]): compact, fast and stable, with no schema in the bytes, so
//! the rules in the crate's "Versions" section keep old and new sides
//! compatible. New variants go at the end of each enum.
//!
//! Contributions are named by the ids in the plugin's manifest: a format's
//! `id`, a service's `interface` and `id`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Blob;
use crate::abi::Status;
use crate::score::Score;

/// Encodes a message.
pub fn encode<T: Serialize + ?Sized>(message: &T) -> Result<Vec<u8>, DecodeError> {
    postcard::to_allocvec(message).map_err(|e| DecodeError(e.to_string()))
}

/// Decodes a message, borrowing [`Blob`]s from `bytes`. Bytes left over
/// after the message are an error: they mean the two sides disagree about
/// the message's shape.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, DecodeError> {
    match postcard::take_from_bytes(bytes) {
        Ok((message, [])) => Ok(message),
        Ok((_, rest)) => Err(DecodeError(format!(
            "{} bytes left over after the message",
            rest.len()
        ))),
        Err(e) => Err(DecodeError(e.to_string())),
    }
}

/// A message that couldn't be encoded or decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed message: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

/// What a call's answer means, from its [`Status`]: the answer message's
/// bytes on success, the error otherwise. A panic, a malformed request or
/// a plugin that wasn't started become [`ErrorKind::Failed`] errors whose
/// message says so.
pub fn outcome(status: Status, answer: &[u8]) -> Result<&[u8], Error> {
    let text = || String::from_utf8_lossy(answer).into_owned();
    match status {
        Status::OK => Ok(answer),
        Status::FAILED => Err(decode::<Error>(answer).unwrap_or_else(|e| {
            Error::new(
                ErrorKind::Failed,
                format!("an error came back unreadable ({e})"),
            )
        })),
        Status::PANICKED => Err(Error::new(
            ErrorKind::Failed,
            format!("it crashed (a panic: {})", text()),
        )),
        Status::MALFORMED => Err(Error::new(
            ErrorKind::Failed,
            format!("it couldn't read the request ({})", text()),
        )),
        Status::NOT_STARTED => Err(Error::new(
            ErrorKind::Failed,
            "it was called before it started",
        )),
        Status(other) => Err(Error::new(
            ErrorKind::Failed,
            format!("it answered with the unknown status {other}"),
        )),
    }
}

/// What the host asks a plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Request<'a> {
    /// Read a file of the format `format` (a format id from the manifest).
    /// Answered with [`Answer::Read`].
    Read {
        /// The format's id.
        format: String,
        /// The file's contents.
        #[serde(borrow)]
        data: Blob<'a>,
    },
    /// Write `score` in the format `format`. Answered with
    /// [`Answer::Written`].
    Write {
        /// The format's id.
        format: String,
        /// The score to write.
        score: Box<Score>,
    },
    /// Say whether a service can work here (a sound server is running, a
    /// device is present). Answered with [`Answer::Probe`]. The host asks
    /// before the first use, and tries the next service on
    /// [`Availability::Unavailable`].
    Probe {
        /// The service's interface.
        interface: String,
        /// The service's id.
        id: String,
    },
    /// Call a service. The interface defines the methods and what their
    /// input and output bytes hold. Answered with [`Answer::Service`].
    Service {
        /// The service's interface.
        interface: String,
        /// The service's id.
        id: String,
        /// The method's name.
        method: String,
        /// The method's input, as the interface defines it.
        #[serde(borrow)]
        input: Blob<'a>,
    },
}

/// A plugin's answer to a [`Request`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer<'a> {
    /// The score read from a file, and what couldn't be carried over.
    Read {
        /// The score.
        score: Box<Score>,
        /// What the reader couldn't carry over faithfully (FMT-2).
        log: Vec<LogEntry>,
    },
    /// The written file, and what the format couldn't hold.
    Written {
        /// The file's contents.
        #[serde(borrow)]
        data: Blob<'a>,
        /// What the format can't hold and was left out (FMT-3).
        losses: Vec<LogEntry>,
    },
    /// Whether a service can work here.
    Probe(Availability),
    /// A service method's output.
    Service(#[serde(borrow)] Blob<'a>),
}

impl Answer<'_> {
    /// A copy that owns all its bytes.
    pub fn into_owned(self) -> Answer<'static> {
        match self {
            Answer::Read { score, log } => Answer::Read { score, log },
            Answer::Written { data, losses } => Answer::Written {
                data: data.into_owned(),
                losses,
            },
            Answer::Probe(a) => Answer::Probe(a),
            Answer::Service(b) => Answer::Service(b.into_owned()),
        }
    }
}

/// Whether a service can work here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    /// It can.
    Available,
    /// It can't; the text says why, for the plugin manager and the footer.
    Unavailable(String),
}

/// Why a call failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    /// What kind of failure.
    pub kind: ErrorKind,
    /// One sentence for the user.
    pub message: String,
    /// The byte offset in the input where it was found, if known.
    pub offset: Option<u64>,
    /// What was being done at the time, such as "bar 12, track 2".
    pub context: String,
}

impl Error {
    /// An error of `kind` with `message`.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Error {
        Error {
            kind,
            message: message.into(),
            offset: None,
            context: String::new(),
        }
    }

    /// The plugin has no contribution of that name.
    pub fn not_found(what: impl Into<String>) -> Error {
        Error::new(ErrorKind::NotFound, what)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if !self.context.is_empty() {
            write!(f, " (in {})", self.context)?;
        }
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// The kinds of [`Error`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    /// The plugin has no contribution of the requested id, or doesn't
    /// implement the requested operation for it.
    NotFound,
    /// The input is of a kind or version this plugin can't handle.
    Unsupported,
    /// The input is damaged or breaks the format's rules.
    Invalid,
    /// The input ends before the data it promises.
    UnexpectedEnd,
    /// Anything else went wrong.
    Failed,
}

/// How serious a log entry or a footer message is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Worth knowing; nothing was lost.
    Info,
    /// Something was lost or changed, or doesn't work until the user acts.
    Warning,
    /// Something went wrong.
    Error,
}

/// One entry of an import log or a list of losses. Repeats of the same
/// message are counted, not repeated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// How serious it is.
    pub level: Level,
    /// What happened.
    pub message: String,
    /// The first place it happened, such as "bar 3, track 1".
    pub place: String,
    /// How many times it happened.
    pub count: u32,
}

/// What a plugin asks the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HostRequest<'a> {
    /// Show a message in the footer. Answered with [`HostAnswer::Done`].
    Notify {
        /// How much it matters.
        level: Level,
        /// One or two sentences.
        text: String,
    },
    /// The plugin's own settings, the object under its id in
    /// settings.json, as JSON text (`{}` when there are none). Answered
    /// with [`HostAnswer::Settings`].
    Settings,
    /// Replace the plugin's own settings with `json` (a JSON object), and
    /// save settings.json. Answered with [`HostAnswer::Done`].
    SetSettings {
        /// The new settings.
        json: String,
    },
    /// Call the service the host chose for `interface` (declared under
    /// `requires` in the manifest). Answered with [`HostAnswer::Service`].
    Service {
        /// The service's interface.
        interface: String,
        /// The method's name.
        method: String,
        /// The method's input.
        #[serde(borrow)]
        input: Blob<'a>,
    },
}

/// The host's answer to a [`HostRequest`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HostAnswer<'a> {
    /// Done.
    Done,
    /// The plugin's settings, as JSON text.
    Settings(String),
    /// A service method's output.
    Service(#[serde(borrow)] Blob<'a>),
}

impl HostAnswer<'_> {
    /// A copy that owns all its bytes.
    pub fn into_owned(self) -> HostAnswer<'static> {
        match self {
            HostAnswer::Done => HostAnswer::Done,
            HostAnswer::Settings(s) => HostAnswer::Settings(s),
            HostAnswer::Service(b) => HostAnswer::Service(b.into_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let data = b"FICHIER GUITAR PRO v5.10".to_vec();
        let request = Request::Read {
            format: "gp5".into(),
            data: Blob::borrowed(&data),
        };
        let bytes = encode(&request).unwrap();
        assert_eq!(decode::<Request<'_>>(&bytes).unwrap(), request);

        let error = Error {
            offset: Some(31),
            context: "bar 2".into(),
            ..Error::new(ErrorKind::UnexpectedEnd, "the file ends too early")
        };
        assert_eq!(
            error.to_string(),
            "the file ends too early (in bar 2) at byte 31"
        );
        let bytes = encode(&error).unwrap();
        assert_eq!(decode::<Error>(&bytes).unwrap(), error);
    }

    #[test]
    fn left_over_bytes_are_an_error() {
        let mut bytes = encode(&HostRequest::Settings).unwrap();
        bytes.push(0);
        assert!(decode::<HostRequest<'_>>(&bytes).is_err());
        assert!(decode::<HostRequest<'_>>(&[200]).is_err());
    }

    #[test]
    fn outcomes() {
        assert_eq!(outcome(Status::OK, b"x"), Ok(&b"x"[..]));
        let e = Error::not_found("no format \"x\"");
        assert_eq!(outcome(Status::FAILED, &encode(&e).unwrap()), Err(e));
        let crash = outcome(Status::PANICKED, b"index out of bounds").unwrap_err();
        assert_eq!(crash.message, "it crashed (a panic: index out of bounds)");
        assert_eq!(
            outcome(Status::FAILED, &[0xff]).unwrap_err().kind,
            ErrorKind::Failed
        );
        assert!(outcome(Status(77), b"").unwrap_err().message.contains("77"));
    }

    /// The encoding of each enum's variants is its position. This test
    /// pins it, so a variant inserted anywhere but at the end fails here
    /// instead of breaking built plugins.
    #[test]
    fn variant_positions_are_frozen() {
        let first = |b: Vec<u8>| b[0];
        assert_eq!(first(encode(&HostRequest::Settings).unwrap()), 1);
        assert_eq!(
            first(encode(&HostRequest::SetSettings { json: "{}".into() }).unwrap()),
            2
        );
        let probe = Request::Probe {
            interface: "a".into(),
            id: "b".into(),
        };
        assert_eq!(first(encode(&probe).unwrap()), 2);
        assert_eq!(
            first(encode(&Answer::Probe(Availability::Available)).unwrap()),
            2
        );
        assert_eq!(first(encode(&ErrorKind::Failed).unwrap()), 4);
        assert_eq!(first(encode(&Level::Error).unwrap()), 2);
    }
}
