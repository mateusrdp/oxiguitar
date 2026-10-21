// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track list's state: which tracks are shown, and in what notation.
//!
//! Each track has two notation buttons, standard notation (a G clef) and
//! tablature (TAB). Any combination is allowed, and both off hides the
//! track (Mateus, 16 Oct 2026); a hidden track comes back, in the notation
//! of the button clicked, when either button is clicked. There is no
//! separate show tick: the buttons do that job, so the two can't disagree
//! (Mateus, 17 Oct 2026). A button on another track adds it, so several
//! tracks can be shown at once without a separate "Multitrack" switch
//! (Mateus, 17 Oct 2026). A click on a track's name selects it for the
//! editor and changes nothing here (Mateus, 19 Oct 2026). The last shown
//! track cannot be hidden.

use std::collections::BTreeMap;

use oxiguitar_layout::Notation;
use oxiguitar_model::Mix;

/// One row as the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) shown: bool,
    pub(crate) standard: bool,
    pub(crate) tablature: bool,
}

/// Which tracks are shown and how.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tracks {
    /// Shown tracks, in score order. Never empty while there are tracks.
    shown: Vec<usize>,
    /// Each track's notation when shown (never `Notation::NONE`).
    notation: Vec<Notation>,
    /// Whether each track has strings, so tablature can be shown.
    can_tab: Vec<bool>,
}

impl Tracks {
    /// A newly opened score: its first track shown, every track in its
    /// default notation.
    pub(crate) fn new(defaults: Vec<Notation>, can_tab: Vec<bool>) -> Tracks {
        let notation = defaults
            .into_iter()
            .map(|n| {
                if n.any() {
                    n
                } else {
                    Notation {
                        standard: true,
                        tablature: false,
                    }
                }
            })
            .collect::<Vec<_>>();
        let shown = if notation.is_empty() {
            Vec::new()
        } else {
            vec![0]
        };
        Tracks {
            shown,
            notation,
            can_tab,
        }
    }

    /// The track list after tracks were added, deleted, moved or changed
    /// (M3 slice 5). `was[i]` is where track `i` was before, if it was
    /// there: shown tracks stay shown wherever they went, with `caret`
    /// (the caret's track) shown too when it is new; a track keeps its
    /// notation unless it gained or lost strings. A caret left on a hidden
    /// track moves to a shown one ([`Tracks::nearest_shown`]).
    pub(crate) fn refit(
        &self,
        defaults: Vec<Notation>,
        can_tab: Vec<bool>,
        was: &[Option<usize>],
        caret: usize,
    ) -> Tracks {
        let mut fresh = Tracks::new(defaults, can_tab);
        let before = |i: usize| was.get(i).copied().flatten();
        for (i, n) in fresh.notation.iter_mut().enumerate() {
            if let Some(o) = before(i)
                && self.can_tab.get(o) == fresh.can_tab.get(i)
                && let Some(old) = self.notation.get(o)
            {
                *n = *old;
            }
        }
        let len = fresh.notation.len();
        let mut shown: Vec<usize> = (0..len)
            .filter(|i| before(*i).is_some_and(|o| self.shown.contains(&o)))
            .collect();
        if caret < len && before(caret).is_none() {
            shown.push(caret);
            shown.sort_unstable();
        }
        if !shown.is_empty() {
            fresh.shown = shown;
        }
        fresh
    }

    /// The shown track nearest `track`, the earlier one on a tie.
    pub(crate) fn nearest_shown(&self, track: usize) -> Option<usize> {
        self.shown
            .iter()
            .copied()
            .min_by_key(|i| (i.abs_diff(track), *i))
    }

    pub(crate) fn len(&self) -> usize {
        self.notation.len()
    }

    /// The shown tracks, for the engraver.
    pub(crate) fn shown(&self) -> Vec<usize> {
        self.shown.clone()
    }

    /// Every track's notation, for the engraver.
    pub(crate) fn notation(&self) -> BTreeMap<usize, Notation> {
        self.notation.iter().copied().enumerate().collect()
    }

    /// What row `i` shows: a hidden track has both buttons off.
    pub(crate) fn row(&self, i: usize) -> Row {
        let shown = self.shown.contains(&i);
        let n = self.notation.get(i).copied().unwrap_or(Notation::NONE);
        Row {
            shown,
            standard: shown && n.standard,
            tablature: shown && n.tablature,
        }
    }

    /// The standard-notation button of track `i` was clicked.
    pub(crate) fn toggle_standard(&mut self, i: usize) {
        let r = self.row(i);
        self.apply(
            i,
            Notation {
                standard: !r.standard,
                tablature: r.tablature,
            },
        );
    }

    /// The tablature button of track `i` was clicked. Tracks without
    /// strings have no tablature.
    pub(crate) fn toggle_tablature(&mut self, i: usize) {
        if !self.can_tab.get(i).copied().unwrap_or(false) {
            return;
        }
        let r = self.row(i);
        self.apply(
            i,
            Notation {
                standard: r.standard,
                tablature: !r.tablature,
            },
        );
    }

