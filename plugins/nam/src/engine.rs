// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What plays a track's amp model and cabinet.
//!
//! - Every build (`nam.oxp`): nam-rs 0.4.0 for the model, `convolve` for
//!   the cabinet, with no latency.
//! - The build for Linux on x86-64-v3 processors (`cfg(x86_64_v3)`,
//!   `nam-x86-64-v3.oxp`, build.rs): NeuralAmpModeler-rs 1.0.1 for both,
//!   about ten times faster. Its model plays what nam-rs plays (both match
//!   NAM's reference and each other to under 1e-5), and its cabinet what
//!   ours plays, `Cabinet::latency` frames later (one partition of its
//!   convolution, which it is given whole), which the effect reports so the
//!   player lines the track up with the others.
//!
//! The rest of the plugin (the model file's level and rate, the response's
//! rate and level, the conversion to the model's rate) is the same code in
//! both builds, so they sound the same.

use std::path::Path;

/// Which library plays the models, for messages and tests.
#[cfg(not(x86_64_v3))]
pub const ENGINE: &str = "nam-rs";
/// Which library plays the models, for messages and tests.
#[cfg(x86_64_v3)]
pub const ENGINE: &str = "NeuralAmpModeler-rs";

/// A model ready to play, warmed up.
pub(crate) struct Model {
    #[cfg(not(x86_64_v3))]
    model: nam_rs::Model,
    #[cfg(x86_64_v3)]
    model: Box<neural_amp_modeler_rs::prelude::StaticModel>,
    /// The input, copied: NeuralAmpModeler-rs reads one buffer and writes
    /// another.
    #[cfg(x86_64_v3)]
    scratch: Vec<f32>,
}

/// The most frames NeuralAmpModeler-rs is given at once: under the buffer
/// size it prepares its models for (8192).
#[cfg(x86_64_v3)]
const MOST: usize = 4096;

impl Model {
    /// The model in the file at `path`, which `file` holds as nam-rs read
    /// it.
    #[cfg(not(x86_64_v3))]
    pub(crate) fn new(_path: &Path, file: &nam_rs::NamModel) -> Result<Model, String> {
        let mut model = nam_rs::Model::from_nam(file).map_err(|e| e.to_string())?;
        // A fresh model's first sound is a transient against silence:
        // spent now, not on the first note.
        let mut warm = vec![0.0f32; model.receptive_field().max(1)];
        model.process_buffer(&mut warm);
        Ok(Model { model })
    }

    /// The model in the file at `path`, which `file` holds as nam-rs read
    /// it.
    #[cfg(x86_64_v3)]
    pub(crate) fn new(path: &Path, _file: &nam_rs::NamModel) -> Result<Model, String> {
        use neural_amp_modeler_rs::prelude::{LoadOptions, SystemSnapshot, load_and_build_model};
        // Loading warms the model up (its prewarm: silence through the
        // whole receptive field), as nam-rs's is above. Its level
        // adjustments (`input_mult_adj`, `output_mult_adj`) are left
        // unused: the plugin sets the level from the file itself, as for
        // nam-rs.
        let pair = load_and_build_model(
            path,
            &SystemSnapshot::capture(),
            false,
            LoadOptions::default(),
        )
        .map_err(|e| e.to_string())?;
        let model = pair
            .model_l
            .ok_or_else(|| "the file holds no model".to_owned())?;
        Ok(Model {
            model,
            scratch: vec![0.0; MOST],
        })
    }

    /// Plays `io` through the model, in place.
    pub(crate) fn process(&mut self, io: &mut [f32]) {
        #[cfg(not(x86_64_v3))]
        self.model.process_buffer(io);
        #[cfg(x86_64_v3)]
        {
            use neural_amp_modeler_rs::prelude::NamModel as _;
            for piece in io.chunks_mut(MOST) {
                let input = &mut self.scratch[..piece.len()];
                input.copy_from_slice(piece);
                self.model.process(input, piece);
            }
        }
    }
}

