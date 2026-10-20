// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Round trips through the .gp writer (FMT-4), on every fixture and, where
//! `corpus/files/` exists, every file of the private corpus:
//! `cargo test --release -p oxiguitar-formats --test round_trip -- --nocapture`
//!
//! - A .gp file read, written and read again is the same score, with no
//!   losses.
//! - Any other file, written as .gp and read back, is the same score in its
//!   normal form ([`normal`]: what changes and nobody sees), unless the
//!   writer named a loss. No loss goes unnamed.
//! - Writing what was read back loses nothing more: the second round trip
//!   is exact.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use oxiguitar_formats::{Format, read, write_gp};
use oxiguitar_model::{Beat, BeatKind, BendPoint, ChordDiagram, Dynamic, Score, Slide, Voice};

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

fn under(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(dir);
    let mut all = Vec::new();
    files(&root, &mut all);
    all.sort();
    all
}

/// The score as a .gp file holds it without losses: what the writer
/// changes and nobody sees.
fn normal(s: &Score) -> Score {
    let mut s = s.clone();
    let trim = |t: &mut String| *t = t.trim().to_owned();
    let i = &mut s.info;
    for t in [
        &mut i.title,
        &mut i.subtitle,
        &mut i.artist,
        &mut i.album,
        &mut i.words,
        &mut i.music,
        &mut i.copyright,
        &mut i.transcriber,
        &mut i.instructions,
    ] {
        trim(t);
    }
    for n in &mut i.notices {
        *n = n.trim_end().to_owned();
    }
    while i.notices.last().is_some_and(String::is_empty) {
        i.notices.pop();
    }
    for mb in &mut s.master_bars {
        mb.tempo_changes.sort_by_key(|t| t.position);
        if let Some(m) = &mut mb.marker {
            trim(&mut m.letter);
            trim(&mut m.text);
        }
        // A section without a letter or name shows nothing.
        if mb
            .marker
            .as_ref()
            .is_some_and(|m| m.letter.is_empty() && m.text.is_empty())
        {
            mb.marker = None;
        }
    }
    for t in &mut s.tracks {
        trim(&mut t.name);
        trim(&mut t.short_name);
        for l in &mut t.lyrics {
            trim(&mut l.text);
        }
        t.lyrics.retain(|l| !l.text.is_empty());
        // Equal diagrams are one diagram.
        let mut chords: Vec<ChordDiagram> = Vec::new();
        let mut map = Vec::new();
        for c in &t.chords {
            match chords.iter().position(|x| x == c) {
                Some(i) => map.push(i),
                None => {
                    chords.push(c.clone());
                    map.push(chords.len() - 1);
                }
            }
        }
        t.chords = chords;
        // A diagram starts at fret 1 at the lowest.
        for c in &mut t.chords {
            c.first_fret = c.first_fret.max(1);
        }
        let percussion = t.instrument.percussion;
        for staff in &mut t.staves {
            // Drums have no strings to show.
            if percussion {
                staff.tuning.clear();
                staff.show_standard = true;
                staff.show_tablature = false;
            }
            for bar in &mut staff.bars {
                if percussion {
                    bar.clef = oxiguitar_model::Clef::Neutral;
                }
                // Voices holding only placeholders are not written: they
                // come back as empty voices, or not at all after the last
                // used one.
                for v in bar.voices.iter_mut().skip(1) {
                    if v.is_empty() {
                        *v = Voice::default();
                    }
                }
                while bar.voices.len() > 1 && bar.voices.last() == Some(&Voice::default()) {
                    bar.voices.pop();
                }
                for v in &mut bar.voices {
                    normal_voice(v, &map);
                }
            }
        }
    }
    s
}

fn normal_voice(v: &mut Voice, chords: &[usize]) {
    for b in &mut v.beats {
        normal_beat(b, chords);
    }
}

fn normal_beat(b: &mut Beat, chords: &[usize]) {
    b.chord = b.chord.and_then(|c| chords.get(c).copied());
    b.text = b.text.trim().to_owned();
    if b.kind != BeatKind::Notes {
        b.notes.clear();
    }
    b.kind = if b.notes.is_empty() {
        BeatKind::Rest
    } else {
        BeatKind::Notes
    };
    // A marking from the notes' shared dynamic; the notes take the beat's.
    if b.effects.dynamic.is_none()
        && let Some(d) = b.notes.first().map(|n| n.dynamic)
        && d != Dynamic::default()
    {
        b.effects.dynamic = Some(d);
    }
    let dynamic = b.effects.dynamic.unwrap_or_default();
    // Tremolo picking is the beat's; let ring the notes'.
    if b.effects.tremolo_picking.is_none() {
        b.effects.tremolo_picking = b.notes.iter().find_map(|n| n.effects.tremolo_picking);
    }
    let ring = std::mem::take(&mut b.effects.let_ring);
    for n in &mut b.notes {
        n.dynamic = dynamic;
        n.effects.tremolo_picking = None;
        n.effects.let_ring |= ring;
        // Slides in, then out.
        n.effects
            .slides
            .sort_by_key(|s| !matches!(s, Slide::InFromBelow | Slide::InFromAbove));
    }
    b.notes.sort_by_key(|n| n.string().unwrap_or(0));
    for n in &mut b.notes {
        if let Some(bend) = &mut n.effects.bend {
            bend.points = fit_curve(&bend.points);
        }
        if n.effects
            .bend
            .as_ref()
            .is_some_and(|b| b.points.iter().all(|p| p.cents == 0))
        {
            n.effects.bend = None;
        }
    }
    if let Some(w) = &mut b.effects.whammy {
        w.points = fit_curve(&w.points);
    }
    if b.effects
        .whammy
        .as_ref()
        .is_some_and(|w| w.points.iter().all(|p| p.cents == 0))
    {
        b.effects.whammy = None;
    }
    b.duration
        .tuplets
        .retain(|t| t.notes > 1 && t.in_time_of > 0 && t.notes != t.in_time_of);
}