    /// Shows track `i` in place of the only one shown: Ctrl + Up and
    /// Ctrl + Down with a single track on the page, as in Guitar Pro's
    /// single-track view (Mateus, 19 Oct 2026).
    pub(crate) fn switch_to(&mut self, i: usize) {
        if i < self.len() && self.shown.len() == 1 {
            self.shown = vec![i];
        }
    }

    fn apply(&mut self, i: usize, n: Notation) {
        if !n.any() {
            self.hide(i);
        } else if let Some(slot) = self.notation.get_mut(i) {
            *slot = n;
            self.show(i);
        }
    }

    fn show(&mut self, i: usize) {
        if i < self.len() && !self.shown.contains(&i) {
            self.shown.push(i);
            self.shown.sort_unstable();
        }
    }

    fn hide(&mut self, i: usize) {
        if self.shown.len() > 1 {
            self.shown.retain(|&t| t != i);
        }
    }
}

/// The track's overview for its row (Mateus, 19 Oct 2026): how much it
/// plays across the song, like a waveform, as SVG path data in a box as
/// wide as the song has bars and 2 high, mirrored around y = 1. Each bar's
/// activity (0 to 1) sets the height at its middle; silence is a thin line.
pub(crate) fn waveform(activity: &[f32]) -> String {
    use std::fmt::Write;
    // Half the line's thickness where the track is silent.
    const FLOOR: f32 = 0.04;
    let n = activity.len();
    if n == 0 {
        return String::new();
    }
    let h = |i: usize| FLOOR.max(activity.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 0.95);
    let mut top: Vec<(f32, f32)> = Vec::with_capacity(n + 2);
    top.push((0.0, h(0)));
    top.extend((0..n).map(|i| (i as f32 + 0.5, h(i))));
    top.push((n as f32, h(n - 1)));
    let mut d = String::with_capacity(top.len() * 24);
    for (k, (x, y)) in top.iter().enumerate() {
        let _ = write!(
            d,
            "{}{x:.2} {:.3} ",
            if k == 0 { "M" } else { "L" },
            1.0 - y
        );
    }
    for (x, y) in top.iter().rev() {
        let _ = write!(d, "L{x:.2} {:.3} ", 1.0 + y);
    }
    d.push('Z');
    d
}

/// Each track's mixer: volume, pan, mute and solo, as the file sets them
/// until the user changes them (Mateus, 19 Oct 2026: the mixer sits in the
/// track rows). Nothing sounds yet: the controls are greyed out until an
/// output plugin is installed and reach the player in M4. The file on disk
/// is never changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Mixer {
    mix: Vec<Mix>,
}

impl Mixer {
    pub(crate) fn new(mix: Vec<Mix>) -> Mixer {
        Mixer { mix }
    }

    pub(crate) fn get(&self, i: usize) -> Mix {
        self.mix.get(i).copied().unwrap_or_default()
    }

    fn change(&mut self, i: usize, f: impl FnOnce(&mut Mix)) -> Mix {
        if let Some(m) = self.mix.get_mut(i) {
            f(m);
        }
        self.get(i)
    }

    /// Volume, 0 to 1.
    pub(crate) fn set_volume(&mut self, i: usize, v: f32) -> Mix {
        self.change(i, |m| m.volume = v.clamp(0.0, 1.0))
    }

    /// Pan, -1 (left) to 1 (right).
    pub(crate) fn set_pan(&mut self, i: usize, v: f32) -> Mix {
        self.change(i, |m| m.pan = v.clamp(-1.0, 1.0))
    }

    pub(crate) fn toggle_mute(&mut self, i: usize) -> Mix {
        self.change(i, |m| m.mute = !m.mute)
    }

