// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Note entry (M3 slice 3): what typing at the caret does to the score.
//!
//! Every [`Command`] works on the caret's staff and voice (or on the beats
//! a selection holds there), changes only those bars, and is recorded as
//! one undoable change. The keys are Guitar Pro 8's (its shortcut list,
//! checked 20 Oct 2026):
//!
//! | Key | Command |
//! | --- | --- |
//! | 0 to 9 | [`Command::Fret`]: the fret on the caret's string; two digits typed in a row make a two-digit fret |
//! | R | [`Command::Rest`] |
//! | − and + | [`Command::Longer`], [`Command::Shorter`]; the palette's note values [`Command::Value`] |
//! | Shift + . and Ctrl + . | [`Command::Dot`], [`Command::DoubleDot`] |
//! | L and Shift + L | [`Command::Tie`], [`Command::TieBeat`] |
//! | Backspace | [`Command::DeleteNote`] |
//! | Shift + Delete | [`Command::DeleteBeats`] |
//! | Ctrl + Insert | [`Command::InsertBeat`] |
//! | Alt + Shift + Up and Down | [`Command::SemitoneUp`], [`Command::SemitoneDown`] |
//! | Alt + Up and Down | [`Command::StringUp`], [`Command::StringDown`]: the same pitch on the next string |
//!
//! At the caret's place after a voice's last beat (see [`Caret::at_end`])
//! a fret, a rest or a tie adds a beat as long as the beat before it.

use oxiguitar_model::{
    Bar, Beat, BeatKind, BeatTechnique, Duration, Dynamic, Note, NotePlace, NoteValue, Pitch,
    Score, Staff, Voice,
};

use crate::{Caret, History, Region, Selection, Touched};

/// An edit typed at the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// A fret on the caret's string. With `again`, the fret replaces the
    /// fret just typed, as its second digit: one change, not two.
    Fret {
        /// The fret, from 0.
        fret: u8,
        /// Replaces the last change, if that was a fret typed.
        again: bool,
    },
    /// The beat becomes a rest.
    Rest,
    /// The next longer note value (−).
    Longer,
    /// The next shorter note value (+).
    Shorter,
    /// This note value (the palette's whole to sixty-fourth notes).
    Value(NoteValue),
    /// One dot, or none if the beat has one.
    Dot,
    /// Two dots, or none if the beat has two.
    DoubleDot,
    /// The note on the caret's string tied to the note before it on that
    /// string, or untied.
    Tie,
    /// Every note of the beat before tied into the caret's beat, or untied.
    TieBeat,
    /// The note on the caret's string goes; a beat left without notes
    /// becomes a rest.
    DeleteNote,
    /// The caret's beat, or the selected beats, go.
    DeleteBeats,
    /// A rest as long as the caret's beat, before it.
    InsertBeat,
    /// The note a semitone higher, on its string.
    SemitoneUp,
    /// The note a semitone lower, on its string.
    SemitoneDown,
    /// The note at the same pitch on the string above.
    StringUp,
    /// The note at the same pitch on the string below.
    StringDown,
}

impl Command {
    /// The change's name, as Edit → Undo shows it.
    pub fn label(self) -> &'static str {
        match self {
            Command::Fret { .. } => FRET,
            Command::Rest => "Rest",
            Command::Longer => "Longer Note",
            Command::Shorter => "Shorter Note",
            Command::Value(_) => "Note Value",
            Command::Dot => "Dot",
            Command::DoubleDot => "Double Dot",
            Command::Tie => "Tie Note",
            Command::TieBeat => "Tie Beat",
            Command::DeleteNote => "Delete Note",
            Command::DeleteBeats => "Delete Beats",
            Command::InsertBeat => "Insert Beat",
            Command::SemitoneUp => "Semitone Up",
            Command::SemitoneDown => "Semitone Down",
            Command::StringUp => "String Up",
            Command::StringDown => "String Down",
        }
    }
}

const FRET: &str = "Type Fret";

/// What an edit did: what it touched, and where the caret goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edited {
    /// What the change touched, for the bar check and the window.
    pub touched: Touched,
    /// The caret after the edit.
    pub caret: Caret,
}

