// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track list's state: which tracks are shown, and in what notation.
//!
//! Each track has a show tick and two notation buttons, standard notation
//! (a G clef) and tablature (TAB). Any combination is allowed; with both off
//! the track is hidden, which is the same as unticking it (Mateus,
//! 16 Oct 2026). A track remembers its last notation, so ticking it again
//! brings that back. The last shown track cannot be hidden.

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

    /// The show tick of track `i` was set to `on`.
    pub(crate) fn set_shown(&mut self, i: usize, on: bool) {
        if on {
            self.show(i);
        } else {
            self.hide(i);
        }
    }

    /// Show track `i` alone.
    pub(crate) fn solo(&mut self, i: usize) {
        if i < self.len() {
            self.shown = vec![i];
        }
    }

    /// View → Multitrack is ticked whenever more than one track is shown.
    pub(crate) fn is_multitrack(&self) -> bool {
        self.shown.len() > 1
    }

    /// View → Multitrack was clicked: from several tracks back to the first
    /// of them, from one track to all.
    pub(crate) fn toggle_multitrack(&mut self) {
        if self.is_multitrack() {
            self.shown.truncate(1);
        } else {
            self.shown = (0..self.len()).collect();
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
        t.set_shown(1, true);
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
        // Ticking it again brings back its last notation: tab only.
        t.set_shown(0, true);
        assert!(t.row(0).tablature && !t.row(0).standard);
        // A button on a hidden track shows it with that notation.
        t.set_shown(0, false);
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
        t.set_shown(0, false);
        assert!(t.row(0).shown);
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
    fn multitrack_and_solo() {
        let mut t = guitar_piano_drums();
        assert!(!t.is_multitrack());
        t.toggle_multitrack();
        assert_eq!(t.shown(), vec![0, 1, 2]);
        t.toggle_multitrack();
        assert_eq!(t.shown(), vec![0]);
        t.solo(1);
        assert_eq!(t.shown(), vec![1]);
        t.set_shown(2, true);
        assert!(t.is_multitrack());
        assert_eq!(t.notation().len(), 3);
    }
}
