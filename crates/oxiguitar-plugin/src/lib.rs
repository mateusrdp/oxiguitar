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
pub use oxiguitar_plugin_api::message::{
    Advice, Availability, CORE_TOOLS, Clicked, DrawPlace, DrawTimes, Drawn, DrawnRun, Error,
    ErrorKind, Level, LogEntry, Perform, PerformStaff, PlayedBar, PlayedNote, PluginFile,
    PluginFileState, PluginFiles, PluginFolderKind, TextMeasure, ToolEntry, TrackInfo,
};
pub use oxiguitar_plugin_api::score;
pub use oxiguitar_plugin_api::score2;
pub use oxiguitar_plugin_api::tab::{
    Canvas, Cursor, Key, KeyName, Pointer, PointerButton, PointerKind, Row, Surface, TabContent,
    TabEvent, Widget,
};
pub use oxiguitar_plugin_api::view::{Place, TrackView, Value};
/// serde_json, as the SDK uses it: plugin settings and track sections'
/// settings are its JSON objects, so a plugin needs no dependency of its
/// own (interface 1.3).
pub use serde_json;

use std::sync::Arc;

use oxiguitar_plugin_api::Blob;
use oxiguitar_plugin_api::manifest::Manifest;
pub use oxiguitar_plugin_api::message::Link;
use oxiguitar_plugin_api::message::{Answer, HostAnswer, HostRequest, Request};

#[doc(hidden)]
pub mod audio;
pub mod glue;

pub use audio::{Effect, Instrument, Piece, Processor, Renderer, Update};
pub use oxiguitar_plugin_api::manifest::{ExportKind, ExporterSpec};

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

    /// What draws on the page, for a manifest with `draws` (interface
    /// 1.8).
    fn drawer(&self) -> Option<&dyn Drawer> {
        None
    }

    /// What plays the notes its own way, for a manifest with `performs`
    /// (interface 1.8).
    fn performer(&self) -> Option<&dyn Performer> {
        None
    }

    /// Its own UI, for a manifest with `views` (interface 1.15): its
    /// `.slint` file and what its views' callbacks do.
    fn ui(&self) -> Option<&dyn Ui> {
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

    /// The audio exporter `id` (an id from the manifest's `exporters`, of
    /// kind `audio`; interface 1.4).
    fn audio_exporter(&self, id: &str) -> Option<&dyn AudioExporter> {
        let _ = id;
        None
    }
}

/// An entry of File → Export that writes the score's sound to a file
/// (interface 1.4, `exporters` of kind `audio`). The core renders the score
/// through the player, faster than real time, and hands the sound over in
/// pieces; the exporter turns it into the file's bytes, which the core
/// writes. One export runs at a time, so an exporter keeps the one under
/// way itself (behind a `Mutex`); the calls come from one thread, never
/// the audio thread, so it may allocate.
pub trait AudioExporter: Send + Sync {
    /// A new export starts (dropping one left unfinished): the rate to
    /// render at, and the file's first bytes.
    fn start(&self) -> Result<ExportStart, Error>;

    /// The next stereo frames, left and right interleaved, at the rate
    /// asked for, as the mixer leaves them (full scale is ±1, and louder
    /// is possible): the bytes that follow in the file.
    fn samples(&self, interleaved: &[f32]) -> Result<Vec<u8>, Error>;

    /// The sound is over: the file's last bytes, and bytes to write over
    /// its start (a header naming the length). With `cancelled`, the user
    /// stopped the export and the file is thrown away; the answer is
    /// ignored.
    fn finish(&self, cancelled: bool) -> Result<ExportEnd, Error>;
}

/// How an audio export starts ([`AudioExporter::start`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportStart {
    /// The rate to render at, in frames a second (8000 to 384000).
    pub rate: u32,
    /// The file's first bytes, perhaps none.
    pub data: Vec<u8>,
}

/// How an audio export ends ([`AudioExporter::finish`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportEnd {
    /// The file's last bytes, perhaps none.
    pub data: Vec<u8>,
    /// Bytes to write over the file's start once all is written, or none:
    /// at most as long as the whole file.
    pub start: Option<Vec<u8>>,
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
}

/// Where a shared folder of the plugin's is (interface 1.8;
/// [`Host::folder`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderPlaces {
    /// The one in the user's OxiGuitar folder, beside `plugins`: where the
    /// user puts files, and the plugin may write.
    pub user: std::path::PathBuf,
    /// The same name where it exists beside a system plugin folder or the
    /// program: a package's files, read only.
    pub others: Vec<std::path::PathBuf>,
}