/// Runs `command` at `caret` (on `selection`'s beats where the command
/// takes a selection) as one change in `history`. `None` when the command
/// doesn't apply there (a fret on a staff without strings, a tie with
/// nothing to tie to, a note value past the shortest): nothing changes and
/// nothing is recorded.
pub fn apply(
    score: &mut Score,
    history: &mut History,
    caret: Caret,
    selection: Option<Selection>,
    command: Command,
) -> Option<Edited> {
    let undone = matches!(command, Command::Fret { again: true, .. })
        && history.undo_label() == Some(FRET)
        && history.undo(score).is_some();
    let Some(plan) = plan(score, caret, selection, command) else {
        if undone {
            history.redo(score);
        }
        return None;
    };
    let mut plan = plan;
    plan.bars.iter_mut().for_each(tidy);
    if let Some(staff) = staff_of(score, caret) {
        follow_ties(staff, caret.voice, plan.first, &mut plan.bars);
    }
    let region = Region::Bars {
        track: caret.track,
        staff: caret.staff,
        bars: plan.first..plan.first + plan.bars.len(),
    };
    let (track, staff, first) = (caret.track, caret.staff, plan.first);
    let touched = history.record(score, command.label(), vec![region], |s| {
        if let Some(dst) = s
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(staff))
            .and_then(|st| st.bars.get_mut(first..first + plan.bars.len()))
        {
            dst.clone_from_slice(&plan.bars);
        }
    });
    let caret = plan.caret.clamp(score).unwrap_or(plan.caret);
    Some(Edited { touched, caret })
}

/// Tied notes take the fret of the note they continue: the last note
/// played on their string in the voice, as Guitar Pro reads a tie. From the
/// edited bars on, every tied note is set so; bars after them that change
/// too join `bars`, so a note changed under a tie carries through it.
fn follow_ties(staff: &Staff, voice: usize, first: usize, bars: &mut Vec<Bar>) {
    let mut last: Vec<Option<u8>> = vec![None; staff.string_count() + 1];
    // What the bars before the edit leave on each string.
    for bar in staff.bars.get(..first).unwrap_or_default() {
        remember(bar, voice, &mut last);
    }
    let mut at = first;
    let mut kept = bars.len();
    loop {
        let index = at - first;
        let mut bar = match bars.get(index) {
            Some(b) => b.clone(),
            None => match staff.bars.get(at) {
                Some(b) => b.clone(),
                None => break,
            },
        };
        let before = bar.clone();
        played(&mut bar, voice, &mut last);
        let changed = bar != before;
        if index < bars.len() {
            if let Some(b) = bars.get_mut(index) {
                *b = bar;
            }
        } else {
            bars.push(bar);
            if changed {
                kept = bars.len();
            }
        }
        at += 1;
    }
    bars.truncate(kept);
}

/// What a bar's notes in `voice` leave on each string, as [`played`]
/// would, without changing it.
fn remember(bar: &Bar, voice: usize, last: &mut [Option<u8>]) {
    let Some(v) = bar.voices.get(voice) else {
        return;
    };
    for n in v.beats.iter().flat_map(|b| &b.notes) {
        let NotePlace::Fretted { string, fret } = n.place else {
            continue;
        };
        if let Some(slot) = last.get_mut(usize::from(string)) {
            *slot = Some(match *slot {
                Some(previous) if n.tie => previous,
                _ => fret,
            });
        }
    }
}

/// Follows a bar's notes on each string in `voice`: tied notes take the
/// fret before them, and every note leaves its fret for the next.
fn played(bar: &mut Bar, voice: usize, last: &mut [Option<u8>]) {
    let Some(v) = bar.voices.get_mut(voice) else {
        return;
    };
    for b in &mut v.beats {
        for n in &mut b.notes {
            let NotePlace::Fretted { string, fret } = &mut n.place else {
                continue;
            };
            let Some(slot) = last.get_mut(usize::from(*string)) else {
                continue;
            };
            if n.tie
                && let Some(previous) = *slot
            {
                *fret = previous;
            }
            *slot = Some(*fret);
        }
    }
}

/// A bar as files keep it: a voice after the first that holds no notes or
/// rests is an empty placeholder, and none follows the last used voice.
fn tidy(bar: &mut Bar) {
    for v in bar.voices.iter_mut().skip(1) {
        if v.is_empty() {
            v.beats.clear();
        }
    }
    while bar.voices.len() > 1 && bar.voices.last().is_some_and(|v| v.beats.is_empty()) {
        bar.voices.pop();
    }
}

