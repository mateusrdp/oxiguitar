// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Transposition (M3 slice 5, EDIT-8), three ways in (Mateus, 19 Oct 2026):
//! a track, a selection, or every track.
//!
//! - **Chromatic**: every note moves by the same number of semitones.
//! - **Diatonic**: every note moves by steps of its bar's key, so the music
//!   stays in the key; a note outside the key keeps its alteration from the
//!   key note below it.
//!
//! On strings, a note either **keeps its fingering** (its string, its fret
//! moved; another string where the fret would leave the neck) or gets **new
//! frets**: each beat's notes placed again, highest pitch on the highest
//! string free, at the lowest fret. A note no string can play goes;
//! [`notes_lost`] counts them first. Transposing every track chromatically
//! moves the key signatures too. Drum tracks are never transposed.

use oxiguitar_model::{BeatKind, KeySignature, Note, NotePlace, Pitch, Score, Staff};

use crate::{Caret, Command, Edited, History, Region, Selection};

/// What a transposition does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transposition {
    /// Semitones (chromatic) or steps of the key (diatonic), up or down.
    pub amount: i8,
    /// By steps of the key rather than semitones.
    pub diatonic: bool,
    /// Keep each note's string (moving its fret) rather than finding new
    /// frets.
    pub keep_fingering: bool,
    /// What it applies to.
    pub scope: Scope,
}

/// What a transposition applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The caret's track (its row's right-click menu).
    Track,
    /// The selection, or the caret's beat (the page's right-click menu).
    Selection,
    /// Every track (Tools → Transpose…).
    All,
}

/// Runs a transposition; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let Command::Transpose(t) = command else {
        return None;
    };
    if t.amount == 0 {
        return Some(None);
    }
    let mut work = score.clone();
    let (changed, _) = transpose(&mut work, c, selection, t);
    if !changed {
        return Some(None);
    }
    let regions = match t.scope {
        Scope::All => vec![Region::Whole],
        _ => {
            let tracks = match (t.scope, selection) {
                (Scope::Selection, Some(s)) => s.tracks(),
                _ => (c.track, c.track),
            };
            let bars = score.bar_count();
            (tracks.0..=tracks.1)
                .flat_map(|track| {
                    let staves = score.tracks.get(track).map_or(0, |tr| tr.staves.len());
                    (0..staves).map(move |staff| Region::Bars {
                        track,
                        staff,
                        bars: 0..bars,
                    })
                })
                .collect()
        }
    };
    let label = command.label();
    let touched = history.record(score, label, regions, |s| *s = work);
    Some(Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    }))
}

/// How many notes `t` would leave out: notes no string can play at their
/// new pitch, or pitches past MIDI's range.
pub fn notes_lost(
    score: &Score,
    c: Caret,
    selection: Option<Selection>,
    t: &Transposition,
) -> usize {
    let mut work = score.clone();
    transpose(&mut work, c, selection, t).1
}

