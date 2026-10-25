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
//! `id`, a service's `interface` and `id`, a tab's `id`.
//!
//! Interface 1.1 added tabs ([`Request::Tab`], [`Request::TabEvent`],
//! [`HostRequest::Redraw`]) and pushed events ([`Request::Event`],
//! [`HostRequest::Score`]). The host sends them only to plugins whose
//! manifest asks for interface 1.1 or later and declares the tab or the
//! event kind.
//!
//! Interface 1.2 added the score version 2 ([`Request::Write2`],
//! [`Answer::Read2`], [`HostRequest::Score2`]), tools ([`Request::Tool`],
//! [`Answer::Report`]) and edits ([`HostRequest::Apply`]). The host sends
//! a plugin built for 1.2 or later [`Request::Write2`] instead of
//! [`Request::Write`], and takes either answer to a [`Request::Read`].
//!
//! Interface 1.3 added settings windows ([`Request::SettingsWindow`],
//! [`Request::SettingsEvent`]), sections of the track dialog
//! ([`Request::TrackSection`], [`Request::TrackSectionEvent`],
//! [`Answer::TrackSection`]), footer messages with a link
//! ([`HostRequest::Notify2`]) and the plugin's folders
//! ([`HostRequest::Folders`]).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Blob;
use crate::abi::Status;
use crate::class::InstrumentClass;
use crate::event::Event;
use crate::manifest::SharedFolder;
use crate::score::Score;
use crate::score2::Score2;
use crate::tab::{TabContent, TabEvent};

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
    /// What the tab `tab` (an id from the manifest) shows now. Answered with
    /// [`Answer::Tab`]. Since interface 1.1.
    Tab {
        /// The tab's id.
        tab: String,
    },
    /// The user used the widget `widget` of the tab `tab`. Answered with
    /// [`Answer::Tab`], the tab's contents after it. Since interface 1.1.
    TabEvent {
        /// The tab's id.
        tab: String,
        /// The widget's id.
        widget: String,
        /// What the user did.
        event: TabEvent,
    },
    /// Something the plugin subscribed to happened. Answered with
    /// [`Answer::Done`]. Since interface 1.1.
    Event(Event),
    /// Write `score`, version 2, in the format `format`. Answered with
    /// [`Answer::Written`]. Sent instead of [`Request::Write`] to plugins
    /// built for interface 1.2 or later. Since interface 1.2.
    Write2 {
        /// The format's id.
        format: String,
        /// The score to write.
        score: Box<Score2>,
    },
    /// Run the tool `tool` (an id from the manifest's `tools`), which the
    /// user picked in the Tools menu. A tool that changes the score asks
    /// for it ([`HostRequest::Score2`]) and hands back its change
    /// ([`HostRequest::Apply`]). Answered with [`Answer::Report`]. Since
    /// interface 1.2.
    Tool {
        /// The tool's id.
        tool: String,
    },
    /// What the plugin's settings window shows now (the manifest's
    /// `settings_window`): asked when the user opens it from the plugin
    /// manager. Answered with [`Answer::Tab`]. The plugin keeps what the
    /// user sets with [`HostRequest::SetSettings`]. Since interface 1.3.
    SettingsWindow,
    /// The user used the widget `widget` of the settings window. Answered
    /// with [`Answer::Tab`], the window's contents after it. Since
    /// interface 1.3.
    SettingsEvent {
        /// The widget's id.
        widget: String,
        /// What the user did.
        event: TabEvent,
    },
    /// What the track dialog's section `section` (an id from the manifest's
    /// `track_sections`) shows for `track`, whose settings for this plugin
    /// are `settings` (a JSON object, `{}` at first). Answered with
    /// [`Answer::TrackSection`]. Since interface 1.3.
    TrackSection {
        /// The section's id.
        section: String,
        /// The track the dialog is open for.
        track: TrackInfo,
        /// The track's settings for this plugin, as JSON text.
        settings: String,
    },
    /// The user used the widget `widget` of the section `section` for
    /// `track`. Answered with [`Answer::TrackSection`]: the section's
    /// contents and the track's settings after it. The dialog keeps them
    /// until the user presses OK, which makes them one change the user can
    /// undo; Cancel drops them. Since interface 1.3.
    TrackSectionEvent {
        /// The section's id.
        section: String,
        /// The track the dialog is open for.
        track: TrackInfo,
        /// The track's settings for this plugin, as the dialog holds them.
        settings: String,
        /// The widget's id.
        widget: String,
        /// What the user did.
        event: TabEvent,
    },
}

