// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugins: what the plugin folders hold, which of them can run here,
//! and calling them (docs/design/plugins.md).
//!
//! [`Plugins::discover`] reads every plugin's manifest and loads nothing.
//! From the manifests alone the host knows which formats can be opened and
//! saved (and which plugin recognises a file, by its signatures), which
//! instruments play which [`InstrumentClass`]es, and which services exist.
//! A plugin's code is loaded the first time one of its contributions is
//! used, and stays loaded until the program ends.
//!
//! Which plugin file is used when several hold the same plugin id: the
//! highest version; on a version tie, the build needing more processor
//! features (a faster build this processor can run: the NAM plugin's for
//! x86-64-v3 processors); then the file in the later folder of the search
//! list, so the user's copy wins over the system's (system folders first,
//! user folders after; Mateus, 1 Nov 2026); in one folder, the earlier file
//! name. Plugins listed under `disabled` in
//! settings.json are never loaded; neither are plugins whose conditions
//! (system, processor, display system) don't hold here.
//!
//! Which service is used for an interface: the one settings.json pins
//! under `services` (`{"audio-device": "PLUGIN-ID/SERVICE-ID"}`) if it can
//! run here; otherwise the highest `priority`, then the earliest plugin.
//! Each candidate is asked whether it can work here (its probe) before it
//! is chosen; the choice is kept for the rest of the session.
//!
//! In safe mode (`oxiguitar --safe-mode`) every manifest is still read, so
//! the plugin manager can list them, but no plugin is used or loaded.
//!
//! Plugins with tabs (interface 1.1) are asked for their contents when the
//! window shows the tab, and told when the user uses a widget. Events
//! (score, selection, playback) are pushed to the plugins that subscribe
//! to them, in order, on a thread of their own that sleeps until there is
//! something to deliver.
//!
//! A plugin that fails (it can't be loaded or started, it crashes, or it
//! breaks the interface's contract: an answer that can't be read, of the
//! wrong kind, or tab contents the core can't show) is withdrawn for the
//! rest of the session, and everything goes on as if it weren't installed
//! (Mateus, 19 Oct 2026): its formats, tabs, services, instruments and
//! events are gone, a file it would have read goes to the next plugin
//! that recognises it, and a service is chosen again among the others. The
//! footer says so once, as an error; [`Core::withdrawn`] lets the window
//! take its tabs away. An ordinary error a plugin answers with (a damaged
//! file, a widget it refuses) is not a failure.
//!
//! Nothing here polls or runs on a timer: everything happens when it is
//! asked for.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use oxiguitar_plugin_api::Blob;
use oxiguitar_plugin_api::abi::Status;
use oxiguitar_plugin_api::class::InstrumentClass;
use oxiguitar_plugin_api::event::{Event, EventKind};
use oxiguitar_plugin_api::manifest::{
    Environment, ExportKind, ExporterSpec, FormatSpec, InstrumentSpec, Manifest, ServiceSpec,
    Shortcut, ToolSpec, TrackSectionSpec,
};
use oxiguitar_plugin_api::message::{
    Answer, Availability, Clicked, DrawPlace, DrawTimes, Drawn, Engraving, Error, ErrorKind,
    FolderPlaces, HostAnswer, HostRequest, Link, LogEntry, PERFORMED_MAX, Perform, PlayedBeat,
    PlayedNote, PluginFolders, Prepare, PrepareEffect, PrepareFollowing, Request, TrackInfo,
    decode, encode, outcome,
};
use oxiguitar_plugin_api::score::Score;
use oxiguitar_plugin_api::score2::{Score2, Since};
use oxiguitar_plugin_api::signature::Sniff;
use oxiguitar_plugin_api::tab::{Area, TabContent, TabEvent, TabSpec};
use serde_json::Value;

use crate::advice;
use crate::events::Queue;
use crate::folders::Folders;

use crate::inventory::{ISSUES_URL, Level, Notice, PLUGINS_URL, plugin_files};
use crate::loader::{self, Loaded};
use crate::oxp;
use crate::settings::SharedSettings;

/// The settings.json key listing plugin ids that are never loaded.
pub const DISABLED_KEY: &str = "disabled";

/// The settings.json key pinning a service per interface.
pub const SERVICES_KEY: &str = "services";

/// What the rest of the core does for plugins: shows their messages,
/// shows their tabs again when they ask, and hands them the open score.
/// Called from whichever thread the plugin was called on.
pub trait Core: Send + Sync {
    /// Shows `text` in the footer. `text` starts with the plugin's name.
    fn notify(&self, level: Level, text: String);

    /// Shows `text` in the footer with `link`, which opens in the system's
    /// browser when clicked (interface 1.3; the host has checked that it is
    /// an `https:` address). The default, for programs without a window,
    /// writes the address after the text.
    fn notify_with_link(&self, level: Level, text: String, link: Link) {
        self.notify(level, format!("{text} {}: {}", link.text, link.url));
    }

    /// The plugin's tab `tab` has changed: ask for its contents
    /// ([`Plugins::tab_content`]) when it is next on screen. The default
    /// does nothing, for programs without a window.
    fn redraw(&self, tab: &TabId) {
        let _ = tab;
    }

    /// The open score as engraved in the window (interface 1.8), or `None`
    /// when none is. The default, for programs without a window, has none.
    fn engraving(&self) -> Option<Engraving> {
        None
    }

    /// The caret or the selection in the open score (interface 1.8), or
    /// `None` when there is none. The default, for programs without a
    /// window, has none.
    fn selection(&self) -> Option<crate::api::event::Selection2> {
        None
    }

    /// A plugin asks for its dialog `tab` (a tab whose area is `dialog`,
    /// interface 1.8), titled `title`, to be opened over the window. The
    /// window asks for its contents ([`Plugins::tab_content`]) and says
    /// how the user closed it ([`Plugins::tab_closed`]). The error is a
    /// sentence for the plugin. The default has no window.
    fn open_dialog(&self, tab: &TabId, title: &str) -> Result<(), String> {
        let _ = (tab, title);
        Err("there is no window to show a dialog in".to_owned())
    }

    /// A plugin asks for its tab `tab` to be brought to the front of its
    /// area (interface 1.8). The default does nothing, for programs
    /// without a window.
    fn show_tab(&self, tab: &TabId) {
        let _ = tab;
    }

    /// The open score's file, or `None` when none is open or it was never
    /// saved (interface 1.13). The default has none.
    fn score_file(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// The open score, as plugins see it, or `None` when none is open. The
    /// default has none.
    fn score(&self) -> Option<Score> {
        None
    }

    /// The open score, version 2, and its revision (as score events give
    /// it), or `None` when none is open (interface 1.2). The default has
    /// none.
    fn score2(&self) -> Option<(u64, Score2)> {
        None
    }

    /// The plugin named `plugin` hands back the open score changed
    /// (interface 1.2): record it as one undoable change named `name`,
    /// unless the score has changed since `revision`. The error is a
    /// sentence for the plugin. The default has no score to change.
    fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: Score2,
    ) -> Result<(), String> {
        let _ = (plugin, revision, name, score);
        Err("no score is open".to_owned())
    }

    /// A plugin asks to show advice (interface 1.4): a dialog once the
    /// window is up, one at a time, with a "Don't show this again" box that
    /// the window answers with [`advice::hide`]. The host has checked the
    /// advice, left out what the user already silenced, and passes each
    /// advice on once per session. The default, for programs without a
    /// window, shows nothing.
    fn advise(&self, asked: advice::Asked) {
        let _ = asked;
    }

    /// The plugin `id` saved settings that differ from what it had
    /// ([`HostRequest::SetSettings`]), from whichever of its parts: the
    /// window prepares its sounds again if it plays any. The default does
    /// nothing.
    fn settings_changed(&self, id: &str) {
        let _ = id;
    }

    /// The plugin `id` failed and is withdrawn for the rest of the session:
    /// its tabs should go. The footer has already been told. The default
    /// does nothing.
    fn withdrawn(&self, id: &str) {
        let _ = id;
    }

    /// Where each character of `text` would be drawn in `font` at `size`
    /// (interface 1.14), for a plugin laying out a surface; `None` for a
    /// program without fonts, the default.
    fn measure_text(
        &self,
        text: &str,
        size: f32,
        font: crate::api::display::Font,
    ) -> Option<crate::api::message::TextMeasure> {
        let _ = (text, size, font);
        None
    }

    /// A plugin puts `text` on the clipboard (interface 1.14). The default,
    /// for programs without a window, does nothing.
    fn copy(&self, text: String) {
        let _ = text;
    }

    /// The core's own tools, as plugins may run them (interface 1.14):
    /// each with [`crate::api::message::CORE_TOOLS`] as its owner. The
    /// default has none.
    fn core_tools(&self) -> Vec<crate::api::message::ToolEntry> {
        Vec::new()
    }

    /// The plugin `caller` runs a tool (interface 1.14): `owner`'s tool
    /// `tool`, which the host has found among [`Core::core_tools`] or the
    /// usable plugins' tools, with `options` (JSON text) only for a tool
    /// that has an options schema. Runs it as the user's pick would, and
    /// returns its report once it has run; the error is a sentence for the
    /// plugin. The default, for programs without a window, runs nothing.
    fn run_tool(
        &self,
        caller: &str,
        owner: &str,
        tool: &str,
        options: Option<String>,
    ) -> Result<String, String> {
        let _ = (caller, owner, tool, options);
        Err("this program runs no tools for plugins".to_owned())
    }

    /// A plugin moves the caret or the selection (interface 1.14). The
    /// error is a sentence for the plugin. The default has no score.
    fn select(&self, selection: crate::api::event::Selection2) -> Result<(), String> {
        let _ = selection;
        Err("no score is open".to_owned())
    }
}

/// A core without a window: messages go nowhere, and no score is open.
/// For tests and tools without a footer.
#[derive(Debug, Default)]
pub struct Unheard;

impl Core for Unheard {
    fn notify(&self, _level: Level, _text: String) {}
}

/// Every plugin file found, and the host's way to them. Cloning shares
/// it; the plugins it loads stay loaded until the program ends.
#[derive(Clone)]
pub struct Plugins(Arc<Inner>);

struct Inner {
    found: Vec<Found>,
    /// Safe mode: no plugin is used.
    safe: bool,
    env: Environment,
    settings: SharedSettings,
    core: Arc<dyn Core>,
    /// The service chosen per interface, or why there is none.
    chosen: Mutex<HashMap<String, Choice>>,
    /// Events waiting to be delivered.
    events: Arc<Queue>,
    /// The plugins that answered an event with an error: told once in the
    /// footer.
    failed_events: Mutex<Vec<usize>>,
    /// The advice already passed on to the window this session, by plugin
    /// and advice id: a plugin may ask again whenever its reason holds,
    /// and the user sees each once.
    advised: Mutex<HashSet<(String, String)>>,
    /// OxiGuitar's folders, for the plugins' own folders and the shared
    /// ones (interface 1.3); unset for programs that keep none.
    folders: OnceLock<Folders>,
    /// The folders are only read (`oxiguitar-cli`): a plugin's own folders
    /// are named but not created.
    read_only: std::sync::atomic::AtomicBool,
    /// Each drawing plugin's last request (encoded) and what it drew then
    /// (interface 1.8): the same places aren't asked about twice.
    drawn: Mutex<HashMap<usize, LastDrawn>>,
    /// Each performing plugin's last request and answer by track
    /// (interface 1.8).
    performed: Mutex<HashMap<(usize, u32), LastPerformed>>,
}

/// A drawing plugin's last request, encoded, and its answer.
type LastDrawn = (Vec<u8>, Vec<Drawn>);

/// A performing plugin's last request for a track, encoded, and its answer.
type LastPerformed = (Vec<u8>, Vec<PlayedNote>);

impl Drop for Inner {
    fn drop(&mut self) {
        // Ends the delivery thread, which holds the host only weakly.
        self.events.close();
    }
}

/// A chosen service (the plugin's index and the service's id), or why
/// there is none.
type Choice = Result<(usize, String), String>;

/// One plugin file and what became of it.
pub struct Found {
    /// The file.
    pub path: PathBuf,
    /// Its manifest, if it could be read.
    pub manifest: Option<Manifest>,
    /// Whether it can be used.
    pub state: State,
    manifest_bytes: Vec<u8>,
    loaded: OnceLock<Result<Loaded, String>>,
    /// Why it was withdrawn, once it failed.
    failed: OnceLock<String>,
}

/// Whether a plugin file can be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// It is used when needed.
    Ready,
    /// It can't be used here; the text says why (a damaged file, another
    /// system's plugin, an interface this OxiGuitar doesn't have).
    Unusable(String),
    /// settings.json disables it.
    Disabled,
    /// Another file holds the same plugin at a higher version, or the same
    /// version built for more of this processor's features, or the same
    /// version and build in a later folder (or earlier in this one), or a
    /// plugin that replaces this one's id (interface 1.4); that file is
    /// used.
    Replaced(PathBuf),
    /// A plugin it needs (interface 1.4) is missing, too old, or can't be
    /// used itself; the text says which. Unlike [`State::Unusable`], this is
    /// something the user can put right, so the footer shows it as an
    /// error.
    Unmet(String),
}

impl Found {
    /// How a crash report names the plugin: id, version, file and the
    /// homepage its users report problems to.
    fn crash_name(&self) -> String {
        let file = self.path.display();
        match &self.manifest {
            Some(m) if !m.homepage.is_empty() => {
                format!("{} {} ({file}; {})", m.id, m.version, m.homepage)
            }
            Some(m) => format!("{} {} ({file})", m.id, m.version),
            None => format!("({file})"),
        }
    }

    /// The plugin's name, or its file's name if the manifest is unreadable.
    pub fn name(&self) -> String {
        match &self.manifest {
            Some(m) => m.name.clone(),
            None => file_name(&self.path),
        }
    }

