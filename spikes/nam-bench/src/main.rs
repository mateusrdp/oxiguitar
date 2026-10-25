// SPDX-License-Identifier: GPL-3.0-or-later
//! nam-bench MODEL: the NAM engines on the same mono 48 kHz input (ten
//! seconds of plucked strings, made here), in 64-sample blocks: speed, worst
//! block, and how far apart the outputs are.

use std::time::Instant;

/// Ten seconds of plucked notes (Karplus–Strong), a DI-like guitar signal.
fn plucks() -> Vec<f32> {
    let rate = 48_000usize;
    let mut out = vec![0.0f32; rate * 10];
    let mut seed = 12345u32;
    let mut noise = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 9) as f32 / (1u32 << 23) as f32 - 1.0
    };
    let notes = [82.4f32, 110.0, 146.8, 196.0, 246.9, 329.6, 196.0, 110.0];
    for (n, f) in notes.iter().cycle().take(40).enumerate() {
        let start = n * rate / 4;
        let period = (rate as f32 / f) as usize;
        let mut line: Vec<f32> = (0..period).map(|_| noise() * 0.5).collect();
        for i in 0..rate {
            let Some(o) = out.get_mut(start + i) else { break };
            let j = i % period;
            let v = line[j];
            line[j] = 0.996 * 0.5 * (v + line[(j + 1) % period]);
            *o += v;
        }
    }
    out
}

fn time(name: &str, x: &[f32], mut f: impl FnMut(&[f32], &mut [f32])) -> Vec<f32> {
    const B: usize = 64;
    let mut y = vec![0.0; x.len()];
    // Warm-up pass, then the timed pass.
    for (i, o) in x.chunks(B).zip(y.chunks_mut(B)) {
        f(i, o);
    }
    let mut worst = 0.0f64;
    let mut times = Vec::new();
    let t0 = Instant::now();
    for (i, o) in x.chunks(B).zip(y.chunks_mut(B)) {
        let t = Instant::now();
        f(i, o);
        let us = t.elapsed().as_secs_f64() * 1e6;
        worst = worst.max(us);
        times.push(us);
    }
    let total = t0.elapsed().as_secs_f64();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{name}: {:.1}× real time; 64-sample block median {:.1} µs, 99th percentile {:.1} µs, worst {:.0} µs (budget 1333)",
        x.len() as f64 / 48000.0 / total,
        times[times.len() / 2],
        times[times.len() * 99 / 100],
        worst
    );
    y
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = &args[1];
    let x = plucks();

    let file = nam_rs::NamModel::from_file(model).unwrap();
    let mut a = nam_rs::Model::from_nam(&file).unwrap();
    let ya = time("nam-rs 0.4.0", &x, |i, o| {
        o.copy_from_slice(i);
        a.process_buffer(o);
    });

    #[cfg(feature = "nam2")]
    {
        use neural_amp_modeler_rs::common::diagnostics::SystemSnapshot;
        use neural_amp_modeler_rs::loader::{LoadOptions, load_and_build_model};
        use neural_amp_modeler_rs::models::NamModel as _;
        let sys = SystemSnapshot::capture();
        let mut pair = load_and_build_model(std::path::Path::new(model), &sys, false, LoadOptions::default()).unwrap();
        println!("NeuralAmpModeler-rs: {} {}", pair.architecture, pair.topology);
        let m = pair.model_l.as_mut().unwrap();
        let yb = time("NeuralAmpModeler-rs 1.0.1", &x, |i, o| m.process(i, o));
        let n = ya.len().min(yb.len());
        let peak = ya.iter().fold(0.0f32, |p, v| p.max(v.abs()));
        let diff = ya[..n].iter().zip(&yb[..n]).fold(0.0f32, |p, (u, v)| p.max((u - v).abs()));
        println!("largest difference between the two outputs: {diff:.2e} (output peak {peak:.3})");
    }
}
