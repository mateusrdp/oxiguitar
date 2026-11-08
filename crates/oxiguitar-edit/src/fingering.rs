// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Automatic fingering (#157, the brief's TAB-3 and TAB-4): a string and
//! fret for each note, so a hand can play the passage with as little effort
//! as it can. See `docs/design/fingering.md`.
//!
//! The search is a cheapest path through the passage (Viterbi). A staff's
//! notes are cut into slices, one per moment a note is struck, across its
//! voices (one hand plays them all). Each slice has candidates: every way to
//! put its notes on distinct strings, with a hand that can hold them (at
//! most four fingers, the index barring, the thumb on a bass string at a
//! price, a limited stretch) and an index position on the neck. A candidate
//! costs its stretch, barres and thumb, and its distance from the beat's
//! chord diagram; moving from one candidate to the next costs the hand's
//! shift (cheap within two or three frets, dearer the less time there is),
//! string skipping between single notes and a hammer, pull or slide that
//! leaves its string. Notes still ringing keep their place and their
//! string. The whole neck is equally good (Mateus, 8 Nov 2026): no fret or
//! open string is preferred for its own sake.
//!
//! Harmonics stay the same harmonics: a natural harmonic may move to any
//! string and node that sounds the same note; other kinds keep their pitch
//! and their offset (the touched fret of a tapped harmonic moves with the
//! note). A tied note keeps the string of the note it continues.

use std::collections::BTreeMap;

use oxiguitar_model::{ChordDiagram, Harmonic, NotePlace, Ratio, Score, Slide, Staff};

use crate::{Caret, Command, Edited, History, Scope, Selection};

/// The text above a beat whose fingering needs a stretch past what a hand
/// does comfortably.
pub const HARD_STRETCH: &str = "Hard: stretch";
/// The text above a beat that needs more fingers than a hand has.
pub const HARD_FINGERS: &str = "Hard: fingers";
/// The text above a beat reached by a long, fast shift.
pub const HARD_SHIFT: &str = "Hard: shift";
const HARD_TEXTS: [&str; 3] = [HARD_STRETCH, HARD_FINGERS, HARD_SHIFT];

/// Frets where a natural harmonic sounds, and how far above the open
/// string, as the player plays them (the same table as playback's).
fn natural(fret: u8) -> Option<u8> {
    Some(match fret {
        12 => 12,
        7 | 19 => 19,
        5 | 24 => 24,
        4 | 9 | 16 => 28,
        3 => 31,
        _ => return None,
    })
}

/// A note a slice's shape leaves where it was: no string is free for it.
const UNPLACED: (u8, u8) = (0, 0);

/// Candidates kept for each slice: the cheapest by their own cost.
const KEPT: usize = 48;
/// The most assignments tried for one slice.
const BUDGET: usize = 40_000;
/// What an impossible hand costs, on top of its other costs: a path takes
/// one only where nothing else plays the notes.
const HARD: f64 = 100.0;

/// Runs [`Command::Fingering`]; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let Command::Fingering(scope) = command else {
        return None;
    };
    let mut work = score.clone();
    if !finger_scope(&mut work, c, selection, *scope) {
        return Some(None);
    }
    let regions = crate::transpose::regions(score, c, selection, *scope);
    let touched = history.record(score, command.label(), regions, |s| *s = work);
    Some(Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    }))
}

/// Fingers every note of `scope` again; whether anything changed. Beats
/// that stay hard to play get a text saying why (Mateus, 8 Nov 2026).
fn finger_scope(score: &mut Score, c: Caret, selection: Option<Selection>, scope: Scope) -> bool {
    let spans = crate::transpose::spans(score, c, selection, scope);
    // The spans of each staff.
    let mut by_staff: BTreeMap<(usize, usize), Vec<crate::Span>> = BTreeMap::new();
    for s in spans {
        by_staff
            .entry((s.at.track, s.at.staff))
            .or_default()
            .push(s);
    }
    let starts = bar_starts(score);
    let mut changed = false;
    for ((t, st), spans) in by_staff {
        let Some(track) = score.tracks.get_mut(t) else {
            continue;
        };
        if track.instrument.percussion {
            continue;
        }
        let chords = track.chords.clone();
        let Some(staff) = track.staves.get_mut(st) else {
            continue;
        };
        let free = |at: At| {
            spans.iter().any(|s| {
                s.at.bar == at.bar
                    && match s.beats {
                        None => true,
                        Some((v, from, to)) => at.voice == v && (from..=to).contains(&at.beat),
                    }
            })
        };
        changed |= finger_staff(staff, &chords, &starts, &free, true).changed;
    }
    changed
}

/// Gives a string and fret to every note that has only a pitch, on every
/// staff with strings (not drums): a file read from a format without
/// tablature (MusicXML, MIDI files, ABC) shows its tab at once. Notes that
/// have a string keep it, and no text is written. Returns how many notes
/// were placed.
pub fn finger_pitched(score: &mut Score) -> usize {
    let starts = bar_starts(score);
    let mut placed = 0;
    for track in &mut score.tracks {
        if track.instrument.percussion {
            continue;
        }
        let chords = track.chords.clone();
        for staff in &mut track.staves {
            if staff.tuning.is_empty() {
                continue;
            }
            let pitched: Vec<At> = notes_at(staff)
                .filter(|(_, n)| matches!(n, NotePlace::Pitched(_)))
                .map(|(at, _)| at)
                .collect();
            if pitched.is_empty() {
                continue;
            }
            let free = |at: At| pitched.binary_search(&at).is_ok();
            placed += finger_staff(staff, &chords, &starts, &free, false).placed;
        }
    }
    placed
}

/// Every note of a staff with its place, in [`At`] order.
fn notes_at(staff: &Staff) -> impl Iterator<Item = (At, NotePlace)> + '_ {
    staff.bars.iter().enumerate().flat_map(|(bar, b)| {
        b.voices.iter().enumerate().flat_map(move |(voice, v)| {
            v.beats.iter().enumerate().flat_map(move |(beat, bt)| {
                bt.notes.iter().enumerate().map(move |(note, n)| {
                    (
                        At {
                            bar,
                            voice,
                            beat,
                            note,
                        },
                        n.place,
                    )
                })
            })
        })
    })
}

/// Where each bar starts, in whole notes from the start of the score: each
/// bar as long as its time signature says.
fn bar_starts(score: &Score) -> Vec<f64> {
    let mut at = 0.0;
    score
        .master_bars
        .iter()
        .map(|mb| {
            let start = at;
            at += mb.expected_length().map_or(1.0, Ratio::to_f64);
            start
        })
        .collect()
}

/// A note's address in a staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct At {
    bar: usize,
    voice: usize,
    beat: usize,
    note: usize,
}

/// What fingering a staff did.
#[derive(Debug, Default)]
struct Outcome {
    changed: bool,
    placed: usize,
}