    pub(crate) fn toggle_solo(&mut self, i: usize) -> Mix {
        self.change(i, |m| m.solo = !m.solo)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_changed_list_keeps_what_it_can() {
        let both = Notation {
            standard: true,
            tablature: true,
        };
        let mut t = Tracks::new(vec![both, both, both], vec![true; 3]);
        t.toggle_tablature(0);
        // A track added after the first, with the caret on it.
        let added = t.refit(
            vec![both; 4],
            vec![true; 4],
            &[Some(0), None, Some(1), Some(2)],
            1,
        );
        assert_eq!(added.shown(), [0, 1]);
        assert_eq!(added.row(0).tablature, t.row(0).tablature, "kept");
        // The first deleted: the added one, shown, moves up to 0, and the
        // hidden ones stay hidden though they moved up too.
        let deleted = added.refit(
            vec![both; 3],
            vec![true; 3],
            &[Some(1), Some(2), Some(3)],
            0,
        );
        assert_eq!(deleted.shown(), [0]);
        // A caret left on a hidden track is not a reason to show it.
        let gone = added.refit(
            vec![both; 3],
            vec![true; 3],
            &[Some(0), Some(2), Some(3)],
            2,
        );
        assert_eq!(gone.shown(), [0]);
        assert_eq!(gone.nearest_shown(2), Some(0));
        // A shown track moved down goes on being shown where it went.
        let mut two = Tracks::new(vec![both; 3], vec![true; 3]);
        two.switch_to(2);
        let moved = two.refit(
            vec![both; 3],
            vec![true; 3],
            &[Some(0), Some(2), Some(1)],
            1,
        );
        assert_eq!(moved.shown(), [1]);
        // Strings gone: the default notation.
        let piano = t.refit(
            vec![
                Notation {
                    standard: true,
                    tablature: false,
                },
                both,
                both,
            ],
            vec![false, true, true],
            &[Some(0), Some(1), Some(2)],
            0,
        );
        assert!(!piano.row(0).tablature);
    }

    use super::*;

    const STD: Notation = Notation {
        standard: true,
        tablature: false,
    };
    const BOTH: Notation = Notation {
        standard: true,
        tablature: true,
    };

    fn guitar_piano_drums() -> Tracks {
        Tracks::new(vec![BOTH, STD, STD], vec![true, true, false])
    }

    #[test]
    fn any_combination_and_none_hides() {
        let mut t = guitar_piano_drums();
        // The piano is hidden: its clef button shows it.
        t.toggle_standard(1);
        assert_eq!(t.shown(), vec![0, 1]);
        // Guitar: tab only, then nothing (hidden), then back.
        t.toggle_standard(0);
        assert_eq!(
            t.row(0),
            Row {
                shown: true,
                standard: false,
                tablature: true
            }
        );
        t.toggle_tablature(0);
        assert_eq!(
            t.row(0),
            Row {
                shown: false,
                standard: false,
                tablature: false
            }
        );
        assert_eq!(t.shown(), vec![1]);
        // A button on a hidden track shows it with that notation alone.
        t.toggle_standard(0);
        assert_eq!(
            t.row(0),
            Row {
                shown: true,
                standard: true,
                tablature: false
            }
        );
    }

    #[test]
    fn the_last_shown_track_stays() {
        let mut t = guitar_piano_drums();
        t.toggle_standard(0);
        t.toggle_tablature(0);
        // Standard notation off is fine; then tab off would leave nothing
        // on the only shown track, so that click is refused.
        assert!(t.row(0).shown, "both off on the only track is refused");
        assert!(t.row(0).tablature && !t.row(0).standard);
    }

    #[test]
    fn ctrl_up_and_down_switch_a_single_track() {
        let mut t = guitar_piano_drums();
        t.switch_to(2);
        assert_eq!(t.shown(), vec![2]);
        t.toggle_standard(0);
        t.switch_to(1);
        assert_eq!(t.shown(), vec![0, 2], "only with one track shown");
    }

    #[test]
    fn drums_have_no_tab() {
        let mut t = guitar_piano_drums();
        t.toggle_standard(2);
        t.toggle_tablature(2);
        assert!(!t.row(2).tablature);
        assert!(t.row(2).standard);
    }

    #[test]
    fn several_tracks_shown_via_their_own_buttons() {
        let mut t = guitar_piano_drums();
        assert_eq!(t.shown(), vec![0]);
        // A button on another track adds it: no separate multitrack switch.
        t.toggle_standard(2);
        assert_eq!(t.shown(), vec![0, 2]);
        t.toggle_standard(1);
        assert_eq!(t.shown(), vec![0, 1, 2], "in score order");
        assert_eq!(t.notation().len(), 3);
    }

    #[test]
    fn the_mixer_keeps_each_tracks_values_in_range() {
        let file = Mix {
            volume: 0.5,
            pan: -0.25,
            mute: false,
            solo: true,
        };
        let mut m = Mixer::new(vec![file, Mix::default()]);
        assert_eq!(m.get(0), file);
        assert_eq!(m.set_volume(0, 1.7).volume, 1.0);
        assert_eq!(m.set_pan(1, -3.0).pan, -1.0);
        assert!(m.toggle_mute(1).mute);
        assert!(!m.toggle_solo(0).solo);
        // A track that doesn't exist changes nothing.
        assert_eq!(m.toggle_mute(9), Mix::default());
        assert_eq!(m.get(1).volume, Mix::default().volume);
    }

    #[test]
    fn the_waveform_spans_the_song_mirrored() {
        assert_eq!(waveform(&[]), "");
        let d = waveform(&[0.0, 1.0]);
        assert!(
            d.starts_with("M0.00 0.960 L0.50 0.960 L1.50 0.050 L2.00 0.050 "),
            "{d}"
        );
        assert!(d.contains("L1.50 1.950 L0.50 1.040 L0.00 1.040 Z"), "{d}");
        // Out-of-range values are clamped into the box.
        assert!(!waveform(&[7.0, -1.0]).contains('-'));
    }
}
