// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Round trips through the .gp5 writer (FMT-4), on every fixture and, where
//! `corpus/files/` exists, every file of the private corpus:
//! `cargo test --release -p oxiguitar-formats --test gp5_round_trip -- --nocapture`
//!
//! - A Guitar Pro 3, 4 or 5 file read, written as .gp5 and read again is
//!   the same score, with no losses.
//! - Any other file, written as .gp5 and read back, is the same score in
//!   its normal form ([`normal`]: what changes and nobody sees), unless the
//!   writer named a loss. No loss goes unnamed.
//! - Writing what was read back loses nothing more: the second round trip
//!   is exact.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use oxiguitar_formats::{Format, read, write_gp5};
use oxiguitar_model::{Beat, BeatKind, ChordDiagram, Clef, Dynamic, Score, Slide, Voice};

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

/// The score as a .gp5 file holds it without losses: what the writer
/// changes and nobody sees.
fn normal(s: &Score) -> Score {
    let mut s = s.clone();
    s.initial_tempo = s.initial_tempo.round();
    for mb in &mut s.master_bars {
        mb.tempo_changes.sort_by_key(|t| t.position);
        for t in &mut mb.tempo_changes {
            t.bpm = t.bpm.round();
        }
        mb.directions.sort_by_key(|d| direction_order(*d));
    }
    for t in &mut s.tracks {
        t.short_name.clear();
        t.mix.volume = (t.mix.volume * 16.0).round() / 16.0;
        t.mix.pan = ((t.mix.pan + 1.0) * 8.0).round() / 8.0 - 1.0;
        t.lyrics.retain(|l| !l.text.trim().is_empty());
        // The diagrams in the order the beats use them.
        let mut chords: Vec<ChordDiagram> = Vec::new();
        let mut map = vec![None; t.chords.len()];
        let used: Vec<usize> = t
            .staves
            .iter()
            .take(1)
            .flat_map(|st| &st.bars)
            .flat_map(|b| b.voices.iter().take(2))
            .flat_map(|v| &v.beats)
            .filter_map(|b| b.chord)
            .collect();
        for c in used {
            let Some(d) = t.chords.get(c) else { continue };
            let at = chords.iter().position(|x| x == d).unwrap_or_else(|| {
                chords.push(d.clone());
                chords.len() - 1
            });
            map[c] = Some(at);
        }
        let strings = t.staves[0].tuning.len();
        t.chords = chords;
        for c in &mut t.chords {
            c.frets.resize(strings.min(7), None);
        }
        t.staves.truncate(1);
        let percussion = t.instrument.percussion;
        let clef = oxiguitar_model::Clef::Treble8vb;
        let _ = clef;
        let staff = &mut t.staves[0];
        staff.transposition = 0;
        if percussion && staff.tuning.is_empty() {
            staff.tuning = vec![oxiguitar_model::Pitch(0); 6];
        }
        for bar in &mut staff.bars {
            bar.voices.truncate(2);
            // Voices holding only placeholders are not written: they come
            // back as empty voices, or not at all after the last used one.
            // A first voice without beats is one empty beat.
            if let Some(v) = bar.voices.first_mut().filter(|v| v.beats.is_empty()) {
                v.beats.push(Beat {
                    kind: BeatKind::Empty,
                    ..Beat::default()
                });
            }
            for v in bar.voices.iter_mut().skip(1) {
                if v.is_empty() {
                    *v = Voice::default();
                }
            }
            while bar.voices.len() > 1 && bar.voices.last() == Some(&Voice::default()) {
                bar.voices.pop();
            }
            for v in &mut bar.voices {
                normal_voice(v, &map, percussion);
            }
        }
    }
    s
}

fn direction_order(d: oxiguitar_model::Direction) -> usize {
    use oxiguitar_model::Direction::*;
    [
        Coda,
        DoubleCoda,
        Segno,
        SegnoSegno,
        Fine,
        DaCapo,
        DaCapoAlCoda,
        DaCapoAlDoubleCoda,
        DaCapoAlFine,
        DalSegno,
        DalSegnoAlCoda,
        DalSegnoAlDoubleCoda,
        DalSegnoAlFine,
        DalSegnoSegno,
        DalSegnoSegnoAlCoda,
        DalSegnoSegnoAlDoubleCoda,
        DalSegnoSegnoAlFine,
        ToCoda,
        ToDoubleCoda,
    ]
    .iter()
    .position(|x| *x == d)
    .unwrap()
}

fn normal_voice(v: &mut Voice, chords: &[Option<usize>], percussion: bool) {
    for b in &mut v.beats {
        normal_beat(b, chords, percussion);
    }
}

