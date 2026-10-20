// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The chord dialog (M3 slice 6), without window code: Guitar Pro 8's
//! Chord window (its guide, "Chord Diagrams", checked 20 Oct 2026; Mateus,
//! 20 Oct 2026: with its builder).
//!
//! The builder (root, type, extension, alterations, bass) names the chord
//! and lists the ways to play it on the track's strings, easiest first; a
//! click on one makes it the diagram. Clicks on the diagram's grid place
//! and take away notes, and above it open and muted strings; the chord
//! they make is named, as Guitar Pro suggests a chord once notes are
//! placed. A name typed by hand is kept as typed. The track's chords can
//! be picked again. With nothing chosen yet, the beat's notes start the
//! diagram.

use oxiguitar_edit::harmony::{self, Chord, Extension, Triad};
use oxiguitar_model::{ChordDiagram, NotePlace, Score};

/// Frets the grid shows from its first.
pub(crate) const ROWS: u8 = 5;
/// Ways to play the chord listed at most.
const VOICINGS: usize = 12;

/// The dialog's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChordForm {
    /// The open strings' pitches with the capo, highest string first.
    tuning: Vec<u8>,
    /// The track's last fret.
    frets_max: u8,
    pub(crate) name: String,
    /// The name follows the builder and the grid (until one is typed).
    auto_name: bool,
    /// The builder's chord; `None` for a name it can't make.
    pub(crate) chord: Option<Chord>,
    /// Fret per string, highest first; `None` not played.
    pub(crate) frets: Vec<Option<u8>>,
    /// The grid's first fret (1 at the nut).
    pub(crate) first_fret: u8,
    /// Whether the diagram is drawn (else the name alone).
    pub(crate) show_diagram: bool,
    /// The ways to play the builder's chord.
    pub(crate) voicings: Vec<Vec<Option<u8>>>,
    /// The track's chords, to pick again.
    pub(crate) library: Vec<ChordDiagram>,
}

impl ChordForm {
    /// The dialog for the caret's beat: its chord, or the beat's notes in
    /// a new diagram. `None` on a staff without strings, unless the beat
    /// has a chord already (a name).
    pub(crate) fn open(score: &Score, c: oxiguitar_edit::Caret) -> Option<ChordForm> {
        let track = score.tracks.get(c.track)?;
        let staff = track.staves.get(c.staff)?;
        let beat = c.voice_in(score).and_then(|v| v.beats.get(c.beat));
        let tuning: Vec<u8> = (1..=staff.string_count())
            .filter_map(|s| staff.pitch_of(u8::try_from(s).ok()?, 0).map(|p| p.0))
            .collect();
        let current = beat
            .and_then(|b| b.chord)
            .and_then(|i| track.chords.get(i))
            .cloned();
        let mut form = ChordForm {
            tuning,
            frets_max: staff.fret_count,
            name: String::new(),
            auto_name: true,
            chord: None,
            frets: vec![None; staff.string_count()],
            first_fret: 1,
            show_diagram: staff.string_count() > 0 && !track.instrument.percussion,
            voicings: Vec::new(),
            library: track.chords.clone(),
        };
        match current {
            Some(d) => form.load(&d),
            None => {
                for n in beat.iter().flat_map(|b| &b.notes) {
                    if let NotePlace::Fretted { string, fret } = n.place
                        && let Some(slot) = form.frets.get_mut(usize::from(string) - 1)
                    {
                        *slot = Some(fret);
                    }
                }
                form.first_fret = lowest_fretted(&form.frets).unwrap_or(1);
                form.recognise();
                if form.chord.is_none() {
                    form.chord = Some(Chord::major(0));
                    form.name = "C".to_owned();
                    form.rebuild(form.frets.iter().all(Option::is_none));
                }
            }
        }
        if form.tuning.is_empty() && form.name.is_empty() {
            return None;
        }
        Some(form)
    }

    /// A diagram of the track's, or the beat's own, loaded.
    fn load(&mut self, d: &ChordDiagram) {
        self.name = d.name.clone();
        self.chord = Chord::parse(&d.name);
        self.auto_name = self.chord.is_some();
        self.show_diagram = !d.frets.is_empty();
        if d.frets.len() == self.tuning.len() {
            self.frets = d.frets.clone();
        }
        self.first_fret = d.first_fret.max(1);
        self.rebuild(false);
    }

    /// The builder changed: the name follows (unless typed), the ways to
    /// play it are listed again, and with `play` the first becomes the
    /// diagram.
    fn rebuild(&mut self, play: bool) {
        let Some(c) = self.chord else {
            self.voicings.clear();
            return;
        };
        if self.auto_name {
            self.name = c.name();
        }
        self.voicings = c.voicings(&self.tuning, self.frets_max, VOICINGS);
        if play && let Some(v) = self.voicings.first().cloned() {
            self.pick_voicing_shape(v);
        }
    }