/// A cabinet's response, ready to play.
pub(crate) struct Cabinet {
    #[cfg(not(x86_64_v3))]
    convolver: crate::convolve::Convolver,
    #[cfg(x86_64_v3)]
    adapter: neural_amp_modeler_rs::prelude::CabSimAdapter,
    /// The partition being filled with the track's sound.
    #[cfg(x86_64_v3)]
    input: [f32; PARTITION],
    /// The cabinet's sound of the partition before, handed out while the
    /// next one fills: what makes the latency exactly one partition.
    #[cfg(x86_64_v3)]
    output: [f32; PARTITION],
    /// How much of `input` is filled (and of `output` handed out).
    #[cfg(x86_64_v3)]
    at: usize,
}

/// NeuralAmpModeler-rs's partition, in frames: the cabinet's latency
/// (1.3 ms at 48 kHz).
#[cfg(x86_64_v3)]
pub(crate) const PARTITION: usize = 64;

impl Cabinet {
    /// The response `taps`, at the track's rate and level already.
    pub(crate) fn new(taps: &[f32]) -> Result<Cabinet, String> {
        #[cfg(not(x86_64_v3))]
        {
            Ok(Cabinet {
                convolver: crate::convolve::Convolver::new(taps),
            })
        }
        #[cfg(x86_64_v3)]
        {
            use neural_amp_modeler_rs::prelude::{CabSimAdapter, ConvEngine};
            let engine = ConvEngine::new(taps, PARTITION).map_err(|e| e.to_string())?;
            let adapter = CabSimAdapter::new(Box::new(engine)).map_err(|e| e.to_string())?;
            Ok(Cabinet {
                adapter,
                input: [0.0; PARTITION],
                output: [0.0; PARTITION],
                at: 0,
            })
        }
    }

    /// Plays `io` through the cabinet, in place.
    pub(crate) fn process(&mut self, io: &mut [f32]) {
        #[cfg(not(x86_64_v3))]
        self.convolver.process(io);
        // NeuralAmpModeler-rs's convolution is given whole partitions only:
        // given a piece shorter than one, it hands back silence for what it
        // hasn't computed yet, so pieces of uneven sizes would leave gaps.
        // Each frame goes in and the frame a partition earlier comes out.
        #[cfg(x86_64_v3)]
        for frame in io.iter_mut() {
            let heard = self.output[self.at];
            self.input[self.at] = *frame;
            *frame = heard;
            self.at += 1;
            if self.at == PARTITION {
                self.adapter.process_block(&mut self.input, None);
                self.output = self.input;
                self.at = 0;
            }
        }
    }

    /// How many frames late the cabinet's sound comes.
    pub(crate) fn latency(&self) -> usize {
        #[cfg(not(x86_64_v3))]
        return 0;
        #[cfg(x86_64_v3)]
        return PARTITION;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/nam")
            .join(name)
    }

