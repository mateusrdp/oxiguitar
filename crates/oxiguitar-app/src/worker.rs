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
    /// The user agreed to the save asked about.
    ConfirmSave,
    /// The user declined it.
    CancelSave,
    /// A plugin handed back the score changed, made from `revision`
    /// (interface 1.2): one change the user can undo, named `name`.
    PluginEdit {
        plugin: String,
        revision: u64,
        name: String,
        score: Box<Score>,
    },
    /// Run a plugin's tool from the Tools menu: the plugin's id, the
    /// tool's id.
    PluginTool(String, String),
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
    },
    /// A save waits for the user: a heading and lines to show, with Save
    /// and Cancel ([`Request::ConfirmSave`], [`Request::CancelSave`]).
    AskSave {
        heading: String,
        lines: Vec<String>,
        /// The score would go to another file than it came from: the
        /// question also offers Save As.
        elsewhere: bool,
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
    /// A message for the footer: an error, or a report such as a tool's.
    Message(Level, String),
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
            Event::Changed { bad, .. } => write!(f, "Changed({} bad bars)", bad.len()),
            Event::Tools(t) => write!(f, "Tools({})", t.len()),
            Event::Message(l, s) => write!(f, "Message({l:?}, {s:?})"),
            Event::AskSave { heading, .. } => write!(f, "AskSave({heading:?})"),
            Event::Saved { name, .. } => write!(f, "Saved({name:?})"),
            Event::SaveFailed => write!(f, "SaveFailed"),
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
                | Request::Engrave(_)
                | Request::ApplyTool(_)
                | Request::Edit { .. }
                | Request::Undo
                | Request::Redo => true,
                Request::Selection(..) => false,
                Request::Stop => true,
                Request::Tiles { .. } => tiles,
                Request::PluginEdit { .. } | Request::PluginTool(..) => true,
                Request::ExportPdf(..)
                | Request::Save(_)
                | Request::ConfirmSave
                | Request::CancelSave => false,
            })
        };
        match request {
            Request::Stop => return,
            Request::Open(path) => open(&mut st, path, sink),
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
                sink(Event::Message(Level::Info, report));
                if changed {
                    changed_score(&mut st, None, sink);
                }
                engrave(&mut st, sink);
            }
            Request::Edit {
                command,
                caret,
                selection,
            } => {
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                if let Some(caret) = doc.edit(command, caret, selection) {
                    changed_score(&mut st, Some(caret), sink);
                    st.stale = true;
                }
                // Typing ahead: the last edit engraves.
                pending.extend(rx.try_iter());
                if st.stale && !superseded(&pending, false) {
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
                    changed_score(&mut st, None, sink);
                    engrave(&mut st, sink);
                }
            }
            Request::Save(to) => save(&mut st, to, sink),
            Request::ConfirmSave => {
                if let Some(prepared) = st.pending_save.take() {
                    write_save(&mut st, &prepared, sink);
                }
            }
            Request::CancelSave => st.pending_save = None,
            Request::PluginEdit {
                plugin,
                revision,
                name,
                score,
            } => {
                let Some(doc) = st.doc.as_mut() else {
                    continue;
                };
                if revision != st.revision {
                    sink(Event::Message(
                        Level::Warning,
                        format!(
                            "{plugin}: its change came after another and was left out: {name}."
                        ),
                    ));
                    continue;
                }
                doc.apply_change(&name, *score);
                sink(Event::Tools(doc.tools()));
                changed_score(&mut st, None, sink);
                engrave(&mut st, sink);
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
                        sink(Event::Message(Level::Info, format!("{name}: {report}")));
                    }
                    Ok(_) => {}
                    Err(e) => sink(Event::Message(Level::Error, format!("{name}: {e}"))),
                }
            }
            Request::Selection(selection, selection2) => {
                st.plugins.push(PluginEvent::Selection(selection));
                st.plugins.push(PluginEvent::Selection2(selection2));
            }
            Request::ExportPdf(path, settings) => {
                let Some(doc) = st.doc.as_ref() else {
                    continue;
                };
                let name = file_name(&path);
                let message = match st.engraver.pdf(doc, &settings) {
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
        Ok(doc) => {
            // Plugins hear of it as the window does (pushing never waits).
            st.revision += 1;
            st.open.set(Some(doc.score.clone()), st.revision);
            st.plugins.push(PluginEvent::ScoreOpened {
                revision: st.revision,
            });
            if let Some(warning) = doc.bad_bars_warning() {
                sink(Event::Message(Level::Warning, warning));
            }
            sink(Event::Opened {
                score: doc.score.clone(),
                edit: doc.edit_state(),
                bad: doc.check.iter().collect(),
                tracks: doc.tracks(),
                tools: doc.tools(),
                pdf_name: doc.pdf_name(),
                folder: path.parent().map(PathBuf::from),
                name: doc.name(),
            });
            st.doc = Some(doc);
            st.pending_save = None;
        }
        Err(e) => sink(Event::Message(Level::Error, e)),
    }
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
            let (heading, lines) = prepared.question(&doc.path);
            let elsewhere = prepared.instead.is_some();
            st.pending_save = Some(prepared);
            sink(Event::AskSave {
                heading,
                lines,
                elsewhere,
            });
        }
        Ok(prepared) => write_save(st, &prepared, sink),
        Err(e) => {
            sink(Event::Message(Level::Error, e));
            sink(Event::SaveFailed);
        }
    }
}