    /// Whether its code has been loaded (and started, or failed to).
    pub fn is_loaded(&self) -> bool {
        self.loaded.get().is_some()
    }

    /// Why it was withdrawn for the rest of the session, if it failed.
    pub fn failure(&self) -> Option<&str> {
        self.failed.get().map(String::as_str)
    }

    fn ready(&self) -> Option<&Manifest> {
        match (&self.state, &self.manifest, self.failed.get()) {
            (State::Ready, Some(m), None) => Some(m),
            _ => None,
        }
    }
}

impl fmt::Debug for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Found")
            .field("path", &self.path)
            .field("id", &self.manifest.as_ref().map(|m| &m.id))
            .field("state", &self.state)
            .field("loaded", &self.is_loaded())
            .field("failed", &self.failure())
            .finish()
    }
}

impl fmt::Debug for Plugins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.0.found).finish()
    }
}

/// A file format a plugin reads or writes.
#[derive(Clone, Copy, Debug)]
pub struct FormatRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The format, as its manifest declares it.
    pub spec: &'a FormatSpec,
    index: usize,
}

/// An instrument a plugin offers the player.
#[derive(Clone, Copy, Debug)]
pub struct InstrumentRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The instrument, as its manifest declares it.
    pub spec: &'a InstrumentSpec,
}

/// The service chosen for an interface.
#[derive(Clone, Copy, Debug)]
pub struct ServiceRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The service, as its manifest declares it.
    pub spec: &'a ServiceSpec,
}

impl ServiceRef<'_> {
    /// The service's name, for the user: its manifest's, else its id.
    pub fn name(&self) -> &str {
        service_name(self.spec)
    }
}

/// A plugin's tab, by the plugin's id and the tab's.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TabId {
    /// The plugin's id.
    pub plugin: String,
    /// The tab's id, from the plugin's manifest.
    pub tab: String,
}

/// A tool a plugin adds to the Tools menu (interface 1.2).
#[derive(Clone, Copy, Debug)]
pub struct ToolRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The tool, as its manifest declares it.
    pub spec: &'a ToolSpec,
    index: usize,
}

/// An export a plugin adds to File → Export (interface 1.4).
#[derive(Clone, Copy, Debug)]
pub struct ExporterRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The exporter, as its manifest declares it.
    pub spec: &'a ExporterSpec,
    index: usize,
}

impl ExporterRef<'_> {
    /// The id of the exporter's plugin.
    pub fn plugin_id(&self) -> &str {
        self.plugin.manifest.as_ref().map_or("", |m| m.id.as_str())
    }
}

/// An audio export under way ([`Plugins::start_audio_export`]): the
/// exporter's rate and first bytes, and the way to hand it the sound.
/// Dropped without [`AudioExport::finish`], it tells the exporter the
/// export was cancelled.
pub struct AudioExport {
    plugins: Plugins,
    index: usize,
    exporter: String,
    /// The rate the exporter wants the sound at.
    pub rate: u32,
    /// The file's first bytes.
    pub first: Vec<u8>,
    finished: bool,
}

impl fmt::Debug for AudioExport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioExport")
            .field("exporter", &self.exporter)
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

/// How an audio export ended ([`AudioExport::finish`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportEnd {
    /// The file's last bytes.
    pub last: Vec<u8>,
    /// Bytes to write over the file's start, if any.
    pub start: Option<Vec<u8>>,
}

impl AudioExport {
    /// Hands the exporter the next stereo frames (interleaved); returns the
    /// bytes that follow in the file.
    pub fn samples(&mut self, interleaved: &[f32]) -> Result<Vec<u8>, Error> {
        let request = Request::ExportSamples {
            exporter: self.exporter.clone(),
            samples: interleaved.to_vec(),
        };
        match self.plugins.request(self.index, &request)? {
            Answer::Exported(b) => Ok(b.as_slice().to_vec()),
            other => Err(self.plugins.unexpected(self.index, &other)),
        }
    }

    /// The sound is over: the file's last bytes and what to write over its
    /// start.
    pub fn finish(mut self) -> Result<ExportEnd, Error> {
        self.finished = true;
        let request = Request::ExportFinish {
            exporter: self.exporter.clone(),
            cancelled: false,
        };
        match self.plugins.request(self.index, &request)? {
            Answer::ExportFinished { data, start } => Ok(ExportEnd {
                last: data.as_slice().to_vec(),
                start: start.map(|s| s.as_slice().to_vec()),
            }),
            other => Err(self.plugins.unexpected(self.index, &other)),
        }
    }

    /// Stops the export: the exporter is told, and nothing more is
    /// written.
    pub fn cancel(mut self) {
        self.cancelled();
    }

    fn cancelled(&mut self) {
        if !self.finished {
            self.finished = true;
            let _ = self.plugins.request(
                self.index,
                &Request::ExportFinish {
                    exporter: self.exporter.clone(),
                    cancelled: true,
                },
            );
        }
    }
}

impl Drop for AudioExport {
    fn drop(&mut self) {
        self.cancelled();
    }
}

/// A section a plugin adds to the track dialog (interface 1.3).
#[derive(Clone, Copy, Debug)]
pub struct SectionRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The section, as its manifest declares it.
    pub spec: &'a TrackSectionSpec,
    index: usize,
}

impl SectionRef<'_> {
    /// The id of the section's plugin, under which the track keeps its
    /// settings.
    pub fn plugin_id(&self) -> &str {
        self.plugin.manifest.as_ref().map_or("", |m| m.id.as_str())
    }
}

/// A tab a plugin adds to the window.
#[derive(Clone, Copy, Debug)]
pub struct TabRef<'a> {
    /// The plugin.
    pub plugin: &'a Found,
    /// The tab, as its manifest declares it.
    pub spec: &'a TabSpec,
    index: usize,
}

impl TabRef<'_> {
    /// The tab's id.
    pub fn id(&self) -> TabId {
        TabId {
            plugin: self
                .plugin
                .manifest
                .as_ref()
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            tab: self.spec.id.clone(),
        }
    }
}

/// Why no plugin opens a file ([`Plugins::recognise`]). Its text is a
/// sentence for the footer, to follow the file's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unrecognised {
    /// No plugin that reads files is installed.
    NoReaders,
    /// No installed plugin recognises the file, and none declares its
    /// extension: a feature request.
    Unknown,
    /// A usable plugin declares the file's extension but doesn't recognise
    /// it: a bug report.
    NotRecognised {
        /// The plugin's name.
        plugin: String,
        /// The format's name.
        format: String,
        /// The file's extension, lowercase.
        extension: String,
    },
    /// OxiGuitar was started in safe mode, with no plugins.
    SafeMode,
    /// The plugin that declares the file's extension can't be used here.
    NotUsable {
        /// The plugin's name.
        plugin: String,
        /// The file's extension, lowercase.
        extension: String,
        /// Why it can't be used.
        why: String,
    },
}

impl fmt::Display for Unrecognised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unrecognised::NoReaders => write!(
                f,
                "no plugin that opens files is installed. The basic plugins come with every OxiGuitar download ({PLUGINS_URL}): install it again, or put plugins in your plugins folder."
            ),
            Unrecognised::Unknown => write!(
                f,
                "no installed plugin recognises this file. To ask for a plugin that opens it, please open a feature request: {ISSUES_URL}"
            ),
            Unrecognised::NotRecognised {
                plugin,
                format,
                extension,
            } => write!(
                f,
                "the {plugin} plugin opens .{extension} files but doesn't recognise this one. If it is a {format} file, please report a bug: {ISSUES_URL}"
            ),
            Unrecognised::SafeMode => write!(
                f,
                "OxiGuitar was started in safe mode, without plugins, so it can't open any file. Start it again without --safe-mode."
            ),
            Unrecognised::NotUsable {
                plugin,
                extension,
                why,
            } => write!(
                f,
                "the {plugin} plugin, which opens .{extension} files, can't be used: {why}."
            ),
        }
    }
}

impl std::error::Error for Unrecognised {}

/// What a format plugin read.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadScore {
    /// The score, as plugins see it, version 2 (the app converts it to its
    /// model). A plugin built for an interface before 1.2 reads version 1,
    /// given here as version 2 with no extensions.
    pub score: Score2,
    /// What the reader couldn't carry over faithfully (FMT-2).
    pub log: Vec<LogEntry>,
}

/// A file opened by [`Plugins::open`]: the format that read it, and what
/// its plugin read.
#[derive(Clone, Debug)]
pub struct OpenedFile<'a> {
    /// The format, and its plugin.
    pub format: FormatRef<'a>,
    /// The score and the import log.
    pub read: ReadScore,
}

