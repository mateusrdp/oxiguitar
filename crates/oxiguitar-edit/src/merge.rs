// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Merging tracks (#312): the notes of one track (the donor, the caret's)
//! written alongside another's (the receiver's), then the donor deleted.
//!
//! - **Voices:** each donor voice picked goes to one receiver voice
//!   ([`Merge::voices`]). The donor's staff N goes to the receiver's staff
//!   N, or its last staff when it has fewer.
//! - **Rhythms:** where the two rhythms differ, beats are cut at every
//!   moment either track strikes or ends a note; the pieces of a cut note
//!   are tied, so both sound as before (the pieces are written as Fit Bars
//!   writes them, `fit::spell`). A bar whose pieces no note value can show
//!   stops the merge ([`MergeError::Rhythms`]).
//! - **Strings:** a donor note keeps its string and fret where the
//!   receiver's tuning plays its pitch there and that string is free; else
//!   it goes, at its pitch, to the free string nearest its fret (as the
//!   track dialog does, `tracks::replace`); else it is left out and counted
//!   ([`merge_lost`]). A tied note stays on the string of the note it
//!   continues, or is struck again where that string isn't free.
//! - **Unisons:** with [`Merge::discard_unisons`], a donor note is left
//!   out when the receiver strikes the same pitch at the same moment (with
//!   the notes it ties on to).
//! - Where both play, the beat's own marks (text, chord, stroke …) are
//!   the receiver's; where only the donor plays, the donor's (its chord
//!   diagrams join the receiver's). The donor's lyrics go with it; the
//!   receiver's keep their syllables on the same notes.

use std::collections::BTreeMap;

use oxiguitar_model::{Bar, Beat, BeatKind, Note, NotePlace, Ratio, Score, Staff, Track, Voice};

use crate::fit::{Prefer, piece, rests, spell};
use crate::{Caret, Edited, History, Region};

/// Merge the caret's track into another (#312).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    /// The track that takes the notes.
    pub into: usize,
    /// The donor's voices that go, each with the receiver's voice it goes
    /// to: `(from, to)`, from 0.
    pub voices: Vec<(usize, usize)>,
    /// Leave out a donor note when the receiver strikes the same pitch at
    /// the same moment.
    pub discard_unisons: bool,
}

/// Why a merge can't be done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeError {
    /// The tracks or voices asked for aren't there (or are one track).
    Nothing,
    /// A drum track goes only into a drum track, and a pitched one only
    /// into a pitched one.
    Kinds,
    /// The two rhythms of this bar (from 0) can't be written in one voice.
    Rhythms {
        /// The bar, from 0.
        bar: usize,
    },
}

/// How many donor notes merging track `from` as `m` asks would leave out,
/// for want of a string; or why it can't be done.
pub fn merge_lost(score: &Score, from: usize, m: &Merge) -> Result<usize, MergeError> {
    merged(score, from, m).map(|(_, lost)| lost)
}

/// Runs [`crate::Command::MergeTracks`]: one change, recorded whole.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    label: &str,
    m: &Merge,
) -> Option<Edited> {
    let (work, _) = merged(score, c.track, m).ok()?;
    let into = if m.into > c.track { m.into - 1 } else { m.into };
    let voice = m.voices.first().map_or(0, |v| v.1);
    let touched = history.record(score, label, vec![Region::Whole], |s| *s = work);
    let caret = Caret {
        track: into,
        staff: 0,
        voice,
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret
            .clamp(score)
            .or_else(|| Caret::at_start(score, into))?,
    })
}

