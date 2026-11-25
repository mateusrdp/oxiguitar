// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The worker thread: everything slow happens here, so the window never
//! waits. It reads files, engraves, draws tiles, runs tools and writes PDFs,
//! and reports back through events.
//!
//! Requests arrive in order, but only the newest engraving and the newest
//! set of tiles matter: older ones still queued are skipped, and a batch of
//! tiles stops as soon as a newer request arrives. The window simply asks
//! again for whatever tiles it still lacks.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use oxiguitar_display::{Rect, Theme};
use oxiguitar_edit::{Caret, Command, Selection, StaffBar};
use oxiguitar_host::api::event::{Event as PluginEvent, Selection as PluginSelection, Selection2};
use oxiguitar_host::{Level, Plugins};
use oxiguitar_layout::Hits;
use oxiguitar_model::Score;
use oxiguitar_render::Raster;

use crate::canvas::Sheets;
#[cfg(test)]
use crate::canvas::TILE;
use crate::document::{
    Document, EditState, Engraver, OpenScore, Prepared, SaveFormat, Settings, ToolEntry, TrackInfo,
    file_name,
};

/// What the window asks of the worker.
#[derive(Debug)]
pub(crate) enum Request {
    /// Read a score file.
    Open(PathBuf),
    /// File → New: a new score, not saved yet.
    New,
    /// Engrave the open score with these settings.
    Engrave(Settings),
    /// Draw tiles of engraving `engraving` (skipped if that is no longer the
    /// newest) at `scale` device pixels per point.
    Tiles {
        engraving: u64,
        paint: u64,
        scale: f32,
        dark: bool,
        tiles: Vec<(u32, u32)>,
    },
    /// Apply the Tools menu's tool with this index, as one undoable
    /// command.
    ApplyTool(usize),
    /// Note entry: a command typed at the caret, on the selection where
    /// the command takes one.
    Edit {
        command: Command,
        caret: Caret,
        selection: Option<Selection>,
    },
    /// Edit → Undo.
    Undo,
    /// Edit → Redo.
    Redo,
    /// The caret or the selection moved: tell the plugins that listen, in
    /// both forms (interface 1.1's, and 1.2's for plugins built for it).
    Selection(Option<PluginSelection>, Option<Selection2>),
    /// Write the shown tracks as a PDF.
    ExportPdf(PathBuf, Settings),
    /// File → Save (`None`): the file the score came from, in its own
    /// format unless that loses part of the score; File → Save As: this
    /// file, in the format with this id. Asks first ([`Event::AskSave`])
    /// when something would be lost or the file changes.
    Save(Option<(PathBuf, String)>),
    /// The user agreed to the save asked about; with `keep`, what the
    /// format can't hold goes in the file beside the score (FILE.oxg).
    ConfirmSave { keep: bool },
    /// The user declined it.
    CancelSave,
    /// A plugin handed back the score changed, made from `revision`
    /// (interface 1.2): one change the user can undo, named `name`.
    /// `reply`, when there is one, hears whether it was made (interface
    /// 1.14: the plugin then reads the score as changed).
    PluginEdit {
        plugin: String,
        revision: u64,
        name: String,
        score: Box<Score>,
        reply: Option<Sender<Result<(), String>>>,
    },
    /// Run a plugin's tool from the Tools menu: the plugin's id, the
    /// tool's id.
    PluginTool(String, String),
    /// Run a plugin's tool for another plugin (interface 1.14): with
    /// `options` (JSON text) as told, without as from its menu; `reply`
    /// hears its report, or why it didn't run.
    RunPluginTool {
        plugin: String,
        tool: String,
        options: Option<String>,
        reply: Sender<Result<String, String>>,
    },
    /// Every request before this one is done: `reply` hears whether the
    /// score changed since revision `before`, as the report of the core's
    /// tool `name`, which a plugin ran (interface 1.14).
    Fence {
        before: u64,
        name: String,
        reply: Sender<Result<String, String>>,
    },
    /// A click on what a plugin drew on the page (interface 1.8): the
    /// drawing's tag and place, where in its box (staff spaces) and
    /// whether it was a double click.
    DrawingClicked {
        tag: u32,
        at: oxiguitar_layout::DrawnAt,
        x: f32,
        y: f32,
        double: bool,
    },
    /// End the worker.
    Stop,
}

/// What the worker tells the window.
pub(crate) enum Event {
    /// A score was opened (and replaces the previous one).
    Opened {
        /// The score, shared read-only with the window for the caret.
        score: Arc<Score>,
        edit: EditState,
        /// The bars that don't add up.
        bad: Vec<StaffBar>,
        tracks: Vec<TrackInfo>,
        tools: Vec<ToolEntry>,
        pdf_name: String,
        folder: Option<PathBuf>,
        /// The file's name, for the window's title.
        name: String,
        /// A new score with no file yet (File → New).
        untitled: bool,
    },
    /// A save waits for the user: a heading and lines to show, with Save
    /// and Cancel ([`Request::ConfirmSave`], [`Request::CancelSave`]).
    AskSave {
        heading: String,
        lines: Vec<String>,
        /// The format can't hold everything: the question also offers to
        /// keep the rest in the file beside the score.
        can_keep: bool,
    },
    /// The score was saved, perhaps under another name.
    Saved {
        name: String,
        edit: EditState,
        pdf_name: String,
        folder: Option<PathBuf>,
    },
    /// The save didn't happen; the reason is in the footer.
    SaveFailed,
    /// A new engraving: where each sheet sits on the canvas, in points,
    /// and where every staff, bar and beat is on the sheets.
    Engraved {
        engraving: u64,
        sheets: Vec<Rect>,
        hits: Arc<Hits>,
    },
    /// The score changed (an edit, a tool, an undo, a redo).
    Changed {
        score: Arc<Score>,
        edit: EditState,
        bad: Vec<StaffBar>,
        /// Where the caret goes after an edit typed at it.
        caret: Option<Caret>,
        /// The tracks again, when the edit added, removed, moved, renamed
        /// or restrung any, with where each was before ([`was`]).
        tracks: Option<(Vec<TrackInfo>, Vec<Option<usize>>)>,
        /// Whether the change was a note insertion or deletion (for the
        /// engraved-beat preview toggle, #246).
        note_edit: bool,
        /// The change was the window's own mixer moves (`Command::Mix`),
        /// which the window and the sound already have: nothing to engrave
        /// or play again.
        mixer: bool,
    },
    /// One tile, premultiplied RGBA, `canvas::TILE` pixels square.
    Tile {
        engraving: u64,
        paint: u64,
        col: u32,
        row: u32,
        rgba: Vec<u8>,
    },
    /// The Tools menu changed (after a tool ran).
    Tools(Vec<ToolEntry>),
    /// A message for the footer: an error, a warning, or information the
    /// footer doesn't keep.
    Message(Level, String),
    /// A plugin's report of what the user just asked it to do (a tool's
    /// run, a click on its drawing): the footer shows it.
    Report(String),
    /// An edit was refused: the sound runs (no editing while playing).
    Refused,
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Opened { tracks, .. } => write!(f, "Opened({} tracks)", tracks.len()),
            Event::Engraved {
                engraving, sheets, ..
            } => {
                write!(f, "Engraved({engraving}, {} sheets)", sheets.len())
            }
            Event::Tile { col, row, .. } => write!(f, "Tile({col}, {row})"),
            Event::Changed { bad, note_edit, .. } => {
                write!(f, "Changed({} bad bars, note_edit={note_edit})", bad.len())
            }
            Event::Tools(t) => write!(f, "Tools({})", t.len()),
            Event::Message(l, s) => write!(f, "Message({l:?}, {s:?})"),
            Event::Report(s) => write!(f, "Report({s:?})"),
            Event::AskSave { heading, .. } => write!(f, "AskSave({heading:?})"),
            Event::Saved { name, .. } => write!(f, "Saved({name:?})"),
            Event::SaveFailed => write!(f, "SaveFailed"),
            Event::Refused => write!(f, "Refused"),
        }
    }
}

