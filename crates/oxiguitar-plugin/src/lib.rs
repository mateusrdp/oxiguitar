// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The SDK for OxiGuitar plugins (docs/design/plugins.md).
//!
//! A plugin is a Rust library crate that:
//!
//! 1. writes its manifest as `manifest.json` beside its `Cargo.toml`
//!    ([`api::manifest`] describes the keys);
//! 2. implements [`Plugin`], handing out a [`FormatReader`],
//!    [`FormatWriter`], [`Service`] or [`Tab`] for each contribution its
//!    manifest declares, and handling the [`Event`]s it subscribes to;
//! 3. calls [`export_plugin!`] once, in its `lib.rs`;
//! 4. has a Cargo feature named `oxp` (`oxp = []`), which the `.oxp` build
//!    turns on.
//!
//! ```ignore
//! use oxiguitar_plugin::{Error, FormatReader, Host, Plugin, Read};
//!
//! pub struct Gp5;
//!
//! impl Plugin for Gp5 {
//!     fn start(_host: Host) -> Result<Gp5, Error> {
//!         Ok(Gp5)
//!     }
//!     fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
//!         (format == "gp5").then_some(self as &dyn FormatReader)
//!     }
//! }
//!
//! impl FormatReader for Gp5 {
//!     fn read(&self, data: &[u8]) -> Result<Read, Error> {
//!         todo!()
//!     }
//! }
//!
//! oxiguitar_plugin::export_plugin!(Gp5);
//! ```
//!
//! Build the `.oxp` file with
//! `cargo rustc --release -p NAME --features oxp --crate-type cdylib`, then
//! copy the library (`libNAME.so`, `libNAME.dylib` or `NAME.dll`) to
//! `NAME.oxp`. The `oxiguitar-plugin-test` tool does both and checks the
//! result against the interface.
//!
//! Plugin authors write no `unsafe` code. `export_plugin!` expands to the
//! entry symbol and the manifest section (two `unsafe` attributes), and the
//! SDK's export glue does the rest; both are reviewed line by line. Without
//! the `oxp` feature the crate is an ordinary library: tests and apps that
//! build plugins in (the mobile apps) call [`Plugin`] directly, or through
//! [`dispatch`] with messages.
//!
//! Every call may come from any thread, several at once, so a plugin is
//! `Send + Sync`. A panic inside a call is caught (build with the default
//! `panic = "unwind"`): OxiGuitar then withdraws the plugin for the rest
//! of the session and carries on as if it weren't installed.

pub use oxiguitar_plugin_api as api;
pub use oxiguitar_plugin_api::class::InstrumentClass;
pub use oxiguitar_plugin_api::display;
pub use oxiguitar_plugin_api::event::{Event, Playback, Position, Selection, Selection2};
pub use oxiguitar_plugin_api::manifest::SharedFolder;
pub use oxiguitar_plugin_api::message::{
    Availability, Error, ErrorKind, Level, LogEntry, TrackInfo,
};
pub use oxiguitar_plugin_api::score;
pub use oxiguitar_plugin_api::score2;
pub use oxiguitar_plugin_api::tab::{
    Canvas, Pointer, PointerButton, PointerKind, Row, TabContent, TabEvent, Widget,
};
/// serde_json, as the SDK uses it: plugin settings and track sections'
/// settings are its JSON objects, so a plugin needs no dependency of its
/// own (interface 1.3).
pub use serde_json;

use std::sync::Arc;

use oxiguitar_plugin_api::Blob;
use oxiguitar_plugin_api::manifest::Manifest;
use oxiguitar_plugin_api::message::{Answer, HostAnswer, HostRequest, Link, Request};

#[doc(hidden)]
pub mod audio;
pub mod glue;

pub use audio::{Effect, Instrument, Piece, Processor, Renderer};

/// A plugin: the one value an `.oxp` file holds. Built once when the host
/// first needs one of its contributions, and kept until the program ends.
pub trait Plugin: Send + Sync + 'static {
    /// Builds the plugin. `host` reaches the core's services; keep it if the
    /// plugin needs them later. An error here means the plugin can't work;
    /// the host says so in the footer and never calls it again.
    fn start(host: Host) -> Result<Self, Error>
    where
        Self: Sized;

    /// The reader for the format `format` (an id from the manifest).
    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        let _ = format;
        None
    }

    /// The writer for the format `format`.
    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        let _ = format;
        None
    }

    /// The service `id` of the interface `interface`.
    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        let _ = (interface, id);
        None
    }

    /// The tab `id` (an id from the manifest's `tabs`).
    fn tab(&self, id: &str) -> Option<&dyn Tab> {
        let _ = id;
        None
    }

    /// Something the manifest's `events` subscribe to happened. Called on
    /// any thread, as it happens; keep it short, and call
    /// [`Host::redraw`] for a tab that should show it. The default does
    /// nothing.
    fn event(&self, event: &Event) {
        let _ = event;
    }

    /// The reader for the format `format` into the score version 2
    /// (interface 1.2). When a plugin has both, this one is used.
    fn reader2(&self, format: &str) -> Option<&dyn FormatReader2> {
        let _ = format;
        None
    }

    /// The writer for the format `format` from the score version 2
    /// (interface 1.2). When a plugin has both, this one is used.
    fn writer2(&self, format: &str) -> Option<&dyn FormatWriter2> {
        let _ = format;
        None
    }

    /// The tool `id` (an id from the manifest's `tools`; interface 1.2).
    fn tool(&self, id: &str) -> Option<&dyn Tool> {
        let _ = id;
        None
    }

    /// The settings window, for a manifest with `settings_window`
    /// (interface 1.3): standard widgets, as a tab has, in a window the
    /// plugin manager opens. Keep what the user sets with
    /// [`Host::set_settings`].
    fn settings_window(&self) -> Option<&dyn Tab> {
        None
    }

    /// The track dialog's section `id` (an id from the manifest's
    /// `track_sections`; interface 1.3).
    fn track_section(&self, id: &str) -> Option<&dyn TrackSection> {
        let _ = id;
        None
    }

    /// The instrument `id` (an id from the manifest's `instruments`;
    /// interface 1.3).
    fn instrument(&self, id: &str) -> Option<&dyn audio::Instrument> {
        let _ = id;
        None
    }

    /// The effect `id` (an id from the manifest's `effects`; interface
    /// 1.3).
    fn effect(&self, id: &str) -> Option<&dyn audio::Effect> {
        let _ = id;
        None
    }
}

