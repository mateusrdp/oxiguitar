// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

use super::*;
use oxiguitar_model::NoteValue::{Half, Quarter, Whole};
use oxiguitar_model::{
    Bar, Beat, Duration, Instrument, KeySignature, MasterBar, Note, NoteValue, Pitch,
    TimeSignature, Track, Voice,
};

/// A beat of pitches (MIDI numbers; none for a rest).
fn beat(value: NoteValue, pitches: &[u8]) -> Beat {
    Beat {
        kind: if pitches.is_empty() {
            BeatKind::Rest
        } else {
            BeatKind::Notes
        },
        duration: Duration::new(value),
        notes: pitches
            .iter()
            .map(|p| Note {
                place: NotePlace::Pitched(Pitch(*p)),
                ..Note::fretted(1, 0)
            })
            .collect(),
        ..Beat::default()
    }
}

fn bar(beats: Vec<Beat>) -> Bar {
    Bar {
        voices: vec![Voice { beats }],
        ..Bar::default()
    }
}

/// A guitar track of these bars.
fn track(name: &str, bars: Vec<Bar>) -> Track {
    Track {
        name: name.to_owned(),
        staves: vec![Staff {
            tuning: Staff::standard_guitar_tuning(),
            bars,
            ..Staff::default()
        }],
        ..Track::default()
    }
}

/// A song of one track of quarter notes, four a bar, in the key with
/// `fifths` sharps.
fn song(fifths: i8, bars: &[[u8; 4]]) -> Score {
    Score {
        master_bars: vec![
            MasterBar {
                key_signature: KeySignature {
                    fifths,
                    ..KeySignature::default()
                },
                ..MasterBar::default()
            };
            bars.len()
        ],
        tracks: vec![track(
            "Melody",
            bars.iter()
                .map(|b| bar(b.iter().map(|p| beat(Quarter, &[*p])).collect()))
                .collect(),
        )],
        ..Score::new()
    }
}

fn caret() -> Caret {
    Caret {
        track: 0,
        staff: 0,
        voice: 0,
        bar: 0,
        beat: 0,
        string: 1,
    }
}

const WHOLE: Harmonize = Harmonize {
    sevenths: false,
    inside_bars: false,
    keep: false,
    whole_track: true,
};

/// The chord names over a track's first staff, as (bar, beat, name).
fn names(score: &Score, track: usize) -> Vec<(usize, usize, String)> {
    let t = &score.tracks[track];
    let mut out = Vec::new();
    for (k, bar) in t.staves[0].bars.iter().enumerate() {
        for voice in &bar.voices {
            for (i, b) in voice.beats.iter().enumerate() {
                if let Some(c) = b.chord {
                    out.push((k, i, t.chords[c].name.clone()));
                }
            }
        }
    }
    out
}

fn run(score: &mut Score, h: Harmonize) -> Option<Edited> {
    let mut history = History::default();
    crate::apply(score, &mut history, caret(), None, Command::Harmonize(h))
}

/// A tune that spells out C, F, G and C, a bar each.
const TUNE: [[u8; 4]; 4] = [
    [60, 64, 67, 64],
    [65, 69, 72, 69],
    [67, 71, 74, 71],
    [72, 67, 64, 60],
];

#[test]
fn a_tune_gets_the_chords_it_spells_out() {
    let mut s = song(0, &TUNE);
    run(&mut s, WHOLE).unwrap();
    assert_eq!(
        names(&s, 0),
        [
            (0, 0, "C".to_owned()),
            (1, 0, "F".to_owned()),
            (2, 0, "G".to_owned()),
            (3, 0, "C".to_owned())
        ]
    );
    // A guitar gets shapes it can play: the open C chord.
    let c = s.tracks[0].chords.iter().find(|d| d.name == "C").unwrap();
    assert_eq!(c.frets.len(), 6);
    assert_eq!(c.first_fret, 1);
    assert!(c.frets.iter().flatten().count() >= 3);
}

