// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Bars and structure (M3 slice 4, EDIT-6 and EDIT-7): inserting and
//! deleting bars, and what a bar carries for every track (time and key
//! signature, tempo, feel, repeats, endings, directions, sections) or for
//! one staff (its clef).
//!
//! A command works on the caret's bar, or on the selection's bars: a
//! change of signature, clef or feel starts at the first and holds until
//! the next bar that had another (as Guitar Pro changes it "until the next
//! change"); a repeat opens at the first bar and closes at the last.
//! Changes to what every track shares are recorded as master bars, so
//! undoing one costs those bars, not the score.

use oxiguitar_model::{
    Bar, Clef, Direction, KeySignature, Marker, MasterBar, Ratio, Score, TempoChange,
    TimeSignature, TripletFeel, Voice,
};

use crate::{Caret, Command, Edited, History, Region, Selection};

/// Runs a bar command; `None` for a command that isn't one, or that
/// changes nothing.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let (first, last) = selection.map_or((c.bar, c.bar), |s| s.bars());
    let count = score.bar_count();
    if last >= count {
        return Some(None);
    }
    let label = command.label();
    let edited = match command {
        Command::InsertBar => insert(score, history, c, first, label),
        Command::DeleteBars => delete(score, history, c, first, last, label),
        Command::Clef(clef) => clef_from(score, history, c, first, *clef, label),
        Command::Tempo(bpm) => tempo(score, history, c, first, *bpm, label),
        Command::Bar(to) => properties(score, history, c, selection, to, label),
        _ => {
            let (from, bars) = master(score, first, last, command)?;
            if score.master_bars.get(from..from + bars.len())? == bars.as_slice() {
                return Some(None);
            }
            let end = from + bars.len();
            let touched = history.record(score, label, vec![Region::MasterBars(from..end)], |s| {
                if let Some(dst) = s.master_bars.get_mut(from..end) {
                    dst.clone_from_slice(&bars);
                }
            });
            Some(Edited { touched, caret: c })
        }
    };
    Some(edited)
}

/// The master bars a command over bars `first..=last` makes, from the bar
/// they start at; `None` for a command that isn't about master bars.
fn master(
    score: &Score,
    first: usize,
    last: usize,
    command: &Command,
) -> Option<(usize, Vec<MasterBar>)> {
    let bars = &score.master_bars;
    let one = |at: usize, f: &dyn Fn(&mut MasterBar)| {
        let mut b = bars.get(at)?.clone();
        f(&mut b);
        Some((at, vec![b]))
    };
    match command {
        Command::RepeatStart => {
            let on = !bars.get(first)?.repeat_start;
            one(first, &|b| b.repeat_start = on)
        }
        Command::RepeatEnd => {
            let to = match bars.get(last)?.repeat_end {
                Some(_) => None,
                None => Some(2),
            };
            one(last, &|b| b.repeat_end = to)
        }
        Command::RepeatCount(n) => {
            let to = (*n >= 2).then_some(*n);
            one(last, &|b| b.repeat_end = to)
        }
        Command::DoubleBar => {
            let on = !bars.get(last)?.double_bar;
            one(last, &|b| b.double_bar = on)
        }
        Command::AlternateEndings(endings) => Some((
            first,
            bars.get(first..=last)?
                .iter()
                .map(|b| MasterBar {
                    alternate_endings: *endings,
                    ..b.clone()
                })
                .collect(),
        )),
        Command::FreeTime => {
            let on = !bars.get(first)?.free_time;
            Some((
                first,
                bars.get(first..=last)?
                    .iter()
                    .map(|b| MasterBar {
                        free_time: on,
                        ..b.clone()
                    })
                    .collect(),
            ))
        }
        Command::Direction(d) => {
            let has = bars.get(first)?.directions.contains(d);
            one(first, &|b| {
                if has {
                    b.directions.retain(|x| x != d);
                } else {
                    b.directions.push(*d);
                }
            })
        }
        Command::Section(marker) => {
            let marker = marker
                .clone()
                .filter(|m| !(m.letter.is_empty() && m.text.is_empty()));
            one(first, &|b| b.marker.clone_from(&marker))
        }
        Command::TimeSignature(ts) => {
            let (from, mut out) = until_changed(bars, first, |b| &mut b.time_signature, *ts)?;
            for (b, old) in out.iter_mut().zip(bars.get(from..)?) {
                rescale_tempo(b, old.time_signature);
            }
            Some((from, out))
        }
        Command::KeySignature(ks) => until_changed(bars, first, |b| &mut b.key_signature, *ks),
        Command::TripletFeel(feel) => until_changed(bars, first, |b| &mut b.triplet_feel, *feel),
        _ => None,
    }
}

