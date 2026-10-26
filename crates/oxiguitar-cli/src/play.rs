// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli play FILE`: plays a score through the installed
//! instrument and audio-device plugins (M4 slice 2), from the terminal,
//! printing each bar as it is heard. Ctrl + C stops it. With `--wav OUT`
//! it plays into a WAV file instead (48 kHz, 16-bit stereo), as fast as it
//! can, through the same engine and instruments, and needs no sound card.
//!
//! Each track plays through the first installed instrument for its class;
//! the tracks of one instrument share one prepared instance. The audio
//! device is the one the plugin host chooses (settings.json's pin, else
//! the highest priority that works here: PipeWire before ALSA on Linux).

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use oxiguitar_model::Score;
use oxiguitar_play::plugin_source;
use oxiguitar_play::{Route, Schedule, Sources, Timeline, TrackMix, engine};

use crate::plugins::{open, plugins};

/// The engine's largest piece, in frames.
const CAPACITY: usize = 512;

/// What `play` was asked.
#[derive(Debug, Default, PartialEq)]
struct Asked {
    path: String,
    /// The bar to start from, from 1.
    from: Option<usize>,
    /// The bars to loop, from 1, both included.
    looped: Option<(usize, usize)>,
    /// The speed, in percent.
    speed: Option<u32>,
    count_in: bool,
    metronome: bool,
    /// The tracks to hear, from 1; all if empty.
    tracks: Vec<usize>,
    /// The output's name; the system's default if empty.
    device: String,
    /// A WAV file to play into instead of a device.
    wav: Option<String>,
}

fn parse(args: &[String]) -> Result<Asked, String> {
    let mut asked = Asked::default();
    let mut it = args.iter();
    let number = |v: Option<&String>, what: &str| -> Result<usize, String> {
        v.and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .ok_or_else(|| format!("{what} needs a number from 1"))
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => asked.from = Some(number(it.next(), "--from")?),
            "--loop" => {
                let v = it.next().ok_or("--loop needs bars, such as 5-8")?;
                let (a, b) = v.split_once('-').ok_or("--loop needs bars, such as 5-8")?;
                let (a, b) = (
                    number(Some(&a.to_owned()), "--loop")?,
                    number(Some(&b.to_owned()), "--loop")?,
                );
                if b < a {
                    return Err("--loop's last bar comes before its first".into());
                }
                asked.looped = Some((a, b));
            }
            "--speed" => {
                let s = number(it.next(), "--speed")?;
                if !(25..=200).contains(&s) {
                    return Err("--speed is a percentage from 25 to 200".into());
                }
                asked.speed = Some(s as u32);
            }
            "--device" => {
                asked.device = it.next().ok_or("--device needs an output's name")?.clone();
            }
            "--wav" => asked.wav = Some(it.next().ok_or("--wav needs a file's name")?.clone()),
            "--count-in" => asked.count_in = true,
            "--metronome" => asked.metronome = true,
            "--tracks" => {
                let v = it
                    .next()
                    .ok_or("--tracks needs track numbers, such as 1,3")?;
                for t in v.split(',') {
                    asked.tracks.push(number(Some(&t.to_owned()), "--tracks")?);
                }
            }
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            path if asked.path.is_empty() => asked.path = path.to_owned(),
            _ => return Err("play takes one file".into()),
        }
    }
    if asked.path.is_empty() {
        return Err("play needs a file".into());
    }
    if asked.wav.is_some() && asked.looped.is_some() {
        return Err("--wav and --loop don't go together: a loop never ends".into());
    }
    if asked.wav.is_some() && !asked.device.is_empty() {
        return Err("--wav plays into a file, not an output".into());
    }
    Ok(asked)
}

pub(crate) fn run(args: &[String]) -> ExitCode {
    let asked = match parse(args) {
        Ok(a) => a,
        Err(e) => return crate::usage_error(&e),
    };
    match play(&asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("oxiguitar-cli: {}: {e}", asked.path);
            ExitCode::FAILURE
        }
    }
}

/// What [`sources`] gets ready.
struct Ready {
    sources: Sources,
    routes: Vec<Option<Route>>,
    count: usize,
    names: Vec<String>,
}