#[test]
fn the_same_tune_in_another_key_gets_the_same_chords_there() {
    // Up a minor third, under three flats: E flat, A flat, B flat.
    let up: Vec<[u8; 4]> = TUNE.iter().map(|b| b.map(|p| p + 3)).collect();
    let mut s = song(-3, &up);
    run(&mut s, WHOLE).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["Eb", "Ab", "Bb", "Eb"]);
    // Down a minor third, under three sharps: A, D, E.
    let down: Vec<[u8; 4]> = TUNE.iter().map(|b| b.map(|p| p - 3)).collect();
    let mut s = song(3, &down);
    run(&mut s, WHOLE).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["A", "D", "E", "A"]);
}

#[test]
fn a_minor_tune_is_harmonized_under_its_relative_major_s_signature() {
    // A minor, E (the dominant, with its G sharp), A minor: no sharps.
    let mut s = song(
        0,
        &[
            [57, 60, 64, 60],
            [62, 65, 69, 65],
            [64, 68, 71, 68],
            [69, 64, 60, 57],
        ],
    );
    run(&mut s, WHOLE).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["Am", "Dm", "E", "Am"]);
}

#[test]
fn a_chord_changes_inside_a_bar_only_when_asked() {
    // C then F in one bar, G then C in the next, spelled out in eighths.
    let half = |p: [u8; 4]| p.map(|p| beat(NoteValue::Eighth, &[p]));
    let tune = || {
        let mut s = song(0, &TUNE[..2]);
        s.tracks[0].staves[0].bars = vec![
            bar([half([60, 64, 67, 64]), half([65, 69, 72, 69])].concat()),
            bar([half([67, 71, 74, 71]), half([72, 67, 64, 60])].concat()),
        ];
        s
    };
    let mut s = tune();
    run(&mut s, WHOLE).unwrap();
    assert!(names(&s, 0).iter().all(|n| n.1 == 0), "{:?}", names(&s, 0));
    let mut s = tune();
    run(
        &mut s,
        Harmonize {
            inside_bars: true,
            ..WHOLE
        },
    )
    .unwrap();
    assert_eq!(
        names(&s, 0),
        [
            (0, 0, "C".to_owned()),
            (0, 4, "F".to_owned()),
            (1, 0, "G".to_owned()),
            (1, 4, "C".to_owned())
        ]
    );
}

#[test]
fn a_chord_is_written_only_where_a_beat_starts() {
    // A whole note has no beat at the half bar: no change there.
    let mut s = song(0, &TUNE);
    s.tracks[0].staves[0].bars[1] = bar(vec![beat(Whole, &[65])]);
    run(
        &mut s,
        Harmonize {
            inside_bars: true,
            ..WHOLE
        },
    )
    .unwrap();
    assert!(names(&s, 0).iter().filter(|n| n.0 == 1).all(|n| n.1 == 0));
}

#[test]
fn every_pitched_track_is_heard() {
    // The melody holds a G; the bass says which chord it belongs to.
    let melody = |s: &mut Score| {
        for b in &mut s.tracks[0].staves[0].bars {
            *b = bar(vec![beat(Whole, &[67])]);
        }
    };
    let with_bass = |roots: [u8; 4]| {
        let mut s = song(0, &TUNE);
        melody(&mut s);
        s.tracks.push(track(
            "Bass",
            roots
                .iter()
                .map(|p| bar(vec![beat(Half, &[*p]), beat(Half, &[*p + 7])]))
                .collect(),
        ));
        // Drums never count, whatever their notes.
        s.tracks.push(Track {
            instrument: Instrument {
                midi_program: 0,
                percussion: true,
            },
            ..track("Drums", vec![bar(vec![beat(Whole, &[61, 66])]); 4])
        });
        run(&mut s, WHOLE).unwrap();
        assert!(names(&s, 1).is_empty(), "chords go on the caret's track");
        names(&s, 0).into_iter().map(|n| n.2).collect::<Vec<_>>()
    };
    assert_eq!(with_bass([36, 40, 43, 36]), ["C", "Em", "G", "C"]);
    assert_eq!(with_bass([40, 36, 40, 43]), ["Em", "C", "Em", "G"]);
}