/// Why [`Plugins::open`] opened nothing. Its text is a sentence for the
/// footer, to follow the file's name.
#[derive(Clone, Debug, PartialEq)]
pub enum OpenError {
    /// No usable plugin recognises the file.
    Unrecognised(Unrecognised),
    /// The plugin that recognised it couldn't read it (a damaged file).
    Read(Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Unrecognised(why) => why.fmt(f),
            OpenError::Read(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

/// What a format plugin wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrittenFile {
    /// The file's contents.
    pub data: Vec<u8>,
    /// What the format can't hold and was left out (FMT-3).
    pub losses: Vec<LogEntry>,
}

thread_local! {
    /// The plugins this thread is starting, to refuse a plugin that asks
    /// for itself while it starts (which would otherwise wait forever).
    static STARTING: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

impl Plugins {
    /// Reads the manifests of every plugin file in `folders` (in order),
    /// without loading any plugin. `display` names the display system the
    /// core runs on (`wayland`, `x11`, `windows`, `macos`), for plugins'
    /// conditions. `settings` holds `disabled` and `services`, and every
    /// plugin's own settings; `core` shows plugins' footer messages and
    /// tabs and hands them the open score.
    ///
    /// A program has one `Plugins`: the system loads a library file once
    /// per process, so a second `Plugins` starting the same file gets the
    /// plugin's "it was started twice" error.
    pub fn discover(
        folders: &[PathBuf],
        settings: SharedSettings,
        display: &str,
        core: Arc<dyn Core>,
    ) -> Plugins {
        Plugins::find(folders, settings, display, core, false)
    }

    /// Like [`Plugins::discover`], in safe mode: every manifest is read, so
    /// [`Plugins::found`] lists every plugin file as usual, but no plugin is
    /// used or loaded. Nothing can be opened and nobody hears an event.
    pub fn discover_in_safe_mode(
        folders: &[PathBuf],
        settings: SharedSettings,
        display: &str,
        core: Arc<dyn Core>,
    ) -> Plugins {
        Plugins::find(folders, settings, display, core, true)
    }

    fn find(
        folders: &[PathBuf],
        settings: SharedSettings,
        display: &str,
        core: Arc<dyn Core>,
        safe: bool,
    ) -> Plugins {
        let env = Environment::current(display);
        let disabled: Vec<String> = match settings.get(DISABLED_KEY) {
            Some(Value::Array(ids)) => ids
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        // Each file with the place of its folder in the search list.
        let (folder_of, mut found): (Vec<usize>, Vec<Found>) = folders
            .iter()
            .enumerate()
            .flat_map(|(n, folder)| plugin_files(folder).into_iter().map(move |p| (n, p)))
            .map(|(n, path)| {
                // dlopen and LoadLibrary search the system's folders for a
                // bare file name; a full path never is.
                let path = std::path::absolute(&path).unwrap_or(path);
                (n, examine(path, &env, &disabled))
            })
            .unzip();
        // One file per plugin id: the highest version wins; on a version
        // tie, the build needing more processor features; then the file in
        // the later folder (the user's over the system's); in one folder,
        // the earlier file name.
        for i in 0..found.len() {
            let Some(id) = found[i].ready().map(|m| m.id.clone()) else {
                continue;
            };
            let winner = (0..found.len())
                .filter(|&j| found[j].ready().is_some_and(|m| m.id == id))
                .reduce(|best, j| {
                    let (b, c) = (found[best].ready(), found[j].ready());
                    match (b, c) {
                        (Some(b), Some(c))
                            if better(c, b)
                                || (!better(b, c) && folder_of[j] > folder_of[best]) =>
                        {
                            j
                        }
                        _ => best,
                    }
                });
            if let Some(w) = winner
                && w != i
            {
                found[i].state = State::Replaced(found[w].path.clone());
            }
        }
        settle(&mut found);
        Plugins(Arc::new(Inner {
            found,
            safe,
            env,
            settings,
            core,
            chosen: Mutex::new(HashMap::new()),
            events: Arc::new(Queue::default()),
            failed_events: Mutex::new(Vec::new()),
            advised: Mutex::new(HashSet::new()),
            folders: OnceLock::new(),
            read_only: std::sync::atomic::AtomicBool::new(false),
            drawn: Mutex::new(HashMap::new()),
            performed: Mutex::new(HashMap::new()),
        }))
    }

    /// The ids that ready plugins replace (interface 1.4), each with the id
    /// of the plugin that replaces it: `(old, new)`.
    pub fn replaced_ids(&self) -> Vec<(String, String)> {
        self.0
            .found
            .iter()
            .filter_map(Found::ready)
            .flat_map(|m| m.replaces.iter().map(|old| (old.clone(), m.id.clone())))
            .collect()
    }

    /// Carries what OxiGuitar keeps under a replaced plugin's id over to
    /// the plugin that replaces it (interface 1.4), once: its object in
    /// settings.json and its data folder, each only where the new id has
    /// nothing yet. Nothing old is changed or deleted, so an older OxiGuitar
    /// still finds it. The caches aren't carried: they are made again. The
    /// answer is a line for the footer per thing carried or failed.
    pub fn carry_over(&self, folders: &Folders) -> Vec<Notice> {
        let mut out = Vec::new();
        for (old, new) in self.replaced_ids() {
            let settings = &self.0.settings;
            if settings.get(&new).is_none()
                && let Some(value) = settings.get(&old)
            {
                out.push(match settings.set(&new, value) {
                    Ok(()) => Notice::new(
                        Level::Info,
                        format!("The settings of {old} now belong to {new}, which replaces it."),
                    ),
                    Err(e) => Notice::new(Level::Warning, e),
                });
            }
            let (from, to) = (folders.plugin_data(&old), folders.plugin_data(&new));
            if from.is_dir() && !to.exists() {
                out.push(match copy_folder(&from, &to) {
                    Ok(()) => Notice::new(
                        Level::Info,
                        format!("Copied {} to {}.", from.display(), to.display()),
                    ),
                    Err(e) => {
                        // Half a copy would stop the next start from trying.
                        let _ = std::fs::remove_dir_all(&to);
                        Notice::new(
                            Level::Warning,
                            format!("Cannot copy {} to {}: {e}.", from.display(), to.display()),
                        )
                    }
                });
            }
        }
        out
    }

    /// Gives the plugins OxiGuitar's folders, for their own data and cache
    /// folders and the shared folders their manifests list (interface 1.3).
    /// Without it, plugins asking for their folders get an error. Set once;
    /// later calls change nothing.
    pub fn use_folders(&self, folders: Folders) {
        let _ = self.0.folders.set(folders);
    }

    /// Gives the plugins OxiGuitar's folders to read only, as a program
    /// that changes nothing in them does (`oxiguitar-cli`): plugins asking
    /// for their own data and cache folders are told where they are, but
    /// they aren't created, so what a plugin writes there fails unless the
    /// app made them.
    pub fn use_folders_read_only(&self, folders: Folders) {
        self.0
            .read_only
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.use_folders(folders);
    }

    /// The shared folders the ready plugins list in their manifests, each
    /// once, so the host can create them at start (interface 1.3).
    pub fn shared_folders(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .ready()
            .filter(|(_, _, m)| m.knows_minor(3))
            .flat_map(|(_, _, m)| m.folders.iter().cloned())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Every plugin file found, in folder order and then by name.
    pub fn found(&self) -> &[Found] {
        &self.0.found
    }

    /// Whether this is safe mode: no plugin is used.
    pub fn safe_mode(&self) -> bool {
        self.0.safe
    }

    /// The system the plugins' conditions were checked against.
    pub fn environment(&self) -> &Environment {
        &self.0.env
    }

    /// The settings plugins read and write.
    pub fn settings(&self) -> &SharedSettings {
        &self.0.settings
    }

    /// Every format the ready plugins read, in plugin order.
    pub fn readers(&self) -> Vec<FormatRef<'_>> {
        self.formats(|f| f.read)
    }

    /// Every format the ready plugins write, in plugin order.
    pub fn writers(&self) -> Vec<FormatRef<'_>> {
        self.formats(|f| f.write)
    }

    fn formats(&self, keep: impl Fn(&FormatSpec) -> bool) -> Vec<FormatRef<'_>> {
        self.ready()
            .flat_map(|(index, plugin, m)| {
                m.formats
                    .iter()
                    .filter(|f| keep(f))
                    .map(move |spec| FormatRef {
                        plugin,
                        spec,
                        index,
                    })
            })
            .collect()
    }

    /// The format of `data`, a whole file, from the ready plugins'
    /// signatures, without loading any plugin: the first plugin (in folder
    /// order) with a matching signature reads it.
    pub fn format_for(&self, data: &[u8]) -> Option<FormatRef<'_>> {
        let sniff = Sniff::new(data);
        self.ready().find_map(|(index, plugin, m)| {
            m.format_for(&sniff).map(|spec| FormatRef {
                plugin,
                spec,
                index,
            })
        })
    }

    /// The format of `data`, a whole file, or why no plugin opens it. The
    /// installed plugins' formats are all the core can open: it recognises
    /// nothing itself (Mateus, 18 Oct 2026). `extension` is the file's
    /// extension, if it has one; it never decides the format, only which
    /// report the refusal asks for.
    pub fn recognise(
        &self,
        data: &[u8],
        extension: Option<&str>,
    ) -> Result<FormatRef<'_>, Unrecognised> {
        if self.0.safe {
            return Err(Unrecognised::SafeMode);
        }
        if let Some(format) = self.format_for(data) {
            return Ok(format);
        }
        let claims = |m: &Manifest| {
            let ext = extension?;
            m.formats
                .iter()
                .filter(|f| f.read)
                .find(|f| f.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                .map(|f| (f.name.clone(), ext.to_ascii_lowercase()))
        };
        if let Some((found, (format, extension))) = self
            .ready()
            .find_map(|(_, found, m)| claims(m).map(|c| (found, c)))
        {
            return Err(Unrecognised::NotRecognised {
                plugin: found.name(),
                format,
                extension,
            });
        }
        let idle = self
            .0
            .found
            .iter()
            .find_map(|found| match (&found.state, &found.manifest) {
                (State::Disabled | State::Unusable(_), Some(m)) => claims(m).map(|c| (found, c)),
                _ => None,
            });
        if let Some((found, (_, extension))) = idle {
            return Err(Unrecognised::NotUsable {
                plugin: found.name(),
                extension,
                why: match &found.state {
                    State::Unusable(why) => why.clone(),
                    _ => format!("it is disabled in {}", crate::SETTINGS_FILE),
                },
            });
        }
        if self.readers().is_empty() {
            Err(Unrecognised::NoReaders)
        } else {
            Err(Unrecognised::Unknown)
        }
    }

    /// The extensions of every format the usable plugins read, without the
    /// dot, lowercase, each once: File → Open's filter.
    pub fn read_extensions(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for format in self.readers() {
            for e in &format.spec.extensions {
                let e = e.to_ascii_lowercase();
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
        out
    }

    /// Recognises `data`, a whole file, and reads it with the plugin that
    /// recognises it. If that plugin fails while reading, it is withdrawn
    /// and the next plugin that recognises the file reads it, as if the
    /// first had never been installed. `extension` is as for
    /// [`Plugins::recognise`].
    pub fn open(&self, data: &[u8], extension: Option<&str>) -> Result<OpenedFile<'_>, OpenError> {
        loop {
            let format = self
                .recognise(data, extension)
                .map_err(OpenError::Unrecognised)?;
            match self.read(format, data) {
                Ok(read) => return Ok(OpenedFile { format, read }),
                // Withdrawn: each turn withdraws one plugin, so this ends.
                Err(_) if format.plugin.failure().is_some() => {}
                Err(e) => return Err(OpenError::Read(e)),
            }
        }
    }

    /// Reads `data` with the plugin that owns `format`, loading it first if
    /// needed.
    pub fn read(&self, format: FormatRef<'_>, data: &[u8]) -> Result<ReadScore, Error> {
        match self.request(
            format.index,
            &Request::Read {
                format: format.spec.id.clone(),
                data: Blob::borrowed(data),
            },
        )? {
            Answer::Read { score, log } => Ok(ReadScore {
                score: Score2::from_v1(*score),
                log,
            }),
            Answer::Read2 { score, log } => Ok(ReadScore { score: *score, log }),
            other => Err(self.unexpected(format.index, &other)),
        }
    }

    /// Writes `score` with the plugin that owns `format`. A plugin built
    /// for interface 1.2 or later gets the score version 2, without what
    /// its version doesn't know; an older one gets version 1. What the
    /// plugin's version can't hold is added to the losses.
    pub fn write(&self, format: FormatRef<'_>, score: &Score2) -> Result<WrittenFile, Error> {
        // What plugins keep on beats (interface 1.8) is saved beside the
        // score: no format is sent it, and it is no loss.
        let mut score = score.clone();
        score.drop_plugin_data();
        let score = &score;
        let minor = format
            .plugin
            .manifest
            .as_ref()
            .map_or(0, |m| m.interface.minor);
        let lost: Vec<LogEntry>;
        let request = if minor >= 2 {
            let name = format.plugin.name();
            lost = score
                .extensions_newer_than(minor)
                .into_iter()
                .map(|(what, bar, count)| LogEntry {
                    level: oxiguitar_plugin_api::message::Level::Warning,
                    message: format!(
                        "{what} can't be passed to {name}, made for an older OxiGuitar"
                    ),
                    place: format!("bar {bar}"),
                    count: u32::try_from(count).unwrap_or(u32::MAX),
                })
                .collect();
            let mut score = score.clone();
            score.keep_known(minor);
            Request::Write2 {
                format: format.spec.id.clone(),
                score: Box::new(score),
            }
        } else {
            let name = format.plugin.name();
            lost = score
                .extensions_used()
                .into_iter()
                .map(|(what, bar, count)| LogEntry {
                    level: oxiguitar_plugin_api::message::Level::Warning,
                    message: format!(
                        "{what} can't be passed to {name}, made for an older OxiGuitar"
                    ),
                    place: format!("bar {bar}"),
                    count: u32::try_from(count).unwrap_or(u32::MAX),
                })
                .collect();
            Request::Write {
                format: format.spec.id.clone(),
                score: Box::new(score.to_v1()),
            }
        };
        match self.request(format.index, &request)? {
            Answer::Written { data, mut losses } => {
                losses.extend(lost);
                Ok(WrittenFile {
                    data: data.into_vec(),
                    losses,
                })
            }
            other => Err(self.unexpected(format.index, &other)),
        }
    }

    /// Every instrument the ready plugins offer.
    pub fn instruments(&self) -> Vec<InstrumentRef<'_>> {
        self.ready()
            .flat_map(|(_, plugin, m)| {
                m.instruments
                    .iter()
                    .map(move |spec| InstrumentRef { plugin, spec })
            })
            .collect()
    }

    /// The instruments the player can use for tracks of `class`: those of
    /// plugins built for interface 1.3 or later (Mateus, 19 Oct 2026: the
    /// player ignores instruments declared before the instrument
    /// interface), in plugin order.
    pub fn playable(&self, class: InstrumentClass) -> Vec<InstrumentRef<'_>> {
        self.instruments_for(class)
            .into_iter()
            .filter(|i| i.plugin.manifest.as_ref().is_some_and(|m| m.knows_minor(3)))
            .collect()
    }

