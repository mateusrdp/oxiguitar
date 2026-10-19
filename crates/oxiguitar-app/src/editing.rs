// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The editor's state in the window: the caret, the selection, and where
//! to draw them. No window code, so every key and click is tested without
//! a display.
//!
//! The caret and the selection live on the window's thread with a shared,
//! read-only copy of the score and the engraver's map, so moving them never
//! waits for the worker and never redraws a tile: the window draws them as
//! boxes over the page. Keys follow Guitar Pro 8's shortcut list (checked
//! 19 Oct 2026): the arrows move by beat and string, Shift extends the
//! selection, Ctrl + Shift + Left and Right extend it by bar, Home and End
//! go to the bar's ends, Ctrl + Home and Ctrl + End to the first and last
//! bars, Tab and Shift + Tab to the next and previous staff, Ctrl + Up and
//! Ctrl + Down to the previous and next track, Ctrl + 1 to 4 pick the voice,
//! Ctrl + A selects everything.

use std::sync::Arc;

use oxiguitar_display::Rect;
use oxiguitar_edit::{Caret, Selection, StaffBar};
use oxiguitar_host::api::event::{Position, Selection as PluginSelection};
use oxiguitar_layout::{Found, Hits, Spot};
use oxiguitar_model::Score;

/// The keys the editor takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    BackTab,
    /// Ctrl + 1 to 4: the voice, from 0.
    Voice(usize),
    /// Ctrl + A.
    SelectAll,
    /// Escape: drop the selection.
    Escape,
}

impl Key {
    /// The key the window names, as `app.slint` passes it.
    pub(crate) fn from_name(name: &str) -> Option<Key> {
        Some(match name {
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "home" => Key::Home,
            "end" => Key::End,
            "tab" => Key::Tab,
            "backtab" => Key::BackTab,
            "1" => Key::Voice(0),
            "2" => Key::Voice(1),
            "3" => Key::Voice(2),
            "4" => Key::Voice(3),
            "a" | "A" => Key::SelectAll,
            "escape" => Key::Escape,
            _ => return None,
        })
    }
}

/// The caret, the selection and what they need.
#[derive(Default)]
pub(crate) struct Editing {
    score: Option<Arc<Score>>,
    hits: Arc<Hits>,
    /// The shown tracks, top to bottom.
    shown: Vec<usize>,
    caret: Option<Caret>,
    /// Where Shift was first held, while a selection grows.
    anchor: Option<Caret>,
    selection: Option<Selection>,
}

impl Editing {
    /// A score was opened: the caret goes to the start of `first`.
    pub(crate) fn opened(&mut self, score: Arc<Score>, first: usize) {
        self.caret = Caret::at_start(&score, first);
        self.anchor = None;
        self.selection = None;
        self.score = Some(score);
    }

    /// The score changed (an edit, undo, redo): the caret stays where it
    /// was, as far as the score still has that place; a selection whose
    /// bars went is dropped.
    pub(crate) fn changed(&mut self, score: Arc<Score>) {
        self.caret = self.caret.and_then(|c| c.clamp(&score));
        if let Some(sel) = self.selection {
            let (_, last_track) = sel.tracks();
            let (_, last_bar) = sel.bars();
            if last_track >= score.tracks.len() || last_bar >= score.bar_count() {
                self.selection = None;
                self.anchor = None;
            }
        }
        self.score = Some(score);
    }

