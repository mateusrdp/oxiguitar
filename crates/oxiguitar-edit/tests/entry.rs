// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Slice 3's gates for note entry, on the public fixtures: random runs of
//! caret moves and commands, every one undone back to the original and
//! redone to the end; and the edited score saved as .gp reads back equal.
//! Where the private corpus is (`corpus/files/`), its files too.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use oxiguitar_edit::{Caret, Command, History, Selection, apply};
use oxiguitar_model::Score;

/// A small, fixed random sequence (xorshift), so a failure repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const COMMANDS: [Command; 25] = [
    Command::MoveBeatLeft,
    Command::MoveBeatRight,
    Command::ClearBeats,
    Command::ClearBar,
    Command::MoveToVoice(1),
    Command::RemoveUnusedVoices,
    Command::AppendBar,
    Command::Pitch(oxiguitar_model::Pitch(62)),
    Command::Value(oxiguitar_model::NoteValue::Sixteenth),
    Command::StringUp,
    Command::Fret {
        fret: 3,
        again: false,
    },
    Command::Fret {
        fret: 12,
        again: true,
    },
    Command::Rest,
    Command::Longer,
    Command::Shorter,
    Command::Dot,
    Command::DoubleDot,
    Command::Tie,
    Command::TieBeat,
    Command::DeleteNote,
    Command::DeleteBeats,
    Command::InsertBeat,
    Command::SemitoneUp,
    Command::SemitoneDown,
    Command::StringDown,
];

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = vec![root.join("fixtures/gp/oxiguitar-test-song.gp")];
    let mut refs: Vec<PathBuf> = std::fs::read_dir(root.join("fixtures/reference"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "gp"))
        .collect();
    refs.sort();
    out.extend(refs);
    // The private corpus too, where it is (never in CI).
    for dir in ["corpus/files/tabs", "corpus/files/scores"] {
        if let Ok(entries) = std::fs::read_dir(root.join(dir)) {
            let mut files: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).collect();
            files.sort();
            out.extend(files);
        }
    }
    out
}

fn read(path: &Path) -> Option<Score> {
    oxiguitar_formats::read(&std::fs::read(path).unwrap())
        .ok()
        .map(|r| r.score)
}

/// A random caret move: anywhere in the score, the voice and string too.
fn somewhere(score: &Score, rng: &mut Rng, from: Caret) -> Caret {
    let track = rng.below(score.tracks.len());
    let staves = score.tracks[track].staves.len();
    let c = Caret {
        track,
        staff: rng.below(staves),
        voice: if rng.below(4) == 0 { rng.below(4) } else { 0 },
        bar: rng.below(score.bar_count()),
        beat: rng.below(9),
        string: 1 + rng.below(7) as u8,
    };
    c.clamp(score).unwrap_or(from)
}

/// Runs `steps` random edits; returns the scores after each change.
fn random_edits(score: &mut Score, history: &mut History, seed: u64, steps: usize) -> usize {
    let mut rng = Rng(seed);
    let mut caret = Caret::at_start(score, 0).unwrap();
    let mut changes = 0;
    for _ in 0..steps {
        if rng.below(3) == 0 {
            caret = somewhere(score, &mut rng, caret);
        }
        let command = COMMANDS[rng.below(COMMANDS.len())];
        let selection = (rng.below(5) == 0).then(|| {
            let head = caret.next_beat(score).next_beat(score);
            Selection::between(caret, head)
        });
        if let Some(e) = apply(score, history, caret, selection, command) {
            caret = e.caret;
            changes += 1;
        }
    }
    changes
}

#[test]
fn random_edits_undo_to_the_original_and_redo_to_the_end() {
    let mut total = 0;
    for (i, path) in fixtures().into_iter().enumerate() {
        let Some(original) = read(&path) else {
            continue;
        };
        let mut score = original.clone();
        let mut history = History::new();
        let changes = random_edits(&mut score, &mut history, 0x9e37_79b9 + i as u64, 400);
        total += changes;
        let end = score.clone();
        score.check_structure().unwrap();
        while history.undo(&mut score).is_some() {}
        assert!(score == original, "{}: undo all", path.display());
        while history.redo(&mut score).is_some() {}
        assert!(score == end, "{}: redo all", path.display());
    }
    assert!(total > 2000, "only {total} changes were made");
}

#[test]
fn every_edit_is_saved_and_read_back_equal() {
    let mut compared = 0;
    for (i, path) in fixtures().into_iter().enumerate() {
        let Some(mut score) = read(&path) else {
            continue;
        };
        // Start from the score as a .gp file holds it, which comes back
        // exactly; edits must keep it so.
        let before = oxiguitar_formats::write_gp(&score).unwrap();
        if before.losses.warning_count() > 0 {
            score = oxiguitar_formats::read(&before.data).unwrap().score;
        }
        score = oxiguitar_formats::read(&oxiguitar_formats::write_gp(&score).unwrap().data)
            .unwrap()
            .score;
        let mut history = History::new();
        random_edits(&mut score, &mut history, 0x51ed_270b + i as u64, 400);
        // Deleting what a note slid to leaves a slide to nothing, which a
        // .gp file doesn't keep (the writer names it).
        drop_slides_to_nothing(&mut score);
        let written = oxiguitar_formats::write_gp(&score).unwrap();
        assert_eq!(
            written.losses.warning_count(),
            0,
            "{}: {:?}",
            path.display(),
            written.losses.entries()
        );
        let back = oxiguitar_formats::read(&written.data).unwrap().score;
        if back != score {
            for (t, (a, b)) in score.tracks.iter().zip(&back.tracks).enumerate() {
                for (s, (x, y)) in a.staves.iter().zip(&b.staves).enumerate() {
                    for (n, (p, q)) in x.bars.iter().zip(&y.bars).enumerate() {
                        if p == q {
                            continue;
                        }
                        assert_eq!(p.voices.len(), q.voices.len(), "track {t} bar {n}");
                        for (vi, (v, w)) in p.voices.iter().zip(&q.voices).enumerate() {
                            assert_eq!(
                                v.beats.len(),
                                w.beats.len(),
                                "track {t} bar {n} voice {vi}"
                            );
                            for (bi, (a, b)) in v.beats.iter().zip(&w.beats).enumerate() {
                                assert_eq!(
                                    a,
                                    b,
                                    "{}: track {t} staff {s} bar {n} voice {vi} beat {bi}",
                                    path.display()
                                );
                            }
                        }
                        assert_eq!(p, q, "{}: track {t} staff {s} bar {n}", path.display());
                    }
                }
            }
            panic!("{}: read back differently", path.display());
        }
        compared += 1;
    }
    assert!(compared >= 40, "only {compared} files compared");
}

/// Shift and legato slides from each voice's last note go, as the .gp
/// writer leaves them out.
fn drop_slides_to_nothing(score: &mut Score) {
    use oxiguitar_model::{BeatKind, Slide};
    for t in &mut score.tracks {
        for s in &mut t.staves {
            for voice in 0..4 {
                let last = s
                    .bars
                    .iter_mut()
                    .filter_map(|b| b.voices.get_mut(voice))
                    .flat_map(|v| v.beats.iter_mut())
                    .rfind(|b| b.kind == BeatKind::Notes && !b.notes.is_empty());
                if let Some(b) = last {
                    for n in &mut b.notes {
                        n.effects
                            .slides
                            .retain(|s| !matches!(s, Slide::Shift | Slide::Legato));
                    }
                }
            }
        }
    }
}
