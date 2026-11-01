// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A cabinet impulse response, applied with no latency: the first block of
//! its taps directly, the rest by uniformly partitioned convolution in the
//! frequency domain (overlap-save, a frequency-domain delay line), whose
//! output for each block is ready before the block starts. rustfft
//! (MIT OR Apache-2.0) does the transforms, with scratch space allocated
//! once, so nothing is allocated on the audio thread.

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

/// The block, in frames: the taps applied directly, and each partition.
const BLOCK: usize = 64;

/// The longest response kept, in seconds: a cabinet's dies out long before.
pub(crate) const LONGEST: f64 = 1.0;

/// One impulse response, ready to run.
pub(crate) struct Convolver {
    /// The first block's taps, newest sample's tap last.
    head: [f32; BLOCK],
    /// The last `BLOCK` inputs, twice over, so any window is one slice.
    line: [f32; 2 * BLOCK],
    /// Where the newest input went in `line`.
    at: usize,
    /// Each later partition's spectrum (already divided by the transform's
    /// size).
    parts: Vec<Vec<Complex32>>,
    /// The latest input spectra, newest at `newest`, a ring as long as
    /// `parts`.
    spectra: Vec<Vec<Complex32>>,
    newest: usize,
    /// The block being filled, and the one before it.
    block: [f32; BLOCK],
    before: [f32; BLOCK],
    /// Where the next input goes in `block`.
    pos: usize,
    /// The later partitions' sound for the current block.
    tail: [f32; BLOCK],
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    work: Vec<Complex32>,
    sum: Vec<Complex32>,
    scratch: Vec<Complex32>,
}

impl std::fmt::Debug for Convolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Convolver")
            .field("partitions", &(self.parts.len() + 1))
            .finish_non_exhaustive()
    }
}

impl Convolver {
    /// Ready to apply `taps` (already at the playback rate).
    pub(crate) fn new(taps: &[f32]) -> Convolver {
        let n = 2 * BLOCK;
        let mut planner = FftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(n);
        let inverse = planner.plan_fft_inverse(n);
        let scratch_len = forward
            .get_inplace_scratch_len()
            .max(inverse.get_inplace_scratch_len());
        let mut head = [0.0f32; BLOCK];
        for (k, &t) in taps.iter().take(BLOCK).enumerate() {
            // The window holds the oldest input first: tap k reads the
            // input k samples before the newest.
            head[BLOCK - 1 - k] = t;
        }
        let scale = 1.0 / n as f32;
        let mut scratch = vec![Complex32::default(); scratch_len];
        let parts: Vec<Vec<Complex32>> = taps
            .get(BLOCK..)
            .unwrap_or_default()
            .chunks(BLOCK)
            .map(|chunk| {
                let mut spectrum = vec![Complex32::default(); n];
                for (s, &t) in spectrum.iter_mut().zip(chunk) {
                    *s = Complex32::new(t * scale, 0.0);
                }
                forward.process_with_scratch(&mut spectrum, &mut scratch);
                spectrum
            })
            .collect();
        let spectra = vec![vec![Complex32::default(); n]; parts.len().max(1)];
        Convolver {
            head,
            line: [0.0; 2 * BLOCK],
            at: BLOCK - 1,
            parts,
            spectra,
            newest: 0,
            block: [0.0; BLOCK],
            before: [0.0; BLOCK],
            pos: 0,
            tail: [0.0; BLOCK],
            forward,
            inverse,
            work: vec![Complex32::default(); n],
            sum: vec![Complex32::default(); n],
            scratch,
        }
    }

    /// Applies the response to `io`, in place.
    pub(crate) fn process(&mut self, io: &mut [f32]) {
        for v in io {
            let x = *v;
            self.at = (self.at + 1) % BLOCK;
            self.line[self.at] = x;
            self.line[self.at + BLOCK] = x;
            let window = &self.line[self.at + 1..self.at + 1 + BLOCK];
            let mut y = 0.0f32;
            for (h, s) in self.head.iter().zip(window) {
                y += h * s;
            }
            *v = y + self.tail[self.pos];
            self.block[self.pos] = x;
            self.pos += 1;
            if self.pos == BLOCK {
                self.pos = 0;
                self.next_tail();
            }
        }
    }