/// A note as the search sees it.
#[derive(Debug)]
struct Item {
    at: At,
    /// When it is struck, and when it stops, in whole notes.
    time: f64,
    end: f64,
    /// Where it may go (string, fret); one place for a note kept as it is.
    options: Vec<(u8, u8)>,
    /// Whether the search may move it.
    free: bool,
    /// The item it sounds with: the note a tied note continues, or the
    /// same note struck by another voice at the same moment.
    follows: Option<usize>,
    /// The note it hammers, pulls or slides to.
    legato_to: Option<usize>,
    /// A tie that continues nothing: the note is struck instead.
    untie: bool,
    /// Its place before.
    old: Option<(u8, u8)>,
    /// The sounding pitch (with a natural harmonic, the harmonic's).
    pitch: u8,
    /// Its beat's chord diagram.
    chord: Option<usize>,
}

/// One hand shape for a slice.
#[derive(Clone, Debug)]
struct Cand {
    /// A place per member of the slice, in the slice's member order.
    places: Vec<(u8, u8)>,
    cost: f64,
    hard: Option<&'static str>,
    /// Where the index can be: the frets the hand covers start there.
    positions: Vec<Option<u8>>,
}

/// A moment notes are struck.
#[derive(Debug)]
struct Slice {
    time: f64,
    /// The notes struck, then the notes still ringing from before.
    members: Vec<usize>,
    struck: usize,
    cands: Vec<Cand>,
}

/// Fingers the notes of `staff` that `free` names, around the others.
fn finger_staff(
    staff: &mut Staff,
    chords: &[ChordDiagram],
    starts: &[f64],
    free: &dyn Fn(At) -> bool,
    marks: bool,
) -> Outcome {
    if staff.tuning.is_empty() {
        return Outcome::default();
    }
    let mut items = collect(staff, starts, free);
    let mut slices = slice(&mut items);
    // The slices with something to place, and one either side as context.
    let Some(first) = slices
        .iter()
        .position(|s| s.members[..s.struck].iter().any(|&i| items[i].free))
    else {
        // Only notes that follow others (ties) may still need their place.
        return write(staff, &items, &[], &[], marks, free);
    };
    let last = slices
        .iter()
        .rposition(|s| s.members[..s.struck].iter().any(|&i| items[i].free))
        .unwrap_or(first);
    let window = first.saturating_sub(1)..=(last + 1).min(slices.len() - 1);
    let slices = &mut slices[window];
    for k in 0..slices.len() {
        let (before, rest) = slices.split_at_mut(k);
        let prev = before.last();
        let cur = &mut rest[0];
        cur.cands = candidates(staff, chords, &items, cur, prev);
    }
    let path = viterbi(&items, slices);
    // The hard spots: hands past the limits, and long fast shifts.
    let mut hard: Vec<Option<&'static str>> = path
        .iter()
        .zip(slices.iter())
        .map(|(&(c, _), s)| s.cands.get(c).and_then(|c| c.hard))
        .collect();
    for k in 1..path.len() {
        let (pa, pb) = (path[k - 1].1, path[k].1);
        let dt = slices[k].time - slices[k - 1].time;
        if let (Some(a), Some(b)) = (pa, pb)
            && a.abs_diff(b) >= 7
            && dt <= 0.125 + 1e-9
            && hard[k].is_none()
        {
            hard[k] = Some(HARD_SHIFT);
        }
    }
    // Each item's new place.
    let mut place: Vec<Option<(u8, u8)>> = vec![None; items.len()];
    for (s, &(c, _)) in slices.iter().zip(&path) {
        for (m, &i) in s.members.iter().enumerate().take(s.struck) {
            place[i] = s.cands.get(c).and_then(|c| c.places.get(m).copied());
        }
    }
    let hard_at: Vec<(At, &'static str)> = slices
        .iter()
        .zip(&hard)
        .filter_map(|(s, h)| {
            let h = (*h)?;
            let i = *s.members[..s.struck].iter().find(|&&i| items[i].free)?;
            Some((items[i].at, h))
        })
        .collect();
    write(staff, &items, &place, &hard_at, marks, free)
}

/// The staff's notes as items: their times, places and links.
fn collect(staff: &Staff, starts: &[f64], free: &dyn Fn(At) -> bool) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    // The last item of each voice by string, and by pitch for notes
    // without one, for ties: a tied note continues the note before it on
    // its string (as `retie` and playback have it), whatever fret it says.
    let mut last: BTreeMap<(usize, u8), usize> = BTreeMap::new();
    let mut last_pitch: BTreeMap<(usize, u8), usize> = BTreeMap::new();
    // Each voice's beats in order, as the items they hold, for legato.
    let mut beats: BTreeMap<usize, Vec<Vec<usize>>> = BTreeMap::new();
    for (bar, b) in staff.bars.iter().enumerate() {
        let start = starts.get(bar).copied().unwrap_or_else(|| {
            starts.last().copied().unwrap_or(0.0) + (bar + 1 - starts.len()) as f64
        });
        for (voice, v) in b.voices.iter().enumerate() {
            let on = crate::onsets(v);
            // Grace beats come just before their main beat, in order.
            let mut grace_rank = vec![0i32; v.beats.len()];
            let mut run = 0;
            for (k, bt) in v.beats.iter().enumerate().rev() {
                if bt.grace.is_some() {
                    run += 1;
                    grace_rank[k] = -run;
                } else {
                    run = 0;
                }
            }
            for (beat, bt) in v.beats.iter().enumerate() {
                let onset = on.get(beat).map_or(0.0, |r| r.to_f64());
                let time = start + onset + f64::from(grace_rank[beat]) / 256.0;
                let length = if bt.grace.is_some() {
                    1.0 / 256.0
                } else {
                    bt.duration.length().map_or(0.0, Ratio::to_f64)
                };
                let mut held = Vec::new();
                for (note, n) in bt.notes.iter().enumerate() {
                    let at = At {
                        bar,
                        voice,
                        beat,
                        note,
                    };
                    let natural_h = n.effects.harmonic == Some(Harmonic::Natural);
                    let (pitch, old) = match n.place {
                        NotePlace::Fretted { string, fret } => {
                            let Some(open) = staff.pitch_of(string, 0) else {
                                continue;
                            };
                            let p = if natural_h {
                                natural(fret)
                                    .map_or(open.0.saturating_add(12), |h| open.0.saturating_add(h))
                            } else {
                                open.0.saturating_add(fret)
                            };
                            (p, Some((string, fret)))
                        }
                        NotePlace::Pitched(p) => (p.0, None),
                    };
                    let is_free = free(at);
                    let follows = match (n.tie, old) {
                        (false, _) => None,
                        (true, Some((string, _))) => last.get(&(voice, string)).copied(),
                        (true, None) => last_pitch.get(&(voice, pitch)).copied(),
                    };
                    // A tie continues a note still ringing up to it; one
                    // that continues nothing (some files have them) is
                    // struck, and loses its tie.
                    let follows = follows.filter(|&f| {
                        items
                            .get(root_of(&items, f))
                            .is_some_and(|r| r.end + 1e-6 >= time)
                    });
                    let untie = n.tie && follows.is_none();
                    let pitch = follows.map_or(pitch, |f| items[f].pitch);
                    let mut options = if is_free && follows.is_none() {
                        places_for(staff, pitch, n, old)
                    } else {
                        Vec::new()
                    };
                    // Its own place stays possible, even past the neck's
                    // frets where a file put it.
                    if let Some(o) = old
                        && !options.is_empty()
                        && !options.contains(&o)
                    {
                        options.push(o);
                    }
                    let mut moves = is_free;
                    if options.is_empty() {
                        // Kept where it is (or left as a pitch).
                        moves = false;
                        options.extend(old);
                    }
                    let idx = items.len();
                    let root = follows.map(|f| root_of(&items, f));
                    if let Some(r) = root {
                        let end = time + length;
                        if let Some(ri) = items.get_mut(r)
                            && ri.end < end
                        {
                            ri.end = end;
                        }
                    }
                    items.push(Item {
                        at,
                        time,
                        end: time + length,
                        options,
                        free: moves,
                        follows: root,
                        legato_to: None,
                        untie,
                        old,
                        pitch,
                        chord: bt.chord,
                    });
                    if let Some((string, _)) = old {
                        last.insert((voice, string), idx);
                    }
                    last_pitch.insert((voice, pitch), idx);
                    held.push(idx);
                }
                beats.entry(voice).or_default().push(held);
            }
        }
    }
    // Hammers, pulls and slides go to the next note on their string in
    // their voice. In the next beat, the search keeps the two on one
    // string; otherwise the note keeps its place (and the note further on
    // that it reaches, if any), or the legato would reach another note. A note with only a pitch goes to the next beat's
    // only note.
    let mut pinned: Vec<usize> = Vec::new();
    for list in beats.values() {
        for (k, now) in list.iter().enumerate() {
            for &i in now {
                let Some(n) = note_of(staff, items[i].at) else {
                    continue;
                };
                let legato = n.effects.hammer_pull
                    || n.effects
                        .slides
                        .iter()
                        .any(|s| matches!(s, Slide::Shift | Slide::Legato));
                if !legato {
                    continue;
                }
                let Some(next) = list.get(k + 1) else {
                    continue;
                };
                let Some(string) = items[i].old.map(|o| o.0) else {
                    items[i].legato_to = (next.len() == 1).then(|| next[0]);
                    continue;
                };
                let on_string = |j: &usize| items[*j].old.map(|o| o.0) == Some(string);
                if let Some(&j) = next.iter().find(|j| on_string(j)) {
                    items[i].legato_to = Some(j);
                } else {
                    pinned.push(i);
                    pinned.extend(list[k + 1..].iter().flatten().find(|j| on_string(j)));
                }
            }
        }
    }
    for i in pinned {
        let it = &mut items[i];
        if let Some(o) = it.old {
            it.options = vec![o];
            it.free = false;
        }
    }
    items
}