    /// A shape becomes the diagram, the grid starting at the nut when the
    /// shape fits the first frets, else at its lowest fret.
    fn pick_voicing_shape(&mut self, v: Vec<Option<u8>>) {
        let high = v.iter().flatten().max().copied().unwrap_or(0);
        self.first_fret = if high <= ROWS {
            1
        } else {
            lowest_fretted(&v).unwrap_or(1)
        };
        self.frets = v;
    }

    /// The diagram's notes named, as Guitar Pro suggests (not over a name
    /// typed by hand).
    fn recognise(&mut self) {
        if !self.auto_name {
            return;
        }
        let pitches: Vec<u8> = self
            .frets
            .iter()
            .zip(&self.tuning)
            .filter_map(|(f, open)| f.map(|f| open.saturating_add(f)))
            .collect();
        if let Some(c) = Chord::recognise(&pitches) {
            self.chord = Some(c);
            self.name = c.name();
            self.voicings = c.voicings(&self.tuning, self.frets_max, VOICINGS);
        }
    }

    /// The builder's root, type, extension, alteration or bass changed.
    pub(crate) fn build(&mut self, f: impl FnOnce(&mut Chord)) {
        let mut c = self.chord.unwrap_or_else(|| Chord::major(0));
        f(&mut c);
        if Some(c) == self.chord {
            return;
        }
        self.chord = Some(c);
        self.auto_name = true;
        self.rebuild(true);
    }

    /// A name typed: kept as typed, and the builder follows when it makes
    /// that name.
    pub(crate) fn type_name(&mut self, name: &str) {
        if name == self.name {
            return;
        }
        self.name = name.to_owned();
        self.auto_name = false;
        if let Some(c) = Chord::parse(name) {
            self.chord = Some(c);
            self.voicings = c.voicings(&self.tuning, self.frets_max, VOICINGS);
        }
    }

    /// A click on the grid: string `string` (0 the highest) at row `row`
    /// (0 the first fret shown) places a note there, or takes it away.
    pub(crate) fn click_grid(&mut self, string: usize, row: u8) {
        let fret = self.first_fret.saturating_add(row).min(self.frets_max);
        let Some(slot) = self.frets.get_mut(string) else {
            return;
        };
        *slot = if *slot == Some(fret) {
            None
        } else {
            Some(fret)
        };
        self.show_diagram = true;
        self.recognise();
    }

    /// A click above a string: open, then not played.
    pub(crate) fn click_head(&mut self, string: usize) {
        let Some(slot) = self.frets.get_mut(string) else {
            return;
        };
        *slot = if *slot == Some(0) { None } else { Some(0) };
        self.show_diagram = true;
        self.recognise();
    }

    /// The grid scrolled to start at `first` (1 at the nut).
    pub(crate) fn scroll(&mut self, first: u8) {
        self.first_fret = first.clamp(1, self.frets_max.max(1));
    }

    /// One of the ways to play it picked.
    pub(crate) fn pick_voicing(&mut self, i: usize) {
        if let Some(v) = self.voicings.get(i).cloned() {
            self.pick_voicing_shape(v);
            self.show_diagram = true;
        }
    }

    /// One of the track's chords picked.
    pub(crate) fn pick_library(&mut self, i: usize) {
        if let Some(d) = self.library.get(i).cloned() {
            self.load(&d);
        }
    }

    /// The diagram the dialog makes: `None` for no name and no notes.
    pub(crate) fn diagram(&self) -> Option<ChordDiagram> {
        let name = self.name.trim().to_owned();
        let frets = if self.show_diagram && !self.tuning.is_empty() {
            self.frets.clone()
        } else {
            Vec::new()
        };
        if name.is_empty() && frets.iter().all(Option::is_none) {
            return None;
        }
        // The first fret is never above a fretted note.
        let first = match lowest_fretted(&frets) {
            Some(low) => self.first_fret.min(low).max(1),
            None => 1,
        };
        Some(ChordDiagram {
            name,
            first_fret: if frets.is_empty() { 1 } else { first },
            frets,
        })
    }

