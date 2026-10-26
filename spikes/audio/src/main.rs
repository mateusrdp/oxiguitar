// SPDX-License-Identifier: GPL-3.0-or-later
//! The audio spike (M4): does the chain events → SoundFont synthesiser →
//! NAM → audio device fit together, in real time, without allocating on the
//! audio thread, and with sample-accurate timing? See README.md.

mod alloc;
#[cfg(feature = "nam2")]
mod amp;
mod engines;
mod sf3;
mod song;

use engines::{Engine, Oxi, Rusty};
use song::{Event, RATE, TRACKS};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[global_allocator]
static GLOBAL: alloc::Counting = alloc::Counting;

pub(crate) fn load_sf2_bytes(path: &str) -> Vec<u8> {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let t = Instant::now();
    let c = sf3::to_sf2(&raw).unwrap();
    if c.compressed > 0 {
        eprintln!(
            "{path}: SF3 → SF2 in {:.0} ms ({} of {} samples decoded; {:.1} MB → {:.1} MB)",
            t.elapsed().as_secs_f64() * 1e3,
            c.compressed,
            c.samples,
            raw.len() as f64 / 1e6,
            c.sf2.len() as f64 / 1e6
        );
    }
    c.sf2
}

fn engine(kind: &str, sf: &[u8], rusty_font: &mut Option<Arc<rustysynth::SoundFont>>) -> Box<dyn Engine> {
    if let Some(block) = kind.strip_prefix("rusty") {
        let block: usize = block.trim_start_matches('/').parse().unwrap_or(64);
        let font = rusty_font
            .get_or_insert_with(|| Arc::new(rustysynth::SoundFont::new(&mut std::io::Cursor::new(sf)).unwrap()))
            .clone();
        Box::new(Rusty::new(&font, block))
    } else {
        Box::new(Oxi::new(sf))
    }
}

/// Renders every track in its own engine, splitting each render call at the
/// events' samples (sample-accurate as far as the engine allows), in
/// callbacks of `callback` samples. Returns each track's stereo buffer, the
/// worst callback time, the total render time and the allocations counted
/// during the timed part (events and renders after the first second).
pub(crate) struct Stems {
    pub(crate) tracks: Vec<(Vec<f32>, Vec<f32>)>,
    worst_callback_us: f64,
    total_s: f64,
    allocations: usize,
}

pub(crate) fn render_song(engines: &mut [Box<dyn Engine>], events: &[Event], callback: usize) -> Stems {
    let len = song::length();
    let mut tracks: Vec<(Vec<f32>, Vec<f32>)> = (0..engines.len()).map(|_| (vec![0.0; len], vec![0.0; len])).collect();
    let mut next = 0;
    let mut worst = 0.0f64;
    let mut allocations = 0;
    let started = Instant::now();
    let mut pos = 0;
    while pos < len {
        let end = (pos + callback).min(len);
        let t = Instant::now();
        let (_, n) = alloc::count(|| {
            let mut at = pos;
            while at < end {
                // Render every track up to the next event (or the callback's end).
                let stop = if next < events.len() && (events[next].at as usize) < end {
                    (events[next].at as usize).max(at)
                } else {
                    end
                };
                if stop > at {
                    for (e, (l, r)) in engines.iter_mut().zip(tracks.iter_mut()) {
                        e.render(&mut l[at..stop], &mut r[at..stop]);
                    }
                    at = stop;
                }
                while next < events.len() && events[next].at as usize == at {
                    let ev = events[next];
                    engines[ev.track].send(ev.channel, ev.msg);
                    next += 1;
                }
            }
        });
        let us = t.elapsed().as_secs_f64() * 1e6;
        if pos >= RATE as usize {
            worst = worst.max(us);
            allocations += n;
        }
        pos = end;
    }
    Stems { tracks, worst_callback_us: worst, total_s: started.elapsed().as_secs_f64(), allocations }
}

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec { channels: 2, sample_rate: RATE, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample(*l).unwrap();
        w.write_sample(*r).unwrap();
    }
    w.finalize().unwrap();
}

fn read_wav_mono(path: &str) -> Vec<f32> {
    let mut r = hound::WavReader::open(path).unwrap();
    let spec = r.spec();
    let ch = spec.channels as usize;
    let s: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|x| x.unwrap() as f32 / scale).collect()
        }
    };
    assert_eq!(spec.sample_rate, RATE, "{path} is not at {RATE} Hz");
    s.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect()
}

