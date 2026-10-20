// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The open score and how it is shown: reading the file, the View settings,
//! engraving, tools and PDF export. No window code here, so all of it is
//! tested without a display.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet, Theme};
use oxiguitar_edit::{BarCheck, Caret, Command, History, Region, Selection};
use oxiguitar_host::Plugins;
use oxiguitar_host::api::message::{Level as LogLevel, LogEntry};
use oxiguitar_layout::{Engraved, Layout, Notation, Options, Paper, engrave};
use oxiguitar_model::{Mix, Score};
use oxiguitar_tools::Availability;

/// How the score is shown: what the View menu and the track list set.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Settings {
    pub(crate) layout: Layout,
    pub(crate) paper: Paper,
    /// Shown tracks, by index, in score order. Never empty once a score is
    /// open.
    pub(crate) tracks: Vec<usize>,
    pub(crate) dynamics: bool,
    /// Width of the score area in points, for the Vertical layout.
    pub(crate) screen_width: f32,
    /// Each track's notation, from the track list.
    pub(crate) notation: BTreeMap<usize, Notation>,
    /// The caret's voice: the other voices are greyed out on the screen
    /// (never in a PDF).
    pub(crate) active_voice: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: Layout::Page,
            paper: Paper::A4,
            tracks: vec![0],
            dynamics: true,
            screen_width: 800.0,
            notation: BTreeMap::new(),
            active_voice: Some(0),
        }
    }
}

impl Settings {
    /// The engraver's options for these settings.
    pub(crate) fn options(&self) -> Options {
        Options {
            layout: self.layout,
            screen_width: self.screen_width,
            tracks: self.tracks.clone(),
            dynamics: self.dynamics,
            notation: self.notation.clone(),
            active_voice: self.active_voice,
            ..Options::default().with_paper(self.paper)
        }
    }
}

/// A track as the track list shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrackInfo {
    pub(crate) name: String,
    /// The track has strings to tab (not drums, not a track without strings).
    pub(crate) can_tab: bool,
    /// Its notation unless the user decides otherwise.
    pub(crate) default_notation: Notation,
    /// Volume, pan, mute and solo, as the file sets them.
    pub(crate) mix: Mix,
    /// How much the track plays in each bar of the song, 0 (nothing) to 1
    /// (the busiest bar of any track), for the track list's overview.
    pub(crate) activity: Vec<f32>,
}

/// How much each track plays in each bar: the notes it strikes there (grace
/// notes included, every staff and voice), relative to the busiest bar of
/// any track, as a square root so quiet bars still show. Bars with only
/// rests are 0.
fn activity(score: &Score) -> Vec<Vec<f32>> {
    let bars = score.master_bars.len();
    let counts: Vec<Vec<usize>> = score
        .tracks
        .iter()
        .map(|t| {
            (0..bars)
                .map(|b| {
                    t.staves
                        .iter()
                        .filter_map(|s| s.bars.get(b))
                        .flat_map(|bar| &bar.voices)
                        .flat_map(|v| &v.beats)
                        .filter(|beat| beat.kind == oxiguitar_model::BeatKind::Notes)
                        .map(|beat| beat.notes.len())
                        .sum()
                })
                .collect()
        })
        .collect();
    let most = counts.iter().flatten().copied().max().unwrap_or(0).max(1) as f32;
    counts
        .into_iter()
        .map(|c| c.into_iter().map(|n| (n as f32 / most).sqrt()).collect())
        .collect()
}

/// A Tools menu entry for the open score.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolEntry {
    pub(crate) name: String,
    /// Why the tool cannot run on this score; `None` if it can.
    pub(crate) unavailable: Option<String>,
}

/// An open score. The score is shared (never copied) with plugins that ask
/// for it ([`OpenScore`]), and with the window, which moves the caret over
/// it.
pub(crate) struct Document {
    pub(crate) path: PathBuf,
    /// The format the score was read in or last saved in.
    pub(crate) format: SaveFormat,
    /// What the reader couldn't carry over from the file (FMT-2), named
    /// before the file is written over; emptied once it is.
    pub(crate) opened_losses: Vec<LogEntry>,
    pub(crate) score: Arc<Score>,
    /// Every change made since the file was opened, to undo and redo.
    pub(crate) history: History,
    /// The bars that don't add up.
    pub(crate) check: BarCheck,
    /// A score made with File → New, not saved yet: Save asks for a name.
    pub(crate) untitled: bool,
}