/// New bars for a run of one staff's bars, and the caret after them.
struct Plan {
    first: usize,
    bars: Vec<Bar>,
    caret: Caret,
}

fn staff_of(score: &Score, c: Caret) -> Option<&Staff> {
    score.tracks.get(c.track)?.staves.get(c.staff)
}

/// The command's new bars, or `None` when it changes nothing.
fn plan(score: &Score, c: Caret, selection: Option<Selection>, command: Command) -> Option<Plan> {
    let staff = staff_of(score, c)?;
    // A drum track's notes are drum sounds, not frets on strings (entering
    // them comes with the drum tools).
    let drums = score
        .tracks
        .get(c.track)
        .is_some_and(|t| t.instrument.percussion);
    if drums
        && matches!(
            command,
            Command::Fret { .. }
                | Command::Tie
                | Command::StringUp
                | Command::StringDown
                | Command::SemitoneUp
                | Command::SemitoneDown
        )
    {
        return None;
    }
    let old = staff.bars.get(c.bar)?;
    let one = |bar: Bar, caret: Caret| -> Option<Plan> {
        (bar != *old).then_some(Plan {
            first: c.bar,
            bars: vec![bar],
            caret,
        })
    };
    match command {
        Command::Fret { fret, .. } => {
            if staff.string_count() == 0 || fret > staff.fret_count {
                return None;
            }
            let dynamic = dynamic_at(score, c);
            let mut bar = old.clone();
            let beat = beat_at_caret(&mut bar, c, BeatKind::Notes)?;
            set_note(beat, Note::fretted(c.string, fret));
            settle_dynamic(beat, dynamic);
            one(bar, c)
        }
        Command::Rest => {
            let mut bar = old.clone();
            let beat = beat_at_caret(&mut bar, c, BeatKind::Rest)?;
            make_rest(beat);
            one(bar, c)
        }
        Command::Longer
        | Command::Shorter
        | Command::Value(_)
        | Command::Dot
        | Command::DoubleDot => {
            let targets = targets(score, c, selection);
            on_beats(score, c, &targets, |b| {
                let d = &mut b.duration;
                match command {
                    Command::Longer => d.value = longer(d.value)?,
                    Command::Shorter => d.value = shorter(d.value)?,
                    Command::Value(v) if d.value != v => d.value = v,
                    Command::Value(_) => return None,
                    Command::Dot => d.dots = if d.dots == 1 { 0 } else { 1 },
                    _ => d.dots = if d.dots == 2 { 0 } else { 2 },
                }
                Some(())
            })
        }
        Command::SemitoneUp | Command::SemitoneDown => {
            let up = command == Command::SemitoneUp;
            let targets = targets(score, c, selection);
            let only = (targets.len() == 1).then_some(c.string);
            on_beats(score, c, &targets, |b| {
                let mut any = false;
                for n in b
                    .notes
                    .iter_mut()
                    .filter(|n| only.is_none() || n.string().is_none_or(|s| Some(s) == only))
                {
                    any |= shift_semitone(n, up, staff.fret_count);
                }
                any.then_some(())
            })
        }
        Command::Tie | Command::TieBeat => {
            let before = note_before(score, c);
            let dynamic = dynamic_at(score, c);
            let mut bar = old.clone();
            let whole = command == Command::TieBeat || staff.string_count() == 0;
            let beat = beat_at_caret(&mut bar, c, BeatKind::Notes)?;
            if whole {
                let tied = beat.kind == BeatKind::Notes
                    && !beat.notes.is_empty()
                    && beat.notes.iter().all(|n| n.tie);
                if tied {
                    beat.notes.iter_mut().for_each(|n| n.tie = false);
                } else {
                    let prev = before?;
                    beat.kind = BeatKind::Notes;
                    beat.notes = prev.notes.iter().map(tied_to).collect();
                }
            } else {
                let at = beat.notes.iter().position(|n| n.string() == Some(c.string));
                match at.and_then(|i| beat.notes.get_mut(i)) {
                    Some(n) if n.tie => n.tie = false,
                    _ => {
                        let prev = before?;
                        let origin = prev.notes.iter().find(|n| n.string() == Some(c.string))?;
                        beat.kind = BeatKind::Notes;
                        set_note(beat, tied_to(origin));
                    }
                }
            }
            settle_dynamic(beat, dynamic);
            one(bar, c)
        }
        Command::DeleteNote => {
            let mut bar = old.clone();
            let beat = bar.voices.get_mut(c.voice)?.beats.get_mut(c.beat)?;
            if beat.kind != BeatKind::Notes {
                return None;
            }
            let at = if staff.string_count() == 0 {
                beat.notes.len().checked_sub(1)?
            } else {
                beat.notes
                    .iter()
                    .position(|n| n.string() == Some(c.string))?
            };
            beat.notes.remove(at);
            if beat.notes.is_empty() {
                make_rest(beat);
            }
            one(bar, c)
        }
        Command::DeleteBeats => {
            let targets = targets(score, c, selection);
            let (&(first, _), &(last, _)) = (targets.first()?, targets.last()?);
            let mut bars: Vec<Bar> = staff.bars.get(first..=last)?.to_vec();
            for &(bar, beat) in targets.iter().rev() {
                let v = bars.get_mut(bar - first)?.voices.get_mut(c.voice)?;
                if beat < v.beats.len() {
                    v.beats.remove(beat);
                }
            }
            let caret = Caret {
                bar: first,
                beat: targets.first().map_or(0, |t| t.1),
                ..c
            };
            Some(Plan { first, bars, caret })
        }
        Command::InsertBeat => {
            let mut bar = old.clone();
            grow_voices(&mut bar, c.voice);
            let v = bar.voices.get_mut(c.voice)?;
            let at = c.beat.min(v.beats.len());
            let duration = v
                .beats
                .get(at)
                .map_or_else(|| duration_before(v, at), |b| b.duration.clone());
            v.beats.insert(
                at,
                Beat {
                    kind: BeatKind::Rest,
                    duration,
                    ..Beat::default()
                },
            );
            one(bar, Caret { beat: at, ..c })
        }
        Command::StringUp | Command::StringDown => {
            let to = if command == Command::StringUp {
                c.string.checked_sub(1).filter(|s| *s >= 1)?
            } else {
                c.string
                    .checked_add(1)
                    .filter(|s| usize::from(*s) <= staff.string_count())?
            };
            let mut bar = old.clone();
            let beat = bar.voices.get_mut(c.voice)?.beats.get_mut(c.beat)?;
            if beat.notes.iter().any(|n| n.string() == Some(to)) {
                return None;
            }
            let note = beat
                .notes
                .iter_mut()
                .find(|n| n.string() == Some(c.string))?;
            let NotePlace::Fretted { fret, .. } = note.place else {
                return None;
            };
            let pitch = i32::from(staff.pitch_of(c.string, fret)?.0);
            let open = i32::from(staff.pitch_of(to, 0)?.0);
            let new = u8::try_from(pitch - open)
                .ok()
                .filter(|f| *f <= staff.fret_count)?;
            note.place = NotePlace::Fretted {
                string: to,
                fret: new,
            };
            beat.notes.sort_by_key(|n| n.string());
            one(bar, Caret { string: to, ..c })
        }
    }
}

