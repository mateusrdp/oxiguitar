// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Edits as commands, with unlimited undo and redo (EDIT-5).
//!
//! A command declares the parts of the score it changes ([`Region`]s);
//! [`History::record`] copies those parts before and after the change, and
//! undo and redo put the copies back. Copying only what a command touches
//! keeps a long history cheap: typing a note costs one bar of one staff.
//! Commands that change the score's shape (bars or tracks added or removed)
//! use [`Region::Whole`].
//!
//! Each change also says what it touched ([`Touched`]), so the bar check
//! and the window redo only that.

use std::ops::Range;

use oxiguitar_model::{Bar, MasterBar, Score, Staff, Track};

use crate::StaffBar;

/// A part of the score a command changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Region {
    /// Bars `bars` of one staff: their notes, voices and clefs.
    Bars {
        /// The track.
        track: usize,
        /// The staff in the track.
        staff: usize,
        /// The bars.
        bars: Range<usize>,
    },
    /// What every track shares in these bars: time and key signatures,
    /// repeats, tempo, sections.
    MasterBars(Range<usize>),
    /// A track's own settings (name, instrument, mixer, tuning, capo,
    /// chord diagrams, lyrics), not its bars.
    Track(usize),
    /// The score's header (title, artist, tempo at the start).
    Info,
    /// Everything: for commands that add or remove bars or tracks.
    Whole,
}

/// What a change touched, for the bar check and the window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    /// The staves' bars whose beats or time signature changed.
    pub bars: Vec<StaffBar>,
    /// The whole score may have changed: check and redraw everything.
    pub everything: bool,
}

impl Touched {
    /// Everything.
    pub fn everything() -> Touched {
        Touched {
            bars: Vec::new(),
            everything: true,
        }
    }

    fn of(regions: &[Region], score: &Score) -> Touched {
        let mut t = Touched::default();
        for r in regions {
            match r {
                Region::Bars { track, staff, bars } => {
                    t.bars.extend(bars.clone().map(|bar| StaffBar {
                        track: *track,
                        staff: *staff,
                        bar,
                    }));
                }
                Region::MasterBars(bars) => {
                    for (track, tr) in score.tracks.iter().enumerate() {
                        for staff in 0..tr.staves.len() {
                            t.bars
                                .extend(bars.clone().map(|bar| StaffBar { track, staff, bar }));
                        }
                    }
                }
                Region::Track(_) | Region::Info => {}
                Region::Whole => t.everything = true,
            }
        }
        if t.everything {
            t.bars.clear();
        } else {
            t.bars.sort_unstable();
            t.bars.dedup();
        }
        t
    }
}

/// A copy of one region.
#[derive(Clone, Debug)]
enum Saved {
    Bars {
        track: usize,
        staff: usize,
        start: usize,
        bars: Vec<Bar>,
    },
    MasterBars {
        start: usize,
        bars: Vec<MasterBar>,
    },
    Track {
        index: usize,
        track: Box<Track>,
    },
    Info {
        info: Box<oxiguitar_model::ScoreInfo>,
        tempo: f64,
        label: String,
    },
    Whole(Box<Score>),
}

/// A track's own settings, without its bars. Every field is spelled out,
/// so a field added to the model must be decided here.
fn settings_of(track: &Track) -> Track {
    let Track {
        name,
        short_name,
        color,
        instrument,
        mix,
        staves,
        chords,
        lyrics,
        plugins,
    } = track;
    Track {
        name: name.clone(),
        short_name: short_name.clone(),
        color: *color,
        instrument: *instrument,
        mix: *mix,
        staves: staves.iter().map(staff_settings).collect(),
        chords: chords.clone(),
        lyrics: lyrics.clone(),
        // Plugins' settings are the track's own: undone with the track
        // dialog's change.
        plugins: plugins.clone(),
    }
}

/// A staff's settings, without its bars.
fn staff_settings(staff: &Staff) -> Staff {
    let Staff {
        tuning,
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars: _,
    } = staff;
    Staff {
        tuning: tuning.clone(),
        capo: *capo,
        fret_count: *fret_count,
        transposition: *transposition,
        show_standard: *show_standard,
        show_tablature: *show_tablature,
        bars: Vec::new(),
    }
}

impl Saved {
    fn take(region: &Region, score: &Score) -> Option<Saved> {
        Some(match region {
            Region::Bars { track, staff, bars } => Saved::Bars {
                track: *track,
                staff: *staff,
                start: bars.start,
                bars: score
                    .tracks
                    .get(*track)?
                    .staves
                    .get(*staff)?
                    .bars
                    .get(bars.clone())?
                    .to_vec(),
            },
            Region::MasterBars(bars) => Saved::MasterBars {
                start: bars.start,
                bars: score.master_bars.get(bars.clone())?.to_vec(),
            },
            Region::Track(i) => Saved::Track {
                index: *i,
                track: Box::new(settings_of(score.tracks.get(*i)?)),
            },
            Region::Info => Saved::Info {
                info: Box::new(score.info.clone()),
                tempo: score.initial_tempo,
                label: score.tempo_label.clone(),
            },
            Region::Whole => Saved::Whole(Box::new(score.clone())),
        })
    }