/// A section of the track dialog (interface 1.3): rows of widgets for one
/// track, whose values the host keeps per track and saves beside the score
/// (`song.gp.oxg`). The plugin keeps nothing itself: every call gets the
/// track's settings, a JSON object (empty at first).
pub trait TrackSection: Send + Sync {
    /// What the section shows for `track` with `settings`.
    fn content(
        &self,
        track: &TrackInfo,
        settings: &serde_json::Map<String, serde_json::Value>,
    ) -> TabContent;

    /// The user used the widget `widget`: change `settings` to match. The
    /// contents are asked for again afterwards. The default does nothing.
    fn on(
        &self,
        track: &TrackInfo,
        settings: &mut serde_json::Map<String, serde_json::Value>,
        widget: &str,
        event: TabEvent,
    ) -> Result<(), Error> {
        let _ = (track, settings, widget, event);
        Ok(())
    }
}

/// A plugin's folders (interface 1.3; [`Host::folders`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folders {
    /// Its own data folder, kept until the user deletes it.
    pub data: std::path::PathBuf,
    /// Its own cache folder, for what it can make again; the system or the
    /// user may empty it at any time.
    pub cache: std::path::PathBuf,
    /// The shared folders its manifest lists, in the manifest's order.
    pub shared: Vec<(SharedFolder, std::path::PathBuf)>,
}

impl Folders {
    /// The shared folder `which`, if the manifest lists it.
    pub fn shared(&self, which: SharedFolder) -> Option<&std::path::Path> {
        self.shared
            .iter()
            .find(|(f, _)| *f == which)
            .map(|(_, p)| p.as_path())
    }
}

/// Reads one file format into the score version 2 (interface 1.2).
pub trait FormatReader2: Send + Sync {
    /// Reads `data`, a whole file. Treat it as untrusted: a damaged file
    /// gives an error saying where, never a panic (FMT-1).
    fn read(&self, data: &[u8]) -> Result<Read2, Error>;
}

/// What a [`FormatReader2`] read.
#[derive(Clone, Debug, PartialEq)]
pub struct Read2 {
    /// The score.
    pub score: score2::Score2,
    /// What couldn't be carried over faithfully (FMT-2).
    pub log: Vec<LogEntry>,
}

/// Writes the score version 2 in one file format (interface 1.2).
pub trait FormatWriter2: Send + Sync {
    /// Writes `score` as a whole file.
    fn write(&self, score: &score2::Score2) -> Result<Written, Error>;
}

/// A Tools menu entry (interface 1.2). A tool that changes the score asks
/// for it ([`Host::score2`]) and hands back its change ([`Host::apply`]),
/// which the user can undo as one step.
pub trait Tool: Send + Sync {
    /// Runs the tool, which the user picked in the Tools menu. Returns a
    /// sentence for the footer about what it did (or an empty one).
    fn run(&self) -> Result<String, Error>;
}

/// A tab in one of the window's areas (docs/design/plugins.md, "What
/// plugins contribute"): standard widgets the core draws, and a canvas the
/// plugin draws.
pub trait Tab: Send + Sync {
    /// What the tab shows now. The core asks when the tab is shown, after
    /// every [`Tab::on`], and after [`Host::redraw`].
    fn content(&self) -> TabContent;

    /// The user used the widget `widget` (its id). The tab's contents are
    /// asked for again afterwards. The default does nothing.
    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let _ = (widget, event);
        Ok(())
    }
}

/// Reads one file format into a score.
pub trait FormatReader: Send + Sync {
    /// Reads `data`, a whole file. Treat it as untrusted: a damaged file
    /// gives an error saying where, never a panic (FMT-1).
    fn read(&self, data: &[u8]) -> Result<Read, Error>;
}

/// What a [`FormatReader`] read.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    /// The score.
    pub score: score::Score,
    /// What couldn't be carried over faithfully (FMT-2).
    pub log: Vec<LogEntry>,
}

/// Writes a score in one file format.
pub trait FormatWriter: Send + Sync {
    /// Writes `score` as a whole file.
    fn write(&self, score: &score::Score) -> Result<Written, Error>;
}

/// What a [`FormatWriter`] wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    /// The file's contents.
    pub data: Vec<u8>,
    /// What the format can't hold and was left out (FMT-3).
    pub losses: Vec<LogEntry>,
}

/// One implementation of a service interface.
pub trait Service: Send + Sync {
    /// Whether the service can work here. The host asks once, before the
    /// first use; the default says yes.
    fn probe(&self) -> Availability {
        Availability::Available
    }

    /// Calls the method `method` with `input`; the interface defines both,
    /// and the output.
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error>;
}