/// The sources for `score` at `rate`: one prepared instance per
/// instrument, the tracks each plays, and the routes. Tracks nothing
/// plays are said and left silent.
fn sources(score: &Score, rate: u32) -> Result<Ready, String> {
    let plugins = plugins();
    let plan = plugin_source::plan(plugins, score);
    let p = plugin_source::prepare(plugins, score, &plan, rate, CAPACITY)?;
    for s in &p.silent {
        eprintln!("{s}");
    }
    Ok(Ready {
        sources: p.sources,
        routes: p.routes,
        count: p.count,
        names: p.names,
    })
}

fn play(asked: &Asked) -> Result<(), String> {
    let data = std::fs::read(&asked.path).map_err(|e| format!("cannot open: {e}"))?;
    let opened = open(&asked.path, &data)?;
    let score = opened.score;
    let timeline = Timeline::new(&score);
    let visits = timeline.visits();
    let start_of = |bar: usize| -> Result<f64, String> {
        visits
            .iter()
            .find(|v| v.visit.bar + 1 == bar)
            .map(|v| v.start)
            .ok_or_else(|| format!("it has no bar {bar}"))
    };
    let end_of = |bar: usize| -> Result<f64, String> {
        visits
            .iter()
            .find(|v| v.visit.bar + 1 == bar)
            .map(|v| v.end)
            .ok_or_else(|| format!("it has no bar {bar}"))
    };
    let from = match (asked.from, asked.looped) {
        (Some(b), _) => start_of(b)?,
        (None, Some((a, _))) => start_of(a)?,
        (None, None) => 0.0,
    };
    let looped = match asked.looped {
        Some((a, b)) => Some((start_of(a)?, end_of(b)?)),
        None => None,
    };
    let speed = f64::from(asked.speed.unwrap_or(100)) / 100.0;
    if let Some(wav) = &asked.wav {
        return play_into(asked, &score, &timeline, from, speed, wav);
    }

    // The device first, at 48 kHz if it takes it: the engine and the
    // instruments then run at the rate it opened at.
    let mut rate = oxiguitar_play::engine::RATE;
    let (stream, mut control, names) = loop {
        let (mut engine, control) = engine(rate, CAPACITY);
        let stream = plugins()
            .open_stream(rate, &asked.device, move |out, channels, delay_ns| {
                let delay = delay_ns.saturating_mul(u64::from(rate)) / 1_000_000_000;
                engine.render(out, channels, delay);
            })
            .map_err(|e| format!("no sound: {e}"))?;
        if stream.opened().rate == rate {
            let Ready {
                mut sources,
                routes,
                count,
                names,
            } = sources(&score, rate)?;
            let schedule = Arc::new(Schedule::new(&timeline, &routes, count));
            sources.fit(&schedule);
            let mut control = control;
            control.set_schedule(schedule);
            control.set_sources(sources);
            break (stream, control, names);
        }
        // The device runs at another rate: again, at its own.
        rate = stream.opened().rate;
        drop(stream);
    };
    mix(&mut control, &score, asked);
    control.set_speed(speed);
    control.set_loop(looped);
    control.set_metronome(asked.metronome);
    control.play(from, asked.count_in);
    println!(
        "Playing {} through {} ({}, {} Hz) with {}. Ctrl + C stops.",
        asked.path,
        stream.service(),
        stream.opened().device,
        stream.opened().rate,
        names.join(", ")
    );

    // Each bar as it is heard: sleep until its time comes, then say it.
    let mut shown: Option<usize> = None;
    loop {
        let done = control.collect();
        if done.ended {
            println!("The end.");
            break;
        }
        let Some(heard) = control.heard() else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let at = timeline.find(heard.max(from));
        if let Some((visit, _)) = at
            && shown != Some(visit)
        {
            shown = Some(visit);
            println!("Bar {}", visits[visit].visit.bar + 1);
        }
        // Until the next bar starts (or a little, during the count-in).
        let next = at
            .and_then(|(v, _)| visits.get(v + 1))
            .map_or(0.05, |n| ((n.start - heard) / speed).clamp(0.005, 1.0));
        std::thread::sleep(Duration::from_secs_f64(next));
    }
    // Let the last sound reach the speaker before closing.
    std::thread::sleep(Duration::from_millis(300));
    drop(stream);
    Ok(())
}

