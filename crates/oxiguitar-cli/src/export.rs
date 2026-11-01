// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli export FILE OUT`: File → Export of an audio exporter
//! plugin (plugin interface 1.4), from the command line. The exporter is
//! the one whose extension the new file has (`.wav`); the score plays as
//! the window's export dialog would make it play: the whole song or some
//! bars, as written or at a speed, with or without the metronome, every
//! track with its sound, amp and mixer settings (the file's own, and its
//! `FILE.oxg` beside it).

use std::path::Path;
use std::process::ExitCode;

use oxiguitar_play::Timeline;
use oxiguitar_play::export::{self, Plan};

use crate::plugins::{open, plugins};

#[derive(Debug, Default, PartialEq)]
struct Asked {
    path: String,
    out: String,
    bars: Option<(usize, usize)>,
    speed: Option<u32>,
    metronome: bool,
}

fn parse(args: &[String]) -> Result<Asked, String> {
    let mut asked = Asked::default();
    let mut it = args.iter();
    let number = |v: &str, what: &str| -> Result<usize, String> {
        v.parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| format!("{what} needs a number from 1"))
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--bars" => {
                let v = it.next().ok_or("--bars needs bars, such as 5-8")?;
                let (a, b) = v.split_once('-').unwrap_or((v, v));
                let (a, b) = (number(a, "--bars")?, number(b, "--bars")?);
                if b < a {
                    return Err("--bars's last bar comes before its first".into());
                }
                asked.bars = Some((a, b));
            }
            "--speed" => {
                let s = number(it.next().map_or("", String::as_str), "--speed")?;
                if !(25..=200).contains(&s) {
                    return Err("--speed is a percentage from 25 to 200".into());
                }
                asked.speed = Some(s as u32);
            }
            "--metronome" => asked.metronome = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            path if asked.path.is_empty() => asked.path = path.to_owned(),
            out if asked.out.is_empty() => asked.out = out.to_owned(),
            _ => return Err("export takes a file and the new file".into()),
        }
    }
    if asked.out.is_empty() {
        return Err("export needs a file and the new file".into());
    }
    Ok(asked)
}

pub(crate) fn run(args: &[String]) -> ExitCode {
    let asked = match parse(args) {
        Ok(a) => a,
        Err(e) => return crate::usage_error(&e),
    };
    match export_file(&asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("oxiguitar-cli: {}: {e}", asked.path);
            ExitCode::FAILURE
        }
    }
}

fn export_file(asked: &Asked) -> Result<(), String> {
    let out = Path::new(&asked.out);
    let extension = out
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .ok_or_else(|| {
            format!(
                "{}: the new file needs an extension, such as .wav",
                asked.out
            )
        })?;
    let plugins = plugins();
    let exporters = plugins.exporters();
    let exporter = exporters
        .iter()
        .find(|e| e.spec.extension == extension)
        .copied()
        .ok_or_else(|| {
            let known: Vec<String> = exporters
                .iter()
                .map(|e| format!(".{} ({})", e.spec.extension, e.spec.name))
                .collect();
            if known.is_empty() {
                format!("no installed plugin exports .{extension} files, nor any audio")
            } else {
                format!(
                    "no installed plugin exports .{extension} files; they export {}",
                    known.join(", ")
                )
            }
        })?;

    let data = std::fs::read(&asked.path).map_err(|e| format!("cannot open: {e}"))?;
    let mut score = open(&asked.path, &data)?.score;
    // The tracks' plugin settings (NAM's amp, say) live beside the file.
    if let Some(sidecar) = oxiguitar_host::sidecar::Sidecar::read_for(Path::new(&asked.path))? {
        let names: Vec<&str> = score.tracks.iter().map(|t| t.name.as_str()).collect();
        let settled = sidecar.settle(&names);
        for (track, plugins) in score.tracks.iter_mut().zip(settled) {
            track.plugins = plugins;
        }
    }
    let timeline = Timeline::new(&score);
    let mut plan = Plan::whole(&score);
    if let Some((a, b)) = asked.bars {
        let visits = timeline.visits();
        let find = |bar: usize| {
            visits
                .iter()
                .find(|v| v.visit.bar + 1 == bar)
                .ok_or_else(|| format!("it has no bar {bar}"))
        };
        plan.from = find(a)?.start;
        plan.until = Some(find(b)?.end);
    }
    plan.speed = f64::from(asked.speed.unwrap_or(100)) / 100.0;
    plan.metronome = asked.metronome;
    plan.master = oxiguitar_play::engine::MASTER_USUAL;

    let made = export::export(plugins, exporter, &score, &plan, out, &mut |_| true)
        .map_err(|e| e.to_string())?;
    for s in &made.silent {
        eprintln!("{s}");
    }
    println!(
        "Exported {} into {} ({:.1} s, {} Hz) through {}, with {}.",
        asked.path,
        asked.out,
        made.seconds,
        made.rate,
        exporter.spec.name,
        made.names.join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options() {
        assert_eq!(
            parse(&args("song.gp out.wav --bars 5-8 --speed 50 --metronome")),
            Ok(Asked {
                path: "song.gp".into(),
                out: "out.wav".into(),
                bars: Some((5, 8)),
                speed: Some(50),
                metronome: true,
            })
        );
        assert_eq!(
            parse(&args("song.gp out.wav --bars 3")).map(|a| a.bars),
            Ok(Some((3, 3)))
        );
        assert!(parse(&args("song.gp")).is_err());
        assert!(parse(&args("song.gp out.wav --bars 8-5")).is_err());
        assert!(parse(&args("song.gp out.wav --speed 300")).is_err());
        assert!(parse(&args("song.gp out.wav extra")).is_err());
    }
}