/// Transposes `score` in place: whether anything changed, and how many
/// notes went.
fn transpose(
    score: &mut Score,
    c: Caret,
    selection: Option<Selection>,
    t: &Transposition,
) -> (bool, usize) {
    // Which beats of which staves: (track, staff, bar, voice and beats).
    let targets: Vec<crate::Span> = match (t.scope, selection) {
        (Scope::Selection, Some(s)) => s.spans(score),
        (Scope::Selection, None) => vec![crate::Span {
            at: crate::StaffBar {
                track: c.track,
                staff: c.staff,
                bar: c.bar,
            },
            beats: Some((c.voice, c.beat, c.beat)),
        }],
        (Scope::Track, _) => Selection::Bars {
            tracks: (c.track, c.track),
            bars: (0, score.bar_count().saturating_sub(1)),
        }
        .spans(score),
        (Scope::All, _) => match Selection::all(score) {
            Some(s) => s.spans(score),
            None => Vec::new(),
        },
    };
    let keys: Vec<KeySignature> = score.master_bars.iter().map(|b| b.key_signature).collect();
    let mut changed = false;
    let mut lost = 0;
    let mut first_bar: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
    for span in &targets {
        let at = span.at;
        let Some(track) = score.tracks.get_mut(at.track) else {
            continue;
        };
        if track.instrument.percussion {
            continue;
        }
        let Some(staff) = track.staves.get_mut(at.staff) else {
            continue;
        };
        let key = keys.get(at.bar).copied().unwrap_or_default();
        let shape = Staff {
            bars: Vec::new(),
            ..staff.clone()
        };
        let Some(bar) = staff.bars.get_mut(at.bar) else {
            continue;
        };
        for (vi, voice) in bar.voices.iter_mut().enumerate() {
            for (bi, beat) in voice.beats.iter_mut().enumerate() {
                if let Some((v, from, to)) = span.beats
                    && (vi != v || bi < from || bi > to)
                {
                    continue;
                }
                if beat.notes.is_empty() {
                    continue;
                }
                let before = beat.notes.clone();
                let (notes, gone) = moved(&shape, &before, key, t);
                lost += gone;
                if notes != before {
                    changed = true;
                    beat.notes = notes;
                    if beat.notes.is_empty() && beat.kind == BeatKind::Notes {
                        beat.kind = BeatKind::Rest;
                        beat.effects = Default::default();
                    }
                }
            }
        }
        let first = first_bar.entry((at.track, at.staff)).or_insert(at.bar);
        *first = (*first).min(at.bar);
    }
    // Ties follow the notes they continue.
    for ((track, staff), bar) in first_bar {
        if let Some(st) = score
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(staff))
        {
            crate::entry::retie(st, bar);
        }
    }
    // The key moves with every track, chromatically.
    if t.scope == Scope::All && !t.diatonic {
        for mb in &mut score.master_bars {
            let to = key_moved(mb.key_signature, t.amount);
            if to != mb.key_signature {
                mb.key_signature = to;
                changed = true;
            }
        }
    }
    (changed, lost)
}

/// A beat's notes transposed: the notes, and how many went.
fn moved(
    staff: &Staff,
    notes: &[Note],
    key: KeySignature,
    t: &Transposition,
) -> (Vec<Note>, usize) {
    let strings = u8::try_from(staff.string_count()).unwrap_or(0);
    // Each note's new pitch, or `None` past MIDI's range.
    let pitched: Vec<(Note, Option<Pitch>)> = notes
        .iter()
        .map(|n| {
            let p = match n.place {
                NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret),
                NotePlace::Pitched(p) => Some(p),
            };
            (n.clone(), p.and_then(|p| shifted(p, key, t)))
        })
        .collect();
    let mut out: Vec<Note> = Vec::with_capacity(notes.len());
    let mut lost = 0;
    let fret_on = |s: u8, p: Pitch| -> Option<u8> {
        let open = staff.pitch_of(s, 0)?.0;
        p.0.checked_sub(open).filter(|f| *f <= staff.fret_count)
    };
    if strings == 0 {
        for (n, p) in pitched {
            match p {
                Some(p) => out.push(Note {
                    place: NotePlace::Pitched(p),
                    ..n
                }),
                None => lost += 1,
            }
        }
        return (out, lost);
    }
    let mut order: Vec<(Note, Option<Pitch>)> = pitched;
    if !t.keep_fingering {
        // Highest pitch first, so a chord keeps its order across strings.
        order.sort_by_key(|(_, p)| std::cmp::Reverse(p.map_or(0, |p| p.0)));
    }
    for (n, p) in order {
        let Some(p) = p else {
            lost += 1;
            continue;
        };
        let free = |s: u8| !out.iter().any(|o| o.string() == Some(s));
        let kept = match n.place {
            NotePlace::Fretted { string, .. } if t.keep_fingering && free(string) => {
                fret_on(string, p).map(|fret| (string, fret))
            }
            _ => None,
        };
        let place = kept.or_else(|| {
            let mut options: Vec<(u8, u8)> = (1..=strings)
                .filter(|s| free(*s))
                .filter_map(|s| fret_on(s, p).map(|f| (s, f)))
                .collect();
            if t.keep_fingering {
                // Nearest the note's own string.
                let own = n.string().unwrap_or(1);
                options.sort_by_key(|&(s, f)| (s.abs_diff(own), f));
            } else {
                // Where the hand was: the fret nearest the note's old
                // one, then the higher string.
                let was = match n.place {
                    NotePlace::Fretted { fret, .. } => fret,
                    NotePlace::Pitched(_) => 0,
                };
                options.sort_by_key(|&(s, f)| (f.abs_diff(was), s));
            }
            options.first().copied()
        });
        match place {
            Some((string, fret)) => out.push(Note {
                place: NotePlace::Fretted { string, fret },
                ..n
            }),
            None => lost += 1,
        }
    }
    out.sort_by_key(|n| n.string().unwrap_or(u8::MAX));
    (out, lost)
}