/// Answers one request with `plugin`: what the export glue does for every
/// call through the interface, and what a host that builds plugins in
/// calls directly.
pub fn dispatch(plugin: &dyn Plugin, request: Request<'_>) -> Result<Answer<'static>, Error> {
    dispatch_kept(plugin, request, &NoKeeper)
}

/// Keeps no instance: [`dispatch`] without the glue refuses to prepare.
struct NoKeeper;

impl audio::Keeper for NoKeeper {
    fn keep(&self, _instance: audio::Instance) -> u64 {
        0
    }

    fn release(&self, _id: u64) -> bool {
        false
    }
}

/// [`dispatch`], with prepared instruments and effects kept by `keeper`.
pub fn dispatch_kept(
    plugin: &dyn Plugin,
    request: Request<'_>,
    keeper: &dyn audio::Keeper,
) -> Result<Answer<'static>, Error> {
    match request {
        Request::Read { format, data } => {
            if let Some(reader) = plugin.reader2(&format) {
                let Read2 { score, log } = reader.read(data.as_slice())?;
                return Ok(Answer::Read2 {
                    score: Box::new(score),
                    log,
                });
            }
            let reader = plugin
                .reader(&format)
                .ok_or_else(|| Error::not_found(format!("this plugin doesn't read {format:?}")))?;
            let Read { score, log } = reader.read(data.as_slice())?;
            Ok(Answer::Read {
                score: Box::new(score),
                log,
            })
        }
        Request::Write { format, score } => {
            let Written { data, losses } = match plugin.writer(&format) {
                Some(writer) => writer.write(&score)?,
                None => writer2(plugin, &format)?.write(&score2::Score2::from_v1(*score))?,
            };
            Ok(Answer::Written {
                data: Blob::owned(data),
                losses,
            })
        }
        Request::Write2 { format, score } => {
            let Written { data, losses } =
                match plugin.writer2(&format) {
                    Some(writer) => writer.write(&score)?,
                    None => {
                        // A plugin with only a version-1 writer: what version 1
                        // can't hold is lost, and said so.
                        let writer = plugin.writer(&format).ok_or_else(|| {
                            Error::not_found(format!("this plugin doesn't write {format:?}"))
                        })?;
                        let mut written = writer.write(&score.to_v1())?;
                        written
                            .losses
                            .extend(score.extensions_used().into_iter().map(
                                |(what, bar, count)| LogEntry {
                                    level: Level::Warning,
                                    message: format!("{what} is left out by this plugin"),
                                    place: format!("bar {bar}"),
                                    count: u32::try_from(count).unwrap_or(u32::MAX),
                                },
                            ));
                        written
                    }
                };
            Ok(Answer::Written {
                data: Blob::owned(data),
                losses,
            })
        }
        Request::Tool { tool } => {
            let tool = plugin
                .tool(&tool)
                .ok_or_else(|| Error::not_found(format!("this plugin has no tool {tool:?}")))?;
            Ok(Answer::Report(tool.run()?))
        }
        Request::Probe { interface, id } => {
            let service = find_service(plugin, &interface, &id)?;
            Ok(Answer::Probe(service.probe()))
        }
        Request::Service {
            interface,
            id,
            method,
            input,
        } => {
            let service = find_service(plugin, &interface, &id)?;
            Ok(Answer::Service(Blob::owned(
                service.call(&method, input.as_slice())?,
            )))
        }
        Request::Tab { tab } => Ok(Answer::Tab(Box::new(find_tab(plugin, &tab)?.content()))),
        Request::TabEvent { tab, widget, event } => {
            let tab = find_tab(plugin, &tab)?;
            tab.on(&widget, event)?;
            Ok(Answer::Tab(Box::new(tab.content())))
        }
        Request::Event(event) => {
            plugin.event(&event);
            Ok(Answer::Done)
        }
        Request::SettingsWindow => Ok(Answer::Tab(Box::new(find_settings(plugin)?.content()))),
        Request::SettingsEvent { widget, event } => {
            let window = find_settings(plugin)?;
            window.on(&widget, event)?;
            Ok(Answer::Tab(Box::new(window.content())))
        }
        Request::TrackSection {
            section,
            track,
            settings,
        } => {
            let s = find_section(plugin, &section)?;
            let map = settings_object(&settings)?;
            Ok(Answer::TrackSection {
                content: Box::new(s.content(&track, &map)),
                settings,
            })
        }
        Request::TrackSectionEvent {
            section,
            track,
            settings,
            widget,
            event,
        } => {
            let s = find_section(plugin, &section)?;
            let mut map = settings_object(&settings)?;
            s.on(&track, &mut map, &widget, event)?;
            let content = s.content(&track, &map);
            Ok(Answer::TrackSection {
                content: Box::new(content),
                settings: serde_json::Value::Object(map).to_string(),
            })
        }
        Request::Prepare(p) => {
            let instrument = plugin.instrument(&p.instrument).ok_or_else(|| {
                Error::not_found(format!("this plugin has no instrument {:?}", p.instrument))
            })?;
            prepared(keeper, audio::Instance::Instrument(instrument.prepare(&p)?))
        }
        Request::PrepareEffect(p) => {
            let effect = plugin.effect(&p.effect).ok_or_else(|| {
                Error::not_found(format!("this plugin has no effect {:?}", p.effect))
            })?;
            prepared(keeper, audio::Instance::Effect(effect.prepare(&p)?))
        }
        Request::Release { instance } => {
            if keeper.release(instance) {
                Ok(Answer::Done)
            } else {
                Err(Error::not_found(format!("no instance {instance}")))
            }
        }
    }
}

fn prepared(
    keeper: &dyn audio::Keeper,
    instance: audio::Instance,
) -> Result<Answer<'static>, Error> {
    let (latency, resolution) = (instance.latency(), instance.resolution());
    match keeper.keep(instance) {
        0 => Err(Error::new(
            ErrorKind::Unsupported,
            "instruments and effects run only from the plugin's .oxp file",
        )),
        id => Ok(Answer::Prepared {
            instance: id,
            latency,
            resolution,
        }),
    }
}

fn find_settings(plugin: &dyn Plugin) -> Result<&dyn Tab, Error> {
    plugin
        .settings_window()
        .ok_or_else(|| Error::not_found("this plugin has no settings window"))
}

fn find_section<'p>(plugin: &'p dyn Plugin, id: &str) -> Result<&'p dyn TrackSection, Error> {
    plugin
        .track_section(id)
        .ok_or_else(|| Error::not_found(format!("this plugin has no track section {id:?}")))
}

fn settings_object(json: &str) -> Result<serde_json::Map<String, serde_json::Value>, Error> {
    match serde_json::from_str(json) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        _ => Err(Error::new(
            ErrorKind::Invalid,
            "a track's settings must be a JSON object",
        )),
    }
}

fn writer2<'p>(plugin: &'p dyn Plugin, format: &str) -> Result<&'p dyn FormatWriter2, Error> {
    plugin
        .writer2(format)
        .ok_or_else(|| Error::not_found(format!("this plugin doesn't write {format:?}")))
}

fn find_tab<'p>(plugin: &'p dyn Plugin, id: &str) -> Result<&'p dyn Tab, Error> {
    plugin
        .tab(id)
        .ok_or_else(|| Error::not_found(format!("this plugin has no tab {id:?}")))
}

fn find_service<'p>(
    plugin: &'p dyn Plugin,
    interface: &str,
    id: &str,
) -> Result<&'p dyn Service, Error> {
    plugin.service(interface, id).ok_or_else(|| {
        Error::not_found(format!(
            "this plugin has no service {id:?} of {interface:?}"
        ))
    })
}

/// The core's services, as a plugin reaches them.
#[derive(Clone)]
pub struct Host(Arc<dyn HostServices>);

