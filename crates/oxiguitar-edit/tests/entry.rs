// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Slice 3's gates for note entry, on the public fixtures: random runs of
//! caret moves and commands, every one undone back to the original and
//! redone to the end; and the edited score saved as .gp reads back equal.
//! Where the private corpus is (`corpus/files/`), its files too.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// A paste; the run puts in what it last copied.
fn paste(times: u8, replace: bool) -> Command {
    Command::Paste(oxiguitar_edit::Paste {
        clip: Arc::new(oxiguitar_edit::Clip::Beats {
            beats: Vec::new(),
            drums: false,
            chords: Vec::new(),
        }),
        times,
        replace,
    })
}

/// A curve as the bend editor makes them: origin, a middle held at one
/// value, destination, on sixtieths of the note.
fn curve(points: &[(i64, i32)]) -> oxiguitar_model::Bend {
    oxiguitar_model::Bend {
        points: points
            .iter()
            .map(|&(sixtieths, cents)| oxiguitar_model::BendPoint {
                offset: oxiguitar_model::Ratio::new(sixtieths, 60).unwrap(),
                cents,
            })
            .collect(),
    }
}

/// Every effect (slice 6), as random runs pick them.
fn effects() -> Vec<Command> {
    use oxiguitar_edit::{Effect, HarmonicKind};
    use oxiguitar_model::{BeatTechnique, GraceKind, Slide, StrokeDirection};
    let mut out: Vec<Command> = [
        Effect::Dead,
        Effect::Ghost,
        Effect::Accent,
        Effect::HeavyAccent,
        Effect::Staccato,
        Effect::HammerPull,
        Effect::LeftHandTap,
        Effect::Slide(Slide::Legato),
        Effect::Slide(Slide::Shift),
        Effect::Slide(Slide::InFromBelow),
        Effect::Slide(Slide::InFromAbove),
        Effect::Slide(Slide::OutDown),
        Effect::Slide(Slide::OutUp),
        Effect::Vibrato,
        Effect::PalmMute,
        Effect::PalmMuteBeat,
        Effect::LetRing,
        Effect::Harmonic(HarmonicKind::Natural),
        Effect::Harmonic(HarmonicKind::Artificial),
        Effect::Harmonic(HarmonicKind::Tapped),
        Effect::Harmonic(HarmonicKind::Pinch),
        Effect::Harmonic(HarmonicKind::Semi),
        Effect::Trill,
        Effect::TremoloPicking,
        Effect::WideVibrato,
        Effect::FadeIn,
        Effect::Technique(BeatTechnique::Tap),
        Effect::Technique(BeatTechnique::Slap),
        Effect::Technique(BeatTechnique::Pop),
        Effect::Brush(StrokeDirection::Down),
        Effect::Brush(StrokeDirection::Up),
        Effect::PickStroke(StrokeDirection::Down),
        Effect::PickStroke(StrokeDirection::Up),
        Effect::Grace(GraceKind::BeforeBeat),
        Effect::Grace(GraceKind::OnBeat),
    ]
    .into_iter()
    .map(Command::Effect)
    .collect();
    out.extend([
        Command::Dynamic(oxiguitar_model::Dynamic::PP),
        Command::Dynamic(oxiguitar_model::Dynamic::FFF),
        Command::Bend(Some(curve(&[(0, 0), (30, 200), (60, 200)]))),
        Command::Bend(Some(curve(&[(0, 0), (15, 100), (45, 100), (60, 0)]))),
        Command::Bend(Some(curve(&[(0, 300), (60, 300)]))),
        Command::Bend(None),
        Command::Whammy(Some(curve(&[(0, 0), (60, -400)]))),
        Command::Whammy(Some(curve(&[(0, 0), (30, -100), (60, 0)]))),
        Command::Whammy(None),
    ]);
    // The Inspector's values (slice 6).
    use oxiguitar_edit::{BeatChange as B, NoteChange as N};
    use oxiguitar_model::{Accent, Harmonic, NoteValue, Stroke, Trill};
    out.extend(
        [
            N::Accent(Some(Accent::Heavy)),
            N::Accent(None),
            N::SlideIn(Some(Slide::InFromAbove)),
            N::SlideOut(Some(Slide::Legato)),
            N::SlideOut(None),
            N::Harmonic(Some(Harmonic::Artificial { semitones: 7 })),
            N::Harmonic(Some(Harmonic::Tapped { fret: 5 })),
            N::Harmonic(Some(Harmonic::Feedback)),
            N::Harmonic(None),
            N::Trill(Some(Trill {
                fret: 2,
                speed: NoteValue::Sixteenth,
            })),
            N::Trill(None),
            N::LeftFinger(Some(0)),
            N::LeftFinger(Some(4)),
            N::RightFinger(Some(2)),
            N::LeftFinger(None),
        ]
        .into_iter()
        .map(Command::Note),
    );
    out.extend(
        [
            B::Tremolo(Some(NoteValue::Eighth)),
            B::Tremolo(Some(NoteValue::Sixteenth)),
            B::Tremolo(None),
            B::Stroke(Some(Stroke {
                direction: StrokeDirection::Down,
                speed: NoteValue::Eighth,
            })),
            B::Stroke(Some(Stroke {
                direction: StrokeDirection::Up,
                speed: NoteValue::SixtyFourth,
            })),
            B::Stroke(None),
            B::PickStroke(Some(StrokeDirection::Up)),
            B::PickStroke(None),
            B::Technique(Some(BeatTechnique::Pop)),
            B::Technique(None),
            B::Grace(Some(GraceKind::OnBeat)),
            B::Grace(None),
        ]
        .into_iter()
        .map(Command::Beat),
    );
    out
}

