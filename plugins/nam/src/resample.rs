// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Sample-rate conversion. A NAM model runs at the rate it was captured at
//! (48 kHz, nearly always) and gives wrong sound at any other, so a track
//! played at another rate (a device that refuses 48 kHz) goes to the
//! model's rate and back ([`Converted`]); impulse responses are converted
//! once, when a track is prepared ([`convert`]).
//!
//! A windowed sinc (Kaiser, β 8, 32 taps), read from a table of 256 phases
//! with linear interpolation between them, cut at 95 % of the lower rate's
//! Nyquist frequency.

/// Taps per output sample.
const TAPS: usize = 32;
/// Table rows between two input samples.
const PHASES: usize = 256;
/// Half the taps: how far ahead of an output an input must have arrived.
const HALF: usize = TAPS / 2;

/// The kernel for one ratio, as a table: row `p` holds the taps for an
/// output `p / PHASES` of a sample after an input sample.
#[derive(Debug)]
struct Kernel {
    table: Vec<f32>,
}

/// The zeroth-order modified Bessel function, for the Kaiser window.
fn bessel0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..50 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

impl Kernel {
    /// The kernel converting `from` to `to` (rates in hertz).
    fn new(from: f64, to: f64) -> Kernel {
        // The cut-off, in cycles per input sample.
        let cut = 0.5 * (to / from).min(1.0) * 0.95;
        let beta = 8.0;
        let norm = bessel0(beta);
        let mut table = vec![0.0f32; (PHASES + 1) * TAPS];
        for p in 0..=PHASES {
            let frac = p as f64 / PHASES as f64;
            for j in 0..TAPS {
                // Tap j reads the input sample `j - HALF + 1` after the one
                // before the output: its distance from the output.
                let t = (j as f64 - HALF as f64 + 1.0) - frac;
                let x = 2.0 * cut * t;
                let sinc = if x.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                let r = t / HALF as f64;
                let window = if r.abs() >= 1.0 {
                    0.0
                } else {
                    bessel0(beta * (1.0 - r * r).sqrt()) / norm
                };
                table[p * TAPS + j] = (2.0 * cut * sinc * window) as f32;
            }
        }
        Kernel { table }
    }

    /// The output at `frac` (0 to 1) of a sample after `input[HALF - 1]`,
    /// from `input[0..TAPS]`: `input` starts `HALF - 1` samples before the
    /// one the output follows.
    fn at(&self, input: &[f32], frac: f64) -> f32 {
        let pos = frac * PHASES as f64;
        let p = (pos as usize).min(PHASES - 1);
        let w = (pos - p as f64) as f32;
        let a = &self.table[p * TAPS..(p + 1) * TAPS];
        let b = &self.table[(p + 1) * TAPS..(p + 2) * TAPS];
        let mut sum = 0.0f32;
        for j in 0..TAPS {
            sum += input[j] * (a[j] + (b[j] - a[j]) * w);
        }
        sum
    }
}

/// Converts `input`, sampled at `from` hertz, to `to` hertz, whole: for
/// impulse responses, off the audio thread.
pub(crate) fn convert(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let kernel = Kernel::new(f64::from(from), f64::from(to));
    let step = f64::from(from) / f64::from(to);
    let len = ((input.len() as f64) / step).ceil() as usize;
    let padded: Vec<f32> = std::iter::repeat_n(0.0, HALF)
        .chain(input.iter().copied())
        .chain(std::iter::repeat_n(0.0, TAPS))
        .collect();
    (0..len)
        .map(|n| {
            let t = n as f64 * step;
            let i = t.floor() as usize;
            // padded[i + 1 .. i + 1 + TAPS] holds input[i - HALF + 1 ..].
            kernel.at(&padded[i + 1..i + 1 + TAPS], t - i as f64)
        })
        .collect()
}

/// A stream converted from one rate to another, a piece at a time, on the
/// audio thread: nothing is allocated after [`Stream::new`].
#[derive(Debug)]
struct Stream {
    kernel: Kernel,
    /// Input samples per output sample.
    step: f64,
    /// The inputs still needed, oldest first: `buf[0]` is input `start`.
    buf: Vec<f32>,
    /// The time of the next output, in inputs from `buf[0]`.
    pos: f64,
}

impl Stream {
    /// A stream from `from` to `to` hertz taking at most `most` inputs a
    /// call.
    fn new(from: u32, to: u32, most: usize) -> Stream {
        let mut buf = Vec::with_capacity(TAPS + most + 2);
        // Silence before the first input, so the first output (at the
        // first input's time, `buf[HALF]`) has its taps.
        buf.resize(HALF, 0.0);
        Stream {
            kernel: Kernel::new(f64::from(from), f64::from(to)),
            step: f64::from(from) / f64::from(to),
            buf,
            pos: HALF as f64,
        }
    }