pub(crate) fn mix(tracks: &[(Vec<f32>, Vec<f32>)]) -> (Vec<f32>, Vec<f32>) {
    let len = tracks[0].0.len();
    let mut l = vec![0.0; len];
    let mut r = vec![0.0; len];
    for (tl, tr) in tracks {
        for i in 0..len {
            l[i] += tl[i];
            r[i] += tr[i];
        }
    }
    (l, r)
}

/// RMS in dB per 10 ms frame.
fn envelope(x: &[f32]) -> Vec<f64> {
    x.chunks(480)
        .map(|c| {
            let ms = c.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / c.len() as f64;
            10.0 * (ms + 1e-12).log10()
        })
        .collect()
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let (a, b) = (&a[..n], &b[..n]);
    let ma = a.iter().sum::<f64>() / n as f64;
    let mb = b.iter().sum::<f64>() / n as f64;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        sab += (a[i] - ma) * (b[i] - mb);
        saa += (a[i] - ma).powi(2);
        sbb += (b[i] - mb).powi(2);
    }
    sab / (saa * sbb).sqrt()
}

/// Average magnitude spectrum in 48 log-spaced bands (40 Hz–16 kHz), in dB,
/// normalised to its mean, from 2048-sample frames with a Hann window.
fn spectrum(x: &[f32]) -> Vec<f64> {
    const N: usize = 2048;
    let bands = 48;
    let edges: Vec<f64> = (0..=bands).map(|i| 40.0 * (16000.0f64 / 40.0).powf(i as f64 / bands as f64)).collect();
    let mut energy = vec![0.0f64; bands];
    let window: Vec<f64> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / N as f64).cos()).collect();
    // Frames every 0.25 s; a direct DFT on the bins we need (k up to 16 kHz).
    let kmax = (16000.0 * N as f64 / RATE as f64) as usize;
    let mut frame = 0;
    while frame + N <= x.len() {
        for k in 1..kmax {
            let f = k as f64 * RATE as f64 / N as f64;
            let Some(b) = edges.windows(2).position(|w| f >= w[0] && f < w[1]) else { continue };
            let (mut re, mut im) = (0.0, 0.0);
            let w = 2.0 * std::f64::consts::PI * k as f64 / N as f64;
            for n in 0..N {
                let v = x[frame + n] as f64 * window[n];
                re += v * (w * n as f64).cos();
                im -= v * (w * n as f64).sin();
            }
            energy[b] += re * re + im * im;
        }
        frame += RATE as usize / 4;
    }
    let db: Vec<f64> = energy.iter().map(|e| 10.0 * (e + 1e-12).log10()).collect();
    let mean = db.iter().sum::<f64>() / db.len() as f64;
    db.iter().map(|d| d - mean).collect()
}

fn compare(a: &str, b: &str) {
    let (x, y) = (read_wav_mono(a), read_wav_mono(b));
    let n = x.len().min(y.len());
    let (x, y) = (&x[..n], &y[..n]);
    let env = correlation(&envelope(x), &envelope(y));
    let (sx, sy) = (spectrum(x), spectrum(y));
    let lsd = (sx.iter().zip(&sy).map(|(p, q)| (p - q).powi(2)).sum::<f64>() / sx.len() as f64).sqrt();
    let worst = sx.iter().zip(&sy).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max);
    println!("{a} vs {b}: loudness envelope correlation {env:.4}; spectral shape distance {lsd:.2} dB RMS (worst band {worst:.1} dB)");
}

