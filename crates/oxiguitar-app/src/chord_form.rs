// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The chord dialog (#278), without window code.
//!
//! Three views of one chord follow one another: the diagram, the voicing
//! in inline notation (`x 0 2 2 1 0`, low string first) and the name.
//! The diagram and the voicing are the same frets, so a click on the grid
//! rewrites the text and a typed text moves the dots. From the frets come
//! the names their notes can take, one for each note as the root; from a
//! name, the ways to play it on the track's strings, easiest first. When
//! one side changes, the other keeps its value if that is still among
//! the possibilities and takes the first of them otherwise.
//!
//! Names are read in any convention and shown in the chord notation style
//! picked (`oxiguitar_edit::chord_names`); a name the library can't read
//! is kept as typed, with whatever the diagram holds.

use oxiguitar_edit::chord_names::{self, Reading, Styles};
use oxiguitar_model::{ChordDiagram, NotePlace, Score};

/// Frets the grid shows from its first.
pub(crate) const ROWS: u8 = 5;
/// Ways to play a name listed at most.
const VOICINGS: usize = 12;

/// The dialog's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChordForm {
    /// The open strings' pitches with the capo, highest string first.
    tuning: Vec<u8>,
    /// The track's last fret.
    frets_max: u8,
    /// The name, as typed or picked.
    pub(crate) name: String,
    /// The names the diagram's notes can take; with no note placed, the
    /// track's chords, to pick again.
    pub(crate) names: Vec<String>,
    /// The voicing in inline notation, as typed or as the diagram reads.
    pub(crate) voicing: String,
    /// The ways to play the name.
    pub(crate) voicings: Vec<Vec<Option<u8>>>,
    /// Fret per string, highest first; `None` not played.
    pub(crate) frets: Vec<Option<u8>>,
    /// The grid's first fret (1 at the nut).
    pub(crate) first_fret: u8,
    /// Whether the chord gets its diagram (else its name alone).
    pub(crate) add_diagram: bool,
    /// The chord notation styles, and the one names are shown in.
    styles: Styles,
    pub(crate) style: usize,
    /// The track's chords.
    library: Vec<ChordDiagram>,
}

impl ChordForm {
    /// The dialog for the caret's beat: its chord, or the beat's notes as
    /// a voicing, named. `add_diagram` is the user's last choice, which a
    /// new chord takes; a chord already there shows its own. `None` on a
    /// staff without strings, unless the beat has a chord already (a
    /// name).
    pub(crate) fn open(
        score: &Score,
        c: oxiguitar_edit::Caret,
        styles: Styles,
        style: usize,
        add_diagram: bool,
    ) -> Option<ChordForm> {
        let track = score.tracks.get(c.track)?;
        let staff = track.staves.get(c.staff)?;
        let beat = c.voice_in(score).and_then(|v| v.beats.get(c.beat));
        let tuning: Vec<u8> = (1..=staff.string_count())
            .filter_map(|s| staff.pitch_of(u8::try_from(s).ok()?, 0).map(|p| p.0))
            .collect();
        let current = beat.and_then(|b| b.chord).and_then(|i| track.chords.get(i));
        let stringed = !tuning.is_empty() && !track.instrument.percussion;
        let mut form = ChordForm {
            frets: vec![None; tuning.len()],
            tuning,
            frets_max: staff.fret_count,
            name: String::new(),
            names: Vec::new(),
            voicing: String::new(),
            voicings: Vec::new(),
            first_fret: 1,
            add_diagram: add_diagram && stringed,
            styles,
            style,
            library: track.chords.clone(),
        };
        match current {
            Some(d) => {
                form.name.clone_from(&d.name);
                form.add_diagram = !d.frets.is_empty();
                if d.frets.len() == form.frets.len() {
                    form.frets.clone_from(&d.frets);
                }
                form.first_fret = d.first_fret.max(1);
                form.voicing = shape_text(&form.frets);
                form.names = form.names_of_frets();
                form.voicings = form.voicings_of_name();
            }
            None => {
                for n in beat.iter().flat_map(|b| &b.notes) {
                    if let NotePlace::Fretted { string, fret } = n.place
                        && let Some(slot) = form.frets.get_mut(usize::from(string) - 1)
                    {
                        *slot = Some(fret);
                    }
                }
                form.frame();
                form.voicing = shape_text(&form.frets);
                form.frets_changed();
            }
        }
        if form.tuning.is_empty() && form.name.is_empty() {
            return None;
        }
        Some(form)
    }