    /// The effects the ready plugins offer tracks of `class` (interface
    /// 1.3), in plugin order.
    pub fn effects_for(&self, class: InstrumentClass) -> Vec<InstrumentRef<'_>> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(3))
            .flat_map(|(_, plugin, m)| {
                m.effects
                    .iter()
                    .filter(move |spec| spec.plays(class))
                    .map(move |spec| InstrumentRef { plugin, spec })
            })
            .collect()
    }

    /// The effects for a track of `class` whose effects the user ordered
    /// as `order` (plugin ids, the track's own; empty for the default), in
    /// the order they run: see [`in_effect_order`].
    pub fn effects_in_order(
        &self,
        class: InstrumentClass,
        order: &[String],
    ) -> Vec<InstrumentRef<'_>> {
        in_effect_order(
            self.effects_for(class),
            order,
            |e| e.plugin.manifest.as_ref().map_or("", |m| m.id.as_str()),
            |e| e.spec.order,
        )
    }

    /// Prepares `instrument` (one of [`Plugins::playable`]) for
    /// `prepare.tracks`, loading its plugin if needed: off the audio
    /// thread, since the plugin loads its sounds now.
    pub fn prepare(
        &self,
        instrument: InstrumentRef<'_>,
        prepare: Prepare,
    ) -> Result<crate::realtime::Instance, Error> {
        let buffers = prepare.tracks.len() * 2;
        let prepare = Prepare {
            instrument: instrument.spec.id.clone(),
            ..prepare
        };
        self.prepared(
            instrument.plugin,
            &Request::Prepare(Box::new(prepare)),
            buffers,
        )
    }

    /// Prepares `effect` (one of [`Plugins::effects_for`]) for one track.
    pub fn prepare_effect(
        &self,
        effect: InstrumentRef<'_>,
        prepare: PrepareEffect,
    ) -> Result<crate::realtime::Instance, Error> {
        self.prepare_effect_following(effect, prepare, &[])
    }

    /// Whether `effect` follows the score (interface 1.8): it is prepared
    /// with its track's marked beats ([`Plugins::prepare_effect_following`]).
    pub fn follows_score(&self, effect: InstrumentRef<'_>) -> bool {
        effect.spec.follows_score
            && effect
                .plugin
                .manifest
                .as_ref()
                .is_some_and(|m| m.knows_minor(8))
    }

    /// Prepares `effect` for one track whose marked beats are played as
    /// `beats`. An effect that follows the score is told them, with the
    /// marks its interface version knows; any other is prepared as by
    /// [`Plugins::prepare_effect`].
    pub fn prepare_effect_following(
        &self,
        effect: InstrumentRef<'_>,
        prepare: PrepareEffect,
        beats: &[PlayedBeat],
    ) -> Result<crate::realtime::Instance, Error> {
        let prepare = PrepareEffect {
            effect: effect.spec.id.clone(),
            ..prepare
        };
        let request = if self.follows_score(effect) {
            let minor = effect
                .plugin
                .manifest
                .as_ref()
                .map_or(0, |m| m.interface.minor);
            let beats = beats
                .iter()
                .filter_map(|b| {
                    let ext: Vec<_> = b
                        .ext
                        .iter()
                        .filter(|e| Since::since(*e) <= minor)
                        .cloned()
                        .collect();
                    (!ext.is_empty()).then(|| PlayedBeat { ext, ..b.clone() })
                })
                .collect();
            Request::PrepareFollowing(Box::new(PrepareFollowing { prepare, beats }))
        } else {
            Request::PrepareEffect(Box::new(prepare))
        };
        self.prepared(effect.plugin, &request, 2)
    }

    fn prepared(
        &self,
        plugin: &Found,
        request: &Request<'_>,
        buffers: usize,
    ) -> Result<crate::realtime::Instance, Error> {
        let index = self.index_of(plugin);
        if index == usize::MAX {
            return Err(Error::not_found("that plugin isn't one of these"));
        }
        match self.request(index, request)? {
            Answer::Prepared {
                instance,
                latency,
                resolution,
            } => {
                let loaded = self.loaded(index)?;
                if !loaded.is_realtime() {
                    return Err(self.fail(
                        index,
                        "it prepared an instrument but has no real-time functions".to_owned(),
                    ));
                }
                Ok(crate::realtime::Instance::new(
                    loaded,
                    self.clone(),
                    index,
                    instance,
                    latency,
                    resolution,
                    buffers,
                ))
            }
            other => Err(self.unexpected(index, &other)),
        }
    }

    /// Sends `request` to the plugin at `index` (an instance's release).
    pub(crate) fn send_index(
        &self,
        index: usize,
        request: &Request<'_>,
    ) -> Result<Answer<'static>, Error> {
        self.request(index, request)
    }

    /// The instruments that play tracks of `class`: what the player offers
    /// a track (`InstrumentClass::of` gives a track's class).
    pub fn instruments_for(&self, class: InstrumentClass) -> Vec<InstrumentRef<'_>> {
        self.instruments()
            .into_iter()
            .filter(|i| i.spec.plays(class))
            .collect()
    }

    /// The names of the services declared for `interface` by usable
    /// plugins whose conditions hold here, in plugin order. Nothing is
    /// loaded or probed: a declared service may still turn out to be
    /// unavailable, which [`Plugins::service`] finds out on first use.
    pub fn declared_services(&self, interface: &str) -> Vec<String> {
        self.ready()
            .flat_map(|(_, _, m)| {
                m.services
                    .iter()
                    .filter(|s| {
                        s.interface == interface && s.conditions.unmet(&self.0.env).is_none()
                    })
                    .map(|s| service_name(s).to_owned())
            })
            .collect()
    }

    /// The service for `interface`: chosen on first use (see the module's
    /// documentation), then kept. The error says why there is none.
    pub fn service(&self, interface: &str) -> Result<ServiceRef<'_>, String> {
        let known = self.chosen().get(interface).cloned();
        let choice = match known {
            Some(choice) => choice,
            None => {
                // Chosen without the lock held: probing loads plugins, which
                // may ask for services themselves.
                let choice = self.choose(interface);
                self.chosen()
                    .entry(interface.to_owned())
                    .or_insert(choice)
                    .clone()
            }
        };
        let (index, id) = choice?;
        let plugin = &self.0.found[index];
        let spec = plugin
            .ready()
            .and_then(|m| {
                m.services
                    .iter()
                    .find(|s| s.interface == interface && s.id == id)
            })
            .ok_or_else(|| format!("the service {id:?} has gone"))?;
        Ok(ServiceRef { plugin, spec })
    }

    /// Calls `method` of the service chosen for `interface`. If the
    /// service's plugin fails, it is withdrawn and the call goes to the
    /// service chosen among the others, as if it had never been installed.
    pub fn call_service(
        &self,
        interface: &str,
        method: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        loop {
            let service = self
                .service(interface)
                .map_err(|why| Error::new(ErrorKind::NotFound, why))?;
            let index = self.index_of(service.plugin);
            let answer = self.request(
                index,
                &Request::Service {
                    interface: interface.to_owned(),
                    id: service.spec.id.clone(),
                    method: method.to_owned(),
                    input: Blob::borrowed(input),
                },
            );
            let error = match answer {
                Ok(Answer::Service(output)) => return Ok(output.into_vec()),
                Ok(other) => self.unexpected(index, &other),
                Err(e) => e,
            };
            // Withdrawn: each turn withdraws one plugin, so this ends.
            if service.plugin.failure().is_none() {
                return Err(error);
            }
        }
    }

    /// Sends `request` to `plugin` (one of [`Plugins::found`]), loading it
    /// first if needed: the interface without the conveniences above, for
    /// the contract tests and for interfaces the host doesn't wrap yet.
    pub fn send(&self, plugin: &Found, request: &Request<'_>) -> Result<Answer<'static>, Error> {
        let index = self.index_of(plugin);
        if index == usize::MAX {
            return Err(Error::not_found("that plugin isn't one of these"));
        }
        self.request(index, request)
    }

    /// The tools the ready plugins add to menus, in plugin order, each with
    /// its menu ([`ToolSpec::menu_key`]: the Plugins menu unless its
    /// manifest names another, interface 1.8). Palette and toolbar buttons
    /// without a menu are left out ([`Plugins::palette_tools`]).
    pub fn menu_tools(&self) -> Vec<(String, ToolRef<'_>)> {
        self.tools()
            .into_iter()
            .filter_map(|t| {
                let newer = t.plugin.manifest.as_ref().is_some_and(|m| m.knows_minor(8));
                let menu = if newer {
                    t.spec.menu_key()?
                } else {
                    // A plugin built before interface 1.8 names no place.
                    crate::api::manifest::PLUGINS_PLACE.to_lowercase()
                };
                Some((menu, t))
            })
            .collect()
    }

    /// The tools the ready plugins add to the editing palette (interface
    /// 1.8), in plugin order: buttons that run their tool
    /// ([`Plugins::run_tool`]).
    pub fn palette_tools(&self) -> Vec<ToolRef<'_>> {
        self.tools()
            .into_iter()
            .filter(|t| {
                t.spec.palette.is_some()
                    && t.plugin.manifest.as_ref().is_some_and(|m| m.knows_minor(8))
            })
            .collect()
    }

    /// The tools the ready plugins add to the toolbar's Plugins section
    /// (interface 1.8), in plugin order: buttons that run their tool
    /// ([`Plugins::run_tool`]).
    pub fn toolbar_tools(&self) -> Vec<ToolRef<'_>> {
        self.tools()
            .into_iter()
            .filter(|t| {
                t.spec.toolbar.is_some()
                    && t.plugin.manifest.as_ref().is_some_and(|m| m.knows_minor(8))
            })
            .collect()
    }

    /// Every tool the ready plugins add, in plugin order. Only plugins
    /// built for interface 1.2 or later have tools.
    pub fn tools(&self) -> Vec<ToolRef<'_>> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(2))
            .flat_map(|(index, plugin, m)| {
                m.tools.iter().map(move |spec| ToolRef {
                    plugin,
                    spec,
                    index,
                })
            })
            .collect()
    }

    /// The tools of the ready plugins that have keys (interface 1.8), each
    /// with its keys, the first in plugin order for each; and, for each
    /// tool whose keys an earlier tool has, a sentence for the footer.
    pub fn shortcut_tools(&self) -> (Vec<(Shortcut, ToolRef<'_>)>, Vec<String>) {
        let mut out: Vec<(Shortcut, ToolRef<'_>)> = Vec::new();
        let mut clashes = Vec::new();
        for tool in self.tools() {
            if !tool
                .plugin
                .manifest
                .as_ref()
                .is_some_and(|m| m.knows_minor(8))
            {
                continue;
            }
            let Some(keys) = tool.spec.shortcut.as_deref().and_then(Shortcut::parse) else {
                continue;
            };
            match out.iter().find(|(k, _)| *k == keys) {
                Some((_, first)) => clashes.push(format!(
                    "{}: {} has no keys: {} are {}'s {}.",
                    tool.plugin.name(),
                    tool.spec.name,
                    keys.label(),
                    first.plugin.name(),
                    first.spec.name
                )),
                None => out.push((keys, tool)),
            }
        }
        (out, clashes)
    }

    /// The ids of the ready plugins that play the notes their own way
    /// (interface 1.8), in plugin order: the order they take turns in.
    pub fn performers(&self) -> Vec<String> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(12) && m.performs)
            .map(|(_, _, m)| m.id.clone())
            .collect()
    }

    /// How the plugin `plugin` plays a track (interface 1.8): its answer to
    /// [`Request::Perform`], or the last one when it is given the same as
    /// last time. `None` leaves the notes as they are: a plugin that
    /// doesn't perform, can't, or answers notes outside the interface's
    /// limits (the footer says why).
    pub fn perform(&self, plugin: &str, track: Perform) -> Option<Vec<PlayedNote>> {
        let (_, found, _) = self
            .ready()
            .find(|(_, _, m)| m.id == plugin && m.knows_minor(12) && m.performs)?;
        match self.try_perform(plugin, track) {
            Ok(notes) => Some(notes),
            Err(e) => {
                if found.failure().is_none() {
                    let name = found.name();
                    self.0.core.notify(
                        Level::Warning,
                        format!("{name} couldn't play a track, so it plays as written: {e}"),
                    );
                }
                None
            }
        }
    }

    /// [`Plugins::perform`], with why it can't: notes outside the
    /// interface's limits are an [`ErrorKind::Invalid`] error.
    pub fn try_perform(&self, plugin: &str, track: Perform) -> Result<Vec<PlayedNote>, Error> {
        let (index, _, _) = self
            .ready()
            .find(|(_, _, m)| m.id == plugin && m.knows_minor(12) && m.performs)
            .ok_or_else(|| Error::not_found(format!("no ready plugin {plugin} plays notes")))?;
        let key = (index, track.track);
        let visits = track.visits.len();
        let request = Request::Perform(Box::new(track));
        let bytes = encode(&request).map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
        if let Some((last, notes)) = self
            .0
            .performed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            && *last == bytes
        {
            return Ok(notes.clone());
        }
        let notes = match self.request(index, &request)? {
            Answer::Performed(notes) => notes,
            other => return Err(self.unexpected(index, &other)),
        };
        let wrong = if notes.len() > PERFORMED_MAX {
            Some(format!(
                "it gave back {} notes; at most {PERFORMED_MAX}",
                notes.len()
            ))
        } else {
            notes.iter().find_map(|n| n.problem(visits))
        };
        if let Some(why) = wrong {
            return Err(Error::new(ErrorKind::Invalid, why));
        }
        self.0
            .performed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, (bytes, notes.clone()));
        Ok(notes)
    }

    /// Tells the plugin `plugin` the user clicked what it drew (interface
    /// 1.8), loading it if needed; returns what it says it did, a sentence
    /// for the footer (or nothing), as a tool does.
    pub fn drawing_clicked(&self, plugin: &str, click: Clicked) -> Result<String, Error> {
        let (index, _, _) = self
            .ready()
            .find(|(_, _, m)| m.id == plugin && m.knows_minor(13) && m.draws)
            .ok_or_else(|| Error::not_found(format!("no ready plugin {plugin} draws")))?;
        match self.request(index, &Request::Clicked(Box::new(click)))? {
            Answer::Report(report) => Ok(report),
            other => Err(self.unexpected(index, &other)),
        }
    }

    /// The ids of the ready plugins that draw on the page (interface 1.8;
    /// since 1.13 only those built for it, as `Request::Draw` grew then),
    /// in plugin order.
    pub fn drawers(&self) -> Vec<String> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(13) && m.draws)
            .map(|(_, _, m)| m.id.clone())
            .collect()
    }

    /// What the plugin `plugin` draws at `places` (interface 1.8): its
    /// answer to [`Request::Draw`], or the last one when the places are the
    /// same as last time. No places, no request: nothing is drawn. A
    /// plugin that can't draw, or answers with a drawing that breaks the
    /// interface's limits, draws nothing, and the footer says why.
    pub fn draw(&self, plugin: &str, places: Vec<DrawPlace>, times: DrawTimes) -> Vec<Drawn> {
        let Some((index, found, _)) = self
            .ready()
            .find(|(_, _, m)| m.id == plugin && m.knows_minor(13) && m.draws)
        else {
            return Vec::new();
        };
        if places.is_empty() {
            self.0
                .drawn
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&index);
            return Vec::new();
        }
        let count = places.len();
        let request = Request::Draw { places, times };
        let Ok(bytes) = encode(&request) else {
            return Vec::new();
        };
        if let Some((last, drawn)) = self
            .0
            .drawn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&index)
            && *last == bytes
        {
            return drawn.clone();
        }
        let drawn = match self.drawn_by(index, &request, count) {
            Ok(drawn) => drawn,
            Err(e) => {
                if found.failure().is_none() {
                    let name = found.name();
                    self.0
                        .core
                        .notify(Level::Warning, format!("{name} couldn't draw: {e}"));
                }
                Vec::new()
            }
        };
        self.0
            .drawn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(index, (bytes, drawn.clone()));
        drawn
    }

    /// Asks the plugin `plugin` to draw at `places` (interface 1.8), as
    /// [`Plugins::draw`] does but each time, with what goes wrong as an
    /// error: a drawing that breaks the interface's limits, or one at a
    /// place it wasn't given. For the contract suite.
    pub fn try_draw(
        &self,
        plugin: &str,
        places: Vec<DrawPlace>,
        times: DrawTimes,
    ) -> Result<Vec<Drawn>, Error> {
        let Some((index, _, _)) = self
            .ready()
            .find(|(_, _, m)| m.id == plugin && m.knows_minor(13) && m.draws)
        else {
            return Err(Error::not_found(format!(
                "no usable plugin {plugin:?} draws"
            )));
        };
        let count = places.len();
        self.drawn_by(index, &Request::Draw { places, times }, count)
    }

    fn drawn_by(
        &self,
        index: usize,
        request: &Request<'_>,
        count: usize,
    ) -> Result<Vec<Drawn>, Error> {
        match self.request(index, request)? {
            Answer::Drawn(drawn) => {
                for d in &drawn {
                    if let Some(why) = d.problem() {
                        return Err(Error::new(ErrorKind::Invalid, why));
                    }
                    if d.place as usize >= count {
                        return Err(Error::new(
                            ErrorKind::Invalid,
                            format!("it drew at place {}, and was given {count}", d.place),
                        ));
                    }
                }
                Ok(drawn)
            }
            other => Err(self.unexpected(index, &other)),
        }
    }

    /// Runs a plugin's tool, loading the plugin if needed; returns what it
    /// says it did, a sentence for the footer (or nothing).
    pub fn run_tool(&self, tool: ToolRef<'_>) -> Result<String, Error> {
        match self.request(
            tool.index,
            &Request::Tool {
                tool: tool.spec.id.clone(),
            },
        )? {
            Answer::Report(report) => Ok(report),
            other => Err(self.unexpected(tool.index, &other)),
        }
    }

    /// Runs `tool` as told by `options`, JSON text (interface 1.14): for a
    /// tool whose manifest has an options schema, in a plugin built for
    /// 1.14 or later, which another plugin runs. Returns its report.
    pub fn run_tool_with(&self, tool: ToolRef<'_>, options: String) -> Result<String, Error> {
        let knows = tool
            .plugin
            .manifest
            .as_ref()
            .is_some_and(|m| m.knows_minor(14));
        if !knows || tool.spec.options.is_none() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "the tool {:?} runs only as from its menu, without options",
                    tool.spec.id
                ),
            ));
        }
        match self.request(
            tool.index,
            &Request::Tool2 {
                tool: tool.spec.id.clone(),
                options,
            },
        )? {
            Answer::Report(report) => Ok(report),
            other => Err(self.unexpected(tool.index, &other)),
        }
    }

    /// Every tool there is, as plugins see it (interface 1.14): the core's
    /// own ([`Core::core_tools`]), then the usable plugins' in plugin
    /// order. A tool's options schema is listed only for a plugin built
    /// for 1.14 or later, the only ones that can run as told.
    pub fn tool_entries(&self) -> Vec<crate::api::message::ToolEntry> {
        let mut out = self.0.core.core_tools();
        out.extend(self.tools().into_iter().filter_map(|t| {
            let m = t.plugin.manifest.as_ref()?;
            Some(crate::api::message::ToolEntry {
                owner: m.id.clone(),
                id: t.spec.id.clone(),
                name: t.spec.name.clone(),
                description: t.spec.description.clone(),
                edits: t.spec.edits,
                options: t
                    .spec
                    .options
                    .as_ref()
                    .filter(|_| m.knows_minor(14))
                    .map(serde_json::Value::to_string),
            })
        }));
        out
    }

    /// Every export the ready plugins add to File → Export, in plugin
    /// order. Only plugins built for interface 1.4 or later have them.
    pub fn exporters(&self) -> Vec<ExporterRef<'_>> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(4))
            .flat_map(|(index, plugin, m)| {
                m.exporters.iter().map(move |spec| ExporterRef {
                    plugin,
                    spec,
                    index,
                })
            })
            .collect()
    }

    /// Starts an audio export with `exporter` (one of
    /// [`Plugins::exporters`], of kind audio), loading its plugin if
    /// needed.
    pub fn start_audio_export(&self, exporter: ExporterRef<'_>) -> Result<AudioExport, Error> {
        if exporter.spec.kind != ExportKind::Audio {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("{} isn't an audio export", exporter.spec.name),
            ));
        }
        let request = Request::ExportStart {
            exporter: exporter.spec.id.clone(),
        };
        match self.request(exporter.index, &request)? {
            Answer::ExportStarted { rate, data } => {
                if !(8_000..=384_000).contains(&rate) {
                    return Err(self.fail(
                        exporter.index,
                        format!("its export asked for the rate {rate}, outside 8000 to 384000"),
                    ));
                }
                Ok(AudioExport {
                    plugins: self.clone(),
                    index: exporter.index,
                    exporter: exporter.spec.id.clone(),
                    rate,
                    first: data.as_slice().to_vec(),
                    finished: false,
                })
            }
            other => Err(self.unexpected(exporter.index, &other)),
        }
    }

    /// Every tab the ready plugins add, in plugin order. Only plugins built
    /// for interface 1.1 or later have tabs.
    pub fn tabs(&self) -> Vec<TabRef<'_>> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(1))
            .flat_map(|(index, plugin, m)| {
                m.tabs.iter().map(move |spec| TabRef {
                    plugin,
                    spec,
                    index,
                })
            })
            .collect()
    }

    /// The tab `id`, if a ready plugin adds it.
    pub fn tab(&self, id: &TabId) -> Option<TabRef<'_>> {
        self.tabs().into_iter().find(|t| {
            t.spec.id == id.tab
                && t.plugin
                    .manifest
                    .as_ref()
                    .is_some_and(|m| m.id == id.plugin)
        })
    }

    /// What the tab `id` shows now, from its plugin (loaded first if
    /// needed). Contents the core can't show are an error that says why.
    pub fn tab_content(&self, id: &TabId) -> Result<TabContent, Error> {
        let tab = self.find_tab(id)?;
        self.tab_answer(
            tab,
            &Request::Tab {
                tab: id.tab.clone(),
            },
        )
    }

    /// Tells the tab's plugin that the user used its widget `widget`, and
    /// returns the tab's contents after it.
    pub fn tab_event(
        &self,
        id: &TabId,
        widget: &str,
        event: TabEvent,
    ) -> Result<TabContent, Error> {
        let tab = self.find_tab(id)?;
        let answer = self.tab_answer(
            tab,
            &Request::TabEvent {
                tab: id.tab.clone(),
                widget: widget.to_owned(),
                event,
            },
        );
        match answer {
            // An ordinary error (the plugin refused the event): said in the
            // footer, and the tab keeps showing what it shows now.
            Err(e) if tab.plugin.failure().is_none() => {
                let name = tab.plugin.name();
                self.0.core.notify(Level::Warning, format!("{name}: {e}"));
                self.tab_content(id)
            }
            answer => answer,
        }
    }

    /// Tells the dialog's plugin that the user closed it (interface 1.8),
    /// with OK or not. An error the plugin answers is said in the footer.
    pub fn tab_closed(&self, id: &TabId, ok: bool) {
        let Ok(tab) = self.find_tab(id) else {
            return;
        };
        let answered = self.request(
            tab.index,
            &Request::TabClosed {
                tab: id.tab.clone(),
                ok,
            },
        );
        if let Err(e) = answered
            && tab.plugin.failure().is_none()
        {
            let name = tab.plugin.name();
            self.0.core.notify(Level::Warning, format!("{name}: {e}"));
        }
    }

    fn find_tab(&self, id: &TabId) -> Result<TabRef<'_>, Error> {
        self.tab(id).ok_or_else(|| {
            Error::not_found(format!(
                "no usable plugin {:?} has a tab {:?}",
                id.plugin, id.tab
            ))
        })
    }

    fn tab_answer(&self, tab: TabRef<'_>, request: &Request<'_>) -> Result<TabContent, Error> {
        match self.request(tab.index, request)? {
            Answer::Tab(content) => match content.problems().into_iter().next() {
                None => Ok(*content),
                Some(problem) => Err(self.fail(
                    tab.index,
                    format!("its tab {} can't be shown ({problem})", tab.spec.title),
                )),
            },
            other => Err(self.unexpected(tab.index, &other)),
        }
    }

    /// The sections the ready plugins add to the track dialog for a track
    /// of class `class` (interface 1.3), in plugin order.
    pub fn track_sections(&self, class: InstrumentClass) -> Vec<SectionRef<'_>> {
        self.ready()
            .filter(|(_, _, m)| m.knows_minor(3))
            .flat_map(|(index, plugin, m)| {
                m.track_sections
                    .iter()
                    .filter(move |spec| spec.shows_for(class))
                    .map(move |spec| SectionRef {
                        plugin,
                        spec,
                        index,
                    })
            })
            .collect()
    }

    /// What `section` shows for `track`, whose settings for the section's
    /// plugin are `settings` (a JSON object), loading the plugin if needed:
    /// the contents and the settings the plugin hands back.
    pub fn track_section(
        &self,
        section: SectionRef<'_>,
        track: &TrackInfo,
        settings: &str,
    ) -> Result<(TabContent, String), Error> {
        self.section_answer(
            section,
            &Request::TrackSection {
                section: section.spec.id.clone(),
                track: track.clone(),
                settings: settings.to_owned(),
            },
        )
    }

    /// Tells the plugin of `section` that the user used its widget `widget`
    /// for `track`; returns the section's contents and the track's settings
    /// after it. An ordinary error is said in the footer, and the settings
    /// stay as they were.
    pub fn track_section_event(
        &self,
        section: SectionRef<'_>,
        track: &TrackInfo,
        settings: &str,
        widget: &str,
        event: TabEvent,
    ) -> Result<(TabContent, String), Error> {
        let answer = self.section_answer(
            section,
            &Request::TrackSectionEvent {
                section: section.spec.id.clone(),
                track: track.clone(),
                settings: settings.to_owned(),
                widget: widget.to_owned(),
                event,
            },
        );
        match answer {
            Err(e) if section.plugin.failure().is_none() => {
                let name = section.plugin.name();
                self.0.core.notify(Level::Warning, format!("{name}: {e}"));
                self.track_section(section, track, settings)
            }
            answer => answer,
        }
    }

    fn section_answer(
        &self,
        section: SectionRef<'_>,
        request: &Request<'_>,
    ) -> Result<(TabContent, String), Error> {
        match self.request(section.index, request)? {
            Answer::TrackSection { content, settings } => {
                let problem = content.problems().into_iter().next().or_else(|| {
                    match serde_json::from_str::<Value>(&settings) {
                        Ok(Value::Object(_)) => None,
                        _ => Some("its settings are not a JSON object".to_owned()),
                    }
                });
                match problem {
                    None => Ok((*content, settings)),
                    Some(problem) => Err(self.fail(
                        section.index,
                        format!(
                            "its track section {} can't be shown ({problem})",
                            section.spec.title
                        ),
                    )),
                }
            }
            other => Err(self.unexpected(section.index, &other)),
        }
    }

    /// Whether any ready plugin subscribes to events of `kind`, so the core
    /// can skip preparing events nobody hears.
    pub fn wants(&self, kind: EventKind) -> bool {
        self.ready().any(|(_, _, m)| m.wants(kind))
    }

    /// Pushes `event` to every ready plugin that subscribes to its kind,
    /// loading each one the first time. Returns at once: events are
    /// delivered in order, one at a time, on a thread of their own that is
    /// started with the first event anyone wants and sleeps until the next.
    /// A plugin that is slow to answer gets the latest selection, playback
    /// position and score revision rather than every one in between.
    pub fn push(&self, event: Event) {
        if !self.wants(event.kind()) {
            return;
        }
        if self.0.events.push(event) {
            let host = Arc::downgrade(&self.0);
            let queue = Arc::clone(&self.0.events);
            let started = std::thread::Builder::new()
                .name("oxiguitar-plugin-events".to_owned())
                .spawn(move || {
                    while let Some(event) = queue.next() {
                        if let Some(inner) = host.upgrade() {
                            Plugins(inner).deliver(&event);
                        }
                        queue.done();
                    }
                });
            if let Err(e) = started {
                self.0.events.close();
                self.0.core.notify(
                    Level::Error,
                    format!("Plugins can't be told what happens: {e}."),
                );
            }
        }
    }

    /// Waits until every event pushed so far has been delivered: for tests,
    /// and for a program that wants its plugins told before it ends. Never
    /// call it from a plugin's own event handler.
    pub fn wait_for_events(&self) {
        self.0.events.wait_idle();
    }

    fn deliver(&self, event: &Event) {
        let kind = event.kind();
        // The selection comes in two forms: plugins built for 1.2 or later
        // get the editor's, older ones the one they know.
        let for_version = |m: &Manifest| match event {
            Event::Selection(_) => !m.knows_minor(2),
            Event::Selection2(_) => m.knows_minor(2),
            _ => true,
        };
        let subscribers: Vec<usize> = self
            .ready()
            .filter(|(_, _, m)| m.wants(kind) && for_version(m))
            .map(|(index, _, _)| index)
            .collect();
        for index in subscribers {
            let answered = self.request(index, &Request::Event(event.clone()));
            let error = match answered {
                Ok(Answer::Done) => continue,
                Ok(other) => self.unexpected(index, &other),
                Err(e) => e,
            };
            if self.0.found[index].failure().is_some() {
                // Withdrawn, and the footer told.
                continue;
            }
            let mut failed = self
                .0
                .failed_events
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !failed.contains(&index) {
                failed.push(index);
                let name = self.0.found[index].name();
                let text = error
                    .message
                    .strip_prefix(&format!("{name}: "))
                    .unwrap_or(&error.message);
                self.0
                    .core
                    .notify(Level::Warning, format!("{name}: {text}"));
            }
        }
    }

    /// Loads every ready plugin that isn't loaded yet; for the contract
    /// tests. The errors name each plugin that
    /// couldn't start.
    pub fn load_all(&self) -> Vec<String> {
        self.ready()
            .filter_map(|(index, plugin, _)| {
                self.loaded(index)
                    .err()
                    .map(|e| format!("{}: {}", plugin.name(), e.message))
            })
            .collect()
    }

    fn ready(&self) -> impl Iterator<Item = (usize, &Found, &Manifest)> {
        // In safe mode nothing is ready, so nothing is ever loaded.
        let found = if self.0.safe {
            &[][..]
        } else {
            &self.0.found[..]
        };
        found
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.ready().map(|m| (i, f, m)))
    }

    fn index_of(&self, plugin: &Found) -> usize {
        self.0
            .found
            .iter()
            .position(|f| std::ptr::eq(f, plugin))
            .unwrap_or(usize::MAX)
    }

    fn chosen(&self) -> std::sync::MutexGuard<'_, HashMap<String, Choice>> {
        // Every change is one insert, so a panic elsewhere can't leave the
        // map half changed.
        self.0.chosen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn choose(&self, interface: &str) -> Choice {
        let mut candidates: Vec<(usize, &ServiceSpec)> = self
            .ready()
            .flat_map(|(i, _, m)| {
                m.services
                    .iter()
                    .filter(|s| {
                        s.interface == interface && s.conditions.unmet(&self.0.env).is_none()
                    })
                    .map(move |s| (i, s))
            })
            .collect();
        // Highest priority first; the sort is stable, so plugin order next.
        candidates.sort_by_key(|(_, s)| std::cmp::Reverse(s.priority));
        let mut reasons = Vec::new();
        let pinned = self
            .0
            .settings
            .get(SERVICES_KEY)
            .and_then(|s| s.get(interface).and_then(Value::as_str).map(str::to_owned));
        if let Some(pin) = pinned {
            let at = candidates.iter().position(|(i, s)| {
                self.0.found[*i]
                    .ready()
                    .is_some_and(|m| pin == format!("{}/{}", m.id, s.id))
            });
            match at {
                Some(at) => {
                    let c = candidates.remove(at);
                    candidates.insert(0, c);
                }
                None => reasons.push(format!(
                    "{pin}, chosen in settings.json, isn't installed or can't run here"
                )),
            }
        }
        if candidates.is_empty() && reasons.is_empty() {
            return Err(format!("no plugin provides {interface}"));
        }
        for (index, spec) in candidates {
            let name = format!("{} ({})", self.0.found[index].name(), service_name(spec));
            match self.request(
                index,
                &Request::Probe {
                    interface: interface.to_owned(),
                    id: spec.id.clone(),
                },
            ) {
                Ok(Answer::Probe(Availability::Available)) => return Ok((index, spec.id.clone())),
                Ok(Answer::Probe(Availability::Unavailable(why))) => {
                    reasons.push(format!("{name}: {why}"));
                }
                Ok(other) => reasons.push(self.unexpected(index, &other).message),
                Err(e) => reasons.push(format!("{name}: {}", e.message)),
            }
        }
        Err(format!(
            "nothing provides {interface} here ({})",
            reasons.join("; ")
        ))
    }

    /// The plugin at `index`, loaded and started on first use.
    fn loaded(&self, index: usize) -> Result<Loaded, Error> {
        let found = &self.0.found[index];
        let name = found.name();
        if self.0.safe {
            return Err(Error::new(
                ErrorKind::Failed,
                format!("{name} isn't used in safe mode"),
            ));
        }
        if let Some(why) = found.failure() {
            return Err(Error::new(
                ErrorKind::Failed,
                format!("{name} stopped working: {why}"),
            ));
        }
        if found.ready().is_none() {
            return Err(Error::new(
                ErrorKind::Failed,
                format!("{name} can't be used"),
            ));
        }
        let result = match found.loaded.get() {
            Some(result) => result,
            None => {
                if STARTING.with(|s| s.borrow().contains(&index)) {
                    return Err(Error::new(
                        ErrorKind::Failed,
                        format!("{name} asked for its own contributions while starting"),
                    ));
                }
                let result = found.loaded.get_or_init(|| {
                    STARTING.with(|s| s.borrow_mut().push(index));
                    let crash = crate::crash::register(found.crash_name());
                    let result = loader::load(
                        &found.path,
                        &found.manifest_bytes,
                        self.host_calls(index),
                        crash,
                    );
                    STARTING.with(|s| s.borrow_mut().retain(|&i| i != index));
                    result
                });
                if let Err(e) = result {
                    // A system library the plugin needs isn't installed (the
                    // PipeWire plugin without libpipewire): it can't run on
                    // this system, which isn't a failure to tell about in
                    // the footer; the plugin manager says why.
                    if let Some(library) = missing_library(e) {
                        return Err(self.unavailable(
                            index,
                            format!("it needs {library}, which isn't installed on this system"),
                        ));
                    }
                    return Err(self.fail(index, e.clone()));
                }
                result
            }
        };
        result
            .clone()
            .map_err(|e| Error::new(ErrorKind::Failed, format!("{name}: {e}")))
    }

    /// Sends `request` to the plugin at `index`.
    fn request(&self, index: usize, request: &Request<'_>) -> Result<Answer<'static>, Error> {
        let plugin = self.loaded(index)?;
        let bytes = encode(request).map_err(|e| Error::new(ErrorKind::Failed, e.to_string()))?;
        let (status, answer) = plugin.call(&bytes);
        // An error the plugin answered with is an answer; anything else
        // that isn't a readable answer (a crash, a request it couldn't
        // read, an error or an answer that can't be decoded) is a failure.
        let ordinary = status == Status::FAILED && decode::<Error>(&answer).is_ok();
        let answer = match outcome(status, &answer) {
            Ok(answer) => answer,
            Err(e) if ordinary => return Err(e),
            Err(e) => return Err(self.fail(index, e.message)),
        };
        decode::<Answer<'_>>(answer)
            .map(Answer::into_owned)
            .map_err(|e| self.fail(index, format!("its answer can't be read ({e})")))
    }

    /// Withdraws the plugin at `index` for the rest of the session, because
    /// `why`: from now on it is as if it weren't installed. Tells the
    /// footer and the window the first time. Returns the error for the
    /// call that found it.
    fn fail(&self, index: usize, why: String) -> Error {
        let found = &self.0.found[index];
        let name = found.name();
        if found.failed.set(why.clone()).is_ok() {
            // Forget the services chosen with it in the running, so the
            // next use chooses among the others.
            self.chosen()
                .retain(|_, choice| matches!(choice, Ok((i, _)) if *i != index));
            self.0.core.notify(
                Level::Error,
                format!("{name} stopped working and isn't used until OxiGuitar restarts: {why}."),
            );
            if let Some(m) = &found.manifest {
                self.0.core.withdrawn(&m.id);
            }
        }
        Error::new(ErrorKind::Failed, format!("{name} stopped working: {why}"))
    }

    /// Withdraws the plugin at `index` quietly: it can't run on this system
    /// (`why`), which the plugin manager shows; the footer isn't told.
    fn unavailable(&self, index: usize, why: String) -> Error {
        let found = &self.0.found[index];
        let name = found.name();
        if found.failed.set(why.clone()).is_ok() {
            self.chosen()
                .retain(|_, choice| matches!(choice, Ok((i, _)) if *i != index));
            if let Some(m) = &found.manifest {
                self.0.core.withdrawn(&m.id);
            }
        }
        Error::new(
            ErrorKind::Unsupported,
            format!("{name} can't run here: {why}"),
        )
    }

    /// A plugin answered with something it wasn't asked for: a broken
    /// contract, so it is withdrawn.
    pub(crate) fn unexpected(&self, index: usize, answer: &Answer<'_>) -> Error {
        let kind = match answer {
            Answer::Read { .. } => "a score",
            Answer::Written { .. } => "a file",
            Answer::Probe(_) => "a probe",
            Answer::Service(_) => "a service's output",
            Answer::Tab(_) => "a tab's contents",
            Answer::Done => "\"done\"",
            Answer::Read2 { .. } => "a score",
            Answer::Report(_) => "a report",
            Answer::TrackSection { .. } => "a track section",
            Answer::Prepared { .. } => "a prepared instrument",
            Answer::ExportStarted { .. } | Answer::Exported(_) | Answer::ExportFinished { .. } => {
                "an export's bytes"
            }
            Answer::Updated(_) => "an update's answer",
            Answer::Drawn(_) => "drawings",
            Answer::Performed(_) => "notes",
        };
        self.fail(
            index,
            format!("it answered with {kind}, which wasn't asked for"),
        )
    }

    /// What answers the requests of the plugin at `index`. It holds the
    /// host weakly: a loaded plugin outlives everything, the host need not.
    fn host_calls(&self, index: usize) -> Box<loader::HostCalls> {
        let host: Weak<Inner> = Arc::downgrade(&self.0);
        Box::new(move |request: &[u8]| match host.upgrade() {
            Some(inner) => Plugins(inner).answer(index, request),
            None => (
                Status::FAILED,
                encode(&Error::new(
                    ErrorKind::Failed,
                    "OxiGuitar's plugin host has closed",
                ))
                .unwrap_or_default(),
            ),
        })
    }

    /// Passes advice on to the window, unless the user silenced it or it
    /// was passed on already this session. Never in safe mode.
    fn advise(&self, manifest: &Manifest, given: oxiguitar_plugin_api::message::Advice) {
        if self.0.safe || advice::is_hidden(&self.0.settings, &manifest.id, &given.id) {
            return;
        }
        let first = self
            .0
            .advised
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((manifest.id.clone(), given.id.clone()));
        if first {
            self.0.core.advise(advice::Asked {
                plugin: manifest.id.clone(),
                plugin_name: manifest.name.clone(),
                advice: given,
            });
        }
    }

    /// Answers one request from the plugin at `index`.
    /// A plugin asks where the characters of `text` would be drawn
    /// (interface 1.14).
    fn measure(
        &self,
        text: &str,
        size: f32,
        font: crate::api::display::Font,
    ) -> Result<HostAnswer<'static>, Error> {
        if !(size.is_finite() && size > 0.0 && size <= crate::api::display::MAX_SIDE) {
            return Err(Error::new(
                ErrorKind::Invalid,
                format!("text can't be measured at size {size}"),
            ));
        }
        self.0
            .core
            .measure_text(text, size, font)
            .map(HostAnswer::Measured)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    "this program has no fonts to measure with",
                )
            })
    }

    /// The plugin `caller` runs `owner`'s tool `tool` (interface 1.14):
    /// checked here, run by the core.
    fn run_for(
        &self,
        caller: &str,
        owner: &str,
        tool: &str,
        options: Option<String>,
    ) -> Result<HostAnswer<'static>, Error> {
        let entry = self
            .tool_entries()
            .into_iter()
            .find(|t| t.owner == owner && t.id == tool)
            .ok_or_else(|| Error::not_found(format!("no usable tool {tool:?} of {owner:?}")))?;
        if options.is_some() && entry.options.is_none() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("the tool {tool:?} runs only as from its menu, without options"),
            ));
        }
        if let Some(o) = &options
            && !serde_json::from_str::<serde_json::Value>(o).is_ok_and(|v| v.is_object())
        {
            return Err(Error::new(
                ErrorKind::Invalid,
                format!("the options of {tool:?} must be a JSON object"),
            ));
        }
        self.0
            .core
            .run_tool(caller, owner, tool, options)
            .map(HostAnswer::Report)
            .map_err(|e| Error::new(ErrorKind::Failed, e))
    }

    fn answer(&self, index: usize, request: &[u8]) -> (Status, Vec<u8>) {
        let found = &self.0.found[index];
        let Some(manifest) = found.manifest.as_ref() else {
            return (Status::NOT_STARTED, Vec::new());
        };
        let request: HostRequest<'_> = match decode(request) {
            Ok(r) => r,
            Err(e) => return (Status::MALFORMED, e.to_string().into_bytes()),
        };
        let answered: Result<HostAnswer<'_>, Error> = match request {
            HostRequest::Notify { level, text } => {
                self.0
                    .core
                    .notify(level.into(), format!("{}: {text}", manifest.name));
                Ok(HostAnswer::Done)
            }
            HostRequest::Redraw { tab } => {
                if manifest.knows_minor(1) && manifest.tabs.iter().any(|t| t.id == tab) {
                    self.0.core.redraw(&TabId {
                        plugin: manifest.id.clone(),
                        tab,
                    });
                    Ok(HostAnswer::Done)
                } else {
                    Err(Error::not_found(format!(
                        "{} has no tab {tab:?} in its manifest",
                        manifest.name
                    )))
                }
            }
            HostRequest::OpenDialog { tab } => {
                let dialog = manifest
                    .tabs
                    .iter()
                    .find(|t| t.id == tab && t.area == Area::Dialog)
                    .filter(|_| manifest.knows_minor(8));
                match dialog {
                    Some(spec) => self
                        .0
                        .core
                        .open_dialog(
                            &TabId {
                                plugin: manifest.id.clone(),
                                tab,
                            },
                            &spec.title,
                        )
                        .map(|()| HostAnswer::Done)
                        .map_err(|why| Error::new(ErrorKind::Failed, why)),
                    None => Err(Error::not_found(format!(
                        "{} has no dialog {tab:?} in its manifest",
                        manifest.name
                    ))),
                }
            }
            HostRequest::ShowTab { tab } => {
                if manifest.knows_minor(8)
                    && manifest
                        .tabs
                        .iter()
                        .any(|t| t.id == tab && t.area != Area::Dialog)
                {
                    self.0.core.show_tab(&TabId {
                        plugin: manifest.id.clone(),
                        tab,
                    });
                    Ok(HostAnswer::Done)
                } else {
                    Err(Error::not_found(format!(
                        "{} has no tab {tab:?} in its manifest",
                        manifest.name
                    )))
                }
            }
            HostRequest::Engraving => {
                if manifest.knows_minor(8) {
                    Ok(HostAnswer::Engraving(self.0.core.engraving().map(Box::new)))
                } else {
                    Err(Error::new(
                        ErrorKind::Unsupported,
                        "the engraving needs interface 1.8",
                    ))
                }
            }
            HostRequest::Selection => {
                if manifest.knows_minor(8) {
                    Ok(HostAnswer::Selection(self.0.core.selection()))
                } else {
                    Err(Error::new(
                        ErrorKind::Unsupported,
                        "asking for the selection needs interface 1.8",
                    ))
                }
            }
            HostRequest::MeasureText { .. }
            | HostRequest::Copy { .. }
            | HostRequest::Tools
            | HostRequest::RunTool { .. }
            | HostRequest::Select(_)
                if !manifest.knows_minor(14) =>
            {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "measuring text, the clipboard, running tools and selecting need interface 1.14",
                ))
            }
            HostRequest::MeasureText { text, size, font } => self.measure(&text, size, font),
            HostRequest::Copy { text } => {
                self.0.core.copy(text);
                Ok(HostAnswer::Done)
            }
            HostRequest::Tools => Ok(HostAnswer::Tools(self.tool_entries())),
            HostRequest::RunTool {
                owner,
                tool,
                options,
            } => self.run_for(&manifest.id, &owner, &tool, options),
            HostRequest::Select(selection) => self
                .0
                .core
                .select(selection)
                .map(|()| HostAnswer::Done)
                .map_err(|e| Error::new(ErrorKind::Failed, e)),
            HostRequest::ScoreFile => {
                if manifest.knows_minor(13) {
                    Ok(HostAnswer::ScoreFile(
                        self.0
                            .core
                            .score_file()
                            .and_then(|p| p.to_str().map(str::to_owned)),
                    ))
                } else {
                    Err(Error::new(
                        ErrorKind::Unsupported,
                        "asking for the score's file needs interface 1.13",
                    ))
                }
            }
            HostRequest::Score => Ok(HostAnswer::Score(self.0.core.score().map(Box::new))),
            HostRequest::Score2 => {
                let (revision, score) = match self.0.core.score2() {
                    Some((revision, mut score)) => {
                        score.keep_known(manifest.interface.minor);
                        (revision, Some(Box::new(score)))
                    }
                    None => (0, None),
                };
                Ok(HostAnswer::Score2 { revision, score })
            }
            HostRequest::Apply {
                revision,
                name,
                score,
            } => self
                .0
                .core
                .apply(&manifest.name, revision, name, *score)
                .map(|()| HostAnswer::Done)
                .map_err(|why| Error::new(ErrorKind::Invalid, why)),
            HostRequest::Settings => {
                let mine = match self.0.settings.get(&manifest.id) {
                    Some(v @ Value::Object(_)) => v,
                    _ => Value::Object(serde_json::Map::new()),
                };
                Ok(HostAnswer::Settings(mine.to_string()))
            }
            HostRequest::SetSettings { json } => match serde_json::from_str::<Value>(&json) {
                Ok(v @ Value::Object(_)) => {
                    let changed = self.0.settings.get(&manifest.id).as_ref() != Some(&v);
                    let saved = self.0.settings.set(&manifest.id, v);
                    if changed && saved.is_ok() {
                        // What it played may follow its settings: ask again.
                        self.0
                            .performed
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .retain(|(i, _), _| *i != index);
                        self.0.core.settings_changed(&manifest.id);
                    }
                    saved
                        .map(|()| HostAnswer::Done)
                        .map_err(|e| Error::new(ErrorKind::Failed, e))
                }
                _ => Err(Error::new(
                    ErrorKind::Invalid,
                    "a plugin's settings must be a JSON object",
                )),
            },
            HostRequest::Notify2 { level, text, link } => {
                if is_https(&link.url) {
                    self.0.core.notify_with_link(
                        level.into(),
                        format!("{}: {text}", manifest.name),
                        link,
                    );
                    Ok(HostAnswer::Done)
                } else {
                    Err(Error::new(
                        ErrorKind::Invalid,
                        format!(
                            "a footer link must be an https: address, and {:?} isn't",
                            link.url
                        ),
                    ))
                }
            }
            HostRequest::Folders => self.plugin_folders(manifest).map(HostAnswer::Folders),
            HostRequest::Folder { name } => {
                self.folder_places(manifest, &name).map(HostAnswer::Folder)
            }
            HostRequest::Advise(given) => {
                if !manifest.knows_minor(4) {
                    Err(Error::new(
                        ErrorKind::Invalid,
                        format!(
                            "{} asks for interface 1.{}, and advice came with 1.4",
                            manifest.name, manifest.interface.minor
                        ),
                    ))
                } else if let Some(problem) = given.problem() {
                    Err(Error::new(ErrorKind::Invalid, problem))
                } else {
                    self.advise(manifest, given);
                    Ok(HostAnswer::Done)
                }
            }
            HostRequest::Service {
                interface,
                method,
                input,
            } => {
                if manifest.requires.contains(&interface) {
                    self.call_service(&interface, &method, input.as_slice())
                        .map(|output| HostAnswer::Service(Blob::owned(output)))
                } else {
                    Err(Error::not_found(format!(
                        "{} doesn't list {interface} under \"requires\" in its manifest",
                        manifest.name
                    )))
                }
            }
        };
        let (status, bytes) = match answered {
            Ok(answer) => (Status::OK, encode(&answer)),
            Err(error) => (Status::FAILED, encode(&error)),
        };
        match bytes {
            Ok(bytes) => (status, bytes),
            Err(e) => (Status::PANICKED, e.to_string().into_bytes()),
        }
    }
}