impl FolderPlaces {
    /// Every place, the user's first.
    pub fn all(&self) -> impl Iterator<Item = &std::path::Path> {
        std::iter::once(self.user.as_path()).chain(self.others.iter().map(|p| p.as_path()))
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

    /// Runs the tool as told by `options` (interface 1.14): another plugin
    /// asked ([`Host::run_tool`]), so it runs without asking the user. Only
    /// for a tool whose manifest entry has an options schema; the host
    /// hands over a JSON value as the plugin that asked sent it, so check
    /// it. Refusing is the default.
    fn run_with(&self, options: &serde_json::Value) -> Result<String, Error> {
        let _ = options;
        Err(Error::new(
            ErrorKind::Unsupported,
            "this tool runs only as from its menu",
        ))
    }
}

/// Draws on the page (interface 1.8), at the bars and beats that carry the
/// plugin's data or marks: [`Request::Draw`]'s places. Asked each time the
/// score is engraved and those places changed, so keep it quick and pure:
/// the same places, the same drawings.
pub trait Drawer: Send + Sync {
    /// What to draw at each place: at most one [`Drawn`] a place (its
    /// `place` the index in `places`), in staff spaces, from the box's top
    /// left corner; none where nothing is drawn. `times` says when each
    /// place and every bar are played (interface 1.13), for a drawing whose
    /// [`DrawnRun`] lasts as long as something it plays.
    fn draw(&self, places: &[DrawPlace], times: &DrawTimes) -> Result<Vec<Drawn>, Error>;

    /// The user clicked what it drew at `place`, at `at` in the drawing's
    /// box (staff spaces from its top left corner); `double` for a double
    /// click. The caret is already there. Returns a sentence for the footer
    /// (or an empty one), as a tool does; it may change the score as a tool
    /// does. Doing nothing is the default.
    fn clicked(
        &self,
        place: &DrawPlace,
        at: display::Point,
        double: bool,
    ) -> Result<String, Error> {
        let _ = (place, at, double);
        Ok(String::new())
    }
}

/// A plugin's own UI (interface 1.15; #346): one `.slint` file whose
/// components the manifest's `views` place, which OxiGuitar compiles with
/// Slint's interpreter and shows with its own Slint, widgets and style.
/// The plugin draws nothing itself and carries no widget code: it sets
/// properties ([`Host::set_view`]) and is told of callbacks here. Every
/// method is called off OxiGuitar's UI thread.
pub trait Ui: Send + Sync {
    /// The `.slint` file, usually `include_str!("../ui/NAME.slint")`.
    fn source(&self) -> String;

    /// The view `view` was made (a window opened, a panel shown, the
    /// track dialog opened on `track`, the export dialog showing the
    /// exporter). Its properties are as last set; set any that depend on
    /// what it shows. The default does nothing.
    fn shown(&self, view: &str, track: Option<&TrackView>) -> Result<(), Error> {
        let _ = (view, track);
        Ok(())
    }

    /// The callback `callback` of the view's component was called, with
    /// `args`.
    fn callback(&self, view: &str, callback: &str, args: Vec<Value>) -> Result<(), Error>;

    /// The view is gone: its window closed, or the dialog it was in,
    /// `accepted` when with OK. The default does nothing.
    fn closed(&self, view: &str, accepted: bool) -> Result<(), Error> {
        let _ = (view, accepted);
        Ok(())
    }
}

/// Plays the notes its own way (interface 1.8): given a track's notes as
/// the score gives them, gives them back as they should sound, changed,
/// added or left out. Asked when the song is got ready to play or export
/// and the notes changed, off the audio thread; keep it a pure function of
/// what it is given.
pub trait Performer: Send + Sync {
    /// The track's notes as they should sound.
    fn perform(&self, track: &Perform) -> Result<Vec<PlayedNote>, Error>;
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