/// The caret's beat in `bar`, made if the caret is after the voice's last
/// beat: a beat of kind `kind`, as long as the one before it. The voice
/// itself is made if the bar doesn't have it yet. `None` for a caret past
/// the place after the last beat.
fn beat_at_caret(bar: &mut Bar, c: Caret, kind: BeatKind) -> Option<&mut Beat> {
    grow_voices(bar, c.voice);
    let v = bar.voices.get_mut(c.voice)?;
    if c.beat == v.beats.len() {
        let duration = duration_before(v, c.beat);
        v.beats.push(Beat {
            kind,
            duration,
            ..Beat::default()
        });
    }
    let beat = v.beats.get_mut(c.beat)?;
    if beat.kind != BeatKind::Notes {
        beat.notes.clear();
    }
    Some(beat)
}

/// The beat becomes a rest: no notes, and nothing only notes have (in a
/// Guitar Pro file tapping belongs to the notes).
fn make_rest(beat: &mut Beat) {
    beat.kind = BeatKind::Rest;
    beat.notes.clear();
    if beat.effects.technique == Some(BeatTechnique::Tap) {
        beat.effects.technique = None;
    }
}

/// Empty voices up to `voice`, so the caret's voice exists.
fn grow_voices(bar: &mut Bar, voice: usize) {
    while bar.voices.len() <= voice {
        bar.voices.push(Voice::default());
    }
}

