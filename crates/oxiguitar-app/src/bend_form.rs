// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The bend and whammy-bar editor (M3 slice 6), without window code.
//!
//! As Guitar Pro 8's Bend window (its guide, "Bends", checked 20 Oct
//! 2026): a type, a pitch from ¼ tone to 3 tones, and a curve whose points
//! are dragged on a grid, time across the note (twelfths), pitch up the
//! side (quarter tones). The curve has the shape .gp files hold: an origin,
//! a middle held at one pitch between two places, and a destination
//! (`BendOrigin…`, `BendMiddle…`, `BendDestination…`), so what the editor
//! makes is saved as it is shown.

use oxiguitar_model::{Bend, BendPoint, Ratio};

/// Columns across the note: a point sits on a twelfth (five sixtieths, as
/// files keep them).
pub(crate) const COLUMNS: i32 = 12;
/// Cents in a row: a quarter tone.
pub(crate) const ROW_CENTS: i32 = 50;
/// Rows from the unbent pitch to the top: 3 tones.
pub(crate) const ROWS: i32 = 12;

/// The bends' types, as Guitar Pro 8 names them; the first removes it.
pub(crate) const BEND_TYPES: [&str; 7] = [
    "No bend",
    "Bend",
    "Bend and release",
    "Release",
    "Prebend",
    "Prebend and bend",
    "Prebend and release",
];

/// The whammy bar's types; the first removes it.
pub(crate) const WHAMMY_TYPES: [&str; 6] = [
    "No whammy bar",
    "Dive",
    "Dip",
    "Return",
    "Predive",
    "Predive and dive",
];

/// The pitches offered, ¼ tone to 3 tones.
pub(crate) fn amounts() -> Vec<String> {
    (1..=ROWS)
        .map(|r| {
            let whole = r / 4;
            let part = ["", "¼", "½", "¾"].get(usize::try_from(r % 4).unwrap_or(0));
            match (whole, part.copied().unwrap_or("")) {
                (1, "") => "Full".to_owned(),
                (0, p) => format!("{p} tone"),
                (w, p) => format!("{w}{p} tones"),
            }
        })
        .collect()
}

/// A point of the curve on the grid: a column (0 to 12) and a row of
/// quarter tones (negative for a whammy dip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Point {
    pub(crate) col: i32,
    pub(crate) row: i32,
}

/// The editor's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BendForm {
    /// The whammy bar's editor (a curve down and up), else a bend's (up).
    pub(crate) whammy: bool,
    /// The type picked (an index into [`BEND_TYPES`] or
    /// [`WHAMMY_TYPES`]); `None` once points were dragged (custom).
    pub(crate) kind: Option<usize>,
    /// The pitch picked, in rows (1 to 12).
    pub(crate) amount: i32,
    /// Origin, the middle's two places, destination; empty for none.
    pub(crate) points: Vec<Point>,
    /// The point being dragged.
    dragging: Option<usize>,
    /// Something was changed, so OK applies it.
    pub(crate) changed: bool,
}

impl BendForm {
    /// The editor opened on `bend` (the caret's note's, or its beat's
    /// whammy curve).
    pub(crate) fn open(bend: Option<&Bend>, whammy: bool) -> BendForm {
        let points = bend
            .map(|b| grid_points(&b.points, whammy))
            .unwrap_or_default();
        let amount = points
            .iter()
            .map(|p| p.row.abs())
            .max()
            .filter(|a| *a > 0)
            .unwrap_or(4);
        let mut form = BendForm {
            whammy,
            kind: if points.is_empty() { Some(0) } else { None },
            amount,
            points,
            dragging: None,
            changed: false,
        };
        if form.kind.is_none() {
            form.kind = form.matching_kind();
        }
        form
    }

    /// The lowest and highest rows.
    pub(crate) fn rows(&self) -> (i32, i32) {
        if self.whammy {
            (-ROWS, ROWS)
        } else {
            (0, ROWS)
        }
    }

    /// The preset of the type picked, at the pitch picked.
    fn preset(&self, kind: usize) -> Vec<Point> {
        let a = self.amount.clamp(1, ROWS);
        let half = (a / 2).max(1).min(a);
        let p = |col, row| Point { col, row };
        let rows: &[(i32, i32)] = if self.whammy {
            match kind {
                1 => &[(0, 0), (12, -a), (12, -a), (12, -a)],
                2 => &[(0, 0), (6, -a), (6, -a), (12, 0)],
                3 => &[(0, -a), (12, 0), (12, 0), (12, 0)],
                4 => &[(0, -a), (12, -a), (12, -a), (12, -a)],
                5 => &[(0, -half), (12, -a), (12, -a), (12, -a)],
                _ => &[],
            }
        } else {
            match kind {
                1 => &[(0, 0), (6, a), (12, a), (12, a)],
                2 => &[(0, 0), (3, a), (9, a), (12, 0)],
                3 => &[(0, a), (6, 0), (12, 0), (12, 0)],
                4 => &[(0, a), (12, a), (12, a), (12, a)],
                5 => &[(0, half), (6, a), (12, a), (12, a)],
                6 => &[(0, a), (6, a), (6, a), (12, 0)],
                _ => &[],
            }
        };
        rows.iter().map(|&(c, r)| p(c, r)).collect()
    }

