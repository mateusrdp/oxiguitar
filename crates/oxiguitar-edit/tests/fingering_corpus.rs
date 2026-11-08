// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Automatic fingering (#157) on every Guitar Pro file of the private
//! corpus, where `corpus/files/` exists (skipped quietly otherwise):
//! `cargo test --release -p oxiguitar-edit --test fingering_corpus -- --nocapture`
//!
//! Each file's tab is taken away (every note becomes its pitch), found again
//! as a format without tablature would have it, and checked: every note
//! sounds as it did. Then Find Fingering runs on every track. It prints how
//! many notes landed where the transcriber put them, how many beats are
//! marked hard, and how long it took.

use std::path::{Path, PathBuf};
use std::time::Instant;

use oxiguitar_edit::{Caret, Command, HARD_FINGERS, HARD_SHIFT, HARD_STRETCH, History, Scope};
use oxiguitar_model::{NotePlace, Score};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every note's sounding pitch (with its place, if on a string), staff by
/// staff, for staves with strings outside drum tracks, each beat's in
/// pitch order (fingering sorts them by string). A tied note sounds as the
/// note it continues on its string, as playback has it.
fn notes(score: &Score) -> Vec<(u8, Option<(u8, u8)>)> {
    let mut out = Vec::new();
    for t in score.tracks.iter().filter(|t| !t.instrument.percussion) {
        for s in t.staves.iter().filter(|s| !s.tuning.is_empty()) {
            let voices = s.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
            let mut ringing: Vec<std::collections::BTreeMap<u8, u8>> =
                vec![Default::default(); voices];
            for b in &s.bars {
                for (vi, v) in b.voices.iter().enumerate() {
                    for bt in &v.beats {
                        let mut beat: Vec<(u8, Option<(u8, u8)>)> = bt
                            .notes
                            .iter()
                            .map(|n| match n.place {
                                NotePlace::Fretted { string, fret } => {
                                    let mut p = sounding(s, n, string, fret);
                                    if n.tie
                                        && let Some(r) = ringing[vi].get(&string)
                                    {
                                        p = *r;
                                    }
                                    ringing[vi].insert(string, p);
                                    (p, Some((string, fret)))
                                }
                                NotePlace::Pitched(p) => (p.0, None),
                            })
                            .collect();
                        beat.sort_by_key(|n| n.0);
                        out.extend(beat);
                    }
                }
            }
        }
    }
    out
}