/// How long a new beat at `at` is: as long as the main beat before it, or a
/// quarter.
fn duration_before(v: &Voice, at: usize) -> Duration {
    v.beats
        .get(..at)
        .and_then(|b| b.iter().rev().find(|b| b.grace.is_none()))
        .map_or_else(
            || Duration::new(NoteValue::Quarter),
            |b| Duration {
                tuplets: Vec::new(),
                ..b.duration.clone()
            },
        )
}

/// Puts `note` into `beat` in place of the note on its string (or, for a
/// note without a string, of a note of the same pitch), strings in order.
fn set_note(beat: &mut Beat, note: Note) {
    beat.kind = BeatKind::Notes;
    let same = |n: &Note| match (n.place, note.place) {
        (NotePlace::Fretted { string: a, .. }, NotePlace::Fretted { string: b, .. }) => a == b,
        (a, b) => a == b,
    };
    match beat.notes.iter_mut().find(|n| same(n)) {
        Some(n) => *n = note,
        None => beat.notes.push(note),
    }
    beat.notes.sort_by_key(|n| n.string());
}

/// A note tied to `origin`: the same place, no effects of its own.
fn tied_to(origin: &Note) -> Note {
    Note {
        place: origin.place,
        tie: true,
        dynamic: origin.dynamic,
        effects: Default::default(),
    }
}

/// The beat before the caret's in its voice that holds notes or a rest,
/// across bar lines (grace beats and placeholders skipped). Only a beat
/// with notes is given: nothing ties across a rest.
fn note_before(score: &Score, c: Caret) -> Option<Beat> {
    let staff = staff_of(score, c)?;
    let mut bar = c.bar;
    let mut end = c.beat;
    loop {
        if let Some(v) = staff.bars.get(bar)?.voices.get(c.voice) {
            let upto = end.min(v.beats.len());
            for b in v.beats.get(..upto)?.iter().rev() {
                if b.grace.is_some() || b.kind == BeatKind::Empty {
                    continue;
                }
                return (b.kind == BeatKind::Notes).then(|| b.clone());
            }
        }
        bar = bar.checked_sub(1)?;
        end = usize::MAX;
    }
}

/// Every note of `beat` as loud as `dynamic`, and the beat marked so when
/// that isn't the default, as a Guitar Pro 6–8 file holds it: one dynamic
/// per beat, which its notes take.
fn settle_dynamic(beat: &mut Beat, dynamic: Dynamic) {
    for n in &mut beat.notes {
        n.dynamic = dynamic;
    }
    if beat.effects.dynamic.is_none() && dynamic != Dynamic::default() {
        beat.effects.dynamic = Some(dynamic);
    }
}

/// How loud a note typed at the caret is: as its beat, or else as the last
/// beat with notes before it in the voice (Guitar Pro keeps one dynamic per
/// beat, and a dynamic lasts until the next).
fn dynamic_at(score: &Score, c: Caret) -> Dynamic {
    let loudness = |b: &Beat| {
        b.effects
            .dynamic
            .or_else(|| b.notes.first().map(|n| n.dynamic))
    };
    let here = staff_of(score, c)
        .and_then(|s| s.bars.get(c.bar))
        .and_then(|b| b.voices.get(c.voice))
        .and_then(|v| v.beats.get(c.beat))
        .and_then(|b| {
            b.effects.dynamic.or_else(|| {
                (b.kind == BeatKind::Notes)
                    .then(|| b.notes.first().map(|n| n.dynamic))
                    .flatten()
            })
        });
    here.or_else(|| {
        let staff = staff_of(score, c)?;
        (0..=c.bar).rev().find_map(|bar| {
            let v = staff.bars.get(bar)?.voices.get(c.voice)?;
            let upto = if bar == c.bar {
                c.beat.min(v.beats.len())
            } else {
                v.beats.len()
            };
            v.beats
                .get(..upto)?
                .iter()
                .rev()
                .find(|b| b.kind == BeatKind::Notes && !b.notes.is_empty())
                .and_then(loudness)
        })
    })
    .unwrap_or_default()
}