/// What the Edit menu and the toolbar show about the history.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EditState {
    /// The name of the change Undo would undo.
    pub(crate) undo: Option<String>,
    /// The name of the change Redo would redo.
    pub(crate) redo: Option<String>,
    /// The score differs from the file.
    pub(crate) modified: bool,
}

/// The open score as plugins may ask for it, from any thread: set by the
/// worker when a score is opened or changed, with its revision. Converted
/// to the plugins' form only when one asks. Changes plugins hand back go
/// to the worker ([`OpenScore::apply`]).
#[derive(Clone, Default)]
pub(crate) struct OpenScore(Arc<Mutex<Shared>>);

#[derive(Default)]
struct Shared {
    score: Option<Arc<Score>>,
    revision: u64,
    /// The worker, where plugins' changes go.
    edits: Option<std::sync::mpsc::Sender<crate::worker::Request>>,
}

impl OpenScore {
    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The open score, and its revision.
    pub(crate) fn set(&self, score: Option<Arc<Score>>, revision: u64) {
        let mut s = self.lock();
        s.score = score;
        s.revision = revision;
    }

    /// Where the changes plugins hand back go: the worker's requests.
    pub(crate) fn connect(&self, edits: std::sync::mpsc::Sender<crate::worker::Request>) {
        self.lock().edits = Some(edits);
    }

    /// The open score, as plugins built for interface 1.0 or 1.1 see it.
    pub(crate) fn wire(&self) -> Option<oxiguitar_host::api::score::Score> {
        let score = self.lock().score.clone();
        score.map(|s| oxiguitar_model::wire::to_wire(&s))
    }

    /// The open score, version 2, and its revision (interface 1.2).
    pub(crate) fn wire2(&self) -> Option<(u64, oxiguitar_host::api::score2::Score2)> {
        let (score, revision) = {
            let s = self.lock();
            (s.score.clone(), s.revision)
        };
        score.map(|s| (revision, oxiguitar_model::wire::to_wire2(&s)))
    }

    /// A plugin hands back the score changed, made from `revision`: checked
    /// here, recorded by the worker as one change the user can undo. The
    /// error is a sentence for the plugin.
    pub(crate) fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: oxiguitar_host::api::score2::Score2,
    ) -> Result<(), String> {
        let s = self.lock();
        if s.score.is_none() {
            return Err("no score is open".to_owned());
        }
        if revision != s.revision {
            return Err(format!(
                "the score has changed since revision {revision} (it is at {}): ask for it again",
                s.revision
            ));
        }
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err("a change needs a name, for the Edit menu".to_owned());
        }
        let score = oxiguitar_model::wire::from_wire2(score)?;
        score.check_structure()?;
        let Some(edits) = &s.edits else {
            return Err("the score can't be changed now".to_owned());
        };
        edits
            .send(crate::worker::Request::PluginEdit {
                plugin: plugin.to_owned(),
                revision,
                name,
                score: Box::new(score),
            })
            .map_err(|_| "the score can't be changed now".to_owned())
    }
}

impl Document {
    /// Reads a score file with the format plugin that recognises it (the
    /// core reads no format itself). The error is a sentence for the footer.
    pub(crate) fn open(path: &Path, plugins: &Plugins) -> Result<Document, String> {
        let name = file_name(path);
        let data = std::fs::read(path).map_err(|e| format!("Cannot open {name}: {e}."))?;
        let extension = path.extension().and_then(|e| e.to_str());
        // A plugin that fails while reading is withdrawn and the next one
        // that recognises the file reads it, as if it weren't installed.
        let opened = plugins
            .open(&data, extension)
            .map_err(|why| format!("{name}: {why}"))?;
        let plugin = opened.format.plugin.name();
        let format = SaveFormat::of(opened.format.spec);
        let read = opened.read;
        let opened_losses = read
            .log
            .iter()
            .filter(|e| e.level != LogLevel::Info)
            .cloned()
            .collect();
        let score = oxiguitar_model::wire::from_wire2(read.score).map_err(|e| {
            format!("{name}: the {plugin} plugin gave a score this OxiGuitar can't hold ({e}).")
        })?;
        let check = BarCheck::of(&score);
        Ok(Document {
            path: path.to_owned(),
            format,
            opened_losses,
            score: Arc::new(score),
            history: History::new(),
            check,
            untitled: false,
        })
    }