impl Plugins {
    /// Where the shared folder `name` of the plugin with `manifest` is
    /// (interface 1.8): the user's, created on the way, and the ones that
    /// exist beside the system's plugin folders.
    fn folder_places(&self, manifest: &Manifest, name: &str) -> Result<FolderPlaces, Error> {
        if !manifest.knows_minor(8) {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "asking for a folder by name needs interface 1.8",
            ));
        }
        if !manifest.folders.iter().any(|f| f == name) {
            return Err(Error::not_found(format!(
                "the manifest lists no folder {name:?}"
            )));
        }
        let Some(folders) = self.0.folders.get() else {
            return Err(Error::not_found(
                "this program keeps no folders for plugins",
            ));
        };
        let text = |path: &Path| {
            path.to_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("the folder {} is not UTF-8 text", path.display()),
                )
            })
        };
        let user = folders.named(name);
        if !self.0.read_only.load(std::sync::atomic::Ordering::Relaxed) {
            std::fs::create_dir_all(&user).map_err(|e| {
                Error::new(
                    ErrorKind::Failed,
                    format!("cannot create {}: {e}", user.display()),
                )
            })?;
        }
        Ok(FolderPlaces {
            user: text(&user)?,
            others: folders
                .named_elsewhere(name)
                .iter()
                .map(|p| text(p))
                .collect::<Result<_, _>>()?,
        })
    }

    /// The folders of the plugin with `manifest`, its own two created on
    /// the way.
    fn plugin_folders(&self, manifest: &Manifest) -> Result<PluginFolders, Error> {
        let Some(folders) = self.0.folders.get() else {
            return Err(Error::not_found(
                "this program keeps no folders for plugins",
            ));
        };
        let text = |path: &Path| {
            path.to_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("the folder {} is not UTF-8 text", path.display()),
                )
            })
        };
        let data = folders.plugin_data(&manifest.id);
        let cache = folders.plugin_cache(&manifest.id);
        let read_only = self.0.read_only.load(std::sync::atomic::Ordering::Relaxed);
        for dir in [&data, &cache].into_iter().filter(|_| !read_only) {
            std::fs::create_dir_all(dir).map_err(|e| {
                Error::new(
                    ErrorKind::Failed,
                    format!("cannot create {}: {e}", dir.display()),
                )
            })?;
        }
        // The shared folders are asked for by name (`HostRequest::Folder`):
        // the core knows none (interface 1.10, ADR 0012).
        Ok(PluginFolders {
            data: text(&data)?,
            cache: text(&cache)?,
            retired_shared: Vec::new(),
        })
    }
}