    /// A block is complete: the later partitions' sound for the next one,
    /// which needs only the blocks up to this one.
    fn next_tail(&mut self) {
        if self.parts.is_empty() {
            return;
        }
        for (w, &s) in self
            .work
            .iter_mut()
            .zip(self.before.iter().chain(&self.block))
        {
            *w = Complex32::new(s, 0.0);
        }
        self.before = self.block;
        self.forward
            .process_with_scratch(&mut self.work, &mut self.scratch);
        let count = self.spectra.len();
        self.newest = (self.newest + 1) % count;
        self.spectra[self.newest].copy_from_slice(&self.work);
        // Partition j (from 1) meets the block j - 1 back.
        self.sum.fill(Complex32::default());
        for (j, part) in self.parts.iter().enumerate() {
            let x = &self.spectra[(self.newest + count - j) % count];
            for ((s, a), b) in self.sum.iter_mut().zip(x).zip(part) {
                *s += a * b;
            }
        }
        self.inverse
            .process_with_scratch(&mut self.sum, &mut self.scratch);
        for (t, s) in self.tail.iter_mut().zip(&self.sum[BLOCK..]) {
            *t = s.re;
        }
    }
}

/// Scales `taps` so white noise keeps its level through them (unit
/// energy), so every response sounds about as loud, and keeps at most
/// [`LONGEST`] seconds at `rate`.
pub(crate) fn normalise(mut taps: Vec<f32>, rate: u32) -> Vec<f32> {
    taps.truncate((LONGEST * f64::from(rate)) as usize);
    let energy: f64 = taps.iter().map(|&t| f64::from(t) * f64::from(t)).sum();
    if energy > 0.0 {
        let k = (1.0 / energy.sqrt()) as f32;
        for t in &mut taps {
            *t *= k;
        }
    }
    taps
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plain sum, for comparison.
    fn direct(taps: &[f32], x: &[f32]) -> Vec<f32> {
        (0..x.len())
            .map(|n| {
                taps.iter()
                    .enumerate()
                    .take(n + 1)
                    .map(|(k, &t)| t * x[n - k])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn it_matches_the_plain_sum_with_no_latency_in_pieces_of_any_size() {
        let taps: Vec<f32> = (0..1000)
            .map(|k| ((k * 7919) % 101) as f32 / 101.0 - 0.5)
            .map(|v| v * (-(1.0 / 300.0f32)).exp())
            .collect();
        let x: Vec<f32> = (0..5000)
            .map(|n| ((n * 104_729) % 211) as f32 / 211.0 - 0.5)
            .collect();
        let want = direct(&taps, &x);
        let mut c = Convolver::new(&taps);
        let mut got = Vec::new();
        let sizes = [1usize, 63, 64, 65, 200, 7, 512];
        let mut at = 0;
        let mut k = 0;
        while at < x.len() {
            let n = sizes[k % sizes.len()].min(x.len() - at);
            k += 1;
            let mut piece = x[at..at + n].to_vec();
            c.process(&mut piece);
            got.extend_from_slice(&piece);
            at += n;
        }
        let worst = want
            .iter()
            .zip(&got)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-4, "{worst}");
    }

    #[test]
    fn a_short_response_needs_no_partitions() {
        let mut c = Convolver::new(&[0.5, 0.25]);
        let mut x = [1.0, 0.0, 0.0, 2.0];
        c.process(&mut x);
        assert_eq!(x, [0.5, 0.25, 0.0, 1.0]);
    }

    #[test]
    fn responses_are_brought_to_one_level() {
        let t = normalise(vec![2.0, 0.0, 2.0], 48_000);
        let e: f32 = t.iter().map(|v| v * v).sum();
        assert!((e - 1.0).abs() < 1e-6);
        assert_eq!(normalise(vec![1.0; 100_000], 48_000).len(), 48_000);
    }
}
