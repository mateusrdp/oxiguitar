// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track list's state: which tracks are shown, and in what notation.
//!
//! Each track has two notation buttons, standard notation (a G clef) and
//! tablature (TAB). Any combination is allowed, and both off hides the
//! track (Mateus, 16 Oct 2026); a hidden track comes back, in the notation
//! of the button clicked, when either button is clicked. There is no
//! separate show tick: the buttons do that job, so the two can't disagree
//! (Mateus, 17 Oct 2026). Clicking a name shows that track alone; clicking
//! a button on another track adds it, so several tracks can be shown at
//! once without a separate "Multitrack" switch (Mateus, 17 Oct 2026). The
//! last shown track cannot be hidden.

use std::collections::BTreeMap;

use oxiguitar_layout::Notation;

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

    /// Show track `i` alone.
    pub(crate) fn solo(&mut self, i: usize) {
        if i < self.len() {
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

#[cfg(test)]
mod tests {
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
    fn drums_have_no_tab() {
        let mut t = guitar_piano_drums();
        t.solo(2);
        t.toggle_tablature(2);
        assert!(!t.row(2).tablature);
        assert!(t.row(2).standard);
    }

    #[test]
    fn several_tracks_shown_via_their_own_buttons() {
        let mut t = guitar_piano_drums();
        assert_eq!(t.shown(), vec![0]);
        t.solo(1);
        assert_eq!(t.shown(), vec![1]);
        // A button on another track adds it: no separate multitrack switch.
        t.toggle_standard(2);
        assert_eq!(t.shown(), vec![1, 2]);
        assert_eq!(t.notation().len(), 3);
    }
}