/// The score with track `from` merged as `m` asks, and the notes left out.
fn merged(score: &Score, from: usize, m: &Merge) -> Result<(Score, usize), MergeError> {
    let (Some(donor), Some(receiver)) = (score.tracks.get(from), score.tracks.get(m.into)) else {
        return Err(MergeError::Nothing);
    };
    if from == m.into
        || m.voices.is_empty()
        || m.voices
            .iter()
            .any(|&(a, b)| a >= crate::VOICES || b >= crate::VOICES)
        || receiver.staves.is_empty()
    {
        return Err(MergeError::Nothing);
    }
    if donor.instrument.percussion != receiver.instrument.percussion {
        return Err(MergeError::Kinds);
    }
    let opts = Opts {
        chords: &[],
        discard_unisons: m.discard_unisons,
    };
    let mut track = receiver.clone();
    let chords = join_chords(&mut track, donor);
    let opts = Opts {
        chords: &chords,
        ..opts
    };
    // Each receiver voice and what goes into it: (staff, voice) of each.
    let mut targets: BTreeMap<(usize, usize), Vec<(usize, usize)>> = BTreeMap::new();
    let last = receiver.staves.len() - 1;
    for ds in 0..donor.staves.len() {
        for &(dv, rv) in &m.voices {
            targets
                .entry((ds.min(last), rv))
                .or_default()
                .push((ds, dv));
        }
    }
    let mut lost = 0;
    for (&(rs, rv), sources) in &targets {
        let mut ties = vec![BTreeMap::new(); sources.len()];
        for k in 0..score.bar_count() {
            let Some(staff) = track.staves.get(rs) else {
                continue;
            };
            // Drum notes are keys, never on strings, though a file may
            // give a drum staff strings.
            let view = Staff {
                tuning: if receiver.instrument.percussion {
                    Vec::new()
                } else {
                    staff.tuning.clone()
                },
                bars: Vec::new(),
                ..*staff
            };
            let layers: Vec<Layer<'_>> = sources
                .iter()
                .zip(&mut ties)
                .map(|(&(ds, dv), ties)| Layer {
                    staff: donor.staves.get(ds),
                    voice: dv,
                    ties,
                })
                .collect();
            let bar = merge_bar(staff, &view, k, rv, layers, &opts, &mut lost)
                .ok_or(MergeError::Rhythms { bar: k })?;
            if let Some(bar) = bar
                && let Some(slot) = track.staves.get_mut(rs).and_then(|s| s.bars.get_mut(k))
            {
                *slot = bar;
            }
        }
    }
    for &(rs, rv) in targets.keys() {
        if let Some(staff) = track.staves.get_mut(rs) {
            strike_broken_ties(staff, rv);
        }
    }
    if m.voices.iter().any(|v| v.1 == 0)
        && let (Some(old), Some(new)) = (receiver.staves.first(), track.staves.first())
    {
        keep_lyrics(&mut track.lyrics, old, new);
    }
    let mut out = score.clone();
    if let Some(dst) = out.tracks.get_mut(m.into) {
        *dst = track;
    }
    remove_track(&mut out, from, Some(m.into));
    Ok((out, lost))
}

/// Takes track `t` out of `score`, with what refers to it by number:
/// another track showing its chords shows `instead`'s (none for the track
/// itself), and its own bar colours go.
pub(crate) fn remove_track(score: &mut Score, t: usize, instead: Option<usize>) {
    if t >= score.tracks.len() {
        return;
    }
    score.tracks.remove(t);
    let renumber = |i: usize| match i.cmp(&t) {
        std::cmp::Ordering::Less => Some(i),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(i - 1),
    };
    let instead = instead.and_then(renumber);
    for (i, track) in score.tracks.iter_mut().enumerate() {
        if track.chords_from == usize::MAX {
            continue;
        }
        track.chords_from = match renumber(track.chords_from) {
            Some(n) => n,
            None => instead.filter(|n| *n != i).unwrap_or(usize::MAX),
        };
    }
    score.track_bar_colors = std::mem::take(&mut score.track_bar_colors)
        .into_iter()
        .filter_map(|(i, c)| renumber(i).map(|n| (n, c)))
        .collect();
}

/// The donor's chord diagrams added to the receiver's (each once); where
/// each donor diagram went.
fn join_chords(track: &mut Track, donor: &Track) -> Vec<usize> {
    donor
        .chords
        .iter()
        .map(|d| match track.chords.iter().position(|k| k == d) {
            Some(i) => i,
            None => {
                track.chords.push(d.clone());
                track.chords.len() - 1
            }
        })
        .collect()
}

/// What became of a donor note that a later tied note continues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Went {
    /// On this string of the receiver (0 for a staff without strings).
    Placed(u8),
    /// Left out: the receiver strikes it.
    Unison,
    /// Left out: no string plays it.
    Lost,
}

/// Where a donor note is on its own staff, for its ties: its string, or
/// its pitch (above every string number) on a staff without strings.
fn tie_key(n: &Note) -> u16 {
    match n.place {
        NotePlace::Fretted { string, .. } => u16::from(string),
        NotePlace::Pitched(p) => 256 + u16::from(p.0),
    }
}

/// One donor voice going into the receiver's voice.
struct Layer<'a> {
    /// The donor's staff.
    staff: Option<&'a Staff>,
    /// Its voice.
    voice: usize,
    /// What became of the notes later tied notes continue, by [`tie_key`]:
    /// kept from bar to bar.
    ties: &'a mut BTreeMap<u16, Went>,
}