    /// File → New: a new score (one bar, one guitar), in the fullest format
    /// the plugins write, with no file yet.
    pub(crate) fn untitled(plugins: &Plugins) -> Document {
        let writable = SaveFormat::writable(plugins);
        let format = writable
            .iter()
            .find(|f| f.id == FULLEST)
            .or(writable.first())
            .cloned()
            .unwrap_or_else(|| SaveFormat {
                id: FULLEST.to_owned(),
                name: "Guitar Pro 7–8".to_owned(),
                extensions: vec!["gp".to_owned()],
            });
        let score = oxiguitar_edit::new_score();
        Document {
            path: PathBuf::from("Untitled.gp"),
            format,
            opened_losses: Vec::new(),
            check: BarCheck::of(&score),
            score: Arc::new(score),
            history: History::new(),
            untitled: true,
        }
    }

    /// What the Edit menu shows.
    pub(crate) fn edit_state(&self) -> EditState {
        EditState {
            undo: self.history.undo_label().map(str::to_owned),
            redo: self.history.redo_label().map(str::to_owned),
            modified: self.history.is_modified(),
        }
    }

    /// The bars that don't add up, as the engraver marks them.
    pub(crate) fn marked(&self) -> BTreeSet<(usize, usize, usize)> {
        self.check
            .iter()
            .map(|b| (b.track, b.staff, b.bar))
            .collect()
    }

    /// The footer's warning about bars that don't add up, when there are
    /// any: how many, and where the first is.
    pub(crate) fn bad_bars_warning(&self) -> Option<String> {
        let first = self.check.in_score_order(|_| true).first().copied()?;
        let n = self.check.len();
        let track = self
            .score
            .tracks
            .get(first.track)
            .map(|t| t.name.trim())
            .filter(|n| !n.is_empty())
            .map_or_else(
                || format!("track {}", first.track + 1),
                |n| format!("“{n}”"),
            );
        Some(format!(
            "{n} bar{} {} add up to {} time signature; the first is bar {} of {track}. \
             F4 goes to the next.",
            if n == 1 { "" } else { "s" },
            if n == 1 { "doesn't" } else { "don't" },
            if n == 1 { "its" } else { "their" },
            first.bar + 1
        ))
    }

    /// Undoes the last change. Returns its name and whether the bad bars
    /// changed, or `None` with nothing to undo.
    pub(crate) fn undo(&mut self) -> Option<String> {
        let score = Arc::make_mut(&mut self.score);
        let (label, touched) = self.history.undo(score)?;
        self.check.update(&self.score, &touched);
        Some(label)
    }

    /// Redoes the last undone change. Returns its name, or `None` with
    /// nothing to redo.
    pub(crate) fn redo(&mut self) -> Option<String> {
        let score = Arc::make_mut(&mut self.score);
        let (label, touched) = self.history.redo(score)?;
        self.check.update(&self.score, &touched);
        Some(label)
    }

    /// The tracks, for the track list.
    pub(crate) fn tracks(&self) -> Vec<TrackInfo> {
        let activity = activity(&self.score);
        self.score
            .tracks
            .iter()
            .zip(activity)
            .enumerate()
            .map(|(i, (t, activity))| TrackInfo {
                name: if t.name.trim().is_empty() {
                    format!("Track {}", i + 1)
                } else {
                    t.name.trim().to_owned()
                },
                can_tab: !t.instrument.percussion && t.staves.iter().any(|s| s.string_count() > 0),
                default_notation: oxiguitar_layout::default_notation(t),
                mix: t.mix,
                activity,
            })
            .collect()
    }

    /// The Tools menu for this score.
    pub(crate) fn tools(&self) -> Vec<ToolEntry> {
        oxiguitar_tools::all()
            .iter()
            .map(|t| ToolEntry {
                name: t.name().to_owned(),
                unavailable: match t.availability(&self.score) {
                    Availability::Available => None,
                    Availability::Unavailable(why) => Some(why),
                },
            })
            .collect()
    }