    /// Takes `input` and writes into `out` every output its inputs now
    /// allow; returns how many. `out` must hold
    /// `input.len() / step + 2`.
    fn process(&mut self, input: &[f32], out: &mut [f32]) -> usize {
        self.buf.extend_from_slice(input);
        let mut n = 0;
        loop {
            let i = self.pos.floor() as usize;
            // Taps read buf[i + 1 - HALF ..= i + HALF].
            if i + HALF >= self.buf.len() || n == out.len() {
                break;
            }
            out[n] = self
                .kernel
                .at(&self.buf[i + 1 - HALF..i + 1 + HALF], self.pos - i as f64);
            n += 1;
            self.pos += self.step;
        }
        // Keep only what the next outputs need.
        let first = (self.pos.floor() as usize + 1).saturating_sub(HALF);
        self.buf.drain(..first);
        self.pos -= first as f64;
        n
    }
}

/// A piece of sound run through something that works at another rate (a
/// NAM model): converted there, run, converted back, then held in a queue
/// of a fixed length, so each piece comes back whole, `latency` frames
/// late. Nothing is allocated after [`Converted::new`].
#[derive(Debug)]
pub(crate) struct Converted {
    there: Stream,
    back: Stream,
    /// The piece at the other rate.
    far: Vec<f32>,
    /// The piece back at ours, before the queue.
    near: Vec<f32>,
    /// What came back and isn't out yet, in a ring.
    queue: Vec<f32>,
    head: usize,
    len: usize,
    latency: usize,
}

impl Converted {
    /// For pieces of at most `most` frames at `ours` hertz, through
    /// something running at `theirs`.
    pub(crate) fn new(ours: u32, theirs: u32, most: usize) -> Converted {
        let far_most = (most as f64 * f64::from(theirs) / f64::from(ours)).ceil() as usize + 4;
        // The two kernels' look-ahead, at our rate, and a few frames for the
        // pieces' rounding.
        let ahead = HALF + (HALF as f64 * f64::from(ours) / f64::from(theirs)).ceil() as usize;
        let latency = ahead + 4;
        let capacity = latency + most + 8;
        let mut c = Converted {
            there: Stream::new(ours, theirs, most),
            back: Stream::new(theirs, ours, far_most),
            far: vec![0.0; far_most],
            near: vec![0.0; most + 4],
            queue: vec![0.0; capacity],
            head: 0,
            len: 0,
            latency,
        };
        c.len = latency;
        c
    }

    /// The frames the sound comes back late by.
    pub(crate) fn latency(&self) -> usize {
        self.latency
    }

    /// Runs `io` through `run` at the other rate; `io` comes back the
    /// latency late.
    pub(crate) fn process(&mut self, io: &mut [f32], mut run: impl FnMut(&mut [f32])) {
        let n = self.there.process(io, &mut self.far);
        run(&mut self.far[..n]);
        let m = self.back.process(&self.far[..n], &mut self.near);
        let cap = self.queue.len();
        for &v in &self.near[..m] {
            if self.len < cap {
                self.queue[(self.head + self.len) % cap] = v;
                self.len += 1;
            }
        }
        for v in io.iter_mut() {
            if self.len > 0 {
                *v = self.queue[self.head];
                self.head = (self.head + 1) % cap;
                self.len -= 1;
            } else {
                *v = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, hz: f64, len: usize) -> Vec<f32> {
        (0..len)
            .map(|n| (2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(rate)).sin() as f32)
            .collect()
    }

    #[test]
    fn a_whole_signal_keeps_its_pitch_and_level() {
        let x = sine(44_100, 1000.0, 44_100);
        let y = convert(&x, 44_100, 48_000);
        assert_eq!(y.len(), 48_000);
        // The middle, against a 1 kHz sine at 48 kHz.
        let want = sine(48_000, 1000.0, 48_000);
        let worst = (2000..46_000)
            .map(|n| (y[n] - want[n]).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 2e-3, "{worst}");
    }

    #[test]
    fn a_stream_there_and_back_comes_back_whole_and_late_by_its_latency() {
        for (ours, sizes) in [
            (44_100, [1usize, 64, 333, 512]),
            (96_000, [7, 100, 512, 13]),
        ] {
            let mut c = Converted::new(ours, 48_000, 512);
            let x = sine(ours, 440.0, ours as usize);
            let mut y = Vec::new();
            let mut at = 0;
            let mut k = 0;
            while at < x.len() {
                let n = sizes[k % sizes.len()].min(x.len() - at);
                k += 1;
                let mut piece = x[at..at + n].to_vec();
                c.process(&mut piece, |_| {});
                y.extend_from_slice(&piece);
                at += n;
            }
            let d = c.latency();
            let worst = (4000..x.len() - 4000)
                .map(|n| (y[n] - x[n - d]).abs())
                .fold(0.0f32, f32::max);
            // Within the fraction of a sample the conversion leaves.
            assert!(worst < 0.03, "{ours}: {worst} at a latency of {d}");
        }
    }
}