/// Every command, as random runs pick them.
fn commands() -> Vec<Command> {
    let mut all = effects();
    all.extend(vec![
        Command::Tuplet(oxiguitar_model::Tuplet::TRIPLET),
        Command::Tuplet(oxiguitar_model::Tuplet {
            notes: 5,
            in_time_of: 4,
        }),
        Command::InsertBar,
        Command::DeleteBars,
        Command::TimeSignature(oxiguitar_model::TimeSignature {
            numerator: 7,
            denominator: 8,
        }),
        Command::KeySignature(oxiguitar_model::KeySignature {
            fifths: -3,
            mode: oxiguitar_model::KeyMode::Minor,
        }),
        Command::Clef(oxiguitar_model::Clef::Bass),
        Command::Tempo(97),
        Command::TripletFeel(oxiguitar_model::TripletFeel::Triplet8th),
        Command::RepeatStart,
        Command::RepeatEnd,
        Command::RepeatCount(3),
        Command::AlternateEndings(0b11),
        Command::Direction(oxiguitar_model::Direction::Segno),
        Command::Direction(oxiguitar_model::Direction::DalSegnoAlCoda),
        Command::Section(None),
        Command::DoubleBar,
        Command::FreeTime,
        Command::Section(Some(oxiguitar_model::Marker {
            letter: String::new(),
            text: String::new(),
            color: oxiguitar_model::Color { r: 0, g: 0, b: 0 },
        })),
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
        Command::Section(Some(oxiguitar_model::Marker {
            letter: "B".into(),
            text: "Chorus".into(),
            color: oxiguitar_model::Color::default(),
        })),
        Command::AddTrack,
        Command::DuplicateTrack,
        Command::DeleteTrack,
        Command::MoveTrackUp,
        Command::MoveTrackDown,
        Command::TrackProperties(Box::new(oxiguitar_edit::TrackSettings {
            name: "Retuned".into(),
            short_name: "Rt.".into(),
            instrument: oxiguitar_model::Instrument::default(),
            capo: 1,
            fret_count: 22,
            tuning: oxiguitar_model::tuning::tuning(
                oxiguitar_model::tuning::Pattern::Drop,
                2,
                6,
                false,
            )
            .unwrap(),
            keep_pitches: true,
        })),
        Command::TrackProperties(Box::new(oxiguitar_edit::TrackSettings {
            name: "Seven".into(),
            short_name: String::new(),
            instrument: oxiguitar_model::Instrument {
                midi_program: 30,
                percussion: false,
            },
            capo: 0,
            fret_count: 24,
            tuning: oxiguitar_model::tuning::tuning(
                oxiguitar_model::tuning::Pattern::Standard,
                4,
                7,
                false,
            )
            .unwrap(),
            keep_pitches: false,
        })),
        Command::Transpose(oxiguitar_edit::Transposition {
            amount: 3,
            diatonic: false,
            keep_fingering: true,
            scope: oxiguitar_edit::Scope::Track,
        }),
        Command::Transpose(oxiguitar_edit::Transposition {
            amount: -2,
            diatonic: true,
            keep_fingering: false,
            scope: oxiguitar_edit::Scope::Selection,
        }),
        Command::Transpose(oxiguitar_edit::Transposition {
            amount: -5,
            diatonic: false,
            keep_fingering: false,
            scope: oxiguitar_edit::Scope::All,
        }),
        Command::Cut,
        paste(1, false),
        paste(1, true),
        paste(3, false),
        paste(2, true),
        Command::Bar(Box::new(oxiguitar_edit::BarState {
            time_signature: oxiguitar_model::TimeSignature {
                numerator: 5,
                denominator: 4,
            },
            key_signature: oxiguitar_model::KeySignature::default(),
            clef: oxiguitar_model::Clef::Treble8vb,
            tempo: 133.0,
            triplet_feel: oxiguitar_model::TripletFeel::None,
            repeat_start: true,
            repeat_end: Some(2),
            alternate_endings: 0,
            directions: vec![oxiguitar_model::Direction::ToCoda],
            marker: None,
            double_bar: false,
            free_time: false,
        })),
    ]);
    all
}

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