fn root_of(items: &[Item], mut i: usize) -> usize {
    while let Some(f) = items.get(i).and_then(|it| it.follows) {
        i = f;
    }
    i
}

fn note_of(staff: &Staff, at: At) -> Option<&oxiguitar_model::Note> {
    staff
        .bars
        .get(at.bar)?
        .voices
        .get(at.voice)?
        .beats
        .get(at.beat)?
        .notes
        .get(at.note)
}

/// Every string and fret that plays `pitch` as `n` does: for a natural
/// harmonic, every node that sounds it; otherwise the fretted pitch, with
/// a trill's or tapped harmonic's other fret still on the neck.
fn places_for(
    staff: &Staff,
    pitch: u8,
    n: &oxiguitar_model::Note,
    old: Option<(u8, u8)>,
) -> Vec<(u8, u8)> {
    let strings = u8::try_from(staff.string_count()).unwrap_or(0);
    let fx = &n.effects;
    let shaped = fx.bend.is_some() || fx.vibrato || !fx.slides.is_empty() || fx.trill.is_some();
    let mut out = Vec::new();
    for s in 1..=strings {
        let Some(open) = staff.pitch_of(s, 0).map(|p| p.0) else {
            continue;
        };
        if n.effects.harmonic == Some(Harmonic::Natural) {
            for f in 3..=staff.fret_count {
                if natural(f).is_some_and(|h| open.saturating_add(h) == pitch) {
                    out.push((s, f));
                }
            }
            continue;
        }
        let Some(f) = pitch.checked_sub(open).filter(|f| *f <= staff.fret_count) else {
            continue;
        };
        // What the left hand does to a note needs it fretted: a bend,
        // vibrato, a slide or a trill on an open string can't be played.
        if f == 0 && shaped && old.is_none_or(|o| o.1 != 0) {
            continue;
        }
        let delta = i16::from(f) - i16::from(old.map_or(f, |o| o.1));
        let fits = |other: u8| {
            let o = i16::from(other) + delta;
            (0..=i16::from(staff.fret_count)).contains(&o)
        };
        if let Some(t) = n.effects.trill
            && old.is_some()
            && !fits(t.fret)
        {
            continue;
        }
        if let Some(Harmonic::Tapped { fret: touched }) = n.effects.harmonic
            && old.is_some()
            && !fits(touched)
        {
            continue;
        }
        out.push((s, f));
    }
    out
}

/// The items cut into slices: one per moment notes are struck, in time
/// order, each with the notes still ringing into it.
fn slice(items: &mut [Item]) -> Vec<Slice> {
    // Struck notes by moment, as exact keys.
    let mut by_time: BTreeMap<(usize, i64), Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if it.follows.is_some() || it.options.is_empty() {
            continue;
        }
        // Times are sums of exact fractions: rounding to 1/3072 of a whole
        // note keeps equal moments equal.
        let key = (it.at.bar, (it.time * 3072.0).round() as i64);
        by_time.entry(key).or_default().push(i);
    }
    let mut slices: Vec<Slice> = Vec::new();
    for (_, mut members) in by_time {
        // The same note struck by two voices at once is played once.
        let mut kept: Vec<usize> = Vec::new();
        for &i in &members {
            match kept.iter().find(|&&k| items[k].pitch == items[i].pitch) {
                Some(&k) => {
                    // It rings as long as either does (a tie in one voice).
                    items[k].end = items[k].end.max(items[i].end);
                    items[i].follows = Some(k);
                }
                None => kept.push(i),
            }
        }
        members = kept;
        let time = items[members[0]].time;
        let struck = members.len();
        // Still ringing: struck before and not over yet.
        if let Some(prev) = slices.last() {
            for &i in &prev.members {
                if items[i].end > time + 1e-9 && !members.contains(&i) {
                    members.push(i);
                }
            }
        }
        slices.push(Slice {
            time,
            members,
            struck,
            cands: Vec::new(),
        });
    }
    slices
}