/// A beat of a voice in time: from `start` to `end` of its bar, with the
/// grace notes before it.
#[derive(Clone, Debug)]
struct Event {
    start: Ratio,
    end: Ratio,
    beat: Beat,
    graces: Vec<Beat>,
}

/// A voice's beats in time, and the grace notes after its last.
fn events(voice: Option<&Voice>) -> Option<(Vec<Event>, Vec<Beat>)> {
    let mut out = Vec::new();
    let mut graces = Vec::new();
    let mut at = Ratio::ZERO;
    for b in voice.map(|v| v.beats.as_slice()).unwrap_or_default() {
        if b.grace.is_some() {
            graces.push(b.clone());
            continue;
        }
        let end = at.checked_add(b.duration.length()?)?;
        out.push(Event {
            start: at,
            end,
            beat: b.clone(),
            graces: std::mem::take(&mut graces),
        });
        at = end;
    }
    Some((out, graces))
}

fn pitch_of(staff: &Staff, n: &Note) -> Option<u8> {
    match n.place {
        NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret).map(|p| p.0),
        NotePlace::Pitched(p) => Some(p.0),
    }
}

/// What a merge asks, for each bar.
#[derive(Clone, Copy)]
struct Opts<'a> {
    /// Where each donor chord diagram went.
    chords: &'a [usize],
    /// [`Merge::discard_unisons`].
    discard_unisons: bool,
}