/// The beats a command on durations or pitches acts on, as (bar, beat) in
/// the caret's voice: the selected beats of the caret's staff and voice,
/// or the caret's beat.
fn targets(score: &Score, c: Caret, selection: Option<Selection>) -> Vec<(usize, usize)> {
    let Some(staff) = staff_of(score, c) else {
        return Vec::new();
    };
    let beats = |bar: usize| {
        staff
            .bars
            .get(bar)
            .and_then(|b| b.voices.get(c.voice))
            .map_or(0, |v| v.beats.len())
    };
    let mut out = Vec::new();
    match selection {
        Some(Selection::Beats {
            track,
            staff: st,
            voice,
            from,
            to,
        }) if (track, st, voice) == (c.track, c.staff, c.voice) => {
            for bar in from.0..=to.0 {
                let first = if bar == from.0 { from.1 } else { 0 };
                let last = if bar == to.0 { to.1 + 1 } else { usize::MAX };
                out.extend((first..last.min(beats(bar))).map(|b| (bar, b)));
            }
        }
        Some(Selection::Bars { tracks, bars }) if (tracks.0..=tracks.1).contains(&c.track) => {
            for bar in bars.0..=bars.1 {
                out.extend((0..beats(bar)).map(|b| (bar, b)));
            }
        }
        _ => {
            if c.beat < beats(c.bar) {
                out.push((c.bar, c.beat));
            }
        }
    }
    out
}

/// Runs `edit` on each target beat (in the caret's voice) and gives the
/// changed bars, the caret staying. `None` if no beat changed.
fn on_beats(
    score: &Score,
    c: Caret,
    targets: &[(usize, usize)],
    mut edit: impl FnMut(&mut Beat) -> Option<()>,
) -> Option<Plan> {
    let staff = staff_of(score, c)?;
    let (&(first, _), &(last, _)) = (targets.first()?, targets.last()?);
    let mut bars: Vec<Bar> = staff.bars.get(first..=last)?.to_vec();
    let mut changed = false;
    for &(bar, beat) in targets {
        if let Some(b) = bars
            .get_mut(bar - first)
            .and_then(|b| b.voices.get_mut(c.voice))
            .and_then(|v| v.beats.get_mut(beat))
        {
            changed |= edit(b).is_some();
        }
    }
    let same = staff.bars.get(first..=last).is_some_and(|old| old == bars);
    (changed && !same).then_some(Plan {
        first,
        bars,
        caret: c,
    })
}

/// The next longer value Guitar Pro enters: up to a whole note.
fn longer(v: NoteValue) -> Option<NoteValue> {
    Some(match v {
        NoteValue::DoubleWhole | NoteValue::Whole => return None,
        NoteValue::Half => NoteValue::Whole,
        NoteValue::Quarter => NoteValue::Half,
        NoteValue::Eighth => NoteValue::Quarter,
        NoteValue::Sixteenth => NoteValue::Eighth,
        NoteValue::ThirtySecond => NoteValue::Sixteenth,
        NoteValue::SixtyFourth => NoteValue::ThirtySecond,
        NoteValue::OneTwentyEighth | NoteValue::TwoFiftySixth => NoteValue::SixtyFourth,
    })
}

/// The next shorter value Guitar Pro enters: down to a sixty-fourth.
fn shorter(v: NoteValue) -> Option<NoteValue> {
    Some(match v {
        NoteValue::DoubleWhole => NoteValue::Whole,
        NoteValue::Whole => NoteValue::Half,
        NoteValue::Half => NoteValue::Quarter,
        NoteValue::Quarter => NoteValue::Eighth,
        NoteValue::Eighth => NoteValue::Sixteenth,
        NoteValue::Sixteenth => NoteValue::ThirtySecond,
        NoteValue::ThirtySecond => NoteValue::SixtyFourth,
        NoteValue::SixtyFourth | NoteValue::OneTwentyEighth | NoteValue::TwoFiftySixth => {
            return None;
        }
    })
}