/// A slice's candidates, the cheapest kept.
fn candidates(
    staff: &Staff,
    chords: &[ChordDiagram],
    items: &[Item],
    slice: &Slice,
    prev: Option<&Slice>,
) -> Vec<Cand> {
    // Each member's places: a note still ringing stays where the slice
    // before can have it.
    let options: Vec<Vec<(u8, u8)>> = slice
        .members
        .iter()
        .enumerate()
        .map(|(m, &i)| {
            let own = items[i].options.clone();
            if m < slice.struck {
                return own;
            }
            let Some(p) = prev else { return own };
            let Some(pm) = p.members.iter().position(|&x| x == i) else {
                return own;
            };
            let mut from: Vec<(u8, u8)> = p
                .cands
                .iter()
                .filter_map(|c| c.places.get(pm).copied())
                .collect();
            from.sort_unstable();
            from.dedup();
            if from.is_empty() { own } else { from }
        })
        .collect();
    let mut found = Vec::new();
    let mut budget = BUDGET;
    enumerate(
        &options,
        Some(6),
        &mut Vec::new(),
        0,
        &mut found,
        &mut budget,
    );
    if found.is_empty() {
        budget = BUDGET;
        enumerate(&options, None, &mut Vec::new(), 0, &mut found, &mut budget);
    }
    let mut stuck = false;
    if found.is_empty() {
        // More notes than strings, or notes still ringing where the struck
        // ones must go: the struck notes first, the ringing ones cut short
        // where they share a string (a new note stops an old one).
        stuck = true;
        budget = BUDGET;
        let struck: Vec<Vec<(u8, u8)>> = options.iter().take(slice.struck).cloned().collect();
        let mut first = Vec::new();
        enumerate(&struck, None, &mut Vec::new(), 0, &mut first, &mut budget);
        let held = options
            .iter()
            .skip(slice.struck)
            .map(|o| o.first().copied().unwrap_or(UNPLACED));
        found = first
            .into_iter()
            .map(|mut p| {
                // A ringing note that shares a struck note's string stops.
                for h in held.clone() {
                    p.push(if p.iter().any(|q| q.0 == h.0) {
                        UNPLACED
                    } else {
                        h
                    });
                }
                p
            })
            .collect();
        if found.is_empty() {
            // Each note on the first string still free, in turn; one that
            // finds none stays where it was.
            let mut used = Vec::new();
            let greedy = options
                .iter()
                .map(|o| match o.iter().find(|p| !used.contains(&p.0)) {
                    Some(&p) => {
                        used.push(p.0);
                        p
                    }
                    None => UNPLACED,
                })
                .collect();
            found.push(greedy);
        }
    }
    let inherited: Vec<Option<u8>> = match prev {
        Some(p) => {
            let mut v: Vec<Option<u8>> = p
                .cands
                .iter()
                .flat_map(|c| c.positions.iter().copied())
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        }
        None => vec![None],
    };
    let mut cands: Vec<Cand> = found
        .into_iter()
        .map(|places| {
            let (mut cost, mut hard, positions) = hand(staff, &places, slice.struck);
            if stuck {
                cost += HARD;
                hard = hard.or(Some(HARD_FINGERS));
            }
            // The beat's chord diagram: its strings and frets.
            for (m, &(s, f)) in places.iter().enumerate().take(slice.struck) {
                if (s, f) == UNPLACED {
                    continue;
                }
                let it = &items[slice.members[m]];
                if let Some(d) = it.chord.and_then(|c| chords.get(c))
                    && d.frets.iter().any(Option::is_some)
                    && d.frets.get(usize::from(s) - 1).copied().flatten() != Some(f)
                {
                    cost += 0.8;
                }
                // Ties only: the note's own place first, then the lower fret.
                if it.old.is_some_and(|o| o != (s, f)) {
                    cost += 0.01;
                }
                cost += 0.0005 * f64::from(f);
            }
            let positions = if positions.is_empty() {
                inherited.clone()
            } else {
                positions
            };
            Cand {
                places,
                cost,
                hard,
                positions,
            }
        })
        .collect();
    cands.sort_by(|a, b| a.cost.total_cmp(&b.cost));
    cands.truncate(KEPT);
    cands
}

/// Every way to give each member one of its places on distinct strings;
/// with `span`, none whose fretted notes spread wider.
fn enumerate(
    options: &[Vec<(u8, u8)>],
    span: Option<u8>,
    chosen: &mut Vec<(u8, u8)>,
    used: u64,
    out: &mut Vec<Vec<(u8, u8)>>,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    let Some(opts) = options.get(chosen.len()) else {
        out.push(chosen.clone());
        return;
    };
    for &(s, f) in opts {
        let bit = 1u64 << (s.min(63));
        if used & bit != 0 {
            continue;
        }
        if let Some(limit) = span
            && f > 0
        {
            let fretted = chosen.iter().map(|p| p.1).filter(|f| *f > 0);
            let (lo, hi) = fretted.fold((f, f), |(lo, hi), x| (lo.min(x), hi.max(x)));
            if hi - lo > limit {
                continue;
            }
        }
        chosen.push((s, f));
        enumerate(options, span, chosen, used | bit, out, budget);
        chosen.pop();
    }
}

/// What holding `places` costs the hand, whether it can't be done
/// comfortably at all, and where the index can be. Ringing notes count
/// like struck ones: their fingers stay down.
fn hand(
    staff: &Staff,
    places: &[(u8, u8)],
    struck: usize,
) -> (f64, Option<&'static str>, Vec<Option<u8>>) {
    let fretted: Vec<(u8, u8)> = places
        .iter()
        .copied()
        .filter(|p| p.0 > 0 && p.1 > 0)
        .collect();
    if fretted.is_empty() {
        return (0.0, None, Vec::new());
    }
    let lowest_strings = u8::try_from(staff.string_count())
        .unwrap_or(0)
        .saturating_sub(1);
    let plain = shape(&fretted);
    // The thumb over the neck, on one of the two lowest strings, for a
    // bass note at or behind the index: chords only.
    let thumbed = if places.len() >= 3 && struck >= 2 {
        fretted
            .iter()
            .enumerate()
            .filter(|(_, (s, _))| *s >= lowest_strings)
            .max_by_key(|(_, (s, _))| *s)
            .and_then(|(k, &(_, f))| {
                let rest: Vec<(u8, u8)> = fretted
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != k)
                    .map(|(_, p)| *p)
                    .collect();
                let lo = rest.iter().map(|p| p.1).min()?;
                (f <= lo && f + 2 >= lo).then(|| {
                    let mut s = shape(&rest);
                    s.0 += 3.0;
                    s
                })
            })
    } else {
        None
    };
    match thumbed {
        Some(t) if t.0 < plain.0 => t,
        _ => plain,
    }
}

/// [`hand`] for fretted notes with fingers 1 to 4 only.
fn shape(fretted: &[(u8, u8)]) -> (f64, Option<&'static str>, Vec<Option<u8>>) {
    let Some(lo) = fretted.iter().map(|p| p.1).min() else {
        return (0.0, None, Vec::new());
    };
    let hi = fretted.iter().map(|p| p.1).max().unwrap_or(lo);
    let mut cost = 0.0;
    let mut hard = None;
    // Fingers: the index barres its fret; every other note needs a finger
    // of its own, unless another finger barres (avoided, at a price).
    let mut groups: BTreeMap<u8, usize> = BTreeMap::new();
    for p in fretted {
        *groups.entry(p.1).or_default() += 1;
    }
    let mut others: Vec<usize> = groups
        .iter()
        .filter(|(f, _)| **f != lo)
        .map(|(_, n)| *n)
        .collect();
    let mut fingers = 1 + others.iter().sum::<usize>();
    others.sort_unstable_by(|a, b| b.cmp(a));
    for n in others {
        if fingers <= 4 {
            break;
        }
        if n > 1 {
            fingers -= n - 1;
            cost += 2.0;
        }
    }
    if fingers > 4 {
        cost += HARD;
        hard = Some(HARD_FINGERS);
    }
    let stretch = hi - lo;
    cost += match stretch {
        0..=3 => 0.0,
        4 => {
            if lo >= 5 {
                1.5
            } else {
                3.0
            }
        }
        5 if lo >= 7 => 4.0,
        _ => {
            hard = hard.or(Some(HARD_STRETCH));
            HARD + f64::from(stretch)
        }
    };
    // Where the index can be: anywhere the four-fret box still covers the
    // notes, or at the lowest note when they spread wider.
    let positions = if stretch <= 3 {
        (hi.saturating_sub(3).max(1)..=lo).map(Some).collect()
    } else {
        vec![Some(lo)]
    };
    (cost, hard, positions)
}