/// Bar `k` of the receiver's voice `rv` on `staff` with the layers' notes
/// written in, on the strings of `to` (`staff` without its bars, and
/// without strings on a drum track); `Some(None)` when they bring none,
/// `None` when the rhythms can't be written in one voice.
fn merge_bar(
    staff: &Staff,
    to: &Staff,
    k: usize,
    rv: usize,
    mut layers: Vec<Layer<'_>>,
    opts: &Opts<'_>,
    lost: &mut usize,
) -> Option<Option<Bar>> {
    let Opts {
        chords,
        discard_unisons,
    } = *opts;
    let old = staff.bars.get(k)?;
    let (mut receiver, mut tail) = events(old.voices.get(rv))?;
    // A voice of placeholders only (as Guitar Pro fills an unused voice)
    // holds nothing to keep.
    if !receiver
        .iter()
        .any(|e| matches!(e.beat.kind, BeatKind::Notes | BeatKind::Rest))
    {
        receiver.clear();
    }
    // The donor's events with notes, their notes placed on the receiver's
    // staff, in time order (layer by layer at one moment).
    let mut donor: Vec<Event> = Vec::new();
    let mut tails = Vec::new();
    let mut brings = false;
    // The merged voice lasts as long as the longest voice merged, rests
    // included.
    let mut ends = vec![receiver.last().map_or(Ratio::ZERO, |e| e.end)];
    for layer in &mut layers {
        let Some(from) = layer.staff else {
            continue;
        };
        let (evs, after) = events(from.bars.get(k).and_then(|b| b.voices.get(layer.voice)))?;
        brings |= !after.is_empty();
        let end = evs.last().map_or(Ratio::ZERO, |e| e.end);
        ends.push(end);
        let busy = sounding(&receiver, &donor, end);
        tails.extend(
            after
                .into_iter()
                .map(|g| grace_placed(from, to, g, chords, &busy)),
        );
        for mut ev in evs {
            if ev.beat.kind != BeatKind::Notes || ev.beat.notes.is_empty() {
                continue;
            }
            brings = true;
            ev.graces = std::mem::take(&mut ev.graces)
                .into_iter()
                .map(|g| grace_placed(from, to, g, chords, &sounding(&receiver, &donor, ev.start)))
                .collect();
            ev.beat.chord = ev.beat.chord.and_then(|i| chords.get(i).copied());
            // Tied notes first: they keep their string only if no other
            // note took it.
            let mut notes = std::mem::take(&mut ev.beat.notes);
            notes.sort_by_key(|n| {
                !(n.tie && matches!(layer.ties.get(&tie_key(n)), Some(Went::Placed(_))))
            });
            // What already sounds on the receiver's strings meanwhile.
            let mut beside: Vec<Note> = receiver
                .iter()
                .chain(&donor)
                .filter(|o| o.start < ev.end && o.end > ev.start)
                .flat_map(|o| o.beat.notes.iter().cloned())
                .collect();
            let struck: Vec<u8> = receiver
                .iter()
                .filter(|r| r.start == ev.start && r.beat.kind == BeatKind::Notes)
                .flat_map(|r| &r.beat.notes)
                .filter(|n| !n.tie)
                .filter_map(|n| pitch_of(staff, n))
                .collect();
            for n in notes {
                let key = tie_key(&n);
                let went = match (n.tie, layer.ties.get(&key).copied()) {
                    // A tied note follows the note it continues.
                    (true, Some(Went::Placed(s))) => match tied_on(from, to, &beside, &n, s) {
                        Some(place) => place_note(&mut ev.beat, &mut beside, n, Some(place), true),
                        None => {
                            let place = place_anew(from, to, &beside, &n);
                            place_note(&mut ev.beat, &mut beside, n, place, false)
                        }
                    },
                    // The note it continues was left out: so is it (and
                    // counted there).
                    (true, Some(gone)) => {
                        layer.ties.insert(key, gone);
                        continue;
                    }
                    _ if discard_unisons
                        && !n.tie
                        && pitch_of(from, &n).is_some_and(|p| struck.contains(&p)) =>
                    {
                        Went::Unison
                    }
                    _ => {
                        let place = place_anew(from, to, &beside, &n);
                        let tie = n.tie;
                        place_note(&mut ev.beat, &mut beside, n, place, tie)
                    }
                };
                if went == Went::Lost {
                    *lost += 1;
                }
                layer.ties.insert(key, went);
            }
            if ev.beat.notes.is_empty() {
                // Every note was the receiver's own, or lost: the donor
                // brings nothing here.
                continue;
            }
            ev.beat.notes.sort_by_key(|n| n.string().unwrap_or(u8::MAX));
            donor.push(ev);
        }
    }
    if !brings {
        return Some(None);
    }
    let mut bounds: Vec<Ratio> = receiver
        .iter()
        .chain(&donor)
        .flat_map(|e| [e.start, e.end])
        .chain([Ratio::ZERO])
        .chain(ends)
        .collect();
    bounds.sort();
    bounds.dedup();
    let mut beats = Vec::new();
    for w in bounds.windows(2) {
        let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else {
            return None;
        };
        let len = b.checked_sub(a)?;
        let r = receiver.iter().find(|e| e.start <= a && e.end > a);
        let d: Vec<&Event> = donor.iter().filter(|e| e.start <= a && e.end > a).collect();
        for e in r.into_iter().chain(d.iter().copied()) {
            if e.start == a {
                beats.extend(e.graces.iter().cloned());
            }
        }
        // The beat whose own marks the pieces carry.
        let notes = |e: &&Event| e.beat.kind == BeatKind::Notes;
        let Some(base) = r.filter(notes).or_else(|| d.first().copied()).or(r) else {
            beats.extend(rests(len)?);
            continue;
        };
        let durations = spell(len, &base.beat.duration.tuplets, Prefer::Own)?;
        let n = durations.len();
        for (i, duration) in durations.into_iter().enumerate() {
            let cut = |e: &Event| {
                piece(
                    &e.beat,
                    duration.clone(),
                    e.start == a && i == 0,
                    e.end == b && i + 1 == n,
                )
            };
            let mut beat = cut(base);
            for other in r.filter(notes).into_iter().chain(d.iter().copied()) {
                if !std::ptr::eq(other, base) {
                    beat.notes.extend(cut(other).notes);
                }
            }
            if !beat.notes.is_empty() {
                beat.kind = BeatKind::Notes;
            }
            beat.notes.sort_by_key(|n| n.string().unwrap_or(u8::MAX));
            beats.push(beat);
        }
    }
    beats.append(&mut tail);
    beats.extend(tails);
    let mut bar = old.clone();
    while bar.voices.len() <= rv {
        bar.voices.push(Voice::default());
    }
    if let Some(v) = bar.voices.get_mut(rv) {
        v.beats = beats;
    }
    Some(Some(bar))
}

/// Every tied note of `voice` whose string's note before it has another
/// pitch struck again: a tie that skipped beats (as some files have) may
/// find another track's note on its string in between.
fn strike_broken_ties(staff: &mut Staff, voice: usize) {
    let mut last: BTreeMap<u8, Option<u8>> = BTreeMap::new();
    let mut bars = std::mem::take(&mut staff.bars);
    for bar in &mut bars {
        let Some(v) = bar.voices.get_mut(voice) else {
            continue;
        };
        for n in v.beats.iter_mut().flat_map(|b| &mut b.notes) {
            let NotePlace::Fretted { string, fret } = n.place else {
                continue;
            };
            let pitch = staff.pitch_of(string, fret).map(|p| p.0);
            if n.tie && last.get(&string).is_some_and(|before| *before != pitch) {
                n.tie = false;
            }
            last.insert(string, pitch);
        }
    }
    staff.bars = bars;
}

