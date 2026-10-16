// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The horizontal grid of one bar, shared by every staff shown: each moment
//! where any staff has a beat gets one x position, so tracks and the two
//! notations of a track line up (VIEW-1, VIEW-4). Spacing is proportional to
//! duration and widened where symbols would touch (NOT-4).

use oxiguitar_model::Ratio;

use crate::plan::{BarPlan, duration_space};

/// One moment of the bar.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GridCol {
    pub(crate) onset: Ratio,
    /// Room needed left and right of the note centre, on any staff.
    pub(crate) left: f32,
    pub(crate) right: f32,
    /// Half the widest tab number.
    pub(crate) tab_half: f32,
    /// Width of the widest chord name here, 0 if none.
    pub(crate) chord_w: f32,
    /// What the duration asks for, to the next moment.
    pub(crate) space: f32,
}

/// One bar's moments and the natural distances between them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BarGrid {
    pub(crate) cols: Vec<GridCol>,
    /// From the start of the bar's notes to the first column.
    pub(crate) lead: f32,
    /// From each column to the next, and from the last to the barline.
    pub(crate) gaps: Vec<f32>,
}

impl BarGrid {
    /// Merges the columns of every staff's plan of the same bar.
    pub(crate) fn merge(plans: &[&BarPlan], sp: f32) -> BarGrid {
        let length = plans.iter().map(|p| p.length).max().unwrap_or(Ratio::ONE);
        let mut cols: Vec<GridCol> = Vec::new();
        for plan in plans {
            for c in &plan.cols {
                match cols.binary_search_by(|g| g.onset.cmp(&c.column.onset)) {
                    Ok(i) => {
                        if let Some(g) = cols.get_mut(i) {
                            g.left = g.left.max(c.left);
                            g.right = g.right.max(c.right);
                            g.tab_half = g.tab_half.max(c.tab_half);
                            g.chord_w = g.chord_w.max(c.chord_w);
                        }
                    }
                    Err(i) => cols.insert(
                        i,
                        GridCol {
                            onset: c.column.onset,
                            left: c.left,
                            right: c.right,
                            tab_half: c.tab_half,
                            chord_w: c.chord_w,
                            space: 0.0,
                        },
                    ),
                }
            }
        }
        if cols.is_empty() {
            cols.push(GridCol {
                onset: Ratio::ZERO,
                left: 0.0,
                right: 0.0,
                tab_half: 0.0,
                chord_w: 0.0,
                space: 0.0,
            });
        }
        let ends: Vec<Ratio> = cols
            .iter()
            .skip(1)
            .map(|c| c.onset)
            .chain(std::iter::once(length))
            .collect();
        for (c, end) in cols.iter_mut().zip(ends) {
            let span = end.checked_sub(c.onset).unwrap_or(Ratio::ZERO);
            c.space = duration_space(span.max(Ratio::ZERO)) * sp;
        }

        let mut gaps = Vec::with_capacity(cols.len());
        for (i, c) in cols.iter().enumerate() {
            let gap = match cols.get(i + 1) {
                Some(next) => c
                    .space
                    .max(c.right + next.left + 0.3 * sp)
                    .max(c.tab_half + next.tab_half + 0.5 * sp),
                None => c.space.max(c.right + sp).max(c.tab_half + 0.8 * sp),
            };
            gaps.push(gap);
        }
        make_room_for_chords(&cols, &mut gaps, sp);
        let lead = cols
            .first()
            .map_or(sp, |c| (c.left + sp).max(c.tab_half + 0.6 * sp));
        BarGrid { cols, lead, gaps }
    }

    /// Width of the notes at natural spacing.
    pub(crate) fn natural(&self) -> f32 {
        self.lead + self.gaps.iter().sum::<f32>()
    }

    /// How strongly the bar takes up extra room when a system is stretched.
    pub(crate) fn weight(&self) -> f32 {
        self.cols.iter().map(|c| c.space).sum()
    }

