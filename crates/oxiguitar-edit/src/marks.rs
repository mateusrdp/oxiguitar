// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The rest of the palette (v0.17, docs/design/palette.md): the marks that
//! are not switched on and off like an effect.
//!
//! - [`Command::Fermata`]: a fermata at the caret beat's moment in its
//!   master bar (or at each selected beat's), held by every track.
//! - [`Command::Simile`]: a repeat sign on the caret's bar, which takes a
//!   copy of the bar (or two bars) it repeats.
//! - [`Command::Pedal`]: the sustain pedal down and up.
//! - [`Command::Spell`] and [`Command::Respell`]: a note spelled with an
//!   accidental, or the next way to spell it.
//! - [`Command::NextFinger`]: the next finger of either hand, as Guitar Pro's
//!   fingering tools step through them.

use oxiguitar_model::{
    Bar, BeatKind, Fermata, FermataKind, KeySignature, Note, NotePlace, Pedal, Pitch, Ratio, Score,
    Simile, Spelling, Staff,
};

use crate::{Caret, Command, Edited, History, Region, Selection, onsets};

/// Runs a mark of this module; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let keys: Vec<KeySignature> = score.master_bars.iter().map(|m| m.key_signature).collect();
    let keys = keys.as_slice();
    Some(match command {
        Command::Fermata(kind) => fermata(score, history, c, selection, *kind, command.label()),
        Command::Simile(kind) => simile(score, history, c, *kind, command.label()),
        Command::SystemLayout { track, layout } => {
            system_layout(score, history, c, (*track, layout), command.label())
        }
        Command::Pedal => staff_edit(
            score,
            history,
            c,
            selection,
            command,
            |bars, first, t, _| pedal(bars, first, t, c),
        ),
        Command::Spell(alter) => staff_edit(
            score,
            history,
            c,
            selection,
            command,
            |bars, first, t, staff| {
                notes(
                    bars,
                    first,
                    t,
                    (c, selection),
                    (staff, keys),
                    |n, written, _| {
                        let to = Spelling::with_alter(written, *alter)?;
                        (n.spelling != Some(to)).then(|| n.spelling = Some(to))
                    },
                )
            },
        ),
        Command::Respell => staff_edit(
            score,
            history,
            c,
            selection,
            command,
            |bars, first, t, staff| {
                notes(
                    bars,
                    first,
                    t,
                    (c, selection),
                    (staff, keys),
                    |n, written, key| {
                        let all = Spelling::all(written);
                        let now = n
                            .spelling
                            .filter(|s| s.spells(written))
                            .unwrap_or_else(|| Spelling::automatic(written, key));
                        let at = all.iter().position(|s| *s == now).unwrap_or(0);
                        let next = *all.get((at + 1) % all.len().max(1))?;
                        (next != now).then(|| n.spelling = Some(next))
                    },
                )
            },
        ),
        Command::NextFinger { right } => {
            staff_edit(
                score,
                history,
                c,
                selection,
                command,
                |bars, first, t, staff| {
                    notes(bars, first, t, (c, selection), (staff, keys), |n, _, _| {
                        let f = if *right {
                            &mut n.effects.right_finger
                        } else {
                            &mut n.effects.left_finger
                        };
                        // None, then each finger, then none again.
                        *f = match *f {
                            None => Some(if *right { 0 } else { 1 }),
                            Some(4) => None,
                            Some(x) => Some(x + 1),
                        };
                        Some(())
                    })
                },
            )
        }
        _ => return None,
    })
}