#[test]
fn sevenths_come_only_when_asked() {
    // A second guitar plays G7 in the third bar.
    let band = || {
        let mut s = song(0, &TUNE);
        let chords = [[48, 52, 55], [53, 57, 60], [55, 59, 65], [48, 52, 55]];
        s.tracks.push(track(
            "Rhythm",
            chords.iter().map(|c| bar(vec![beat(Whole, c)])).collect(),
        ));
        s
    };
    let mut s = band();
    run(&mut s, WHOLE).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["C", "F", "G", "C"]);
    let mut s = band();
    run(
        &mut s,
        Harmonize {
            sevenths: true,
            inside_bars: true,
            ..WHOLE
        },
    )
    .unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["C", "F", "G7", "C"]);
}

#[test]
fn old_chords_are_replaced_or_kept() {
    let old = ChordDiagram {
        name: "Am7".to_owned(),
        first_fret: 1,
        frets: vec![Some(0), Some(1), Some(0), Some(2), Some(0), None],
    };
    let with_old = || {
        let mut s = song(0, &TUNE);
        s.tracks[0].chords.push(old.clone());
        // On the second beat of bar 2, where the tool never writes.
        s.tracks[0].staves[0].bars[1].voices[0].beats[1].chord = Some(0);
        s
    };
    let mut s = with_old();
    run(&mut s, WHOLE).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["C", "F", "G", "C"]);
    assert!(
        s.tracks[0].chords.contains(&old),
        "the diagram stays listed"
    );

    let mut s = with_old();
    run(
        &mut s,
        Harmonize {
            keep: true,
            ..WHOLE
        },
    )
    .unwrap();
    assert_eq!(
        names(&s, 0),
        [
            (0, 0, "C".to_owned()),
            (1, 1, "Am7".to_owned()),
            (2, 0, "G".to_owned()),
            (3, 0, "C".to_owned())
        ]
    );
    // Again with the chords kept: nothing left to do.
    assert!(
        run(
            &mut s,
            Harmonize {
                keep: true,
                ..WHOLE
            }
        )
        .is_none()
    );
}