/// A donor note written into `beat` at `place`; `None` leaves it out.
fn place_note(
    beat: &mut Beat,
    beside: &mut Vec<Note>,
    n: Note,
    place: Option<NotePlace>,
    tie: bool,
) -> Went {
    let Some(place) = place else {
        return Went::Lost;
    };
    let note = Note { place, tie, ..n };
    beside.push(note.clone());
    beat.notes.push(note);
    Went::Placed(note_string(place))
}

fn note_string(place: NotePlace) -> u8 {
    match place {
        NotePlace::Fretted { string, .. } => string,
        NotePlace::Pitched(_) => 0,
    }
}

/// A donor note on the receiver's staff, beside the notes already there.
fn place_anew(from: &Staff, to: &Staff, beside: &[Note], n: &Note) -> Option<NotePlace> {
    crate::tracks::replace(from, to, beside, n, true)
}

/// A tied donor note on string `s` (where the note it continues went), if
/// that string is free and plays its pitch.
fn tied_on(from: &Staff, to: &Staff, beside: &[Note], n: &Note, s: u8) -> Option<NotePlace> {
    let pitch = pitch_of(from, n)?;
    if to.tuning.is_empty() {
        return Some(NotePlace::Pitched(oxiguitar_model::Pitch(pitch)));
    }
    if beside.iter().any(|b| b.string() == Some(s)) {
        return None;
    }
    let fret = pitch.checked_sub(to.pitch_of(s, 0)?.0)?;
    (fret <= to.fret_count).then_some(NotePlace::Fretted { string: s, fret })
}

/// The notes of `receiver` and `donor` that sound at `at`, or end there:
/// the strings a grace note just before `at` must leave alone.
fn sounding(receiver: &[Event], donor: &[Event], at: Ratio) -> Vec<Note> {
    receiver
        .iter()
        .chain(donor)
        .filter(|e| e.start < at && e.end >= at)
        .flat_map(|e| e.beat.notes.iter().cloned())
        .collect()
}

/// A donor grace beat with its notes on the receiver's staff, on strings
/// `busy` leaves free (those no string plays are left out: grace notes,
/// they aren't counted).
fn grace_placed(from: &Staff, to: &Staff, mut g: Beat, chords: &[usize], busy: &[Note]) -> Beat {
    let mut beside = busy.to_vec();
    let mut placed: Vec<Note> = Vec::new();
    for n in std::mem::take(&mut g.notes) {
        if let Some(place) = place_anew(from, to, &beside, &n) {
            let note = Note { place, ..n };
            beside.push(note.clone());
            placed.push(note);
        }
    }
    g.notes = placed;
    g.chord = g.chord.and_then(|i| chords.get(i).copied());
    if g.notes.is_empty() && g.kind == BeatKind::Notes {
        g.kind = BeatKind::Rest;
    }
    g
}

/// Each lyrics line on its notes again: an empty beat for each new note
/// the first voice of the first staff (`new`, once `old`) sings.
fn keep_lyrics(lines: &mut [oxiguitar_model::LyricLine], old: &Staff, new: &Staff) {
    use oxiguitar_model::lyrics;
    for line in lines {
        let before = sung_moments(old, line.start_bar);
        let mut j = 0;
        for (index, moment) in sung_moments(new, line.start_bar).into_iter().enumerate() {
            while before.get(j).is_some_and(|b| *b < moment) {
                j += 1;
            }
            if before.get(j) == Some(&moment) {
                j += 1;
            } else if index < lyrics::syllables(&line.text).len() {
                line.text = lyrics::with_empty_beat(&line.text, index);
            }
        }
    }
}