/// Edits the caret's staff's bars that the targets are in, as one change.
/// `edit` gets the bars (from bar `first`), the targets and the staff, and
/// says whether it did anything.
fn staff_edit(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
    edit: impl FnOnce(&mut [Bar], usize, &[(usize, usize)], &Staff) -> bool,
) -> Option<Edited> {
    let staff = crate::entry::staff_of(score, c)?;
    let targets = crate::entry::targets(score, c, selection);
    let (&(first, _), &(last, _)) = (targets.first()?, targets.last()?);
    let mut bars: Vec<Bar> = staff.bars.get(first..=last)?.to_vec();
    if !edit(&mut bars, first, &targets, staff) {
        return None;
    }
    crate::effects::unmark_similes(staff.bars.get(first..=last)?, &mut bars);
    if staff.bars.get(first..=last).is_some_and(|old| old == bars) {
        return None;
    }
    let region = Region::Bars {
        track: c.track,
        staff: c.staff,
        bars: first..last + 1,
    };
    let (track, st) = (c.track, c.staff);
    let touched = history.record(score, command.label(), vec![region], |s| {
        if let Some(dst) = s
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(st))
            .and_then(|staff| staff.bars.get_mut(first..=last))
        {
            dst.clone_from_slice(&bars);
        }
    });
    Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    })
}

/// The sustain pedal: at the caret, none → down → up → none; on a
/// selection, down on its first beat and up on its last.
fn pedal(bars: &mut [Bar], first: usize, targets: &[(usize, usize)], c: Caret) -> bool {
    fn beat(
        bars: &mut [Bar],
        first: usize,
        c: Caret,
        at: (usize, usize),
    ) -> Option<&mut oxiguitar_model::Beat> {
        bars.get_mut(at.0.checked_sub(first)?)?
            .voices
            .get_mut(c.voice)?
            .beats
            .get_mut(at.1)
            .filter(|b| b.kind != BeatKind::Empty)
    }
    match targets {
        [] => false,
        [one] => beat(bars, first, c, *one).is_some_and(|b| {
            b.effects.pedal = match b.effects.pedal {
                None => Some(Pedal::Down),
                Some(Pedal::Down) => Some(Pedal::Up),
                Some(Pedal::Up) => None,
            };
            true
        }),
        [start, .., end] => {
            let down = beat(bars, first, c, *start).map(|b| b.effects.pedal = Some(Pedal::Down));
            let up = beat(bars, first, c, *end).map(|b| b.effects.pedal = Some(Pedal::Up));
            down.is_some() || up.is_some()
        }
    }
}

/// Runs `edit` on each target note: the note on the caret's string, or
/// every note of the beat on a staff without strings or with a selection.
/// `edit` gets the note, its written pitch class and the key in force, and
/// says whether it changed the note.
fn notes(
    bars: &mut [Bar],
    first: usize,
    targets: &[(usize, usize)],
    (c, selection): (Caret, Option<Selection>),
    (staff, keys): (&Staff, &[KeySignature]),
    mut edit: impl FnMut(&mut Note, Pitch, KeySignature) -> Option<()>,
) -> bool {
    let all = selection.is_some() || staff.string_count() == 0;
    let mut any = false;
    for &(bar, beat) in targets {
        let key = keys.get(bar).copied().unwrap_or_default();
        let Some(b) = bars
            .get_mut(bar.saturating_sub(first))
            .and_then(|b| b.voices.get_mut(c.voice))
            .and_then(|v| v.beats.get_mut(beat))
        else {
            continue;
        };
        for n in &mut b.notes {
            if !all && n.string() != Some(c.string) {
                continue;
            }
            let sounding = match n.place {
                NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret),
                NotePlace::Pitched(p) => Some(p),
            };
            let Some(sounding) = sounding else {
                continue;
            };
            let written = (i32::from(sounding.0) + i32::from(staff.transposition)).rem_euclid(12);
            let Ok(written) = u8::try_from(written) else {
                continue;
            };
            any |= edit(n, Pitch(written), key).is_some();
        }
    }
    any
}