/// Where a lone note's sound starts, for a note-on asked for at `offset`
/// samples into the render, compared with one asked for at 0.
fn onset_error(kind: &str, sf: &[u8], offset: usize) -> isize {
    let start = |off: usize| {
        let mut font = None;
        let mut e = engine(kind, sf, &mut font);
        e.send(0, song::Msg::Program(80)); // square lead: starts at once
        let mut l = vec![0.0; 4096];
        let mut r = vec![0.0; 4096];
        e.render(&mut l[..1024], &mut r[..1024]); // settle, then the note at 1024 + off
        e.render(&mut l[1024..1024 + off], &mut r[1024..1024 + off]);
        e.send(0, song::Msg::NoteOn(69, 127));
        e.render(&mut l[1024 + off..], &mut r[1024 + off..]);
        l.iter().position(|v| v.abs() > 1e-5).unwrap_or(usize::MAX)
    };
    start(offset) as isize - start(0) as isize - offset as isize
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["midi", out] => {
            std::fs::write(out, song::midi_file(&song::events())).unwrap();
            println!("wrote {out}");
        }
        ["render", kind, sf, out] => {
            let bytes = load_sf2_bytes(sf);
            let mut font = None;
            let t = Instant::now();
            let mut engines: Vec<Box<dyn Engine>> = (0..TRACKS.len()).map(|_| engine(kind, &bytes, &mut font)).collect();
            let load_ms = t.elapsed().as_secs_f64() * 1e3;
            let name = engines[0].name();
            let stems = render_song(&mut engines, &song::events(), 64);
            let (l, r) = mix(&stems.tracks);
            write_wav(Path::new(out), &l, &r);
            // The guitar alone, mono, for the NAM step.
            let gl: Vec<f32> = stems.tracks[0].0.iter().zip(&stems.tracks[0].1).map(|(a, b)| (a + b) * 0.5).collect();
            write_wav(&Path::new(out).with_extension("guitar.wav"), &gl, &gl);
            let audio_s = song::length() as f64 / RATE as f64;
            println!(
                "{name} {sf}: load {load_ms:.0} ms; {audio_s:.1} s of 3 tracks rendered in {:.3} s ({:.0}× real time); worst 64-sample callback {:.0} µs of 1333; {} allocations after the first second",
                stems.total_s,
                audio_s / stems.total_s,
                stems.worst_callback_us,
                stems.allocations
            );
        }
        ["compare", x, y] => compare(x, y),
        ["timing", kind, sf] => {
            let bytes = load_sf2_bytes(sf);
            let errs: Vec<isize> = [1usize, 7, 13, 31, 50, 63, 100, 129].iter().map(|o| onset_error(kind, &bytes, *o)).collect();
            println!("{kind}: onset error (samples late, for note-ons 1..129 samples into a render): {errs:?}");
        }
        ["nam", model, input, out] => nam(model, input, out),
        ["pipeline", sf, model, out] => pipeline(sf, model, out),
        #[cfg(feature = "nam2")]
        ["amp", di, gm, model, ir, dir, name] => amp::run(di, gm, model, ir, dir, name),
        ["dls", path] => dls(path),
        #[cfg(feature = "devices")]
        ["devices"] => devices(),
        #[cfg(feature = "devices")]
        ["stream", sf] => stream(sf),
        _ => eprintln!(
            "usage: audio-spike midi OUT.mid | render rusty/N|oxi SF OUT.wav | compare A.wav B.wav | timing rusty/N|oxi SF | nam MODEL IN.wav OUT.wav | pipeline SF MODEL OUT.wav | dls FILE | devices"
        ),
    }
}

fn nam(model: &str, input: &str, out: &str) {
    let x = read_wav_mono(input);
    let t = Instant::now();
    let file = nam_rs::NamModel::from_file(model).unwrap();
    let mut amp = nam_rs::Model::from_nam(&file).unwrap();
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "{model}: rate {} Hz, receptive field {} samples, loaded in {load_ms:.1} ms",
        file.expected_sample_rate(),
        amp.receptive_field()
    );
    for block in [64usize, 128, 256] {
        amp.reset();
        let mut y = x.clone();
        let mut worst = 0.0f64;
        let mut allocations = 0;
        let started = Instant::now();
        for (i, chunk) in y.chunks_mut(block).enumerate() {
            let t = Instant::now();
            let (_, n) = alloc::count(|| amp.process_buffer(chunk));
            if i > 10 {
                worst = worst.max(t.elapsed().as_secs_f64() * 1e6);
                allocations += n;
            }
        }
        let total = started.elapsed().as_secs_f64();
        let audio = x.len() as f64 / RATE as f64;
        println!(
            "  block {block}: {:.1}× real time; worst block {:.0} µs of {:.0}; {} allocations",
            audio / total,
            worst,
            block as f64 / RATE as f64 * 1e6,
            allocations
        );
        if block == 64 {
            let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-9);
            let y: Vec<f32> = y.iter().map(|v| v / peak * 0.5).collect();
            write_wav(Path::new(out), &y, &y);
        }
    }
}