/// Where the `k`th note of [`notes`] is, in words, with the bar before
/// and its own, before and after.
fn locate_both(score: &Score, after: &Score, k: usize) -> String {
    let mut seen = 0;
    for (ti, t) in score
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.instrument.percussion)
    {
        for (si, s) in t
            .staves
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.tuning.is_empty())
        {
            for (bi, b) in s.bars.iter().enumerate() {
                for v in &b.voices {
                    for bt in &v.beats {
                        seen += bt.notes.len();
                        if seen > k {
                            let mut out = locate(score, k);
                            for (name, sc) in [("before", score), ("after", after)] {
                                let st = &sc.tracks[ti].staves[si];
                                for bj in bi.saturating_sub(1)..=bi {
                                    for (vi, v) in st.bars[bj].voices.iter().enumerate() {
                                        out += &format!(
                                            "\n  {name} bar {} voice {}: {:?}",
                                            bj + 1,
                                            vi + 1,
                                            v.beats
                                                .iter()
                                                .map(|b| {
                                                    let g =
                                                        if b.grace.is_some() { "g" } else { "" };
                                                    b.notes
                                                        .iter()
                                                        .map(|n| match n.place {
                                                            NotePlace::Fretted { string, fret } => {
                                                                format!(
                                                                    "{g}{string}/{fret}{}{}",
                                                                    if n.tie { "~" } else { "" },
                                                                    if n.effects.harmonic.is_some()
                                                                    {
                                                                        "h"
                                                                    } else {
                                                                        ""
                                                                    }
                                                                )
                                                            }
                                                            NotePlace::Pitched(p) => {
                                                                format!("{g}p{}", p.0)
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                })
                                                .collect::<Vec<_>>()
                                        );
                                    }
                                }
                            }
                            return out;
                        }
                    }
                }
            }
        }
    }
    locate(score, k)
}

/// Where the `k`th note of [`notes`] is, in words.
fn locate(score: &Score, k: usize) -> String {
    let mut seen = 0;
    for (ti, t) in score
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.instrument.percussion)
    {
        for (si, s) in t
            .staves
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.tuning.is_empty())
        {
            for (bi, b) in s.bars.iter().enumerate() {
                for (vi, v) in b.voices.iter().enumerate() {
                    for (ki, bt) in v.beats.iter().enumerate() {
                        seen += bt.notes.len();
                        if seen > k {
                            return format!(
                                "track {ti} staff {si} bar {} voice {} beat {ki} ({:?})",
                                bi + 1,
                                vi + 1,
                                bt.notes
                                    .iter()
                                    .map(|n| (n.place, n.tie, n.effects.harmonic))
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
    }
    format!("note {k}")
}

/// Tied notes given the fret of the note they continue on their string, in
/// their voice, as the editor and playback read them: some files store
/// another fret there.
fn follow_ties(score: &mut Score) {
    for staff in score.tracks.iter_mut().flat_map(|t| &mut t.staves) {
        let voices = staff.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
        for voice in 0..voices {
            let mut last: std::collections::BTreeMap<u8, u8> = Default::default();
            for bar in &mut staff.bars {
                let Some(v) = bar.voices.get_mut(voice) else {
                    continue;
                };
                for n in v.beats.iter_mut().flat_map(|b| &mut b.notes) {
                    if let NotePlace::Fretted { string, fret } = &mut n.place {
                        if n.tie
                            && let Some(f) = last.get(string)
                        {
                            *fret = *f;
                        }
                        last.insert(*string, *fret);
                    }
                }
            }
        }
    }
}

/// What a fretted note sounds: a natural harmonic its node's pitch (as
/// playback has it), any other note its fretted pitch.
fn sounding(s: &oxiguitar_model::Staff, n: &oxiguitar_model::Note, string: u8, fret: u8) -> u8 {
    let open = s.pitch_of(string, 0).map_or(0, |p| p.0);
    if n.effects.harmonic == Some(oxiguitar_model::Harmonic::Natural) {
        let above = match fret {
            12 => 12,
            7 | 19 => 19,
            5 | 24 => 24,
            4 | 9 | 16 => 28,
            3 => 31,
            _ => 12,
        };
        return open.saturating_add(above);
    }
    s.pitch_of(string, fret).map_or(0, |p| p.0)
}

#[test]
fn fingering_runs_on_the_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();
    let (mut total, mut same) = (0usize, 0usize);
    for path in all {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_plugin_test::released::guitar_pro::read(&data) else {
            continue;
        };
        let mut score = imported.score;
        follow_ties(&mut score);
        let before = notes(&score);
        // The tab taken away: pitches only, harmonics' sounding pitch kept
        // by leaving those notes where they are.
        let mut bare = score.clone();
        for t in bare.tracks.iter_mut().filter(|t| !t.instrument.percussion) {
            for s in t.staves.iter_mut() {
                let shape = oxiguitar_model::Staff {
                    bars: Vec::new(),
                    ..s.clone()
                };
                let voices = s.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
                for voice in 0..voices {
                    // Whether the note last on each string stayed on it
                    // (a harmonic, or a tie to one): its tie stays too.
                    let mut stayed: std::collections::BTreeMap<u8, bool> = Default::default();
                    for bar in &mut s.bars {
                        let Some(v) = bar.voices.get_mut(voice) else {
                            continue;
                        };
                        for n in v.beats.iter_mut().flat_map(|b| &mut b.notes) {
                            let NotePlace::Fretted { string, fret } = n.place else {
                                continue;
                            };
                            let stays = n.effects.harmonic.is_some()
                                || (n.tie && stayed.get(&string).copied().unwrap_or(false));
                            stayed.insert(string, stays);
                            if !stays && let Some(p) = shape.pitch_of(string, fret) {
                                n.place = NotePlace::Pitched(p);
                            }
                        }
                    }
                }
            }
        }
        let t0 = Instant::now();
        oxiguitar_edit::finger_pitched(&mut bare);
        let open_time = t0.elapsed();
        let after = notes(&bare);
        assert_eq!(before.len(), after.len(), "{}", path.display());
        let mut kept = 0;
        for (k, (b, a)) in before.iter().zip(&after).enumerate() {
            assert_eq!(
                b.0,
                a.0,
                "{}: {} changed pitch at open: {b:?} became {a:?}",
                path.display(),
                locate_both(&score, &bare, k)
            );
            if b.1.is_some() && b.1 == a.1 {
                kept += 1;
            }
        }
        total += before.len();
        same += kept;
        // Find Fingering on every track of the file as it is.
        let mut edited = score.clone();
        let mut history = History::new();
        let t1 = Instant::now();
        let hard = match Caret::at_start(&edited, 0) {
            Some(c) => {
                oxiguitar_edit::apply(
                    &mut edited,
                    &mut history,
                    c,
                    None,
                    Command::Fingering(Scope::All),
                );
                let now = notes(&edited);
                if let Some(k) = now.iter().zip(&before).position(|(a, b)| a.0 != b.0) {
                    panic!(
                        "{}: {} was {:?}, is {:?}",
                        path.display(),
                        locate_both(&score, &edited, k),
                        before[k],
                        now[k]
                    );
                }
                edited
                    .tracks
                    .iter()
                    .flat_map(|t| &t.staves)
                    .flat_map(|s| &s.bars)
                    .flat_map(|b| &b.voices)
                    .flat_map(|v| &v.beats)
                    .filter(|b| [HARD_STRETCH, HARD_FINGERS, HARD_SHIFT].contains(&b.text.as_str()))
                    .count()
            }
            None => 0,
        };
        let tool_time = t1.elapsed();
        println!(
            "{}: {} notes, {:.0} % where the tab had them, {hard} hard beats, {:.2} s at open, {:.2} s as the tool",
            path.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            before.len(),
            100.0 * kept as f64 / before.len().max(1) as f64,
            open_time.as_secs_f64(),
            tool_time.as_secs_f64(),
        );
    }
    if total > 0 {
        println!(
            "All: {total} notes, {:.1} % where the tabs had them",
            100.0 * same as f64 / total as f64
        );
    }
}