/// A fermata at each target beat's moment in its master bar: on (medium)
/// where any is missing, else off; with `kind`, that kind at each.
fn fermata(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    kind: Option<FermataKind>,
    label: &str,
) -> Option<Edited> {
    let staff = crate::entry::staff_of(score, c)?;
    let targets = crate::entry::targets(score, c, selection);
    let mut at: Vec<(usize, Ratio)> = Vec::new();
    for &(bar, beat) in &targets {
        let voice = staff.bars.get(bar)?.voices.get(c.voice)?;
        if voice.beats.get(beat)?.grace.is_some() {
            continue;
        }
        let onset = *onsets(voice).get(beat)?;
        if !at.contains(&(bar, onset)) {
            at.push((bar, onset));
        }
    }
    let (first, last) = (at.first()?.0, at.last()?.0);
    let mut masters = score.master_bars.get(first..=last)?.to_vec();
    let has = |m: &[oxiguitar_model::MasterBar], (bar, pos): (usize, Ratio)| {
        m.get(bar - first)
            .is_some_and(|m| m.fermatas.iter().any(|f| f.position == pos))
    };
    let off = kind.is_none() && at.iter().all(|&a| has(&masters, a));
    for &(bar, position) in &at {
        let Some(m) = masters.get_mut(bar - first) else {
            continue;
        };
        m.fermatas.retain(|f| f.position != position);
        if !off {
            m.fermatas.push(Fermata {
                position,
                kind: kind.unwrap_or_default(),
                length: 0.0,
            });
            m.fermatas.sort_by_key(|f| f.position);
        }
    }
    if score.master_bars.get(first..=last)? == masters.as_slice() {
        return None;
    }
    let touched = history.record(
        score,
        label,
        vec![Region::MasterBars(first..last + 1)],
        |s| {
            if let Some(dst) = s.master_bars.get_mut(first..=last) {
                dst.clone_from_slice(&masters);
            }
        },
    );
    Some(Edited { touched, caret: c })
}

/// A repeat sign on the caret's bar: one bar repeating the bar before, or
/// (`FirstOfDouble`) two bars repeating the two before them. Each bar takes
/// a copy of the bar it repeats. On a bar that has one already, the sign
/// (of both bars of a double) is taken off and the music stays.
fn simile(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    kind: Simile,
    label: &str,
) -> Option<Edited> {
    let staff = crate::entry::staff_of(score, c)?;
    let here = staff.bars.get(c.bar)?;
    let (first, bars): (usize, Vec<Bar>) = match here.simile {
        Some(Simile::Simple) => (c.bar, vec![unmarked(here)]),
        Some(Simile::FirstOfDouble) => (
            c.bar,
            vec![unmarked(here), unmarked(staff.bars.get(c.bar + 1)?)],
        ),
        Some(Simile::SecondOfDouble) => (
            c.bar.checked_sub(1)?,
            vec![unmarked(staff.bars.get(c.bar - 1)?), unmarked(here)],
        ),
        None if kind == Simile::Simple => {
            let source = staff.bars.get(c.bar.checked_sub(1)?)?;
            (c.bar, vec![copy(source, here, Simile::Simple)])
        }
        None => {
            let a = staff.bars.get(c.bar.checked_sub(2)?)?;
            let b = staff.bars.get(c.bar - 1)?;
            let next = staff.bars.get(c.bar + 1)?;
            (
                c.bar,
                vec![
                    copy(a, here, Simile::FirstOfDouble),
                    copy(b, next, Simile::SecondOfDouble),
                ],
            )
        }
    };
    let last = first + bars.len() - 1;
    if staff.bars.get(first..=last)? == bars.as_slice() {
        return None;
    }
    let region = Region::Bars {
        track: c.track,
        staff: c.staff,
        bars: first..last + 1,
    };
    let (track, st) = (c.track, c.staff);
    let touched = history.record(score, label, vec![region], |s| {
        if let Some(dst) = s
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(st))
            .and_then(|staff| staff.bars.get_mut(first..=last))
        {
            dst.clone_from_slice(&bars);
        }
    });
    Some(Edited {
        touched,
        caret: Caret { beat: 0, ..c }.clamp(score).unwrap_or(c),
    })
}

/// Bars per system: a track's (`Some`) or the score's.
fn system_layout(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    (track, layout): (Option<usize>, &[u8]),
    label: &str,
) -> Option<Edited> {
    let now = match track {
        Some(t) => &score.tracks.get(t)?.layout,
        None => &score.layout,
    };
    if now.as_slice() == layout {
        return None;
    }
    let region = track.map_or(Region::Info, Region::Track);
    let layout = layout.to_vec();
    let touched = history.record(score, label, vec![region], |s| match track {
        Some(t) => {
            if let Some(tr) = s.tracks.get_mut(t) {
                tr.layout = layout;
            }
        }
        None => s.layout = layout,
    });
    Some(Edited { touched, caret: c })
}