    /// Applies the Tools menu's tool number `index`; returns its report,
    /// and whether it changed the score. The file on disk is not touched.
    pub(crate) fn apply_tool(&mut self, index: usize) -> (String, bool) {
        let Some(tool) = oxiguitar_tools::all().get(index) else {
            return (String::new(), false);
        };
        let outcome = tool.apply(&self.score);
        let changed = outcome.score.is_some();
        if let Some(new) = outcome.score {
            // One undoable command (M3). Tools may change anything.
            let score = Arc::make_mut(&mut self.score);
            let touched = self
                .history
                .record(score, tool.name(), vec![Region::Whole], |s| *s = new);
            self.check.update(&self.score, &touched);
        }
        (format!("{}: {}", tool.name(), outcome.report), changed)
    }

    /// Note entry: `command` typed at `caret` (on `selection`'s beats where
    /// it takes them), one change the user can undo. The caret after it, or
    /// `None` when the command doesn't apply there and nothing changed.
    pub(crate) fn edit(
        &mut self,
        command: Command,
        caret: Caret,
        selection: Option<Selection>,
    ) -> Option<Caret> {
        let score = Arc::make_mut(&mut self.score);
        let edited = oxiguitar_edit::apply(score, &mut self.history, caret, selection, command)?;
        self.check.update(&self.score, &edited.touched);
        Some(edited.caret)
    }

    /// A plugin's change, the whole score changed, as one command named
    /// `name` that the user can undo.
    pub(crate) fn apply_change(&mut self, name: &str, new: Score) {
        let score = Arc::make_mut(&mut self.score);
        let touched = self
            .history
            .record(score, name, vec![Region::Whole], |s| *s = new);
        self.check.update(&self.score, &touched);
    }

    /// The title for PDF metadata: the score's, or the file's name.
    pub(crate) fn title(&self) -> String {
        let t = self.score.info.title.trim();
        if t.is_empty() {
            self.path.file_stem().map_or_else(
                || "OxiGuitar".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            )
        } else {
            t.to_owned()
        }
    }

    /// A file name for the exported PDF, beside the score: "Song.pdf".
    pub(crate) fn pdf_name(&self) -> String {
        self.path
            .file_stem()
            .map_or_else(|| "score".to_owned(), |s| s.to_string_lossy().into_owned())
            + ".pdf"
    }
}

/// A format the installed plugins write: its id in the plugin's manifest,
/// its name ("Guitar Pro 7–8") and its extensions, without the dot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SaveFormat {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) extensions: Vec<String>,
}

impl SaveFormat {
    fn of(spec: &oxiguitar_host::api::manifest::FormatSpec) -> SaveFormat {
        SaveFormat {
            id: spec.id.clone(),
            name: spec.name.clone(),
            extensions: spec.extensions.clone(),
        }
    }

    /// Every format the installed plugins write, in plugin order.
    pub(crate) fn writable(plugins: &Plugins) -> Vec<SaveFormat> {
        plugins
            .writers()
            .iter()
            .map(|f| SaveFormat::of(f.spec))
            .collect()
    }
}

/// The format a file is saved in when its own would lose part of the
/// score, or no plugin writes it: the fullest OxiGuitar writes, Guitar Pro
/// 7–8's .gp (Mateus, 19 Oct 2026).
pub(crate) const FULLEST: &str = "gp";

/// A file ready to be written: where, in which format, its bytes, and what
/// the user should hear before it is written.
#[derive(Clone, Debug)]
pub(crate) struct Prepared {
    pub(crate) path: PathBuf,
    pub(crate) format: SaveFormat,
    pub(crate) data: Vec<u8>,
    /// What the format can't hold (FMT-3): the writer's warnings.
    pub(crate) losses: Vec<LogEntry>,
    /// What the reader left out of the file being written over (FMT-2).
    pub(crate) opened_losses: Vec<LogEntry>,
    /// Why the file is saved as another file, in another format, than the
    /// one it was opened from: "Guitar Pro 5 can't hold everything in
    /// this score" or "no installed plugin writes Guitar Pro 5 files".
    pub(crate) instead: Option<String>,
}

impl Prepared {
    /// Whether to ask before writing: something is lost, or the score goes
    /// to another file than the user expects.
    pub(crate) fn ask(&self) -> bool {
        !self.losses.is_empty() || !self.opened_losses.is_empty() || self.instead.is_some()
    }