/// Whether `url` is an `https:` address with a host: the only links the
/// footer opens.
pub fn is_https(url: &str) -> bool {
    oxiguitar_plugin_api::message::is_https(url)
}

/// The system library a plugin couldn't be loaded without, from the
/// loader's error, if that was the reason: the dynamic linker's words on
/// Linux ("libpipewire-0.3.so.0: cannot open shared object file") and
/// macOS ("Library not loaded: …").
fn missing_library(error: &str) -> Option<String> {
    if let Some(at) = error.find(": cannot open shared object file") {
        let before = &error[..at];
        let start = before.rfind(['(', ' ']).map_or(0, |i| i + 1);
        return Some(before[start..].to_owned());
    }
    if let Some(at) = error.find("Library not loaded: ") {
        let rest = &error[at + "Library not loaded: ".len()..];
        let end = rest.find(['\n', ')', ' ']).unwrap_or(rest.len());
        return Some(rest[..end].to_owned());
    }
    None
}

/// Whether `c` is to be used rather than `b`, two usable files of one
/// plugin: a higher version, or the same version needing more processor
/// features (both are usable here, so this processor has them all: the
/// build made for it is the faster one). On a full tie neither is better,
/// and the folders decide (`Plugins::find`).
fn better(c: &Manifest, b: &Manifest) -> bool {
    (&c.version, c.conditions.cpu.len()) > (&b.version, b.conditions.cpu.len())
}