/// The worker thread and the way to reach it.
pub(crate) struct Worker {
    requests: Sender<Request>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the worker, which opens files through `plugins`, keeps
    /// `open` up to date for plugins that ask for the score, and tells the
    /// plugins that subscribe when a score is opened or changed; `sink`
    /// receives its events, on the worker thread.
    pub(crate) fn start(
        plugins: Plugins,
        open: OpenScore,
        sink: impl Fn(Event) + Send + 'static,
    ) -> Worker {
        let (requests, rx) = channel();
        open.connect(requests.clone());
        let thread = std::thread::Builder::new()
            .name("oxiguitar-worker".to_owned())
            .spawn(move || run(&rx, plugins, open, &sink))
            .ok();
        Worker { requests, thread }
    }

    /// Queues a request. Never blocks.
    pub(crate) fn send(&self, request: Request) {
        let _ = self.requests.send(request);
    }

    /// A sender for other threads (such as a file dialog's).
    pub(crate) fn sender(&self) -> Sender<Request> {
        self.requests.clone()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Other threads may still hold senders, so ask the loop to end
        // rather than waiting for the channel to close.
        let _ = self.requests.send(Request::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct State {
    plugins: Plugins,
    engraver: Engraver,
    doc: Option<Document>,
    /// The score plugins may ask for, and its revision: a number that
    /// changes with every score opened or changed this session.
    open: OpenScore,
    revision: u64,
    settings: Settings,
    sheets: Sheets,
    engraving: u64,
    raster: Raster,
    /// A save waiting for the user's answer.
    pending_save: Option<Prepared>,
    /// An edit left its engraving to the edit after it.
    stale: bool,
    /// What the window's track list shows of each track (name, sound,
    /// strings), to send the list again when an edit changes it.
    track_shape: Vec<(String, u8, bool, usize)>,
}

/// What the track list shows of each track.
fn track_shape(score: &Score) -> Vec<(String, u8, bool, usize)> {
    score
        .tracks
        .iter()
        .map(|t| {
            (
                t.name.clone(),
                t.instrument.midi_program,
                t.instrument.percussion,
                t.staves
                    .first()
                    .map_or(0, oxiguitar_model::Staff::string_count),
            )
        })
        .collect()
}

fn run(rx: &Receiver<Request>, plugins: Plugins, open_score: OpenScore, sink: &dyn Fn(Event)) {
    let engraver = match Engraver::new() {
        Ok(e) => e,
        Err(e) => {
            sink(Event::Message(Level::Error, e));
            return;
        }
    };
    let mut st = State {
        plugins,
        engraver,
        doc: None,
        open: open_score,
        revision: 0,
        settings: Settings::default(),
        stale: false,
        sheets: Sheets::default(),
        engraving: 0,
        raster: Raster::new(),
        pending_save: None,
        track_shape: Vec::new(),
    };
    let mut pending: VecDeque<Request> = VecDeque::new();
    loop {
        if pending.is_empty() {
            match rx.recv() {
                Ok(r) => pending.push_back(r),
                Err(_) => return,
            }
        }
        pending.extend(rx.try_iter());
        let Some(request) = pending.pop_front() else {
            continue;
        };
        let superseded = |p: &VecDeque<Request>, tiles: bool| {
            p.iter().any(|r| match r {
                Request::Open(_)
                | Request::New
                | Request::Engrave(_)
                | Request::ApplyTool(_)
                | Request::Edit { .. }
                | Request::Undo
                | Request::Redo => true,
                Request::Selection(..) => false,
                Request::Stop => true,
                Request::Tiles { .. } => tiles,
                Request::PluginEdit { .. }
                | Request::PluginTool(..)
                | Request::RunPluginTool { .. }
                | Request::DrawingClicked { .. } => true,
                Request::Fence { .. } => false,
                Request::ExportPdf(..)
                | Request::Save(_)
                | Request::ConfirmSave { .. }
                | Request::CancelSave => false,
            })
        };
        // No editing while playing: the window and plugins alike are
        // refused until the sound is paused or stopped. Only what plugins
        // keep for a track (their sections of the Track tab) changes while
        // the song plays: the effects follow it (#232). So does the mixer,
        // which the sound follows from the window as it moves.
        let edits = match &request {
            Request::ApplyTool(_)
            | Request::Edit { .. }
            | Request::Undo
            | Request::Redo
            | Request::PluginEdit { .. }
            | Request::DrawingClicked { .. } => true,
            // A tool that says it doesn't edit the score (interface 1.9),
            // such as one opening a plugin's settings, runs while it plays.
            Request::PluginTool(plugin, tool) | Request::RunPluginTool { plugin, tool, .. } => {
                st.plugins.tools().iter().any(|t| {
                    t.spec.id == *tool
                        && t.spec.edits
                        && t.plugin.manifest.as_ref().is_some_and(|m| m.id == *plugin)
                })
            }
            _ => false,
        };
        let settings_only = match &request {
            Request::Edit {
                command: Command::Mix { .. },
                ..
            } => true,
            Request::Edit { command, caret, .. } => st
                .doc
                .as_ref()
                .is_some_and(|d| d.plugin_settings_only(command, *caret)),
            _ => false,
        };
        if edits && !settings_only && st.open.playing() {
            let playing = "the score is playing: it can be changed once it is paused or stopped";
            match &request {
                Request::RunPluginTool { reply, .. } => {
                    let _ = reply.send(Err(playing.to_owned()));
                }
                Request::PluginEdit {
                    reply: Some(reply), ..
                } => {
                    let _ = reply.send(Err(playing.to_owned()));
                }
                _ => {}
            }
            sink(Event::Refused);
            continue;
        }
        match request {
            Request::Stop => return,
            Request::Open(path) => open(&mut st, path, sink),
            Request::New => {
                let doc = Document::untitled(&st.plugins);
                opened(&mut st, doc, None, sink);
            }
            Request::Engrave(settings) => {
                st.settings = settings;
                if !superseded(&pending, false) {
                    engrave(&mut st, sink);
                }
            }
            Request::Tiles {
                engraving,
                paint,
                scale,
                dark,
                tiles,
            } => {
                if engraving != st.engraving || superseded(&pending, true) {
                    continue;
                }
                let theme = if dark { Theme::DARK } else { Theme::LIGHT };
                for (col, row) in tiles {
                    let Some(px) = st.sheets.draw_tile(
                        &mut st.raster,
                        &st.engraver.fonts,
                        &theme,
                        col,
                        row,
                        scale,
                    ) else {
                        continue;
                    };
                    sink(Event::Tile {
                        engraving,
                        paint,
                        col,
                        row,
                        rgba: px.take(),
                    });
                    pending.extend(rx.try_iter());
                    if superseded(&pending, true) {
                        break;
                    }
                }
            }
            Request::ApplyTool(i) => {
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                let (report, changed) = doc.apply_tool(i);
                sink(Event::Tools(doc.tools()));
                sink(Event::Report(report));
                if changed && changed_score(&mut st, None, false, false, sink) {
                    st.stale = true;
                } else {
                    engrave(&mut st, sink);
                }
            }
            Request::Edit {
                command: Command::Mix { track, mix },
                caret,
                ..
            } => {
                // A slider dragged sends many: the ones already waiting
                // join this one, and the score is told about once.
                let mut moves = vec![(track, mix)];
                pending.extend(rx.try_iter());
                while let Some(Request::Edit {
                    command: Command::Mix { track, mix },
                    ..
                }) = pending.front()
                {
                    moves.push((*track, *mix));
                    pending.pop_front();
                }
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                let mut changed = false;
                for (track, mix) in moves {
                    changed |= doc.edit(Command::Mix { track, mix }, caret, None).is_some();
                }
                if changed {
                    changed_score(&mut st, None, false, true, sink);
                }
            }
            Request::Edit {
                command,
                caret,
                selection,
            } => {
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                let mut wait = false;
                let is_note = command.is_note_edit();
                if let Some(caret) = doc.edit(command, caret, selection) {
                    wait = changed_score(&mut st, Some(caret), is_note, false, sink);
                    st.stale = true;
                }
                // Typing ahead: the last edit engraves. A changed track list
                // waits for the window's settings (`changed_score`).
                pending.extend(rx.try_iter());
                if st.stale && !wait && !superseded(&pending, false) {
                    engrave(&mut st, sink);
                }
            }
            Request::Undo | Request::Redo => {
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                let done = if matches!(request, Request::Undo) {
                    doc.undo()
                } else {
                    doc.redo()
                };
                if done.is_some() {
                    sink(Event::Tools(doc.tools()));
                    if changed_score(&mut st, None, false, false, sink) {
                        st.stale = true;
                    } else {
                        engrave(&mut st, sink);
                    }
                }
            }
            Request::Save(to) => save(&mut st, to, sink),
            Request::ConfirmSave { keep } => {
                if let Some(prepared) = st.pending_save.take() {
                    write_save(&mut st, &prepared, keep || prepared.keeps, sink);
                }
            }
            Request::CancelSave => st.pending_save = None,
            Request::PluginEdit {
                plugin,
                revision,
                name,
                score,
                reply,
            } => {
                let answer = |done: Result<(), String>| {
                    if let Some(r) = &reply {
                        let _ = r.send(done);
                    }
                };
                let Some(doc) = st.doc.as_mut() else {
                    answer(Err("no score is open".to_owned()));
                    continue;
                };
                if revision != st.revision {
                    sink(Event::Message(
                        Level::Warning,
                        format!(
                            "{plugin}: its change came after another and was left out: {name}."
                        ),
                    ));
                    answer(Err(format!(
                        "the score changed since revision {revision}: ask for it again"
                    )));
                    continue;
                }
                doc.apply_change(&name, *score);
                sink(Event::Tools(doc.tools()));
                if changed_score(&mut st, None, false, false, sink) {
                    st.stale = true;
                } else {
                    engrave(&mut st, sink);
                }
                answer(Ok(()));
            }
            Request::PluginTool(plugin, tool) => {
                let tools = st.plugins.tools();
                let Some(t) = tools.iter().find(|t| {
                    t.spec.id == tool && t.plugin.manifest.as_ref().is_some_and(|m| m.id == plugin)
                }) else {
                    continue;
                };
                let name = t.spec.name.clone();
                match st.plugins.run_tool(*t) {
                    Ok(report) if !report.trim().is_empty() => {
                        sink(Event::Report(format!("{name}: {report}")));
                    }
                    Ok(_) => {}
                    Err(e) => sink(Event::Message(Level::Error, format!("{name}: {e}"))),
                }
            }
            Request::RunPluginTool {
                plugin,
                tool,
                options,
                reply,
            } => {
                let tools = st.plugins.tools();
                let Some(t) = tools.iter().find(|t| {
                    t.spec.id == tool && t.plugin.manifest.as_ref().is_some_and(|m| m.id == plugin)
                }) else {
                    let _ = reply.send(Err(format!("{plugin} has no usable tool {tool:?}")));
                    continue;
                };
                let name = t.spec.name.clone();
                let ran = match options {
                    Some(o) => st.plugins.run_tool_with(*t, o),
                    None => st.plugins.run_tool(*t),
                };
                // The footer says what it did, as when the user picks it.
                match &ran {
                    Ok(report) if !report.trim().is_empty() => {
                        sink(Event::Report(format!("{name}: {report}")));
                    }
                    Ok(_) => {}
                    Err(e) => sink(Event::Message(Level::Error, format!("{name}: {e}"))),
                }
                let _ = reply.send(ran.map_err(|e| e.to_string()));
            }
            Request::Fence {
                before,
                name,
                reply,
            } => {
                let report = if st.revision == before {
                    format!("{name} changed nothing.")
                } else {
                    format!("{name} is done: Undo takes it back as one step.")
                };
                let _ = reply.send(Ok(report));
            }
            Request::DrawingClicked {
                tag,
                at,
                x,
                y,
                double,
            } => {
                let Some(doc) = st.doc.as_ref() else {
                    continue;
                };
                let Some((plugin, place)) =
                    crate::drawings::clicked(&st.plugins, &doc.score, tag, at)
                else {
                    continue;
                };
                let click = oxiguitar_host::api::message::Clicked {
                    place,
                    at: oxiguitar_host::api::display::Point { x, y },
                    double,
                };
                let name = st
                    .plugins
                    .found()
                    .iter()
                    .find(|f| f.manifest.as_ref().is_some_and(|m| m.id == plugin))
                    .map_or_else(|| plugin.clone(), |f| f.name());
                match st.plugins.drawing_clicked(&plugin, click) {
                    Ok(report) if !report.trim().is_empty() => {
                        sink(Event::Report(format!("{name}: {report}")));
                    }
                    Ok(_) => {}
                    Err(e) => sink(Event::Message(Level::Error, format!("{name}: {e}"))),
                }
            }
            Request::Selection(selection, selection2) => {
                st.open.set_selection(selection2);
                st.plugins.push(PluginEvent::Selection(selection));
                st.plugins.push(PluginEvent::Selection2(selection2));
            }
            Request::ExportPdf(path, settings) => {
                let Some(doc) = st.doc.as_ref() else {
                    continue;
                };
                let name = file_name(&path);
                let drawings = crate::drawings::of(&st.plugins, &doc.score);
                let message = match st.engraver.pdf(doc, &settings, drawings) {
                    Ok((bytes, pages)) => match std::fs::write(&path, bytes) {
                        Ok(()) => (
                            Level::Info,
                            format!(
                                "Exported {name}: {pages} page{}.",
                                if pages == 1 { "" } else { "s" }
                            ),
                        ),
                        Err(e) => (Level::Error, format!("Could not write {name}: {e}.")),
                    },
                    Err(e) => (Level::Error, format!("Could not export {name}: {e}.")),
                };
                sink(Event::Message(message.0, message.1));
            }
        }
    }
}

fn open(st: &mut State, path: PathBuf, sink: &dyn Fn(Event)) {
    match Document::open(&path, &st.plugins) {
        Ok(doc) => opened(st, doc, path.parent().map(PathBuf::from), sink),
        Err(e) => sink(Event::Message(Level::Error, e)),
    }
}

/// A score opened or made: plugins hear of it, then the window.
fn opened(st: &mut State, doc: Document, folder: Option<PathBuf>, sink: &dyn Fn(Event)) {
    // Plugins hear of it as the window does (pushing never waits).
    st.revision += 1;
    st.open.set(Some(doc.score.clone()), st.revision);
    st.open.set_file(
        (!doc.untitled).then(|| std::path::absolute(&doc.path).unwrap_or(doc.path.clone())),
    );
    st.plugins.push(PluginEvent::ScoreOpened {
        revision: st.revision,
    });
    if let Some(warning) = doc.bad_bars_warning() {
        sink(Event::Message(Level::Warning, warning));
    }
    if let Some(warning) = &doc.sidecar_problem {
        sink(Event::Message(Level::Warning, warning.clone()));
    }
    sink(Event::Opened {
        score: doc.score.clone(),
        edit: doc.edit_state(),
        bad: doc.check.iter().collect(),
        tracks: doc.tracks(),
        tools: doc.tools(),
        pdf_name: doc.pdf_name(),
        folder,
        name: doc.name(),
        untitled: doc.untitled,
    });
    st.track_shape = track_shape(&doc.score);
    st.doc = Some(doc);
    st.pending_save = None;
}

/// File → Save and Save As: the file is got ready, then written, or held
/// while the user is asked.
fn save(st: &mut State, to: Option<(PathBuf, String)>, sink: &dyn Fn(Event)) {
    let Some(doc) = st.doc.as_ref() else {
        sink(Event::SaveFailed);
        return;
    };
    let to = match to {
        Some((path, id)) => match SaveFormat::writable(&st.plugins)
            .into_iter()
            .find(|f| f.id == id)
        {
            Some(format) => Some((path, format)),
            None => {
                sink(Event::Message(
                    Level::Error,
                    "No installed plugin writes that format.".to_owned(),
                ));
                sink(Event::SaveFailed);
                return;
            }
        },
        None => None,
    };
    match doc.prepare_save(&st.plugins, to) {
        Ok(prepared) if prepared.ask() => {
            let (heading, lines) = prepared.question();
            let can_keep = prepared.can_keep() && !prepared.keeps;
            st.pending_save = Some(prepared);
            sink(Event::AskSave {
                heading,
                lines,
                can_keep,
            });
        }
        Ok(prepared) => {
            let keep = prepared.keeps;
            write_save(st, &prepared, keep, sink);
        }
        Err(e) => {
            sink(Event::Message(Level::Error, e));
            sink(Event::SaveFailed);
        }
    }
}

fn write_save(st: &mut State, prepared: &Prepared, keep: bool, sink: &dyn Fn(Event)) {
    let Some(doc) = st.doc.as_mut() else {
        sink(Event::SaveFailed);
        return;
    };
    match doc.save(prepared, keep) {
        Ok(warning) => {
            if let Some(warning) = warning {
                sink(Event::Message(Level::Error, warning));
            }
            st.open.set_file(Some(
                std::path::absolute(&doc.path).unwrap_or(doc.path.clone()),
            ));
            let name = doc.name();
            sink(Event::Message(Level::Info, format!("Saved {name}.")));
            sink(Event::Saved {
                name,
                edit: doc.edit_state(),
                pdf_name: doc.pdf_name(),
                folder: doc.path.parent().map(PathBuf::from),
            });
        }
        Err(e) => {
            sink(Event::Message(Level::Error, e));
            sink(Event::SaveFailed);
        }
    }
}

fn engrave(st: &mut State, sink: &dyn Fn(Event)) {
    let Some(doc) = st.doc.as_ref() else {
        return;
    };
    st.stale = false;
    // What plugins draw, asked of them first (interface 1.8).
    let drawings = crate::drawings::of(&st.plugins, &doc.score);
    let engraved = st.engraver.engrave(
        &doc.score,
        &st.settings,
        doc.marked(),
        doc.bar_colour_global,
        drawings,
    );
    st.sheets = Sheets::stack(engraved.pages);
    st.engraving += 1;
    let hits = Arc::new(engraved.hits);
    // Plugins read the engraving too (interface 1.8).
    st.open
        .set_engraved(st.revision, st.sheets.lists.clone(), hits.clone());
    sink(Event::Engraved {
        engraving: st.engraving,
        sheets: st.sheets.rects.clone(),
        hits,
    });
}

/// Where each of the `new` tracks was among the `old` ones, matching what
/// the track list shows of them: the same place if it is still there,
/// else the nearest place that was; an added track was nowhere. When the
/// count is unchanged, a track that changed (renamed, restrung) stays put.
fn was<T: PartialEq>(old: &[T], new: &[T]) -> Vec<Option<usize>> {
    let mut used = vec![false; old.len()];
    let mut was: Vec<Option<usize>> = vec![None; new.len()];
    for (j, t) in new.iter().enumerate() {
        if old.get(j) == Some(t) && !used[j] {
            used[j] = true;
            was[j] = Some(j);
        }
    }
    for (j, t) in new.iter().enumerate() {
        if was[j].is_some() {
            continue;
        }
        let near = (0..old.len())
            .filter(|i| !used[*i] && old[*i] == *t)
            .min_by_key(|i| i.abs_diff(j));
        if let Some(i) = near {
            used[i] = true;
            was[j] = Some(i);
        }
    }
    if old.len() == new.len() {
        for (j, w) in was.iter_mut().enumerate() {
            if w.is_none() && !used[j] {
                used[j] = true;
                *w = Some(j);
            }
        }
    }
    was
}

/// The open score changed: plugins hear of it (pushing never waits), then
/// the window. True when the track list changed: the window answers with
/// the tracks to show (`Request::Engrave`), so engraving waits for that
/// rather than draw the old tracks' places. `note_edit` is whether the
/// change was a note insertion or deletion (for the engraved-beat preview,
/// #246).
fn changed_score(
    st: &mut State,
    caret: Option<Caret>,
    note_edit: bool,
    mixer: bool,
    sink: &dyn Fn(Event),
) -> bool {
    let Some(doc) = st.doc.as_ref() else {
        return false;
    };
    st.revision += 1;
    st.open.set(Some(doc.score.clone()), st.revision);
    st.plugins.push(PluginEvent::ScoreChanged {
        revision: st.revision,
    });
    let shape = track_shape(&doc.score);
    let tracks = (shape != st.track_shape).then(|| (doc.tracks(), was(&st.track_shape, &shape)));
    sink(Event::Changed {
        score: doc.score.clone(),
        edit: doc.edit_state(),
        bad: doc.check.iter().collect(),
        caret,
        tracks,
        note_edit,
        mixer,
    });
    let changed = shape != st.track_shape;
    st.track_shape = shape;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_are_followed_where_they_went() {
        let (a, b, c, d) = ("A", "B", "C", "D");
        assert_eq!(was(&[a, b, c, d], &[a, b, d]), [Some(0), Some(1), Some(3)]);
        assert_eq!(was(&[a, b, c], &[a, c, b]), [Some(0), Some(2), Some(1)]);
        assert_eq!(was(&[a, b], &[a, b, "B (copy)"]), [Some(0), Some(1), None]);
        assert_eq!(was(&[a, b], &[a, "Renamed"]), [Some(0), Some(1)]);
        assert_eq!(was(&[a, a, b], &[a, b]), [Some(0), Some(2)]);
    }

    /// Bytes in one tile.
    const TILE_BYTES: usize = (TILE * TILE * 4) as usize;
    use std::path::Path;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp")
    }

    fn next(rx: &Receiver<Event>) -> Event {
        rx.recv_timeout(Duration::from_secs(30)).unwrap()
    }

    #[test]
    fn opening_a_score_tells_the_plugins_that_listen_and_they_can_read_it() {
        use oxiguitar_host::api::view::Value;
        let host = crate::test_support::tab_host();
        let (tx, rx) = channel();
        let worker = Worker::start(host.plugins.clone(), host.open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::Open(fixture()));
        assert!(matches!(next(&rx), Event::Opened { .. }));
        host.plugins.wait_for_events();
        let events = String::from_utf8(
            host.plugins
                .call_service("test-echo", "events", b"")
                .unwrap(),
        )
        .unwrap();
        assert!(events.contains("ScoreOpened"), "{events}");
        // The test plugin shows a score being opened in its panel, and
        // reads the open score when its button asks for it.
        let set = |name: &str| {
            host.views
                .lock()
                .unwrap()
                .iter()
                .rev()
                .filter(|(v, _)| v == "panel")
                .find_map(|(_, p)| p.iter().find(|(n, _)| n == name).map(|p| p.1.clone()))
        };
        assert!(set("events").is_some());
        let title = host.open.wire().expect("a score is open").info.title;
        host.plugins
            .view_callback(&crate::test_support::panel(), "ask-score", vec![]);
        assert_eq!(set("score"), Some(Value::String(format!("Score: {title}"))));
    }

    #[test]
    fn open_engrave_draw_and_export() {
        let (tx, rx) = channel();
        let open = OpenScore::default();
        let worker = Worker::start(crate::test_plugins().clone(), open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::Open(fixture()));
        let Event::Opened { tracks, tools, .. } = next(&rx) else {
            panic!("expected Opened")
        };
        assert!(!tracks.is_empty() && !tools.is_empty());
        // Nothing for plugins to read of the page until it is engraved.
        assert!(open.engraving().is_none());

        worker.send(Request::Engrave(Settings::default()));
        let Event::Engraved {
            engraving,
            sheets,
            hits,
        } = next(&rx)
        else {
            panic!("expected Engraved")
        };
        assert_eq!(sheets.len(), 1);

        // Plugins read the same page (interface 1.8): its drawing, in the
        // text and the music font, and every beat's place on it.
        {
            use oxiguitar_host::api::display::{Font, Item};
            let read = open.engraving().expect("the engraving");
            assert_eq!(read.revision, open.wire2().expect("a score").0);
            assert_eq!(read.pages.len(), 1);
            let page = &read.pages[0];
            assert!(page.width > 0.0 && page.height > 0.0);
            let font = |f: Font| {
                page.items
                    .iter()
                    .any(|i| matches!(i, Item::Text { font, .. } if *font == f))
            };
            assert!(font(Font::Music) && font(Font::Sans));
            assert!(page.items.iter().any(|i| matches!(i, Item::Line { .. })));
            let beats: usize = hits
                .systems
                .iter()
                .flat_map(|s| &s.staves)
                .flat_map(|s| &s.bars)
                .map(|b| b.beats.len())
                .sum();
            assert!(beats > 0);
            assert_eq!(read.beats.len(), beats);
            assert!(read.beats.iter().all(|b| b.page == 0
                && b.width > 0.0
                && b.height > 0.0
                && b.x < page.width
                && b.y < page.height));
        }

        worker.send(Request::Tiles {
            engraving,
            paint: 7,
            scale: 1.0,
            dark: false,
            tiles: vec![(0, 0), (1, 0)],
        });
        for want in [(0, 0), (1, 0)] {
            let Event::Tile {
                paint,
                col,
                row,
                rgba,
                ..
            } = next(&rx)
            else {
                panic!("expected a tile")
            };
            assert_eq!((paint, (col, row)), (7, want));
            assert_eq!(rgba.len(), TILE_BYTES);
            // Some ink on the first tile: the title and the music.
            if want == (0, 0) {
                assert!(rgba.chunks(4).any(|p| p[3] == 255 && p[0] < 128));
            }
        }

        // Tiles of an old engraving are skipped.
        worker.send(Request::Engrave(Settings::default()));
        worker.send(Request::Tiles {
            engraving,
            paint: 7,
            scale: 1.0,
            dark: false,
            tiles: vec![(0, 0)],
        });
        assert!(matches!(next(&rx), Event::Engraved { engraving: e, .. } if e == engraving + 1));
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(300)),
            Err(RecvTimeoutError::Timeout)
        ));