#[test]
fn it_is_one_change_and_only_in_the_bars_asked_for() {
    let mut s = song(0, &TUNE);
    let before = s.clone();
    let mut history = History::default();
    let selection = Selection::Bars {
        tracks: (0, 0),
        bars: (1, 2),
    };
    let h = Harmonize {
        whole_track: false,
        ..WHOLE
    };
    crate::apply(
        &mut s,
        &mut history,
        caret(),
        Some(selection),
        Command::Harmonize(h),
    )
    .unwrap();
    let found: Vec<(usize, String)> = names(&s, 0).into_iter().map(|n| (n.0, n.2)).collect();
    assert_eq!(found, [(1, "F".to_owned()), (2, "G".to_owned())]);
    assert_eq!(history.undo_label(), Some("Harmonize"));
    history.undo(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn rests_before_and_after_the_music_get_no_chord() {
    let mut s = song(0, &TUNE);
    let rest = bar(vec![beat(Whole, &[])]);
    s.tracks[0].staves[0].bars[0] = rest.clone();
    s.tracks[0].staves[0].bars[3] = rest;
    run(&mut s, WHOLE).unwrap();
    let found: Vec<(usize, String)> = names(&s, 0).into_iter().map(|n| (n.0, n.2)).collect();
    assert_eq!(found, [(1, "F".to_owned()), (2, "G".to_owned())]);
}

#[test]
fn nothing_to_harmonize() {
    // No notes.
    let mut s = song(0, &TUNE);
    for b in &mut s.tracks[0].staves[0].bars {
        *b = bar(vec![beat(Whole, &[])]);
    }
    assert!(run(&mut s, WHOLE).is_none());
    assert!(harmonize_report(&s, caret(), None, &WHOLE).is_none());
    // A drum track takes no chords.
    let mut s = song(0, &TUNE);
    s.tracks[0].instrument.percussion = true;
    assert!(run(&mut s, WHOLE).is_none());
}

#[test]
fn the_report_names_the_key_and_a_better_one() {
    let s = song(0, &TUNE);
    assert_eq!(
        harmonize_report(&s, caret(), None, &WHOLE).unwrap(),
        "4 chords found for bars 1–4 of Melody, in C major or A minor (the key signature)."
    );
    // The same tune in E, left under no sharps as tabs often are.
    let mut bars: Vec<[u8; 4]> = TUNE.iter().map(|b| b.map(|p| p + 4)).collect();
    bars.extend(bars.clone());
    let s = song(0, &bars);
    let report = harmonize_report(&s, caret(), None, &WHOLE).unwrap();
    assert!(
        report.ends_with(
            "The notes fit E major or C# minor better: if the key signature is wrong, \
             run Infer Key Signatures and harmonize again."
        ),
        "{report}"
    );
}

#[test]
fn other_time_signatures() {
    // 3/4: one cell a bar even with changes inside bars; 6/8: two.
    let mut s = song(0, &TUNE);
    for (m, b) in s
        .master_bars
        .iter_mut()
        .zip(&mut s.tracks[0].staves[0].bars)
    {
        m.time_signature = TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        b.voices[0].beats.pop();
    }
    let h = Harmonize {
        inside_bars: true,
        ..WHOLE
    };
    run(&mut s, h).unwrap();
    let found: Vec<String> = names(&s, 0).into_iter().map(|n| n.2).collect();
    assert_eq!(found, ["C", "F", "G", "C"]);
}

#[test]
fn roots_are_spelled_as_the_key_spells_them() {
    let name = |root: u8, kind: Kind, fifths: i8| NOTE_NAMES[root_name(root, kind, fifths)];
    // F major's fourth degree is B flat; E major's third is G sharp.
    assert_eq!(name(10, Kind::Major, -1), "Bb");
    assert_eq!(name(8, Kind::Minor, 4), "G#");
    // Between the degrees: flats (C's flat seventh), but a sharp under a
    // diminished chord (A minor's leading tone) and for the raised fourth.
    assert_eq!(name(10, Kind::Major, 0), "Bb");
    assert_eq!(name(8, Kind::Major, 0), "Ab");
    assert_eq!(name(8, Kind::Diminished, 0), "G#");
    assert_eq!(name(6, Kind::HalfDiminished, 0), "F#");
    // Past the names the builder has (G flat major's fourth, C flat).
    assert_eq!(name(11, Kind::Major, -6), "B");
    for fifths in -7..=7 {
        for root in 0..12 {
            for kind in KINDS {
                let chord = chord_of(
                    12 * KINDS.iter().position(|k| *k == kind).unwrap() + root,
                    fifths,
                );
                assert_eq!(usize::from(chord.root_class()), root);
                // Every chord the tool writes is one the builder reads back.
                assert_eq!(Chord::parse(&chord.name()), Some(chord));
                assert_eq!(Kind::of(&chord), Some(kind));
            }
        }
    }
}

#[test]
fn every_chord_has_a_shape_on_a_guitar() {
    let staff = Staff {
        tuning: Staff::standard_guitar_tuning(),
        ..Staff::default()
    };
    for state in 0..12 * KINDS.len() {
        let d = diagram(&chord_of(state, 0), &staff);
        assert!(d.frets.iter().flatten().count() >= 3, "{}", d.name);
        assert!(d.first_fret >= 1);
        assert!(
            d.frets
                .iter()
                .flatten()
                .all(|f| *f == 0 || *f >= d.first_fret),
            "{}",
            d.name
        );
    }
    // No strings: the name alone.
    let d = diagram(&chord_of(0, 0), &Staff::default());
    assert!(d.frets.is_empty());
}
