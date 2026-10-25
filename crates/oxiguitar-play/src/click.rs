// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The metronome's clicks and the count-in (PLAY-5).
//!
//! A click on every beat of the time signature, the first of each bar
//! stronger: quarter notes in 4/4, eighths in 5/8 and 7/8, dotted quarters
//! in 6/8, 9/8 and 12/8 (compound time), halves in 2/2. Bars in free time
//! have no clicks. The count-in is one bar of clicks before the first bar,
//! in its time signature and at the tempo it starts with.

use oxiguitar_model::{Ratio, Score, TimeSignature};

use crate::time::TimeMap;

/// One click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Click {
    /// Seconds: from the song's start for the metronome, from the count-in's
    /// start for the count-in.
    pub time: f64,
    /// The first beat of a bar.
    pub strong: bool,
}

/// The beat the metronome clicks on, in whole notes, and how many a bar
/// has.
pub fn beat_of(ts: TimeSignature) -> Option<(Ratio, u32)> {
    let (n, d) = (i64::from(ts.numerator), i64::from(ts.denominator));
    if n == 0 || d == 0 {
        return None;
    }
    if d >= 8 && n > 3 && n % 3 == 0 {
        Some((Ratio::new(3, d)?, (n / 3) as u32))
    } else {
        Some((Ratio::new(1, d)?, n as u32))
    }
}

/// The metronome's clicks over the whole song.
pub fn metronome(score: &Score, map: &TimeMap) -> Vec<Click> {
    let mut out = Vec::new();
    for (vi, v) in map.visits.iter().enumerate() {
        let mb = &score.master_bars[v.visit.bar];
        if mb.free_time {
            continue;
        }
        let Some((unit, count)) = beat_of(mb.time_signature) else {
            continue;
        };
        let mut at = Ratio::ZERO;
        for k in 0..count {
            out.push(Click {
                time: map.time_f64(vi, at.to_f64()),
                strong: k == 0,
            });
            at = at + unit;
        }
    }
    out
}

/// The count-in: one bar of clicks before the first bar, and how long it
/// lasts.
pub fn count_in(score: &Score, map: &TimeMap) -> (Vec<Click>, f64) {
    let Some(first) = map.visits.first() else {
        return (Vec::new(), 0.0);
    };
    let ts = score.master_bars[first.visit.bar].time_signature;
    let Some((unit, count)) = beat_of(ts) else {
        return (Vec::new(), 0.0);
    };
    let beat = unit.to_f64() * 240.0 / map.bpm(0, Ratio::ZERO);
    let clicks = (0..count)
        .map(|k| Click {
            time: beat * f64::from(k),
            strong: k == 0,
        })
        .collect();
    (clicks, beat * f64::from(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(n: u8, d: u8) -> TimeSignature {
        TimeSignature {
            numerator: n,
            denominator: d,
        }
    }

    #[test]
    fn beats_of_simple_and_compound_time() {
        assert_eq!(beat_of(ts(4, 4)), Some((Ratio::new(1, 4).unwrap(), 4)));
        assert_eq!(beat_of(ts(6, 8)), Some((Ratio::new(3, 8).unwrap(), 2)));
        assert_eq!(beat_of(ts(12, 8)), Some((Ratio::new(3, 8).unwrap(), 4)));
        assert_eq!(beat_of(ts(3, 8)), Some((Ratio::new(1, 8).unwrap(), 3)));
        assert_eq!(beat_of(ts(7, 8)), Some((Ratio::new(1, 8).unwrap(), 7)));
        assert_eq!(beat_of(ts(2, 2)), Some((Ratio::new(1, 2).unwrap(), 2)));
        assert_eq!(beat_of(ts(0, 4)), None);
    }
}
