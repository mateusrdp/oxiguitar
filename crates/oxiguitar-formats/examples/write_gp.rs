// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writes every file a reader takes, from the folders given, as a .gp file
//! in the last folder given: `NAME.EXT` becomes `NAME.EXT.gp`, beside
//! `NAME.EXT.notes`, the notes our reader finds in the file we wrote, in
//! the form `scripts/interop/alphatab.mjs` and `musescore.py` compare
//! other programs' reading of the same file with, and `NAME.EXT.long`, its
//! bars that are longer than their time signature. For checking what we write in other programs
//! (`.github/workflows/interop.yml`):
//!
//! `cargo run --release -p oxiguitar-formats --example write_gp -- fixtures/gp fixtures/reference out`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use oxiguitar_model::{BeatKind, Harmonic, NotePlace, Score};

/// The notes of a score, bar by bar, as `alphatab.mjs` lists alphaTab's:
/// per track, staff and bar, each voice's beats (empty placeholders left
/// out), each beat its notes' sounding pitches in order, `r` for a rest,
/// `d` for a drum hit. A bent note, or one tied to a bent note, is marked
/// `b`, a natural harmonic `h` and another harmonic `a` (their pitch is the
/// fretted note's) and a grace note `g`, which `musescore.py` needs:
/// MuseScore writes bends, harmonics and trills its own way.
fn notes(score: &Score) -> String {
    let mut out = format!("{} bars\n", score.master_bars.len());
    for (ti, track) in score.tracks.iter().enumerate() {
        for (si, staff) in track.staves.iter().enumerate() {
            // Whether the last note of each voice on each string (or
            // pitch) was bent, for the notes tied to it.
            let mut bent: HashMap<(usize, NotePlace), bool> = HashMap::new();
            for (bi, bar) in staff.bars.iter().enumerate() {
                let mut voices = Vec::new();
                for (vi, v) in bar.voices.iter().enumerate() {
                    let mut beats = Vec::new();
                    for b in v.beats.iter().filter(|b| b.kind != BeatKind::Empty) {
                        let mut p = Vec::new();
                        for n in b.notes.iter().filter(|_| b.kind == BeatKind::Notes) {
                            let mut x = match n.place {
                                _ if track.instrument.percussion => "d".to_owned(),
                                NotePlace::Fretted { string, fret } => staff
                                    .pitch_of(string, fret)
                                    .map_or("?".to_owned(), |p| p.0.to_string()),
                                NotePlace::Pitched(p) => p.0.to_string(),
                            };
                            let key = match n.place {
                                NotePlace::Fretted { string, .. } => {
                                    NotePlace::Fretted { string, fret: 0 }
                                }
                                place => place,
                            };
                            let was = n.tie && bent.get(&(vi, key)).copied().unwrap_or(false);
                            let is = n.effects.bend.is_some() || was;
                            bent.insert((vi, key), is);
                            if is {
                                x.push('b');
                            }
                            match n.effects.harmonic {
                                Some(Harmonic::Natural) => x.push('h'),
                                Some(_) => x.push('a'),
                                None => {}
                            }
                            if b.grace.is_some() {
                                x.push('g');
                            }
                            p.push(x);
                        }
                        p.sort_by_key(|x| {
                            let digits: String =
                                x.chars().take_while(char::is_ascii_digit).collect();
                            (digits.parse::<u32>().unwrap_or(0), x.clone())
                        });
                        beats.push(if p.is_empty() {
                            "r".to_owned()
                        } else {
                            p.join(",")
                        });
                    }
                    if !beats.is_empty() {
                        voices.push(beats.join(" "));
                    }
                }
                let _ = writeln!(out, "t{ti} s{si} b{}: {}", bi + 1, voices.join(" | "));
            }
        }
    }
    out
}

/// The bars longer than their time signature, one `t{track} b{bar}` a line:
/// MuseScore cuts what doesn't fit, so `musescore.py` doesn't compare them.
fn too_long(score: &Score) -> String {
    let mut out = String::new();
    for (ti, track) in score.tracks.iter().enumerate() {
        for staff in &track.staves {
            for (bi, bar) in staff.bars.iter().enumerate() {
                let want = score
                    .master_bars
                    .get(bi)
                    .filter(|mb| !mb.free_time)
                    .and_then(|mb| mb.expected_length());
                let long = bar
                    .voices
                    .iter()
                    .any(|v| matches!((v.length(), want), (Some(l), Some(w)) if l > w));
                if long {
                    let _ = writeln!(out, "t{ti} b{}", bi + 1);
                }
            }
        }
    }
    out
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let Some((out, inputs)) = args.split_last() else {
        eprintln!("usage: write_gp FOLDER... OUT");
        std::process::exit(2);
    };
    std::fs::create_dir_all(out).expect("the output folder");
    let mut written = 0;
    for dir in inputs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("an input folder")
            .filter_map(|e| Some(e.ok()?.path()))
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for path in files {
            let Ok(data) = std::fs::read(&path) else {
                continue;
            };
            let Ok(read) = oxiguitar_formats::read(&data) else {
                continue;
            };
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let file =
                oxiguitar_formats::write_gp(&read.score).unwrap_or_else(|e| panic!("{name}: {e}"));
            std::fs::write(out.join(format!("{name}.gp")), &file.data).unwrap();
            let back = oxiguitar_formats::read(&file.data)
                .unwrap_or_else(|e| panic!("{name}: we can't read what we wrote: {e}"));
            std::fs::write(out.join(format!("{name}.notes")), notes(&back.score)).unwrap();
            std::fs::write(out.join(format!("{name}.long")), too_long(&back.score)).unwrap();
            let lost = file.losses.warning_count();
            println!("{name}: {} bytes, {lost} kinds of loss", file.data.len());
            written += 1;
        }
    }
    println!("{written} files written to {}", Path::new(out).display());
}