/// The cheapest path through the slices: a candidate and index position
/// for each.
fn viterbi(items: &[Item], slices: &[Slice]) -> Vec<(usize, Option<u8>)> {
    // Each slice's states: (candidate, position).
    let states: Vec<Vec<(usize, Option<u8>)>> = slices
        .iter()
        .map(|s| {
            s.cands
                .iter()
                .enumerate()
                .flat_map(|(c, cand)| cand.positions.iter().map(move |p| (c, *p)))
                .collect()
        })
        .collect();
    let mut cost: Vec<Vec<f64>> = Vec::with_capacity(slices.len());
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(slices.len());
    for (k, s) in slices.iter().enumerate() {
        let here = &states[k];
        if k == 0 {
            cost.push(here.iter().map(|&(c, _)| s.cands[c].cost).collect());
            back.push(vec![0; here.len()]);
            continue;
        }
        let prev = &slices[k - 1];
        let dt = s.time - prev.time;
        let mut row = Vec::with_capacity(here.len());
        let mut from = Vec::with_capacity(here.len());
        for &(c, p) in here {
            let cand = &s.cands[c];
            let mut best = (f64::INFINITY, 0);
            for (j, &(pc, pp)) in states[k - 1].iter().enumerate() {
                let base = cost[k - 1][j];
                if base >= best.0 {
                    continue;
                }
                let t = moving(items, prev, &prev.cands[pc], pp, s, cand, p, dt);
                if base + t < best.0 {
                    best = (base + t, j);
                }
            }
            row.push(best.0 + cand.cost);
            from.push(best.1);
        }
        cost.push(row);
        back.push(from);
    }
    // Back from the cheapest end.
    let mut path = vec![(0, None); slices.len()];
    let Some(last) = cost.last() else {
        return path;
    };
    let mut j = last
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(j, _)| j);
    for k in (0..slices.len()).rev() {
        path[k] = states[k].get(j).copied().unwrap_or((0, None));
        j = back[k].get(j).copied().unwrap_or(0);
    }
    path
}

/// What going from candidate `a` at position `pa` to `b` at `pb` costs,
/// `dt` whole notes later.
#[allow(clippy::too_many_arguments)]
fn moving(
    items: &[Item],
    sa: &Slice,
    a: &Cand,
    pa: Option<u8>,
    sb: &Slice,
    b: &Cand,
    pb: Option<u8>,
    dt: f64,
) -> f64 {
    let mut cost = 0.0;
    // Notes still ringing stay where they were: moving one would cut it
    // short, which only a slice with no other way does.
    for (m, &i) in sb.members.iter().enumerate().skip(sb.struck) {
        if let Some(am) = sa.members.iter().position(|&x| x == i)
            && a.places.get(am) != b.places.get(m)
        {
            cost += 1000.0;
        }
    }
    // The hand's shift: cheap within the overlapping blocks of two or
    // three frets, dearer beyond, and dearer the less time there is.
    if let (Some(x), Some(y)) = (pa, pb) {
        let d = x.abs_diff(y);
        let shift = match d {
            0 => 0.0,
            1 => 0.3,
            2 => 0.6,
            3 => 1.0,
            _ => 1.0 + 0.8 * f64::from(d - 3),
        };
        let haste = (0.125 / dt.max(1e-3)).min(4.0);
        cost += shift * (1.0 + haste);
    }
    // String skipping between single notes.
    if sa.struck == 1 && sb.struck == 1 {
        let (Some(x), Some(y)) = (a.places.first(), b.places.first()) else {
            return cost;
        };
        let skip = x.0.abs_diff(y.0);
        if skip > 1 {
            cost += 0.5 * f64::from(skip - 1);
        }
    }
    // Hammers, pulls and slides stay on their string.
    for (m, &i) in sa.members.iter().enumerate().take(sa.struck) {
        let Some(to) = items[i].legato_to else {
            continue;
        };
        if let Some(bm) = sb.members[..sb.struck].iter().position(|&x| x == to)
            && a.places.get(m).map(|p| p.0) != b.places.get(bm).map(|p| p.0)
        {
            cost += 40.0;
        }
    }
    cost
}