    /// The name, read as the style picked writes names.
    fn reading(&self, name: &str) -> Option<Reading> {
        self.styles.read(self.style, name)
    }

    /// The pitches the diagram sounds.
    fn pitches(&self) -> Vec<u8> {
        self.frets
            .iter()
            .zip(&self.tuning)
            .filter_map(|(f, open)| f.map(|f| open.saturating_add(f)))
            .collect()
    }

    /// The Name list: the names the diagram's notes can take, in the
    /// style picked; with no note placed, the track's chords.
    fn names_of_frets(&self) -> Vec<String> {
        let pitches = self.pitches();
        let names = if pitches.is_empty() {
            self.library.iter().map(|d| d.name.clone()).collect()
        } else {
            chord_names::names(&pitches)
        };
        let mut out: Vec<String> = Vec::new();
        for n in names {
            let n = self.styles.write(self.style, &n);
            if !n.is_empty() && !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }

    /// The Voicing list: the way the track already plays a chord of this
    /// name, then the ways to play the name, easiest first.
    fn voicings_of_name(&self) -> Vec<Vec<Option<u8>>> {
        let own = self
            .library
            .iter()
            .filter(|d| d.name == self.name && d.frets.len() == self.tuning.len())
            .map(|d| d.frets.clone());
        let found = self
            .reading(&self.name)
            .map(|r| r.voicings(&self.tuning, self.frets_max, VOICINGS))
            .unwrap_or_default();
        let mut out: Vec<Vec<Option<u8>>> = Vec::new();
        for v in own.chain(found) {
            if !out.contains(&v) {
                out.push(v);
            }
        }
        out
    }

    /// The diagram changed: its names are listed, and the name stays if
    /// it is one of them (in any spelling), else becomes the first. Then
    /// the ways to play the name are listed again.
    fn frets_changed(&mut self) {
        self.names = self.names_of_frets();
        if !self.pitches().is_empty() {
            let now = self.reading(&self.name);
            let stays = self.names.iter().any(|n| {
                *n == self.name
                    || matches!((&now, self.reading(n)), (Some(a), Some(b)) if a.same(&b))
            });
            if !stays && let Some(first) = self.names.first() {
                self.name.clone_from(first);
            }
        }
        self.voicings = self.voicings_of_name();
    }

    /// The name changed: the ways to play it are listed, and the diagram
    /// stays if it plays the name, else becomes the first of them. A name that
    /// can't be read changes nothing else.
    fn name_changed(&mut self) {
        self.voicings = self.voicings_of_name();
        let plays = self
            .reading(&self.name)
            .is_some_and(|r| r.is_played_by(&self.pitches()));
        if plays || self.voicings.contains(&self.frets) {
            return;
        }
        if let Some(first) = self.voicings.first().cloned() {
            self.frets = first;
            self.frame();
            self.voicing = shape_text(&self.frets);
            self.names = self.names_of_frets();
        }
    }

    /// The grid moved to show the diagram's notes: at the nut when they
    /// fit the first frets, else at the lowest fretted one. A grid that
    /// shows them all already stays.
    fn frame(&mut self) {
        let fretted = || self.frets.iter().flatten().filter(|f| **f > 0);
        let (Some(low), Some(high)) = (fretted().min().copied(), fretted().max().copied()) else {
            return;
        };
        if low >= self.first_fret && high < self.first_fret.saturating_add(ROWS) {
            return;
        }
        self.first_fret = if high <= ROWS { 1 } else { low };
    }

    /// A name typed.
    pub(crate) fn type_name(&mut self, name: &str) {
        if name == self.name {
            return;
        }
        name.clone_into(&mut self.name);
        self.name_changed();
    }

    /// A name picked from the list.
    pub(crate) fn pick_name(&mut self, i: usize) {
        if let Some(n) = self.names.get(i).cloned() {
            self.type_name(&n);
        }
    }

    /// A voicing typed: when it reads as frets for the track's strings,
    /// they become the diagram. The text stays as typed.
    pub(crate) fn type_voicing(&mut self, text: &str) {
        if text == self.voicing {
            return;
        }
        text.clone_into(&mut self.voicing);
        if let Some(frets) = parse_shape(text, self.tuning.len(), self.frets_max)
            && frets != self.frets
        {
            self.frets = frets;
            self.frame();
            self.frets_changed();
        }
    }

    /// A voicing picked from the list.
    pub(crate) fn pick_voicing(&mut self, i: usize) {
        if let Some(v) = self.voicings.get(i).cloned() {
            self.frets = v;
            self.frame();
            self.voicing = shape_text(&self.frets);
            self.frets_changed();
        }
    }

    /// A click on the grid: string `string` (0 the highest) at row `row`
    /// (0 the first fret shown) places a note there, or takes it away.
    pub(crate) fn click_grid(&mut self, string: usize, row: u8) {
        let fret = self.first_fret.saturating_add(row).min(self.frets_max);
        self.toggle(string, fret);
    }

    /// A click above a string: open, then not played.
    pub(crate) fn click_head(&mut self, string: usize) {
        self.toggle(string, 0);
    }

    fn toggle(&mut self, string: usize, fret: u8) {
        let Some(slot) = self.frets.get_mut(string) else {
            return;
        };
        *slot = if *slot == Some(fret) {
            None
        } else {
            Some(fret)
        };
        self.voicing = shape_text(&self.frets);
        self.frets_changed();
    }

    /// The grid scrolled to start at `first` (1 at the nut).
    pub(crate) fn scroll(&mut self, first: u8) {
        self.first_fret = first.clamp(1, self.frets_max.max(1));
    }

    /// A style picked: the name and the names listed are rewritten in it.
    pub(crate) fn pick_style(&mut self, style: usize) {
        self.style = style;
        self.name = self.styles.write(style, &self.name);
        self.names = self.names_of_frets();
    }

    /// The name of the style picked, as settings.json keeps it.
    pub(crate) fn style_name(&self) -> Option<String> {
        self.styles.names().into_iter().nth(self.style)
    }

    /// The styles' names.
    pub(crate) fn style_names(&self) -> Vec<String> {
        self.styles.names()
    }

    /// `name` in the style picked.
    pub(crate) fn in_style(&self, name: &str) -> String {
        self.styles.write(self.style, name)
    }

    /// The style was applied to the whole song: the track's chords, as
    /// the dialog knows them, are renamed too.
    pub(crate) fn song_restyled(&mut self) {
        for d in &mut self.library {
            d.name = self.styles.write(self.style, &d.name);
        }
        self.names = self.names_of_frets();
    }

    /// The diagram the dialog makes: `None` for no name and no notes,
    /// which takes the beat's chord away.
    pub(crate) fn diagram(&self) -> Option<ChordDiagram> {
        let name = self.name.trim().to_owned();
        let frets = if self.add_diagram && !self.tuning.is_empty() {
            self.frets.clone()
        } else {
            Vec::new()
        };
        if name.is_empty() && frets.iter().all(Option::is_none) {
            return None;
        }
        // The first fret is never above a fretted note.
        let first = match frets.iter().flatten().filter(|f| **f > 0).min() {
            Some(low) => self.first_fret.min(*low).max(1),
            None => 1,
        };
        Some(ChordDiagram {
            name,
            first_fret: first,
            frets,
        })
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

/// A shape in inline notation: the strings low to high, "x" for one not
/// played, a space between them so frets of two digits read clearly
/// ("x 0 2 2 1 0", "x 12 14 14 13 12"). Empty when no string is played.
pub(crate) fn shape_text(shape: &[Option<u8>]) -> String {
    if shape.iter().all(Option::is_none) {
        return String::new();
    }
    shape
        .iter()
        .rev()
        .map(|f| f.map_or_else(|| "x".to_owned(), |f| f.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Inline notation read as frets for `strings` strings, highest first:
/// "x 0 2 2 1 0" with spaces, or "x02210" without (then every fret is one
/// digit). Strings the text doesn't reach are not played, so a voicing
/// can be typed from the low string up. `None` for anything else: more
/// items than strings, a fret past `frets_max`, another character.
pub(crate) fn parse_shape(text: &str, strings: usize, frets_max: u8) -> Option<Vec<Option<u8>>> {
    let text = text.trim();
    let spaced = text.contains(char::is_whitespace);
    let items: Vec<String> = if spaced {
        text.split_whitespace().map(str::to_owned).collect()
    } else {
        text.chars().map(String::from).collect()
    };
    if items.len() > strings {
        return None;
    }
    let mut low_first = items
        .iter()
        .map(|item| match item.as_str() {
            "x" | "X" => Some(None),
            digits => digits
                .parse::<u8>()
                .ok()
                .filter(|f| *f <= frets_max)
                .map(Some),
        })
        .collect::<Option<Vec<Option<u8>>>>()?;
    low_first.resize(strings, None);
    low_first.reverse();
    Some(low_first)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::chord_prefs;
    use oxiguitar_edit::{Caret, Command, History, apply, new_score};

    fn at(beat: usize, string: u8) -> Caret {
        Caret {
            beat,
            string,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    /// The dialog on `score`, in the style named `style`.
    fn open_in(score: &Score, style: &str, add_diagram: bool) -> ChordForm {
        let styles = chord_prefs::styles(&chord_prefs::first_styles());
        let style = styles.position(style).unwrap();
        ChordForm::open(score, at(0, 1), styles, style, add_diagram).unwrap()
    }

    fn open() -> ChordForm {
        open_in(&new_score(), "Popular", true)
    }

    #[test]
    fn inline_notation_both_ways() {
        let am = vec![Some(0), Some(1), Some(2), Some(2), Some(0), None];
        assert_eq!(shape_text(&am), "x 0 2 2 1 0");
        assert_eq!(parse_shape("x 0 2 2 1 0", 6, 24), Some(am.clone()));
        assert_eq!(parse_shape("x02210", 6, 24), Some(am.clone()));
        assert_eq!(parse_shape("  X  0 2  2 1 0 ", 6, 24), Some(am));
        let high = vec![Some(12), Some(13), Some(14), Some(14), Some(12), None];
        assert_eq!(shape_text(&high), "x 12 14 14 13 12");
        assert_eq!(parse_shape("x 12 14 14 13 12", 6, 24), Some(high));
        // Typed from the low string up; the rest is not played.
        let b = vec![None, None, None, None, Some(2), None];
        assert_eq!(parse_shape("x2", 6, 24), Some(b));
        assert_eq!(parse_shape("", 6, 24), Some(vec![None; 6]));
        assert_eq!(shape_text(&[None; 6]), "");
        for bad in ["x 0 2 2 1 0 0", "x 0 2 y", "x 30", "0221003", "x-0"] {
            assert_eq!(parse_shape(bad, 6, 24), None, "{bad}");
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
        let f = open_in(&s, "Popular", true);
        assert_eq!(f.name, "Am");
        assert_eq!(f.names[0], "Am");
        assert_eq!(f.voicing, "x 0 2 2 1 0");
        assert_eq!(f.dots(), [-2, -1, 1, 1, 0, -1]);
        assert!(f.voicings.contains(&f.frets));
        let d = f.diagram().unwrap();
        assert_eq!((d.name.as_str(), d.first_fret), ("Am", 1));
        assert_eq!(open_in(&s, "Jazz", true).name, "A-");
        assert_eq!(open_in(&s, "Custom", true).name, "Ami");
    }

    #[test]
    fn the_diagram_the_voicing_and_the_name_follow_one_another() {
        // #278's walk, low string first: the 5th string is index 4 from
        // the highest, and the grid's row 1 is the 2nd fret.
        let mut f = open();
        assert_eq!((f.name.as_str(), f.voicing.as_str()), ("", ""));
        assert_eq!(f.diagram(), None);
        f.click_grid(4, 1);
        assert_eq!((f.voicing.as_str(), f.name.as_str()), ("x 2 x x x x", "B"));
        f.click_grid(3, 2);
        assert_eq!(f.voicing, "x 2 3 x x x");
        assert_eq!(f.name, "B(b5,omit3)");
        f.type_voicing("x2323x");
        assert_eq!(f.dots(), [-2, 1, 2, 1, 2, -2]);
        assert_eq!(f.voicing, "x2323x", "as typed");
        assert_eq!(f.name, "Bm7b5");
        assert_eq!(f.names.len(), 4);
        assert!(f.names.contains(&"Dm6/B".to_owned()), "{:?}", f.names);
        // Another of its names: the diagram is one way to play it, so it
        // stays.
        let shape = f.frets.clone();
        f.pick_name(f.names.iter().position(|n| n == "Dm6/B").unwrap());
        assert_eq!(f.frets, shape);
        // A text that is no voicing leaves everything as it was.
        f.type_voicing("x2323x?");
        assert_eq!((f.frets.clone(), f.name.as_str()), (shape, "Dm6/B"));
    }

    #[test]
    fn a_name_lists_the_ways_to_play_it() {
        let mut f = open();
        f.type_name("Am");
        assert_eq!(f.voicing, "x 0 2 2 1 0");
        assert!(f.voicings.len() > 3);
        assert!(f.names.contains(&"Am".to_owned()));
        // Still A minor in another spelling: nothing moves.
        f.type_name("Amin");
        assert_eq!(
            (f.voicing.as_str(), f.name.as_str()),
            ("x 0 2 2 1 0", "Amin")
        );
        // Another way to play it; the name stays, as it names that too.
        f.pick_voicing(1);
        assert_ne!(f.voicing, "x 0 2 2 1 0");
        assert_eq!(f.name, "Amin");
        // Another chord: the first way to play it.
        f.type_name("Am7");
        assert_eq!(f.voicing, "x 0 2 0 1 0");
        // A name nobody reads keeps the diagram, and is what is written.
        f.type_name("Am7 (my way)");
        assert_eq!(f.voicing, "x 0 2 0 1 0");
        assert_eq!(f.diagram().unwrap().name, "Am7 (my way)");
        // A click then names the notes again.
        f.click_head(0);
        assert_ne!(f.name, "Am7 (my way)");
    }

    #[test]
    fn a_shape_up_the_neck_moves_the_grid() {
        let mut f = open();
        f.type_voicing("x 12 14 14 13 12");
        assert_eq!(f.first_fret, 12);
        assert_eq!(f.name, "Am");
        assert_eq!(f.diagram().unwrap().first_fret, 12);
        f.scroll(14);
        assert_eq!(f.diagram().unwrap().first_fret, 12, "never above a note");
        assert_eq!(f.dots()[1], -3);
        f.type_voicing("x 0 2 2 1 0");
        assert_eq!(f.first_fret, 1);
    }

    #[test]
    fn a_style_rewrites_the_names_shown() {
        let mut f = open();
        f.type_voicing("x2323x");
        assert_eq!(f.name, "Bm7b5");
        f.pick_style(f.style_names().iter().position(|n| n == "Compact").unwrap());
        assert_eq!(f.name, "Bø");
        assert_eq!(f.names[0], "Bø");
        assert_eq!(f.style_name().as_deref(), Some("Compact"));
        assert_eq!(f.in_style("Daug"), "D+");
        // The name is still read: its voicings are there.
        assert!(f.voicings.contains(&f.frets));
        f.type_name("Eø");
        assert!(f.voicings.contains(&f.frets) && f.name == "Eø");
    }

    #[test]
    fn the_track_s_chords_are_offered_and_the_diagram_is_a_choice() {
        let mut s = new_score();
        let mut h = History::new();
        let own = ChordDiagram {
            name: "Am".into(),
            first_fret: 5,
            frets: vec![Some(5), Some(5), Some(5), Some(7), Some(7), Some(5)],
        };
        apply(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Command::Chord(Some(own.clone())),
        )
        .unwrap();
        // On the beat that has it: the chord as it is.
        let f = open_in(&s, "Popular", false);
        assert_eq!((f.name.as_str(), f.voicing.as_str()), ("Am", "5 7 7 5 5 5"));
        assert!(
            f.add_diagram,
            "the chord's own diagram, not the last choice"
        );
        assert_eq!(f.diagram(), Some(own.clone()));
        // On an empty beat: the track's chords to pick, played its way.
        let styles = chord_prefs::styles(&chord_prefs::first_styles());
        let mut f = ChordForm::open(&s, at(1, 1), styles, 3, false).unwrap();
        assert_eq!(f.names, ["Am"]);
        f.pick_name(0);
        assert_eq!(f.frets, own.frets);
        assert_eq!(f.first_fret, 5);
        // The last choice was no diagram: the name alone.
        assert!(!f.add_diagram);
        assert_eq!(f.diagram().unwrap().frets, Vec::<Option<u8>>::new());
        // Nothing at all takes the chord away.
        f.type_name("");
        assert_eq!(f.diagram(), None);
        // The song restyled: the dialog's copy of the chords follows.
        f.pick_style(2);
        f.type_voicing("");
        f.song_restyled();
        assert_eq!(f.names, ["A-"]);
    }
}
