// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! From musical time to seconds: how long each bar lasts, the tempo and its
//! gradual changes, and swing and triplet feel (PLAY-4).
//!
//! Positions are in whole notes. Tempo is in quarter notes a minute, so a
//! whole note lasts 240 / bpm seconds. A gradual change moves the tempo in
//! a straight line, in musical time, from its value to the next change's,
//! and the seconds are its exact integral (a logarithm). Times inside a bar
//! come from the score position, not from adding up note lengths; the
//! played bars add their lengths in floating point, which drifts by far
//! less than a sample even over hours.

use oxiguitar_model::{MasterBar, Ratio, Score, TripletFeel};

use crate::order::Visit;

/// The slowest and fastest tempos we accept; a file's tempo outside them
/// (0, or a damaged value) is brought inside.
const BPM_RANGE: (f64, f64) = (1.0, 1000.0);

/// How long each bar lasts, in whole notes: as long as its time signature
/// says, or as its longest voice if that is longer (a bar that doesn't add
/// up plays every note it has; docs/design/player.md). A bar in free time
/// lasts as long as its longest voice. A bar with nothing in it lasts its
/// time signature (a quarter note in free time).
pub fn bar_lengths(score: &Score) -> Vec<Ratio> {
    let quarter = Ratio::new(1, 4).unwrap_or(Ratio::ONE);
    score
        .master_bars
        .iter()
        .enumerate()
        .map(|(i, mb)| {
            let longest = score
                .tracks
                .iter()
                .flat_map(|t| t.staves.iter())
                .filter_map(|s| s.bars.get(i))
                .flat_map(|b| b.voices.iter())
                .filter(|v| !v.is_empty())
                .filter_map(|v| v.length())
                .max()
                .unwrap_or(Ratio::ZERO);
            let written = if mb.free_time {
                None
            } else {
                mb.expected_length()
            };
            match written {
                Some(w) if w > longest => w,
                _ if longest > Ratio::ZERO => longest,
                Some(w) => w,
                None => quarter,
            }
        })
        .collect()
}

/// A tempo point in score position: the tempo from here, and whether it
/// moves gradually to the next point's.
#[derive(Clone, Copy, Debug)]
struct Point {
    at: f64,
    bpm: f64,
    linear: bool,
    /// Seconds from the score's start to here, in score order.
    seconds: f64,
}

/// Seconds for whole notes from `x0` to `x1` at a tempo that goes in a
/// straight line from `b0` at `x0` to `b1` at `x1`.
fn ramp_seconds(x0: f64, x1: f64, b0: f64, b1: f64) -> f64 {
    let dx = x1 - x0;
    if dx <= 0.0 {
        return 0.0;
    }
    if (b1 - b0).abs() < 1e-9 {
        return 240.0 * dx / b0;
    }
    // dt/dx = 240 / (b0 + k (x - x0)), k = (b1 - b0) / dx.
    let k = (b1 - b0) / dx;
    240.0 / k * (b1 / b0).ln()
}

/// The tempo along the score (in score order, before any repeat), and the
/// seconds from its start to any score position.
#[derive(Clone, Debug)]
pub struct TempoMap {
    points: Vec<Point>,
}

impl TempoMap {
    /// The tempo map of `score`, whose bars start at the score positions
    /// `starts` (one more than there are bars: the last is the end).
    pub fn new(score: &Score, starts: &[f64]) -> TempoMap {
        let clamp = |b: f64| {
            if b.is_finite() {
                b.clamp(BPM_RANGE.0, BPM_RANGE.1)
            } else {
                120.0
            }
        };
        let mut points = vec![Point {
            at: 0.0,
            bpm: clamp(score.initial_tempo),
            linear: false,
            seconds: 0.0,
        }];
        for (i, mb) in score.master_bars.iter().enumerate() {
            let (start, end) = (starts[i], starts[i + 1]);
            for tc in &mb.tempo_changes {
                let at = (start + tc.position.to_f64()).min(end);
                points.push(Point {
                    at,
                    bpm: clamp(tc.bpm),
                    linear: tc.linear,
                    seconds: 0.0,
                });
            }
        }
        // Stable: two changes at one place keep their order, the later wins.
        points.sort_by(|a, b| a.at.total_cmp(&b.at));
        for k in 1..points.len() {
            let (p, q) = (points[k - 1], points[k]);
            let to = if p.linear { q.bpm } else { p.bpm };
            points[k].seconds = p.seconds + ramp_seconds(p.at, q.at, p.bpm, to);
        }
        TempoMap { points }
    }

