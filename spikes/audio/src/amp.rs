// SPDX-License-Identifier: GPL-3.0-or-later
//! `amp`: the song's guitar from a DI sound set, through NeuralAmpModeler-rs
//! (an amp model and a cabinet impulse response), with bass and drums from
//! a General MIDI set, as 16-bit WAV files for listening. nam-rs runs the
//! same model on the same input, to compare speed and output. Feature
//! `nam2`: x86_64, Rust 1.99, RUSTFLAGS="-C target-cpu=x86-64-v3".

use crate::engines::{Engine, Oxi};
use crate::song::{self, Msg, RATE};
use neural_amp_modeler_rs::prelude::*;
use std::path::Path;
use std::time::Instant;

const BLOCK: usize = 64;

fn write16(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec { channels: 2, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample((l.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
        w.write_sample((r.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-9)
}

fn rms(x: &[f32]) -> f32 {
    ((x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt() as f32).max(1e-9)
}

/// Scales so the peak is at −1 dBFS.
fn normalise(v: &mut [f32]) {
    let g = 0.891 / peak(v);
    v.iter_mut().for_each(|x| *x *= g);
}

fn timed(mut f: impl FnMut(usize)) -> (f64, f64) {
    let blocks = song::length().div_ceil(BLOCK);
    let mut times = Vec::with_capacity(blocks);
    for b in 0..blocks {
        let t = Instant::now();
        f(b);
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (times[times.len() / 2], times[times.len() * 99 / 100])
}

/// The guitar alone, dry: the DI set's first preset on the guitar's six
/// channels, as the instrument plugin would render it.
pub fn di_guitar(di_sf: &str) -> (Vec<f32>, String) {
    let bytes = crate::load_sf2_bytes(di_sf);
    let font = rustysynth::SoundFont::new(&mut std::io::Cursor::new(&bytes)).unwrap();
    // A General MIDI set's clean electric guitar (program 27), else the
    // set's first preset (a DI set has one).
    let presets = font.get_presets();
    let first = presets
        .iter()
        .find(|p| p.get_bank_number() == 0 && p.get_patch_number() == 27)
        .unwrap_or(&presets[0]);
    let (bank, program) = (first.get_bank_number() as u8, first.get_patch_number() as u8);
    let preset = first.get_name().to_string();
    let mut engines: Vec<Box<dyn Engine>> = vec![Box::new(Oxi::new(&bytes))];
    let events: Vec<song::Event> = song::events()
        .into_iter()
        .filter(|e| e.track == 0)
        .flat_map(|mut e| {
            if let Msg::Program(_) = e.msg {
                e.msg = Msg::Program(program);
                let mut b = e;
                b.msg = Msg::Bank(bank);
                vec![b, e]
            } else {
                vec![e]
            }
        })
        .collect();
    let stems = crate::render_song(&mut engines, &events, BLOCK);
    let (l, r) = &stems.tracks[0];
    let mut mono: Vec<f32> = l.iter().zip(r).map(|(a, b)| (a + b) * 0.5).collect();
    // A DI at an interface's usual level: peaks at −6 dBFS.
    let g = 0.5 / peak(&mono);
    mono.iter_mut().for_each(|x| *x *= g);
    (mono, format!("{preset} (bank {bank}, program {program})"))
}

/// Bass and drums from a General MIDI set (OxiSynth), stereo.
pub fn backing(gm_sf: &str) -> (Vec<f32>, Vec<f32>) {
    let bytes = crate::load_sf2_bytes(gm_sf);
    let mut engines: Vec<Box<dyn Engine>> = vec![Box::new(Oxi::new(&bytes)), Box::new(Oxi::new(&bytes)), Box::new(Oxi::new(&bytes))];
    let events: Vec<song::Event> = song::events().into_iter().filter(|e| e.track != 0).collect();
    let stems = crate::render_song(&mut engines, &events, BLOCK);
    crate::mix(&stems.tracks[1..])
}

pub fn run(di_sf: &str, gm_sf: &str, model: &str, ir: &str, out_dir: &str, name: &str) {
    let out = Path::new(out_dir);
    std::fs::create_dir_all(out).unwrap();
    let (di, preset) = di_guitar(di_sf);
    let len = di.len();
    let di_file = out.join(format!("{name}-0-DI-dry.wav"));
    if !di_file.exists() {
        write16(&di_file, &di, &di);
    }

    // NeuralAmpModeler-rs: the model, then the cabinet.
    let sys = SystemSnapshot::capture();
    let mut pair = load_and_build_model(Path::new(model), &sys, false, LoadOptions::default()).unwrap();
    let (gin, gout) = (pair.input_mult_adj, pair.output_mult_adj);
    let kind = format!("{} {}", pair.architecture, pair.topology);
    let amp = pair.model_l.as_mut().unwrap();
    amp.prewarm(BLOCK * 64);
    let input: Vec<f32> = di.iter().map(|x| x * gin).collect();
    let mut a = vec![0.0f32; len];
    let (med_a, p99_a) = timed(|b| {
        let r = b * BLOCK..((b + 1) * BLOCK).min(len);
        amp.process(&input[r.clone()], &mut a[r]);
    });
    a.iter_mut().for_each(|x| *x *= gout);

    // nam-rs on the same input.
    let file = nam_rs::NamModel::from_file(model).unwrap();
    let mut other = nam_rs::Model::from_nam(&file).unwrap();
    let mut b = input.clone();
    let (med_b, p99_b) = timed(|k| {
        let r = k * BLOCK..((k + 1) * BLOCK).min(len);
        other.process_buffer(&mut b[r]);
    });
    b.iter_mut().for_each(|x| *x *= gout);
    let skip = other.receptive_field() + BLOCK * 64;
    let diff = a[skip..].iter().zip(&b[skip..]).fold(0.0f32, |m, (x, y)| m.max((x - y).abs()));

    // The cabinet, latency compensated.
    let cab = CabSimIr::load(Path::new(ir), RATE, true).unwrap();
    let conv = ConvEngine::new(&cab.samples, BLOCK).unwrap();
    let mut adapter = CabSimAdapter::new(Box::new(conv)).unwrap();
    let lat = adapter.latency_samples();
    let mut padded = a.clone();
    padded.extend(std::iter::repeat_n(0.0, lat + BLOCK));
    let mut wet = vec![0.0f32; padded.len()];
    for (i, o) in padded.chunks(BLOCK).zip(wet.chunks_mut(BLOCK)) {
        adapter.process_variable(i, o, None);
    }
    let mut guitar: Vec<f32> = wet[lat..lat + len].to_vec();

    let model_name = Path::new(model).file_stem().unwrap().to_string_lossy().replace(' ', "_");
    let ir_name = Path::new(ir).file_stem().unwrap().to_string_lossy().replace(' ', "_");
    let mut alone = guitar.clone();
    normalise(&mut alone);
    write16(&out.join(format!("{name}-{model_name}--cab-{ir_name}-guitar.wav")), &alone, &alone);

    // The mix: the guitar a little above the backing, centred.
    let (mut bl, mut br) = backing(gm_sf);
    let g = 1.4 * rms(&bl) / rms(&guitar);
    guitar.iter_mut().for_each(|x| *x *= g);
    for i in 0..len {
        bl[i] += guitar[i];
        br[i] += guitar[i];
    }
    let p = 0.891 / peak(&bl).max(peak(&br));
    bl.iter_mut().for_each(|x| *x *= p);
    br.iter_mut().for_each(|x| *x *= p);
    write16(&out.join(format!("{name}-{model_name}--cab-{ir_name}-mix.wav")), &bl, &br);

    println!(
        "{model_name} ({kind}), DI {preset}: NeuralAmpModeler-rs median {med_a:.0} µs, 99th percentile {p99_a:.0} µs per 64 samples; nam-rs {med_b:.0} / {p99_b:.0} µs; largest difference {diff:.1e}; cabinet {ir_name} ({} samples at {} Hz → {RATE}), latency {lat}",
        cab.samples.len(),
        cab.original_rate
    );
}
