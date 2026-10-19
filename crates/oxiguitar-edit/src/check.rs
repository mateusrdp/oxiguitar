// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Bars that don't add up: the bars whose beats are longer or shorter than
//! their time signature (EDIT-9).
//!
//! Such bars are allowed, shown and played; the window only marks them
//! (Mateus, 19 Oct 2026). Every bar is checked once, when a file opens;
//! after that only the bars an edit touched are checked again ("I want
//! this app to be FAST").
//!
//! A voice counts only if it holds a note or a rest: Guitar Pro 3 to 5
//! store an unused voice as one empty beat, and Guitar Pro 7 fills unused
//! voices with empty beats, which take no part in the music. A voice with
//! a beat whose duration can't be measured doesn't add up.

use std::collections::BTreeSet;

use oxiguitar_model::{BeatKind, Score};

use crate::{StaffBar, Touched};

/// Whether every voice of `at` that holds music fills its bar exactly.
/// A bar that isn't in the score, or whose time signature can't give a
/// length, counts as adding up: there is nothing to judge it by.
pub fn adds_up(score: &Score, at: StaffBar) -> bool {
    let Some(want) = score
        .master_bars
        .get(at.bar)
        .and_then(|mb| mb.expected_length())
    else {
        return true;
    };
    let Some(bar) = score
        .tracks
        .get(at.track)
        .and_then(|t| t.staves.get(at.staff))
        .and_then(|s| s.bars.get(at.bar))
    else {
        return true;
    };
    bar.voices.iter().all(|v| {
        let music = v
            .beats
            .iter()
            .any(|b| matches!(b.kind, BeatKind::Notes | BeatKind::Rest));
        !music || v.length() == Some(want)
    })
}

/// The bars of a score that don't add up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarCheck {
    bad: BTreeSet<StaffBar>,
}

impl BarCheck {
    /// Checks every bar of every staff: once, when a file opens.
    pub fn of(score: &Score) -> BarCheck {
        let mut bad = BTreeSet::new();
        for (track, t) in score.tracks.iter().enumerate() {
            for (staff, s) in t.staves.iter().enumerate() {
                for bar in 0..s.bars.len() {
                    let at = StaffBar { track, staff, bar };
                    if !adds_up(score, at) {
                        bad.insert(at);
                    }
                }
            }
        }
        BarCheck { bad }
    }

    /// Checks again what an edit touched, and nothing else. Returns whether
    /// the set of bad bars changed.
    pub fn update(&mut self, score: &Score, touched: &Touched) -> bool {
        if touched.everything {
            let new = BarCheck::of(score);
            let changed = new != *self;
            *self = new;
            return changed;
        }
        let mut changed = false;
        for &at in &touched.bars {
            let bad = !adds_up(score, at);
            changed |= if bad {
                self.bad.insert(at)
            } else {
                self.bad.remove(&at)
            };
        }
        changed
    }

    /// How many bars don't add up.
    pub fn len(&self) -> usize {
        self.bad.len()
    }

    /// Every bar adds up.
    pub fn is_empty(&self) -> bool {
        self.bad.is_empty()
    }

    /// Whether `at` doesn't add up.
    pub fn contains(&self, at: StaffBar) -> bool {
        self.bad.contains(&at)
    }