/// A bar's tempo changes after its time signature changed from `old`:
/// at the same fraction of the bar, as Guitar Pro keeps them, on its grid
/// of 960 ticks a quarter note.
fn rescale_tempo(bar: &mut MasterBar, old: TimeSignature) {
    const TICKS_PER_WHOLE: i64 = 3840;
    let (Some(old), Some(new)) = (old.bar_length(), bar.time_signature.bar_length()) else {
        return;
    };
    if old == new {
        return;
    }
    for t in &mut bar.tempo_changes {
        let Some(scaled) = t.position.checked_mul(new).and_then(|p| p.checked_div(old)) else {
            continue;
        };
        let ticks = (scaled.to_f64() * TICKS_PER_WHOLE as f64).round() as i64;
        if let Some(r) = Ratio::new(ticks, TICKS_PER_WHOLE) {
            t.position = r;
        }
    }
}

/// Bar `first` given `value` in the field `field` picks, and every bar
/// after it that had the same as `first` had, up to the next change.
fn until_changed<T: Copy + PartialEq>(
    bars: &[MasterBar],
    first: usize,
    field: impl Fn(&mut MasterBar) -> &mut T,
    value: T,
) -> Option<(usize, Vec<MasterBar>)> {
    let mut start = bars.get(first)?.clone();
    let old = *field(&mut start);
    let mut out = Vec::new();
    for b in bars.get(first..)? {
        let mut b = b.clone();
        if !out.is_empty() && *field(&mut b) != old {
            break;
        }
        *field(&mut b) = value;
        out.push(b);
    }
    Some((first, out))
}

/// A bar before bar `at`, in its time, key and feel; in every staff an
/// empty voice in the clef of the bar it goes before.
fn insert(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    at: usize,
    label: &'static str,
) -> Option<Edited> {
    let next = score.master_bars.get(at)?.clone();
    let touched = history.record(score, label, vec![Region::Whole], |s| {
        s.master_bars.insert(
            at,
            MasterBar {
                time_signature: next.time_signature,
                key_signature: next.key_signature,
                triplet_feel: next.triplet_feel,
                free_time: next.free_time,
                ..MasterBar::default()
            },
        );
        for staff in s.tracks.iter_mut().flat_map(|t| &mut t.staves) {
            let clef = staff.bars.get(at).map_or_else(Default::default, |b| b.clef);
            staff.bars.insert(
                at,
                Bar {
                    clef,
                    voices: vec![Voice::default()],
                },
            );
        }
    });
    let caret = Caret {
        bar: at,
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(caret),
    })
}

/// Bars `first..=last` removed from every track; never every bar. A tempo
/// change in them is kept on the bar after them, so the music after them
/// plays as before.
fn delete(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    first: usize,
    last: usize,
    label: &'static str,
) -> Option<Edited> {
    let count = score.bar_count();
    if last + 1 - first >= count {
        return None;
    }
    // The tempo the deleted bars end in, for the bar after them.
    let carried = score
        .master_bars
        .get(first..=last)?
        .iter()
        .flat_map(|b| &b.tempo_changes)
        .last()
        .cloned();
    let touched = history.record(score, label, vec![Region::Whole], |s| {
        s.master_bars.drain(first..=last);
        for staff in s.tracks.iter_mut().flat_map(|t| &mut t.staves) {
            staff.bars.drain(first..=last);
            // A tie after them may have lost the note it continued.
            crate::entry::retie(staff, first);
        }
        if let (Some(change), Some(next)) = (carried, s.master_bars.get_mut(first))
            && !next.tempo_changes.iter().any(|t| t.position == Ratio::ZERO)
        {
            next.tempo_changes.insert(
                0,
                TempoChange {
                    position: Ratio::ZERO,
                    linear: false,
                    ..change
                },
            );
        }
    });
    let caret = Caret {
        bar: first.min(score.bar_count().saturating_sub(1)),
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(caret),
    })
}