    /// The question before writing: a heading, what to say, and the losses
    /// as lines ("tremolo picking … (bar 3, track 1)").
    pub(crate) fn question(&self, from: &Path) -> (String, Vec<String>) {
        let name = file_name(&self.path);
        let mut lines = Vec::new();
        if let Some(why) = &self.instead {
            lines.push(format!(
                "{why}, so it is saved in OxiGuitar's fullest format, {}, as {name}. {} stays as it was.",
                self.format.name,
                file_name(from)
            ));
        }
        let entry = |e: &LogEntry| {
            let times = if e.count > 1 {
                format!(", {} times", e.count)
            } else {
                String::new()
            };
            if e.place.is_empty() {
                format!("• {}", e.message)
            } else {
                format!("• {} ({}{times})", e.message, e.place)
            }
        };
        if !self.losses.is_empty() {
            lines.push(format!("{} files can't hold:", self.format.name));
            lines.extend(self.losses.iter().map(entry));
        }
        if !self.opened_losses.is_empty() {
            lines.push(format!(
                "Left out when {name} was opened, and lost when it is written over:"
            ));
            lines.extend(self.opened_losses.iter().map(entry));
        }
        (format!("Save {name}?"), lines)
    }
}

impl Document {
    /// Gets a save ready, without writing anything: to `to` in its format
    /// (Save As), or else to the file the score came from (Save). Save
    /// keeps the file's own format unless that would lose part of the
    /// score, or no plugin writes it: then the score goes beside it as a
    /// .gp file, the fullest format OxiGuitar writes (Mateus, 19 Oct 2026).
    pub(crate) fn prepare_save(
        &self,
        plugins: &Plugins,
        to: Option<(PathBuf, SaveFormat)>,
    ) -> Result<Prepared, String> {
        let wire = oxiguitar_model::wire::to_wire2(&self.score);
        let write = |format: &SaveFormat| -> Result<(Vec<u8>, Vec<LogEntry>), String> {
            let writers = plugins.writers();
            let Some(f) = writers.iter().find(|f| f.spec.id == format.id) else {
                return Err(format!("No installed plugin writes {} files.", format.name));
            };
            let w = plugins.write(*f, &wire).map_err(|e| {
                format!(
                    "The {} plugin could not write {}: {e}",
                    f.plugin.name(),
                    format.name
                )
            })?;
            let losses = w
                .losses
                .into_iter()
                .filter(|e| e.level != LogLevel::Info)
                .collect();
            Ok((w.data, losses))
        };
        let prepared = |path: PathBuf, format: SaveFormat, written: (Vec<u8>, Vec<LogEntry>)| {
            let opened_losses = if path == self.path {
                self.opened_losses.clone()
            } else {
                Vec::new()
            };
            Prepared {
                path,
                format,
                data: written.0,
                losses: written.1,
                opened_losses,
                instead: None,
            }
        };
        if let Some((path, format)) = to {
            let written = write(&format)?;
            return Ok(prepared(path, format, written));
        }
        let writable = SaveFormat::writable(plugins);
        let own = writable.iter().find(|f| f.id == self.format.id).cloned();
        let own_written = own.as_ref().map(write).transpose()?;
        if let (Some(own), Some(written)) = (&own, &own_written)
            && (written.1.is_empty() || own.id == FULLEST)
        {
            return Ok(prepared(self.path.clone(), own.clone(), written.clone()));
        }
        let Some(fullest) = writable.iter().find(|f| f.id == FULLEST).cloned() else {
            // No .gp writer: the file's own format, losses named, if any.
            return match (own, own_written) {
                (Some(own), Some(written)) => Ok(prepared(self.path.clone(), own, written)),
                _ => Err(format!(
                    "No installed plugin writes {} files, nor Guitar Pro 7–8 files: choose a format with Save As.",
                    self.format.name
                )),
            };
        };
        let written = write(&fullest)?;
        let path = self
            .path
            .with_extension(fullest.extensions.first().map_or("gp", String::as_str));
        let mut p = prepared(path, fullest, written);
        p.instead = Some(match own {
            Some(_) => format!("{} can't hold everything in this score", self.format.name),
            None => format!("No installed plugin writes {} files", self.format.name),
        });
        Ok(p)
    }