/// A curve without the points that change nothing, when GPIF can't hold
/// it as it is (the writer's `fit_curve`; curves that still don't fit are
/// named losses).
fn fit_curve(p: &[BendPoint]) -> Vec<BendPoint> {
    let fits = |p: &[BendPoint]| {
        p.windows(2)
            .all(|w| w[0] != w[1] && w[0].offset <= w[1].offset)
            && match p {
                [_, b, c, _] => b.cents == c.cents,
                _ => p.len() <= 3,
            }
    };
    if fits(p) {
        return p.to_vec();
    }
    let mut v = p.to_vec();
    v.dedup();
    let mut i = 1;
    while i + 1 < v.len() {
        if v[i - 1].cents == v[i].cents && v[i].cents == v[i + 1].cents {
            v.remove(i);
        } else {
            i += 1;
        }
    }
    if v.len() >= 2 && v[0].cents == 0 && v[1].cents == 0 {
        v.remove(0);
    }
    if v.len() >= 2 && v[v.len() - 2].cents == 0 && v[v.len() - 1].cents == 0 {
        v.pop();
    }
    v
}

/// Where two scores first differ, for the failure message.
fn first_difference(a: &Score, b: &Score) -> String {
    if a.info != b.info {
        return format!("info: {:?}\n  vs {:?}", a.info, b.info);
    }
    if a.initial_tempo != b.initial_tempo || a.tempo_label != b.tempo_label {
        return format!(
            "tempo: {} {:?} vs {} {:?}",
            a.initial_tempo, a.tempo_label, b.initial_tempo, b.tempo_label
        );
    }
    for (i, (x, y)) in a.master_bars.iter().zip(&b.master_bars).enumerate() {
        if x != y {
            return format!("master bar {}: {x:?}\n  vs {y:?}", i + 1);
        }
    }
    if a.master_bars.len() != b.master_bars.len() {
        return "bar count".into();
    }
    for (ti, (x, y)) in a.tracks.iter().zip(&b.tracks).enumerate() {
        let t = ti + 1;
        if x.name != y.name || x.short_name != y.short_name || x.color != y.color {
            return format!(
                "track {t} name or colour: {:?} {:?} {:?} vs {:?} {:?} {:?}",
                x.name, x.short_name, x.color, y.name, y.short_name, y.color
            );
        }
        if x.instrument != y.instrument {
            return format!(
                "track {t} instrument: {:?} vs {:?}",
                x.instrument, y.instrument
            );
        }
        if x.mix != y.mix {
            return format!("track {t} mix: {:?} vs {:?}", x.mix, y.mix);
        }
        if x.chords != y.chords {
            return format!("track {t} chords: {:?}\n  vs {:?}", x.chords, y.chords);
        }
        if x.lyrics != y.lyrics {
            return format!("track {t} lyrics");
        }
        for (si, (p, q)) in x.staves.iter().zip(&y.staves).enumerate() {
            let s = si + 1;
            if p.tuning != q.tuning
                || p.capo != q.capo
                || p.fret_count != q.fret_count
                || p.transposition != q.transposition
                || p.show_standard != q.show_standard
                || p.show_tablature != q.show_tablature
            {
                return format!(
                    "track {t} staff {s}: {:?} {} {} {} {} {}\n  vs {:?} {} {} {} {} {}",
                    p.tuning,
                    p.capo,
                    p.fret_count,
                    p.transposition,
                    p.show_standard,
                    p.show_tablature,
                    q.tuning,
                    q.capo,
                    q.fret_count,
                    q.transposition,
                    q.show_standard,
                    q.show_tablature
                );
            }
            for (bi, (u, v)) in p.bars.iter().zip(&q.bars).enumerate() {
                let place = format!("track {t} staff {s} bar {}", bi + 1);
                if u.clef != v.clef {
                    return format!("{place} clef: {:?} vs {:?}", u.clef, v.clef);
                }
                if u.voices.len() != v.voices.len() {
                    return format!("{place} voices: {} vs {}", u.voices.len(), v.voices.len());
                }
                for (vi, (m, n)) in u.voices.iter().zip(&v.voices).enumerate() {
                    if m.beats.len() != n.beats.len() {
                        return format!(
                            "{place} voice {} beats: {} vs {}",
                            vi + 1,
                            m.beats.len(),
                            n.beats.len()
                        );
                    }
                    for (ki, (g, h)) in m.beats.iter().zip(&n.beats).enumerate() {
                        if g != h {
                            return format!(
                                "{place} voice {} beat {}: {g:?}\n  vs {h:?}",
                                vi + 1,
                                ki + 1
                            );
                        }
                    }
                }
            }
            if p.bars.len() != q.bars.len() {
                return format!("track {t} staff {s} bar count");
            }
        }
        if x.staves.len() != y.staves.len() {
            return format!("track {t} staff count");
        }
    }
    if a.tracks.len() != b.tracks.len() {
        return "track count".into();
    }
    "no difference found".into()
}