/// A note a semitone up or down: a fret on its string (within the frets),
/// or a pitch. Whether it moved.
fn shift_semitone(n: &mut Note, up: bool, frets: u8) -> bool {
    match &mut n.place {
        NotePlace::Fretted { fret, .. } => {
            let new = if up {
                fret.checked_add(1).filter(|f| *f <= frets)
            } else {
                fret.checked_sub(1)
            };
            new.map(|f| *fret = f).is_some()
        }
        NotePlace::Pitched(Pitch(p)) => {
            let new = if up {
                p.checked_add(1).filter(|p| *p <= 127)
            } else {
                p.checked_sub(1)
            };
            new.map(|q| *p = q).is_some()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caret::tests::{bar, score};
    use oxiguitar_model::NoteValue::{Eighth, Half, Quarter};

    fn at(beat: usize, string: u8) -> Caret {
        Caret {
            track: 0,
            staff: 0,
            voice: 0,
            bar: 0,
            beat,
            string,
        }
    }

    fn beats(s: &Score) -> &[Beat] {
        &s.tracks[0].staves[0].bars[0].voices[0].beats
    }

    fn run(s: &mut Score, h: &mut History, c: Caret, cmd: Command) -> Option<Caret> {
        apply(s, h, c, None, cmd).map(|e| e.caret)
    }

    fn fret(fret: u8) -> Command {
        Command::Fret { fret, again: false }
    }

    #[test]
    fn frets_go_on_the_caret_string_and_two_digits_are_one_change() {
        let mut s = score(1, 1);
        s.tracks[0].staves[0].bars[0] = bar(&[Quarter, Quarter]);
        let mut h = History::new();
        run(&mut s, &mut h, at(0, 3), fret(1)).unwrap();
        let again = Command::Fret {
            fret: 12,
            again: true,
        };
        run(&mut s, &mut h, at(0, 3), again).unwrap();
        let places: Vec<_> = beats(&s)[0].notes.iter().map(|n| n.place).collect();
        assert_eq!(
            places,
            [
                NotePlace::Fretted { string: 1, fret: 0 },
                NotePlace::Fretted {
                    string: 3,
                    fret: 12
                }
            ]
        );
        h.undo(&mut s);
        assert_eq!(beats(&s)[0].notes.len(), 1, "one undo for both digits");
        // Past the frets, or on a staff without strings: nothing.
        assert!(run(&mut s, &mut h, at(0, 1), fret(25)).is_none());
        // After the last beat: a new beat as long as the one before.
        run(&mut s, &mut h, at(2, 2), fret(5)).unwrap();
        assert_eq!(beats(&s).len(), 3);
        assert_eq!(beats(&s)[2].duration.value, Quarter);
    }

    #[test]
    fn rests_durations_and_dots() {
        let mut s = score(1, 1);
        let mut h = History::new();
        run(&mut s, &mut h, at(1, 1), Command::Rest).unwrap();
        assert_eq!(beats(&s)[1].kind, BeatKind::Rest);
        assert!(beats(&s)[1].notes.is_empty());
        assert!(run(&mut s, &mut h, at(1, 1), Command::Rest).is_none());
        run(&mut s, &mut h, at(0, 1), Command::Shorter).unwrap();
        assert_eq!(beats(&s)[0].duration.value, Eighth);
        run(&mut s, &mut h, at(0, 1), Command::Longer).unwrap();
        run(&mut s, &mut h, at(0, 1), Command::Longer).unwrap();
        assert_eq!(beats(&s)[0].duration.value, Half);
        run(&mut s, &mut h, at(0, 1), Command::Dot).unwrap();
        assert_eq!(beats(&s)[0].duration.dots, 1);
        run(&mut s, &mut h, at(0, 1), Command::DoubleDot).unwrap();
        assert_eq!(beats(&s)[0].duration.dots, 2);
        run(&mut s, &mut h, at(0, 1), Command::DoubleDot).unwrap();
        assert_eq!(beats(&s)[0].duration.dots, 0);
        // A selection: every selected beat.
        let sel = Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (0, 1),
            to: (0, 3),
        };
        apply(&mut s, &mut h, at(1, 1), Some(sel), Command::Shorter).unwrap();
        let values: Vec<_> = beats(&s).iter().map(|b| b.duration.value).collect();
        assert_eq!(values, [Half, Eighth, Eighth, Eighth]);
    }

    #[test]
    fn ties_take_the_fret_before_and_untie() {
        let mut s = score(1, 2);
        let mut h = History::new();
        run(&mut s, &mut h, at(0, 2), fret(7)).unwrap();
        // The next beat has a note on string 1 only: tie string 2 into it.
        run(&mut s, &mut h, at(1, 2), Command::Tie).unwrap();
        let n = beats(&s)[1]
            .notes
            .iter()
            .find(|n| n.string() == Some(2))
            .unwrap();
        assert!(n.tie);
        assert_eq!(n.place, NotePlace::Fretted { string: 2, fret: 7 });
        run(&mut s, &mut h, at(1, 2), Command::Tie).unwrap();
        assert!(!beats(&s)[1].notes.iter().any(|n| n.tie));
        // Nothing on string 4 before: nothing to tie to.
        assert!(run(&mut s, &mut h, at(1, 4), Command::Tie).is_none());
        // Across the bar line, the whole beat.
        let c = Caret { bar: 1, ..at(0, 1) };
        apply(&mut s, &mut h, c, None, Command::TieBeat).unwrap();
        let first = &s.tracks[0].staves[0].bars[1].voices[0].beats[0];
        assert!(first.notes.iter().all(|n| n.tie));
        // After a rest nothing ties.
        run(&mut s, &mut h, at(2, 1), Command::Rest).unwrap();
        assert!(run(&mut s, &mut h, at(3, 1), Command::TieBeat).is_none());
    }

    #[test]
    fn deleting_and_inserting() {
        let mut s = score(1, 1);
        let mut h = History::new();
        run(&mut s, &mut h, at(0, 1), Command::DeleteNote).unwrap();
        assert_eq!(beats(&s)[0].kind, BeatKind::Rest, "no note left: a rest");
        assert!(run(&mut s, &mut h, at(0, 1), Command::DeleteNote).is_none());
        run(&mut s, &mut h, at(1, 1), Command::InsertBeat).unwrap();
        assert_eq!(beats(&s).len(), 5);
        assert_eq!(beats(&s)[1].kind, BeatKind::Rest);
        let sel = Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (0, 1),
            to: (0, 2),
        };
        let e = apply(&mut s, &mut h, at(3, 1), Some(sel), Command::DeleteBeats).unwrap();
        assert_eq!(beats(&s).len(), 3);
        assert_eq!(e.caret.beat, 1);
    }

    #[test]
    fn semitones_and_strings() {
        let mut s = score(1, 1);
        let mut h = History::new();
        run(&mut s, &mut h, at(0, 1), fret(5)).unwrap();
        run(&mut s, &mut h, at(0, 1), Command::SemitoneUp).unwrap();
        assert_eq!(
            beats(&s)[0].notes[0].place,
            NotePlace::Fretted { string: 1, fret: 6 }
        );
        // E string fret 6 (A#4) is fret 11 on the B string.
        let c = run(&mut s, &mut h, at(0, 1), Command::StringDown).unwrap();
        assert_eq!(c.string, 2);
        assert_eq!(
            beats(&s)[0].notes[0].place,
            NotePlace::Fretted {
                string: 2,
                fret: 11
            }
        );
        assert!(run(&mut s, &mut h, at(1, 1), Command::StringUp).is_none());
        run(&mut s, &mut h, at(1, 1), Command::SemitoneUp).unwrap();
        run(&mut s, &mut h, at(1, 1), Command::SemitoneDown).unwrap();
        assert!(
            run(&mut s, &mut h, at(1, 1), Command::SemitoneDown).is_none(),
            "below the open string"
        );
    }

    #[test]
    fn a_new_voice_starts_where_the_caret_is() {
        let mut s = score(1, 1);
        let mut h = History::new();
        let c = Caret {
            voice: 2,
            ..at(0, 2)
        };
        run(&mut s, &mut h, c, fret(3)).unwrap();
        let voices = &s.tracks[0].staves[0].bars[0].voices;
        assert_eq!(voices.len(), 3);
        assert!(voices[1].beats.is_empty(), "voice 2 stays empty");
        assert_eq!(voices[2].beats.len(), 1);
    }
}