    /// For a dialog (a tab whose area is `dialog`, interface 1.8): the
    /// user closed it, with OK (`ok`) or with Cancel or Escape. What the
    /// dialog changes, it changes now ([`Host::apply`] for the score). An
    /// error is shown in the footer. The default does nothing.
    fn closed(&self, ok: bool) -> Result<(), Error> {
        let _ = ok;
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

fn ui_of<P: Plugin + ?Sized>(plugin: &P) -> Result<&dyn Ui, Error> {
    plugin
        .ui()
        .ok_or_else(|| Error::not_found("this plugin has no UI of its own"))
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
        Request::Clicked(click) => {
            let drawer = plugin
                .drawer()
                .ok_or_else(|| Error::not_found("this plugin doesn't draw"))?;
            Ok(Answer::Report(drawer.clicked(
                &click.place,
                click.at,
                click.double,
            )?))
        }
        Request::Perform(track) => {
            let performer = plugin
                .performer()
                .ok_or_else(|| Error::not_found("this plugin doesn't play notes"))?;
            Ok(Answer::Performed(performer.perform(&track)?))
        }
        Request::Draw { places, times } => {
            let drawer = plugin
                .drawer()
                .ok_or_else(|| Error::not_found("this plugin draws nothing"))?;
            Ok(Answer::Drawn(drawer.draw(&places, &times)?))
        }
        Request::Tool { tool } => {
            let tool = plugin
                .tool(&tool)
                .ok_or_else(|| Error::not_found(format!("this plugin has no tool {tool:?}")))?;
            Ok(Answer::Report(tool.run()?))
        }
        Request::Ui => Ok(Answer::Ui(ui_of(plugin)?.source())),
        Request::ViewShown { view, track } => {
            ui_of(plugin)?.shown(&view, track.as_ref())?;
            Ok(Answer::Done)
        }
        Request::ViewCallback {
            view,
            callback,
            args,
        } => {
            ui_of(plugin)?.callback(&view, &callback, args)?;
            Ok(Answer::Done)
        }
        Request::ViewClosed { view, accepted } => {
            ui_of(plugin)?.closed(&view, accepted)?;
            Ok(Answer::Done)
        }
        Request::Tool2 { tool, options } => {
            let found = plugin
                .tool(&tool)
                .ok_or_else(|| Error::not_found(format!("this plugin has no tool {tool:?}")))?;
            let options: serde_json::Value = serde_json::from_str(&options).map_err(|e| {
                Error::new(
                    ErrorKind::Invalid,
                    format!("the options of {tool:?} aren't JSON: {e}"),
                )
            })?;
            Ok(Answer::Report(found.run_with(&options)?))
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
        Request::TabClosed { tab, ok } => {
            find_tab(plugin, &tab)?.closed(ok)?;
            Ok(Answer::Done)
        }
        Request::Event(event) => {
            plugin.event(&event);
            Ok(Answer::Done)
        }
        // Retired in interface 1.9: no host sends them.
        Request::RetiredSettingsWindow | Request::RetiredSettingsEvent { .. } => Err(
            Error::not_found("settings windows were retired in interface 1.9"),
        ),
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
        Request::PrepareFollowing(p) => {
            let effect = plugin.effect(&p.prepare.effect).ok_or_else(|| {
                Error::not_found(format!("this plugin has no effect {:?}", p.prepare.effect))
            })?;
            prepared(
                keeper,
                audio::Instance::Effect(effect.prepare_following(&p.prepare, &p.beats)?),
            )
        }
        Request::Release { instance } => {
            if keeper.release(instance) {
                Ok(Answer::Done)
            } else {
                Err(Error::not_found(format!("no instance {instance}")))
            }
        }
        Request::UpdateEffect {
            instance,
            prepare,
            previous,
        } => {
            let effect = plugin.effect(&prepare.effect).ok_or_else(|| {
                Error::not_found(format!("this plugin has no effect {:?}", prepare.effect))
            })?;
            let Some(update) = effect.update(&prepare, &previous)? else {
                return Ok(Answer::Updated(false));
            };
            if keeper.update(instance, update) {
                Ok(Answer::Updated(true))
            } else {
                Err(Error::not_found(format!("no effect instance {instance}")))
            }
        }
        Request::ExportStart { exporter } => {
            let ExportStart { rate, data } = find_exporter(plugin, &exporter)?.start()?;
            if !(8_000..=384_000).contains(&rate) {
                return Err(Error::new(
                    ErrorKind::Failed,
                    format!("the exporter asked for the rate {rate}, outside 8000 to 384000"),
                ));
            }
            Ok(Answer::ExportStarted {
                rate,
                data: Blob::owned(data),
            })
        }
        Request::ExportSamples { exporter, samples } => Ok(Answer::Exported(Blob::owned(
            find_exporter(plugin, &exporter)?.samples(&samples)?,
        ))),
        Request::ExportFinish {
            exporter,
            cancelled,
        } => {
            let ExportEnd { data, start } = find_exporter(plugin, &exporter)?.finish(cancelled)?;
            Ok(Answer::ExportFinished {
                data: Blob::owned(data),
                start: start.map(Blob::owned),
            })
        }
    }
}

fn find_exporter<'a>(plugin: &'a dyn Plugin, id: &str) -> Result<&'a dyn AudioExporter, Error> {
    plugin
        .audio_exporter(id)
        .ok_or_else(|| Error::not_found(format!("this plugin has no audio exporter {id:?}")))
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

