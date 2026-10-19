// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The open score and how it is shown: reading the file, the View settings,
//! engraving, tools and PDF export. No window code here, so all of it is
//! tested without a display.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet, Theme};
use oxiguitar_host::Plugins;
use oxiguitar_layout::{Layout, Notation, Options, Paper, engrave};
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
/// for it ([`OpenScore`]).
pub(crate) struct Document {
    pub(crate) path: PathBuf,
    pub(crate) score: Arc<Score>,
}

/// The open score as plugins may ask for it, from any thread: set by the
/// worker when a score is opened or changed. Converted to the plugins' form
/// only when one asks.
#[derive(Clone, Default)]
pub(crate) struct OpenScore(Arc<Mutex<Option<Arc<Score>>>>);

impl OpenScore {
    pub(crate) fn set(&self, score: Option<Arc<Score>>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = score;
    }

    /// The open score, as plugins see it.
    pub(crate) fn wire(&self) -> Option<oxiguitar_host::api::score::Score> {
        let score = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        score.map(|s| oxiguitar_model::wire::to_wire(&s))
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
        let read = opened.read;
        let score = oxiguitar_model::wire::from_wire(read.score).map_err(|e| {
            format!("{name}: the {plugin} plugin gave a score this OxiGuitar can't hold ({e}).")
        })?;
        Ok(Document {
            path: path.to_owned(),
            score: Arc::new(score),
        })
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
        if let Some(score) = outcome.score {
            self.score = Arc::new(score);
        }
        (format!("{}: {}", tool.name(), outcome.report), changed)
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

    /// Engraves the score as the settings say.
    pub(crate) fn engrave(&self, score: &Score, settings: &Settings) -> Vec<DisplayList> {
        engrave(score, &self.fonts, &self.meta, &settings.options()).pages
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
            ..settings.clone()
        };
        let pages = self.engrave(&doc.score, &paged);
        let refs: Vec<&DisplayList> = pages.iter().collect();
        let bytes = oxiguitar_render::to_pdf(&refs, &self.fonts, &Theme::LIGHT, &doc.title())
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
            assert_eq!(e.engrave(&doc.score, &s).len(), 1, "{layout:?}");
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
}