/// Runs `steps` random edits; returns how many changed the score, and how
/// many of those were effects (slice 6).
fn random_edits(
    score: &mut Score,
    history: &mut History,
    seed: u64,
    steps: usize,
) -> (usize, usize) {
    let mut rng = Rng(seed);
    let mut caret = Caret::at_start(score, 0).unwrap();
    let mut changes = 0;
    let mut effects = 0;
    let all = commands();
    let mut clip: Option<Arc<oxiguitar_edit::Clip>> = None;
    for _ in 0..steps {
        if rng.below(3) == 0 {
            caret = somewhere(score, &mut rng, caret);
        }
        let selection = match rng.below(10) {
            0 | 1 => {
                let head = caret.next_beat(score).next_beat(score);
                Some(Selection::between(caret, head))
            }
            // Whole bars across tracks.
            2 => {
                let last = score.tracks.len() - 1;
                let bars = (
                    caret.bar,
                    (caret.bar + rng.below(3)).min(score.bar_count() - 1),
                );
                Some(Selection::Bars {
                    tracks: (caret.track, (caret.track + rng.below(2)).min(last)),
                    bars,
                })
            }
            _ => None,
        };
        if rng.below(8) == 0 {
            clip = oxiguitar_edit::copy(score, caret, selection)
                .map(Arc::new)
                .or(clip);
        }
        let mut command = all[rng.below(all.len())].clone();
        if let Command::Paste(p) = &mut command {
            match &clip {
                Some(c) => p.clip = c.clone(),
                None => continue,
            }
        }
        let effect = matches!(
            command,
            Command::Effect(_)
                | Command::Dynamic(_)
                | Command::Bend(_)
                | Command::Whammy(_)
                | Command::Note(_)
                | Command::Beat(_)
        );
        if let Some(e) = apply(score, history, caret, selection, command) {
            caret = e.caret;
            changes += 1;
            effects += usize::from(effect);
        }
    }
    (changes, effects)
}

#[test]
fn random_edits_undo_to_the_original_and_redo_to_the_end() {
    let mut total = 0;
    let mut effects = 0;
    for (i, path) in fixtures().into_iter().enumerate() {
        let Some(original) = read(&path) else {
            continue;
        };
        let mut score = original.clone();
        let mut history = History::new();
        let (changes, fx) = random_edits(&mut score, &mut history, 0x9e37_79b9 + i as u64, 600);
        total += changes;
        effects += fx;
        let end = score.clone();
        score.check_structure().unwrap();
        while history.undo(&mut score).is_some() {}
        assert!(score == original, "{}: undo all", path.display());
        while history.redo(&mut score).is_some() {}
        assert!(score == end, "{}: redo all", path.display());
    }
    assert!(total > 2000, "only {total} changes were made");
    assert!(effects > 1000, "only {effects} effects were changed");
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
        random_edits(&mut score, &mut history, 0x51ed_270b + i as u64, 600);
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
            // Elsewhere than the bars: the first line that differs.
            let (x, y) = (format!("{score:#?}"), format!("{back:#?}"));
            let at = x
                .lines()
                .zip(y.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            let around = |s: &str| {
                s.lines()
                    .skip(at.saturating_sub(6))
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            panic!(
                "{}: read back differently at line {at}:\n{}\n--- read back:\n{}",
                path.display(),
                around(&x),
                around(&y)
            );
        }
        compared += 1;
    }
    assert!(compared >= 40, "only {compared} files compared");
}
