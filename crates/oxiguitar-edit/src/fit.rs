// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Fitting bars to the time signature they now have (#222,
//! `docs/design/fit-bars.md`).
//!
//! The user changes a song's time signature and the bars turn red. Two
//! ways to fit them (Mateus, 8 Nov 2026: both, picked in a dialog):
//!
//! - **Re-bar** ([`FitHow::Rebar`]): the music flows across the bar lines.
//!   Every note keeps its length and its moment; notes that now cross a
//!   bar line become notes tied across it, and bars are added or removed
//!   in every track. The time line is cut where the structure needs a bar
//!   line (repeats, endings, directions, double bars, sections, changes of
//!   time or key signature); a passage that doesn't fill its last bar
//!   leaves it short, and the bar check marks it (Mateus, 8 Nov 2026).
//! - **Stretch** ([`FitHow::Stretch`]): each red bar keeps its notes,
//!   scaled to fill its time signature, with tuplets where no plain value
//!   fits (3/4 to 4/4: three quarters become a half-note triplet). The
//!   tempo stays as it is (Mateus, 8 Nov 2026).
//!
//! Only passages with a bar that doesn't add up change; a pickup (a first
//! bar shorter than its time signature and than the bar after it) stays
//! as it is. The whole fit is one change for Undo.

use oxiguitar_model::{
    Bar, Beat, BeatKind, Direction, Duration, MasterBar, NoteValue, Ratio, Score, Slide,
    TempoChange, Tuplet, Voice,
};

use crate::{Caret, Command, Edited, History, Region, Selection, StaffBar};

/// How bars are fitted to their time signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitHow {
    /// The music flows across the bar lines: notes keep their lengths,
    /// bars are added or removed.
    Rebar,
    /// Each bar keeps its notes, scaled to fill its time signature.
    Stretch,
}

/// What Fit Bars to Time Signature does, and to what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fit {
    /// Re-bar or stretch.
    pub how: FitHow,
    /// The whole song (Tools), rather than the selected bars or the
    /// caret's bar (the page's right-click menu). Always every track.
    pub whole_song: bool,
}

/// Runs [`Command::FitBars`]; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let Command::FitBars(fit) = command else {
        return None;
    };
    let count = score.bar_count();
    if count == 0 {
        return Some(None);
    }
    let (first, last) = if fit.whole_song {
        (0, count - 1)
    } else {
        selection.map_or((c.bar, c.bar), |s| s.bars())
    };
    if first > last || last >= count {
        return Some(None);
    }
    let Some(work) = fitted(score, first, last, fit.how) else {
        return Some(None);
    };
    let touched = history.record(score, command.label(), vec![Region::Whole], |s| *s = work);
    Some(Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    }))
}

/// Bars `first..=last` of `score` fitted to their time signatures, or
/// `None` when nothing needs fitting.
pub(crate) fn fitted(score: &Score, first: usize, last: usize, how: FitHow) -> Option<Score> {
    match how {
        FitHow::Rebar => rebar(score, first, last),
        FitHow::Stretch => stretch(score, first, last),
    }
}

// ---------------------------------------------------------------------------
// What the bars hold

/// Whether a voice holds a note or a rest (not only placeholders).
fn has_music(v: &Voice) -> bool {
    v.beats
        .iter()
        .any(|b| matches!(b.kind, BeatKind::Notes | BeatKind::Rest))
}

/// Every staff's bar `k`.
fn staff_bars(score: &Score, k: usize) -> impl Iterator<Item = &Bar> + '_ {
    score
        .tracks
        .iter()
        .flat_map(|t| &t.staves)
        .filter_map(move |s| s.bars.get(k))
}

/// The length of bar `k`'s longest voice that holds music, in every
/// staff; zero for a bar with no music; `None` if a duration can't be
/// measured.
fn content(score: &Score, k: usize) -> Option<Ratio> {
    let mut longest = Ratio::ZERO;
    for bar in staff_bars(score, k) {
        for v in &bar.voices {
            let len = v.length()?;
            if has_music(v) && len > longest {
                longest = len;
            }
        }
    }
    Some(longest)
}

/// Whether bar `k` adds up in every staff.
fn adds_up_everywhere(score: &Score, k: usize) -> bool {
    score.tracks.iter().enumerate().all(|(track, t)| {
        (0..t.staves.len()).all(|staff| {
            crate::adds_up(
                score,
                StaffBar {
                    track,
                    staff,
                    bar: k,
                },
            )
        })
    })
}