/// A pitch moved by `t` in `key`, or `None` past MIDI's range.
fn shifted(p: Pitch, key: KeySignature, t: &Transposition) -> Option<Pitch> {
    let to = if t.diatonic {
        diatonic(i32::from(p.0), key, i32::from(t.amount))
    } else {
        i32::from(p.0) + i32::from(t.amount)
    };
    u8::try_from(to).ok().filter(|p| *p <= 127).map(Pitch)
}

/// `pitch` moved by `steps` steps of `key`'s major scale (a minor key
/// shares its relative major's notes). A note outside the scale keeps its
/// distance from the scale note below it.
fn diatonic(pitch: i32, key: KeySignature, steps: i32) -> i32 {
    const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let tonic = (7 * i32::from(key.fifths)).rem_euclid(12);
    let rel = pitch - tonic;
    let octave = rel.div_euclid(12);
    let pc = rel.rem_euclid(12);
    // The scale note at or below the pitch.
    let degree = MAJOR.iter().rposition(|d| *d <= pc).unwrap_or(0);
    let alteration = pc - MAJOR.get(degree).copied().unwrap_or(0);
    let target = i32::try_from(degree).unwrap_or(0) + steps;
    let (o, d) = (target.div_euclid(7), target.rem_euclid(7));
    let d = usize::try_from(d).unwrap_or(0);
    tonic + 12 * (octave + o) + MAJOR.get(d).copied().unwrap_or(0) + alteration
}