    /// Writes a prepared file: beside the old one first, then in its place,
    /// so a failure never leaves half a file. The document then belongs to
    /// the new file. The error is a sentence for the footer.
    pub(crate) fn save(&mut self, prepared: &Prepared) -> Result<(), String> {
        let name = file_name(&prepared.path);
        let temp = prepared
            .path
            .with_file_name(format!(".{name}.oxiguitar-saving"));
        let written = std::fs::write(&temp, &prepared.data)
            .and_then(|()| std::fs::rename(&temp, &prepared.path));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("Could not save {name}: {e}."));
        }
        if prepared.path == self.path {
            self.opened_losses.clear();
        }
        self.path.clone_from(&prepared.path);
        self.format = prepared.format.clone();
        self.untitled = false;
        self.history.mark_saved();
        Ok(())
    }

    /// The file's name, for the window's title ("Untitled" before a new
    /// score is saved).
    pub(crate) fn name(&self) -> String {
        if self.untitled {
            "Untitled".to_owned()
        } else {
            file_name(&self.path)
        }
    }
}

/// What the engraver and renderer need, loaded once.
pub(crate) struct Engraver {
    pub(crate) fonts: FontSet,
    pub(crate) meta: Metadata,
}

impl Engraver {
    pub(crate) fn new() -> Result<Engraver, String> {
        let meta = Metadata::bravura().map_err(|e| format!("Bravura metadata: {e}"))?;
        Ok(Engraver {
            fonts: FontSet::bundled(),
            meta,
        })
    }

    /// Engraves the score as the settings say, with `marked` bars laid in
    /// red, and the map of where everything is drawn.
    pub(crate) fn engrave(
        &self,
        score: &Score,
        settings: &Settings,
        marked: BTreeSet<(usize, usize, usize)>,
    ) -> Engraved {
        let options = Options {
            marked,
            ..settings.options()
        };
        engrave(score, &self.fonts, &self.meta, &options)
    }

    /// The PDF of the shown tracks: always the Page layout on the chosen
    /// paper, light page (EXP-1). Returns the bytes and the page count.
    pub(crate) fn pdf(
        &self,
        doc: &Document,
        settings: &Settings,
    ) -> Result<(Vec<u8>, usize), String> {
        let paged = Settings {
            layout: Layout::Page,
            active_voice: None,
            ..settings.clone()
        };
        // A PDF shows what the file says: no marks, every voice in ink.
        let pages = self.engrave(&doc.score, &paged, BTreeSet::new()).pages;
        let refs: Vec<&DisplayList> = pages.iter().collect();
        let bytes = oxiguitar_render::to_pdf(&refs, &self.fonts, &Theme::PRINT, &doc.title())
            .map_err(|e| e.to_string())?;
        Ok((bytes, pages.len()))
    }
}