    /// The index of the last point at or before `x`.
    fn point_at(&self, x: f64) -> usize {
        self.points.partition_point(|p| p.at <= x).saturating_sub(1)
    }

    /// The tempo at score position `x`, in quarter notes a minute.
    pub fn bpm(&self, x: f64) -> f64 {
        let k = self.point_at(x);
        let p = self.points[k];
        match self.points.get(k + 1) {
            Some(q) if p.linear && q.at > p.at => {
                p.bpm + (q.bpm - p.bpm) * (x - p.at) / (q.at - p.at)
            }
            _ => p.bpm,
        }
    }

    /// Seconds from the score's start to score position `x`, in score order.
    pub fn seconds(&self, x: f64) -> f64 {
        let k = self.point_at(x);
        let p = self.points[k];
        p.seconds + ramp_seconds(p.at, x, p.bpm, self.bpm(x))
    }
}

/// Swing and triplet feel: where a position inside a bar is played.
///
/// The feel works on pairs of notes inside each beat of its unit (a
/// quarter for eighth-note feels, an eighth for sixteenth-note feels): the
/// first half of the unit stretches or shrinks to `split` of it, and the
/// second half takes the rest. Positions on the unit's beats don't move.
pub fn feel(feel: TripletFeel, offset: Ratio) -> Ratio {
    let (unit, split) = match feel {
        TripletFeel::None => return offset,
        TripletFeel::Triplet8th => ((1, 4), (2, 3)),
        TripletFeel::Triplet16th => ((1, 8), (2, 3)),
        TripletFeel::Dotted8th => ((1, 4), (3, 4)),
        TripletFeel::Dotted16th => ((1, 8), (3, 4)),
        TripletFeel::Scottish8th => ((1, 4), (1, 4)),
        TripletFeel::Scottish16th => ((1, 8), (1, 4)),
    };
    let (Some(unit), Some(split)) = (Ratio::new(unit.0, unit.1), Ratio::new(split.0, split.1))
    else {
        return offset;
    };
    let warp = || -> Option<Ratio> {
        let beats = offset.checked_div(unit)?;
        let whole = Ratio::from_integer(beats.numer().div_euclid(beats.denom()));
        let r = beats.checked_sub(whole)?;
        let half = Ratio::new(1, 2)?;
        let two = Ratio::new(2, 1)?;
        let w = if r < half {
            r.checked_mul(two)?.checked_mul(split)?
        } else {
            let rest = Ratio::ONE.checked_sub(split)?;
            split.checked_add(r.checked_sub(half)?.checked_mul(two)?.checked_mul(rest)?)?
        };
        whole.checked_add(w)?.checked_mul(unit)
    };
    warp().unwrap_or(offset)
}

/// One bar as played, with its time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisitTime {
    /// The bar and its pass.
    pub visit: Visit,
    /// Seconds from the start of the song (at 100 % speed).
    pub start: f64,
    /// Seconds from the start of the song to its end.
    pub end: f64,
}

/// When every played bar starts and ends, and the way back from a time to
/// a place in the music (for the cursor).
#[derive(Clone, Debug)]
pub struct TimeMap {
    tempo: TempoMap,
    /// Score position where each bar starts, and the end of the last.
    starts: Vec<f64>,
    lengths: Vec<Ratio>,
    feels: Vec<TripletFeel>,
    /// The played bars.
    pub visits: Vec<VisitTime>,
}