/// When the first voice sings, from bar `start` on: (bar, moment).
fn sung_moments(staff: &Staff, start: usize) -> Vec<(usize, Ratio)> {
    let mut out = Vec::new();
    for (k, bar) in staff.bars.iter().enumerate().skip(start) {
        let mut at = Ratio::ZERO;
        for b in bar
            .voices
            .first()
            .map(|v| v.beats.as_slice())
            .unwrap_or_default()
        {
            if b.grace.is_some() {
                continue;
            }
            if oxiguitar_model::lyrics::is_sung(b) {
                out.push((k, at));
            }
            at = b
                .duration
                .length()
                .and_then(|l| at.checked_add(l))
                .unwrap_or(at);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caret::tests::score;
    use crate::{Command, apply as run};
    use oxiguitar_model::NoteValue::{Half, Quarter, TwoFiftySixth};
    use oxiguitar_model::{Color, Duration, LyricLine, NoteValue, Pitch};

    /// A beat of notes `(string, fret)`, as long as `value`.
    fn b(value: NoteValue, notes: &[(u8, u8)]) -> Beat {
        Beat {
            duration: Duration::new(value),
            notes: notes.iter().map(|&(s, f)| Note::fretted(s, f)).collect(),
            ..Beat::default()
        }
    }

    /// Two tracks of one bar: the receiver's voice 1 and the donor's.
    fn two(receiver: Vec<Beat>, donor: Vec<Beat>) -> Score {
        let mut s = score(2, 1);
        s.tracks[0].name = "Receiver".into();
        s.tracks[1].name = "Donor".into();
        s.tracks[0].staves[0].bars[0].voices[0].beats = receiver;
        s.tracks[1].staves[0].bars[0].voices[0].beats = donor;
        s
    }

    fn into_first(voices: &[(usize, usize)], discard_unisons: bool) -> Merge {
        Merge {
            into: 0,
            voices: voices.to_vec(),
            discard_unisons,
        }
    }

    /// Runs the merge of track `from` with the caret there.
    fn merge(s: &mut Score, h: &mut History, from: usize, m: Merge) -> Option<Edited> {
        let c = Caret {
            track: from,
            ..Caret::at_start(s, from).unwrap()
        };
        run(s, h, c, None, Command::MergeTracks(m))
    }

    /// A beat's notes as (string, fret, tied), and its length.
    type Seen = (Vec<(u8, u8, bool)>, Ratio);

    /// A voice's beats.
    fn beats(s: &Score, voice: usize) -> Vec<Seen> {
        s.tracks[0].staves[0].bars[0].voices[voice]
            .beats
            .iter()
            .map(|b| {
                let notes = b
                    .notes
                    .iter()
                    .map(|n| match n.place {
                        NotePlace::Fretted { string, fret } => (string, fret, n.tie),
                        NotePlace::Pitched(p) => (0, p.0, n.tie),
                    })
                    .collect();
                (notes, b.duration.length().unwrap())
            })
            .collect()
    }

    fn q() -> Ratio {
        Ratio::new(1, 4).unwrap()
    }

    #[test]
    fn same_rhythms_become_chords_and_the_donor_goes() {
        let mut s = two(
            vec![b(Quarter, &[(1, 0)]); 4],
            vec![b(Quarter, &[(3, 2)]); 4],
        );
        let before = s.clone();
        let mut h = History::new();
        let e = merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        assert_eq!(s.tracks.len(), 1);
        assert_eq!(s.tracks[0].name, "Receiver");
        assert_eq!((e.caret.track, e.caret.voice), (0, 0));
        assert_eq!(
            beats(&s, 0),
            vec![(vec![(1, 0, false), (3, 2, false)], q()); 4]
        );
        assert_eq!(h.undo_label(), Some("Merge Tracks"));
        h.undo(&mut s).unwrap();
        assert_eq!(s, before);
        s.check_structure().unwrap();
    }

    #[test]
    fn different_rhythms_are_cut_and_tied() {
        // Receiver: two halves; donor: four quarters. Each half becomes
        // two tied quarters, the donor's notes beside them.
        let mut s = two(
            vec![b(Half, &[(1, 5)]), b(Half, &[(1, 7)])],
            vec![b(Quarter, &[(3, 2)]); 4],
        );
        let mut h = History::new();
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        assert_eq!(
            beats(&s, 0),
            vec![
                (vec![(1, 5, false), (3, 2, false)], q()),
                (vec![(1, 5, true), (3, 2, false)], q()),
                (vec![(1, 7, false), (3, 2, false)], q()),
                (vec![(1, 7, true), (3, 2, false)], q()),
            ]
        );
        // And the other way round: the donor's halves are tied.
        let mut s = two(
            vec![b(Quarter, &[(3, 2)]); 4],
            vec![b(Half, &[(1, 5)]), b(Half, &[(1, 7)])],
        );
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        assert_eq!(beats(&s, 0)[1].0, [(1, 5, true), (3, 2, false)]);
    }

    #[test]
    fn a_taken_string_moves_the_donor_s_note_or_loses_it() {
        // Receiver G on string 1 (fret 3); donor B (59) on string 1's
        // place is taken: it goes to string 2, open.
        let r = vec![b(Half, &[(1, 3)]), b(Half, &[])];
        let mut d = vec![b(Half, &[(1, 0)]), b(Half, &[])];
        d[0].notes[0].place = NotePlace::Pitched(Pitch(59));
        d[1].kind = BeatKind::Rest;
        let mut s = two(r.clone(), d);
        s.tracks[0].staves[0].bars[0].voices[0].beats[1].kind = BeatKind::Rest;
        let mut h = History::new();
        let m = into_first(&[(0, 0)], false);
        assert_eq!(merge_lost(&s, 1, &m), Ok(0));
        merge(&mut s, &mut h, 1, m.clone()).unwrap();
        assert_eq!(beats(&s, 0)[0].0, [(1, 3, false), (2, 0, false)]);
        // A low E (40) on a receiver of the top four strings (lowest D,
        // 50): no string plays it.
        let mut s = two(r, vec![b(Half, &[(6, 0)]), b(Half, &[(6, 0)])]);
        s.tracks[0].staves[0].tuning.truncate(4);
        assert_eq!(merge_lost(&s, 1, &m), Ok(2));
        merge(&mut s, &mut h, 1, m).unwrap();
        assert_eq!(beats(&s, 0)[0].0, [(1, 3, false)]);
    }

    #[test]
    fn unisons_struck_together_are_discarded_when_asked() {
        // Receiver E4 (string 2, fret 5); the donor's open string 1 is the
        // same E4, and its tied continuation goes with it.
        let r = vec![b(Quarter, &[(2, 5)]); 4];
        let mut d = vec![b(Quarter, &[(1, 0)]); 4];
        d[1].notes[0].tie = true;
        let mut s = two(r.clone(), d.clone());
        let mut h = History::new();
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], true)).unwrap();
        assert_eq!(beats(&s, 0), vec![(vec![(2, 5, false)], q()); 4]);
        // Not asked: both kept, on their own strings.
        let mut s = two(r, d);
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        assert_eq!(beats(&s, 0)[1].0, [(1, 0, true), (2, 5, false)]);
        // A held receiver note isn't struck with the donor's.
        let mut s = two(
            vec![b(Half, &[(2, 5)]), b(Half, &[(2, 5)])],
            vec![b(Quarter, &[(1, 0)]); 4],
        );
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], true)).unwrap();
        assert_eq!(beats(&s, 0)[0].0, [(2, 5, false)]);
        assert_eq!(beats(&s, 0)[1].0, [(1, 0, false), (2, 5, true)]);
    }

    #[test]
    fn voices_go_where_they_are_sent() {
        let mut s = two(
            vec![b(Quarter, &[(1, 0)]); 4],
            vec![b(Half, &[(4, 2)]), b(Half, &[(4, 4)])],
        );
        s.tracks[1].staves[0].bars[0].voices.push(Voice {
            beats: vec![b(Half, &[(6, 3)]); 2],
        });
        let mut h = History::new();
        // One voice, to the receiver's second voice: the first is as it was.
        let m = into_first(&[(1, 1)], false);
        let first = beats(&s, 0);
        let e = merge(&mut s.clone(), &mut h, 1, m.clone()).unwrap();
        assert_eq!(e.caret.voice, 1);
        let mut one = s.clone();
        merge(&mut one, &mut h, 1, m).unwrap();
        assert_eq!(beats(&one, 0), first);
        assert_eq!(
            beats(&one, 1),
            vec![(vec![(6, 3, false)], Ratio::new(1, 2).unwrap()); 2]
        );
        // Voice by voice.
        merge(&mut s, &mut h, 1, into_first(&[(0, 0), (1, 1)], false)).unwrap();
        assert_eq!(beats(&s, 0)[1].0, [(1, 0, false), (4, 2, true)]);
        assert_eq!(beats(&s, 1).len(), 2);
    }

    #[test]
    fn the_receiver_s_lyrics_keep_their_notes() {
        let mut s = two(
            vec![b(Half, &[(1, 5)]), b(Half, &[(1, 7)])],
            vec![b(Quarter, &[(3, 2)]); 4],
        );
        s.tracks[0].lyrics = vec![LyricLine {
            start_bar: 0,
            text: "Hel-lo".into(),
        }];
        let mut h = History::new();
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        let sung: Vec<String> = oxiguitar_model::lyrics::syllables(&s.tracks[0].lyrics[0].text)
            .into_iter()
            .map(|s| s.text)
            .collect();
        assert_eq!(sung, ["Hel", "", "lo"]);
    }

    #[test]
    fn rhythms_no_value_can_show_stop_the_merge() {
        // A double-dotted 256th beside a 256th leaves a piece shorter than
        // any note value.
        let mut tiny = b(TwoFiftySixth, &[(1, 0)]);
        tiny.duration.dots = 2;
        let mut s = two(vec![tiny], vec![b(TwoFiftySixth, &[(3, 0)])]);
        let m = into_first(&[(0, 0)], false);
        assert_eq!(merge_lost(&s, 1, &m), Err(MergeError::Rhythms { bar: 0 }));
        let mut h = History::new();
        assert!(merge(&mut s, &mut h, 1, m).is_none());
        assert_eq!(s.tracks.len(), 2);
        assert_eq!(
            merge_lost(&s, 0, &into_first(&[(0, 0)], false)),
            Err(MergeError::Nothing),
            "not into itself"
        );
    }

    #[test]
    fn the_merged_voice_lasts_as_long_as_the_longest() {
        // The donor's half note then half rest, into an empty voice 2: a
        // whole bar still, the rest kept.
        let mut rest = b(Half, &[]);
        rest.kind = BeatKind::Rest;
        let mut s = two(
            vec![b(Quarter, &[(1, 0)]); 4],
            vec![b(Half, &[(6, 0)]), rest],
        );
        let mut h = History::new();
        merge(&mut s, &mut h, 1, into_first(&[(0, 1)], false)).unwrap();
        let v = &s.tracks[0].staves[0].bars[0].voices[1];
        assert_eq!(v.length(), Some(Ratio::ONE));
        assert_eq!(v.beats[1].kind, BeatKind::Rest);
        // A voice of placeholders only is no voice: a pickup's two
        // sixteenths stay a pickup.
        let mut empty = b(Quarter, &[]);
        empty.kind = BeatKind::Empty;
        let mut s = two(vec![empty], vec![b(NoteValue::Sixteenth, &[(5, 0)]); 2]);
        merge(&mut s, &mut h, 1, into_first(&[(0, 0)], false)).unwrap();
        assert_eq!(
            s.tracks[0].staves[0].bars[0].voices[0].length(),
            Ratio::new(1, 8)
        );
    }

    #[test]
    fn drums_stay_keys_and_only_join_drums() {
        // Two drum tracks as Guitar Pro 3 to 5 files give them: six strings
        // tuned to nothing. Kick (36) and hi-hat (42) stay keys.
        let key = |k: u8| Beat {
            duration: Duration::new(Quarter),
            notes: vec![Note {
                place: NotePlace::Pitched(Pitch(k)),
                ..Note::fretted(1, 0)
            }],
            ..Beat::default()
        };
        let mut s = two(vec![key(36); 4], vec![key(42); 4]);
        for t in &mut s.tracks {
            t.instrument.percussion = true;
            t.staves[0].tuning = vec![Pitch(0); 6];
        }
        let mut h = History::new();
        let m = into_first(&[(0, 0)], true);
        assert_eq!(merge_lost(&s, 1, &m), Ok(0));
        let mut guitar = s.clone();
        guitar.tracks[0].instrument.percussion = false;
        assert_eq!(merge_lost(&guitar, 1, &m), Err(MergeError::Kinds));
        merge(&mut s, &mut h, 1, m).unwrap();
        assert_eq!(
            beats(&s, 0),
            vec![(vec![(0, 36, false), (0, 42, false)], q()); 4]
        );
    }

    #[test]
    fn what_names_tracks_by_number_follows() {
        let mut s = score(4, 1);
        s.tracks[0].chords_from = 2;
        s.tracks[3].chords_from = 1;
        let red = Color { r: 255, g: 0, b: 0 };
        s.track_bar_colors.insert(2, vec![Some(red)]);
        s.track_bar_colors.insert(3, vec![None]);
        // Track 3 (index 2) merged into track 2 (index 1).
        remove_track(&mut s, 2, Some(1));
        assert_eq!(s.tracks[0].chords_from, 1, "its chords are track 2's now");
        assert_eq!(s.tracks[2].chords_from, 1);
        assert_eq!(s.track_bar_colors.keys().copied().collect::<Vec<_>>(), [2]);
        // Deleting: whoever showed it shows nothing.
        remove_track(&mut s, 1, None);
        assert_eq!(s.tracks[0].chords_from, usize::MAX);
        assert_eq!(s.tracks[1].chords_from, usize::MAX);
    }
}