/// A key signature moved by `semitones`: the key a fifth-circle away by as
/// many fifths, spelt with the fewest accidentals (six flats over six
/// sharps).
fn key_moved(key: KeySignature, semitones: i8) -> KeySignature {
    let fifths = i32::from(key.fifths) + 7 * i32::from(semitones);
    let mut f = fifths.rem_euclid(12);
    if f > 5 {
        f -= 12;
    }
    KeySignature {
        fifths: i8::try_from(f).unwrap_or(0),
        mode: key.mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply as run, new_score};
    use oxiguitar_model::KeyMode;

    fn at(beat: usize) -> Caret {
        Caret {
            beat,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    fn up(amount: i8, diatonic: bool, keep_fingering: bool, scope: Scope) -> Command {
        Command::Transpose(Transposition {
            amount,
            diatonic,
            keep_fingering,
            scope,
        })
    }

    fn places(s: &Score) -> Vec<Vec<NotePlace>> {
        s.tracks[0].staves[0].bars[0].voices[0]
            .beats
            .iter()
            .map(|b| b.notes.iter().map(|n| n.place).collect())
            .collect()
    }

    fn song() -> (Score, History) {
        let mut s = new_score();
        let mut h = History::new();
        // C on the B string (fret 1), E on the high E (open), and a chord.
        for (beat, string, fret) in [(0, 2, 1), (1, 1, 0), (2, 3, 2), (2, 2, 3)] {
            let c = Caret { string, ..at(beat) };
            run(
                &mut s,
                &mut h,
                c,
                None,
                Command::Fret { fret, again: false },
            )
            .unwrap();
        }
        (s, h)
    }

    #[test]
    fn chromatic_keeping_the_fingering() {
        let (mut s, mut h) = song();
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            up(2, false, true, Scope::Track),
        )
        .unwrap();
        assert_eq!(
            places(&s)[..2],
            [
                vec![NotePlace::Fretted { string: 2, fret: 3 }],
                vec![NotePlace::Fretted { string: 1, fret: 2 }]
            ]
        );
        // Down a tone and a half: the open E goes to the B string.
        h.undo(&mut s);
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            up(-3, false, true, Scope::Track),
        )
        .unwrap();
        assert_eq!(places(&s)[1], [NotePlace::Fretted { string: 2, fret: 2 }]);
        assert!(
            s.master_bars[0].key_signature.fifths == 0,
            "one track: the key stays"
        );
    }

    #[test]
    fn new_frets_stay_where_the_hand_was() {
        let mut s = new_score();
        let mut h = History::new();
        // A run in fifth position: A and C on the high E, E on the B.
        for (beat, string, fret) in [(0, 1, 5), (1, 1, 8), (2, 2, 5)] {
            let c = Caret { string, ..at(beat) };
            run(
                &mut s,
                &mut h,
                c,
                None,
                Command::Fret { fret, again: false },
            )
            .unwrap();
        }
        // A step of C major up: B, D and F. F stays on the B string at
        // fret 6 rather than going to the first fret of the high E.
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            up(1, true, false, Scope::Track),
        )
        .unwrap();
        assert_eq!(
            places(&s)[..3],
            [
                vec![NotePlace::Fretted { string: 1, fret: 7 }],
                vec![NotePlace::Fretted {
                    string: 1,
                    fret: 10
                }],
                vec![NotePlace::Fretted { string: 2, fret: 6 }],
            ]
        );
    }

    #[test]
    fn diatonic_steps_stay_in_the_key() {
        let (mut s, mut h) = song();
        // C major, a third up: C → E, E → G, chord A + D → C + F.
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            up(2, true, false, Scope::Track),
        )
        .unwrap();
        let pitch = |p: &NotePlace| match *p {
            NotePlace::Fretted { string, fret } => {
                s.tracks[0].staves[0].pitch_of(string, fret).unwrap().0
            }
            NotePlace::Pitched(p) => p.0,
        };
        let pitches: Vec<Vec<u8>> = places(&s)
            .iter()
            .map(|b| b.iter().map(pitch).collect())
            .collect();
        assert_eq!(pitches[0], [64]);
        assert_eq!(pitches[1], [67]);
        let mut chord = pitches[2].clone();
        chord.sort_unstable();
        assert_eq!(chord, [60, 65]);
        // A note outside the key keeps its alteration: C♯ a step up is D♯.
        let key = KeySignature::default();
        assert_eq!(diatonic(61, key, 1), 63);
        // In G major, B a step up is C, F♯ a step up G.
        let g = KeySignature {
            fifths: 1,
            mode: KeyMode::Major,
        };
        assert_eq!(diatonic(59, g, 1), 60);
        assert_eq!(diatonic(66, g, 1), 67);
        assert_eq!(diatonic(60, key, -1), 59);
    }

    #[test]
    fn every_track_moves_the_key_and_a_selection_only_itself() {
        let (mut s, mut h) = song();
        s.master_bars[0].key_signature.fifths = 0;
        run(&mut s, &mut h, at(0), None, up(2, false, false, Scope::All)).unwrap();
        assert_eq!(s.master_bars[0].key_signature.fifths, 2, "C to D");
        assert_eq!(
            key_moved(KeySignature::default(), 1).fifths,
            -5,
            "D♭, not C♯"
        );
        h.undo(&mut s);
        let before = places(&s);
        let sel = Selection::between(at(1), at(1));
        run(
            &mut s,
            &mut h,
            at(1),
            Some(sel),
            up(12, false, true, Scope::Selection),
        )
        .unwrap();
        let after = places(&s);
        assert_eq!(after[0], before[0]);
        assert_eq!(
            after[1],
            [NotePlace::Fretted {
                string: 1,
                fret: 12
            }]
        );
        // Past the neck: counted first, then gone.
        let t = Transposition {
            amount: 40,
            diatonic: false,
            keep_fingering: true,
            scope: Scope::Track,
        };
        assert_eq!(notes_lost(&s, at(0), None, &t), 4);
        h.undo(&mut s);
        assert_eq!(places(&s), before);
    }
}