fn write_save(st: &mut State, prepared: &Prepared, sink: &dyn Fn(Event)) {
    let Some(doc) = st.doc.as_mut() else {
        sink(Event::SaveFailed);
        return;
    };
    match doc.save(prepared) {
        Ok(()) => {
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
    let engraved = st.engraver.engrave(&doc.score, &st.settings, doc.marked());
    st.sheets = Sheets::stack(engraved.pages);
    st.engraving += 1;
    sink(Event::Engraved {
        engraving: st.engraving,
        sheets: st.sheets.rects.clone(),
        hits: Arc::new(engraved.hits),
    });
}

/// The open score changed: plugins hear of it (pushing never waits), then
/// the window.
fn changed_score(st: &mut State, caret: Option<Caret>, sink: &dyn Fn(Event)) {
    let Some(doc) = st.doc.as_ref() else {
        return;
    };
    st.revision += 1;
    st.open.set(Some(doc.score.clone()), st.revision);
    st.plugins.push(PluginEvent::ScoreChanged {
        revision: st.revision,
    });
    sink(Event::Changed {
        score: doc.score.clone(),
        edit: doc.edit_state(),
        bad: doc.check.iter().collect(),
        caret,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use oxiguitar_host::api::tab::{TabEvent, Widget};
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
        // The test plugin asks for its tab to be redrawn after a score
        // opens, and reads the open score when its button is clicked.
        assert!(
            host.redraws
                .lock()
                .unwrap()
                .contains(&crate::test_support::panel())
        );
        let title = host.open.wire().expect("a score is open").info.title;
        let content = host
            .plugins
            .tab_event(&crate::test_support::panel(), "score", TabEvent::Clicked)
            .unwrap();
        assert!(
            content.rows.iter().flat_map(|r| &r.widgets).any(
                |w| matches!(w, Widget::Label { text, .. } if *text == format!("Score: {title}"))
            ),
            "{content:?}"
        );
    }

    #[test]
    fn open_engrave_draw_and_export() {
        let (tx, rx) = channel();
        let worker = Worker::start(
            crate::test_plugins().clone(),
            OpenScore::default(),
            move |e| {
                let _ = tx.send(e);
            },
        );
        worker.send(Request::Open(fixture()));
        let Event::Opened { tracks, tools, .. } = next(&rx) else {
            panic!("expected Opened")
        };
        assert!(!tracks.is_empty() && !tools.is_empty());

        worker.send(Request::Engrave(Settings::default()));
        let Event::Engraved {
            engraving, sheets, ..
        } = next(&rx)
        else {
            panic!("expected Engraved")
        };
        assert_eq!(sheets.len(), 1);

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
        assert!(matches!(next(&rx), Event::Message(Level::Info, _)));
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
        let gp5 = dir.join("song.gp5");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp5"),
            &gp5,
        )
        .unwrap();
        worker.send(Request::Open(gp5));
        while !matches!(next(&rx), Event::Opened { .. }) {}

        // No plugin writes .gp5: Save asks before writing song.gp.
        worker.send(Request::Save(None));
        let Event::AskSave { heading, .. } = next(&rx) else {
            panic!("expected a question")
        };
        assert_eq!(heading, "Save song.gp?");
        worker.send(Request::CancelSave);
        worker.send(Request::ConfirmSave);
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(300)),
            Err(RecvTimeoutError::Timeout)
        ));
        assert!(!dir.join("song.gp").exists());

        worker.send(Request::Save(None));
        assert!(matches!(next(&rx), Event::AskSave { .. }));
        worker.send(Request::ConfirmSave);
        assert!(matches!(next(&rx), Event::Message(Level::Info, s) if s == "Saved song.gp."));
        let Event::Saved { name, edit, .. } = next(&rx) else {
            panic!("expected Saved")
        };
        assert_eq!(name, "song.gp");
        assert!(!edit.modified);
        assert!(dir.join("song.gp").exists());

        // Now a .gp file with nothing lost: written at once.
        worker.send(Request::Save(None));
        assert!(matches!(next(&rx), Event::Message(Level::Info, _)));
        assert!(matches!(next(&rx), Event::Saved { .. }));
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
            .apply("A plugin", revision + 1, "Rename".into(), score.clone())
            .unwrap_err();
        assert!(late.contains("has changed since"), "{late}");
        assert!(
            open.apply("A plugin", revision, " ".into(), score.clone())
                .is_err()
        );
        open.apply("A plugin", revision, "Rename".into(), score)
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
    }
}