/// What a plugin file is, from its manifest alone.
fn examine(path: PathBuf, env: &Environment, disabled: &[String]) -> Found {
    let (manifest, bytes, state) = match oxp::read_manifest(&path) {
        Err(why) => (None, Vec::new(), State::Unusable(why)),
        Ok(file) => {
            let m = &file.manifest;
            let (major, minor) = oxiguitar_plugin_api::INTERFACE;
            let state = if m.interface.major != major {
                State::Unusable(format!(
                    "it is built for interface {}, and this OxiGuitar has {major}.{minor}",
                    m.interface
                ))
            } else if m.interface.minor > minor {
                State::Unusable(format!(
                    "it needs interface {}, and this OxiGuitar has {major}.{minor}: update OxiGuitar",
                    m.interface
                ))
            } else if let Some(why) = m.conditions.unmet(env) {
                State::Unusable(why)
            } else if disabled.contains(&m.id) {
                State::Disabled
            } else {
                State::Ready
            };
            (Some(file.manifest), file.bytes, state)
        }
    };
    Found {
        path,
        manifest,
        state,
        manifest_bytes: bytes,
        loaded: OnceLock::new(),
        failed: OnceLock::new(),
    }
}

/// What `replaces` and `needs` (interface 1.4) make of the plugins left
/// ready once each id has its file: a plugin whose id another one replaces
/// is that plugin's older self, whatever its version; then a plugin whose
/// needs aren't met is set aside.
/// `items` (a track's effects, in plugin order) in the order they run on
/// a track whose effects the user ordered as `order`, by plugin id
/// (`plugin` gives an item's): first the plugins `order` names, as it
/// names them; then the effects whose manifest gives them a place
/// (`stage`, interface 1.8: lower first, a pedal before the amp before the
/// equalizer); then the rest as found. A plugin's own effects keep their
/// order.
pub fn in_effect_order<T>(
    mut items: Vec<T>,
    order: &[String],
    plugin: impl Fn(&T) -> &str,
    stage: impl Fn(&T) -> Option<i32>,
) -> Vec<T> {
    let rank = |item: &T| {
        let id = plugin(item);
        if let Some(at) = order.iter().position(|o| o == id) {
            (0, i64::try_from(at).unwrap_or(i64::MAX))
        } else if let Some(stage) = stage(item) {
            (1, i64::from(stage))
        } else {
            (2, 0)
        }
    };
    // Stable: the rest keep the order found.
    items.sort_by_key(|item| rank(item));
    items
}

fn settle(found: &mut [Found]) {
    // A plugin whose id another one replaces (interface 1.4) is that
    // plugin's older self, whatever its version.
    for i in 0..found.len() {
        let Some(id) = found[i].ready().map(|m| m.id.clone()) else {
            continue;
        };
        let successor = (0..found.len()).find(|&j| {
            j != i
                && found[j]
                    .ready()
                    .is_some_and(|m| m.replaces.contains(&id) && m.id != id)
        });
        if let Some(j) = successor {
            found[i].state = State::Replaced(found[j].path.clone());
        }
    }
    // Then the plugins whose needs aren't met (interface 1.4), again
    // and again, since a plugin set aside can leave another one's need
    // unmet.
    loop {
        let unmet: Vec<(usize, String)> = (0..found.len())
            .filter_map(|i| Some((i, unmet_need(found[i].ready()?, found)?)))
            .collect();
        if unmet.is_empty() {
            break;
        }
        for (i, why) in unmet {
            found[i].state = State::Unmet(why);
        }
    }
}