    /// The type whose preset the points are, at the pitch picked.
    fn matching_kind(&self) -> Option<usize> {
        let n = if self.whammy {
            WHAMMY_TYPES.len()
        } else {
            BEND_TYPES.len()
        };
        (0..n).find(|k| self.preset(*k) == self.points)
    }

    /// A type picked: its preset. "Custom" (past the types) keeps the
    /// points.
    pub(crate) fn pick_kind(&mut self, kind: usize) {
        let n = if self.whammy {
            WHAMMY_TYPES.len()
        } else {
            BEND_TYPES.len()
        };
        if kind >= n {
            return;
        }
        self.kind = Some(kind);
        self.points = self.preset(kind);
        self.changed = true;
    }

    /// A pitch picked (1 to 12 rows): the type's preset again, or a custom
    /// curve scaled to it.
    pub(crate) fn pick_amount(&mut self, amount: i32) {
        let amount = amount.clamp(1, ROWS);
        let old = self.amount;
        self.amount = amount;
        match self.kind {
            Some(k) => self.points = self.preset(k),
            None => {
                let (lo, hi) = self.rows();
                for p in &mut self.points {
                    p.row = (p.row * amount / old.max(1)).clamp(lo, hi);
                }
            }
        }
        self.changed = true;
    }

    /// A press on the grid at a column and row (fractions allowed): the
    /// nearest point is picked up, if any is within a cell and a half. On
    /// an empty curve, a press starts the bend preset at that pitch.
    pub(crate) fn press(&mut self, col: f32, row: f32) {
        if self.points.is_empty() {
            let r = row.round() as i32;
            if r == 0 {
                return;
            }
            self.amount = r.abs().clamp(1, ROWS);
            self.pick_kind(1);
            return;
        }
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

    /// The pointer moved to a column and row while a point is held: the
    /// point moves there, kept between its neighbours in time; the middle's
    /// two places share one pitch.
    pub(crate) fn drag(&mut self, col: f32, row: f32) {
        let Some(i) = self.dragging else {
            return;
        };
        let (lo, hi) = self.rows();
        let low = i
            .checked_sub(1)
            .and_then(|j| self.points.get(j))
            .map_or(0, |p| p.col);
        let high = self.points.get(i + 1).map_or(COLUMNS, |p| p.col);
        let to = Point {
            col: (col.round() as i32).clamp(low, high),
            row: (row.round() as i32).clamp(lo, hi),
        };
        let Some(p) = self.points.get_mut(i) else {
            return;
        };
        if *p == to {
            return;
        }
        *p = to;
        if i == 1 || i == 2 {
            let other = 3 - i;
            if let Some(q) = self.points.get_mut(other) {
                q.row = to.row;
            }
        }
        self.changed = true;
        self.kind = self.matching_kind();
    }

    /// The pointer let go.
    pub(crate) fn release(&mut self) {
        self.dragging = None;
    }

    /// The curve the editor makes: `None` for none (or one that never
    /// leaves the unbent pitch).
    pub(crate) fn bend(&self) -> Option<Bend> {
        let mut points: Vec<BendPoint> = Vec::new();
        for p in &self.points {
            let q = BendPoint {
                offset: Ratio::new(i64::from(p.col), i64::from(COLUMNS))?,
                cents: p.row * ROW_CENTS,
            };
            if points.last() != Some(&q) {
                points.push(q);
            }
        }
        points
            .iter()
            .any(|p| p.cents != 0)
            .then_some(Bend { points })
    }

    /// The curve as SVG path data in grid units (10 a cell; y down from
    /// the top row), for the window to stretch over its grid.
    pub(crate) fn path(&self) -> String {
        let (_, hi) = self.rows();
        let mut out = String::new();
        for (i, p) in self.points.iter().enumerate() {
            let x = p.col * 10;
            let y = (hi - p.row) * 10;
            out.push_str(&format!("{}{x} {y} ", if i == 0 { "M" } else { "L" }));
        }
        out.trim_end().to_owned()
    }
}

/// A file's curve on the grid, in the editor's four places: one that
/// doesn't have that shape keeps its first, furthest and last points, as
/// the .gp writer does.
fn grid_points(points: &[BendPoint], whammy: bool) -> Vec<Point> {
    let (lo, hi) = if whammy { (-ROWS, ROWS) } else { (0, ROWS) };
    let at = |q: &BendPoint| Point {
        col: (q.offset.to_f64() * f64::from(COLUMNS)).round() as i32,
        row: ((f64::from(q.cents) / f64::from(ROW_CENTS)).round() as i32).clamp(lo, hi),
    };
    let four: Vec<BendPoint> = match points {
        [] => return Vec::new(),
        [a] => vec![*a; 4],
        [a, b] => vec![*a, *b, *b, *b],
        [a, b, c] => vec![*a, *b, *b, *c],
        [a, b, c, d] if b.cents == c.cents => vec![*a, *b, *c, *d],
        [first, middle @ .., last] => {
            let far = middle
                .iter()
                .max_by_key(|q| q.cents.abs())
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
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn cents(b: &Bend) -> Vec<(f64, i32)> {
        b.points
            .iter()
            .map(|p| (p.offset.to_f64(), p.cents))
            .collect()
    }

    #[test]
    fn presets_make_curves_files_hold() {
        let mut f = BendForm::open(None, false);
        assert_eq!(f.kind, Some(0));
        assert!(f.bend().is_none());
        f.pick_kind(1);
        assert_eq!(
            cents(&f.bend().unwrap()),
            [(0.0, 0), (0.5, 200), (1.0, 200)]
        );
        f.pick_amount(6);
        assert_eq!(
            cents(&f.bend().unwrap()),
            [(0.0, 0), (0.5, 300), (1.0, 300)]
        );
        f.pick_kind(2);
        assert_eq!(
            cents(&f.bend().unwrap()),
            [(0.0, 0), (0.25, 300), (0.75, 300), (1.0, 0)]
        );
        f.pick_kind(0);
        assert!(f.bend().is_none());
        let mut w = BendForm::open(None, true);
        w.pick_kind(2);
        assert_eq!(cents(&w.bend().unwrap()), [(0.0, 0), (0.5, -200), (1.0, 0)]);
    }

    #[test]
    fn dragging_keeps_points_in_order_and_the_middle_level() {
        let mut f = BendForm::open(None, false);
        f.pick_kind(2); // (0,0) (3,4) (9,4) (12,0)
        f.press(3.2, 3.8);
        f.drag(10.0, 6.0);
        // Not past the next point; the middle's other place follows up.
        assert_eq!(f.points[1], Point { col: 9, row: 6 });
        assert_eq!(f.points[2].row, 6);
        assert_eq!(f.kind, None);
        f.release();
        // Out of range: kept on the grid.
        f.press(12.0, 0.0);
        f.drag(14.0, -3.0);
        assert_eq!(f.points[3], Point { col: 12, row: 0 });
        // Far from every point: nothing picked up.
        f.release();
        f.press(6.0, 11.0);
        f.drag(0.0, 0.0);
        assert_eq!(f.points[1], Point { col: 9, row: 6 });
    }

    #[test]
    fn a_file_s_curve_opens_on_the_grid_and_round_trips() {
        let b = Bend {
            points: vec![
                BendPoint {
                    offset: Ratio::ZERO,
                    cents: 0,
                },
                BendPoint {
                    offset: Ratio::new(1, 2).unwrap(),
                    cents: 100,
                },
                BendPoint {
                    offset: Ratio::ONE,
                    cents: 100,
                },
            ],
        };
        let f = BendForm::open(Some(&b), false);
        assert_eq!(f.bend(), Some(b));
        assert!(!f.changed);
        // Five points (Guitar Pro 3–5): first, furthest, last.
        let five = Bend {
            points: [0, 50, 200, 100, 0]
                .iter()
                .enumerate()
                .map(|(i, c)| BendPoint {
                    offset: Ratio::new(i as i64, 4).unwrap(),
                    cents: *c,
                })
                .collect(),
        };
        let f = BendForm::open(Some(&five), false);
        assert_eq!(cents(&f.bend().unwrap()), [(0.0, 0), (0.5, 200), (1.0, 0)]);
        // An empty curve: a press starts a bend at that pitch.
        let mut f = BendForm::open(None, false);
        f.press(5.0, 2.0);
        assert_eq!(f.kind, Some(1));
        assert_eq!(f.amount, 2);
        assert_eq!(amounts()[0], "¼ tone");
        assert_eq!(amounts()[3], "Full");
        assert_eq!(amounts()[5], "1½ tones");
        assert_eq!(f.path(), "M0 120 L60 100 L120 100 L120 100");
    }
}
