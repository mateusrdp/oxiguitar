// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli play FILE`: plays a score through the installed
//! instrument and audio-device plugins (M4 slice 2), from the terminal,
//! printing each bar as it is heard. Ctrl + C stops it.
//!
//! Each track plays through the first installed instrument for its class;
//! the tracks of one instrument share one prepared instance. The audio
//! device is the one the plugin host chooses (settings.json's pin, else
//! the highest priority that works here: PipeWire before ALSA on Linux).

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use oxiguitar_host::api::class::InstrumentClass;
use oxiguitar_host::api::message::{Prepare, PreparedTrack, TrackInfo};
use oxiguitar_model::Score;
use oxiguitar_play::plugin_source::PluginSource;
use oxiguitar_play::{Route, Schedule, Slot, Sources, Timeline, TrackMix, engine};

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

/// The class of a track's instrument.
fn class_of(score: &Score, track: usize) -> InstrumentClass {
    let i = score.tracks[track].instrument;
    InstrumentClass::of(i.midi_program, i.percussion)
}

/// What [`sources`] gets ready.
struct Ready {
    /// One prepared instance per instrument.
    sources: Sources,
    /// Each track's source and slot, or `None` when nothing plays it.
    routes: Vec<Option<Route>>,
    /// The instruments' names.
    names: Vec<String>,
}

/// The sources for `score` at `rate`: one prepared instance per
/// instrument, the tracks each plays, and the routes. Tracks no
/// instrument plays are said and left silent.
fn sources(score: &Score, rate: u32) -> Result<Ready, String> {
    let plugins = plugins();
    // The instrument for each track: the first that plays its class.
    let chosen: Vec<Option<(String, String)>> = (0..score.tracks.len())
        .map(|t| {
            plugins.playable(class_of(score, t)).first().map(|i| {
                (
                    i.plugin
                        .manifest
                        .as_ref()
                        .map(|m| m.id.clone())
                        .unwrap_or_default(),
                    i.spec.id.clone(),
                )
            })
        })
        .collect();
    let mut groups: Vec<((String, String), Vec<usize>)> = Vec::new();
    for (t, c) in chosen.iter().enumerate() {
        if let Some(key) = c {
            match groups.iter_mut().find(|(k, _)| k == key) {
                Some((_, tracks)) => tracks.push(t),
                None => groups.push((key.clone(), vec![t])),
            }
        }
    }
    if groups.is_empty() {
        return Err("no installed instrument plays its tracks".into());
    }
    let mut routes = vec![None; score.tracks.len()];
    let mut slots = Vec::new();
    let mut names = Vec::new();
    for (source, ((plugin, id), tracks)) in groups.into_iter().enumerate() {
        let class = class_of(score, tracks[0]);
        let instrument = plugins
            .playable(class)
            .into_iter()
            .find(|i| i.spec.id == id && i.plugin.manifest.as_ref().is_some_and(|m| m.id == plugin))
            .ok_or("an instrument went away")?;
        names.push(instrument.spec.name.clone());
        let prepared: Vec<PreparedTrack> = tracks
            .iter()
            .map(|&t| {
                let track = &score.tracks[t];
                let staff = track.staves.first();
                PreparedTrack {
                    info: TrackInfo {
                        index: t as u32,
                        name: track.name.clone(),
                        class: class_of(score, t),
                    },
                    program: track.instrument.midi_program,
                    percussion: track.instrument.percussion,
                    tuning: staff
                        .map(|s| s.tuning.iter().map(|p| p.0).collect())
                        .unwrap_or_default(),
                    capo: staff.map_or(0, |s| s.capo),
                    settings: track
                        .plugins
                        .get(&plugin)
                        .cloned()
                        .unwrap_or_else(|| "{}".into()),
                }
            })
            .collect();
        let instance = plugins
            .prepare(
                instrument,
                Prepare {
                    instrument: id.clone(),
                    rate,
                    frames: CAPACITY as u32,
                    tracks: prepared,
                },
            )
            .map_err(|e| format!("{} couldn't get ready: {e}", instrument.spec.name))?;
        for (slot, &t) in tracks.iter().enumerate() {
            routes[t] = Some(Route { source, slot });
        }
        let effects = tracks.iter().map(|_| Vec::new()).collect();
        slots.push(Slot::new(
            Box::new(PluginSource::new(instance, effects)),
            tracks,
            CAPACITY,
        ));
    }
    Ok(Ready {
        sources: Sources::new(slots),
        routes,
        names,
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
                names,
            } = sources(&score, rate)?;
            let schedule = Arc::new(Schedule::new(&timeline, &routes, sources_count(&routes)));
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

/// The number of sources the routes name.
fn sources_count(routes: &[Option<Route>]) -> usize {
    routes
        .iter()
        .flatten()
        .map(|r| r.source + 1)
        .max()
        .unwrap_or(0)
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
            })
        );
        assert!(parse(&args("song.gp --speed 300")).is_err());
        assert!(parse(&args("song.gp --loop 8-5")).is_err());
        assert!(parse(&args("song.gp --from 0")).is_err());
        assert!(parse(&args("--metronome")).is_err());
        assert!(parse(&args("a.gp b.gp")).is_err());
    }
}