    /// Asks the core to show the user advice (interface 1.4): a dialog
    /// with `advice`'s title, text and links, shown once the window is up
    /// and one dialog at a time, with a "Don't show this again" box. What
    /// the user ticks is kept by the core, under the plugin's id and the
    /// advice's id; the call returns `Ok` whether the dialog is shown or
    /// not, so call it whenever the reason holds (the core shows each
    /// advice at most once a session). The error says what is wrong with
    /// the advice ([`api::message::Advice::problem`]); never with a link,
    /// which only opens when the user clicks it.
    pub fn advise(&self, advice: api::message::Advice) -> Result<(), Error> {
        match self.0.request(HostRequest::Advise(advice))? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// The plugin's own folders (interface 1.3): its data and cache
    /// folders, created on first ask. Its shared folders are asked for by
    /// name, with [`Host::folder`].
    pub fn folders(&self) -> Result<Folders, Error> {
        match self.0.request(HostRequest::Folders)? {
            HostAnswer::Folders(f) => Ok(Folders {
                data: f.data.into(),
                cache: f.cache.into(),
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

    /// The open score as it is engraved in the window now (interface 1.8):
    /// every page's drawing, and where each beat is on it. `None` when no
    /// score is open. To put something on the page, give a beat a mark
    /// ([`score2::BeatExt::Mark`]) and hand the score back
    /// ([`Host::apply`]).
    pub fn engraving(&self) -> Result<Option<api::message::Engraving>, Error> {
        match self.0.request(HostRequest::Engraving)? {
            HostAnswer::Engraving(engraving) => Ok(engraving.map(|e| *e)),
            other => Err(unexpected(&other)),
        }
    }

    /// Where the shared folder `name` of the manifest's `folders` is
    /// (interface 1.8): a folder of the plugin's own choosing beside
    /// `plugins`, as `soundfonts` and `nam` are. The user's is created if
    /// it is missing.
    pub fn folder(&self, name: &str) -> Result<FolderPlaces, Error> {
        match self.0.request(HostRequest::Folder {
            name: name.to_owned(),
        })? {
            HostAnswer::Folder(f) => Ok(FolderPlaces {
                user: f.user.into(),
                others: f.others.into_iter().map(Into::into).collect(),
            }),
            other => Err(unexpected(&other)),
        }
    }

    /// Shows the view `view` (interface 1.15): opens its window, or brings
    /// its panel's tab to the front.
    pub fn show_view(&self, view: &str) -> Result<(), Error> {
        self.done(HostRequest::ShowView { view: view.into() })
    }

    /// Closes the view's window (interface 1.15).
    pub fn close_view(&self, view: &str) -> Result<(), Error> {
        self.done(HostRequest::CloseView { view: view.into() })
    }

    /// Sets properties of the view's component by their names in the
    /// `.slint` file (interface 1.15). They are kept for when the view is
    /// made again.
    pub fn set_view(&self, view: &str, properties: Vec<(String, Value)>) -> Result<(), Error> {
        self.done(HostRequest::SetView {
            view: view.into(),
            properties,
        })
    }

    /// The track's settings for this plugin, as its track-dialog view
    /// changed them (interface 1.15): kept for the track when the dialog
    /// is accepted.
    pub fn set_track_settings(
        &self,
        view: &str,
        settings: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        self.done(HostRequest::SetTrackSettings {
            view: view.into(),
            settings: serde_json::Value::Object(settings.clone()).to_string(),
        })
    }

    fn done(&self, request: HostRequest<'_>) -> Result<(), Error> {
        match self.0.request(request)? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Every plugin file OxiGuitar found in its plugin folders, usable or
    /// not, copies another file replaces included, and the user's plugin
    /// folder (interface 1.15): for a plugin that manages the others.
    pub fn plugin_files(&self) -> Result<PluginFiles, Error> {
        match self.0.request(HostRequest::PluginFiles)? {
            HostAnswer::PluginFiles(f) => Ok(f),
            other => Err(unexpected(&other)),
        }
    }

    /// The open score's file (interface 1.13), as an absolute path; `None`
    /// when no score is open or it was never saved. For paths kept relative
    /// to the score.
    pub fn score_file(&self) -> Result<Option<std::path::PathBuf>, Error> {
        match self.0.request(HostRequest::ScoreFile)? {
            HostAnswer::ScoreFile(f) => Ok(f.map(Into::into)),
            other => Err(unexpected(&other)),
        }
    }

    /// The caret or the selection in the open score as it is now (interface
    /// 1.8); `None` when no score is open. What [`Event::Selection2`] tells
    /// as it changes, for when the plugin needs it at once: a tool run
    /// before the caret ever moved.
    pub fn selection(&self) -> Result<Option<Selection2>, Error> {
        match self.0.request(HostRequest::Selection)? {
            HostAnswer::Selection(selection) => Ok(selection),
            other => Err(unexpected(&other)),
        }
    }

    /// Asks the core to open the dialog `tab` (the id of a tab whose area
    /// is `dialog`) over the window (interface 1.8): it asks for the
    /// dialog's [`Tab::content`], sends its widgets' events to [`Tab::on`]
    /// and tells [`Tab::closed`] how the user closed it. An error when it
    /// can't be opened: another dialog is open, or there is no window.
    pub fn open_dialog(&self, tab: &str) -> Result<(), Error> {
        match self
            .0
            .request(HostRequest::OpenDialog { tab: tab.into() })?
        {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Asks the core to bring the tab `tab` (an id from the manifest) to
    /// the front of its area, opening the area if it is closed (interface
    /// 1.8): for a tool whose controls are in a tab. A request that can't
    /// be made is dropped.
    pub fn show_tab(&self, tab: &str) {
        let _ = self.0.request(HostRequest::ShowTab { tab: tab.into() });
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

impl Host {
    /// Where each character of `text` would be drawn on one line in the
    /// core's `font` at `size` (interface 1.14): the left edge of each and,
    /// last, the end, with the font's ascent and descent. For laying out a
    /// [`Surface`]: wrapping lines, placing a caret, hitting a character.
    pub fn measure_text(
        &self,
        text: &str,
        size: f32,
        font: display::Font,
    ) -> Result<TextMeasure, Error> {
        match self.0.request(HostRequest::MeasureText {
            text: text.into(),
            size,
            font,
        })? {
            HostAnswer::Measured(m) => Ok(m),
            other => Err(unexpected(&other)),
        }
    }

    /// Puts `text` on the system's clipboard (interface 1.14). Pasting
    /// comes to a surface as [`TabEvent::Typed`].
    pub fn copy(&self, text: &str) -> Result<(), Error> {
        match self.0.request(HostRequest::Copy { text: text.into() })? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Every tool there is, the core's ([`CORE_TOOLS`]) and every usable
    /// plugin's (interface 1.14).
    pub fn tools(&self) -> Result<Vec<ToolEntry>, Error> {
        match self.0.request(HostRequest::Tools)? {
            HostAnswer::Tools(t) => Ok(t),
            other => Err(unexpected(&other)),
        }
    }

    /// Runs the tool `tool` of `owner` ([`CORE_TOOLS`] or a plugin's id)
    /// as if the user had picked it (interface 1.14): with `options`, which
    /// its schema describes, as told; without, as from its menu, asking the
    /// user in its dialog if it has one. Returns its report once it has
    /// run. Don't call it from a tool's own `run`: the core runs one tool
    /// at a time, and answers that with an error.
    pub fn run_tool(
        &self,
        owner: &str,
        tool: &str,
        options: Option<&serde_json::Value>,
    ) -> Result<String, Error> {
        match self.0.request(HostRequest::RunTool {
            owner: owner.into(),
            tool: tool.into(),
            options: options.map(serde_json::Value::to_string),
        })? {
            HostAnswer::Report(r) => Ok(r),
            other => Err(unexpected(&other)),
        }
    }

    /// Moves the caret or the selection in the open score (interface 1.14),
    /// as the user would. An error when no score is open or it has no such
    /// place.
    pub fn select(&self, selection: Selection2) -> Result<(), Error> {
        match self.0.request(HostRequest::Select(selection))? {
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
            | HostRequest::Redraw { .. }
            | HostRequest::ShowTab { .. }
            | HostRequest::Advise(_) => Ok(HostAnswer::Done),
            HostRequest::Engraving => Ok(HostAnswer::Engraving(None)),
            HostRequest::Selection => Ok(HostAnswer::Selection(None)),
            HostRequest::ScoreFile => Ok(HostAnswer::ScoreFile(None)),
            HostRequest::OpenDialog { .. } => Err(Error::new(
                ErrorKind::Failed,
                "a detached host has no window to show a dialog in",
            )),
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
            HostRequest::SetView { .. } | HostRequest::CloseView { .. } => Ok(HostAnswer::Done),
            HostRequest::ShowView { .. } | HostRequest::SetTrackSettings { .. } => Err(Error::new(
                ErrorKind::Failed,
                "a detached host has no window to show a view in",
            )),
            HostRequest::Folders | HostRequest::Folder { .. } | HostRequest::PluginFiles => {
                Err(Error::not_found("a detached host has no folders"))
            }
            HostRequest::Copy { .. } => Ok(HostAnswer::Done),
            HostRequest::Tools => Ok(HostAnswer::Tools(Vec::new())),
            HostRequest::MeasureText { .. }
            | HostRequest::RunTool { .. }
            | HostRequest::Select(_) => Err(Error::new(
                ErrorKind::Failed,
                "a detached host has no window, tools or score",
            )),
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
    for e in &manifest.exporters {
        if plugin.audio_exporter(&e.id).is_none() {
            out.push(format!(
                "exporter {:?} is declared, but not implemented",
                e.id
            ));
        }
    }
    for s in &manifest.track_sections {
        // A section shown by the plugin's own view (interface 1.15) is
        // the view's: nothing to describe.
        if manifest
            .views
            .iter()
            .any(|v| v.id == s.id && v.place == oxiguitar_plugin_api::view::Place::Track)
        {
            continue;
        }
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
    if !manifest.views.is_empty() {
        match plugin.ui() {
            None => out.push("views are declared, but the plugin has no `Ui`".into()),
            Some(ui) if ui.source().trim().is_empty() => {
                out.push("views are declared, but the plugin's .slint file is empty".into());
            }
            Some(_) => {}
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
/// `Cargo.toml`, or the file named after the type: a crate built in two
/// ways for different processors names each build's manifest (the NAM
/// plugin's `manifest-x86-64-v3.json`), under a `cfg` of its own.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        $crate::export_plugin!($plugin, "manifest.json");
    };
    ($plugin:ty, $manifest:literal) => {
        /// The plugin's manifest.
        pub const MANIFEST_JSON: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $manifest));

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
                *include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $manifest));

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
                    "{}: {}",
                    $manifest,
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
        let (major, minor) = oxiguitar_plugin_api::INTERFACE;
        let later = ECHO.replace("\"1.0\"", &format!("\"{major}.{}\"", minor + 1));
        assert!(
            check::<Echo>(&later, "0.1.0")[0].contains(&format!("this SDK is {major}.{minor}"))
        );
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

    /// A plugin with a track section holding a level per track.
    struct Amp;

    impl Plugin for Amp {
        fn start(_host: Host) -> Result<Amp, Error> {
            Ok(Amp)
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
    fn track_sections_answer_and_settings_windows_are_retired() {
        let amp = Amp::start(Host::detached()).unwrap();
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
            dispatch(&amp, Request::RetiredSettingsWindow)
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );

        let manifest = r#"{
            "interface": "1.3", "id": "test.amp", "name": "Amp", "version": "0.1.0",
            "track_sections": [{"id": "amp", "title": "Amp", "plays": ["fretted"]}]
        }"#;
        assert_eq!(check::<Amp>(manifest, "0.1.0"), Vec::<String>::new());
        // A manifest that still declares a settings window is told so.
        let old = manifest
            .replace(
                "\"track_sections\"",
                "\"settings_window\": true, \"track_sections\"",
            )
            .replace("\"id\": \"amp\"", "\"id\": \"cab\"");
        assert_eq!(
            check::<Amp>(&old, "0.1.0"),
            vec![
                "the manifest key \"settings_window\" is unknown",
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