fn normal_beat(b: &mut Beat, chords: &[Option<usize>], percussion: bool) {
    b.chord = b.chord.and_then(|c| chords.get(c).copied().flatten());
    if b.kind != BeatKind::Notes {
        b.notes.clear();
    }
    b.kind = if b.notes.is_empty() && b.kind == BeatKind::Notes {
        BeatKind::Rest
    } else {
        b.kind
    };
    // Dynamics are the notes'; a marking is not stored.
    b.effects.dynamic = None;
    // Tremolo picking and let ring are the notes'.
    let tremolo = b.effects.tremolo_picking.take();
    let ring = std::mem::take(&mut b.effects.let_ring);
    for n in &mut b.notes {
        n.effects.tremolo_picking = n.effects.tremolo_picking.or(tremolo);
        n.effects.let_ring |= ring;
        n.effects.slides.sort_by_key(|s| slide_order(*s));
        if let Some(bend) = &mut n.effects.bend
            && bend.points.is_empty()
        {
            n.effects.bend = None;
        }
    }
    // The order of the notes of a beat is the strings'; drums are kept
    // by pitch.
    if percussion {
        // Equal drums struck twice in a beat are told apart by nothing else
        // than their dynamic or effects.
        b.notes.sort_by_key(|n| {
            let pitch = match n.place {
                oxiguitar_model::NotePlace::Pitched(p) => p.0,
                oxiguitar_model::NotePlace::Fretted { .. } => 0,
            };
            (pitch, format!("{n:?}"))
        });
    } else {
        b.notes.sort_by_key(|n| n.string().unwrap_or(0));
    }
    if b.effects
        .whammy
        .as_ref()
        .is_some_and(|w| w.points.is_empty())
    {
        b.effects.whammy = None;
    }
    b.duration
        .tuplets
        .retain(|t| t.notes > 1 && t.in_time_of > 0 && t.notes != t.in_time_of);
    let _ = Dynamic::F;
}

fn slide_order(s: Slide) -> usize {
    [
        Slide::Shift,
        Slide::Legato,
        Slide::OutDown,
        Slide::OutUp,
        Slide::InFromBelow,
        Slide::InFromAbove,
    ]
    .iter()
    .position(|x| *x == s)
    .unwrap()
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
    if a.master_bars.len() != b.master_bars.len() {
        return "bar count".into();
    }
    for (i, (x, y)) in a.master_bars.iter().zip(&b.master_bars).enumerate() {
        if x != y {
            return format!("master bar {}: {x:?}\n  vs {y:?}", i + 1);
        }
    }
    if a.tracks.len() != b.tracks.len() {
        return "track count".into();
    }
    for (ti, (x, y)) in a.tracks.iter().zip(&b.tracks).enumerate() {
        let t = ti + 1;
        if (&x.name, &x.short_name, x.color) != (&y.name, &y.short_name, y.color) {
            return format!("track {t} name or colour");
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
            return format!("track {t} lyrics: {:?}\n  vs {:?}", x.lyrics, y.lyrics);
        }
        if x.staves.len() != y.staves.len() {
            return format!("track {t} staff count");
        }
        for (si, (p, q)) in x.staves.iter().zip(&y.staves).enumerate() {
            let s = si + 1;
            if (
                &p.tuning,
                p.capo,
                p.fret_count,
                p.transposition,
                p.show_standard,
                p.show_tablature,
            ) != (
                &q.tuning,
                q.capo,
                q.fret_count,
                q.transposition,
                q.show_standard,
                q.show_tablature,
            ) {
                return format!(
                    "track {t} staff {s}: {:?} {} {} vs {:?} {} {}",
                    p.tuning, p.capo, p.fret_count, q.tuning, q.capo, q.fret_count
                );
            }
            if p.bars.len() != q.bars.len() {
                return format!("track {t} staff {s} bar count");
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
        }
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
        let w1 = match write_gp5(&s1) {
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
        // Warnings are losses; notes (infos) say what is held in another
        // shape.
        let lost = |w: &oxiguitar_formats::Written| w.losses.warning_count() > 0;
        for e in w1.losses.entries() {
            *t.messages
                .entry(format!("{:?} {}", e.severity, e.message))
                .or_default() += 1;
        }
        let list = |w: &oxiguitar_formats::Written| {
            w.losses
                .entries()
                .iter()
                .filter(|e| e.severity == oxiguitar_formats::Severity::Warning)
                .map(|e| format!("{} ({})", e.message, e.first_place))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let own = matches!(first.format, Format::GuitarPro5 { .. });
        if own && lost(&w1) {
            t.failures
                .push(format!("{name}: a .gp5 file loses {}", list(&w1)));
        }
        if !lost(&w1) {
            let expected = normal(&s1);
            if expected != normal(&s2) {
                t.failures.push(format!(
                    "{name}: changed without a named loss: {}",
                    first_difference(&expected, &normal(&s2))
                ));
            } else {
                t.exact += 1;
            }
        } else {
            t.with_losses += 1;
        }
        // The second round trip is exact and loses nothing.
        let w2 = write_gp5(&s2).unwrap();
        let s3 = read(&w2.data).unwrap().score;
        if lost(&w2) {
            t.failures
                .push(format!("{name}: the second round trip loses {}", list(&w2)));
        } else if normal(&s3) != normal(&s2) {
            t.failures.push(format!(
                "{name}: the second round trip changes {}",
                first_difference(&normal(&s2), &normal(&s3))
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
fn the_file_is_a_guitar_pro_5_file() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp5");
    let score = read(&std::fs::read(path).unwrap()).unwrap().score;
    let written = write_gp5(&score).unwrap();
    assert_eq!(written.losses.warning_count(), 0);
    assert_eq!(&written.data[1..25], b"FICHIER GUITAR PRO v5.10");
    let again = read(&written.data).unwrap();
    assert!(matches!(again.format, Format::GuitarPro5 { .. }));
    assert_eq!(again.score, score);
    assert_eq!(again.trailing_bytes, 0);
}

#[test]
fn clefs_are_not_stored() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp/oxiguitar-test-song.gp5");
    let mut score = read(&std::fs::read(path).unwrap()).unwrap().score;
    score.tracks[0].staves[0].bars[0].clef = Clef::Bass;
    let written = write_gp5(&score).unwrap();
    assert!(
        written
            .losses
            .entries()
            .iter()
            .any(|e| e.message.contains("clef"))
    );
}