    /// Puts the copy back into `score`.
    fn restore(&self, score: &mut Score) {
        match self {
            Saved::Bars {
                track,
                staff,
                start,
                bars,
            } => {
                if let Some(dst) = score
                    .tracks
                    .get_mut(*track)
                    .and_then(|t| t.staves.get_mut(*staff))
                    .and_then(|s| s.bars.get_mut(*start..*start + bars.len()))
                {
                    dst.clone_from_slice(bars);
                }
            }
            Saved::MasterBars { start, bars } => {
                if let Some(dst) = score.master_bars.get_mut(*start..*start + bars.len()) {
                    dst.clone_from_slice(bars);
                }
            }
            Saved::Track { index, track } => {
                if let Some(dst) = score.tracks.get_mut(*index) {
                    // The bars stay; the settings come back.
                    let mut staves = std::mem::take(&mut dst.staves);
                    *dst = settings_of(track);
                    for (s, saved) in staves.iter_mut().zip(&track.staves) {
                        let bars = std::mem::take(&mut s.bars);
                        *s = Staff {
                            bars,
                            ..staff_settings(saved)
                        };
                    }
                    dst.staves = staves;
                }
            }
            Saved::Info { info, tempo, label } => {
                score.info = (**info).clone();
                score.initial_tempo = *tempo;
                score.tempo_label = label.clone();
            }
            Saved::Whole(s) => *score = (**s).clone(),
        }
    }
}

/// One change: what it was called, the copies before and after, and what
/// it touched.
#[derive(Clone, Debug)]
pub struct Change {
    /// The command's name, as the Edit menu shows it ("Undo Insert Bar").
    pub label: String,
    id: u64,
    regions: Vec<Region>,
    before: Vec<Saved>,
    after: Vec<Saved>,
}

/// The undo and redo stacks.
#[derive(Clone, Debug, Default)]
pub struct History {
    done: Vec<Change>,
    undone: Vec<Change>,
    next_id: u64,
    /// The id of the last change done when the score was saved (0: the
    /// score as it was opened).
    saved: u64,
}

impl History {
    /// An empty history.
    pub fn new() -> History {
        History {
            next_id: 1,
            ..History::default()
        }
    }

    /// Runs `edit` on `score` as one command named `label`, which changes
    /// only `regions`, and records it. Redo is lost, as in every editor.
    /// Returns what it touched.
    ///
    /// The command must not change anything outside `regions`: those alone
    /// are put back by undo. Regions are copied in order, so a command
    /// whose regions overlap still undoes correctly.
    pub fn record(
        &mut self,
        score: &mut Score,
        label: &str,
        regions: Vec<Region>,
        edit: impl FnOnce(&mut Score),
    ) -> Touched {
        let before: Vec<Saved> = regions
            .iter()
            .filter_map(|r| Saved::take(r, score))
            .collect();
        edit(score);
        let after: Vec<Saved> = regions
            .iter()
            .filter_map(|r| Saved::take(r, score))
            .collect();
        let touched = Touched::of(&regions, score);
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        self.done.push(Change {
            label: label.to_owned(),
            id,
            regions,
            before,
            after,
        });
        self.undone.clear();
        touched
    }

    /// Gives the last change another name (a cut is a delete, named so).
    pub(crate) fn rename_last(&mut self, label: &str) {
        if let Some(c) = self.done.last_mut() {
            label.clone_into(&mut c.label);
        }
    }

    /// Undoes the last change. Returns its name and what it touched, or
    /// `None` when there is nothing to undo.
    pub fn undo(&mut self, score: &mut Score) -> Option<(String, Touched)> {
        let change = self.done.pop()?;
        // In reverse, so overlapping regions end as they first were.
        for s in change.before.iter().rev() {
            s.restore(score);
        }
        let out = (change.label.clone(), Touched::of(&change.regions, score));
        self.undone.push(change);
        Some(out)
    }

    /// Redoes the last undone change. Returns its name and what it touched,
    /// or `None` when there is nothing to redo.
    pub fn redo(&mut self, score: &mut Score) -> Option<(String, Touched)> {
        let change = self.undone.pop()?;
        for s in &change.after {
            s.restore(score);
        }
        let out = (change.label.clone(), Touched::of(&change.regions, score));
        self.done.push(change);
        Some(out)
    }