/// The caret's staff in `clef` from bar `first` until its next clef change.
fn clef_from(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    first: usize,
    clef: Clef,
    label: &'static str,
) -> Option<Edited> {
    let track = score.tracks.get(c.track)?;
    // A drum track has the drums' clef.
    if track.instrument.percussion {
        return None;
    }
    let staff = track.staves.get(c.staff)?;
    let old = staff.bars.get(first)?.clef;
    // Sounding an octave below the treble or bass clef belongs to the
    // staff (Guitar Pro keeps it per track): the clef chosen keeps it.
    let low = |c: Clef| matches!(c, Clef::Treble8vb | Clef::Bass8vb);
    let high = |c: Clef| matches!(c, Clef::Treble | Clef::Bass);
    let octave = if low(old) || high(old) {
        low(old)
    } else {
        let lows = staff.bars.iter().filter(|b| low(b.clef)).count();
        lows > 0 && lows >= staff.bars.iter().filter(|b| high(b.clef)).count()
    };
    let clef = match (clef, octave) {
        (Clef::Treble | Clef::Treble8vb, true) => Clef::Treble8vb,
        (Clef::Treble | Clef::Treble8vb, false) => Clef::Treble,
        (Clef::Bass | Clef::Bass8vb, true) => Clef::Bass8vb,
        (Clef::Bass | Clef::Bass8vb, false) => Clef::Bass,
        (other, _) => other,
    };
    let end = first
        + staff
            .bars
            .get(first..)?
            .iter()
            .take_while(|b| b.clef == old)
            .count();
    if old == clef {
        return None;
    }
    let (track, st) = (c.track, c.staff);
    let region = Region::Bars {
        track,
        staff: st,
        bars: first..end,
    };
    let touched = history.record(score, label, vec![region], |s| {
        if let Some(bars) = s
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(st))
            .and_then(|s| s.bars.get_mut(first..end))
        {
            bars.iter_mut().for_each(|b| b.clef = clef);
        }
    });
    Some(Edited { touched, caret: c })
}

/// The tempo at the start of bar `first`: the score's tempo for the first
/// bar, a tempo change at the bar's start for the others.
fn tempo(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    first: usize,
    bpm: u16,
    label: &'static str,
) -> Option<Edited> {
    if bpm == 0 {
        return None;
    }
    let bpm = f64::from(bpm);
    let mut bar = score.master_bars.get(first)?.clone();
    let at_start = bar
        .tempo_changes
        .iter()
        .position(|t| t.position == Ratio::ZERO);
    let regions = if first == 0 {
        // A change at the very start is the score's tempo.
        if let Some(i) = at_start {
            bar.tempo_changes.remove(i);
        }
        if score.initial_tempo == bpm && at_start.is_none() {
            return None;
        }
        vec![Region::Info, Region::MasterBars(0..1)]
    } else {
        match at_start.and_then(|i| bar.tempo_changes.get_mut(i)) {
            Some(t) if t.bpm == bpm => return None,
            Some(t) => t.bpm = bpm,
            None => bar.tempo_changes.insert(
                0,
                TempoChange {
                    position: Ratio::ZERO,
                    bpm,
                    label: String::new(),
                    linear: false,
                },
            ),
        }
        vec![Region::MasterBars(first..first + 1)]
    };
    let touched = history.record(score, label, regions, |s| {
        if first == 0 {
            s.initial_tempo = bpm;
        }
        if let Some(b) = s.master_bars.get_mut(first) {
            *b = bar;
        }
    });
    Some(Edited { touched, caret: c })
}