/// The mixer's settings for each track, with `--tracks` heard alone, and
/// the window's usual master level.
fn mix(control: &mut engine::Control, score: &Score, asked: &Asked) {
    control.set_master(engine::MASTER_USUAL);
    let solo = !asked.tracks.is_empty();
    for t in 0..score.tracks.len() {
        let m = &score.tracks[t].mix;
        control.set_mix(
            t,
            TrackMix {
                volume: m.volume,
                pan: m.pan,
                mute: m.mute,
                solo: if solo {
                    asked.tracks.contains(&(t + 1))
                } else {
                    m.solo
                },
            },
        );
    }
}

/// How long the sound goes on after the song's end, so the last notes'
/// release is heard: two seconds.
const TAIL_SECONDS: u32 = 2;

/// Plays the score into the WAV file `wav` at 48 kHz, through the engine
/// as a device would, as fast as it can.
fn play_into(
    asked: &Asked,
    score: &Score,
    timeline: &Timeline,
    from: f64,
    speed: f64,
    wav: &str,
) -> Result<(), String> {
    let rate = engine::RATE;
    let (mut engine, mut control) = engine(rate, CAPACITY);
    let Ready {
        mut sources,
        routes,
        count,
        names,
    } = sources(score, rate)?;
    let schedule = Arc::new(Schedule::new(timeline, &routes, count));
    sources.fit(&schedule);
    control.set_schedule(schedule);
    control.set_sources(sources);
    mix(&mut control, score, asked);
    control.set_speed(speed);
    control.set_metronome(asked.metronome);
    control.play(from, asked.count_in);
    // At most the song at this speed, the count-in and the tail: a song
    // that never says it ended still stops.
    let most = ((timeline.length() - from).max(0.0) / speed + 10.0) as u64 * u64::from(rate)
        + u64::from(TAIL_SECONDS * rate);
    let mut samples: Vec<i16> = Vec::new();
    let mut out = vec![0f32; CAPACITY * 2];
    let (mut frames, mut tail) = (0u64, None::<u64>);
    while frames < most && tail.is_none_or(|t| t > 0) {
        engine.render(&mut out, 2, 0);
        samples.extend(
            out.iter()
                .map(|v| (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16),
        );
        frames += CAPACITY as u64;
        if let Some(t) = &mut tail {
            *t = t.saturating_sub(CAPACITY as u64);
        } else if control.collect().ended {
            tail = Some(u64::from(TAIL_SECONDS * rate));
        }
    }
    std::fs::write(wav, wav_file(&samples, rate))
        .map_err(|e| format!("cannot write {wav}: {e}"))?;
    println!(
        "Played {} into {wav} ({:.1} s, {rate} Hz) with {}.",
        asked.path,
        frames as f64 / f64::from(rate),
        names.join(", ")
    );
    Ok(())
}

/// A WAV file of interleaved 16-bit stereo `samples` at `rate`.
fn wav_file(samples: &[i16], rate: u32) -> Vec<u8> {
    let bytes = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + bytes as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + bytes).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&2u16.to_le_bytes()); // stereo
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&bytes.to_le_bytes());
    for v in samples {
        w.extend_from_slice(&v.to_le_bytes());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_are_read() {
        assert_eq!(
            parse(&args(
                "song.gp --from 3 --loop 5-8 --speed 50 --count-in --metronome --tracks 1,3"
            )),
            Ok(Asked {
                path: "song.gp".into(),
                from: Some(3),
                looped: Some((5, 8)),
                speed: Some(50),
                count_in: true,
                metronome: true,
                tracks: vec![1, 3],
                device: String::new(),
                wav: None,
            })
        );
        assert_eq!(
            parse(&args("song.gp --wav out.wav")).map(|a| a.wav),
            Ok(Some("out.wav".into()))
        );
        assert!(parse(&args("song.gp --wav out.wav --loop 1-2")).is_err());
        assert!(parse(&args("song.gp --wav out.wav --device hw:0")).is_err());
        assert!(parse(&args("song.gp --speed 300")).is_err());
        assert!(parse(&args("song.gp --loop 8-5")).is_err());
        assert!(parse(&args("song.gp --from 0")).is_err());
        assert!(parse(&args("--metronome")).is_err());
        assert!(parse(&args("a.gp b.gp")).is_err());
    }
}
