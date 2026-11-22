// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Harmonize (#235) on every Guitar Pro file of the test corpus
//! (`OXIGUITAR_CORPUS`, or `corpus/files/`; see `tests/support/corpus.rs`),
//! file by file:
//! `cargo test --release -p oxiguitar-edit --test harmonize_corpus -- --nocapture`
//!
//! Every pitched track of every file is harmonized four ways, which take
//! each of the dialog's choices both ways:
//!
//! - the score stays sound, only the caret's track changes, and of it only
//!   its chords;
//! - every chord written has a name the chord builder reads back;
//! - with the chords kept, every chord the file had is still on its beat;
//! - Undo gives the file back.
//!
//! Where a track of the file has chords of its own, the test also prints
//! how many bars get the file's chord and how many its root (nothing is
//! asserted: a melody has more than one good harmonization). Tabs for a
//! guitar tuned down, or with a capo, name their chords by shape (a C
//! shape tuned a semitone down is written C); Harmonize names what
//! sounds, so the file's roots are moved by the tuning and the capo
//! before they are compared.

#[path = "../../../tests/support/corpus.rs"]
mod corpus;
#[path = "../../../tests/support/guitar_pro.rs"]
mod guitar_pro;

use std::path::Path;
use std::time::Instant;

use oxiguitar_edit::harmony::Chord;
use oxiguitar_edit::{Caret, Command, Harmonize, History, harmonize_report};
use oxiguitar_model::{Score, Track};

/// A track without its chords: what Harmonize must leave alone.
fn without_chords(track: &Track) -> Track {
    let mut t = track.clone();
    t.chords.clear();
    for beat in t
        .staves
        .iter_mut()
        .flat_map(|s| &mut s.bars)
        .flat_map(|b| &mut b.voices)
        .flat_map(|v| &mut v.beats)
    {
        beat.chord = None;
    }
    t
}

/// The chord names on a track's beats, as (staff, bar, voice, beat, name).
fn chords(track: &Track) -> Vec<(usize, usize, usize, usize, String)> {
    let mut out = Vec::new();
    for (s, staff) in track.staves.iter().enumerate() {
        for (k, bar) in staff.bars.iter().enumerate() {
            for (v, voice) in bar.voices.iter().enumerate() {
                for (i, beat) in voice.beats.iter().enumerate() {
                    if let Some(d) = beat.chord.and_then(|c| track.chords.get(c)) {
                        out.push((s, k, v, i, d.name.clone()));
                    }
                }
            }
        }
    }
    out
}

/// The chord in force at the start of each bar of a track's first staff:
/// its root's pitch class and whether it is minor or diminished.
fn by_bar(track: &Track, bars: usize, shift: u8) -> Vec<Option<(u8, bool)>> {
    let mut out = vec![None; bars];
    let mut current = None;
    let mut found = chords(track).into_iter().filter(|c| c.0 == 0).peekable();
    for (k, slot) in out.iter_mut().enumerate() {
        while let Some(c) = found.next_if(|c| c.1 < k || (c.1 == k && c.3 == 0)) {
            current = Chord::parse(&c.4).map(|chord| {
                let minor = matches!(
                    chord.triad,
                    oxiguitar_edit::harmony::Triad::Minor
                        | oxiguitar_edit::harmony::Triad::Diminished
                );
                ((chord.root_class() + shift) % 12, minor)
            });
        }
        *slot = current;
        // A chord later in the bar counts from the next bar on.
        while let Some(c) = found.next_if(|c| c.1 == k) {
            current = Chord::parse(&c.4).map(|chord| {
                let minor = matches!(
                    chord.triad,
                    oxiguitar_edit::harmony::Triad::Minor
                        | oxiguitar_edit::harmony::Triad::Diminished
                );
                ((chord.root_class() + shift) % 12, minor)
            });
        }
    }
    out
}

/// How many semitones above its shape a chord sounds on the track's
/// first staff: the capo, and a tuning moved as a whole from the standard
/// one (judged by its second string, so a dropped lowest string doesn't
/// count).
fn shape_shift(track: &Track) -> u8 {
    let Some(staff) = track.staves.first() else {
        return 0;
    };
    let standard = oxiguitar_model::Staff::standard_guitar_tuning();
    let tuned = match (staff.tuning.get(1), standard.get(1)) {
        (Some(a), Some(b)) if staff.tuning.len() == standard.len() => {
            i32::from(a.0) - i32::from(b.0)
        }
        _ => 0,
    };
    (tuned + i32::from(staff.capo)).rem_euclid(12) as u8
}

fn run(score: &mut Score, history: &mut History, track: usize, h: Harmonize) -> bool {
    let Some(caret) = Caret::at_start(score, track) else {
        return false;
    };
    oxiguitar_edit::apply(score, history, caret, None, Command::Harmonize(h)).is_some()
}