        let dir = std::env::temp_dir().join(format!("oxiguitar-worker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("song.pdf");
        worker.send(Request::ExportPdf(pdf.clone(), Settings::default()));
        let Event::Message(Level::Info, s) = next(&rx) else {
            panic!("expected a report")
        };
        assert_eq!(s, "Exported song.pdf: 1 page.");
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
        let _ = std::fs::remove_dir_all(&dir);

        worker.send(Request::ApplyTool(0));
        assert!(matches!(next(&rx), Event::Tools(_)));
        assert!(matches!(next(&rx), Event::Report(_)));
        assert!(matches!(next(&rx), Event::Engraved { .. }));
        drop(worker);
    }

    #[test]
    fn a_bad_file_reports_and_keeps_going() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::Open(PathBuf::from("/no/such/song.gp")));
        assert!(matches!(next(&rx), Event::Message(Level::Error, s) if s.contains("song.gp")));
        worker.send(Request::Engrave(Settings::default()));
        worker.send(Request::Open(fixture()));
        assert!(matches!(next(&rx), Event::Opened { .. }));
    }

    #[test]
    fn saving_asks_when_the_file_changes_and_writes_on_yes() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        let dir =
            std::env::temp_dir().join(format!("oxiguitar-worker-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gp4 = dir.join("song.gp4");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp4"),
            &gp4,
        )
        .unwrap();
        worker.send(Request::Open(gp4));
        while !matches!(next(&rx), Event::Opened { .. }) {}

        // No plugin writes .gp4, and OxiGuitar picks no other format for
        // the user: Save says so and writes nothing.
        worker.send(Request::Save(None));
        assert!(matches!(
            next(&rx),
            Event::Message(Level::Error, s) if s.ends_with("choose a format with Save As.")
        ));
        assert!(matches!(next(&rx), Event::SaveFailed));
        assert!(!dir.join("song.gp").exists());

        // Save As, in a format a plugin writes. A question answered with
        // Cancel writes nothing; answered, the file is written.
        let to = dir.join("song.gp");
        worker.send(Request::Save(Some((to.clone(), "gp".to_owned()))));
        loop {
            match next(&rx) {
                Event::AskSave { heading, .. } => {
                    assert_eq!(heading, "Save song.gp?");
                    worker.send(Request::ConfirmSave { keep: true });
                }
                Event::Message(Level::Info, s) => assert_eq!(s, "Saved song.gp."),
                Event::Saved { name, edit, .. } => {
                    assert_eq!(name, "song.gp");
                    assert!(!edit.modified);
                    break;
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(to.exists());

        // Now a .gp file with nothing lost: written at once.
        worker.send(Request::Save(None));
        assert!(matches!(next(&rx), Event::Message(Level::Info, _)));
        assert!(matches!(next(&rx), Event::Saved { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gap 2 of docs/parity.md: the user's mixer is saved in the file and
    /// read back from it (Guitar Pro 7-8's `.gp` holds volume, pan, mute and
    /// solo).
    #[test]
    fn the_mixer_is_saved_in_the_file() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        let dir =
            std::env::temp_dir().join(format!("oxiguitar-worker-mixer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        worker.send(Request::Open(fixture()));
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let caret = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        // A second track, to hold a solo while the first is muted.
        worker.send(Request::Edit {
            command: Command::AddTrack,
            caret,
            selection: None,
        });
        let wanted = [
            oxiguitar_model::Mix {
                volume: 0.25,
                pan: -0.5,
                mute: true,
                solo: false,
            },
            oxiguitar_model::Mix {
                volume: 0.9,
                pan: 0.75,
                mute: false,
                solo: true,
            },
        ];
        for (track, mix) in wanted.iter().enumerate() {
            worker.send(Request::Edit {
                command: Command::Mix { track, mix: *mix },
                caret,
                selection: None,
            });
        }
        let to = dir.join("mixed.gp");
        worker.send(Request::Save(Some((to.clone(), "gp".to_owned()))));
        loop {
            match next(&rx) {
                Event::AskSave { .. } => worker.send(Request::ConfirmSave { keep: false }),
                Event::Saved { edit, .. } => {
                    assert!(!edit.modified);
                    break;
                }
                Event::SaveFailed => panic!("not saved"),
                _ => {}
            }
        }
        worker.send(Request::Open(to));
        let read = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        for (track, mix) in wanted.iter().enumerate() {
            assert_eq!(read.tracks[track].mix, *mix, "track {track}");
        }
        assert_eq!(read.tracks.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plugin_s_change_is_one_undoable_command_unless_late() {
        let (tx, rx) = channel();
        let open = OpenScore::default();
        let worker = Worker::start(crate::test_plugins().clone(), open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::Open(fixture()));
        while !matches!(next(&rx), Event::Opened { .. }) {}
        let (revision, mut score) = open.wire2().unwrap();
        score.info.title = "Changed".into();
        // A stale revision is refused at once, with a sentence.
        let late = open
            .apply(
                "A plugin",
                revision + 1,
                "Rename".into(),
                score.clone(),
                None,
            )
            .unwrap_err();
        assert!(late.contains("has changed since"), "{late}");
        assert!(
            open.apply("A plugin", revision, " ".into(), score.clone(), None)
                .is_err()
        );
        open.apply("A plugin", revision, "Rename".into(), score, None)
            .unwrap();
        loop {
            if let Event::Changed { score, edit, .. } = next(&rx) {
                assert_eq!(score.info.title, "Changed");
                assert_eq!(edit.undo.as_deref(), Some("Rename"));
                break;
            }
        }
        // Undone like any other change.
        worker.send(Request::Undo);
        loop {
            if let Event::Changed { score, .. } = next(&rx) {
                assert_ne!(score.info.title, "Changed");
                break;
            }
        }
        // A change with a reply (interface 1.14) is heard once it is made,
        // and the score plugins read is the changed one; a fence then says
        // the score changed since the revision before it.
        let (revision, mut score) = open.wire2().unwrap();
        score.info.title = "Answered".into();
        let (reply, done) = channel();
        open.apply("A plugin", revision, "Rename".into(), score, Some(reply))
            .unwrap();
        done.recv().unwrap().unwrap();
        assert_eq!(open.wire2().unwrap().1.info.title, "Answered");
        let (reply, fenced) = channel();
        worker.send(Request::Fence {
            before: revision,
            name: "Rename".into(),
            reply,
        });
        assert!(fenced.recv().unwrap().unwrap().contains("is done"));
        // A late one is told why.
        let (_, stale) = open.wire2().unwrap();
        let (reply, done) = channel();
        open.apply("A plugin", revision, "Rename".into(), stale, Some(reply))
            .unwrap_err();
        drop(done);
    }

    #[test]
    fn no_edit_is_made_while_the_score_plays() {
        let (tx, rx) = channel();
        let open = OpenScore::default();
        let worker = Worker::start(crate::test_plugins().clone(), open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let caret = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        let fret = |worker: &Worker| {
            worker.send(Request::Edit {
                command: Command::Fret {
                    fret: 5,
                    again: false,
                },
                caret,
                selection: None,
            });
        };
        // Playing: the window's edits and the plugins' are refused.
        open.set_playing(true);
        fret(&worker);
        worker.send(Request::Undo);
        loop {
            match next(&rx) {
                Event::Refused => break,
                Event::Changed { .. } => panic!("an edit was made while playing"),
                _ => {}
            }
        }
        assert!(matches!(next(&rx), Event::Refused), "undo is refused too");
        let (revision, plugin_score) = open.wire2().unwrap();
        let why = open
            .apply("A plugin", revision, "Rename".into(), plugin_score, None)
            .unwrap_err();
        assert!(why.contains("playing"), "{why}");
        // Paused: allowed again.
        open.set_playing(false);
        fret(&worker);
        loop {
            if let Event::Changed { edit, .. } = next(&rx) {
                assert_eq!(edit.undo.as_deref(), Some("Type Fret"));
                break;
            }
        }
    }

    /// The mixer goes into the score, even while the score plays, as
    /// changes Undo takes back: a run of volume moves is one.
    #[test]
    fn the_mixer_is_kept_in_the_score_while_it_plays() {
        let (tx, rx) = channel();
        let open = OpenScore::default();
        let worker = Worker::start(crate::test_plugins().clone(), open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let caret = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        let mix = |volume: f32, mute: bool| {
            worker.send(Request::Edit {
                command: Command::Mix {
                    track: 0,
                    mix: oxiguitar_model::Mix {
                        volume,
                        mute,
                        ..score.tracks[0].mix
                    },
                },
                caret,
                selection: None,
            });
        };
        open.set_playing(true);
        for v in [0.6, 0.5, 0.4] {
            mix(v, false);
        }
        mix(0.4, true);
        let mut last = None;
        while last
            .as_ref()
            .is_none_or(|(s, _): &(Arc<Score>, EditState)| !s.tracks[0].mix.mute)
        {
            match next(&rx) {
                Event::Changed {
                    score,
                    edit,
                    mixer,
                    tracks,
                    ..
                } => {
                    assert!(mixer && tracks.is_none());
                    last = Some((score, edit));
                }
                Event::Refused => panic!("the mixer was refused while playing"),
                _ => {}
            }
        }
        let (changed, edit) = last.unwrap();
        assert_eq!(changed.tracks[0].mix.volume, 0.4);
        assert_eq!(changed.tracks[0].staves, score.tracks[0].staves);
        assert!(edit.modified);
        assert_eq!(edit.undo.as_deref(), Some("Mute"));
        open.set_playing(false);
        worker.send(Request::Undo);
        worker.send(Request::Undo);
        let undone = loop {
            if let Event::Changed {
                score, edit, mixer, ..
            } = next(&rx)
                && edit.undo.is_none()
            {
                assert!(!mixer, "Undo is not the window's own move");
                break score;
            }
        };
        assert_eq!(undone.tracks[0].mix, score.tracks[0].mix);
    }

    /// #232: while the score plays, a track section's settings still go
    /// into the track (the effects follow them as they play); any other
    /// change of the track, even made with them, waits.
    #[test]
    fn a_track_section_s_settings_change_while_the_score_plays() {
        let (tx, rx) = channel();
        let open = OpenScore::default();
        let worker = Worker::start(crate::test_plugins().clone(), open.clone(), move |e| {
            let _ = tx.send(e);
        });
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let caret = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        let mut level = oxiguitar_edit::TrackSettings::of(&score.tracks[0]);
        level
            .plugins
            .insert("oxiguitar.eq".into(), r#"{"mode":"basic"}"#.into());
        let send = |to: &oxiguitar_edit::TrackSettings| {
            worker.send(Request::Edit {
                command: Command::TrackProperties(Box::new(to.clone())),
                caret,
                selection: None,
            });
        };
        open.set_playing(true);
        send(&level);
        let changed = loop {
            match next(&rx) {
                Event::Changed { score, .. } => break score,
                Event::Refused => panic!("a section's settings were refused while playing"),
                _ => {}
            }
        };
        assert_eq!(
            changed.tracks[0]
                .plugins
                .get("oxiguitar.eq")
                .map(String::as_str),
            Some(r#"{"mode":"basic"}"#)
        );
        // The notes are as they were; only the settings changed.
        assert_eq!(changed.tracks[0].staves, score.tracks[0].staves);
        // A rename with the settings is an edit of the score: it waits.
        let mut renamed = oxiguitar_edit::TrackSettings::of(&changed.tracks[0]);
        renamed.name = "Lead".into();
        renamed
            .plugins
            .insert("oxiguitar.eq".into(), r#"{"mode":"graphic"}"#.into());
        send(&renamed);
        // And so does moving a section: the order of the effects.
        let mut moved = oxiguitar_edit::TrackSettings::of(&changed.tracks[0]);
        moved.effects = vec!["oxiguitar.eq".into(), "oxiguitar.nam".into()];
        send(&moved);
        for _ in 0..2 {
            loop {
                match next(&rx) {
                    Event::Refused => break,
                    Event::Changed { .. } => panic!("the track was edited while playing"),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn find_fingering_is_one_change_and_marks_what_stays_hard() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let top = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        // F on the sixth string's first fret and B seven frets up on the
        // first string: no hand spans that, wherever the B goes.
        for (string, fret) in [(1, 7), (6, 1)] {
            worker.send(Request::Edit {
                command: Command::Fret { fret, again: false },
                caret: oxiguitar_edit::Caret { string, ..top },
                selection: None,
            });
            loop {
                if let Event::Changed { caret: Some(_), .. } = next(&rx) {
                    break;
                }
            }
        }
        worker.send(Request::Edit {
            command: Command::Fingering(oxiguitar_edit::Scope::Track),
            caret: top,
            selection: None,
        });
        loop {
            if let Event::Changed { score, edit, .. } = next(&rx) {
                assert_eq!(edit.undo.as_deref(), Some("Find Fingering"));
                let beat = &score.tracks[0].staves[0].bars[0].voices[0].beats[0];
                assert_eq!(beat.text, oxiguitar_edit::HARD_STRETCH);
                assert_eq!(beat.notes.len(), 2);
                break;
            }
        }
    }

    #[test]
    fn harmonize_writes_chords_as_one_change() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let top = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        // A C on the fifth string.
        worker.send(Request::Edit {
            command: Command::Fret {
                fret: 3,
                again: false,
            },
            caret: oxiguitar_edit::Caret { string: 5, ..top },
            selection: None,
        });
        loop {
            if let Event::Changed { caret: Some(_), .. } = next(&rx) {
                break;
            }
        }
        worker.send(Request::Edit {
            command: Command::Harmonize(oxiguitar_edit::Harmonize {
                sevenths: false,
                inside_bars: false,
                keep: false,
                all_tracks: true,
                whole_song: true,
            }),
            caret: top,
            selection: None,
        });
        loop {
            if let Event::Changed { score, edit, .. } = next(&rx) {
                assert_eq!(edit.undo.as_deref(), Some("Harmonize"));
                let track = &score.tracks[0];
                let chord = track.staves[0].bars[0].voices[0].beats[0].chord;
                let diagram = &track.chords[chord.expect("a chord on the first beat")];
                assert!(!diagram.name.is_empty());
                assert_eq!(diagram.frets.len(), 6);
                break;
            }
        }
    }

    #[test]
    fn fit_bars_rebars_every_track_as_one_change() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened { score, .. } = next(&rx) {
                break score;
            }
        };
        let top = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        let edit = |command: Command, caret: oxiguitar_edit::Caret| {
            worker.send(Request::Edit {
                command,
                caret,
                selection: None,
            });
            loop {
                if let Event::Changed {
                    score, edit, bad, ..
                } = next(&rx)
                {
                    break (score, edit, bad);
                }
            }
        };
        // Four quarters in 4/4, then the bar marked 3/4: it doesn't add up.
        for beat in 0..4 {
            edit(
                Command::Fret {
                    fret: beat as u8,
                    again: false,
                },
                oxiguitar_edit::Caret { beat, ..top },
            );
        }
        let (_, _, bad) = edit(
            Command::TimeSignature(oxiguitar_model::TimeSignature {
                numerator: 3,
                denominator: 4,
            }),
            top,
        );
        assert_eq!(bad.len(), 1);
        let fit = oxiguitar_edit::Fit {
            how: oxiguitar_edit::FitHow::Rebar,
            whole_song: true,
        };
        let (score, edit_state, bad) = edit(Command::FitBars(fit), top);
        assert_eq!(edit_state.undo.as_deref(), Some("Fit Bars"));
        assert_eq!(score.bar_count(), 2);
        let second = &score.tracks[0].staves[0].bars[1].voices[0].beats;
        assert_eq!(second.len(), 1);
        // The last bar holds one quarter: left short, and marked.
        assert_eq!(bad.iter().map(|b| b.bar).collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn a_new_score_takes_notes_and_is_saved_where_asked() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::New);
        let score = loop {
            if let Event::Opened {
                score,
                untitled,
                name,
                tracks,
                ..
            } = next(&rx)
            {
                assert!(untitled);
                assert_eq!(name, "Untitled");
                assert_eq!(tracks.len(), 1);
                break score;
            }
        };
        let caret = oxiguitar_edit::Caret::at_start(&score, 0).unwrap();
        worker.send(Request::Edit {
            command: Command::Fret {
                fret: 7,
                again: false,
            },
            caret,
            selection: None,
        });
        let caret = loop {
            if let Event::Changed {
                caret: Some(c),
                edit,
                ..
            } = next(&rx)
            {
                assert!(edit.modified);
                assert_eq!(edit.undo.as_deref(), Some("Type Fret"));
                break c;
            }
        };
        // Adding a track sends the track list again.
        worker.send(Request::Edit {
            command: Command::AddTrack,
            caret,
            selection: None,
        });
        loop {
            if let Event::Changed { tracks, .. } = next(&rx) {
                let (tracks, was) = tracks.expect("the track list again");
                assert_eq!(tracks.len(), 2);
                assert_eq!(was, [Some(0), None], "the new track added last");
                break;
            }
        }
        worker.send(Request::Undo);
        loop {
            if let Event::Changed { tracks, .. } = next(&rx) {
                assert_eq!(tracks.map(|(t, _)| t.len()), Some(1));
                break;
            }
        }
        // Right at the end of the score: a second bar.
        worker.send(Request::Edit {
            command: Command::AppendBar,
            caret: caret.bar_end(&score),
            selection: None,
        });
        loop {
            if let Event::Changed {
                score,
                caret: Some(c),
                ..
            } = next(&rx)
            {
                assert_eq!((score.bar_count(), c.bar), (2, 1));
                break;
            }
        }
        let dir = std::env::temp_dir().join(format!("oxiguitar-new-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.gp");
        worker.send(Request::Save(Some((path.clone(), "gp".to_owned()))));
        loop {
            match next(&rx) {
                Event::Saved { name, edit, .. } => {
                    assert_eq!(name, "song.gp");
                    assert!(!edit.modified);
                    break;
                }
                Event::AskSave { .. } => worker.send(Request::ConfirmSave { keep: false }),
                Event::SaveFailed => panic!("the save failed"),
                _ => {}
            }
        }
        let back = Document::open(&path, crate::test_plugins()).unwrap().score;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back.bar_count(), 2);
        assert_eq!(
            back.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0].place,
            oxiguitar_model::NotePlace::Fretted { string: 1, fret: 7 }
        );
    }
}