/// Something that answers a plugin's requests: the core, through the
/// interface, or a test.
pub trait HostServices: Send + Sync {
    /// Answers one request.
    fn request(&self, request: HostRequest<'_>) -> Result<HostAnswer<'static>, Error>;

    /// Where an audio device's callback has the core fill its buffers
    /// (interface 1.3); none by default.
    fn audio_out(&self) -> Option<glue::AudioOut> {
        None
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Host")
    }
}

impl Host {
    /// A host answering through `services`.
    pub fn new(services: Arc<dyn HostServices>) -> Host {
        Host(services)
    }

    /// A host that shows no messages, keeps no settings and has no
    /// services: for tests that don't care.
    pub fn detached() -> Host {
        Host(Arc::new(Detached))
    }

    /// Shows a message in the footer. The footer holds errors and warnings
    /// only, so a `Level::Info` message isn't shown; a message that can't
    /// be shown is dropped.
    pub fn notify(&self, level: Level, text: impl Into<String>) {
        let _ = self.0.request(HostRequest::Notify {
            level,
            text: text.into(),
        });
    }

    /// The plugin's own settings: the object under its id in settings.json
    /// (empty when there is none).
    pub fn settings(&self) -> Result<serde_json::Map<String, serde_json::Value>, Error> {
        match self.0.request(HostRequest::Settings)? {
            HostAnswer::Settings(json) => match serde_json::from_str(&json) {
                Ok(serde_json::Value::Object(map)) => Ok(map),
                _ => Err(Error::new(
                    ErrorKind::Invalid,
                    "the host's settings are not a JSON object",
                )),
            },
            other => Err(unexpected(&other)),
        }
    }

    /// Replaces the plugin's own settings, and saves settings.json.
    pub fn set_settings(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        let json = serde_json::to_string(settings)
            .map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
        match self.0.request(HostRequest::SetSettings { json })? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Calls `method` of the service the host chose for `interface`, which
    /// the manifest lists under `requires`.
    pub fn call_service(
        &self,
        interface: &str,
        method: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        match self.0.request(HostRequest::Service {
            interface: interface.into(),
            method: method.into(),
            input: Blob::borrowed(input),
        })? {
            HostAnswer::Service(output) => Ok(output.into_vec()),
            other => Err(unexpected(&other)),
        }
    }

    /// Shows a message in the footer with a link the user can click, which
    /// opens `url` (an `https:` address) in the system's browser
    /// (interface 1.3). As with [`Host::notify`], only warnings and errors
    /// are shown, and a message that can't be shown is dropped.
    pub fn notify_with_link(
        &self,
        level: Level,
        text: impl Into<String>,
        link_text: impl Into<String>,
        url: impl Into<String>,
    ) {
        let _ = self.0.request(HostRequest::Notify2 {
            level,
            text: text.into(),
            link: Link {
                text: link_text.into(),
                url: url.into(),
            },
        });
    }

    /// The plugin's folders (interface 1.3): its own data and cache
    /// folders, created on first ask, and the shared folders its manifest
    /// lists.
    pub fn folders(&self) -> Result<Folders, Error> {
        match self.0.request(HostRequest::Folders)? {
            HostAnswer::Folders(f) => Ok(Folders {
                data: f.data.into(),
                cache: f.cache.into(),
                shared: f.shared.into_iter().map(|(w, p)| (w, p.into())).collect(),
            }),
            other => Err(unexpected(&other)),
        }
    }

    /// Where an audio device's callback has the core fill its buffers
    /// (interface 1.3; `oxiguitar_plugin::api::audio_device`), or `None`
    /// from a host without it.
    pub fn audio_out(&self) -> Option<glue::AudioOut> {
        self.0.audio_out()
    }

    /// Asks the core to show the tab `tab` (an id from the manifest) again:
    /// it asks for [`Tab::content`] when the tab is next on screen. A
    /// request that can't be made is dropped.
    pub fn redraw(&self, tab: &str) {
        let _ = self.0.request(HostRequest::Redraw { tab: tab.into() });
    }

    /// The open score, or `None` when none is open: what a plugin asks for
    /// after [`Event::ScoreOpened`] or [`Event::ScoreChanged`].
    pub fn score(&self) -> Result<Option<score::Score>, Error> {
        match self.0.request(HostRequest::Score)? {
            HostAnswer::Score(score) => Ok(score.map(|s| *s)),
            other => Err(unexpected(&other)),
        }
    }
}

impl Host {
    /// The open score, version 2, with its revision, or `None` when none is
    /// open (interface 1.2). Keep the revision for [`Host::apply`].
    pub fn score2(&self) -> Result<Option<(u64, score2::Score2)>, Error> {
        match self.0.request(HostRequest::Score2)? {
            HostAnswer::Score2 { revision, score } => Ok(score.map(|s| (revision, *s))),
            other => Err(unexpected(&other)),
        }
    }