/// The Bar dialog's settings as one change: each part that differs from
/// the bar's, run as its own command on a copy, then recorded at once.
fn properties(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    to: &BarState,
    label: &'static str,
) -> Option<Edited> {
    let (first, _) = selection.map_or((c.bar, c.bar), |s| s.bars());
    let now = BarState::at(score, Caret { bar: first, ..c })?;
    let mut steps = Vec::new();
    if to.time_signature != now.time_signature {
        steps.push(Command::TimeSignature(to.time_signature));
    }
    if to.key_signature != now.key_signature {
        steps.push(Command::KeySignature(to.key_signature));
    }
    if to.clef != now.clef {
        steps.push(Command::Clef(to.clef));
    }
    if to.tempo != now.tempo {
        steps.push(Command::Tempo(to.tempo.round().clamp(1.0, 2000.0) as u16));
    }
    if to.triplet_feel != now.triplet_feel {
        steps.push(Command::TripletFeel(to.triplet_feel));
    }
    if to.repeat_start != now.repeat_start {
        steps.push(Command::RepeatStart);
    }
    if to.repeat_end != now.repeat_end {
        steps.push(Command::RepeatCount(to.repeat_end.unwrap_or(0)));
    }
    if to.alternate_endings != now.alternate_endings {
        steps.push(Command::AlternateEndings(to.alternate_endings));
    }
    for d in to.directions.iter().filter(|d| !now.directions.contains(d)) {
        steps.push(Command::Direction(*d));
    }
    for d in now.directions.iter().filter(|d| !to.directions.contains(d)) {
        steps.push(Command::Direction(*d));
    }
    if to.marker != now.marker {
        steps.push(Command::Section(to.marker.clone()));
    }
    if to.double_bar != now.double_bar {
        steps.push(Command::DoubleBar);
    }
    if to.free_time != now.free_time {
        steps.push(Command::FreeTime);
    }
    let mut work = score.clone();
    let mut scratch = History::new();
    let mut any = false;
    for step in steps {
        any |= crate::apply(&mut work, &mut scratch, c, selection, step).is_some();
    }
    if !any {
        return None;
    }
    // What those commands can change: the master bars and the caret's
    // staff from the first bar on, and the score's tempo.
    let end = score.bar_count();
    let regions = vec![
        Region::Info,
        Region::MasterBars(first..end),
        Region::Bars {
            track: c.track,
            staff: c.staff,
            bars: first..end,
        },
    ];
    let touched = history.record(score, label, regions, |s| {
        s.initial_tempo = work.initial_tempo;
        if let (Some(dst), Some(src)) = (
            s.master_bars.get_mut(first..),
            work.master_bars.get(first..),
        ) {
            dst.clone_from_slice(src);
        }
        if let (Some(dst), Some(src)) = (
            s.tracks
                .get_mut(c.track)
                .and_then(|t| t.staves.get_mut(c.staff))
                .and_then(|st| st.bars.get_mut(first..)),
            work.tracks
                .get(c.track)
                .and_then(|t| t.staves.get(c.staff))
                .and_then(|st| st.bars.get(first..)),
        ) {
            dst.clone_from_slice(src);
        }
    });
    Some(Edited { touched, caret: c })
}

/// What the caret's bar carries, for the bar dialogs and the palette.
#[derive(Clone, Debug, PartialEq)]
pub struct BarState {
    /// Its time signature.
    pub time_signature: TimeSignature,
    /// Its key signature.
    pub key_signature: KeySignature,
    /// The caret's staff's clef there.
    pub clef: Clef,
    /// The tempo in force at its start, in quarter notes per minute.
    pub tempo: f64,
    /// Its feel.
    pub triplet_feel: TripletFeel,
    /// A repeat opens there.
    pub repeat_start: bool,
    /// A repeat closes there, played this many times.
    pub repeat_end: Option<u8>,
    /// Its alternate endings, as a bit set.
    pub alternate_endings: u16,
    /// Its directions.
    pub directions: Vec<Direction>,
    /// Its section.
    pub marker: Option<Marker>,
    /// A double bar line ends it.
    pub double_bar: bool,
    /// It is in free time.
    pub free_time: bool,
}