    /// Column positions from the start of the notes, with `extra` points
    /// shared among the gaps by their duration. A negative `extra` (a bar
    /// wider than the line) squeezes every distance by the same factor.
    /// Returns the positions and the total width.
    pub(crate) fn place(&self, extra: f32) -> (Vec<f32>, f32) {
        if extra < 0.0 {
            let natural = self.natural().max(f32::EPSILON);
            let f = ((natural + extra) / natural).max(0.05);
            let mut x = self.lead * f;
            let mut xs = Vec::with_capacity(self.cols.len());
            for gap in &self.gaps {
                xs.push(x);
                x += gap * f;
            }
            return (xs, x);
        }
        let weight = self.weight().max(f32::EPSILON);
        let mut x = self.lead;
        let mut xs = Vec::with_capacity(self.cols.len());
        for (c, gap) in self.cols.iter().zip(&self.gaps) {
            xs.push(x);
            x += gap + extra * c.space / weight;
        }
        (xs, x)
    }

    /// The column index for an onset, or the nearest earlier one.
    pub(crate) fn index_of(&self, onset: Ratio) -> usize {
        match self.cols.binary_search_by(|c| c.onset.cmp(&onset)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

/// Chord names stay over their beat; they are never pushed sideways. The
/// music opens up instead, so each name ends before the next one starts and
/// the last one before the bar line. A name starts half a notehead left of
/// its column; the extra room goes into the gap just before the next name
/// (or the bar line).
fn make_room_for_chords(cols: &[GridCol], gaps: &mut [f32], sp: f32) {
    for i in 0..cols.len() {
        let w = cols.get(i).map_or(0.0, |c| c.chord_w);
        if w <= 0.0 {
            continue;
        }
        let next = (i + 1..cols.len()).find(|&j| cols.get(j).is_some_and(|c| c.chord_w > 0.0));
        let (need, end) = match next {
            Some(j) => (w + 0.5 * sp, j),
            None => (w - 0.3 * sp, cols.len()),
        };
        let have: f32 = gaps.get(i..end).map_or(0.0, |g| g.iter().sum());
        if have < need
            && let Some(g) = end.checked_sub(1).and_then(|k| gaps.get_mut(k))
        {
            *g += need - have;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(onset: Ratio, space: f32) -> GridCol {
        GridCol {
            onset,
            left: 1.0,
            right: 1.0,
            tab_half: 0.0,
            chord_w: 0.0,
            space,
        }
    }

    #[test]
    fn chord_names_get_room_instead_of_moving() {
        let named = |w: f32| GridCol {
            chord_w: w,
            ..col(Ratio::ZERO, 10.0)
        };
        // Names 30 wide on columns 0 and 2, 20 apart: the gap before the
        // second name grows until the first fits (30 + 0.5 sp), and the
        // last gap until the second ends before the bar line (30 - 0.3 sp).
        let cols = vec![named(30.0), named(0.0), named(30.0)];
        let mut gaps = vec![10.0, 10.0, 10.0];
        make_room_for_chords(&cols, &mut gaps, 5.0);
        assert_eq!(gaps, vec![10.0, 22.5, 28.5]);
        // Without names nothing changes.
        let mut plain = vec![10.0, 10.0, 10.0];
        make_room_for_chords(&[named(0.0), named(0.0), named(0.0)], &mut plain, 5.0);
        assert_eq!(plain, vec![10.0, 10.0, 10.0]);
    }

    #[test]
    fn extra_room_follows_duration() {
        let half = Ratio::new(1, 2).unwrap_or(Ratio::ONE);
        let g = BarGrid {
            cols: vec![col(Ratio::ZERO, 10.0), col(half, 30.0)],
            lead: 5.0,
            gaps: vec![10.0, 30.0],
        };
        assert_eq!(g.natural(), 45.0);
        let (xs, w) = g.place(40.0);
        assert_eq!(xs, vec![5.0, 25.0]);
        assert_eq!(w, 85.0);
        assert_eq!(g.index_of(Ratio::new(3, 4).unwrap_or(Ratio::ONE)), 1);
        let (xs, w) = g.place(-22.5);
        assert_eq!(xs, vec![2.5, 7.5]);
        assert_eq!(w, 22.5);
    }
}