/// Writes the new places into the staff: free notes where the path put
/// them, tied and doubled notes where the note they follow is, and the
/// texts over hard beats.
fn write(
    staff: &mut Staff,
    items: &[Item],
    place: &[Option<(u8, u8)>],
    hard_at: &[(At, &'static str)],
    marks: bool,
    free: &dyn Fn(At) -> bool,
) -> Outcome {
    let mut out = Outcome::default();
    let mut new: Vec<Option<(u8, u8)>> = (0..items.len())
        .map(|i| {
            let it = &items[i];
            if it.follows.is_some() {
                return None;
            }
            match place.get(i).copied().flatten() {
                Some(p) if it.free && p != UNPLACED => Some(p),
                _ => it.old,
            }
        })
        .collect();
    for i in 0..items.len() {
        if items[i].follows.is_some() {
            new[i] = new[root_of(items, i)];
        }
    }
    let mut beats_touched: Vec<(usize, usize, usize)> = Vec::new();
    for it in items.iter().filter(|it| it.untie) {
        let at = it.at;
        if let Some(n) = staff
            .bars
            .get_mut(at.bar)
            .and_then(|b| b.voices.get_mut(at.voice))
            .and_then(|v| v.beats.get_mut(at.beat))
            .and_then(|b| b.notes.get_mut(at.note))
        {
            n.tie = false;
            out.changed = true;
        }
    }
    for (it, to) in items.iter().zip(&new) {
        let Some((s, f)) = *to else { continue };
        if it.old == Some((s, f)) {
            continue;
        }
        let at = it.at;
        let Some(n) = staff
            .bars
            .get_mut(at.bar)
            .and_then(|b| b.voices.get_mut(at.voice))
            .and_then(|v| v.beats.get_mut(at.beat))
            .and_then(|b| b.notes.get_mut(at.note))
        else {
            continue;
        };
        if let Some(o) = it.old {
            let delta = i16::from(f) - i16::from(o.1);
            let moved = |x: u8| u8::try_from(i16::from(x) + delta).unwrap_or(x);
            if let Some(t) = &mut n.effects.trill {
                t.fret = moved(t.fret);
            }
            if let Some(Harmonic::Tapped { fret }) = &mut n.effects.harmonic {
                *fret = moved(*fret);
            }
        } else {
            out.placed += 1;
        }
        n.place = NotePlace::Fretted { string: s, fret: f };
        out.changed = true;
        beats_touched.push((at.bar, at.voice, at.beat));
    }
    if marks {
        // A text of an earlier run goes; a new one goes on a beat without
        // text of its own.
        let wanted: BTreeMap<(usize, usize, usize), &str> = hard_at
            .iter()
            .map(|(at, t)| ((at.bar, at.voice, at.beat), *t))
            .collect();
        for (bar, b) in staff.bars.iter_mut().enumerate() {
            for (voice, v) in b.voices.iter_mut().enumerate() {
                for (beat, bt) in v.beats.iter_mut().enumerate() {
                    let at = At {
                        bar,
                        voice,
                        beat,
                        note: 0,
                    };
                    if !free(at) {
                        continue;
                    }
                    let ours = HARD_TEXTS.contains(&bt.text.as_str());
                    let to = match wanted.get(&(bar, voice, beat)) {
                        Some(t) if ours || bt.text.is_empty() => *t,
                        _ if ours => "",
                        _ => continue,
                    };
                    if bt.text != to {
                        bt.text = to.to_owned();
                        out.changed = true;
                    }
                }
            }
        }
    }
    beats_touched.sort_unstable();
    beats_touched.dedup();
    for (bar, voice, beat) in beats_touched {
        if let Some(bt) = staff
            .bars
            .get_mut(bar)
            .and_then(|b| b.voices.get_mut(voice))
            .and_then(|v| v.beats.get_mut(beat))
        {
            bt.notes.sort_by_key(|n| n.string().unwrap_or(u8::MAX));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, Scope, apply as run, caret::tests::score};
    use oxiguitar_model::{
        Bar, Beat, BeatKind, Duration, MasterBar, Note, NoteValue, Pitch, Track, Voice,
    };

    /// A beat of these pitches, still without strings.
    fn pitched(pitches: &[u8], value: NoteValue) -> Beat {
        Beat {
            kind: if pitches.is_empty() {
                BeatKind::Rest
            } else {
                BeatKind::Notes
            },
            duration: Duration::new(value),
            notes: pitches
                .iter()
                .map(|&p| Note {
                    place: NotePlace::Pitched(Pitch(p)),
                    ..Note::fretted(1, 0)
                })
                .collect(),
            ..Beat::default()
        }
    }

    /// One track on `tuning` (highest first) with `capo`, a bar of 4/4 per
    /// list of beats, in voice 1.
    fn song(tuning: &[u8], capo: u8, bars: Vec<Vec<Beat>>) -> Score {
        let n = bars.len();
        let mut s = score(1, n);
        s.master_bars = vec![MasterBar::default(); n];
        let staff = &mut s.tracks[0].staves[0];
        staff.tuning = tuning.iter().map(|&p| Pitch(p)).collect();
        staff.capo = capo;
        staff.bars = bars
            .into_iter()
            .map(|beats| Bar {
                voices: vec![Voice { beats }],
                ..Bar::default()
            })
            .collect();
        s
    }

    const GUITAR: [u8; 6] = [64, 59, 55, 50, 45, 40];

    fn places(s: &Score) -> Vec<Vec<(u8, u8)>> {
        s.tracks[0].staves[0]
            .bars
            .iter()
            .flat_map(|b| &b.voices[0].beats)
            .map(|b| {
                b.notes
                    .iter()
                    .filter_map(|n| match n.place {
                        NotePlace::Fretted { string, fret } => Some((string, fret)),
                        NotePlace::Pitched(_) => None,
                    })
                    .collect()
            })
            .collect()
    }

    /// Every note sounds the pitch it had.
    fn sounds(s: &Score, voice: usize) -> Vec<Vec<u8>> {
        let staff = &s.tracks[0].staves[0];
        staff
            .bars
            .iter()
            .flat_map(|b| &b.voices[voice].beats)
            .map(|b| {
                let mut v: Vec<u8> = b
                    .notes
                    .iter()
                    .map(|n| match n.place {
                        NotePlace::Fretted { string, fret } => {
                            staff.pitch_of(string, fret).unwrap().0
                        }
                        NotePlace::Pitched(p) => p.0,
                    })
                    .collect();
                v.sort_unstable();
                v
            })
            .collect()
    }

    fn finger_track(s: &mut Score) -> bool {
        let mut h = History::new();
        let c = Caret::at_start(s, 0).unwrap();
        run(s, &mut h, c, None, Command::Fingering(Scope::Track)).is_some()
    }

    fn texts(s: &Score) -> Vec<String> {
        s.tracks[0].staves[0]
            .bars
            .iter()
            .flat_map(|b| &b.voices[0].beats)
            .map(|b| b.text.clone())
            .collect()
    }

    #[test]
    fn open_e_chord_takes_its_open_shape() {
        let mut s = song(
            &GUITAR,
            0,
            vec![vec![pitched(&[40, 47, 52, 56, 59, 64], NoteValue::Whole)]],
        );
        assert_eq!(finger_pitched(&mut s), 6);
        assert_eq!(
            places(&s)[0],
            vec![(1, 0), (2, 0), (3, 1), (4, 2), (5, 2), (6, 0)]
        );
    }

    #[test]
    fn barre_chord_is_one_index_and_three_fingers() {
        let mut s = song(
            &GUITAR,
            0,
            vec![vec![pitched(&[41, 48, 53, 57, 60, 65], NoteValue::Whole)]],
        );
        assert!(finger_track(&mut s));
        assert_eq!(
            places(&s)[0],
            vec![(1, 1), (2, 1), (3, 2), (4, 3), (5, 3), (6, 1)]
        );
        assert_eq!(texts(&s), vec![String::new()]);
    }

    #[test]
    fn a_scale_stays_in_one_position_without_skipping_strings() {
        let scale = [60u8, 62, 64, 65, 67, 69, 71, 72];
        let bars = scale
            .chunks(8)
            .map(|c| {
                c.iter()
                    .map(|&p| pitched(&[p], NoteValue::Eighth))
                    .collect()
            })
            .collect();
        let mut s = song(&GUITAR, 0, bars);
        finger_pitched(&mut s);
        let p: Vec<(u8, u8)> = places(&s).into_iter().flatten().collect();
        assert_eq!(p.len(), 8);
        let fretted: Vec<u8> = p.iter().map(|x| x.1).filter(|f| *f > 0).collect();
        let (lo, hi) = (fretted.iter().min().unwrap(), fretted.iter().max().unwrap());
        assert!(hi - lo <= 3, "one box: {p:?}");
        assert!(p.windows(2).all(|w| w[0].0.abs_diff(w[1].0) <= 1), "{p:?}");
        assert_eq!(sounds(&s, 0).concat(), scale);
    }

    #[test]
    fn a_stretch_no_hand_makes_is_marked_and_cleared_when_gone() {
        // F2 is only on the sixth string; B4 is seven frets from it at best.
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[41, 71], NoteValue::Whole)]]);
        assert!(finger_track(&mut s));
        assert_eq!(texts(&s), vec![HARD_STRETCH.to_owned()]);
        // Running again changes nothing.
        assert!(!finger_track(&mut s));
        // Once the chord is playable, the text goes.
        let staff = &mut s.tracks[0].staves[0];
        staff.bars[0].voices[0].beats[0].notes.truncate(1);
        assert!(finger_track(&mut s));
        assert_eq!(texts(&s), vec![String::new()]);
    }

    #[test]
    fn a_text_of_the_users_own_is_kept() {
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[41, 71], NoteValue::Whole)]]);
        s.tracks[0].staves[0].bars[0].voices[0].beats[0].text = "Solo".into();
        finger_track(&mut s);
        assert_eq!(texts(&s), vec!["Solo".to_owned()]);
    }

    #[test]
    fn drop_d_power_chord_on_open_strings() {
        let tuning = [64, 59, 55, 50, 45, 38];
        let mut s = song(
            &tuning,
            0,
            vec![vec![pitched(&[38, 45, 50], NoteValue::Whole)]],
        );
        finger_pitched(&mut s);
        assert_eq!(places(&s)[0], vec![(4, 0), (5, 0), (6, 0)]);
    }

    #[test]
    fn frets_count_from_the_capo() {
        let mut s = song(
            &GUITAR,
            2,
            vec![vec![pitched(&[42, 49, 54, 58, 61, 66], NoteValue::Whole)]],
        );
        finger_pitched(&mut s);
        // E shape behind a capo on 2: the same frets as open E.
        assert_eq!(
            places(&s)[0],
            vec![(1, 0), (2, 0), (3, 1), (4, 2), (5, 2), (6, 0)]
        );
        assert_eq!(sounds(&s, 0)[0], vec![42, 49, 54, 58, 61, 66]);
    }

    #[test]
    fn a_bass_line_and_a_seven_string_low_b() {
        let bass = [43, 38, 33, 28];
        let line = [28u8, 31, 33, 35, 36, 38, 40, 43];
        let bars = vec![
            line.iter()
                .map(|&p| pitched(&[p], NoteValue::Eighth))
                .collect(),
        ];
        let mut s = song(&bass, 0, bars);
        assert_eq!(finger_pitched(&mut s), 8);
        assert_eq!(sounds(&s, 0).concat(), line);
        let seven = [64, 59, 55, 50, 45, 40, 35];
        let mut s = song(&seven, 0, vec![vec![pitched(&[35], NoteValue::Whole)]]);
        finger_pitched(&mut s);
        assert_eq!(places(&s)[0], vec![(7, 0)]);
    }

    #[test]
    fn a_pitch_no_string_plays_stays_a_pitch() {
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[20, 64], NoteValue::Whole)]]);
        assert_eq!(finger_pitched(&mut s), 1);
        let notes = &s.tracks[0].staves[0].bars[0].voices[0].beats[0].notes;
        assert!(
            notes
                .iter()
                .any(|n| n.place == NotePlace::Pitched(Pitch(20)))
        );
    }

    #[test]
    fn a_chord_diagram_picks_its_shape() {
        // A5: open A and E on the fourth string, or fifth fret of the
        // sixth: the diagram says which.
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[45, 52], NoteValue::Whole)]]);
        s.tracks[0].chords.push(ChordDiagram {
            name: "A5".into(),
            first_fret: 5,
            frets: vec![None, None, None, None, Some(7), Some(5)],
        });
        s.tracks[0].staves[0].bars[0].voices[0].beats[0].chord = Some(0);
        finger_pitched(&mut s);
        assert_eq!(places(&s)[0], vec![(5, 7), (6, 5)]);
    }

    #[test]
    fn natural_harmonics_stay_natural_harmonics_of_the_same_note() {
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[], NoteValue::Whole)]]);
        let beat = &mut s.tracks[0].staves[0].bars[0].voices[0].beats[0];
        beat.kind = BeatKind::Notes;
        let mut n = Note::fretted(2, 5);
        n.effects.harmonic = Some(Harmonic::Natural);
        beat.notes.push(n);
        finger_track(&mut s);
        let staff = &s.tracks[0].staves[0];
        let n = &staff.bars[0].voices[0].beats[0].notes[0];
        assert_eq!(n.effects.harmonic, Some(Harmonic::Natural));
        let NotePlace::Fretted { string, fret } = n.place else {
            panic!("on a string")
        };
        let open = staff.pitch_of(string, 0).unwrap().0;
        assert_eq!(open + natural(fret).unwrap(), 59 + 24);
    }

    #[test]
    fn ties_keep_their_string_and_legato_stays_on_one() {
        // A hammer from fret 5 to 7 of the B string, then E5 tied over.
        let q = NoteValue::Quarter;
        let mut beats = vec![
            pitched(&[64], q),
            pitched(&[66], q),
            pitched(&[69], q),
            pitched(&[69], q),
        ];
        beats[1].notes[0].effects.hammer_pull = true;
        beats[3].notes[0].tie = true;
        let mut s = song(&GUITAR, 0, vec![beats]);
        // Their strings first, as typed: E on the B string, then the hammer.
        for (b, (st, f)) in [(2u8, 5u8), (2, 7), (2, 10), (2, 10)]
            .into_iter()
            .enumerate()
        {
            s.tracks[0].staves[0].bars[0].voices[0].beats[b].notes[0].place = NotePlace::Fretted {
                string: st,
                fret: f,
            };
        }
        finger_track(&mut s);
        let p = places(&s);
        assert_eq!(p[1][0].0, p[2][0].0, "hammer on one string: {p:?}");
        assert_eq!(p[2], p[3], "tie: {p:?}");
    }

    #[test]
    fn a_tie_across_a_bar_line_keeps_the_note_it_continues() {
        let q = NoteValue::Quarter;
        let mut a = vec![pitched(&[76, 71], q), pitched(&[83], q)];
        let mut b = vec![pitched(&[83], q)];
        b[0].notes[0].tie = true;
        a[0].notes[0].place = NotePlace::Fretted {
            string: 1,
            fret: 12,
        };
        a[0].notes[1].place = NotePlace::Fretted {
            string: 2,
            fret: 12,
        };
        a[1].notes[0].place = NotePlace::Fretted {
            string: 2,
            fret: 24,
        };
        b[0].notes[0].place = NotePlace::Fretted {
            string: 2,
            fret: 24,
        };
        let mut s = song(&GUITAR, 0, vec![a, b]);
        finger_track(&mut s);
        let p = places(&s);
        assert_eq!(p[1], p[2], "{p:?}");
        assert_eq!(sounds(&s, 0)[2], vec![83]);
    }

    #[test]
    fn a_note_two_voices_strike_rings_as_long_as_the_longer() {
        // Voice 1: A3 a quarter, then a run. Voice 2: the same A3, tied
        // over the bar: its string stays the A3's through the run.
        let (q, w) = (NoteValue::Quarter, NoteValue::Whole);
        let mut s = song(
            &GUITAR,
            0,
            vec![
                vec![
                    pitched(&[57], q),
                    pitched(&[59], q),
                    pitched(&[60], q),
                    pitched(&[62], q),
                ],
                vec![pitched(&[], w)],
            ],
        );
        let mut held = vec![pitched(&[57], w)];
        let mut tied = vec![pitched(&[57], w)];
        tied[0].notes[0].tie = true;
        s.tracks[0].staves[0].bars[0].voices.push(Voice {
            beats: std::mem::take(&mut held),
        });
        s.tracks[0].staves[0].bars[1]
            .voices
            .push(Voice { beats: tied });
        finger_pitched(&mut s);
        let staff = &s.tracks[0].staves[0];
        let a = staff.bars[0].voices[1].beats[0].notes[0].string();
        for b in &staff.bars[0].voices[0].beats[1..] {
            assert_ne!(b.notes[0].string(), a, "the run keeps off the ringing A");
        }
        assert_eq!(staff.bars[1].voices[1].beats[0].notes[0].string(), a);
    }

    #[test]
    fn a_tie_that_continues_nothing_is_struck() {
        // The tie in bar 2 follows a rest: nothing rings into it.
        let q = NoteValue::Quarter;
        let mut b = vec![pitched(&[57], q)];
        b[0].notes[0].tie = true;
        let mut s = song(
            &GUITAR,
            0,
            vec![vec![pitched(&[57], q), pitched(&[], q)], b],
        );
        finger_pitched(&mut s);
        let n = &s.tracks[0].staves[0].bars[1].voices[0].beats[0].notes[0];
        assert!(!n.tie);
        assert!(n.string().is_some());
    }

    #[test]
    fn frets_past_the_neck_keep_their_pitch() {
        // A chord written past fret 24 (some files have them), then tied,
        // with an open E before it.
        let frets = [24u8, 23, 24, 26, 25, 27];
        let q = NoteValue::Quarter;
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[], q), pitched(&[], q)]]);
        for b in 0..2 {
            let beat = &mut s.tracks[0].staves[0].bars[0].voices[0].beats[b];
            beat.kind = BeatKind::Notes;
            for (i, &f) in frets.iter().enumerate() {
                let mut n = Note::fretted(i as u8 + 1, f);
                n.tie = b == 1;
                beat.notes.push(n);
            }
        }
        let before = sounds(&s, 0);
        finger_track(&mut s);
        assert_eq!(sounds(&s, 0), before, "{:?}", places(&s));
        let p = places(&s);
        assert_eq!(p[0], p[1]);
    }

    #[test]
    fn bends_and_vibrato_stay_off_open_strings() {
        let q = NoteValue::Quarter;
        let mut beats = vec![pitched(&[64], q), pitched(&[64], q), pitched(&[64], q)];
        beats[0].notes[0].effects.bend = Some(oxiguitar_model::Bend::default());
        beats[1].notes[0].effects.vibrato = true;
        let mut s = song(&GUITAR, 0, vec![beats]);
        finger_pitched(&mut s);
        let p = places(&s);
        assert!(p[0][0].1 > 0 && p[1][0].1 > 0, "{p:?}");
        assert_eq!(sounds(&s, 0).concat(), vec![64, 64, 64]);
    }

    #[test]
    fn a_hammer_to_a_note_beats_away_keeps_both_strings() {
        // An open E hammered to the next note on its string, two beats on;
        // another string's note between.
        let q = NoteValue::Quarter;
        let mut s = song(
            &GUITAR,
            0,
            vec![vec![
                pitched(&[64], q),
                pitched(&[60], q),
                pitched(&[65], q),
            ]],
        );
        let beats = &mut s.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].notes[0].place = NotePlace::Fretted { string: 1, fret: 0 };
        beats[0].notes[0].effects.hammer_pull = true;
        beats[1].notes[0].place = NotePlace::Fretted { string: 2, fret: 1 };
        beats[2].notes[0].place = NotePlace::Fretted { string: 1, fret: 1 };
        finger_track(&mut s);
        let p = places(&s);
        assert_eq!((p[0][0], p[2][0]), ((1, 0), (1, 1)), "{p:?}");
    }

    #[test]
    fn a_hammer_to_nothing_keeps_its_place() {
        let q = NoteValue::Quarter;
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[64], q), pitched(&[60], q)]]);
        let beats = &mut s.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].notes[0].place = NotePlace::Fretted { string: 1, fret: 0 };
        beats[0].notes[0].effects.hammer_pull = true;
        beats[1].notes[0].place = NotePlace::Fretted { string: 2, fret: 1 };
        finger_track(&mut s);
        assert_eq!(places(&s)[0][0], (1, 0));
    }

    #[test]
    fn a_ringing_bass_keeps_its_string_from_the_melody() {
        let (e, h) = (NoteValue::Eighth, NoteValue::Whole);
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[45], h)]]);
        let melody: Vec<Beat> = [57u8, 60, 64, 60, 57, 60, 64, 60]
            .iter()
            .map(|&p| pitched(&[p], e))
            .collect();
        s.tracks[0].staves[0].bars[0]
            .voices
            .insert(0, Voice { beats: melody });
        finger_pitched(&mut s);
        let staff = &s.tracks[0].staves[0];
        let bass = staff.bars[0].voices[1].beats[0].notes[0].string().unwrap();
        for b in &staff.bars[0].voices[0].beats {
            assert_ne!(b.notes[0].string(), Some(bass));
        }
        assert_eq!(sounds(&s, 1)[0], vec![45]);
    }

    #[test]
    fn the_selection_alone_moves() {
        let mut s = song(
            &GUITAR,
            0,
            vec![
                vec![pitched(&[64], NoteValue::Whole)],
                vec![pitched(&[64], NoteValue::Whole)],
            ],
        );
        // High on the A string, then fifth fret of the B string.
        for (b, (string, fret)) in [(5, 19), (2, 5)].into_iter().enumerate() {
            s.tracks[0].staves[0].bars[b].voices[0].beats[0].notes[0].place =
                NotePlace::Fretted { string, fret };
        }
        let mut h = History::new();
        let c = Caret::at_start(&s, 0).unwrap();
        run(
            &mut s,
            &mut h,
            c,
            None,
            Command::Fingering(Scope::Selection),
        )
        .unwrap();
        let p = places(&s);
        assert_ne!(p[0], vec![(5, 19)], "it joins the bar after");
        assert_eq!(p[1], vec![(2, 5)]);
        assert_eq!(h.undo_label(), Some("Find Fingering"));
    }

    #[test]
    fn drums_and_strings_without_notes_change_nothing() {
        let mut s = song(&GUITAR, 0, vec![vec![pitched(&[], NoteValue::Whole)]]);
        assert!(!finger_track(&mut s));
        let mut d = song(&GUITAR, 0, vec![vec![pitched(&[40], NoteValue::Whole)]]);
        d.tracks[0].instrument.percussion = true;
        assert_eq!(finger_pitched(&mut d), 0);
        let _ = Track::default();
    }

    #[test]
    fn a_long_track_is_quick() {
        // 300 bars of sixteenths and chords, as a dense tab has.
        let run: [u8; 8] = [52, 55, 57, 59, 62, 64, 67, 69];
        let bars: Vec<Vec<Beat>> = (0..300)
            .map(|b| {
                let mut v: Vec<Beat> = (0..12)
                    .map(|i| pitched(&[run[(i + b) % 8]], NoteValue::Sixteenth))
                    .collect();
                v.push(pitched(&[40, 47, 52, 55], NoteValue::Quarter));
                v
            })
            .collect();
        let mut s = song(&GUITAR, 0, bars);
        let t = std::time::Instant::now();
        assert_eq!(finger_pitched(&mut s), 300 * 16);
        let limit = if cfg!(debug_assertions) { 30.0 } else { 2.0 };
        assert!(t.elapsed().as_secs_f64() < limit, "{:?}", t.elapsed());
    }
}