/// A track, as the track dialog's sections are told about it. Since
/// interface 1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInfo {
    /// Its index in the score.
    pub index: u32,
    /// Its name.
    pub name: String,
    /// Its class of instrument.
    pub class: InstrumentClass,
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
    /// What a tab shows. Since interface 1.1.
    Tab(Box<TabContent>),
    /// Done. Since interface 1.1.
    Done,
    /// The score read from a file, version 2, and what couldn't be carried
    /// over. Since interface 1.2.
    Read2 {
        /// The score.
        score: Box<Score2>,
        /// What the reader couldn't carry over faithfully (FMT-2).
        log: Vec<LogEntry>,
    },
    /// What a tool did, in a sentence for the footer ("Transposed 3
    /// tracks up a tone."); empty to say nothing. Since interface 1.2.
    Report(String),
    /// What a section of the track dialog shows, and the track's settings
    /// for this plugin. Since interface 1.3.
    TrackSection {
        /// The section's contents.
        content: Box<TabContent>,
        /// The track's settings for this plugin, a JSON object as text.
        settings: String,
    },
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
            Answer::Tab(t) => Answer::Tab(t),
            Answer::Done => Answer::Done,
            Answer::Read2 { score, log } => Answer::Read2 { score, log },
            Answer::Report(r) => Answer::Report(r),
            Answer::TrackSection { content, settings } => {
                Answer::TrackSection { content, settings }
            }
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
    /// Show the tab `tab` (an id from the manifest) again: the host asks
    /// for its contents ([`Request::Tab`]) when it is next on screen.
    /// Answered with [`HostAnswer::Done`]. Since interface 1.1.
    Redraw {
        /// The tab's id.
        tab: String,
    },
    /// The open score, as plugins see it. Answered with
    /// [`HostAnswer::Score`]. Since interface 1.1.
    Score,
    /// The open score, version 2, and its revision. Answered with
    /// [`HostAnswer::Score2`]. Since interface 1.2.
    Score2,
    /// Change the open score to `score`, as one change the user can undo,
    /// named `name` in the Edit menu ("Undo Transpose"). `revision` is the
    /// revision the change was made from ([`HostAnswer::Score2`]): if the
    /// score has changed since, the host refuses the change with an error,
    /// and the plugin may ask for the score again and redo it. Answered
    /// with [`HostAnswer::Done`] once the change is accepted; it shows a
    /// moment later. Since interface 1.2.
    Apply {
        /// The revision the change was made from.
        revision: u64,
        /// The change's name, a few words.
        name: String,
        /// The whole score, changed.
        score: Box<Score2>,
    },
    /// Show a message in the footer, with a link the user can click to
    /// open in the system's browser (an `https:` address; others are
    /// refused). Answered with [`HostAnswer::Done`]. Since interface 1.3.
    Notify2 {
        /// How much it matters. The footer shows only warnings and errors.
        level: Level,
        /// One or two sentences.
        text: String,
        /// The link, after the text.
        link: Link,
    },
    /// The plugin's folders: its own data and cache folders and the shared
    /// folders its manifest lists. Answered with [`HostAnswer::Folders`].
    /// Since interface 1.3.
    Folders,
}

/// A link in a footer message. Since interface 1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// What the link says: a few words ("Better sounds").
    pub text: String,
    /// Where it goes: an `https:` address.
    pub url: String,
}

/// A plugin's folders (paths as UTF-8 text). Since interface 1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFolders {
    /// The plugin's own data folder, which only it uses: kept until the
    /// user deletes it. Created when this is asked for.
    pub data: String,
    /// The plugin's own cache folder, in the system's place for caches:
    /// for what it can make again (a decoded sound set), which the system
    /// or the user may delete at any time. Created when this is asked for.
    pub cache: String,
    /// The shared folders the manifest lists, each with its path, in the
    /// manifest's order. The user puts files there; the host creates them
    /// at start.
    pub shared: Vec<(SharedFolder, String)>,
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
    /// The open score, or `None` when none is open. Since interface 1.1.
    Score(Option<Box<Score>>),
    /// The open score, version 2, or `None` when none is open, with its
    /// revision (as score events give it). Since interface 1.2.
    Score2 {
        /// The score's revision.
        revision: u64,
        /// The score.
        score: Option<Box<Score2>>,
    },
    /// The plugin's folders. Since interface 1.3.
    Folders(PluginFolders),
}