/// Whether bar `k` is the song's pickup: the first bar, shorter than its
/// time signature and than the bar after it.
fn is_pickup(score: &Score, k: usize) -> bool {
    if k != 0 {
        return false;
    }
    let Some(want) = score.master_bars.first().and_then(|m| m.expected_length()) else {
        return false;
    };
    match (content(score, 0), content(score, 1)) {
        (Some(here), Some(next)) => here < want && here < next,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Writing a length as note values

/// Which tuplets a length is written in first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefer {
    /// The note's own tuplets (a piece of a split note stays in its
    /// tuplet).
    Own,
    /// No tuplet, if a plain value fits (a stretched triplet that became a
    /// sixteenth leaves its tuplet).
    Plain,
}

/// The shortest note value the model has.
fn shortest() -> Ratio {
    NoteValue::TwoFiftySixth.length()
}

const VALUES: [NoteValue; 10] = [
    NoteValue::DoubleWhole,
    NoteValue::Whole,
    NoteValue::Half,
    NoteValue::Quarter,
    NoteValue::Eighth,
    NoteValue::Sixteenth,
    NoteValue::ThirtySecond,
    NoteValue::SixtyFourth,
    NoteValue::OneTwentyEighth,
    NoteValue::TwoFiftySixth,
];

/// A dotted value's length.
fn dotted(value: NoteValue, dots: u8) -> Option<Ratio> {
    Duration {
        value,
        dots,
        tuplets: Vec::new(),
    }
    .length()
}

/// Whether `q` is a whole number of some power-of-two fraction.
fn dyadic(q: Ratio) -> bool {
    let d = q.denom();
    d > 0 && d & (d - 1) == 0
}

/// `q` as one value with up to two dots.
fn single(q: Ratio) -> Option<(NoteValue, u8)> {
    for dots in 0..=2 {
        for value in VALUES {
            if dotted(value, dots) == Some(q) {
                return Some((value, dots));
            }
        }
    }
    None
}

/// `q` as the fewest tied values (with up to one dot), longest first.
fn tied(q: Ratio) -> Option<Vec<(NoteValue, u8)>> {
    if !dyadic(q) {
        return None;
    }
    let mut choices: Vec<(Ratio, NoteValue, u8)> = VALUES
        .iter()
        .flat_map(|&v| [(v, 0), (v, 1)])
        .filter_map(|(v, d)| Some((dotted(v, d)?, v, d)))
        .collect();
    choices.sort_by_key(|c| std::cmp::Reverse(c.0));
    let mut rem = q;
    let mut out = Vec::new();
    while rem > Ratio::ZERO {
        let &(len, v, d) = choices.iter().find(|c| c.0 <= rem)?;
        out.push((v, d));
        rem = rem.checked_sub(len)?;
        if out.len() > 16 {
            return None;
        }
    }
    Some(out)
}

/// The factor a stack of tuplets scales by.
fn factor(stack: &[Tuplet]) -> Option<Ratio> {
    stack
        .iter()
        .try_fold(Ratio::ONE, |acc, t| acc.checked_mul(t.factor()?))
}

/// The tuplet that makes `q` plain: as many notes as the odd part of its
/// denominator, in the time of the power of two below it (3:2, 5:4, 7:4,
/// 9:8 …). `None` if `q` needs none, or no tuplet the model holds does.
fn tuplet_for(q: Ratio) -> Option<Tuplet> {
    let d = q.denom();
    if d <= 0 {
        return None;
    }
    let odd = d >> d.trailing_zeros();
    if odd == 1 {
        return None;
    }
    let notes = u8::try_from(odd).ok()?;
    let mut in_time_of = 1u8;
    while in_time_of.checked_mul(2)? < notes {
        in_time_of *= 2;
    }
    Some(Tuplet { notes, in_time_of })
}

/// `len` written as tied durations: one value if any choice of tuplets
/// gives one, else the fewest tied values. Tuplets tried: the note's own
/// and none (in `prefer`'s order), then those with one more tuplet inside,
/// then one tuplet alone.
fn spell(len: Ratio, own: &[Tuplet], prefer: Prefer) -> Option<Vec<Duration>> {
    if len < shortest() {
        return None;
    }
    let mut stacks: Vec<Vec<Tuplet>> = match prefer {
        Prefer::Own => vec![own.to_vec(), Vec::new()],
        Prefer::Plain => vec![Vec::new(), own.to_vec()],
    };
    for base in [own.to_vec(), Vec::new()] {
        let q = len.checked_div(factor(&base)?)?;
        if let Some(t) = tuplet_for(q) {
            let mut s = base;
            s.push(t);
            stacks.push(s);
        }
    }
    stacks.dedup();
    let written = |s: &Vec<Tuplet>| len.checked_div(factor(s)?);
    for s in &stacks {
        if let Some((value, dots)) = written(s).and_then(single) {
            return Some(vec![Duration {
                value,
                dots,
                tuplets: s.clone(),
            }]);
        }
    }
    for s in &stacks {
        if let Some(parts) = written(s).and_then(tied) {
            return Some(
                parts
                    .into_iter()
                    .map(|(value, dots)| Duration {
                        value,
                        dots,
                        tuplets: s.clone(),
                    })
                    .collect(),
            );
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Splitting a beat

/// A piece of a beat that was split: `first` keeps what is struck, `last`
/// what leads on to the next note; the pieces after the first are tied.
fn piece(beat: &Beat, duration: Duration, first: bool, last: bool) -> Beat {
    let mut b = beat.clone();
    b.duration = duration;
    if !first {
        b.chord = None;
        b.text.clear();
        b.grace = None;
        let e = &mut b.effects;
        e.stroke = None;
        e.pick_stroke = None;
        e.technique = None;
        e.fade_in = false;
        e.dynamic = None;
        e.whammy = None;
        for n in &mut b.notes {
            n.tie = true;
            let e = &mut n.effects;
            e.accent = None;
            e.ghost = false;
            e.bend = None;
            e.left_finger = None;
            e.right_finger = None;
            e.left_hand_tap = false;
            e.slides
                .retain(|s| !matches!(s, Slide::InFromBelow | Slide::InFromAbove));
        }
    }
    if !last {
        for n in &mut b.notes {
            let e = &mut n.effects;
            e.hammer_pull = false;
            e.staccato = false;
            e.slides
                .retain(|s| matches!(s, Slide::InFromBelow | Slide::InFromAbove));
        }
    }
    b
}

/// `beat` written as `len`, split into tied pieces where one value can't
/// show it; `first` and `last` say whether the result starts and ends the
/// original beat.
fn pieces(beat: &Beat, len: Ratio, prefer: Prefer, first: bool, last: bool) -> Option<Vec<Beat>> {
    let durations = spell(len, &beat.duration.tuplets, prefer)?;
    let n = durations.len();
    Some(
        durations
            .into_iter()
            .enumerate()
            .map(|(i, d)| piece(beat, d, first && i == 0, last && i + 1 == n))
            .collect(),
    )
}

/// Rests filling `len`.
fn rests(len: Ratio) -> Option<Vec<Beat>> {
    let rest = Beat {
        kind: BeatKind::Rest,
        notes: Vec::new(),
        ..Beat::default()
    };
    pieces(&rest, len, Prefer::Plain, true, true)
}

// ---------------------------------------------------------------------------
// Stretch

/// Every red bar in `first..=last` with its notes scaled to fill it.
fn stretch(score: &Score, first: usize, last: usize) -> Option<Score> {
    let mut out = score.clone();
    let mut changed = false;
    for k in first..=last {
        let Some(mb) = score.master_bars.get(k) else {
            continue;
        };
        if mb.free_time || is_pickup(score, k) || adds_up_everywhere(score, k) {
            continue;
        }
        let (Some(want), Some(have)) = (mb.expected_length(), content(score, k)) else {
            continue;
        };
        if have == Ratio::ZERO || have == want {
            continue;
        }
        let Some(f) = want.checked_div(have) else {
            continue;
        };
        let Some(bars) = stretched_bars(score, k, f) else {
            continue;
        };
        for (i, staff) in out
            .tracks
            .iter_mut()
            .flat_map(|t| &mut t.staves)
            .enumerate()
        {
            if let (Some(dst), Some(src)) = (staff.bars.get_mut(k), bars.get(i)) {
                *dst = src.clone();
            }
        }
        if let Some(m) = out.master_bars.get_mut(k) {
            for t in &mut m.tempo_changes {
                if let Some(p) = t.position.checked_mul(f) {
                    t.position = p;
                }
            }
        }
        changed = true;
    }
    changed.then_some(out)
}

/// Bar `k` of every staff, its beats scaled by `f`; `None` if a beat
/// can't be written.
fn stretched_bars(score: &Score, k: usize, f: Ratio) -> Option<Vec<Bar>> {
    let mut bars = Vec::new();
    for staff in score.tracks.iter().flat_map(|t| &t.staves) {
        let Some(bar) = staff.bars.get(k) else {
            bars.push(Bar::default());
            continue;
        };
        let mut bar = bar.clone();
        for v in &mut bar.voices {
            let mut beats = Vec::with_capacity(v.beats.len());
            for b in &v.beats {
                if b.grace.is_some() {
                    beats.push(b.clone());
                    continue;
                }
                let len = b.duration.length()?.checked_mul(f)?;
                beats.extend(pieces(b, len, Prefer::Plain, true, true)?);
            }
            v.beats = beats;
        }
        bars.push(bar);
    }
    Some(bars)
}

// ---------------------------------------------------------------------------
// Re-bar

/// Whether the structure needs a bar line before bar `k`.
fn cut_before(bars: &[MasterBar], k: usize) -> bool {
    let (Some(a), Some(b)) = (k.checked_sub(1).and_then(|p| bars.get(p)), bars.get(k)) else {
        return true;
    };
    b.repeat_start
        || a.repeat_end.is_some()
        || a.alternate_endings != b.alternate_endings
        || !a.directions.is_empty()
        || !b.directions.is_empty()
        || a.double_bar
        || b.marker.is_some()
        || a.free_time
        || b.free_time
        || a.time_signature != b.time_signature
        || a.key_signature != b.key_signature
}

/// The passages of `first..=last` that need re-barring: runs of bars
/// between the structure's bar lines, holding a bar that doesn't add up.
fn passages(score: &Score, first: usize, last: usize) -> Vec<(usize, usize)> {
    let bars = &score.master_bars;
    let start = if is_pickup(score, first) {
        first + 1
    } else {
        first
    };
    let mut out = Vec::new();
    let mut k = start;
    while k <= last {
        let mut end = k;
        while end < last && !cut_before(bars, end + 1) {
            end += 1;
        }
        let usable = (k..=end).all(|i| {
            bars.get(i)
                .is_some_and(|m| !m.free_time && m.expected_length().is_some())
                && content(score, i).is_some()
        });
        if usable && (k..=end).any(|i| !adds_up_everywhere(score, i)) {
            out.push((k, end));
        }
        k = end + 1;
    }
    out
}

/// A main beat at its moment on a passage's time line, with the grace
/// beats before it; a grace run that ends a voice has no beat.
struct Event {
    start: Ratio,
    len: Ratio,
    beat: Option<Beat>,
    graces: Vec<Beat>,
    /// A piece of it at a new bar line would be shorter than any note
    /// value can show (files with odd tuplets): it stays whole, in the bar
    /// holding most of it.
    whole: bool,
}

impl Event {
    fn end(&self) -> Ratio {
        self.start + self.len
    }
}

/// Voice `v` of bars `k0..=k1` of `bars`, each bar from its start in
/// `starts`.
fn events(bars: &[Bar], k0: usize, starts: &[Ratio], v: usize) -> Option<Vec<Event>> {
    let mut out = Vec::new();
    for (i, &t0) in starts.iter().enumerate() {
        let Some(voice) = bars.get(k0 + i).and_then(|b| b.voices.get(v)) else {
            continue;
        };
        if !has_music(voice) {
            continue;
        }
        let mut at = t0;
        let mut graces = Vec::new();
        for b in &voice.beats {
            if b.grace.is_some() {
                graces.push(b.clone());
                continue;
            }
            let len = b.duration.length()?;
            out.push(Event {
                start: at,
                len,
                beat: Some(b.clone()),
                graces: std::mem::take(&mut graces),
                whole: false,
            });
            at = at.checked_add(len)?;
        }
        if !graces.is_empty() {
            out.push(Event {
                start: at,
                len: Ratio::ZERO,
                beat: None,
                graces,
                whole: false,
            });
        }
    }
    Some(out)
}

/// One voice of the new bar `from..to`, from a passage's events; an empty
/// voice if none of them sounds there. `end` is the passage's end.
fn new_voice(events: &[Event], from: Ratio, to: Ratio, end: Ratio) -> Option<Voice> {
    let mut beats = Vec::new();
    let mut at = from;
    let mut music = false;
    // Rests for a gap; one no note value can show is left out (the bar
    // stays short), as in the file it came from.
    let gap = |beats: &mut Vec<Beat>, len: Ratio| {
        if let Some(r) = rests(len) {
            beats.extend(r);
        }
    };
    for ev in events {
        let zero = ev.len == Ratio::ZERO;
        let inside = if zero {
            ev.start >= from && (ev.start < to || (ev.start == to && to == end))
        } else {
            ev.start < to && ev.end() > from
        };
        if !inside {
            continue;
        }
        if ev.whole && !zero {
            let mid = ev
                .start
                .checked_add(ev.len.checked_mul(Ratio::new(1, 2)?)?)?;
            if mid < from || mid >= to {
                // Most of it is in another bar.
                at = at.max(ev.end().min(to));
                continue;
            }
            if ev.start > at {
                gap(&mut beats, ev.start.checked_sub(at)?);
            }
            beats.extend(ev.graces.iter().cloned());
            if let Some(b) = &ev.beat {
                music |= matches!(b.kind, BeatKind::Notes | BeatKind::Rest);
                beats.push(b.clone());
            }
            at = at.max(ev.end());
            continue;
        }
        let s = ev.start.max(from);
        let e = ev.end().min(to);
        if s > at {
            gap(&mut beats, s.checked_sub(at)?);
        }
        let first = s == ev.start;
        if first {
            beats.extend(ev.graces.iter().cloned());
        }
        if let Some(b) = &ev.beat {
            music |= matches!(b.kind, BeatKind::Notes | BeatKind::Rest);
            if first && e == ev.end() {
                beats.push(b.clone());
            } else {
                // Rests have no notes, so only notes are tied.
                beats.extend(pieces(
                    b,
                    e.checked_sub(s)?,
                    Prefer::Own,
                    first,
                    e == ev.end(),
                )?);
            }
        }
        at = at.max(e);
    }
    if !music {
        return Some(Voice::default());
    }
    if at < to {
        gap(&mut beats, to.checked_sub(at)?);
    }
    Some(Voice { beats })
}

/// Marks the events that a bar line would cut into a piece no note value
/// can show.
fn mark_whole(events: &mut [Event], bounds: &[(Ratio, Ratio)]) {
    for ev in events.iter_mut() {
        let Some(b) = &ev.beat else {
            continue;
        };
        let end = ev.end();
        let cut = bounds
            .iter()
            .filter(|&&(s, e)| s < end && e > ev.start)
            .map(|&(s, e)| (s.max(ev.start), e.min(end)))
            .collect::<Vec<_>>();
        if cut.len() > 1 {
            ev.whole = cut.iter().any(|&(s, e)| {
                e.checked_sub(s)
                    .and_then(|len| spell(len, &b.duration.tuplets, Prefer::Own))
                    .is_none()
            });
        }
    }
}

/// Directions that mark where something starts (and so go on the first
/// of the new bars); the others end a bar.
fn starts_bar(d: Direction) -> bool {
    matches!(
        d,
        Direction::Segno | Direction::SegnoSegno | Direction::Coda | Direction::DoubleCoda
    )
}

/// A re-barred passage: its master bars, every staff's bars (track by
/// track, staff by staff), and where each old bar's start went (new bar
/// from the passage's start, offset in it).
struct Rebarred {
    master: Vec<MasterBar>,
    staves: Vec<Vec<Bar>>,
    moved: Vec<(usize, Ratio)>,
}

/// Bars `k0..=k1` re-barred; `None` if something can't be written.
fn rebar_passage(score: &Score, k0: usize, k1: usize) -> Option<Rebarred> {
    let old = score.master_bars.get(k0..=k1)?;
    let n = old.first()?.expected_length()?;
    // The old time line: each bar as long as its music (an empty bar as
    // its time signature).
    let mut starts = Vec::new();
    let mut total = Ratio::ZERO;
    for (i, m) in old.iter().enumerate() {
        starts.push(total);
        let c = content(score, k0 + i)?;
        let len = if c == Ratio::ZERO {
            m.expected_length()?
        } else {
            c
        };
        total = total.checked_add(len)?;
    }
    // The new bars.
    let mut bounds = Vec::new();
    let mut s = Ratio::ZERO;
    while s < total {
        let e = s.checked_add(n)?.min(total);
        bounds.push((s, e));
        s = e;
        if bounds.len() > 100_000 {
            return None;
        }
    }
    let containing = |t: Ratio| -> usize {
        bounds
            .iter()
            .position(|&(s, e)| s <= t && t < e)
            .unwrap_or(bounds.len().saturating_sub(1))
    };
    let old_at = |t: Ratio| -> usize { starts.iter().rposition(|&s| s <= t).unwrap_or(0) };
    let moved: Vec<(usize, Ratio)> = starts
        .iter()
        .map(|&t| {
            let j = containing(t);
            (j, t - bounds.get(j).map_or(Ratio::ZERO, |b| b.0))
        })
        .collect();
    // Master bars.
    let (head, tail) = (old.first()?, old.last()?);
    let count = bounds.len();
    let mut master = Vec::with_capacity(count);
    for (j, &(s, e)) in bounds.iter().enumerate() {
        // The feel of the last old bar starting in this one, else of the
        // bar its start is in.
        let from = starts
            .iter()
            .rposition(|&t| t >= s && t < e)
            .unwrap_or_else(|| old_at(s));
        let base = old.get(from)?;
        let mut m = MasterBar {
            time_signature: head.time_signature,
            key_signature: head.key_signature,
            alternate_endings: head.alternate_endings,
            triplet_feel: base.triplet_feel,
            ..MasterBar::default()
        };
        if j == 0 {
            m.repeat_start = head.repeat_start;
            m.marker.clone_from(&head.marker);
            m.directions
                .extend(head.directions.iter().copied().filter(|&d| starts_bar(d)));
        }
        if j + 1 == count {
            m.repeat_end = tail.repeat_end;
            m.double_bar = tail.double_bar;
            m.directions
                .extend(tail.directions.iter().copied().filter(|&d| !starts_bar(d)));
        }
        master.push(m);
    }
    // Tempo changes keep their moment.
    for (i, m) in old.iter().enumerate() {
        for t in &m.tempo_changes {
            let at = starts.get(i)?.checked_add(t.position)?;
            let j = containing(at);
            let (s, _) = *bounds.get(j)?;
            master.get_mut(j)?.tempo_changes.push(TempoChange {
                position: at.checked_sub(s)?,
                ..t.clone()
            });
        }
    }
    // Every staff.
    let mut staves = Vec::new();
    for staff in score.tracks.iter().flat_map(|t| &t.staves) {
        let bars = staff.bars.get(k0..=k1)?;
        let voices = bars.iter().map(|b| b.voices.len()).max().unwrap_or(1);
        let mut per_voice = Vec::with_capacity(voices);
        for v in 0..voices {
            let mut evs = events(bars, 0, &starts, v)?;
            mark_whole(&mut evs, &bounds);
            per_voice.push(evs);
        }
        let mut out = Vec::with_capacity(count);
        for &(s, e) in &bounds {
            let clef = bars.get(old_at(s))?.clef;
            let mut vs = Vec::with_capacity(voices);
            for evs in &per_voice {
                vs.push(new_voice(evs, s, e, total)?);
            }
            while vs.len() > 1 && vs.last().is_some_and(|v| v.beats.is_empty()) {
                vs.pop();
            }
            out.push(Bar { clef, voices: vs });
        }
        staves.push(out);
    }
    Some(Rebarred {
        master,
        staves,
        moved,
    })
}

/// The passages of `first..=last` re-barred, the rest as it was.
fn rebar(score: &Score, first: usize, last: usize) -> Option<Score> {
    let mut done = Vec::new();
    for (k0, k1) in passages(score, first, last) {
        if let Some(r) = rebar_passage(score, k0, k1) {
            done.push((k0, k1, r));
        }
    }
    if done.is_empty() {
        return None;
    }
    let mut out = Score {
        master_bars: Vec::new(),
        ..score.clone()
    };
    for staff in out.tracks.iter_mut().flat_map(|t| &mut t.staves) {
        staff.bars.clear();
    }
    // Where each old bar's start went: new bar and offset in it.
    let mut moved: Vec<(usize, Ratio)> = Vec::with_capacity(score.bar_count());
    let mut done = done.into_iter().peekable();
    let mut k = 0;
    while k < score.bar_count() {
        if let Some((k0, k1, r)) = done.next_if(|d| d.0 == k) {
            let base = out.master_bars.len();
            moved.extend(r.moved.iter().map(|&(j, off)| (base + j, off)));
            out.master_bars.extend(r.master);
            for (staff, bars) in out
                .tracks
                .iter_mut()
                .flat_map(|t| &mut t.staves)
                .zip(r.staves)
            {
                staff.bars.extend(bars);
            }
            k = k1.max(k0) + 1;
            continue;
        }
        moved.push((out.master_bars.len(), Ratio::ZERO));
        out.master_bars.push(score.master_bars.get(k)?.clone());
        for (dst, src) in out
            .tracks
            .iter_mut()
            .flat_map(|t| &mut t.staves)
            .zip(score.tracks.iter().flat_map(|t| &t.staves))
        {
            dst.bars.push(src.bars.get(k)?.clone());
        }
        k += 1;
    }
    // Lyrics: a line starts on the bar its old first bar's start went to,
    // with an empty beat for each sung beat before that moment.
    for track in &mut out.tracks {
        let first_voice: Vec<Bar> = track
            .staves
            .first()
            .map(|s| s.bars.clone())
            .unwrap_or_default();
        for line in &mut track.lyrics {
            let Some(&(j, off)) = moved.get(line.start_bar) else {
                continue;
            };
            line.start_bar = j;
            if off > Ratio::ZERO {
                let skipped = sung_before(first_voice.get(j), off);
                line.text.insert_str(0, &" ".repeat(skipped));
            }
        }
    }
    out.check_structure().ok()?;
    Some(out)
}

/// How many beats the first voice of `bar` sings before `off`.
fn sung_before(bar: Option<&Bar>, off: Ratio) -> usize {
    let Some(v) = bar.and_then(|b| b.voices.first()) else {
        return 0;
    };
    let mut at = Ratio::ZERO;
    let mut n = 0;
    for b in v.beats.iter().filter(|b| b.grace.is_none()) {
        if at >= off {
            break;
        }
        if oxiguitar_model::lyrics::is_sung(b) {
            n += 1;
        }
        at = at + b.duration.length().unwrap_or(Ratio::ZERO);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BarCheck;
    use crate::caret::tests::score;
    use oxiguitar_model::NoteValue::{Eighth, Half, Quarter, Sixteenth};
    use oxiguitar_model::{
        Accent, LyricLine, MasterBar, Note, NotePlace, Staff, TimeSignature, Track,
    };

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    fn ts(numerator: u8, denominator: u8) -> TimeSignature {
        TimeSignature {
            numerator,
            denominator,
        }
    }

    fn dur(value: NoteValue, dots: u8, tuplets: &[Tuplet]) -> Duration {
        Duration {
            value,
            dots,
            tuplets: tuplets.to_vec(),
        }
    }

    /// A note on the first string at `fret`, as long as `d`.
    fn note(fret: u8, d: Duration) -> Beat {
        Beat {
            duration: d,
            notes: vec![Note::fretted(1, fret)],
            ..Beat::default()
        }
    }

    /// A bar of one voice.
    fn bar(beats: Vec<Beat>) -> Bar {
        Bar {
            voices: vec![Voice { beats }],
            ..Bar::default()
        }
    }

    /// One track whose bars are `bars`, every bar in `sig`.
    fn song(sig: TimeSignature, bars: Vec<Bar>) -> Score {
        let mut s = score(1, bars.len());
        s.tracks[0].staves[0].bars = bars;
        for m in &mut s.master_bars {
            m.time_signature = sig;
        }
        s
    }

    /// `n` bars of four quarters, frets counting up from 0.
    fn quarters(n: usize) -> Vec<Bar> {
        (0..n)
            .map(|b| {
                bar((0..4)
                    .map(|i| note((b * 4 + i) as u8, Duration::new(Quarter)))
                    .collect())
            })
            .collect()
    }

    /// Every beat of a staff's first voice: (fret or none for a rest, tie,
    /// length), bar by bar.
    fn line(s: &Score, track: usize, voice: usize) -> Vec<Vec<(Option<u8>, bool, Ratio)>> {
        s.tracks[track].staves[0]
            .bars
            .iter()
            .map(|b| {
                b.voices
                    .get(voice)
                    .map(|v| {
                        v.beats
                            .iter()
                            .map(|b| {
                                let n = b.notes.first();
                                let fret = n.and_then(|n| match n.place {
                                    NotePlace::Fretted { fret, .. } => Some(fret),
                                    NotePlace::Pitched(_) => None,
                                });
                                (fret, n.is_some_and(|n| n.tie), b.duration.length().unwrap())
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    /// The notes struck, in order, and the total length: what the music
    /// sounds like, whatever its bars.
    fn struck(s: &Score, track: usize, voice: usize) -> (Vec<u8>, Ratio) {
        let mut frets = Vec::new();
        let mut total = Ratio::ZERO;
        for b in line(s, track, voice).into_iter().flatten() {
            if let (Some(f), false) = (b.0, b.1) {
                frets.push(f);
            }
            total = total + b.2;
        }
        (frets, total)
    }

    fn rebarred(s: &Score) -> Score {
        fitted(s, 0, s.bar_count() - 1, FitHow::Rebar).expect("something to re-bar")
    }

    fn stretched(s: &Score) -> Score {
        fitted(s, 0, s.bar_count() - 1, FitHow::Stretch).expect("something to stretch")
    }

    #[test]
    fn lengths_are_written_as_values_and_tuplets() {
        let t3 = Tuplet::TRIPLET;
        assert_eq!(
            spell(r(1, 4), &[], Prefer::Own),
            Some(vec![dur(Quarter, 0, &[])])
        );
        assert_eq!(
            spell(r(3, 16), &[], Prefer::Own),
            Some(vec![dur(Eighth, 1, &[])])
        );
        assert_eq!(
            spell(r(7, 16), &[], Prefer::Own),
            Some(vec![dur(Quarter, 2, &[])])
        );
        // A third of a whole: a half-note triplet.
        assert_eq!(
            spell(r(1, 3), &[], Prefer::Plain),
            Some(vec![dur(Half, 0, &[t3])])
        );
        // A fifth: a quarter in a quintuplet.
        let t5 = Tuplet {
            notes: 5,
            in_time_of: 4,
        };
        assert_eq!(
            spell(r(1, 5), &[], Prefer::Plain),
            Some(vec![dur(Quarter, 0, &[t5])])
        );
        // Half a triplet quarter stays in its triplet...
        assert_eq!(
            spell(r(1, 12), &[t3], Prefer::Own),
            Some(vec![dur(Eighth, 0, &[t3])])
        );
        // ...and a sixteenth leaves it when it can.
        assert_eq!(
            spell(r(1, 16), &[t3], Prefer::Plain),
            Some(vec![dur(Sixteenth, 0, &[])])
        );
        // A triplet stretched by 4/3: a triplet inside a triplet.
        assert_eq!(
            spell(r(2, 9), &[t3], Prefer::Plain),
            Some(vec![dur(Half, 0, &[t3, t3])])
        );
        // No single value: tied, longest first.
        assert_eq!(
            spell(r(9, 32), &[], Prefer::Plain),
            Some(vec![
                dur(Quarter, 0, &[]),
                dur(NoteValue::ThirtySecond, 0, &[])
            ])
        );
        assert_eq!(spell(Ratio::ZERO, &[], Prefer::Plain), None);
    }

    #[test]
    fn four_four_rebarred_into_three_four_and_back() {
        let mut s = song(ts(3, 4), quarters(3));
        let before = struck(&s, 0, 0);
        let out = rebarred(&s);
        assert_eq!(out.bar_count(), 4);
        assert!(BarCheck::of(&out).is_empty());
        assert_eq!(struck(&out, 0, 0), before, "sounds the same");
        assert!(out.master_bars.iter().all(|m| m.time_signature == ts(3, 4)));
        // And back: 4/4 again on the three-four bars.
        s = out;
        for m in &mut s.master_bars {
            m.time_signature = ts(4, 4);
        }
        let back = rebarred(&s);
        assert_eq!(back.bar_count(), 3);
        assert!(BarCheck::of(&back).is_empty());
        assert_eq!(back.tracks, song(ts(4, 4), quarters(3)).tracks);
    }

    #[test]
    fn notes_across_a_new_bar_line_are_tied() {
        // 4/4: a half (accented, hammered on to the next) and a half; in
        // 3/4 the second half crosses the bar line.
        let mut first = note(5, Duration::new(Half));
        let mut second = note(7, Duration::new(Half));
        second.notes[0].effects.accent = Some(Accent::Normal);
        second.notes[0].effects.hammer_pull = true;
        second.text = "here".to_owned();
        first.notes[0].effects.palm_mute = true;
        let s = song(
            ts(3, 4),
            vec![
                bar(vec![first, second]),
                bar(vec![
                    note(8, Duration::new(Quarter)),
                    note(9, dur(Half, 1, &[])),
                ]),
            ],
        );
        let out = rebarred(&s);
        let l = line(&out, 0, 0);
        assert_eq!(
            l,
            [
                vec![(Some(5), false, r(1, 2)), (Some(7), false, r(1, 4))],
                vec![
                    (Some(7), true, r(1, 4)),
                    (Some(8), false, r(1, 4)),
                    (Some(9), false, r(1, 4)),
                ],
                vec![(Some(9), true, r(1, 2))],
            ]
        );
        let head = &out.tracks[0].staves[0].bars[0].voices[0].beats[1];
        let tail = &out.tracks[0].staves[0].bars[1].voices[0].beats[0];
        assert_eq!(head.text, "here");
        assert!(tail.text.is_empty());
        assert_eq!(head.notes[0].effects.accent, Some(Accent::Normal));
        assert_eq!(tail.notes[0].effects.accent, None);
        assert!(!head.notes[0].effects.hammer_pull);
        assert!(
            tail.notes[0].effects.hammer_pull,
            "leads on from the last piece"
        );
        // Eight quarters make two bars of 3/4 and a short one.
        let bad: Vec<usize> = BarCheck::of(&out).iter().map(|b| b.bar).collect();
        assert_eq!(bad, [2]);
    }

    #[test]
    fn six_eight_and_three_four_are_the_same_length() {
        // 6/8 written as a dotted quarter and three eighths: in 3/4 the bars
        // still add up, so neither way changes anything.
        let six = || {
            bar(vec![
                note(0, dur(Quarter, 1, &[])),
                note(1, Duration::new(Eighth)),
                note(2, Duration::new(Eighth)),
                note(3, Duration::new(Eighth)),
            ])
        };
        let s = song(ts(3, 4), vec![six(), six()]);
        assert!(BarCheck::of(&s).is_empty());
        assert_eq!(fitted(&s, 0, 1, FitHow::Rebar), None);
        assert_eq!(fitted(&s, 0, 1, FitHow::Stretch), None);
        // 6/8 to 3/8 (half as long) and back re-bars the music.
        let mut s = s;
        for m in &mut s.master_bars {
            m.time_signature = ts(3, 8);
        }
        let out = rebarred(&s);
        assert_eq!(out.bar_count(), 4);
        assert!(BarCheck::of(&out).is_empty());
        assert_eq!(struck(&out, 0, 0), struck(&s, 0, 0));
        let mut back = out;
        for m in &mut back.master_bars {
            m.time_signature = ts(6, 8);
        }
        let back = rebarred(&back);
        assert_eq!(back.tracks, s.tracks);
    }

    #[test]
    fn stretching_three_four_into_four_four_needs_triplets() {
        let three = |f: u8| {
            bar((0..3)
                .map(|i| note(f + i, Duration::new(Quarter)))
                .collect())
        };
        let s = song(ts(4, 4), vec![three(0), three(3)]);
        let out = stretched(&s);
        assert_eq!(out.bar_count(), 2, "the bars stay");
        assert!(BarCheck::of(&out).is_empty());
        for b in &out.tracks[0].staves[0].bars {
            let beats = &b.voices[0].beats;
            assert_eq!(beats.len(), 3);
            assert!(
                beats
                    .iter()
                    .all(|b| b.duration == dur(Half, 0, &[Tuplet::TRIPLET]))
            );
        }
        assert_eq!(struck(&out, 0, 0).0, [0, 1, 2, 3, 4, 5]);
        // And back to 3/4: the triplets go.
        let mut back = out;
        for m in &mut back.master_bars {
            m.time_signature = ts(3, 4);
        }
        assert_eq!(stretched(&back).tracks, s.tracks);
    }

    #[test]
    fn stretching_triplets_into_three_four_makes_sixteenths() {
        // 4/4 full of triplet eighths, now in 3/4: twelve sixteenths.
        let trip = dur(Eighth, 0, &[Tuplet::TRIPLET]);
        let s = song(
            ts(3, 4),
            vec![bar((0..12).map(|i| note(i, trip.clone())).collect())],
        );
        let out = stretched(&s);
        let beats = &out.tracks[0].staves[0].bars[0].voices[0].beats;
        assert_eq!(beats.len(), 12);
        assert!(beats.iter().all(|b| b.duration == Duration::new(Sixteenth)));
        assert!(BarCheck::of(&out).is_empty());
        // Plain quarters from 4/4 into 3/4: dotted eighths, no tuplet.
        let s = song(ts(3, 4), quarters(1));
        let out = stretched(&s);
        let beats = &out.tracks[0].staves[0].bars[0].voices[0].beats;
        assert!(beats.iter().all(|b| b.duration == dur(Eighth, 1, &[])));
    }

    #[test]
    fn a_tuplet_split_by_a_bar_line_stays_a_tuplet() {
        // 4/4: a half, then a half-note triplet over beats 3 and 4. In 3/4
        // the triplet's second note crosses the bar line at beat 4.
        let t = dur(Quarter, 0, &[Tuplet::TRIPLET]);
        let mut beats = vec![note(0, Duration::new(Half))];
        beats.extend((1..4).map(|i| note(i, t.clone())));
        let s = song(
            ts(3, 4),
            vec![
                bar(beats),
                bar(quarters(1).remove(0).voices.remove(0).beats),
            ],
        );
        let out = rebarred(&s);
        let b0 = &out.tracks[0].staves[0].bars[0].voices[0].beats;
        let b1 = &out.tracks[0].staves[0].bars[1].voices[0].beats;
        assert_eq!(b0[2].duration, dur(Eighth, 0, &[Tuplet::TRIPLET]));
        assert_eq!(b1[0].duration, dur(Eighth, 0, &[Tuplet::TRIPLET]));
        assert!(b1[0].notes[0].tie);
        let bad: Vec<usize> = BarCheck::of(&out).iter().map(|b| b.bar).collect();
        assert_eq!(bad, [2], "only the short last bar");
        assert_eq!(struck(&out, 0, 0), struck(&s, 0, 0));
    }

    #[test]
    fn every_voice_and_track_moves_together() {
        let mut s = song(ts(3, 4), quarters(3));
        // Track 1, voice 2: half notes; a second track of whole notes.
        for (i, b) in s.tracks[0].staves[0].bars.iter_mut().enumerate() {
            b.voices.push(Voice {
                beats: vec![
                    note(20 + 2 * i as u8, Duration::new(Half)),
                    note(21 + 2 * i as u8, Duration::new(Half)),
                ],
            });
        }
        let mut other = s.tracks[0].clone();
        other.staves[0].bars = (0..3)
            .map(|i| bar(vec![note(i, Duration::new(NoteValue::Whole))]))
            .collect();
        s.tracks.push(other);
        let out = rebarred(&s);
        assert!(out.check_structure().is_ok());
        assert_eq!(out.bar_count(), 4);
        assert!(BarCheck::of(&out).is_empty());
        for (t, v) in [(0, 0), (0, 1), (1, 0)] {
            assert_eq!(struck(&out, t, v), struck(&s, t, v), "track {t} voice {v}");
        }
        // The whole notes are split as dotted halves tied to quarters...
        assert_eq!(
            line(&out, 1, 0)[1],
            [(Some(0), true, r(1, 4)), (Some(1), false, r(1, 2))]
        );
    }

    #[test]
    fn a_pickup_stays_and_a_short_last_bar_stays_short() {
        let mut bars = vec![bar(vec![note(99, Duration::new(Quarter))])];
        bars.extend(quarters(4));
        let s = song(ts(3, 4), bars);
        let out = rebarred(&s);
        // The pickup, then sixteen quarters: five bars of 3/4 and one of a
        // quarter, left short (Mateus, 8 Nov 2026).
        assert_eq!(out.bar_count(), 7);
        assert_eq!(line(&out, 0, 0)[0], [(Some(99), false, r(1, 4))]);
        let check = BarCheck::of(&out);
        let bad: Vec<usize> = check.iter().map(|b| b.bar).collect();
        assert_eq!(bad, [0, 6], "the pickup and the short last bar");
        assert_eq!(line(&out, 0, 0)[6], [(Some(15), false, r(1, 4))]);
        // Stretch leaves the pickup alone too.
        let st = stretched(&s);
        assert_eq!(
            st.tracks[0].staves[0].bars[0],
            s.tracks[0].staves[0].bars[0]
        );
    }

    #[test]
    fn repeats_endings_and_sections_keep_their_bar_lines() {
        // Bars 1-2 repeated, bar 3 a section with a coda sign, then bar 4
        // ending in Fine; everything in 4/4 music now marked 3/4.
        let mut s = song(ts(3, 4), quarters(4));
        s.master_bars[0].repeat_start = true;
        s.master_bars[1].repeat_end = Some(3);
        s.master_bars[2].marker = Some(oxiguitar_model::Marker {
            text: "Bridge".to_owned(),
            ..Default::default()
        });
        s.master_bars[3].directions = vec![Direction::Fine];
        let out = rebarred(&s);
        // Repeat: 8 quarters, three bars (3, 3, 2). Section: 4 quarters,
        // two bars (3, 1). Fine: 4 quarters, two bars (3, 1).
        assert_eq!(out.bar_count(), 7);
        let m = &out.master_bars;
        assert!(m[0].repeat_start && !m[1].repeat_start);
        assert_eq!((m[1].repeat_end, m[2].repeat_end), (None, Some(3)));
        assert_eq!(
            m[3].marker.as_ref().map(|x| x.text.as_str()),
            Some("Bridge")
        );
        assert!(m[4].marker.is_none());
        assert_eq!(m[6].directions, [Direction::Fine]);
        assert!(m[5].directions.is_empty());
        let bad: Vec<usize> = BarCheck::of(&out).iter().map(|b| b.bar).collect();
        assert_eq!(bad, [2, 4, 6]);
        assert_eq!(struck(&out, 0, 0), struck(&s, 0, 0));
    }

    #[test]
    fn only_bars_that_dont_add_up_change() {
        // Bars 1-2 in 4/4 and full; bars 3-4 marked 3/4 with 4/4 music.
        let mut s = song(ts(4, 4), quarters(4));
        s.master_bars[2].time_signature = ts(3, 4);
        s.master_bars[3].time_signature = ts(3, 4);
        let out = rebarred(&s);
        assert_eq!(out.bar_count(), 5);
        assert_eq!(
            out.tracks[0].staves[0].bars[..2],
            s.tracks[0].staves[0].bars[..2]
        );
        // Eight quarters: two bars of 3/4 and a short one.
        let bad: Vec<usize> = BarCheck::of(&out).iter().map(|b| b.bar).collect();
        assert_eq!(bad, [4]);
        // Fitting only the first bar finds nothing to do.
        assert_eq!(fitted(&s, 0, 0, FitHow::Rebar), None);
        assert_eq!(fitted(&s, 0, 1, FitHow::Stretch), None);
    }

    #[test]
    fn tempo_changes_and_lyrics_keep_their_moment() {
        let mut s = song(ts(3, 4), quarters(4));
        // A tempo change on bar 3's second beat: 9 quarters in.
        s.master_bars[2].tempo_changes.push(TempoChange {
            position: r(1, 4),
            bpm: 90.0,
            label: String::new(),
            linear: false,
        });
        s.tracks[0].lyrics = vec![
            LyricLine {
                start_bar: 0,
                text: "a b c".to_owned(),
            },
            LyricLine {
                start_bar: 2,
                text: "x y".to_owned(),
            },
        ];
        let out = rebarred(&s);
        // 9 quarters in: the fourth bar of 3/4, its first beat.
        assert_eq!(out.master_bars[3].tempo_changes[0].position, Ratio::ZERO);
        assert_eq!(out.master_bars[3].tempo_changes[0].bpm, 90.0);
        // Bar 3 started 8 quarters in: bar 3 of 3/4, after two sung beats.
        assert_eq!(out.tracks[0].lyrics[0].start_bar, 0);
        assert_eq!(out.tracks[0].lyrics[1].start_bar, 2);
        assert_eq!(out.tracks[0].lyrics[1].text, "  x y");
        let sung = oxiguitar_model::lyrics::sung_beats(&out.tracks[0].staves[0], 2);
        let syl = oxiguitar_model::lyrics::syllables(&out.tracks[0].lyrics[1].text);
        // "x" lands on the beat holding fret 8, as before.
        let x = syl.iter().position(|y| y.text == "x").unwrap();
        let (b, i) = sung[x];
        assert_eq!(
            out.tracks[0].staves[0].bars[b].voices[0].beats[i].notes[0].place,
            NotePlace::Fretted { string: 1, fret: 8 }
        );
    }

    #[test]
    fn one_change_for_undo() {
        let mut s = song(ts(3, 4), quarters(4));
        let original = s.clone();
        let mut h = History::new();
        let c = Caret {
            track: 0,
            staff: 0,
            voice: 0,
            bar: 3,
            beat: 3,
            string: 1,
        };
        let fit = Fit {
            how: FitHow::Rebar,
            whole_song: true,
        };
        let e = crate::apply(&mut s, &mut h, c, None, Command::FitBars(fit)).unwrap();
        assert!(e.touched.everything);
        assert_eq!(s.bar_count(), 6);
        assert_eq!(h.undo_label(), Some("Fit Bars"));
        h.undo(&mut s).unwrap();
        assert_eq!(s, original);
        // The caret's bar alone (the page's menu, nothing selected).
        let one = Fit {
            how: FitHow::Stretch,
            whole_song: false,
        };
        crate::apply(&mut s, &mut h, c, None, Command::FitBars(one)).unwrap();
        assert_eq!(s.bar_count(), 4);
        let bad: Vec<usize> = BarCheck::of(&s).iter().map(|b| b.bar).collect();
        assert_eq!(bad, [0, 1, 2]);
        // Nothing to do: nothing recorded.
        assert!(crate::apply(&mut s, &mut h, c, None, Command::FitBars(one)).is_none());
    }

    #[test]
    fn fitted_bars_of_a_new_track_have_strings() {
        // A plain guitar track keeps its tuning and staff settings.
        let s = Score {
            tracks: vec![Track {
                staves: vec![Staff {
                    tuning: Staff::standard_guitar_tuning(),
                    bars: quarters(2),
                    ..Staff::default()
                }],
                ..Track::default()
            }],
            master_bars: vec![
                MasterBar {
                    time_signature: ts(2, 4),
                    ..MasterBar::default()
                };
                2
            ],
            ..Score::new()
        };
        let out = rebarred(&s);
        assert_eq!(out.bar_count(), 4);
        assert_eq!(out.tracks[0].staves[0].tuning, s.tracks[0].staves[0].tuning);
    }
}
