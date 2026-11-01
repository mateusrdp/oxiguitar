// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading a sample between its frames: a 32-tap windowed sinc (a Kaiser
//! window), so a sample recorded at 44.1 kHz and played at 48 kHz, or a
//! note pitched off its sample, keeps its top octave (flat to about
//! 18.5 kHz from a 44.1 kHz sample) and gains no aliasing (quality first:
//! Mateus, 21 Oct 2026).
//!
//! Played at or below its own speed, a sample is cut at its own Nyquist
//! frequency: the images that leaves fall above 22 kHz. Played faster
//! (pitched up, or a 96 kHz sample at 48 kHz) it needs its top cut lower,
//! or it folds back as aliasing: there is a table for each range of
//! speeds, each cutting at the speed's Nyquist frequency. Each table holds
//! the 32 weights for 1024 positions between two frames; a position takes
//! the nearest (an error of a 2048th of a frame at most, under −60 dB at
//! 10 kHz).
//!
//! The tables are made once, before anything plays, and only read after.

use std::sync::OnceLock;

/// Weights a position: 16 frames on each side.
pub(crate) const TAPS: usize = 32;
/// Frames before the position's own that its weights reach.
pub(crate) const BEFORE: i64 = 15;
/// Positions between two frames.
const PHASES: usize = 1024;
/// The speeds each table is made for; a speed takes the first table at
/// or above it (faster still: the last).
const SPEEDS: [f64; 9] = [1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 8.0];
/// Where the cut is (−6 dB), as a share of the Nyquist frequency at
/// speed 1.
const CUTOFF: f64 = 1.0;
/// The Kaiser window's shape: about −80 dB beyond the cut.
const BETA: f64 = 8.0;

/// The tables.
pub(crate) struct Kernel {
    tables: Vec<Box<[f32]>>,
}

/// The tables, made the first time they are asked for: call it before
/// anything plays.
pub(crate) fn kernel() -> &'static Kernel {
    static KERNEL: OnceLock<Kernel> = OnceLock::new();
    KERNEL.get_or_init(|| Kernel {
        tables: SPEEDS.iter().map(|&s| table(CUTOFF / s)).collect(),
    })
}

/// The zeroth modified Bessel function, by its series.
fn bessel0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..50 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < 1e-12 * sum {
            break;
        }
    }
    sum
}

/// One table: for each position between two frames, the 32 weights,
/// summing to 1 (so a steady signal stays at its level).
fn table(cutoff: f64) -> Box<[f32]> {
    let mut t = vec![0.0f32; PHASES * TAPS];
    let half = TAPS as f64 / 2.0;
    let norm = bessel0(BETA);
    for p in 0..PHASES {
        let frac = p as f64 / PHASES as f64;
        let row = &mut t[p * TAPS..(p + 1) * TAPS];
        let mut sum = 0.0;
        let mut weights = [0.0f64; TAPS];
        for (k, w) in weights.iter_mut().enumerate() {
            // The tap's distance from the position, in frames.
            let x = k as f64 - BEFORE as f64 - frac;
            let sinc = if x.abs() < 1e-12 {
                1.0
            } else {
                let a = std::f64::consts::PI * cutoff * x;
                a.sin() / a
            };
            let r = x / half;
            let window = if r.abs() >= 1.0 {
                0.0
            } else {
                bessel0(BETA * (1.0 - r * r).sqrt()) / norm
            };
            *w = cutoff * sinc * window;
            sum += *w;
        }
        for (out, w) in row.iter_mut().zip(weights) {
            *out = (w / sum) as f32;
        }
    }
    t.into_boxed_slice()
}

impl Kernel {
    /// The weights for reading at fraction `frac` (0 to 1) past a frame,
    /// at `speed` source frames for each frame played.
    #[inline]
    pub(crate) fn weights(&self, speed: f64, frac: f64) -> &[f32] {
        let t = SPEEDS
            .iter()
            .position(|&s| speed <= s * 1.000_001)
            .unwrap_or(SPEEDS.len() - 1);
        let p = ((frac * PHASES as f64) as usize).min(PHASES - 1);
        &self.tables[t][p * TAPS..(p + 1) * TAPS]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sine of `cycles` per frame read through the kernel at `speed`,
    /// against the true value.
    fn error(cycles: f64, speed: f64) -> f64 {
        let x: Vec<f64> = (0..4000)
            .map(|i| (std::f64::consts::TAU * cycles * i as f64).sin())
            .collect();
        let k = kernel();
        let mut worst = 0.0f64;
        let mut pos = 100.0f64;
        while pos < 3800.0 {
            let i = pos.floor() as i64;
            let frac = pos - i as f64;
            let w = k.weights(speed, frac);
            let v: f64 = (0..TAPS)
                .map(|t| f64::from(w[t]) * x[(i - BEFORE + t as i64) as usize])
                .sum();
            let truth = (std::f64::consts::TAU * cycles * pos).sin();
            worst = worst.max((v - truth).abs());
            pos += speed * 0.913;
        }
        worst
    }

    #[test]
    fn it_keeps_what_is_below_its_cut() {
        // 1 kHz, 10 kHz and 16 kHz at 44.1 kHz.
        for (hz, most) in [(1_000.0, 1e-3), (10_000.0, 2e-3), (16_000.0, 2e-2)] {
            let e = error(hz / 44_100.0, 1.0);
            assert!(e < most, "{hz} Hz: {e}");
        }
        // A steady signal stays at its level.
        let w = kernel().weights(1.0, 0.37);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn at_a_frame_it_gives_the_frame() {
        let w = kernel().weights(1.0, 0.0);
        assert!((w[BEFORE as usize] - 1.0).abs() < 0.05, "{w:?}");
    }

    #[test]
    fn faster_it_cuts_lower() {
        // A tone at 0.4 cycles a frame (near the top) read twice as fast:
        // it would fold back, so the faster table removes it.
        let x: Vec<f64> = (0..400)
            .map(|i| (std::f64::consts::TAU * 0.4 * i as f64).sin())
            .collect();
        let k = kernel();
        let level = |speed: f64| {
            let mut peak = 0.0f64;
            for i in 50..350 {
                let w = k.weights(speed, 0.5);
                let v: f64 = (0..TAPS)
                    .map(|t| f64::from(w[t]) * x[(i - BEFORE + t as i64) as usize])
                    .sum();
                peak = peak.max(v.abs());
            }
            peak
        };
        assert!(level(1.0) > 0.5);
        assert!(level(2.0) < 0.01, "{}", level(2.0));
    }
}