/// `bar` without its sign.
fn unmarked(bar: &Bar) -> Bar {
    Bar {
        simile: None,
        ..bar.clone()
    }
}

/// The music of `source` in place of `bar`'s, keeping `bar`'s clef, marked
/// `kind`.
fn copy(source: &Bar, bar: &Bar, kind: Simile) -> Bar {
    Bar {
        clef: bar.clef,
        voices: source.voices.clone(),
        simile: Some(kind),
    }
}

/// Takes off every note's spelling that is the one OxiGuitar would work
/// out itself, or that no longer spells the note: a file's spellings are
/// kept only where they differ (Mateus, 8 Nov 2026), so editing and
/// transposing go on working spellings out. Run when a file opens; returns
/// how many were taken off.
pub fn plain_spellings(score: &mut Score) -> usize {
    let keys: Vec<KeySignature> = score.master_bars.iter().map(|m| m.key_signature).collect();
    let mut dropped = 0;
    for track in &mut score.tracks {
        let drums = track.instrument.percussion;
        for staff in &mut track.staves {
            let (tuning, capo, transposition) =
                (staff.tuning.clone(), staff.capo, staff.transposition);
            let probe = Staff {
                tuning,
                capo,
                ..Staff::default()
            };
            for (bar, b) in staff.bars.iter_mut().enumerate() {
                let key = keys.get(bar).copied().unwrap_or_default();
                for n in b
                    .voices
                    .iter_mut()
                    .flat_map(|v| &mut v.beats)
                    .flat_map(|b| &mut b.notes)
                {
                    let Some(own) = n.spelling else {
                        continue;
                    };
                    let sounding = match n.place {
                        NotePlace::Fretted { string, fret } => probe.pitch_of(string, fret),
                        NotePlace::Pitched(p) => Some(p),
                    };
                    let written = sounding.and_then(|p| {
                        u8::try_from((i32::from(p.0) + i32::from(transposition)).rem_euclid(12))
                            .ok()
                    });
                    let keep = !drums
                        && written.is_some_and(|w| {
                            own.spells(Pitch(w)) && own != Spelling::automatic(Pitch(w), key)
                        });
                    if !keep {
                        n.spelling = None;
                        dropped += 1;
                    }
                }
            }
        }
    }
    dropped
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use oxiguitar_model::{Beat, Duration, NoteValue, Voice};

    use super::*;
    use crate::{apply, new_score};

    fn at(bar: usize, beat: usize, string: u8) -> Caret {
        Caret {
            track: 0,
            staff: 0,
            voice: 0,
            bar,
            beat,
            string,
        }
    }

    /// Three bars of four quarter notes, each beat's note on string 1 at
    /// the bar's number (fret 1, 2, 3).
    fn song() -> (Score, History) {
        let mut s = new_score();
        let staff = &mut s.tracks[0].staves[0];
        staff.bars.clear();
        for bar in 0..3u8 {
            let beats = (0..4)
                .map(|_| Beat {
                    duration: Duration::new(NoteValue::Quarter),
                    notes: vec![Note::fretted(1, bar + 1)],
                    ..Beat::default()
                })
                .collect();
            staff.bars.push(Bar {
                voices: vec![Voice { beats }],
                ..Bar::default()
            });
        }
        let master = s.master_bars[0].clone();
        s.master_bars = vec![master; 3];
        (s, History::default())
    }

    fn fret(s: &Score, bar: usize) -> u8 {
        match s.tracks[0].staves[0].bars[bar].voices[0].beats[0].notes[0].place {
            NotePlace::Fretted { fret, .. } => fret,
            NotePlace::Pitched(_) => 0,
        }
    }

    #[test]
    fn a_fermata_sits_at_its_beats_moment_and_comes_off_again() {
        let (mut s, mut h) = song();
        apply(&mut s, &mut h, at(1, 2, 1), None, Command::Fermata(None)).unwrap();
        let f = &s.master_bars[1].fermatas;
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].position, Ratio::new(1, 2).unwrap());
        assert_eq!(f[0].kind, FermataKind::Medium);
        apply(
            &mut s,
            &mut h,
            at(1, 2, 1),
            None,
            Command::Fermata(Some(FermataKind::Long)),
        )
        .unwrap();
        assert_eq!(s.master_bars[1].fermatas[0].kind, FermataKind::Long);
        apply(&mut s, &mut h, at(1, 2, 1), None, Command::Fermata(None)).unwrap();
        assert!(s.master_bars[1].fermatas.is_empty());
        h.undo(&mut s).unwrap();
        assert_eq!(s.master_bars[1].fermatas[0].kind, FermataKind::Long);
    }

    #[test]
    fn a_repeat_sign_copies_the_bar_before_and_comes_off_keeping_it() {
        let (mut s, mut h) = song();
        apply(
            &mut s,
            &mut h,
            at(1, 0, 1),
            None,
            Command::Simile(Simile::Simple),
        )
        .unwrap();
        assert_eq!(s.tracks[0].staves[0].bars[1].simile, Some(Simile::Simple));
        assert_eq!(fret(&s, 1), 1);
        apply(
            &mut s,
            &mut h,
            at(1, 0, 1),
            None,
            Command::Simile(Simile::Simple),
        )
        .unwrap();
        assert_eq!(s.tracks[0].staves[0].bars[1].simile, None);
        assert_eq!(fret(&s, 1), 1);
        // Editing a note of a marked bar takes its sign off.
        h.undo(&mut s).unwrap();
        apply(&mut s, &mut h, at(1, 0, 1), None, Command::Pedal).unwrap();
        assert_eq!(s.tracks[0].staves[0].bars[1].simile, None);
    }

    #[test]
    fn a_double_repeat_sign_copies_two_bars() {
        let (mut s, mut h) = song();
        // Needs two bars before and one after: none at bar 2 of 3.
        assert!(
            apply(
                &mut s,
                &mut h,
                at(2, 0, 1),
                None,
                Command::Simile(Simile::FirstOfDouble)
            )
            .is_none()
        );
        let (mut s, mut h) = song();
        let staff = &mut s.tracks[0].staves[0];
        staff.bars.push(staff.bars[2].clone());
        let m = s.master_bars[0].clone();
        s.master_bars.push(m);
        apply(
            &mut s,
            &mut h,
            at(2, 0, 1),
            None,
            Command::Simile(Simile::FirstOfDouble),
        )
        .unwrap();
        let bars = &s.tracks[0].staves[0].bars;
        assert_eq!(bars[2].simile, Some(Simile::FirstOfDouble));
        assert_eq!(bars[3].simile, Some(Simile::SecondOfDouble));
        assert_eq!((fret(&s, 2), fret(&s, 3)), (1, 2));
        // Off from the second bar takes both signs off.
        apply(
            &mut s,
            &mut h,
            at(3, 0, 1),
            None,
            Command::Simile(Simile::FirstOfDouble),
        )
        .unwrap();
        let bars = &s.tracks[0].staves[0].bars;
        assert_eq!((bars[2].simile, bars[3].simile), (None, None));
    }

    #[test]
    fn the_pedal_steps_down_up_and_off() {
        let (mut s, mut h) = song();
        let pedal = |s: &Score| {
            s.tracks[0].staves[0].bars[0].voices[0].beats[1]
                .effects
                .pedal
        };
        for want in [Some(Pedal::Down), Some(Pedal::Up), None] {
            apply(&mut s, &mut h, at(0, 1, 1), None, Command::Pedal).unwrap();
            assert_eq!(pedal(&s), want);
        }
        let sel = Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (0, 0),
            to: (1, 3),
        };
        apply(&mut s, &mut h, at(0, 0, 1), Some(sel), Command::Pedal).unwrap();
        let bars = &s.tracks[0].staves[0].bars;
        assert_eq!(bars[0].voices[0].beats[0].effects.pedal, Some(Pedal::Down));
        assert_eq!(bars[1].voices[0].beats[3].effects.pedal, Some(Pedal::Up));
    }

    #[test]
    fn accidentals_spell_the_note_and_change_accidental_steps_through() {
        let (mut s, mut h) = song();
        // String 1 fret 4: G sharp 4 (written G sharp 5 on the guitar's
        // clef; the pitch class is what counts).
        s.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0] = Note::fretted(1, 4);
        let spelling =
            |s: &Score| s.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0].spelling;
        apply(&mut s, &mut h, at(0, 0, 1), None, Command::Spell(-1)).unwrap();
        assert_eq!(spelling(&s), Some(Spelling { step: 5, alter: -1 }));
        // No natural spells G sharp: nothing to do.
        assert!(apply(&mut s, &mut h, at(0, 0, 1), None, Command::Spell(0)).is_none());
        // A flat, then G sharp again (the two ways to spell it).
        apply(&mut s, &mut h, at(0, 0, 1), None, Command::Respell).unwrap();
        assert_eq!(spelling(&s), Some(Spelling { step: 4, alter: 1 }));
        apply(&mut s, &mut h, at(0, 0, 1), None, Command::Respell).unwrap();
        assert_eq!(spelling(&s), Some(Spelling { step: 5, alter: -1 }));
    }

    #[test]
    fn only_spellings_of_our_own_choosing_are_dropped_at_open() {
        let (mut s, _) = song();
        let notes = &mut s.tracks[0].staves[0].bars[0].voices[0].beats;
        // Fret 4 on string 1: G sharp. Spelled G sharp (ours, in C), A flat
        // (kept), and C (wrong: dropped).
        notes[0].notes[0] = Note::fretted(1, 4);
        notes[0].notes[0].spelling = Some(Spelling { step: 4, alter: 1 });
        notes[1].notes[0] = Note::fretted(1, 4);
        notes[1].notes[0].spelling = Some(Spelling { step: 5, alter: -1 });
        notes[2].notes[0] = Note::fretted(1, 4);
        notes[2].notes[0].spelling = Some(Spelling { step: 0, alter: 0 });
        assert_eq!(plain_spellings(&mut s), 2);
        let notes = &s.tracks[0].staves[0].bars[0].voices[0].beats;
        let kept: Vec<_> = notes.iter().take(3).map(|b| b.notes[0].spelling).collect();
        assert_eq!(kept, [None, Some(Spelling { step: 5, alter: -1 }), None]);
    }

    #[test]
    fn bars_per_system_are_set_and_undone() {
        let (mut s, mut h) = song();
        let set = |layout: Vec<u8>, track| Command::SystemLayout { track, layout };
        apply(&mut s, &mut h, at(0, 0, 1), None, set(vec![2, 1], Some(0))).unwrap();
        assert_eq!(s.tracks[0].layout, [2, 1]);
        apply(&mut s, &mut h, at(0, 0, 1), None, set(vec![3], None)).unwrap();
        assert_eq!(s.layout, [3]);
        assert!(apply(&mut s, &mut h, at(0, 0, 1), None, set(vec![3], None)).is_none());
        h.undo(&mut s).unwrap();
        assert!(s.layout.is_empty());
        h.undo(&mut s).unwrap();
        assert!(s.tracks[0].layout.is_empty());
    }

    #[test]
    fn fingering_tools_step_through_the_fingers() {
        let (mut s, mut h) = song();
        let lh = |s: &Score| {
            s.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0]
                .effects
                .left_finger
        };
        for want in [Some(1), Some(2), Some(3), Some(4), None] {
            apply(
                &mut s,
                &mut h,
                at(0, 0, 1),
                None,
                Command::NextFinger { right: false },
            )
            .unwrap();
            assert_eq!(lh(&s), want);
        }
        apply(
            &mut s,
            &mut h,
            at(0, 0, 1),
            None,
            Command::NextFinger { right: true },
        )
        .unwrap();
        assert_eq!(
            s.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0]
                .effects
                .right_finger,
            Some(0)
        );
    }
}