impl BarState {
    /// The caret's bar.
    pub fn at(score: &Score, c: Caret) -> Option<BarState> {
        let b = score.master_bars.get(c.bar)?;
        let clef = score
            .tracks
            .get(c.track)?
            .staves
            .get(c.staff)?
            .bars
            .get(c.bar)?
            .clef;
        let tempo = score
            .master_bars
            .get(..=c.bar)?
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.tempo_changes
                    .iter()
                    .filter(move |t| i < c.bar || t.position == Ratio::ZERO)
            })
            .last()
            .map_or(score.initial_tempo, |t| t.bpm);
        Some(BarState {
            time_signature: b.time_signature,
            key_signature: b.key_signature,
            clef,
            tempo,
            triplet_feel: b.triplet_feel,
            repeat_start: b.repeat_start,
            repeat_end: b.repeat_end,
            alternate_endings: b.alternate_endings,
            directions: b.directions.clone(),
            marker: b.marker.clone(),
            double_bar: b.double_bar,
            free_time: b.free_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, apply as run, new_score};
    use oxiguitar_model::KeyMode;

    fn score(bars: usize) -> Score {
        let mut s = new_score();
        for _ in 1..bars {
            let c = Caret {
                bar: s.bar_count() - 1,
                ..Caret::at_start(&s, 0).unwrap()
            };
            run(&mut s, &mut History::new(), c, None, Command::AppendBar).unwrap();
        }
        s
    }

    fn at(bar: usize) -> Caret {
        Caret {
            bar,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    fn do_it(
        s: &mut Score,
        h: &mut History,
        c: Caret,
        sel: Option<Selection>,
        cmd: Command,
    ) -> Option<Edited> {
        run(s, h, c, sel, cmd)
    }

    #[test]
    fn signatures_hold_until_the_next_change() {
        let mut s = score(6);
        let mut h = History::new();
        let three = TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        do_it(&mut s, &mut h, at(4), None, Command::TimeSignature(three)).unwrap();
        let six = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        let e = do_it(&mut s, &mut h, at(1), None, Command::TimeSignature(six)).unwrap();
        let ts: Vec<u8> = s
            .master_bars
            .iter()
            .map(|b| b.time_signature.numerator)
            .collect();
        assert_eq!(ts, [4, 6, 6, 6, 3, 3]);
        // A tempo change halfway through stays halfway through.
        s.master_bars[5].tempo_changes.push(TempoChange {
            position: Ratio::new(3, 8).unwrap(),
            bpm: 80.0,
            label: String::new(),
            linear: false,
        });
        do_it(&mut s, &mut h, at(5), None, Command::TimeSignature(six)).unwrap();
        assert_eq!(
            s.master_bars[5].tempo_changes[0].position,
            Ratio::new(3, 8).unwrap()
        );
        do_it(
            &mut s,
            &mut h,
            at(5),
            None,
            Command::TimeSignature(TimeSignature::COMMON),
        )
        .unwrap();
        assert_eq!(
            s.master_bars[5].tempo_changes[0].position,
            Ratio::new(1, 2).unwrap()
        );
        h.undo(&mut s);
        h.undo(&mut s);
        s.master_bars[5].tempo_changes.clear();
        // Every staff's bars 1 to 3 are checked again.
        assert_eq!(e.touched.bars.len(), 3);
        let d = KeySignature {
            fifths: 2,
            mode: KeyMode::Major,
        };
        do_it(&mut s, &mut h, at(0), None, Command::KeySignature(d)).unwrap();
        assert!(s.master_bars.iter().all(|b| b.key_signature.fifths == 2));
        h.undo(&mut s);
        h.undo(&mut s);
        assert!(
            s.master_bars
                .iter()
                .all(|b| b.time_signature.numerator != 6)
        );
    }

    #[test]
    fn the_bar_dialog_is_one_change() {
        let mut s = score(3);
        let mut h = History::new();
        let mut to = BarState::at(&s, at(1)).unwrap();
        to.time_signature = TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        to.tempo = 100.0;
        to.repeat_end = Some(3);
        to.directions = vec![Direction::Coda];
        let e = do_it(
            &mut s,
            &mut h,
            at(1),
            None,
            Command::Bar(Box::new(to.clone())),
        )
        .unwrap();
        assert_eq!(BarState::at(&s, at(1)).unwrap(), to);
        assert_eq!(s.master_bars[2].time_signature.numerator, 3);
        assert!(!e.touched.bars.is_empty());
        assert_eq!(h.undo_label(), Some("Bar Properties"));
        assert!(do_it(&mut s, &mut h, at(1), None, Command::Bar(Box::new(to))).is_none());
        h.undo(&mut s);
        assert_eq!(s, score(3));
    }

    #[test]
    fn inserting_and_deleting_bars() {
        let mut s = score(3);
        let mut h = History::new();
        s.master_bars[1].tempo_changes.push(TempoChange {
            position: Ratio::ZERO,
            bpm: 90.0,
            label: String::new(),
            linear: false,
        });
        let e = do_it(&mut s, &mut h, at(1), None, Command::InsertBar).unwrap();
        assert_eq!((s.bar_count(), e.caret.bar), (4, 1));
        assert!(s.master_bars[1].tempo_changes.is_empty());
        s.check_structure().unwrap();
        // Deleting bar 2 (the one with 90) keeps 90 for the bar after it.
        do_it(&mut s, &mut h, at(2), None, Command::DeleteBars).unwrap();
        assert_eq!(s.bar_count(), 3);
        assert_eq!(s.master_bars[2].tempo_changes[0].bpm, 90.0);
        // Never every bar.
        let all = Selection::all(&s).unwrap();
        assert!(do_it(&mut s, &mut h, at(0), Some(all), Command::DeleteBars).is_none());
        h.undo(&mut s);
        h.undo(&mut s);
        assert_eq!(s.bar_count(), 3);
        assert_eq!(s.master_bars[1].tempo_changes[0].bpm, 90.0);
    }

    #[test]
    fn repeats_endings_directions_sections_and_tempo() {
        let mut s = score(4);
        let mut h = History::new();
        let sel = Selection::between(at(1), at(2));
        do_it(&mut s, &mut h, at(2), Some(sel), Command::RepeatStart).unwrap();
        do_it(&mut s, &mut h, at(2), Some(sel), Command::RepeatEnd).unwrap();
        assert!(s.master_bars[1].repeat_start);
        assert_eq!(s.master_bars[2].repeat_end, Some(2));
        do_it(&mut s, &mut h, at(2), None, Command::RepeatCount(4)).unwrap();
        assert_eq!(s.master_bars[2].repeat_end, Some(4));
        do_it(&mut s, &mut h, at(2), None, Command::RepeatEnd).unwrap();
        assert_eq!(s.master_bars[2].repeat_end, None);
        do_it(&mut s, &mut h, at(2), None, Command::AlternateEndings(0b10)).unwrap();
        do_it(
            &mut s,
            &mut h,
            at(3),
            None,
            Command::Direction(Direction::Fine),
        )
        .unwrap();
        assert_eq!(s.master_bars[3].directions, [Direction::Fine]);
        do_it(
            &mut s,
            &mut h,
            at(3),
            None,
            Command::Direction(Direction::Fine),
        )
        .unwrap();
        assert!(s.master_bars[3].directions.is_empty());
        let verse = Marker {
            letter: "A".into(),
            text: "Verse".into(),
            ..Marker::default()
        };
        do_it(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::Section(Some(verse.clone())),
        )
        .unwrap();
        assert_eq!(s.master_bars[0].marker, Some(verse));
        do_it(&mut s, &mut h, at(0), None, Command::Tempo(96)).unwrap();
        assert_eq!(s.initial_tempo, 96.0);
        do_it(&mut s, &mut h, at(2), None, Command::Tempo(140)).unwrap();
        assert_eq!(BarState::at(&s, at(3)).unwrap().tempo, 140.0);
        assert_eq!(BarState::at(&s, at(1)).unwrap().tempo, 96.0);
        assert!(do_it(&mut s, &mut h, at(2), None, Command::Tempo(140)).is_none());
        do_it(&mut s, &mut h, at(0), None, Command::DoubleBar).unwrap();
        do_it(&mut s, &mut h, at(0), None, Command::FreeTime).unwrap();
        // A guitar's staff sounds an octave lower: its bass clef too.
        do_it(&mut s, &mut h, at(0), None, Command::Clef(Clef::Bass)).unwrap();
        assert!(
            s.tracks[0].staves[0]
                .bars
                .iter()
                .all(|b| b.clef == Clef::Bass8vb)
        );
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, score(4));
    }
}
