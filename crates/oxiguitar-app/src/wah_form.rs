// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The wah-wah editor's state and rules (#250), apart from the window so
//! they are tested without one: the bend editor's grid ([`crate::bend_form`])
//! with the pedal up the side, from the heel (closed) at the bottom to the
//! toe (open) at the top, and the beat's duration across; and the filter's
//! Q beside the mark's kind.
//!
//! A wah's curve has the bend's four places: where the pedal starts, the
//! middle's two places (one position, held from the first to the second),
//! and where it ends.

use oxiguitar_model::{Ratio, Wah, WahPoint, WahShape};

use crate::bend_form::{COLUMNS, Point};

/// The grid's rows above the heel: a row is a tenth of the pedal's way.
pub(crate) const ROWS: i32 = 10;

/// The kinds offered, "No wah-wah" first.
pub(crate) const TYPES: [&str; 3] = ["No wah-wah", "Open wah-wah", "Closed wah-wah"];

/// The Qs offered, in tenths.
const QS: [u16; 9] = [10, 15, 20, 30, 40, 60, 80, 120, 160];

/// The words under the grid.
pub(crate) const HELP: &str = "Drag the points: time across the beat, the pedal from heel \
     (closed) to toe (open) up the side. Enter applies; Escape cancels.";

/// The Qs' names, the Cry Baby's own marked.
pub(crate) fn qs() -> Vec<String> {
    QS.iter()
        .map(|&q| {
            let n = if q % 10 == 0 {
                format!("Q {}", q / 10)
            } else {
                format!("Q {}.{}", q / 10, q % 10)
            };
            if q == WahShape::Q {
                format!("{n} (Cry Baby)")
            } else {
                n
            }
        })
        .collect()
}

/// The editor's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WahForm {
    /// The mark: an index into [`TYPES`].
    pub(crate) kind: usize,
    /// The Q picked: an index into the Qs offered.
    pub(crate) q: usize,
    /// Start, the middle's two places, end; empty for no mark.
    pub(crate) points: Vec<Point>,
    /// The point being dragged.
    dragging: Option<usize>,
    /// Something was changed, so OK applies it.
    pub(crate) changed: bool,
}

fn kind_of(index: usize) -> Option<Wah> {
    match index {
        1 => Some(Wah::Open),
        2 => Some(Wah::Closed),
        _ => None,
    }
}

impl WahForm {
    /// The editor opened on the caret's beat's mark (`here`, with its
    /// shape where the user gave it one). On a beat without a mark it
    /// opens on the kind `asked` for, ready to be applied.
    pub(crate) fn open(here: Option<(Wah, Option<&WahShape>)>, asked: Wah) -> WahForm {
        let (wah, shape, changed) = match here {
            Some((wah, shape)) => (wah, shape.cloned().unwrap_or(wah.shape()), false),
            None => (asked, asked.shape(), true),
        };
        WahForm {
            kind: match wah {
                Wah::Open => 1,
                Wah::Closed => 2,
            },
            q: nearest_q(shape.q),
            points: grid_points(&shape.points),
            dragging: None,
            changed,
        }
    }

    /// A kind picked: its own curve, or none for "No wah-wah". The Q
    /// stays.
    pub(crate) fn pick_kind(&mut self, kind: usize) {
        if kind >= TYPES.len() {
            return;
        }
        self.kind = kind;
        self.points = kind_of(kind)
            .map(|w| grid_points(&w.shape().points))
            .unwrap_or_default();
        self.changed = true;
    }

    /// A Q picked.
    pub(crate) fn pick_q(&mut self, q: usize) {
        if q < QS.len() && q != self.q {
            self.q = q;
            self.changed = true;
        }
    }

    /// A press on the grid at a column and row (fractions allowed): the
    /// nearest point is picked up, if any is within a cell and a half.
    pub(crate) fn press(&mut self, col: f32, row: f32) {
        let near = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let d = (p.col as f32 - col).powi(2) + (p.row as f32 - row).powi(2);
                (i, d)
            })
            // The later of two points in one place, so a held middle's
            // second place comes off its first.
            .min_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .filter(|(_, d)| *d <= 2.25);
        self.dragging = near.map(|(i, _)| i);
    }

    /// The pointer moved while a point is held: the point moves there,
    /// kept between its neighbours in time; the middle's two places share
    /// one position.
    pub(crate) fn drag(&mut self, col: f32, row: f32) {
        let Some(i) = self.dragging else {
            return;
        };
        let low = i
            .checked_sub(1)
            .and_then(|j| self.points.get(j))
            .map_or(0, |p| p.col);
        let high = self.points.get(i + 1).map_or(COLUMNS, |p| p.col);
        let to = Point {
            col: (col.round() as i32).clamp(low, high),
            row: (row.round() as i32).clamp(0, ROWS),
        };
        let Some(p) = self.points.get_mut(i) else {
            return;
        };
        if *p == to {
            return;
        }
        *p = to;
        if (i == 1 || i == 2)
            && let Some(q) = self.points.get_mut(3 - i)
        {
            q.row = to.row;
        }
        self.changed = true;
    }

    /// The pointer let go.
    pub(crate) fn release(&mut self) {
        self.dragging = None;
    }

    /// The mark the editor makes and how it is played; `None` for no
    /// mark.
    pub(crate) fn mark(&self) -> Option<(Wah, Option<WahShape>)> {
        let wah = kind_of(self.kind)?;
        let mut points: Vec<WahPoint> = Vec::new();
        for p in &self.points {
            let q = WahPoint {
                offset: Ratio::new(i64::from(p.col), i64::from(COLUMNS))?,
                position: u8::try_from(p.row.clamp(0, ROWS) * 100 / ROWS).ok()?,
            };
            if points.last() != Some(&q) {
                points.push(q);
            }
        }
        let q = QS.get(self.q).copied().unwrap_or(WahShape::Q);
        Some((wah, Some(WahShape { points, q })))
    }

    /// The curve as SVG path data in grid units (10 a cell; y down from
    /// the toe), for the window to stretch over its grid.
    pub(crate) fn path(&self) -> String {
        let mut out = String::new();
        for (i, p) in self.points.iter().enumerate() {
            let x = p.col * 10;
            let y = (ROWS - p.row) * 10;
            out.push_str(&format!("{}{x} {y} ", if i == 0 { "M" } else { "L" }));
        }
        out.trim_end().to_owned()
    }
}