/// A path's file name, for messages.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(ext: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/gp")
            .join(format!("oxiguitar-test-song.{ext}"))
    }

    fn open(path: &Path) -> Result<Document, String> {
        Document::open(path, crate::test_plugins())
    }

    #[test]
    fn opens_engraves_and_exports() {
        let doc = open(&fixture("gp")).unwrap();
        assert!(!doc.tracks().is_empty());
        assert_eq!(doc.pdf_name(), "oxiguitar-test-song.pdf");
        let e = Engraver::new().unwrap();
        for layout in Layout::ALL {
            let s = Settings {
                layout,
                ..Settings::default()
            };
            assert_eq!(
                e.engrave(&doc.score, &s, BTreeSet::new()).pages.len(),
                1,
                "{layout:?}"
            );
        }
        let (pdf, pages) = e
            .pdf(
                &doc,
                &Settings {
                    layout: Layout::Horizontal,
                    paper: Paper::Letter,
                    ..Settings::default()
                },
            )
            .unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(pages, 1);
    }

    #[test]
    fn errors_are_sentences() {
        let missing = open(Path::new("/no/such/file.gp5")).err().unwrap();
        assert!(missing.starts_with("Cannot open file.gp5"), "{missing}");
        let dir = std::env::temp_dir().join(format!("oxiguitar-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let junk = dir.join("junk.gp");
        std::fs::write(&junk, b"not a score").unwrap();
        let err = open(&junk).err().unwrap();
        assert!(
            err.starts_with(
                "junk.gp: the Guitar Pro 7–8 plugin opens .gp files but doesn't recognise this one"
            ),
            "{err}"
        );
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"not a score").unwrap();
        let err = open(&text).err().unwrap();
        assert!(
            err.starts_with("notes.txt: no installed plugin recognises this file"),
            "{err}"
        );
        let broken = dir.join("broken.gp5");
        let mut data = std::fs::read(fixture("gp5")).unwrap();
        data.truncate(200);
        std::fs::write(&broken, data).unwrap();
        let err = open(&broken).err().unwrap();
        assert!(
            err.starts_with("broken.gp5: the file ends too early"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_fretted_tracks_default_to_tab() {
        let doc = open(&fixture("gp5")).unwrap();
        for (info, track) in doc.tracks().iter().zip(&doc.score.tracks) {
            assert_eq!(
                info.default_notation.tablature,
                track.instrument.is_fretted() && info.can_tab
            );
            assert!(info.default_notation.any());
        }
    }

    #[test]
    fn activity_covers_every_bar_relative_to_the_busiest() {
        let doc = open(&fixture("gp5")).unwrap();
        let bars = doc.score.master_bars.len();
        let tracks = doc.tracks();
        assert!(bars > 0 && !tracks.is_empty());
        for t in &tracks {
            assert_eq!(t.activity.len(), bars, "{}", t.name);
            assert!(t.activity.iter().all(|a| (0.0..=1.0).contains(a)));
        }
        let most = tracks
            .iter()
            .flat_map(|t| &t.activity)
            .copied()
            .fold(0.0, f32::max);
        assert!((most - 1.0).abs() < 1e-6, "the busiest bar is 1");
        // A bar of rests only plays nothing.
        let score = &doc.score;
        for (t, info) in score.tracks.iter().zip(&tracks) {
            for (b, a) in info.activity.iter().enumerate() {
                let silent = t.staves.iter().filter_map(|s| s.bars.get(b)).all(|bar| {
                    bar.voices.iter().flat_map(|v| &v.beats).all(|beat| {
                        beat.kind != oxiguitar_model::BeatKind::Notes || beat.notes.is_empty()
                    })
                });
                assert_eq!(silent, *a == 0.0, "track {}, bar {b}", info.name);
            }
        }
    }

    #[test]
    fn undo_redo_and_the_bars_that_dont_add_up() {
        let mut doc = open(&fixture("gp5")).unwrap();
        assert!(doc.check.is_empty() && doc.bad_bars_warning().is_none());
        assert_eq!(doc.edit_state(), EditState::default());
        assert!(doc.undo().is_none() && doc.redo().is_none());
        let shared = doc.score.clone();
        // A change that shortens bar 2 of track 1.
        let score = Arc::make_mut(&mut doc.score);
        let region = Region::Bars {
            track: 0,
            staff: 0,
            bars: 1..2,
        };
        let touched = doc.history.record(score, "Delete Beat", vec![region], |s| {
            s.tracks[0].staves[0].bars[1].voices[0].beats.pop();
        });
        doc.check.update(&doc.score, &touched);
        assert!(
            !Arc::ptr_eq(&shared, &doc.score),
            "a shared score is never changed"
        );
        assert_eq!(doc.marked().into_iter().collect::<Vec<_>>(), [(0, 0, 1)]);
        let warning = doc.bad_bars_warning().unwrap();
        assert!(
            warning
                .starts_with("1 bar doesn't add up to its time signature; the first is bar 2 of "),
            "{warning}"
        );
        assert_eq!(
            doc.edit_state(),
            EditState {
                undo: Some("Delete Beat".into()),
                redo: None,
                modified: true
            }
        );
        assert_eq!(doc.undo().as_deref(), Some("Delete Beat"));
        assert!(doc.check.is_empty(), "the bar adds up again");
        assert_eq!(*doc.score, *shared);
        assert!(!doc.edit_state().modified);
        assert_eq!(doc.redo().as_deref(), Some("Delete Beat"));
        assert_eq!(doc.check.len(), 1);
    }

    #[test]
    fn tools_menu_and_apply() {
        let mut doc = open(&fixture("gp5")).unwrap();
        let tools = doc.tools();
        assert_eq!(tools.len(), oxiguitar_tools::all().len());
        let before = doc.score.clone();
        let (report, changed) = doc.apply_tool(0);
        assert!(report.starts_with("Infer Key Signatures: "), "{report}");
        // Changed or not, a score shared before is never changed under
        // whoever holds it.
        assert_eq!(changed, !Arc::ptr_eq(&before, &doc.score));
        assert_eq!(doc.apply_tool(99), (String::new(), false));
    }

    /// A folder of its own in the system's temporary folder, empty.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("oxiguitar-app-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_gp_file_is_saved_in_place() {
        let dir = scratch("save-in-place");
        let path = dir.join("song.gp");
        std::fs::copy(fixture("gp"), &path).unwrap();
        let mut doc = open(&path).unwrap();
        assert_eq!(doc.format.id, FULLEST);
        doc.apply_tool(0);
        let prepared = doc.prepare_save(crate::test_plugins(), None).unwrap();
        assert!(!prepared.ask(), "{prepared:?}");
        assert_eq!(prepared.path, path);
        doc.save(&prepared).unwrap();
        assert!(!doc.edit_state().modified);
        // What was written is what the document holds, and nothing is left
        // beside it.
        let again = open(&path).unwrap();
        assert_eq!(again.score, doc.score);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn a_file_no_plugin_writes_is_saved_beside_it_as_gp() {
        let dir = scratch("save-beside");
        let path = dir.join("song.gp5");
        std::fs::copy(fixture("gp5"), &path).unwrap();
        let mut doc = open(&path).unwrap();
        let prepared = doc.prepare_save(crate::test_plugins(), None).unwrap();
        assert!(prepared.ask());
        assert_eq!(prepared.path, dir.join("song.gp"));
        assert_eq!(prepared.format.id, FULLEST);
        let (heading, lines) = prepared.question(&doc.path);
        assert_eq!(heading, "Save song.gp?");
        assert!(
            lines[0].starts_with("No installed plugin writes Guitar Pro 5 files, so it is saved in OxiGuitar's fullest format, Guitar Pro 7–8, as song.gp. song.gp5 stays as it was."),
            "{lines:?}"
        );
        let before = std::fs::read(&path).unwrap();
        doc.save(&prepared).unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the .gp5 is untouched"
        );
        assert_eq!(doc.path, dir.join("song.gp"));
        assert_eq!(doc.name(), "song.gp");
        assert_eq!(doc.format.id, FULLEST);
        // From then on, Save writes the .gp in place.
        let next = doc.prepare_save(crate::test_plugins(), None).unwrap();
        assert_eq!(next.path, dir.join("song.gp"));
        assert!(next.instead.is_none());
    }

    #[test]
    fn save_as_writes_where_asked_and_names_losses() {
        let dir = scratch("save-as");
        let mut doc = open(&fixture("gp")).unwrap();
        let gp = SaveFormat::writable(crate::test_plugins())
            .into_iter()
            .find(|f| f.id == FULLEST)
            .unwrap();
        // A section colour, which a .gp file can't hold.
        Arc::make_mut(&mut doc.score).master_bars[0].marker = Some(oxiguitar_model::Marker {
            letter: "A".into(),
            text: "Intro".into(),
            color: oxiguitar_model::Color { r: 255, g: 0, b: 0 },
        });
        let to = dir.join("copy.gp");
        let prepared = doc
            .prepare_save(crate::test_plugins(), Some((to.clone(), gp)))
            .unwrap();
        assert!(prepared.ask());
        assert!(prepared.instead.is_none());
        let (_, lines) = prepared.question(&doc.path);
        assert_eq!(lines[0], "Guitar Pro 7–8 files can't hold:");
        assert_eq!(lines[1], "• section colours are left out (bar 1, track 1)");
        doc.save(&prepared).unwrap();
        assert!(to.exists());
        assert_eq!(doc.path, to);
    }

    #[test]
    fn writing_over_the_opened_file_names_what_opening_left_out() {
        let mut doc = open(&fixture("gp")).unwrap();
        doc.opened_losses = vec![LogEntry {
            level: LogLevel::Warning,
            message: "fermatas are not supported yet".into(),
            place: "bar 2".into(),
            count: 3,
        }];
        let prepared = doc.prepare_save(crate::test_plugins(), None).unwrap();
        assert!(prepared.ask());
        let (_, lines) = prepared.question(&doc.path);
        assert_eq!(
            lines,
            [
                "Left out when oxiguitar-test-song.gp was opened, and lost when it is written over:",
                "• fermatas are not supported yet (bar 2, 3 times)"
            ]
        );
        // Saved as another file, the opened one keeps them.
        let other = scratch("save-elsewhere").join("other.gp");
        let gp = prepared.format.clone();
        let elsewhere = doc
            .prepare_save(crate::test_plugins(), Some((other, gp)))
            .unwrap();
        assert!(!elsewhere.ask());
    }
}