struct Tally {
    files: usize,
    exact: usize,
    with_losses: usize,
    failures: Vec<String>,
    /// Each loss and in how many files.
    messages: std::collections::BTreeMap<String, usize>,
}

fn round_trips(paths: &[PathBuf]) -> Tally {
    let mut t = Tally {
        files: 0,
        exact: 0,
        with_losses: 0,
        failures: Vec::new(),
        messages: Default::default(),
    };
    for path in paths {
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        let Ok(first) = read(&data) else {
            continue;
        };
        t.files += 1;
        let name = path.display().to_string();
        let s1 = first.score;
        let w1 = match write_gp(&s1) {
            Ok(w) => w,
            Err(e) => {
                t.failures.push(format!("{name}: not written: {e}"));
                continue;
            }
        };
        let s2 = match read(&w1.data) {
            Ok(r) => r.score,
            Err(e) => {
                t.failures
                    .push(format!("{name}: what we wrote doesn't read: {e}"));
                continue;
            }
        };
        let gp = first.format == Format::GuitarPro7Plus;
        // Warnings are losses; notes (infos) say what Guitar Pro does
        // differently with what reads back the same here.
        let lost = |w: &oxiguitar_formats::Written| w.losses.warning_count() > 0;
        for e in w1.losses.entries() {
            *t.messages
                .entry(format!("{:?} {}", e.severity, e.message))
                .or_default() += 1;
        }
        if gp && lost(&w1) {
            let l: Vec<String> = w1
                .losses
                .entries()
                .iter()
                .map(|e| format!("{} ({})", e.message, e.first_place))
                .collect();
            t.failures
                .push(format!("{name}: a .gp file loses {}", l.join("; ")));
        }
        if !lost(&w1) {
            let expected = if gp { s1.clone() } else { normal(&s1) };
            if expected != s2 {
                t.failures.push(format!(
                    "{name}: changed without a named loss: {}",
                    first_difference(&expected, &s2)
                ));
            } else {
                t.exact += 1;
            }
        } else {
            t.with_losses += 1;
        }
        // The second round trip is exact and loses nothing.
        let w2 = write_gp(&s2).unwrap();
        let s3 = read(&w2.data).unwrap().score;
        if lost(&w2) {
            t.failures.push(format!(
                "{name}: the second round trip loses {:?}",
                w2.losses.entries().first().map(|e| &e.message)
            ));
        } else if s3 != s2 {
            t.failures.push(format!(
                "{name}: the second round trip changes {}",
                first_difference(&s2, &s3)
            ));
        }
    }
    t
}

fn report(what: &str, t: &Tally) {
    println!(
        "{what}: {} files, {} exact, {} with named losses, {} failures",
        t.files,
        t.exact,
        t.with_losses,
        t.failures.len()
    );
    for (m, n) in &t.messages {
        println!("  {n:4} {m}");
    }
    for f in t.failures.iter().take(30) {
        println!("  {f}");
    }
}

#[test]
fn fixtures_round_trip() {
    let t = round_trips(&under("fixtures"));
    report("fixtures", &t);
    assert!(t.files >= 50, "{}", t.files);
    assert!(t.failures.is_empty(), "{} failures", t.failures.len());
}

#[test]
fn corpus_round_trips() {
    let paths = under("corpus/files");
    if paths.is_empty() {
        return;
    }
    let t = round_trips(&paths);
    report("corpus", &t);
    assert!(t.failures.is_empty(), "{} failures", t.failures.len());
}

#[test]
fn free_time_round_trips() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp");
    let mut score = read(&std::fs::read(path).unwrap()).unwrap().score;
    score.master_bars[1].free_time = true;
    let written = write_gp(&score).unwrap();
    assert_eq!(written.losses.warning_count(), 0);
    assert_eq!(read(&written.data).unwrap().score, score);
}