impl HostAnswer<'_> {
    /// A copy that owns all its bytes.
    pub fn into_owned(self) -> HostAnswer<'static> {
        match self {
            HostAnswer::Done => HostAnswer::Done,
            HostAnswer::Settings(s) => HostAnswer::Settings(s),
            HostAnswer::Service(b) => HostAnswer::Service(b.into_owned()),
            HostAnswer::Score(s) => HostAnswer::Score(s),
            HostAnswer::Score2 { revision, score } => HostAnswer::Score2 { revision, score },
            HostAnswer::Folders(f) => HostAnswer::Folders(f),
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
    fn interface_1_3_messages_round_trip() {
        let track = TrackInfo {
            index: 2,
            name: "Lead".into(),
            class: InstrumentClass::Fretted,
        };
        let requests = [
            Request::SettingsWindow,
            Request::SettingsEvent {
                widget: "on".into(),
                event: TabEvent::Toggled(true),
            },
            Request::TrackSection {
                section: "amp".into(),
                track: track.clone(),
                settings: "{}".into(),
            },
            Request::TrackSectionEvent {
                section: "amp".into(),
                track,
                settings: r#"{"level":0.5}"#.into(),
                widget: "level".into(),
                event: TabEvent::Slid(0.25),
            },
        ];
        for request in requests {
            let bytes = encode(&request).unwrap();
            assert_eq!(decode::<Request<'_>>(&bytes).unwrap(), request);
        }
        let answer = Answer::TrackSection {
            content: Box::default(),
            settings: "{}".into(),
        };
        assert_eq!(
            decode::<Answer<'_>>(&encode(&answer).unwrap()).unwrap(),
            answer
        );
        let host = [
            HostRequest::Notify2 {
                level: Level::Warning,
                text: "Only a basic sound set plays.".into(),
                link: Link {
                    text: "Better sounds".into(),
                    url: "https://example.org/sounds".into(),
                },
            },
            HostRequest::Folders,
        ];
        for request in host {
            let bytes = encode(&request).unwrap();
            assert_eq!(decode::<HostRequest<'_>>(&bytes).unwrap(), request);
        }
        let folders = HostAnswer::Folders(PluginFolders {
            data: "/d".into(),
            cache: "/c".into(),
            shared: vec![(SharedFolder::Soundfonts, "/s".into())],
        });
        assert_eq!(
            decode::<HostAnswer<'_>>(&encode(&folders).unwrap()).unwrap(),
            folders
        );
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

        // Interface 1.1, at the end of each enum.
        assert_eq!(first(encode(&Request::Tab { tab: "t".into() }).unwrap()), 4);
        let event = Request::TabEvent {
            tab: "t".into(),
            widget: "w".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(first(encode(&event).unwrap()), 5);
        assert_eq!(
            first(encode(&Request::Event(Event::ScoreClosed)).unwrap()),
            6
        );
        assert_eq!(first(encode(&Answer::Tab(Box::default())).unwrap()), 4);
        assert_eq!(first(encode(&Answer::Done).unwrap()), 5);
        assert_eq!(
            first(encode(&HostRequest::Redraw { tab: "t".into() }).unwrap()),
            4
        );
        assert_eq!(first(encode(&HostRequest::Score).unwrap()), 5);
        assert_eq!(first(encode(&HostAnswer::Score(None)).unwrap()), 3);

        // Interface 1.2.
        let write2 = Request::Write2 {
            format: "gp".into(),
            score: Box::default(),
        };
        assert_eq!(first(encode(&write2).unwrap()), 7);
        assert_eq!(
            first(encode(&Request::Tool { tool: "t".into() }).unwrap()),
            8
        );
        let read2 = Answer::Read2 {
            score: Box::default(),
            log: vec![],
        };
        assert_eq!(first(encode(&read2).unwrap()), 6);
        assert_eq!(first(encode(&Answer::Report(String::new())).unwrap()), 7);
        assert_eq!(first(encode(&HostRequest::Score2).unwrap()), 6);
        let apply = HostRequest::Apply {
            revision: 3,
            name: "Transpose".into(),
            score: Box::default(),
        };
        assert_eq!(first(encode(&apply).unwrap()), 7);
        let score2 = HostAnswer::Score2 {
            revision: 1,
            score: None,
        };
        assert_eq!(first(encode(&score2).unwrap()), 4);
    }
}