/// The Q offered that is nearest to `q` tenths.
fn nearest_q(q: u16) -> usize {
    QS.iter()
        .enumerate()
        .min_by_key(|(_, offered)| offered.abs_diff(q))
        .map_or(0, |(i, _)| i)
}

/// A curve on the grid, in the editor's four places: one that doesn't
/// have that shape keeps its first point, the one furthest from it and
/// its last, as the bend editor does.
fn grid_points(points: &[WahPoint]) -> Vec<Point> {
    let at = |q: &WahPoint| Point {
        col: (q.offset.to_f64() * f64::from(COLUMNS)).round() as i32,
        row: (i32::from(q.position.min(100)) * ROWS + 50) / 100,
    };
    let four: Vec<WahPoint> = match points {
        [] => return Vec::new(),
        [a] => vec![*a; 4],
        [a, b] => vec![*a, *b, *b, *b],
        [a, b, c] => vec![*a, *b, *b, *c],
        [a, b, c, d] if b.position == c.position => vec![*a, *b, *c, *d],
        [first, middle @ .., last] => {
            let far = middle
                .iter()
                .max_by_key(|q| q.position.abs_diff(first.position))
                .copied()
                .unwrap_or(*first);
            vec![*first, far, far, *last]
        }
    };
    let mut out: Vec<Point> = four.iter().map(at).collect();
    // Snapped columns stay in order.
    for i in 1..out.len() {
        let before = out.get(i - 1).map_or(0, |p| p.col);
        if let Some(p) = out.get_mut(i) {
            p.col = p.col.clamp(before, COLUMNS);
        }
    }
    if let [_, m1, m2, _] = out.as_mut_slice() {
        m2.row = m1.row;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_mark_opens_on_the_kind_asked_for_ready_to_apply() {
        let form = WahForm::open(None, Wah::Open);
        assert_eq!(TYPES[form.kind], "Open wah-wah");
        assert!(form.changed);
        assert_eq!(qs()[form.q], "Q 4 (Cry Baby)");
        // From the heel to the toe over the beat's first half, then held.
        assert_eq!(form.path(), "M0 100 L60 0 L60 0 L120 0");
        let (wah, shape) = form.mark().unwrap();
        assert_eq!(wah, Wah::Open);
        assert_eq!(shape, Some(Wah::Open.shape()));
    }

    #[test]
    fn a_mark_s_own_shape_is_shown_and_left_alone_until_changed() {
        let shape = WahShape {
            points: vec![
                WahPoint {
                    offset: Ratio::ZERO,
                    position: 30,
                },
                WahPoint {
                    offset: Ratio::new(1, 4).unwrap(),
                    position: 100,
                },
                WahPoint {
                    offset: Ratio::ONE,
                    position: 0,
                },
            ],
            q: 80,
        };
        let mut form = WahForm::open(Some((Wah::Closed, Some(&shape))), Wah::Open);
        assert_eq!(TYPES[form.kind], "Closed wah-wah");
        assert!(!form.changed);
        assert_eq!(qs()[form.q], "Q 8");
        assert_eq!(form.mark(), Some((Wah::Closed, Some(shape))));
        // The end dragged up to half way; the start can't pass the middle.
        form.press(12.0, 0.0);
        form.drag(12.3, 5.2);
        form.release();
        form.press(0.0, 3.0);
        form.drag(7.0, 3.0);
        form.release();
        assert!(form.changed);
        let (_, made) = form.mark().unwrap();
        let made = made.unwrap();
        assert_eq!(
            made.points.first().unwrap().offset,
            Ratio::new(1, 4).unwrap()
        );
        assert_eq!(made.points.last().unwrap().position, 50);
        // The middle's two places move as one.
        form.press(3.0, 10.0);
        form.drag(3.0, 6.0);
        assert!(form.points[1].row == 6 && form.points[2].row == 6);
    }

    #[test]
    fn picking_a_kind_gives_its_curve_and_no_wah_gives_none() {
        let mut form = WahForm::open(Some((Wah::Open, None)), Wah::Open);
        form.pick_q(6);
        form.pick_kind(2);
        assert_eq!(form.path(), "M0 0 L60 100 L60 100 L120 100");
        let (wah, shape) = form.mark().unwrap();
        assert_eq!((wah, shape.unwrap().q), (Wah::Closed, 80));
        form.pick_kind(0);
        assert!(form.points.is_empty() && form.mark().is_none());
        form.pick_kind(9);
        assert_eq!(form.kind, 0);
    }
}