    /// Hands the core a changed score (interface 1.2), made from the score
    /// of `revision` ([`Host::score2`]). The core records it as one change
    /// the user can undo, named `name` ("Transpose"), and shows it a moment
    /// later. If the score changed since `revision`, the change is refused
    /// with an error: ask for the score again and redo the change.
    pub fn apply(&self, revision: u64, name: &str, score: score2::Score2) -> Result<(), Error> {
        match self.0.request(HostRequest::Apply {
            revision,
            name: name.into(),
            score: Box::new(score),
        })? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }
}

fn unexpected(answer: &HostAnswer<'_>) -> Error {
    Error::new(
        ErrorKind::Failed,
        format!("the host answered unexpectedly ({answer:?})"),
    )
}

struct Detached;

impl HostServices for Detached {
    fn request(&self, request: HostRequest<'_>) -> Result<HostAnswer<'static>, Error> {
        match request {
            HostRequest::Notify { .. }
            | HostRequest::Notify2 { .. }
            | HostRequest::SetSettings { .. }
            | HostRequest::Redraw { .. } => Ok(HostAnswer::Done),
            HostRequest::Settings => Ok(HostAnswer::Settings("{}".into())),
            HostRequest::Score => Ok(HostAnswer::Score(None)),
            HostRequest::Score2 => Ok(HostAnswer::Score2 {
                revision: 0,
                score: None,
            }),
            HostRequest::Apply { .. } => Err(Error::new(
                ErrorKind::Failed,
                "a detached host has no score to change",
            )),
            HostRequest::Service { interface, .. } => Err(Error::not_found(format!(
                "no service of {interface:?} (a detached host has none)"
            ))),
            HostRequest::Folders => Err(Error::not_found("a detached host has no folders")),
        }
    }
}

/// Everything wrong with a plugin's manifest and the contributions it
/// declares, checked without the interface: what `export_plugin!`'s test
/// runs under `cargo test`, so mistakes show up before an `.oxp` is built.
///
/// `manifest_json` is the manifest's text and `crate_version` the crate's
/// version, which the manifest's version must equal.
pub fn check<P: Plugin>(manifest_json: &str, crate_version: &str) -> Vec<String> {
    let manifest = match Manifest::parse(manifest_json.as_bytes()) {
        Ok(m) => m,
        Err(e) => return vec![e],
    };
    let mut out = manifest.problems();
    out.extend(unknown_keys(manifest_json, &manifest));
    let (major, minor) = oxiguitar_plugin_api::INTERFACE;
    if manifest.interface.major != major || manifest.interface.minor > minor {
        out.push(format!(
            "the manifest asks for interface {}, but this SDK is {major}.{minor}",
            manifest.interface
        ));
    }
    if manifest.version.to_string() != crate_version {
        out.push(format!(
            "the manifest's version is {}, but the crate's is {crate_version}",
            manifest.version
        ));
    }
    let plugin = match P::start(Host::detached()) {
        Ok(p) => p,
        Err(e) => {
            out.push(format!("the plugin doesn't start: {e}"));
            return out;
        }
    };
    let knows_1_2 = manifest.knows_minor(2);
    for f in &manifest.formats {
        if !knows_1_2 && (plugin.reader2(&f.id).is_some() || plugin.writer2(&f.id).is_some()) {
            out.push(format!(
                "format {:?} uses the score version 2, which needs interface 1.2 in the manifest",
                f.id
            ));
        }
        if f.read && plugin.reader(&f.id).is_none() && plugin.reader2(&f.id).is_none() {
            out.push(format!(
                "format {:?} is declared as read, but has no reader",
                f.id
            ));
        }
        if f.write && plugin.writer(&f.id).is_none() && plugin.writer2(&f.id).is_none() {
            out.push(format!(
                "format {:?} is declared as written, but has no writer",
                f.id
            ));
        }
    }
    for s in &manifest.services {
        if plugin.service(&s.interface, &s.id).is_none() {
            out.push(format!(
                "service {:?} of {:?} is declared, but not implemented",
                s.id, s.interface
            ));
        }
    }
    if manifest.knows_minor(3) {
        for i in &manifest.instruments {
            if plugin.instrument(&i.id).is_none() {
                out.push(format!(
                    "instrument {:?} is declared, but not implemented",
                    i.id
                ));
            }
        }
        for e in &manifest.effects {
            if plugin.effect(&e.id).is_none() {
                out.push(format!(
                    "effect {:?} is declared, but not implemented",
                    e.id
                ));
            }
        }
    }
    for t in &manifest.tools {
        if plugin.tool(&t.id).is_none() {
            out.push(format!("tool {:?} is declared, but not implemented", t.id));
        }
    }
    match (manifest.settings_window, plugin.settings_window()) {
        (true, None) => out.push("a settings window is declared, but not implemented".into()),
        (false, Some(_)) => out.push(
            "the plugin has a settings window, but the manifest doesn't declare settings_window"
                .into(),
        ),
        (true, Some(window)) => {
            for p in window.content().problems() {
                out.push(format!("the settings window: {p}"));
            }
        }
        (false, None) => {}
    }
    for s in &manifest.track_sections {
        let Some(section) = plugin.track_section(&s.id) else {
            out.push(format!(
                "track section {:?} is declared, but not implemented",
                s.id
            ));
            continue;
        };
        let class = InstrumentClass::ALL
            .into_iter()
            .find(|&c| s.shows_for(c))
            .unwrap_or(InstrumentClass::Other);
        let track = TrackInfo {
            index: 0,
            name: "Track 1".into(),
            class,
        };
        for p in section.content(&track, &serde_json::Map::new()).problems() {
            out.push(format!("track section {:?}: {p}", s.id));
        }
    }
    for t in &manifest.tabs {
        match plugin.tab(&t.id) {
            None => out.push(format!("tab {:?} is declared, but not implemented", t.id)),
            Some(tab) => {
                for p in tab.content().problems() {
                    out.push(format!("tab {:?}: {p}", t.id));
                }
            }
        }
    }
    out
}

/// Keys in the manifest's text that the manifest types don't know: typing
/// mistakes, or keys from a newer interface than this SDK.
fn unknown_keys(text: &str, manifest: &Manifest) -> Vec<String> {
    let (Ok(written), Ok(known)) = (
        serde_json::from_str::<serde_json::Value>(text),
        serde_json::to_value(manifest),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    compare(&written, &known, "", &mut out);
    out
}

fn compare(
    written: &serde_json::Value,
    known: &serde_json::Value,
    at: &str,
    out: &mut Vec<String>,
) {
    use serde_json::Value;
    match (written, known) {
        (Value::Object(w), Value::Object(k)) => {
            for (key, value) in w {
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                match k.get(key) {
                    Some(known) => compare(value, known, &path, out),
                    None => out.push(format!("the manifest key {path:?} is unknown")),
                }
            }
        }
        (Value::Array(w), Value::Array(k)) => {
            for (i, (w, k)) in w.iter().zip(k).enumerate() {
                compare(w, k, &format!("{at}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// Exports a plugin: its entry symbol and its manifest section, when the
/// crate's `oxp` feature is on, and always a constant `MANIFEST_JSON` with
/// the manifest's text and a test (under `cargo test`) running [`check`].
///
/// Call it once, in the crate's `lib.rs`, with the type implementing
/// [`Plugin`]. The manifest is the file `manifest.json` beside the crate's
/// `Cargo.toml`.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        /// The plugin's manifest, `manifest.json`.
        pub const MANIFEST_JSON: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json"));

        // The section and the entry symbol need `unsafe` attributes, which
        // the plugin crate's `unsafe_code = "deny"` allows here only.
        #[cfg(feature = "oxp")]
        #[allow(unsafe_code)]
        const _: () = {
            // The manifest's bytes, in the section the host reads without
            // loading the plugin (oxiguitar_plugin_api::MANIFEST_SECTION).
            #[cfg_attr(target_os = "windows", unsafe(link_section = "oxgmanif"))]
            #[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,oxg_manifest"))]
            #[cfg_attr(
                not(any(target_os = "windows", target_vendor = "apple")),
                unsafe(link_section = "oxg_manifest")
            )]
            #[used]
            static MANIFEST: [u8; MANIFEST_JSON.len()] =
                *include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json"));

            static TABLE: $crate::glue::Table = $crate::glue::table::<$plugin>(&MANIFEST);

            // The system's allocator, counting what is allocated while the
            // plugin renders (the real-time rule; the contract suite reads
            // the count).
            #[global_allocator]
            static ALLOCATOR: $crate::glue::Counting = $crate::glue::Counting;

            /// The plugin's entry point (oxiguitar_plugin_api::ENTRY_SYMBOL).
            #[unsafe(no_mangle)]
            pub extern "C" fn oxiguitar_plugin_v1() -> *const $crate::api::abi::PluginTableV1 {
                TABLE.get()
            }
        };

        #[cfg(test)]
        mod oxiguitar_plugin_checks {
            #[allow(unused_imports)]
            use super::*;

            #[test]
            fn the_manifest_matches_the_plugin() {
                let problems =
                    $crate::check::<$plugin>(super::MANIFEST_JSON, env!("CARGO_PKG_VERSION"));
                assert!(
                    problems.is_empty(),
                    "manifest.json: {}",
                    problems.join("; ")
                );
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Plugin for Echo {
        fn start(_host: Host) -> Result<Echo, Error> {
            Ok(Echo)
        }

        fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
            (interface == "echo" && id == "echo").then_some(self as &dyn Service)
        }
    }

    impl Service for Echo {
        fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
            match method {
                "echo" => Ok(input.to_vec()),
                _ => Err(Error::not_found(method)),
            }
        }
    }

    const ECHO: &str = r#"{
        "interface": "1.0", "id": "test.echo", "name": "Echo", "version": "0.1.0",
        "services": [{"interface": "echo", "id": "echo"}]
    }"#;

    #[test]
    fn dispatch_reaches_the_contribution_or_says_it_is_missing() {
        let echo = Echo;
        let answer = dispatch(
            &echo,
            Request::Service {
                interface: "echo".into(),
                id: "echo".into(),
                method: "echo".into(),
                input: Blob::borrowed(b"hi"),
            },
        );
        assert_eq!(answer, Ok(Answer::Service(Blob::owned(b"hi".to_vec()))));
        let missing = dispatch(
            &echo,
            Request::Read {
                format: "gp5".into(),
                data: Blob::borrowed(b""),
            },
        )
        .unwrap_err();
        assert_eq!(missing.kind, ErrorKind::NotFound);
        let probe = dispatch(
            &echo,
            Request::Probe {
                interface: "echo".into(),
                id: "echo".into(),
            },
        );
        assert_eq!(probe, Ok(Answer::Probe(Availability::Available)));
    }

    #[test]
    fn check_finds_mistakes() {
        assert_eq!(check::<Echo>(ECHO, "0.1.0"), Vec::<String>::new());
        assert_eq!(
            check::<Echo>(ECHO, "0.2.0"),
            vec!["the manifest's version is 0.1.0, but the crate's is 0.2.0"]
        );
        let typo = ECHO.replace("\"services\"", "\"description\": \"\", \"servces\"");
        assert_eq!(
            check::<Echo>(&typo, "0.1.0"),
            vec!["the manifest key \"servces\" is unknown"]
        );
        let nested = ECHO.replace("\"id\": \"echo\"}", "\"id\": \"echo\", \"prio\": 1}");
        assert_eq!(
            check::<Echo>(&nested, "0.1.0"),
            vec!["the manifest key \"services[0].prio\" is unknown"]
        );
        let missing = ECHO.replace("\"echo\"}]", "\"other\"}]");
        assert_eq!(
            check::<Echo>(&missing, "0.1.0"),
            vec!["service \"other\" of \"echo\" is declared, but not implemented"]
        );
        let later = ECHO.replace("\"1.0\"", "\"1.9\"");
        assert!(check::<Echo>(&later, "0.1.0")[0].contains("this SDK is 1.3"));
    }

    /// A plugin of interface 1.2: a reader into the score version 2, a
    /// version-1 writer, and a tool.
    struct Two;

    impl Plugin for Two {
        fn start(_host: Host) -> Result<Two, Error> {
            Ok(Two)
        }
        fn reader2(&self, format: &str) -> Option<&dyn FormatReader2> {
            (format == "two").then_some(self as &dyn FormatReader2)
        }
        fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
            (format == "two").then_some(self as &dyn FormatWriter)
        }
        fn tool(&self, id: &str) -> Option<&dyn Tool> {
            (id == "t").then_some(self as &dyn Tool)
        }
    }

    impl FormatReader2 for Two {
        fn read(&self, _data: &[u8]) -> Result<Read2, Error> {
            Ok(Read2 {
                score: score2::Score2::default(),
                log: Vec::new(),
            })
        }
    }

    impl FormatWriter for Two {
        fn write(&self, score: &score::Score) -> Result<Written, Error> {
            Ok(Written {
                data: score.info.title.clone().into_bytes(),
                losses: Vec::new(),
            })
        }
    }

    impl Tool for Two {
        fn run(&self) -> Result<String, Error> {
            Ok("ran".into())
        }
    }

    #[test]
    fn interface_1_2_reads_version_2_writes_either_and_runs_tools() {
        let two = Two;
        let read = dispatch(
            &two,
            Request::Read {
                format: "two".into(),
                data: Blob::borrowed(b""),
            },
        )
        .unwrap();
        assert!(matches!(read, Answer::Read2 { .. }));
        let mut score = score2::Score2::default();
        score.info.title = "T".into();
        let mut bar = score2::Score2::from_v1(score::Score {
            master_bars: vec![score::MasterBar {
                time_signature: score::TimeSignature {
                    numerator: 4,
                    denominator: 4,
                },
                key_signature: score::KeySignature {
                    fifths: 0,
                    mode: score::KeyMode::Major,
                },
                repeat_start: false,
                repeat_end: None,
                alternate_endings: 0,
                marker: None,
                double_bar: false,
                triplet_feel: score::TripletFeel::None,
                tempo_changes: vec![],
                directions: vec![],
            }],
            ..score::Score::default()
        })
        .master_bars
        .remove(0);
        bar.ext.push(score2::MasterBarExt::FreeTime);
        score.master_bars.push(bar);
        let Answer::Written { data, losses } = dispatch(
            &two,
            Request::Write2 {
                format: "two".into(),
                score: Box::new(score),
            },
        )
        .unwrap() else {
            panic!("a file")
        };
        assert_eq!(data.as_slice(), b"T");
        assert_eq!(losses[0].message, "free time is left out by this plugin");
        assert_eq!(
            dispatch(&two, Request::Tool { tool: "t".into() }).unwrap(),
            Answer::Report("ran".into())
        );
        assert_eq!(
            dispatch(&two, Request::Tool { tool: "u".into() })
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
        // Version 2 needs interface 1.2 in the manifest.
        let manifest = r#"{"interface": "1.1", "id": "two", "name": "Two", "version": "0.1.0",
            "formats": [{"id": "two", "name": "Two", "extensions": ["two"], "read": true,
            "signatures": [{"at": 0, "text": "TWO"}]}]}"#;
        assert!(
            check::<Two>(manifest, "0.1.0")
                .iter()
                .any(|p| p.contains("needs interface 1.2"))
        );
        assert!(check::<Two>(&manifest.replace("1.1", "1.2"), "0.1.0").is_empty());
    }

    /// A tab counting its button's clicks, and events.
    struct Counter(std::sync::atomic::AtomicU32);

    impl Plugin for Counter {
        fn start(_host: Host) -> Result<Counter, Error> {
            Ok(Counter(0.into()))
        }

        fn tab(&self, id: &str) -> Option<&dyn Tab> {
            (id == "count").then_some(self as &dyn Tab)
        }

        fn event(&self, _event: &Event) {
            self.0.fetch_add(10, std::sync::atomic::Ordering::Relaxed);
        }
    }

    impl Tab for Counter {
        fn content(&self) -> TabContent {
            let n = self.0.load(std::sync::atomic::Ordering::Relaxed);
            TabContent {
                rows: vec![Row {
                    widgets: vec![Widget::Button {
                        id: "more".into(),
                        text: n.to_string(),
                        enabled: true,
                    }],
                }],
            }
        }

        fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
            match (widget, event) {
                ("more", TabEvent::Clicked) => {
                    self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(())
                }
                _ => Err(Error::not_found(widget)),
            }
        }
    }

    fn text_of(answer: Answer<'_>) -> String {
        match answer {
            Answer::Tab(t) => match &t.rows[0].widgets[0] {
                Widget::Button { text, .. } => text.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tabs_answer_with_their_contents_and_events_reach_the_plugin() {
        let counter = Counter::start(Host::detached()).unwrap();
        let tab = |tab: &str| dispatch(&counter, Request::Tab { tab: tab.into() });
        assert_eq!(text_of(tab("count").unwrap()), "0");
        assert_eq!(tab("other").unwrap_err().kind, ErrorKind::NotFound);
        let click = Request::TabEvent {
            tab: "count".into(),
            widget: "more".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(text_of(dispatch(&counter, click).unwrap()), "1");
        let wrong = Request::TabEvent {
            tab: "count".into(),
            widget: "less".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(
            dispatch(&counter, wrong).unwrap_err().kind,
            ErrorKind::NotFound
        );
        let event = Request::Event(Event::ScoreOpened { revision: 1 });
        assert_eq!(dispatch(&counter, event), Ok(Answer::Done));
        assert_eq!(text_of(tab("count").unwrap()), "11");

        let manifest = r#"{
            "interface": "1.1", "id": "test.counter", "name": "Counter", "version": "0.1.0",
            "tabs": [{"id": "count", "title": "Count", "area": "bottom"}],
            "events": ["score"]
        }"#;
        assert_eq!(check::<Counter>(manifest, "0.1.0"), Vec::<String>::new());
        let missing = manifest.replace("\"id\": \"count\"", "\"id\": \"other\"");
        assert_eq!(
            check::<Counter>(&missing, "0.1.0"),
            vec!["tab \"other\" is declared, but not implemented"]
        );
    }

    /// A plugin with a settings window (the counter again) and a track
    /// section holding a level per track.
    struct Amp(Counter);

    impl Plugin for Amp {
        fn start(host: Host) -> Result<Amp, Error> {
            Ok(Amp(Counter::start(host)?))
        }

        fn settings_window(&self) -> Option<&dyn Tab> {
            Some(&self.0)
        }

        fn track_section(&self, id: &str) -> Option<&dyn TrackSection> {
            (id == "amp").then_some(self as &dyn TrackSection)
        }
    }

    impl TrackSection for Amp {
        fn content(
            &self,
            track: &TrackInfo,
            settings: &serde_json::Map<String, serde_json::Value>,
        ) -> TabContent {
            let level = settings
                .get("level")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            TabContent {
                rows: vec![Row {
                    widgets: vec![Widget::Slider {
                        id: format!("level-{}", track.index),
                        value: level as f32,
                        min: 0.0,
                        max: 1.0,
                        enabled: true,
                    }],
                }],
            }
        }

        fn on(
            &self,
            _track: &TrackInfo,
            settings: &mut serde_json::Map<String, serde_json::Value>,
            _widget: &str,
            event: TabEvent,
        ) -> Result<(), Error> {
            if let TabEvent::Slid(v) = event {
                settings.insert("level".into(), f64::from(v).into());
            }
            Ok(())
        }
    }

    #[test]
    fn settings_windows_and_track_sections_answer() {
        let amp = Amp::start(Host::detached()).unwrap();
        assert_eq!(
            text_of(dispatch(&amp, Request::SettingsWindow).unwrap()),
            "0"
        );
        let click = Request::SettingsEvent {
            widget: "more".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(text_of(dispatch(&amp, click).unwrap()), "1");
        let track = TrackInfo {
            index: 3,
            name: "Lead".into(),
            class: InstrumentClass::Fretted,
        };
        let slid = Request::TrackSectionEvent {
            section: "amp".into(),
            track: track.clone(),
            settings: r#"{"model":"5150"}"#.into(),
            widget: "level-3".into(),
            event: TabEvent::Slid(0.5),
        };
        match dispatch(&amp, slid).unwrap() {
            Answer::TrackSection { content, settings } => {
                assert_eq!(settings, r#"{"level":0.5,"model":"5150"}"#);
                assert!(matches!(
                    &content.rows[0].widgets[0],
                    Widget::Slider { value, .. } if *value == 0.5
                ));
            }
            other => panic!("{other:?}"),
        }
        let not_json = Request::TrackSection {
            section: "amp".into(),
            track,
            settings: "[]".into(),
        };
        assert_eq!(
            dispatch(&amp, not_json).unwrap_err().kind,
            ErrorKind::Invalid
        );
        assert_eq!(
            dispatch(&Echo, Request::SettingsWindow).unwrap_err().kind,
            ErrorKind::NotFound
        );

        let manifest = r#"{
            "interface": "1.3", "id": "test.amp", "name": "Amp", "version": "0.1.0",
            "settings_window": true,
            "track_sections": [{"id": "amp", "title": "Amp", "plays": ["fretted"]}]
        }"#;
        assert_eq!(check::<Amp>(manifest, "0.1.0"), Vec::<String>::new());
        let neither = manifest
            .replace("\"settings_window\": true,", "")
            .replace("\"id\": \"amp\"", "\"id\": \"cab\"");
        assert_eq!(
            check::<Amp>(&neither, "0.1.0"),
            vec![
                "the plugin has a settings window, but the manifest doesn't declare settings_window",
                "track section \"cab\" is declared, but not implemented",
            ]
        );
    }

    /// An instrument that writes its notes' keys into its first track.
    struct Keys;

    impl Plugin for Keys {
        fn start(_host: Host) -> Result<Keys, Error> {
            Ok(Keys)
        }

        fn instrument(&self, id: &str) -> Option<&dyn audio::Instrument> {
            (id == "keys").then_some(self as &dyn audio::Instrument)
        }
    }

    struct KeysRenderer;

    impl audio::Instrument for Keys {
        fn prepare(&self, p: &api::message::Prepare) -> Result<Box<dyn audio::Renderer>, Error> {
            let _ = p;
            Ok(Box::new(KeysRenderer))
        }
    }

    impl audio::Renderer for KeysRenderer {
        fn render(&mut self, piece: &mut Piece<'_, '_>) {
            let key = piece
                .events()
                .first()
                .map_or(0.0, |e| f32::from(e.note.key));
            if let Some((l, _)) = piece.track(0) {
                l.fill(key);
            }
        }

        fn resolution(&self) -> u32 {
            7
        }
    }

    /// Keeps instances in a list: what a test does without the glue.
    #[derive(Default)]
    struct Listed(std::sync::Mutex<Vec<Option<audio::Instance>>>);

    impl audio::Keeper for Listed {
        fn keep(&self, instance: audio::Instance) -> u64 {
            let mut l = self.0.lock().unwrap();
            l.push(Some(instance));
            l.len() as u64
        }

        fn release(&self, id: u64) -> bool {
            let mut l = self.0.lock().unwrap();
            l.get_mut(id as usize - 1).and_then(Option::take).is_some()
        }
    }

    #[test]
    fn instruments_are_prepared_kept_rendered_and_released() {
        let keys = Keys::start(Host::detached()).unwrap();
        let prepare = api::message::Prepare {
            instrument: "keys".into(),
            rate: 48_000,
            frames: 64,
            tracks: Vec::new(),
        };
        // Without the glue, nothing keeps an instance.
        let e = dispatch(&keys, Request::Prepare(Box::new(prepare.clone()))).unwrap_err();
        assert_eq!(e.kind, ErrorKind::Unsupported);
        let listed = Listed::default();
        let answer = dispatch_kept(&keys, Request::Prepare(Box::new(prepare.clone())), &listed);
        assert_eq!(
            answer,
            Ok(Answer::Prepared {
                instance: 1,
                latency: 0,
                resolution: 7
            })
        );
        {
            let mut l = listed.0.lock().unwrap();
            let Some(audio::Instance::Instrument(r)) = l[0].as_mut() else {
                panic!("an instrument");
            };
            let events = [audio::RtEvent {
                kind: audio::RtEventKind::NOTE_ON,
                note: audio::RtNote {
                    key: 60,
                    ..audio::RtNote::default()
                },
            }];
            let (mut a, mut b) = ([0.0f32; 4], [0.0f32; 4]);
            let mut out = [&mut a[..], &mut b[..]];
            r.render(&mut Piece::new(&events, &[], 0.0, 1.0, 4, &mut out));
            assert_eq!(a, [60.0; 4]);
        }
        let release = |id| dispatch_kept(&keys, Request::Release { instance: id }, &listed);
        assert_eq!(release(1), Ok(Answer::Done));
        assert_eq!(release(1).unwrap_err().kind, ErrorKind::NotFound);
        let unknown = api::message::Prepare {
            instrument: "other".into(),
            ..prepare
        };
        assert_eq!(
            dispatch_kept(&keys, Request::Prepare(Box::new(unknown)), &listed)
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
        // A curve followed in straight lines, held at its ends.
        let curve = [
            audio::PitchPoint {
                time: 1.0,
                cents: 0,
            },
            audio::PitchPoint {
                time: 2.0,
                cents: 200,
            },
        ];
        assert_eq!(audio::cents_at(&curve, 0.5), 0.0);
        assert_eq!(audio::cents_at(&curve, 1.5), 100.0);
        assert_eq!(audio::cents_at(&curve, 3.0), 200.0);
        assert_eq!(audio::cents_at(&[], 3.0), 0.0);
    }

    #[test]
    fn a_detached_host_answers_plainly() {
        let host = Host::detached();
        host.notify(Level::Info, "nobody hears this");
        assert!(host.settings().unwrap().is_empty());
        assert!(host.set_settings(&serde_json::Map::new()).is_ok());
        let e = host.call_service("audio-device", "open", b"").unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
        host.redraw("count");
        assert_eq!(host.score(), Ok(None));
    }
}