    /// The name of the change Undo would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.done.last().map(|c| c.label.as_str())
    }

    /// The name of the change Redo would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.undone.last().map(|c| c.label.as_str())
    }

    /// The score as it stands has been saved (or is as it was opened).
    pub fn mark_saved(&mut self) {
        self.saved = self.done.last().map_or(0, |c| c.id);
    }

    /// The score differs from what was last saved or opened.
    pub fn is_modified(&self) -> bool {
        self.done.last().map_or(0, |c| c.id) != self.saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caret::tests::score;
    use oxiguitar_model::{NoteValue, Pitch, TimeSignature};

    #[test]
    fn undo_and_redo_put_back_only_what_changed() {
        let mut s = score(2, 3);
        let original = s.clone();
        let mut h = History::new();
        assert!(!h.is_modified());
        let t = h.record(
            &mut s,
            "Delete Beat",
            vec![Region::Bars {
                track: 1,
                staff: 0,
                bars: 1..2,
            }],
            |s| {
                s.tracks[1].staves[0].bars[1].voices[0].beats.pop();
            },
        );
        assert_eq!(
            t.bars,
            [StaffBar {
                track: 1,
                staff: 0,
                bar: 1
            }]
        );
        let edited = s.clone();
        assert!(h.is_modified());
        assert_eq!(h.undo_label(), Some("Delete Beat"));
        let (label, _) = h.undo(&mut s).unwrap();
        assert_eq!(label, "Delete Beat");
        assert_eq!(s, original);
        assert!(!h.is_modified());
        assert_eq!(h.redo_label(), Some("Delete Beat"));
        h.redo(&mut s).unwrap();
        assert_eq!(s, edited);
        assert!(h.undo(&mut s).is_some() && h.undo(&mut s).is_none());
    }

    #[test]
    fn a_new_command_drops_redo_and_saving_moves_the_mark() {
        let mut s = score(1, 2);
        let mut h = History::new();
        let bars = || vec![Region::MasterBars(0..1)];
        h.record(&mut s, "Time Signature", bars(), |s| {
            s.master_bars[0].time_signature = TimeSignature {
                numerator: 3,
                denominator: 4,
            };
        });
        h.mark_saved();
        assert!(!h.is_modified());
        h.undo(&mut s);
        assert!(h.is_modified(), "undoing past the save changes the file");
        h.record(&mut s, "Repeat", bars(), |s| {
            s.master_bars[0].repeat_start = true;
        });
        assert_eq!(h.redo_label(), None);
        assert!(h.is_modified());
        assert_eq!(s.master_bars[0].time_signature, TimeSignature::COMMON);
    }

    #[test]
    fn a_time_signature_touches_every_staff_in_its_bars() {
        let mut s = score(2, 4);
        let mut h = History::new();
        let t = h.record(
            &mut s,
            "Time Signature",
            vec![Region::MasterBars(1..3)],
            |_| (),
        );
        assert_eq!(t.bars.len(), 4, "two tracks, two bars");
        assert!(!t.everything);
    }

    #[test]
    fn track_settings_come_back_without_touching_bars() {
        let mut s = score(1, 2);
        let original = s.clone();
        let mut h = History::new();
        h.record(&mut s, "Track Properties", vec![Region::Track(0)], |s| {
            s.tracks[0].name = "Lead".into();
            s.tracks[0].staves[0].tuning[5] = Pitch(38);
            s.tracks[0].staves[0].capo = 2;
        });
        // A later command changes a bar; undoing the first keeps it.
        h.record(
            &mut s,
            "Rest",
            vec![Region::Bars {
                track: 0,
                staff: 0,
                bars: 0..1,
            }],
            |s| {
                s.tracks[0].staves[0].bars[0].voices[0].beats[0]
                    .duration
                    .value = NoteValue::Half;
            },
        );
        let after_both = s.clone();
        h.undo(&mut s);
        h.undo(&mut s);
        assert_eq!(s, original);
        h.redo(&mut s);
        h.redo(&mut s);
        assert_eq!(s, after_both);
    }

    #[test]
    fn whole_score_changes() {
        let mut s = score(1, 2);
        let original = s.clone();
        let mut h = History::new();
        let t = h.record(&mut s, "Insert Bar", vec![Region::Whole], |s| {
            s.master_bars.push(MasterBar::default());
            s.tracks[0].staves[0].bars.push(Bar::default());
        });
        assert!(t.everything);
        assert_eq!(s.bar_count(), 3);
        h.undo(&mut s);
        assert_eq!(s, original);
        let mut info = s.clone();
        h.record(&mut info, "Score Information", vec![Region::Info], |s| {
            s.info.title = "New".into();
            s.initial_tempo = 90.0;
        });
        h.undo(&mut info);
        assert_eq!(info, original);
    }
}