impl TimeMap {
    /// The time map of `score` played in the order `order`.
    pub fn new(score: &Score, order: &[Visit]) -> TimeMap {
        let lengths = bar_lengths(score);
        let mut starts = Vec::with_capacity(lengths.len() + 1);
        let mut x = 0.0;
        starts.push(0.0);
        for l in &lengths {
            x += l.to_f64();
            starts.push(x);
        }
        let tempo = TempoMap::new(score, &starts);
        let feels = score
            .master_bars
            .iter()
            .map(|mb: &MasterBar| mb.triplet_feel)
            .collect();
        let mut visits = Vec::with_capacity(order.len());
        let mut t = 0.0;
        for v in order {
            let d = tempo.seconds(starts[v.bar + 1]) - tempo.seconds(starts[v.bar]);
            visits.push(VisitTime {
                visit: *v,
                start: t,
                end: t + d,
            });
            t += d;
        }
        TimeMap {
            tempo,
            starts,
            lengths,
            feels,
            visits,
        }
    }

    /// How long the song lasts, in seconds.
    pub fn length(&self) -> f64 {
        self.visits.last().map_or(0.0, |v| v.end)
    }

    /// The bar's length in whole notes.
    pub fn bar_length(&self, bar: usize) -> Ratio {
        self.lengths.get(bar).copied().unwrap_or(Ratio::ZERO)
    }

    /// The time of a musical position `offset` (whole notes from the bar's
    /// start, as written) in the `visit`-th played bar, with the bar's
    /// feel applied.
    pub fn time(&self, visit: usize, offset: Ratio) -> f64 {
        let Some(v) = self.visits.get(visit) else {
            return self.length();
        };
        let bar = v.visit.bar;
        let felt = feel(self.feels[bar], offset);
        let x0 = self.starts[bar];
        v.start + self.tempo.seconds(x0 + felt.to_f64()) - self.tempo.seconds(x0)
    }

    /// The time of `offset` given in whole notes as a float, without feel
    /// (for points inside a note, such as a bend's).
    pub fn time_f64(&self, visit: usize, offset: f64) -> f64 {
        let Some(v) = self.visits.get(visit) else {
            return self.length();
        };
        let x0 = self.starts[v.visit.bar];
        v.start + self.tempo.seconds(x0 + offset) - self.tempo.seconds(x0)
    }

    /// The tempo at a written position of a played bar, in quarter notes a
    /// minute.
    pub fn bpm(&self, visit: usize, offset: Ratio) -> f64 {
        match self.visits.get(visit) {
            Some(v) => self.tempo.bpm(self.starts[v.visit.bar] + offset.to_f64()),
            None => 120.0,
        }
    }