    /// The bars that don't add up, by track, staff and bar.
    pub fn iter(&self) -> impl Iterator<Item = StaffBar> + '_ {
        self.bad.iter().copied()
    }

    /// The bars that don't add up and that `keep` accepts (the shown
    /// tracks, say), in score order: bar by bar, then track and staff.
    pub fn in_score_order(&self, keep: impl Fn(StaffBar) -> bool) -> Vec<StaffBar> {
        let mut out: Vec<StaffBar> = self.iter().filter(|&b| keep(b)).collect();
        out.sort_by_key(|b| (b.bar, b.track, b.staff));
        out
    }

    /// Tools → Next Bar That Doesn't Add Up (F4, as Guitar Pro 8's "Check
    /// bar duration"): the first bad bar `keep` accepts after `from` in
    /// score order, going round to the start.
    pub fn next_after(&self, from: StaffBar, keep: impl Fn(StaffBar) -> bool) -> Option<StaffBar> {
        let order = self.in_score_order(keep);
        let key = |b: &StaffBar| (b.bar, b.track, b.staff);
        order
            .iter()
            .find(|b| key(b) > key(&from))
            .or_else(|| order.first())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caret::tests::{bar, beat, score};
    use crate::{History, Region};
    use oxiguitar_model::{Beat, NoteValue, TimeSignature, Voice};

    fn at(track: usize, bar: usize) -> StaffBar {
        StaffBar {
            track,
            staff: 0,
            bar,
        }
    }

    #[test]
    fn short_long_and_placeholder_voices() {
        let q = NoteValue::Quarter;
        let mut s = score(1, 4);
        let bars = &mut s.tracks[0].staves[0].bars;
        bars[1] = bar(&[q, q, q]);
        bars[2] = bar(&[q, q, q, q, q]);
        // Bar 4: a full first voice, a second voice holding one empty beat
        // (Guitar Pro 3-5) and a third holding a rest that is too short.
        bars[3].voices.push(Voice {
            beats: vec![Beat::default()],
        });
        bars[3].voices[1].beats[0].kind = BeatKind::Empty;
        assert!(adds_up(&s, at(0, 3)), "a placeholder voice doesn't count");
        let mut rest = beat(q);
        rest.kind = BeatKind::Rest;
        rest.notes.clear();
        s.tracks[0].staves[0].bars[3]
            .voices
            .push(Voice { beats: vec![rest] });
        let check = BarCheck::of(&s);
        assert_eq!(
            check.iter().collect::<Vec<_>>(),
            [at(0, 1), at(0, 2), at(0, 3)]
        );
        assert!(!check.contains(at(0, 0)));
        assert_eq!(check.len(), 3);
    }

    #[test]
    fn an_edit_rechecks_only_its_bars() {
        let q = NoteValue::Quarter;
        let mut s = score(2, 3);
        s.tracks[1].staves[0].bars[2] = bar(&[q]);
        let mut check = BarCheck::of(&s);
        assert_eq!(check.len(), 1);
        let mut h = History::new();
        // Change a bar behind the check's back: an edit elsewhere must not
        // notice it, which shows the check only looks where it is told.
        s.tracks[0].staves[0].bars[2] = bar(&[q]);
        let touched = h.record(
            &mut s,
            "Delete Beat",
            vec![Region::Bars {
                track: 0,
                staff: 0,
                bars: 0..1,
            }],
            |s| {
                s.tracks[0].staves[0].bars[0].voices[0].beats.pop();
            },
        );
        assert!(check.update(&s, &touched));
        assert_eq!(check.iter().collect::<Vec<_>>(), [at(0, 0), at(1, 2)]);
        let (_, touched) = h.undo(&mut s).unwrap();
        assert!(check.update(&s, &touched));
        assert_eq!(check.iter().collect::<Vec<_>>(), [at(1, 2)]);
        // A time signature re-checks its bars on every staff.
        let touched = h.record(
            &mut s,
            "Time Signature",
            vec![Region::MasterBars(1..2)],
            |s| {
                s.master_bars[1].time_signature = TimeSignature {
                    numerator: 3,
                    denominator: 4,
                };
            },
        );
        assert!(check.update(&s, &touched));
        assert_eq!(check.len(), 3);
        assert!(!check.update(&s, &touched), "nothing new");
    }

    #[test]
    fn walking_through_the_bad_bars() {
        let q = NoteValue::Quarter;
        let mut s = score(3, 6);
        for (t, b) in [(0, 4), (2, 1), (1, 4), (1, 5)] {
            s.tracks[t].staves[0].bars[b] = bar(&[q]);
        }
        let check = BarCheck::of(&s);
        let all = |_| true;
        let order = check.in_score_order(all);
        assert_eq!(order, [at(2, 1), at(0, 4), at(1, 4), at(1, 5)]);
        assert_eq!(check.next_after(at(0, 0), all), Some(at(2, 1)));
        assert_eq!(check.next_after(at(2, 1), all), Some(at(0, 4)));
        assert_eq!(check.next_after(at(1, 5), all), Some(at(2, 1)), "round");
        // Only the shown tracks.
        let shown = |b: StaffBar| b.track != 2;
        assert_eq!(check.next_after(at(0, 0), shown), Some(at(0, 4)));
        assert_eq!(BarCheck::default().next_after(at(0, 0), all), None);
    }
}