    /// Plucked notes, as a DI guitar gives them: decaying partials.
    fn plucks(frames: usize, rate: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                let t = (i % 12_000) as f32 / rate;
                let f = [110.0, 146.8, 196.0, 246.9][(i / 12_000) % 4];
                let decay = (-t * 6.0).exp();
                0.3 * decay
                    * ((2.0 * std::f32::consts::PI * f * t).sin()
                        + 0.5 * (4.0 * std::f32::consts::PI * f * t).sin())
            })
            .collect()
    }

    /// Plays `x` through `run` in pieces of uneven sizes, as the player's
    /// blocks come.
    fn in_pieces(x: &[f32], mut run: impl FnMut(&mut [f32])) -> Vec<f32> {
        let mut out = x.to_vec();
        let sizes = [64, 128, 96, 256, 1, 17, 512, 4800, 10_000];
        let mut at = 0;
        let mut i = 0;
        while at < out.len() {
            let n = sizes[i % sizes.len()].min(out.len() - at);
            run(&mut out[at..at + n]);
            at += n;
            i += 1;
        }
        out
    }

    #[test]
    fn models_play_whatever_the_pieces_are() {
        for name in ["wavenet.nam", "lstm.nam"] {
            let path = fixture(name);
            let file = nam_rs::NamModel::from_file(&path).unwrap();
            let x = plucks(48_000, 48_000.0);
            let mut a = Model::new(&path, &file).unwrap();
            let mut b = Model::new(&path, &file).unwrap();
            let whole = {
                let mut y = x.clone();
                a.process(&mut y);
                y
            };
            let pieces = in_pieces(&x, |p| b.process(p));
            let peak = whole.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            assert!(peak > 0.01, "{name} ({ENGINE}) is silent");
            let diff = whole
                .iter()
                .zip(&pieces)
                .fold(0.0f32, |p, (u, v)| p.max((u - v).abs()));
            assert!(diff < 1e-6, "{name} ({ENGINE}): pieces differ by {diff}");
        }
    }

    /// The fast build's model and cabinet play what nam-rs and our
    /// convolution play: the two builds sound the same.
    #[cfg(x86_64_v3)]
    #[test]
    fn the_fast_build_sounds_like_the_other() {
        let x = plucks(48_000, 48_000.0);
        for name in ["wavenet.nam", "lstm.nam"] {
            let path = fixture(name);
            let file = nam_rs::NamModel::from_file(&path).unwrap();
            let mut ours = nam_rs::Model::from_nam(&file).unwrap();
            let mut warm = vec![0.0f32; ours.receptive_field().max(1)];
            ours.process_buffer(&mut warm);
            let mut theirs = Model::new(&path, &file).unwrap();
            let a = in_pieces(&x, |p| ours.process_buffer(p));
            let b = in_pieces(&x, |p| theirs.process(p));
            let peak = a.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            let diff = a
                .iter()
                .zip(&b)
                .fold(0.0f32, |p, (u, v)| p.max((u - v).abs()));
            println!("{name}: largest difference {diff:.2e}, peak {peak:.3}");
            assert!(
                diff < 1e-4 * peak.max(1.0),
                "{name}: the builds differ by {diff}"
            );
        }

        for (name, data) in crate::BUILT_IN {
            let response = crate::wav::read(data).unwrap();
            let taps = crate::convolve::normalise(
                crate::resample::convert(&response.taps, response.rate, 48_000),
                48_000,
            );
            let mut ours = crate::convolve::Convolver::new(&taps);
            let mut theirs = Cabinet::new(&taps).unwrap();
            let late = theirs.latency();
            assert_eq!(late, PARTITION);
            let a = in_pieces(&x, |p| ours.process(p));
            let b = in_pieces(&x, |p| theirs.process(p));
            let peak = a.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            // Their output against ours at each lag up to two partitions:
            // the best one must be the latency it declares. (Their
            // denormal guard adds a dither near 1e-11, so silence isn't
            // exactly zero.)
            let differ = |lag: usize| {
                a[..a.len() - lag]
                    .iter()
                    .zip(&b[lag..])
                    .fold(0.0f32, |p, (u, v)| p.max((u - v).abs()))
            };
            let (best, diff) = (0..=2 * PARTITION)
                .map(|lag| (lag, differ(lag)))
                .fold((0, f32::MAX), |m, d| if d.1 < m.1 { d } else { m });
            let before = b[..late].iter().fold(0.0f32, |p, v| p.max(v.abs()));
            println!(
                "{name}: best lag {best} (declared {late}), largest difference {diff:.2e}, \
                 before the latency {before:.2e}, peak {peak:.3}"
            );
            assert_eq!(
                best, late,
                "{name}: the sound comes {best} frames late, not {late}"
            );
            assert!(before < 1e-6, "{name}: sound before its latency: {before}");
            assert!(
                diff < 1e-4 * peak.max(1.0),
                "{name}: the cabinets differ by {diff}"
            );
        }
    }
}