    /// Where the music is at `time` seconds: the played bar's index and the
    /// position in it in whole notes, as played (feel included). `None`
    /// before the start or after the end.
    pub fn find(&self, time: f64) -> Option<(usize, f64)> {
        if !(0.0..self.length()).contains(&time) {
            return None;
        }
        let k = self.visits.partition_point(|v| v.end <= time);
        let v = self.visits.get(k)?;
        let bar = v.visit.bar;
        let (x0, x1) = (self.starts[bar], self.starts[bar + 1]);
        let base = self.tempo.seconds(x0);
        let target = time - v.start;
        // The time is monotonic in the position: bisect.
        let (mut lo, mut hi) = (0.0, x1 - x0);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if self.tempo.seconds(x0 + mid) - base < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some((k, 0.5 * (lo + hi)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::playing_order;
    use oxiguitar_model::TempoChange;

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    fn score(bars: usize, bpm: f64) -> Score {
        let mut s = Score::new();
        s.initial_tempo = bpm;
        s.master_bars = vec![MasterBar::default(); bars];
        s
    }

    #[test]
    fn a_bar_of_four_four_at_120_lasts_two_seconds() {
        let s = score(3, 120.0);
        let m = TimeMap::new(&s, &playing_order(&s.master_bars));
        assert_eq!(m.visits.len(), 3);
        assert!((m.length() - 6.0).abs() < 1e-12);
        assert!((m.time(1, r(1, 4)) - 2.5).abs() < 1e-12);
        let (k, at) = m.find(2.5).unwrap();
        assert_eq!(k, 1);
        assert!((at - 0.25).abs() < 1e-9);
        assert_eq!(m.find(6.0), None);
    }

    #[test]
    fn a_tempo_change_and_a_gradual_one() {
        // 120 to 60 in the middle of bar 1, held; then a ramp in bar 2 from
        // 60 up to 120 at bar 3.
        let mut s = score(3, 120.0);
        s.master_bars[0].tempo_changes.push(TempoChange {
            position: r(1, 2),
            bpm: 60.0,
            label: String::new(),
            linear: true,
        });
        s.master_bars[2].tempo_changes.push(TempoChange {
            position: Ratio::ZERO,
            bpm: 120.0,
            label: String::new(),
            linear: false,
        });
        let m = TimeMap::new(&s, &playing_order(&s.master_bars));
        // Bar 1: half at 120 (1 s), then the ramp starts at 60 and reaches
        // 120 one and a half whole notes later.
        let ramp = 240.0 / ((120.0 - 60.0) / 1.5) * (2.0f64).ln();
        assert!((m.visits[1].end - (1.0 + ramp)).abs() < 1e-9);
        assert!((m.visits[2].end - m.visits[2].start - 2.0).abs() < 1e-9);
        assert!((m.bpm(1, Ratio::ZERO) - 80.0).abs() < 1e-9);
    }

    #[test]
    fn repeats_take_the_tempo_of_the_bars_they_play() {
        // Bar 2 at 60; the repeat goes back to bar 1 at 120.
        let mut s = score(2, 120.0);
        s.master_bars[0].repeat_start = true;
        s.master_bars[1].repeat_end = Some(2);
        s.master_bars[1].tempo_changes.push(TempoChange {
            position: Ratio::ZERO,
            bpm: 60.0,
            label: String::new(),
            linear: false,
        });
        let m = TimeMap::new(&s, &playing_order(&s.master_bars));
        let lens: Vec<f64> = m.visits.iter().map(|v| v.end - v.start).collect();
        assert_eq!(lens.len(), 4);
        for (l, want) in lens.iter().zip([2.0, 4.0, 2.0, 4.0]) {
            assert!((l - want).abs() < 1e-9, "{lens:?}");
        }
    }

    #[test]
    fn triplet_feel_moves_the_second_eighth() {
        assert_eq!(feel(TripletFeel::Triplet8th, r(1, 8)), r(1, 6));
        assert_eq!(feel(TripletFeel::Triplet8th, r(3, 8)), r(1, 4) + r(1, 6));
        assert_eq!(feel(TripletFeel::Triplet8th, r(1, 4)), r(1, 4));
        assert_eq!(feel(TripletFeel::Dotted16th, r(1, 16)), r(3, 32));
        assert_eq!(feel(TripletFeel::Scottish8th, r(1, 8)), r(1, 16));
        assert_eq!(feel(TripletFeel::None, r(1, 8)), r(1, 8));
    }

    #[test]
    fn a_bar_that_doesnt_add_up_lasts_as_long_as_its_notes() {
        use oxiguitar_model::{Bar, Beat, Duration, NoteValue, Staff, Track, Voice};
        let mut s = score(1, 120.0);
        let beat = Beat {
            duration: Duration::new(NoteValue::Whole),
            ..Beat::default()
        };
        let voice = Voice {
            beats: vec![beat.clone(), beat],
        };
        s.tracks.push(Track {
            staves: vec![Staff {
                bars: vec![Bar {
                    voices: vec![voice],
                    ..Bar::default()
                }],
                ..Staff::default()
            }],
            ..Track::default()
        });
        assert_eq!(bar_lengths(&s), [r(2, 1)]);
        s.master_bars[0].free_time = true;
        assert_eq!(bar_lengths(&s), [r(2, 1)]);
    }
}