    /// A new engraving: its map, and so the tracks shown. A caret on a
    /// track no longer shown goes to the first shown track, same bar.
    pub(crate) fn engraved(&mut self, hits: Arc<Hits>) {
        self.shown = hits.tracks();
        self.hits = hits;
        let (Some(score), Some(c), Some(&first)) = (&self.score, self.caret, self.shown.first())
        else {
            return;
        };
        if !self.shown.contains(&c.track) {
            self.caret = Some(c.to_staff(score, first, 0));
            self.selection = None;
            self.anchor = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn caret(&self) -> Option<Caret> {
        self.caret
    }

    #[cfg(test)]
    pub(crate) fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// The shown tracks, top to bottom.
    #[cfg(test)]
    pub(crate) fn shown(&self) -> &[usize] {
        &self.shown
    }

    /// A key, with Shift and Ctrl as held. Returns whether it was the
    /// editor's (the caret or the selection changed, or would have).
    pub(crate) fn key(&mut self, key: Key, shift: bool, ctrl: bool) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let s = &*score;
        let shown = self.shown.clone();
        let moved = match (key, ctrl) {
            (Key::Left, false) => c.prev_beat(s),
            (Key::Right, false) => c.next_beat(s),
            // Ctrl + Shift + arrows extend by bar; Ctrl alone with Left and
            // Right is the player's (Guitar Pro 8: rewind, fast forward).
            (Key::Left, true) if shift => c.prev_beat(s).bar_start(),
            (Key::Right, true) if shift => {
                if c.bar + 1 < s.bar_count() {
                    c.to_bar(s, c.bar + 1).bar_end(s)
                } else {
                    c.bar_end(s)
                }
            }
            (Key::Left | Key::Right, true) => return false,
            (Key::Up, false) if shift => c,
            (Key::Down, false) if shift => c,
            (Key::Up, false) => c.string_up(),
            (Key::Down, false) => c.string_down(s),
            (Key::Up, true) => c.prev_track(s, &shown),
            (Key::Down, true) => c.next_track(s, &shown),
            (Key::Home, false) => c.bar_start(),
            (Key::End, false) => c.bar_end(s),
            (Key::Home, true) => c.first_bar(s),
            (Key::End, true) => c.last_bar(s).bar_end(s),
            (Key::Tab, _) => c.next_staff(s, &shown),
            (Key::BackTab, _) => c.prev_staff(s, &shown),
            (Key::Voice(v), true) => c.with_voice(s, v),
            (Key::Voice(_), false) => return false,
            (Key::SelectAll, true) => {
                self.selection = Selection::all(s);
                self.anchor = None;
                return true;
            }
            (Key::SelectAll, false) => return false,
            (Key::Escape, _) => {
                let had = self.selection.is_some();
                self.selection = None;
                self.anchor = None;
                return had;
            }
        };
        // Ctrl + End goes to the last bar's start in Guitar Pro; its last
        // beat is where a selection to the end must reach. Without Shift,
        // the start of the bar.
        let moved = if key == Key::End && ctrl && !shift {
            moved.bar_start()
        } else {
            moved
        };
        let extends = shift && !matches!(key, Key::Tab | Key::BackTab | Key::Voice(_));
        if extends {
            let anchor = *self.anchor.get_or_insert(c);
            self.selection = Some(Selection::between(anchor, moved));
        } else {
            self.anchor = None;
            self.selection = None;
        }
        self.caret = Some(moved);
        true
    }

    /// A press on the page at `spot` (from [`Hits::hit`]), with Shift as
    /// held: the caret goes there, in its voice, onto the string pressed
    /// (if on the tablature); with Shift, the selection grows to it.
    pub(crate) fn press(&mut self, spot: Spot, shift: bool) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let to = Caret {
            track: spot.track,
            staff: spot.staff,
            bar: spot.bar,
            beat: spot.beat.unwrap_or(0),
            string: spot.string.unwrap_or(c.string),
            ..c
        };
        let Some(to) = to.clamp(&score) else {
            return false;
        };
        if shift {
            let anchor = *self.anchor.get_or_insert(c);
            self.selection = Some(Selection::between(anchor, to));
        } else {
            self.anchor = None;
            self.selection = None;
        }
        self.caret = Some(to);
        true
    }

    /// A press at (`x`, `y`) on sheet `sheet`, in the sheet's points: the
    /// place there in the caret's voice, as [`Editing::press`] takes it.
    pub(crate) fn press_at(&mut self, sheet: usize, x: f32, y: f32, shift: bool) -> bool {
        let voice = self.caret.map_or(0, |c| c.voice);
        match self.hits.hit(sheet, x, y, voice) {
            Some(spot) => self.press(spot, shift),
            None => false,
        }
    }