/// The whole chain as the audio thread would run it, in 64-sample
/// callbacks: three tracks rendered (rustysynth), the guitar through NAM,
/// mixed, written interleaved as a device would take it.
fn pipeline(sf: &str, model: &str, out: &str) {
    let bytes = load_sf2_bytes(sf);
    let font = Arc::new(rustysynth::SoundFont::new(&mut std::io::Cursor::new(&bytes)).unwrap());
    let mut engines: Vec<Rusty> = (0..TRACKS.len()).map(|_| Rusty::new(&font, 64)).collect();
    let file = nam_rs::NamModel::from_file(model).unwrap();
    let mut amp = nam_rs::Model::from_nam(&file).unwrap();
    let events = song::events();
    const CB: usize = 64;
    let len = song::length();
    // Buffers allocated once, before the "audio thread" starts.
    let mut bufs = vec![([0.0f32; CB], [0.0f32; CB]); TRACKS.len()];
    let mut mono = [0.0f32; CB];
    let mut device = vec![0.0f32; len * 2]; // what the device would be handed, interleaved
    let mut next = 0;
    let mut worst = 0.0f64;
    let mut allocations = 0;
    let mut times = Vec::with_capacity(len / CB + 1);
    let mut pos = 0;
    while pos < len {
        let n = CB.min(len - pos);
        let t = Instant::now();
        let (_, a) = alloc::count(|| {
            let mut at = 0;
            while at < n {
                let stop = if next < events.len() && (events[next].at as usize) < pos + n {
                    (events[next].at as usize - pos).max(at)
                } else {
                    n
                };
                for (e, (l, r)) in engines.iter_mut().zip(bufs.iter_mut()) {
                    e.render(&mut l[at..stop], &mut r[at..stop]);
                }
                at = stop;
                while next < events.len() && events[next].at as usize == pos + at {
                    engines[events[next].track].send(events[next].channel, events[next].msg);
                    next += 1;
                }
            }
            for i in 0..n {
                mono[i] = (bufs[0].0[i] + bufs[0].1[i]) * 0.5;
            }
            amp.process_buffer(&mut mono[..n]);
            for i in 0..n {
                let g = mono[i] * 0.25; // the amp's output, gain-staged
                device[(pos + i) * 2] = g + bufs[1].0[i] + bufs[2].0[i];
                device[(pos + i) * 2 + 1] = g + bufs[1].1[i] + bufs[2].1[i];
            }
        });
        let us = t.elapsed().as_secs_f64() * 1e6;
        if pos >= RATE as usize {
            worst = worst.max(us);
            allocations += a;
            times.push(us);
        }
        pos += n;
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "pipeline (3 tracks rustysynth/64 + NAM {model} on the guitar), 64-sample callbacks (1333 µs each at 48 kHz): median {:.0} µs, 99th percentile {:.0} µs, worst {:.0} µs; {} allocations",
        p(0.5),
        p(0.99),
        worst,
        allocations
    );
    let (l, r): (Vec<f32>, Vec<f32>) = device.chunks(2).map(|c| (c[0], c[1])).unzip();
    write_wav(Path::new(out), &l, &r);
}

#[cfg(feature = "devices")]
fn devices() {
    use cpal::traits::{DeviceTrait, HostTrait};
    for id in cpal::available_hosts() {
        let host = cpal::host_from_id(id).unwrap();
        println!("host {:?}", id);
        if let Ok(devs) = host.output_devices() {
            for d in devs {
                let name = d.description().map(|x| x.to_string()).unwrap_or_default();
                let cfg = d.default_output_config().map(|c| format!("{:?}", c)).unwrap_or_else(|e| e.to_string());
                println!("  {name}: {cfg}");
            }
        }
    }
}

/// The outline of a DLS file (the sound sets Windows and macOS ship):
/// instruments, regions and waves, to check they are there and readable.
fn dls(path: &str) {
    let b = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            println!("{path}: {e}");
            return;
        }
    };
    let u32le = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"DLS " {
        println!("{path}: not a DLS file");
        return;
    }
    // Walk every chunk, counting instruments, regions and waves.
    let (mut ins, mut rgn, mut wave, mut art) = (0, 0, 0, 0);
    let mut stack = vec![(12usize, b.len().min(8 + u32le(4)))];
    while let Some((mut at, end)) = stack.pop() {
        while at + 8 <= end {
            let id = &b[at..at + 4];
            let size = u32le(at + 4);
            if id == b"LIST" && at + 12 <= end {
                match &b[at + 8..at + 12] {
                    b"ins " => ins += 1,
                    b"rgn " | b"rgn2" => rgn += 1,
                    b"wave" => wave += 1,
                    b"lart" | b"lar2" => art += 1,
                    _ => {}
                }
                stack.push((at + 12, (at + 8 + size).min(end)));
            }
            at += 8 + size + (size & 1);
        }
    }
    println!("{path}: DLS, {:.1} MB, {ins} instruments, {rgn} regions, {art} articulation lists, {wave} waves", b.len() as f64 / 1e6);
}