    /// The strings, as a ways-to-play list shows them: low to high, "x"
    /// for not played ("x 3 2 0 1 0").
    pub(crate) fn shape_text(shape: &[Option<u8>]) -> String {
        shape
            .iter()
            .rev()
            .map(|f| f.map_or_else(|| "x".to_owned(), |f| f.to_string()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The grid's dots: for each string, low to high, the row of its note
    /// within the grid (−1 open, −2 not played, −3 above or below the
    /// grid).
    pub(crate) fn dots(&self) -> Vec<i32> {
        self.frets
            .iter()
            .rev()
            .map(|f| match f {
                None => -2,
                Some(0) => -1,
                Some(f) if *f >= self.first_fret && *f < self.first_fret + ROWS => {
                    i32::from(*f - self.first_fret)
                }
                Some(_) => -3,
            })
            .collect()
    }

    /// The track's last fret.
    pub(crate) fn frets_max(&self) -> u8 {
        self.frets_max
    }

    /// The number of strings.
    pub(crate) fn strings(&self) -> usize {
        self.tuning.len()
    }
}

/// The lowest fret above the nut that a shape uses.
fn lowest_fretted(frets: &[Option<u8>]) -> Option<u8> {
    frets.iter().flatten().filter(|f| **f > 0).min().copied()
}

/// The builder's lists, for its drop-downs.
pub(crate) fn note_names() -> Vec<String> {
    harmony::NOTE_NAMES
        .iter()
        .map(|n| (*n).to_owned())
        .collect()
}

pub(crate) fn triad_names() -> Vec<String> {
    Triad::ALL.iter().map(|t| t.label().to_owned()).collect()
}

pub(crate) fn extension_names() -> Vec<String> {
    Extension::ALL
        .iter()
        .map(|e| e.label().to_owned())
        .collect()
}

/// The bass drop-down: the root first, then every note.
pub(crate) fn bass_names() -> Vec<String> {
    std::iter::once("Root".to_owned())
        .chain(note_names())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use oxiguitar_edit::{Caret, Command, History, apply, new_score};

    fn at(beat: usize, string: u8) -> Caret {
        Caret {
            beat,
            string,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    #[test]
    fn the_beat_s_notes_start_the_diagram_and_are_named() {
        let mut s = new_score();
        let mut h = History::new();
        // x 0 2 2 1 0: A minor.
        for (string, fret) in [(5, 0), (4, 2), (3, 2), (2, 1), (1, 0)] {
            apply(
                &mut s,
                &mut h,
                at(0, string),
                None,
                Command::Fret { fret, again: false },
            )
            .unwrap();
        }
        let f = ChordForm::open(&s, at(0, 1)).unwrap();
        assert_eq!(f.name, "Am");
        assert_eq!(ChordForm::shape_text(&f.frets), "x 0 2 2 1 0");
        assert_eq!(f.dots(), [-2, -1, 1, 1, 0, -1]);
        assert!(!f.voicings.is_empty());
        let d = f.diagram().unwrap();
        assert_eq!((d.name.as_str(), d.first_fret), ("Am", 1));
    }

    #[test]
    fn the_builder_names_and_plays() {
        let s = new_score();
        let mut f = ChordForm::open(&s, at(0, 1)).unwrap();
        // An empty beat: C, played the easiest way.
        assert_eq!(f.name, "C");
        assert_eq!(ChordForm::shape_text(&f.frets), "x 3 2 0 1 0");
        f.build(|c| {
            c.root = 10; // G
            c.extension = Extension::Seventh;
        });
        assert_eq!(f.name, "G7");
        f.build(|c| c.bass = Some(15)); // over Bb? not a chord note
        assert_eq!(f.name, "G7/Bb");
        // A typed name is kept; the builder follows what it can read.
        f.type_name("G7 (my way)");
        assert_eq!(f.name, "G7 (my way)");
        f.click_grid(0, 0);
        assert_eq!(f.name, "G7 (my way)");
        f.type_name("Dm");
        assert_eq!(f.chord.unwrap().name(), "Dm");
        // Pressing over the top string: open, then not played.
        f.click_head(0);
        assert_eq!(f.frets[0], Some(0));
        f.click_head(0);
        assert_eq!(f.frets[0], None);
        // The name alone.
        f.show_diagram = false;
        assert_eq!(f.diagram().unwrap().frets, Vec::<Option<u8>>::new());
    }

    #[test]
    fn a_barre_starts_the_grid_at_its_fret() {
        let s = new_score();
        let mut f = ChordForm::open(&s, at(0, 1)).unwrap();
        f.pick_voicing_shape(vec![Some(7), Some(8), Some(9), Some(9), Some(7), None]);
        assert_eq!(f.first_fret, 7);
        assert_eq!(f.diagram().unwrap().first_fret, 7);
        f.scroll(9);
        assert_eq!(f.diagram().unwrap().first_fret, 7, "never above a note");
        assert_eq!(f.dots()[0], -2);
        assert_eq!(f.dots()[1], -3);
    }
}