#[test]
fn harmonizing_runs_on_the_corpus() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/files");
    let (mut done, mut tracks, mut written) = (0, 0, 0);
    let (mut bars, mut same, mut roots) = (0usize, 0usize, 0usize);
    let (mut hinted, mut slowest) = (0, std::time::Duration::ZERO);
    corpus::check("harmonize", &fallback, |path, data| {
        let Some(score) = guitar_pro::read(data)? else {
            return Ok(corpus::Outcome::NotTaken);
        };
        // Nothing to harmonize (a file with no bars, or one that doesn't
        // hang together, which the reading checks report).
        if score.bar_count() == 0 || score.check_structure().is_err() {
            return Ok(corpus::Outcome::NotTaken);
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        done += 1;
        let mut hint = false;
        for t in 0..score.tracks.len() {
            if score.tracks[t].instrument.percussion {
                // A drum track takes no chords.
                let mut s = score.clone();
                let h = Harmonize {
                    sevenths: false,
                    inside_bars: false,
                    keep: false,
                    all_tracks: true,
                    whole_song: true,
                };
                assert!(!run(&mut s, &mut History::new(), t, h), "{name}: drums");
                assert_eq!(s, score, "{name}: drums");
                continue;
            }
            tracks += 1;
            let own = chords(&score.tracks[t]);
            for (sevenths, inside_bars, keep, all_tracks) in [
                (false, false, false, true),
                (true, true, false, false),
                (false, true, true, true),
                (true, false, true, false),
            ] {
                let h = Harmonize {
                    sevenths,
                    inside_bars,
                    keep,
                    all_tracks,
                    whole_song: true,
                };
                let mut s = score.clone();
                let mut history = History::new();
                let t0 = Instant::now();
                let changed = run(&mut s, &mut history, t, h);
                slowest = slowest.max(t0.elapsed());
                assert!(s.check_structure().is_ok(), "{name}: track {t}");
                for (i, (a, b)) in score.tracks.iter().zip(&s.tracks).enumerate() {
                    if i == t {
                        assert_eq!(without_chords(a), without_chords(b), "{name}: track {t}");
                    } else {
                        assert_eq!(a, b, "{name}: track {i} while harmonizing {t}");
                    }
                }
                assert_eq!(score.master_bars, s.master_bars, "{name}");
                let now = chords(&s.tracks[t]);
                if keep {
                    for c in &own {
                        assert!(now.contains(c), "{name}: track {t} lost {c:?}");
                    }
                }
                for c in now.iter().filter(|c| !own.contains(c)) {
                    assert!(
                        Chord::parse(&c.4).is_some(),
                        "{name}: track {t} got a chord nobody reads, {}",
                        c.4
                    );
                }
                if changed {
                    history.undo(&mut s);
                    assert_eq!(s, score, "{name}: track {t}, Undo");
                }
                if !sevenths && !inside_bars && !keep && all_tracks {
                    written += now.len();
                    if let Some(caret) = Caret::at_start(&score, t)
                        && let Some(report) = harmonize_report(&score, caret, None, &h)
                    {
                        hint |= report.other_key.is_some();
                    }
                    // Against the file's own chords, where it has enough.
                    if own.len() >= 8 {
                        let mut s = score.clone();
                        run(&mut s, &mut History::new(), t, h);
                        let theirs = by_bar(
                            &score.tracks[t],
                            score.bar_count(),
                            shape_shift(&score.tracks[t]),
                        );
                        let ours = by_bar(&s.tracks[t], score.bar_count(), 0);
                        let (mut n, mut eq, mut rt) = (0, 0, 0);
                        for (a, b) in theirs.iter().zip(&ours) {
                            if let (Some(a), Some(b)) = (a, b) {
                                n += 1;
                                eq += usize::from(a == b);
                                rt += usize::from(a.0 == b.0);
                            }
                        }
                        println!(
                            "{name}: track {t} has {} chords of its own; of {n} bars {eq} get the same chord, {rt} its root",
                            own.len()
                        );
                        bars += n;
                        same += eq;
                        roots += rt;
                    }
                }
            }
        }
        hinted += usize::from(hint);
        Ok(corpus::Outcome::Passed)
    });
    if done > 0 {
        println!(
            "{done} files, {tracks} pitched tracks, {written} chords written (plain, one a bar); slowest run {slowest:?}"
        );
        println!("{hinted} files where the notes fit another key than the key signature's better");
        if bars > 0 {
            println!(
                "against the files' own chords: {same} of {bars} bars the same chord ({:.0} %), {roots} the same root ({:.0} %)",
                100.0 * same as f64 / bars as f64,
                100.0 * roots as f64 / bars as f64
            );
        }
    }
}