/// Plays the song through the default output device for three seconds,
/// rendering inside the device's callback, and reports what the callback
/// saw: buffer sizes, the delay from callback to playback (what the cursor
/// must allow for), render time and allocations.
#[cfg(feature = "devices")]
fn stream(sf: &str) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    const N: usize = 4096;
    let host = cpal::default_host();
    // The default device, or else the first that has a configuration (the
    // ALSA "null" device in a container without sound cards).
    let device = host
        .default_output_device()
        .filter(|d| d.default_output_config().is_ok())
        .or_else(|| host.output_devices().ok()?.find(|d| d.default_output_config().is_ok()))
        .expect("no output device");
    let config = device.default_output_config().unwrap();
    println!("{:?}: {:?}", host.id(), config);
    let rate = config.sample_rate() as usize;
    let channels = config.channels() as usize;
    let bytes = load_sf2_bytes(sf);
    let font = Arc::new(rustysynth::SoundFont::new(&mut std::io::Cursor::new(&bytes)).unwrap());
    let mut engines: Vec<Rusty> = (0..TRACKS.len()).map(|_| Rusty::new(&font, 8)).collect();
    let events = song::events();
    let stats: Arc<Vec<[AtomicU64; 4]>> = Arc::new((0..N).map(|_| Default::default()).collect());
    let calls = Arc::new(AtomicU64::new(0));
    let (st, ca) = (stats.clone(), calls.clone());
    let mut l = vec![0.0f32; 1 << 16];
    let mut r = vec![0.0f32; 1 << 16];
    let mut pos = 0usize;
    let mut next = 0usize;
    let s = device
        .build_output_stream(
            config.config(),
            move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let t = Instant::now();
                let frames = out.len() / channels;
                let (_, allocs) = alloc::count(|| {
                    // The song is at 48 kHz; at another rate it plays at another speed (a spike).
                    let mut at = 0;
                    while at < frames {
                        let stop = if next < events.len() && events[next].at as usize >= pos + at && (events[next].at as usize) < pos + frames {
                            events[next].at as usize - pos
                        } else {
                            frames
                        };
                        out.iter_mut().for_each(|_| {});
                        for e in engines.iter_mut() {
                            e.render(&mut l[at..stop], &mut r[at..stop]);
                        }
                        at = stop;
                        while next < events.len() && events[next].at as usize <= pos + at {
                            engines[events[next].track].send(events[next].channel, events[next].msg);
                            next += 1;
                        }
                    }
                    for (i, frame) in out.chunks_mut(channels).enumerate() {
                        frame[0] = l[i];
                        if channels > 1 {
                            frame[1] = r[i];
                        }
                    }
                });
                pos += frames;
                let ts = info.timestamp();
                let ahead = ts.playback.duration_since(ts.callback).as_micros() as u64;
                let i = ca.fetch_add(1, Relaxed) as usize;
                if i < N {
                    st[i][0].store(frames as u64, Relaxed);
                    st[i][1].store(ahead, Relaxed);
                    st[i][2].store(t.elapsed().as_micros() as u64, Relaxed);
                    st[i][3].store(allocs as u64, Relaxed);
                }
            },
            |e| eprintln!("stream error: {e}"),
            None,
        )
        .unwrap();
    s.play().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    drop(s);
    let n = (calls.load(Relaxed) as usize).min(N);
    let col = |k: usize| -> Vec<u64> { (0..n).map(|i| stats[i][k].load(Relaxed)).collect() };
    let (frames, ahead, us, allocs) = (col(0), col(1), col(2), col(3));
    println!(
        "{n} callbacks at {rate} Hz: frames {}..{}; callback-to-playback {}..{} µs; render worst {} µs; allocations {}",
        frames.iter().min().unwrap_or(&0),
        frames.iter().max().unwrap_or(&0),
        ahead.iter().min().unwrap_or(&0),
        ahead.iter().max().unwrap_or(&0),
        us.iter().skip(5).max().unwrap_or(&0),
        allocs.iter().skip(5).sum::<u64>()
    );
}