    /// A click on a track's row: the track is selected, and the caret goes
    /// to it (same bar) when it is shown.
    pub(crate) fn select_track(&mut self, track: usize) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        if c.track == track || track >= score.tracks.len() {
            return false;
        }
        self.caret = Some(c.to_staff(&score, track, 0));
        self.selection = None;
        self.anchor = None;
        true
    }

    /// The track selected in the track list: the caret's.
    pub(crate) fn track(&self) -> Option<usize> {
        self.caret.map(|c| c.track)
    }

    /// Puts the caret on the first beat of `at`, in voice 1, and drops the
    /// selection (Tools → Next Bar That Doesn't Add Up).
    pub(crate) fn go_to(&mut self, at: StaffBar) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let to = Caret {
            track: at.track,
            staff: at.staff,
            bar: at.bar,
            voice: 0,
            beat: 0,
            ..c
        }
        .clamp(&score);
        self.caret = to.or(self.caret);
        self.selection = None;
        self.anchor = None;
        true
    }

    /// Tools → Next Bar That Doesn't Add Up (F4): of `bad`, the first bar
    /// of a shown track after the caret's, in score order (bar by bar,
    /// then top to bottom), going round to the start.
    pub(crate) fn next_bad(&self, bad: &[StaffBar]) -> Option<StaffBar> {
        let from = self.caret?.staff_bar();
        let key = |b: &StaffBar| (b.bar, b.track, b.staff);
        let mut shown: Vec<StaffBar> = bad
            .iter()
            .copied()
            .filter(|b| self.shown.contains(&b.track))
            .collect();
        shown.sort_by_key(key);
        shown
            .iter()
            .find(|b| key(b) > key(&from))
            .or_else(|| shown.first())
            .copied()
    }

    /// Whether any of `bad` is on a shown track.
    pub(crate) fn any_shown(&self, bad: &[StaffBar]) -> bool {
        bad.iter().any(|b| self.shown.contains(&b.track))
    }

    /// Where to draw the caret, from the engraver's map.
    pub(crate) fn caret_box(&self) -> Option<Found> {
        let c = self.caret?;
        self.hits
            .find(c.track, c.staff, c.bar, c.voice, c.beat, c.string)
    }

    /// Where to draw the selection: a box per bar of each staff, with its
    /// sheet.
    pub(crate) fn selection_boxes(&self) -> Vec<(usize, Rect)> {
        let (Some(sel), Some(score)) = (self.selection, &self.score) else {
            return Vec::new();
        };
        sel.spans(score)
            .into_iter()
            .filter_map(|sp| {
                self.hits
                    .span(sp.at.track, sp.at.staff, sp.at.bar, sp.beats)
            })
            .collect()
    }

    /// The selection as plugins see it (interface 1.1): one track, beats
    /// counted in the bar's first voice. The caret alone is a selection of
    /// one beat; a selection across tracks is given for its first track.
    pub(crate) fn for_plugins(&self) -> Option<PluginSelection> {
        let score = self.score.as_ref()?;
        let c = self.caret?;
        let pos = |bar: usize, beat: usize| Position {
            bar: u32::try_from(bar).unwrap_or(u32::MAX),
            beat: u32::try_from(beat).unwrap_or(u32::MAX),
        };
        let first_voice = |track: usize, staff: usize, voice: usize, bar: usize, beat: usize| {
            Caret {
                track,
                staff,
                voice,
                bar,
                beat,
                string: 1,
            }
            .beat_in_first_voice(score)
        };
        let (track, from, to) = match self.selection {
            None => {
                let b = c.beat_in_first_voice(score);
                (c.track, pos(c.bar, b), pos(c.bar, b))
            }
            Some(Selection::Beats {
                track,
                staff,
                voice,
                from,
                to,
            }) => (
                track,
                pos(from.0, first_voice(track, staff, voice, from.0, from.1)),
                pos(to.0, first_voice(track, staff, voice, to.0, to.1)),
            ),
            Some(Selection::Bars { tracks, bars }) => {
                let last = score
                    .tracks
                    .get(tracks.0)
                    .and_then(|t| t.staves.first())
                    .and_then(|s| s.bars.get(bars.1))
                    .and_then(|b| b.voices.first())
                    .map_or(0, |v| v.beats.len().saturating_sub(1));
                (tracks.0, pos(bars.0, 0), pos(bars.1, last))
            }
        };
        Some(PluginSelection {
            track: u32::try_from(track).unwrap_or(u32::MAX),
            from,
            to,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_layout::{BarHits, BeatHit, StaffHits, SystemHits};
    use oxiguitar_model::{Bar, Beat, Duration, MasterBar, Note, NoteValue, Staff, Track, Voice};

    fn score(tracks: usize, bars: usize) -> Arc<Score> {
        let beat = Beat {
            duration: Duration::new(NoteValue::Quarter),
            notes: vec![Note::fretted(1, 0)],
            ..Beat::default()
        };
        let bar = Bar {
            voices: vec![Voice {
                beats: vec![beat; 4],
            }],
            ..Bar::default()
        };
        Arc::new(Score {
            master_bars: vec![MasterBar::default(); bars],
            tracks: (0..tracks)
                .map(|_| Track {
                    staves: vec![Staff {
                        tuning: Staff::standard_guitar_tuning(),
                        bars: vec![bar.clone(); bars],
                        ..Staff::default()
                    }],
                    ..Track::default()
                })
                .collect(),
            ..Score::new()
        })
    }

    /// One system, the given tracks, every bar 100 points wide.
    fn hits(tracks: &[usize], bars: usize) -> Arc<Hits> {
        let staves = tracks
            .iter()
            .enumerate()
            .map(|(k, &track)| {
                let top = 100.0 + 120.0 * k as f32;
                StaffHits {
                    track,
                    staff: 0,
                    standard: None,
                    tablature: Some((top, 6.0, 6)),
                    top,
                    bottom: top + 30.0,
                    bars: (0..bars)
                        .map(|bar| {
                            let start = 100.0 * bar as f32;
                            BarHits {
                                bar,
                                start,
                                notes: start + 5.0,
                                end: start + 100.0,
                                beats: (0..4)
                                    .map(|beat| BeatHit {
                                        voice: 0,
                                        beat,
                                        x: start + 10.0 + 25.0 * beat as f32,
                                        width: 25.0,
                                    })
                                    .collect(),
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        Arc::new(Hits {
            systems: vec![SystemHits {
                sheet: 0,
                top: 90.0,
                bottom: 600.0,
                staves,
            }],
        })
    }

    fn editing(tracks: usize, shown: &[usize]) -> Editing {
        let mut e = Editing::default();
        e.opened(score(tracks, 3), shown[0]);
        e.engraved(hits(shown, 3));
        e
    }

    #[test]
    fn keys_move_the_caret_as_guitar_pro_does() {
        let mut e = editing(3, &[0, 2]);
        assert!(e.key(Key::Right, false, false));
        assert_eq!(e.caret().unwrap().beat, 1);
        e.key(Key::Down, false, false);
        e.key(Key::Down, false, false);
        assert_eq!(e.caret().unwrap().string, 3);
        e.key(Key::Down, false, true);
        assert_eq!(e.caret().unwrap().track, 2, "the next shown track");
        e.key(Key::End, false, true);
        assert_eq!((e.caret().unwrap().bar, e.caret().unwrap().beat), (2, 0));
        e.key(Key::End, false, false);
        assert_eq!(e.caret().unwrap().beat, 3);
        e.key(Key::Home, false, true);
        assert_eq!(e.caret().unwrap().bar, 0);
        assert!(e.key(Key::Voice(1), false, true));
        assert_eq!(e.caret().unwrap().voice, 1);
        assert!(!e.key(Key::Voice(1), false, false), "1 alone is not ours");
        assert!(
            !e.key(Key::Right, false, true),
            "Ctrl + Right is the player's"
        );
        let b = e.caret_box().unwrap();
        assert_eq!(b.sheet, 0);
        assert!(b.string.is_some());
    }

    #[test]
    fn shift_selects_and_a_plain_move_drops_it() {
        let mut e = editing(2, &[0, 1]);
        e.key(Key::Right, true, false);
        e.key(Key::Right, true, false);
        assert_eq!(
            e.selection(),
            Some(Selection::Beats {
                track: 0,
                staff: 0,
                voice: 0,
                from: (0, 0),
                to: (0, 2)
            })
        );
        assert_eq!(e.selection_boxes().len(), 1);
        e.key(Key::Right, true, true);
        assert_eq!(
            e.selection().unwrap().bars(),
            (0, 1),
            "to the next bar's end"
        );
        let sel = e.for_plugins().unwrap();
        assert_eq!(
            (sel.from.bar, sel.from.beat, sel.to.bar, sel.to.beat),
            (0, 0, 1, 3)
        );
        e.key(Key::Left, false, false);
        assert_eq!(e.selection(), None);
        assert!(e.key(Key::SelectAll, false, true));
        assert_eq!(e.selection_boxes().len(), 6, "two tracks, three bars");
        assert!(e.key(Key::Escape, false, false));
        assert!(!e.key(Key::Escape, false, false), "nothing left to drop");
    }

    #[test]
    fn a_press_puts_the_caret_on_the_string_and_shift_press_selects() {
        let mut e = editing(2, &[0, 1]);
        let spot = e.hits.hit(0, 136.0, 112.0, 0).unwrap();
        assert!(e.press(spot, false));
        let c = e.caret().unwrap();
        assert_eq!((c.track, c.bar, c.beat, c.string), (0, 1, 1, 3));
        let spot = e.hits.hit(0, 260.0, 230.0, 0).unwrap();
        e.press(spot, true);
        assert!(matches!(e.selection(), Some(Selection::Bars { .. })));
    }

    #[test]
    fn tracks_hidden_and_selected() {
        let mut e = editing(3, &[0, 1, 2]);
        assert!(e.select_track(2));
        assert_eq!(e.track(), Some(2));
        assert!(!e.select_track(2), "already");
        // Track 3 is hidden: the caret goes to the first shown.
        e.engraved(hits(&[1], 3));
        assert_eq!(e.track(), Some(1));
        assert_eq!(e.shown(), [1]);
        assert!(e.go_to(StaffBar {
            track: 1,
            staff: 0,
            bar: 2
        }));
        assert_eq!(e.caret().unwrap().bar, 2);
        // The score shrank under the caret.
        e.changed(score(1, 1));
        let c = e.caret().unwrap();
        assert_eq!((c.track, c.bar), (0, 0));
    }

    #[test]
    fn f4_walks_the_shown_bad_bars() {
        let mut e = editing(3, &[0, 2]);
        let at = |track, bar| StaffBar {
            track,
            staff: 0,
            bar,
        };
        let bad = [at(1, 0), at(2, 2), at(0, 2), at(2, 1)];
        assert!(e.any_shown(&bad));
        assert_eq!(
            e.next_bad(&bad),
            Some(at(2, 1)),
            "track 1 is hidden, so its bar 1 is skipped"
        );
        e.go_to(at(2, 1));
        assert_eq!(e.next_bad(&bad), Some(at(0, 2)));
        e.go_to(at(2, 2));
        assert_eq!(e.next_bad(&bad), Some(at(2, 1)), "round to the start");
        assert!(!e.any_shown(&[at(1, 0)]));
        assert_eq!(e.next_bad(&[]), None);
    }

    #[test]
    fn key_names() {
        assert_eq!(Key::from_name("right"), Some(Key::Right));
        assert_eq!(Key::from_name("3"), Some(Key::Voice(2)));
        assert_eq!(Key::from_name("x"), None);
    }
}
