// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The open score and how it is shown: reading the file, the View settings,
//! engraving, tools and PDF export. No window code here, so all of it is
//! tested without a display.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet, Theme};
use oxiguitar_host::Plugins;
use oxiguitar_layout::{Layout, Notation, Options, Paper, engrave};
use oxiguitar_model::Score;
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
    pub(crate) instrument: String,
    /// The track has strings to tab (not drums, not a track without strings).
    pub(crate) can_tab: bool,
    /// Its notation unless the user decides otherwise.
    pub(crate) default_notation: Notation,
}

/// A Tools menu entry for the open score.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolEntry {
    pub(crate) name: String,
    /// Why the tool cannot run on this score; `None` if it can.
    pub(crate) unavailable: Option<String>,
}

/// An open score.
pub(crate) struct Document {
    pub(crate) path: PathBuf,
    pub(crate) score: Score,
}

impl Document {
    /// Reads a score file with the format plugin that recognises it (the
    /// core reads no format itself). The error is a sentence for the footer.
    pub(crate) fn open(path: &Path, plugins: &Plugins) -> Result<Document, String> {
        let name = file_name(path);
        let data = std::fs::read(path).map_err(|e| format!("Cannot open {name}: {e}."))?;
        let extension = path.extension().and_then(|e| e.to_str());
        let format = plugins
            .recognise(&data, extension)
            .map_err(|why| format!("{name}: {why}"))?;
        let plugin = format.plugin.name();
        let read = plugins
            .read(format, &data)
            .map_err(|e| format!("{name}: {e}"))?;
        let score = oxiguitar_model::wire::from_wire(read.score).map_err(|e| {
            format!("{name}: the {plugin} plugin gave a score this OxiGuitar can't hold ({e}).")
        })?;
        Ok(Document {
            path: path.to_owned(),
            score,
        })
    }

    /// The tracks, for the track list.
    pub(crate) fn tracks(&self) -> Vec<TrackInfo> {
        self.score
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| TrackInfo {
                name: if t.name.trim().is_empty() {
                    format!("Track {}", i + 1)
                } else {
                    t.name.trim().to_owned()
                },
                instrument: if t.instrument.percussion {
                    "Drums".to_owned()
                } else {
                    match t.staves.first().map_or(0, |s| s.string_count()) {
                        0 => String::new(),
                        n => format!("{n} strings"),
                    }
                },
                can_tab: !t.instrument.percussion && t.staves.iter().any(|s| s.string_count() > 0),
                default_notation: oxiguitar_layout::default_notation(t),
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

    /// Applies the Tools menu's tool number `index`; returns its report.
    /// The file on disk is not touched.
    pub(crate) fn apply_tool(&mut self, index: usize) -> String {
        let Some(tool) = oxiguitar_tools::all().get(index) else {
            return String::new();
        };
        let outcome = tool.apply(&self.score);
        if let Some(changed) = outcome.score {
            self.score = changed;
        }
        format!("{}: {}", tool.name(), outcome.report)
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
    fn tools_menu_and_apply() {
        let mut doc = open(&fixture("gp5")).unwrap();
        let tools = doc.tools();
        assert_eq!(tools.len(), oxiguitar_tools::all().len());
        let report = doc.apply_tool(0);
        assert!(report.starts_with("Infer Key Signatures: "), "{report}");
        assert_eq!(doc.apply_tool(99), "");
    }
}