/// Copies the folder `from`, with everything in it, to `to`, which mustn't
/// exist. Links are copied as what they point to.
fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if std::fs::metadata(&src)?.is_dir() {
            copy_folder(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Why `manifest`'s needs (interface 1.4) aren't met by the plugins in
/// `found`, or `None` when they are. A plugin meets a need for an id if it
/// has that id or replaces it, at the lowest version or later.
fn unmet_need(manifest: &Manifest, found: &[Found]) -> Option<String> {
    let answers = |m: &Manifest, id: &str| m.id == id || m.replaces.iter().any(|r| r == id);
    manifest.needs.iter().find_map(|need| {
        if found
            .iter()
            .filter_map(Found::ready)
            .any(|m| answers(m, &need.id) && m.version >= need.version)
        {
            return None;
        }
        // The best account of what is there instead.
        let there = found
            .iter()
            .filter(|f| f.manifest.as_ref().is_some_and(|m| answers(m, &need.id)))
            .max_by_key(|f| matches!(f.state, State::Ready));
        let wanted = format!("{} {} or later", need.id, need.version);
        Some(match there {
            None => format!("it needs the plugin {wanted}, which isn't installed"),
            Some(f) => {
                let m = f.manifest.as_ref().expect("filtered on its manifest");
                match &f.state {
                    State::Ready => format!(
                        "it needs the plugin {wanted}, and {} {} is installed",
                        m.name, m.version
                    ),
                    State::Disabled => {
                        format!("it needs the plugin {wanted}, which is disabled")
                    }
                    _ => format!("it needs the plugin {wanted}, which can't be used"),
                }
            }
        })
    })
}

fn service_name(spec: &ServiceSpec) -> &str {
    if spec.name.is_empty() {
        &spec.id
    } else {
        &spec.name
    }
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl From<oxiguitar_plugin_api::message::Level> for Level {
    fn from(level: oxiguitar_plugin_api::message::Level) -> Level {
        use oxiguitar_plugin_api::message::Level as L;
        match level {
            L::Info => Level::Info,
            L::Warning => Level::Warning,
            L::Error => Level::Error,
        }
    }
}

#[cfg(test)]
mod missing {
    #[test]
    fn a_missing_system_library_is_named() {
        assert_eq!(
            super::missing_library("it couldn't be loaded (libpipewire-0.3.so.0: cannot open shared object file: No such file or directory)").as_deref(),
            Some("libpipewire-0.3.so.0")
        );
        assert_eq!(
            super::missing_library("it couldn't be loaded (dlopen(x.oxp, 0x0005): Library not loaded: @rpath/libfoo.dylib\n  Referenced from: x)").as_deref(),
            Some("@rpath/libfoo.dylib")
        );
        assert_eq!(
            super::missing_library("it has no oxiguitar_plugin_v1 entry"),
            None
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders::tests::scratch;

    #[test]
    fn a_track_s_effects_run_in_its_own_order_then_by_their_manifests_then_as_found() {
        // An effect here is "plugin:effect", with the place its manifest
        // gives it after an @ when it gives one.
        fn order(found: &[&'static str], saved: &[&str]) -> Vec<&'static str> {
            let saved: Vec<String> = saved.iter().map(|s| (*s).to_owned()).collect();
            in_effect_order(
                found.to_vec(),
                &saved,
                |id: &&'static str| id.split([':', '@']).next().unwrap_or(""),
                |id: &&'static str| id.split_once('@').and_then(|(_, n)| n.parse().ok()),
            )
        }
        // Without an order of the track's own: the pedal, the amp, the
        // equalizer, as their manifests place them, then the rest as found.
        assert_eq!(
            order(
                &["me.echo", "a.eq@300", "you.verb", "a.amp@200", "a.wah@100"],
                &[]
            ),
            ["a.wah@100", "a.amp@200", "a.eq@300", "me.echo", "you.verb"]
        );
        // The track's order first; what it doesn't name after, as above.
        assert_eq!(
            order(
                &["me.echo", "a.eq@300", "you.verb", "a.amp@200"],
                &["you.verb", "a.eq", "gone.plugin"]
            ),
            ["you.verb", "a.eq@300", "a.amp@200", "me.echo"]
        );
        // A plugin's own effects stay together, in their order.
        assert_eq!(
            order(&["a:1", "b.eq@300", "a:2"], &["a"]),
            ["a:1", "a:2", "b.eq@300"]
        );
    }

    #[test]
    fn system_plugins_are_discovered() {
        let sys_dir = std::path::PathBuf::from("/usr/lib/oxiguitar/plugins");
        let Ok(entries) = std::fs::read_dir(&sys_dir) else {
            println!("System plugins directory not found, skipping");
            return;
        };
        let plugins: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();

        println!("Found {} system plugin files", plugins.len());
        for p in &plugins {
            println!("\n--- {} ---", p.file_name().unwrap().to_string_lossy());
            let env = Environment::current("wayland");
            let found = examine(p.clone(), &env, &[]);
            println!("  State: {:?}", found.state);
            if let Some(m) = &found.manifest {
                println!("  id={}, name={}, version={}", m.id, m.name, m.version);
            }
        }
    }

    #[test]
    fn the_highest_version_wins_then_the_build_for_this_processor() {
        let m = |version: &str, cpu: &[&str]| -> Manifest {
            let mut m: Manifest = serde_json::from_str(&format!(
                r#"{{"interface": "1.3", "id": "me.nam", "name": "NAM", "version": "{version}"}}"#
            ))
            .unwrap();
            m.conditions.cpu = cpu.iter().map(|f| (*f).to_owned()).collect();
            m
        };
        // A newer version wins even if it has no CPU features.
        assert!(better(&m("1.0.1", &[]), &m("1.0.0", &["avx2", "fma"])));
        // An older version loses even if it has more CPU features.
        assert!(!better(&m("1.0.0", &["avx2", "fma"]), &m("1.0.1", &[])));
        // Same version: the build needing more of this processor's
        // features (both usable here, so it has them), whatever the files'
        // folders and names.
        assert!(better(&m("1.0.0", &["avx2", "fma"]), &m("1.0.0", &[])));
        assert!(!better(&m("1.0.0", &[]), &m("1.0.0", &["avx2", "fma"])));
        // A full tie: neither is better; the folders decide.
        assert!(!better(&m("1.0.0", &[]), &m("1.0.0", &[])));
        assert!(!better(&m("1.0.0", &["avx2"]), &m("1.0.0", &["avx2"])));
    }

    /// A ready plugin file `file` with the manifest `json` (interface 1.4).
    fn ready(file: &str, json: &str) -> Found {
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert!(manifest.problems().is_empty(), "{:?}", manifest.problems());
        Found {
            path: PathBuf::from(file),
            manifest: Some(manifest),
            state: State::Ready,
            manifest_bytes: Vec::new(),
            loaded: OnceLock::new(),
            failed: OnceLock::new(),
        }
    }

    const MIDI: &str = r#"{"interface": "1.4", "id": "oxiguitar.midi", "name": "MIDI",
        "version": "1.0.0", "replaces": ["oxiguitar.soundfont"]}"#;
    const SOUNDFONT: &str = r#"{"interface": "1.4", "id": "oxiguitar.soundfont",
        "name": "SoundFont", "version": "0.11.0"}"#;

    fn nam(needs: &str) -> String {
        format!(
            r#"{{"interface": "1.4", "id": "oxiguitar.nam", "name": "NAM", "version": "1.0.0",
            "needs": [{needs}]}}"#
        )
    }

    fn states(found: &[Found]) -> Vec<&State> {
        found.iter().map(|f| &f.state).collect()
    }

    #[test]
    fn a_replaced_id_is_the_older_self_of_the_plugin_replacing_it() {
        // Whatever the versions: the old plugin's is higher here.
        let mut found = vec![
            ready("soundfont.oxp", SOUNDFONT),
            ready("midi.oxp", MIDI),
            ready(
                "nam.oxp",
                &nam(r#"{"id": "oxiguitar.midi", "version": "1.0.0"}"#),
            ),
        ];
        settle(&mut found);
        assert_eq!(
            states(&found),
            [
                &State::Replaced("midi.oxp".into()),
                &State::Ready,
                &State::Ready
            ]
        );
    }

    #[test]
    fn a_need_is_met_by_the_plugin_or_its_successor_at_the_version() {
        // NAM built against the old id still finds MIDI.
        let mut found = vec![
            ready("midi.oxp", MIDI),
            ready(
                "nam.oxp",
                &nam(r#"{"id": "oxiguitar.soundfont", "version": "0.11.0"}"#),
            ),
        ];
        settle(&mut found);
        assert_eq!(states(&found), [&State::Ready, &State::Ready]);

        let mut found = vec![
            ready("midi.oxp", MIDI),
            ready(
                "nam.oxp",
                &nam(r#"{"id": "oxiguitar.midi", "version": "1.2.0"}"#),
            ),
        ];
        settle(&mut found);
        assert_eq!(
            found[1].state,
            State::Unmet(
                "it needs the plugin oxiguitar.midi 1.2.0 or later, and MIDI 1.0.0 is installed"
                    .into()
            )
        );
    }

    #[test]
    fn a_missing_or_unusable_need_sets_the_plugin_aside_and_those_needing_it() {
        let mut found = vec![ready(
            "nam.oxp",
            &nam(r#"{"id": "oxiguitar.midi", "version": "1.0.0"}"#),
        )];
        settle(&mut found);
        assert_eq!(
            found[0].state,
            State::Unmet(
                "it needs the plugin oxiguitar.midi 1.0.0 or later, which isn't installed".into()
            )
        );

        // MIDI disabled: NAM can't be used, and a plugin needing NAM
        // neither, though NAM itself is installed.
        let mut found = vec![
            ready("midi.oxp", MIDI),
            ready(
                "nam.oxp",
                &nam(r#"{"id": "oxiguitar.midi", "version": "1.0.0"}"#),
            ),
            ready(
                "pedals.oxp",
                r#"{"interface": "1.4", "id": "me.pedals", "name": "Pedals", "version": "0.1.0",
                "needs": [{"id": "oxiguitar.nam", "version": "1.0.0"}]}"#,
            ),
        ];
        found[0].state = State::Disabled;
        settle(&mut found);
        assert_eq!(
            found[1].state,
            State::Unmet(
                "it needs the plugin oxiguitar.midi 1.0.0 or later, which is disabled".into()
            )
        );
        assert_eq!(
            found[2].state,
            State::Unmet(
                "it needs the plugin oxiguitar.nam 1.0.0 or later, which can't be used".into()
            )
        );
    }

    #[test]
    fn a_replaced_plugin_s_settings_and_data_are_carried_over_once() {
        let home = scratch("carry-over");
        let folders = Folders::resolve(
            crate::System::Linux,
            |name| (name == crate::HOME_VARIABLE).then(|| home.clone().into_os_string()),
            None,
        )
        .unwrap();
        let old = folders.plugin_data("oxiguitar.soundfont");
        std::fs::create_dir_all(old.join("sets")).unwrap();
        std::fs::write(old.join("sets").join("list.json"), b"[]").unwrap();
        let settings = SharedSettings::new(
            crate::Settings::parse(r#"{"oxiguitar.soundfont": {"basses": "/b.sf2"}}"#).unwrap(),
            Some(folders.settings()),
        );
        let plugins = Plugins(Arc::new(Inner {
            found: vec![ready("midi.oxp", MIDI)],
            safe: false,
            env: Environment::current("x11"),
            settings: settings.clone(),
            core: Arc::new(Unheard),
            chosen: Mutex::new(HashMap::new()),
            events: Arc::new(Queue::default()),
            failed_events: Mutex::new(Vec::new()),
            advised: Mutex::new(HashSet::new()),
            folders: OnceLock::new(),
            read_only: std::sync::atomic::AtomicBool::new(false),
            drawn: Mutex::new(HashMap::new()),
            performed: Mutex::new(HashMap::new()),
        }));
        assert_eq!(
            plugins.replaced_ids(),
            [(
                "oxiguitar.soundfont".to_owned(),
                "oxiguitar.midi".to_owned()
            )]
        );
        let told = plugins.carry_over(&folders);
        assert_eq!(told.len(), 2, "{told:?}");
        assert_eq!(
            settings.get("oxiguitar.midi"),
            Some(serde_json::json!({"basses": "/b.sf2"}))
        );
        // Saved, and the old object kept for an older OxiGuitar.
        let saved = crate::Settings::load(&folders.settings()).unwrap();
        assert!(saved.get("oxiguitar.midi").is_some());
        assert!(saved.get("oxiguitar.soundfont").is_some());
        let new = folders.plugin_data("oxiguitar.midi");
        assert_eq!(
            std::fs::read(new.join("sets").join("list.json")).unwrap(),
            b"[]"
        );
        assert!(old.join("sets").join("list.json").is_file());

        // Once: the new id's own settings and data are never overwritten.
        settings
            .set("oxiguitar.midi", serde_json::json!({"basses": "/new.sfz"}))
            .unwrap();
        std::fs::write(new.join("sets").join("list.json"), b"[1]").unwrap();
        assert!(plugins.carry_over(&folders).is_empty());
        assert_eq!(
            settings.get("oxiguitar.midi"),
            Some(serde_json::json!({"basses": "/new.sfz"}))
        );
        assert_eq!(
            std::fs::read(new.join("sets").join("list.json")).unwrap(),
            b"[1]"
        );
    }

    fn plain(folders: &[PathBuf]) -> Plugins {
        Plugins::discover(folders, SharedSettings::default(), "x11", Arc::new(Unheard))
    }

    #[test]
    fn files_that_are_not_plugins_are_reported_not_loaded() {
        let dir = scratch("plugins-broken");
        std::fs::write(dir.join("junk.oxp"), b"not a library").unwrap();
        let plugins = plain(std::slice::from_ref(&dir));
        assert_eq!(plugins.found().len(), 1);
        let found = &plugins.found()[0];
        assert_eq!(found.name(), "junk.oxp");
        assert!(matches!(&found.state, State::Unusable(why) if why.contains("not a plugin")));
        assert!(plugins.readers().is_empty());
        assert!(plugins.format_for(b"anything").is_none());
        assert_eq!(
            plugins.service("audio-device").unwrap_err(),
            "no plugin provides audio-device"
        );
        assert!(plugins.load_all().is_empty());
        assert!(!found.is_loaded());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_mode_lists_every_file_and_uses_none() {
        let dir = scratch("plugins-safe");
        std::fs::write(dir.join("junk.oxp"), b"not a library").unwrap();
        let plugins = Plugins::discover_in_safe_mode(
            std::slice::from_ref(&dir),
            SharedSettings::default(),
            "x11",
            Arc::new(Unheard),
        );
        assert!(plugins.safe_mode());
        assert_eq!(plugins.found().len(), 1);
        assert_eq!(
            plugins.recognise(b"anything", Some("gp5")).unwrap_err(),
            Unrecognised::SafeMode
        );
        assert!(Unrecognised::SafeMode.to_string().contains("--safe-mode"));
        assert!(plugins.tabs().is_empty() && plugins.load_all().is_empty());
        assert!(!plain(std::slice::from_ref(&dir)).safe_mode());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_folders_no_plugins() {
        let plugins = plain(&[]);
        assert!(plugins.found().is_empty());
        assert!(plugins.instruments_for(InstrumentClass::Drums).is_empty());
    }
}
