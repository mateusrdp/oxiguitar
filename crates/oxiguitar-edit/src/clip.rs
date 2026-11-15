// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Copy, cut and paste (M3 slice 4, EDIT-6), inside OxiGuitar only: the
//! clipboard is OxiGuitar's own (Mateus, 19 Oct 2026).
//!
//! As in Guitar Pro 8: a run of beats in one voice copies as beats, which
//! paste into any voice of any track; a selection across tracks copies
//! whole bars of every selected track, with what the bars share (time and
//! key signatures, repeats, sections, tempo), and pastes as bars. Between a
//! drum track and a pitched one only the rhythm carries over, as rests;
//! notes on strings the track hasn't are left out.
//!
//! Pasted beats take the place of what the caret's voice holds from the
//! caret on, flowing over bar lines as each bar's time signature allows (a
//! beat a bar line cuts goes on as tied pieces); bars are added at the end
//! as needed (#267; Mateus, 15 Nov 2026). Pasted bars go in as new bars.
//!
//! Copy, Cut and Paste Bars (#287) work on whole bars whatever the
//! selection covers of them: the bars it touches, of the caret's track or
//! of every track. Each copied track remembers where it came from, and
//! Paste Bars puts it back into that track, with rests in the tracks the
//! clipboard doesn't hold.

use std::sync::Arc;

use oxiguitar_model::{
    Bar, Beat, BeatKind, ChordDiagram, MasterBar, NotePlace, Ratio, Score, Voice,
};

use crate::entry::{grow_voices, retie, tidy};
use crate::fit::{Prefer, pieces, rests};
use crate::{Caret, Command, Edited, History, Region, Selection};

/// What was copied.
#[derive(Clone, Debug, PartialEq)]
pub enum Clip {
    /// A run of beats of one voice.
    Beats {
        /// The beats, in order.
        beats: Vec<Beat>,
        /// They came from a drum track.
        drums: bool,
        /// That track's chord diagrams, which the beats name by index.
        chords: Vec<ChordDiagram>,
    },
    /// Whole bars of one or more tracks, with what the bars share.
    Bars {
        /// What every track shares in those bars.
        masters: Vec<MasterBar>,
        /// Each track copied, in order.
        tracks: Vec<ClipTrack>,
    },
}

/// One track's bars in a [`Clip::Bars`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClipTrack {
    /// The track they came from.
    pub from: usize,
    /// It is a drum track.
    pub drums: bool,
    /// Its chord diagrams, which the beats name by index.
    pub chords: Vec<ChordDiagram>,
    /// Each staff's bars.
    pub staves: Vec<Vec<Bar>>,
}

/// A paste at the caret (Ctrl + V).
#[derive(Clone, Debug, PartialEq)]
pub struct Paste {
    /// The clipboard.
    pub clip: Arc<Clip>,
}

/// Paste Bars (Ctrl + Shift + V): copied bars as new bars.
#[derive(Clone, Debug, PartialEq)]
pub struct PasteBars {
    /// The clipboard; only [`Clip::Bars`] pastes.
    pub clip: Arc<Clip>,
    /// After the last bar the selection (or the caret) touches, instead of
    /// before the first.
    pub after: bool,
    /// Every copied track into the track it came from, instead of only the
    /// caret's track (which takes the copy of itself, else the first
    /// copied track).
    pub all_tracks: bool,
}

/// What `selection` (or, without one, the caret's beat) holds; `None` when
/// that is nothing (the place after a voice's last beat, say).
pub fn copy(score: &Score, c: Caret, selection: Option<Selection>) -> Option<Clip> {
    match selection {
        Some(Selection::Bars { tracks, bars }) => {
            let masters = score.master_bars.get(bars.0..=bars.1)?.to_vec();
            let tracks = (tracks.0..=tracks.1)
                .filter_map(|ti| {
                    let t = score.tracks.get(ti)?;
                    Some(ClipTrack {
                        from: ti,
                        drums: t.instrument.percussion,
                        chords: t.chords.clone(),
                        staves: t
                            .staves
                            .iter()
                            .filter_map(|s| s.bars.get(bars.0..=bars.1).map(<[Bar]>::to_vec))
                            .collect(),
                    })
                })
                .collect();
            Some(Clip::Bars { masters, tracks })
        }
        Some(Selection::Beats {
            track,
            staff,
            voice,
            from,
            to,
        }) => {
            let t = score.tracks.get(track)?;
            let st = t.staves.get(staff)?;
            let mut beats = Vec::new();
            for bar in from.0..=to.0 {
                let Some(v) = st.bars.get(bar).and_then(|b| b.voices.get(voice)) else {
                    continue;
                };
                let start = if bar == from.0 { from.1 } else { 0 };
                let end = if bar == to.0 {
                    (to.1 + 1).min(v.beats.len())
                } else {
                    v.beats.len()
                };
                beats.extend(
                    v.beats
                        .get(start..end.max(start))
                        .unwrap_or_default()
                        .iter()
                        .filter(|b| b.kind != BeatKind::Empty)
                        .cloned(),
                );
            }
            (!beats.is_empty()).then_some(Clip::Beats {
                beats,
                drums: t.instrument.percussion,
                chords: t.chords.clone(),
            })
        }
        None => {
            let t = score.tracks.get(c.track)?;
            let beat = c
                .voice_in(score)?
                .beats
                .get(c.beat)
                .filter(|b| b.kind != BeatKind::Empty)?;
            Some(Clip::Beats {
                beats: vec![beat.clone()],
                drums: t.instrument.percussion,
                chords: t.chords.clone(),
            })
        }
    }
}

/// Copy Bars (Ctrl + Shift + C): every bar the selection (or, without one,
/// the caret) touches, whole, of the caret's track or of every track.
pub fn copy_bars(
    score: &Score,
    c: Caret,
    selection: Option<Selection>,
    all_tracks: bool,
) -> Option<Clip> {
    let bars = selection.map_or((c.bar, c.bar), |s| s.bars());
    let tracks = if all_tracks {
        (0, score.tracks.len().checked_sub(1)?)
    } else {
        (c.track, c.track)
    };
    copy(score, c, Some(Selection::Bars { tracks, bars }))
}

/// Runs Cut, Cut Bars, Paste or Paste Bars; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    match command {
        Command::Cut | Command::CutBars => {
            let delete = match (command, selection) {
                (Command::CutBars, _) | (_, Some(Selection::Bars { .. })) => Command::DeleteBars,
                _ => Command::DeleteBeats,
            };
            let edited = crate::apply(score, history, c, selection, delete);
            if edited.is_some() {
                history.rename_last(command.label());
            }
            Some(edited)
        }
        Command::Paste(p) => Some(match &*p.clip {
            Clip::Beats {
                beats,
                drums,
                chords,
            } => paste_beats(score, history, c, (beats, *drums, chords), command.label()),
            Clip::Bars { masters, tracks } => {
                // From the caret's track down, as they were copied.
                let into = (0..score.tracks.len())
                    .map(|ti| ti.checked_sub(c.track).and_then(|k| tracks.get(k)))
                    .collect();
                insert_bars(score, history, c, c.bar, masters, into, command.label())
            }
        }),
        Command::PasteBars(p) => Some(paste_bars(score, history, c, selection, p, command.label())),
        _ => None,
    }
}

/// A beat as it goes into a track: a rest of its length between drums and
/// pitched tracks, without the notes on strings the track hasn't, and
/// without notes placed by pitch on a track with strings or by string on
/// one without (a .gp file would read them back the other way). Grace
/// beats don't cross between drums and pitches.
fn fit(b: &Beat, from_drums: bool, to_drums: bool, strings: usize) -> Option<Beat> {
    if from_drums != to_drums {
        return b.grace.is_none().then(|| Beat {
            kind: BeatKind::Rest,
            duration: b.duration.clone(),
            ..Beat::default()
        });
    }
    let mut b = b.clone();
    b.notes.retain(|n| match n.place {
        NotePlace::Fretted { string, .. } => usize::from(string) <= strings,
        NotePlace::Pitched(_) => strings == 0 || to_drums,
    });
    if b.kind == BeatKind::Notes && b.notes.is_empty() {
        b = Beat {
            kind: BeatKind::Rest,
            duration: b.duration,
            grace: b.grace,
            ..Beat::default()
        };
    }
    Some(b)
}

/// Beats' chords, named by index in `from`, renamed by index in `to`:
/// the same diagram where `to` has it, else added to it.
fn map_chords<'a>(
    beats: impl Iterator<Item = &'a mut Beat>,
    from: &[ChordDiagram],
    to: &mut Vec<ChordDiagram>,
) {
    for b in beats {
        let Some(i) = b.chord else { continue };
        b.chord = from.get(i).map(|d| match to.iter().position(|x| x == d) {
            Some(j) => j,
            None => {
                to.push(d.clone());
                to.len() - 1
            }
        });
    }
}

/// How long a beat lasts, a grace beat nothing.
fn length(b: &Beat) -> Ratio {
    if b.grace.is_some() {
        return Ratio::ZERO;
    }
    b.duration.length().unwrap_or(Ratio::ZERO)
}

/// How long beats last together.
fn total(beats: &[Beat]) -> Option<Ratio> {
    beats
        .iter()
        .map(length)
        .try_fold(Ratio::ZERO, Ratio::checked_add)
}

/// Pasted beats poured into one voice of a staff, bar by bar from the
/// caret's: each bar filled to its time signature, a beat a bar line cuts
/// going on as tied pieces.
struct Flow<'a> {
    score: &'a Score,
    track: usize,
    staff: usize,
    voice: usize,
    /// The caret's bar.
    first: usize,
    /// The bars filled, from the caret's.
    done: Vec<Bar>,
    /// The bar being filled, its other voices as they were.
    bar: Bar,
    /// The voice's beats in it so far, and how long they last.
    beats: Vec<Beat>,
    pos: Ratio,
    /// How long the bar lasts.
    cap: Ratio,
    /// Where in the bar the pasted music started, and what the voice held
    /// from there on.
    start: Ratio,
    old: Vec<Beat>,
}

impl<'a> Flow<'a> {
    /// Ready to pour at the caret.
    fn at(score: &'a Score, c: Caret) -> Option<Flow<'a>> {
        let mut f = Flow {
            score,
            track: c.track,
            staff: c.staff,
            voice: c.voice,
            first: c.bar,
            done: Vec::new(),
            bar: empty(Default::default()),
            beats: Vec::new(),
            pos: Ratio::ZERO,
            cap: Ratio::ZERO,
            start: Ratio::ZERO,
            old: Vec::new(),
        };
        f.bar = f.load(c.bar)?;
        f.cap = f.capacity(c.bar)?;
        let v = f.bar.voices.get_mut(c.voice)?;
        if !v.is_empty() {
            f.old = v.beats.split_off(c.beat.min(v.beats.len()));
            f.beats = std::mem::take(&mut v.beats);
        }
        v.beats.clear();
        f.pos = total(&f.beats)?;
        f.start = f.pos;
        Some(f)
    }

    /// How long bar `k` lasts: as its time signature says, or the last
    /// bar's past the end of the song.
    fn capacity(&self, k: usize) -> Option<Ratio> {
        let m = self
            .score
            .master_bars
            .get(k)
            .or(self.score.master_bars.last())?;
        m.time_signature
            .bar_length()
            .filter(|len| *len > Ratio::ZERO)
    }

    /// Bar `k` of the staff (an empty one past the end), with the voice.
    fn load(&self, k: usize) -> Option<Bar> {
        let st = self.score.tracks.get(self.track)?.staves.get(self.staff)?;
        let mut bar = match st.bars.get(k) {
            Some(b) => b.clone(),
            None => empty(st.bars.last().map_or_else(Default::default, |b| b.clef)),
        };
        grow_voices(&mut bar, self.voice);
        Some(bar)
    }

    /// The bar being filled done; the next one started, its voice wholly
    /// replaced.
    fn next(&mut self) -> Option<()> {
        let k = self.first + self.done.len() + 1;
        let mut bar = self.load(k)?;
        let v = bar.voices.get_mut(self.voice)?;
        let old = if v.is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut v.beats)
        };
        v.beats.clear();
        let mut full = std::mem::replace(&mut self.bar, bar);
        full.voices.get_mut(self.voice)?.beats = std::mem::take(&mut self.beats);
        self.done.push(full);
        self.cap = self.capacity(k)?;
        self.pos = Ratio::ZERO;
        self.start = Ratio::ZERO;
        self.old = old;
        Some(())
    }

    /// One pasted beat in, cut into tied pieces where bar lines fall in it.
    fn pour(&mut self, b: &Beat) -> Option<()> {
        let len = length(b);
        if len == Ratio::ZERO {
            // A grace beat goes with the beat after it.
            if self.pos >= self.cap {
                self.next()?;
            }
            self.beats.push(b.clone());
            return Some(());
        }
        let mut left = len;
        let mut whole = true;
        loop {
            if self.pos >= self.cap {
                self.next()?;
            }
            let room = self.cap.checked_sub(self.pos)?;
            if left <= room {
                if whole {
                    self.beats.push(b.clone());
                } else {
                    self.beats
                        .extend(pieces(b, left, Prefer::Own, false, true)?);
                }
                self.pos = self.pos.checked_add(left)?;
                return Some(());
            }
            self.beats
                .extend(pieces(b, room, Prefer::Own, whole, false)?);
            self.pos = self.cap;
            left = left.checked_sub(room)?;
            whole = false;
        }
    }

    /// The last bar closed: what the voice held after the pasted music
    /// stays, less as much as the paste took the place of (what of a beat
    /// it cuts into becomes rests). The bars, and the caret's place after
    /// the pasted beats: in the last bar, or at the start of the bar after
    /// when they fill it (`None`).
    fn finish(mut self) -> Option<(Vec<Bar>, Option<usize>)> {
        let beat = (self.pos < self.cap).then_some(self.beats.len());
        let wanted = self.pos.checked_sub(self.start)?;
        let mut removed = Ratio::ZERO;
        let mut taken = 0;
        while removed < wanted && taken < self.old.len() {
            removed = removed.checked_add(length(self.old.get(taken)?))?;
            taken += 1;
        }
        self.old.drain(..taken);
        if removed > wanted {
            self.beats
                .extend(rests(removed.checked_sub(wanted)?).unwrap_or_default());
        }
        self.beats.append(&mut self.old);
        self.bar.voices.get_mut(self.voice)?.beats = self.beats;
        self.done.push(self.bar);
        self.done.iter_mut().for_each(tidy);
        Some((self.done, beat))
    }
}

/// Beats pasted over the caret's voice from the caret on (#267).
fn paste_beats(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    (beats, from_drums, from_chords): (&[Beat], bool, &[ChordDiagram]),
    label: &'static str,
) -> Option<Edited> {
    let track = score.tracks.get(c.track)?;
    let to_drums = track.instrument.percussion;
    let strings = track.staves.get(c.staff)?.string_count();
    let mut pasted: Vec<Beat> = beats
        .iter()
        .filter_map(|b| fit(b, from_drums, to_drums, strings))
        .collect();
    if pasted.is_empty() {
        return None;
    }
    let mut chords = track.chords.clone();
    map_chords(pasted.iter_mut(), from_chords, &mut chords);
    let new_chords = (chords != track.chords).then_some(chords);
    let mut flow = Flow::at(score, c)?;
    for b in &pasted {
        flow.pour(b)?;
    }
    let (mut bars, beat) = flow.finish()?;
    let (t, s, first) = (c.track, c.staff, c.bar);
    let end = first + bars.len();
    let count = score.bar_count();
    if let Some(old) = score
        .tracks
        .get(t)
        .and_then(|tr| tr.staves.get(s))
        .and_then(|st| st.bars.get(first..end.min(count)))
    {
        crate::effects::unmark_similes(old, &mut bars);
    }
    let regions = if end > count {
        vec![Region::Whole]
    } else {
        let mut r = vec![Region::Bars {
            track: t,
            staff: s,
            bars: first..count,
        }];
        if new_chords.is_some() {
            r.push(Region::Track(t));
        }
        r
    };
    let touched = history.record(score, label, regions, |score| {
        grow(score, end);
        if let (Some(tr), Some(chords)) = (score.tracks.get_mut(t), new_chords) {
            tr.chords = chords;
        }
        if let Some(st) = score.tracks.get_mut(t).and_then(|t| t.staves.get_mut(s)) {
            if let Some(dst) = st.bars.get_mut(first..end) {
                dst.clone_from_slice(&bars);
            }
            retie(st, first);
        }
    });
    let (bar, beat) = match beat {
        Some(beat) => (end - 1, beat),
        None if end < score.bar_count() => (end, 0),
        None => (end - 1, usize::MAX),
    };
    let caret = Caret { bar, beat, ..c };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(c),
    })
}

/// Bars added at the end until the score has `n`, in the last bar's time
/// and key, empty in every staff.
fn grow(s: &mut Score, n: usize) {
    while s.master_bars.len() < n {
        let last = s.master_bars.last().cloned().unwrap_or_default();
        s.master_bars.push(MasterBar {
            time_signature: last.time_signature,
            key_signature: last.key_signature,
            ..MasterBar::default()
        });
        for st in s.tracks.iter_mut().flat_map(|t| &mut t.staves) {
            let clef = st.bars.last().map_or_else(Default::default, |b| b.clef);
            st.bars.push(empty(clef));
        }
    }
}

/// Paste Bars (#287): the copied bars as new bars before or after the
/// bars the selection (or the caret) touches.
fn paste_bars(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    p: &PasteBars,
    label: &'static str,
) -> Option<Edited> {
    let Clip::Bars { masters, tracks } = &*p.clip else {
        return None;
    };
    let (first, last) = selection.map_or((c.bar, c.bar), |s| s.bars());
    let at = if p.after { last + 1 } else { first };
    let own = |ti: usize| tracks.iter().find(|k| k.from == ti);
    let into = (0..score.tracks.len())
        .map(|ti| match (p.all_tracks, ti == c.track) {
            (true, _) => own(ti),
            (false, true) => own(ti).or(tracks.first()),
            (false, false) => None,
        })
        .collect();
    insert_bars(score, history, c, at, masters, into, label)
}

/// `masters` put in as new bars before bar `at` (the end, at the bar
/// count): each track gets the copied bars `into` gives it, the others
/// bars of rests. The caret goes to the bar after them.
fn insert_bars(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    at: usize,
    masters: &[MasterBar],
    into: Vec<Option<&ClipTrack>>,
    label: &'static str,
) -> Option<Edited> {
    let n = masters.len();
    if n == 0 || at > score.bar_count() {
        return None;
    }
    // Each track's new bars, staff by staff, and its chord diagrams.
    let mut new_bars: Vec<Vec<Vec<Bar>>> = Vec::with_capacity(score.tracks.len());
    let mut new_chords: Vec<Vec<ChordDiagram>> = Vec::with_capacity(score.tracks.len());
    for (ti, t) in score.tracks.iter().enumerate() {
        let clip = into.get(ti).copied().flatten();
        let to_drums = t.instrument.percussion;
        let mut chords = t.chords.clone();
        let staves = t
            .staves
            .iter()
            .enumerate()
            .map(|(si, st)| {
                let clef = st
                    .bars
                    .get(at)
                    .or(st.bars.last())
                    .map_or_else(Default::default, |b| b.clef);
                let strings = st.string_count();
                match clip.and_then(|k| Some((k, k.staves.get(si)?))) {
                    Some((clip, bars)) => {
                        let mut bars: Vec<Bar> = bars
                            .iter()
                            .map(|b| Bar {
                                // A pasted bar holds its music: no longer
                                // a repeat of the bar before it.
                                simile: None,
                                // A bar keeps its clef where the track
                                // sounds in the same octave (Guitar Pro
                                // holds the octave per track); else it
                                // takes the track's, and drums theirs.
                                clef: if !to_drums
                                    && !clip.drums
                                    && low_octave(b.clef) == low_octave(clef)
                                {
                                    b.clef
                                } else {
                                    clef
                                },
                                voices: b
                                    .voices
                                    .iter()
                                    .map(|v| Voice {
                                        beats: v
                                            .beats
                                            .iter()
                                            .filter_map(|x| fit(x, clip.drums, to_drums, strings))
                                            .collect(),
                                    })
                                    .collect(),
                            })
                            .collect();
                        map_chords(
                            bars.iter_mut()
                                .flat_map(|b| &mut b.voices)
                                .flat_map(|v| &mut v.beats),
                            &clip.chords,
                            &mut chords,
                        );
                        bars
                    }
                    None => masters.iter().map(|m| resting(m, clef)).collect(),
                }
            })
            .collect();
        new_bars.push(staves);
        new_chords.push(chords);
    }
    let touched = history.record(score, label, vec![Region::Whole], |s| {
        s.master_bars.splice(at..at, masters.iter().cloned());
        for ((t, chords), bars) in s.tracks.iter_mut().zip(new_chords).zip(new_bars) {
            t.chords = chords;
            for (st, mine) in t.staves.iter_mut().zip(bars) {
                st.bars.splice(at..at, mine);
                retie(st, at);
            }
        }
    });
    let caret = Caret {
        bar: (at + n).min(score.bar_count().saturating_sub(1)),
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(c),
    })
}

/// A clef sounding an octave below what it shows (a guitar's or bass's).
fn low_octave(c: oxiguitar_model::Clef) -> bool {
    matches!(
        c,
        oxiguitar_model::Clef::Treble8vb | oxiguitar_model::Clef::Bass8vb
    )
}

/// An empty bar in `clef`.
fn empty(clef: oxiguitar_model::Clef) -> Bar {
    Bar {
        clef,
        voices: vec![Voice::default()],
        simile: None,
    }
}

/// A bar of rests as long as `m`'s time signature, in `clef` (an empty
/// bar where no rests fill it).
fn resting(m: &MasterBar, clef: oxiguitar_model::Clef) -> Bar {
    Bar {
        voices: vec![Voice {
            beats: m
                .time_signature
                .bar_length()
                .and_then(rests)
                .unwrap_or_default(),
        }],
        ..empty(clef)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply as run, new_score};
    use oxiguitar_model::{Instrument, NoteValue, Track};

    fn at(bar: usize, beat: usize) -> Caret {
        Caret {
            bar,
            beat,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    /// One guitar, `bars` bars of four quarter notes on string 1, frets
    /// 0, 1, 2, 3 (then 4, 5, 6, 7 …).
    fn score(bars: usize) -> Score {
        let mut s = new_score();
        let mut h = History::new();
        for bar in 0..bars {
            if bar > 0 {
                run(&mut s, &mut h, at(bar - 1, 4), None, Command::AppendBar).unwrap();
            }
            for i in 0..4 {
                let fret = ((bar * 4 + i) % 24) as u8;
                run(
                    &mut s,
                    &mut h,
                    at(bar, i),
                    None,
                    Command::Fret { fret, again: false },
                )
                .unwrap();
            }
        }
        s
    }

    /// `s` with a drum track after the guitar, holding the same bars.
    fn with_drums(mut s: Score) -> Score {
        let mut drums = s.tracks[0].clone();
        drums.instrument = Instrument {
            midi_program: 0,
            percussion: true,
        };
        s.tracks.push(Track {
            name: "Drums".into(),
            ..drums
        });
        s
    }

    fn frets(s: &Score, track: usize, bar: usize) -> Vec<Option<u8>> {
        s.tracks[track].staves[0].bars[bar].voices[0]
            .beats
            .iter()
            .map(|b| match b.notes.first().map(|n| n.place) {
                Some(NotePlace::Fretted { fret, .. }) => Some(fret),
                _ => None,
            })
            .collect()
    }

    fn paste(clip: Clip) -> Command {
        Command::Paste(Paste {
            clip: Arc::new(clip),
        })
    }

    fn paste_bars(clip: Clip, after: bool, all_tracks: bool) -> Command {
        Command::PasteBars(PasteBars {
            clip: Arc::new(clip),
            after,
            all_tracks,
        })
    }

    fn adds_up(s: &Score) -> bool {
        crate::BarCheck::of(s).iter().count() == 0
    }

    #[test]
    fn beats_flow_over_bar_lines_and_replace() {
        let mut s = score(3);
        let mut h = History::new();
        // Six beats from bar 1's second to bar 2's third.
        let sel = Selection::between(at(0, 1), at(1, 2));
        let clip = copy(&s, at(0, 0), Some(sel)).unwrap();
        let e = run(&mut s, &mut h, at(1, 2), None, paste(clip.clone())).unwrap();
        assert_eq!(frets(&s, 0, 1), [4, 5, 1, 2].map(Some));
        assert_eq!(frets(&s, 0, 2), [3, 4, 5, 6].map(Some));
        assert_eq!((e.caret.bar, e.caret.beat), (2, 3), "the last bar's end");
        assert_eq!(s.bar_count(), 3);
        assert!(adds_up(&s));
        assert_eq!(h.undo_label(), Some("Paste"));
        h.undo(&mut s);
        // Past the end of the song: a bar is added for what is left.
        run(&mut s, &mut h, at(2, 1), None, paste(clip.clone())).unwrap();
        assert_eq!(s.bar_count(), 4);
        s.check_structure().unwrap();
        assert_eq!(frets(&s, 0, 2), [8, 1, 2, 3].map(Some));
        assert_eq!(frets(&s, 0, 3), [4, 5, 6].map(Some));
        h.undo(&mut s);
        assert_eq!(s.bar_count(), 3);
        // Inside one bar it replaces as much as it lasts.
        let one = copy(&s, at(0, 2), None).unwrap();
        run(&mut s, &mut h, at(1, 1), None, paste(one)).unwrap();
        assert_eq!(frets(&s, 0, 1), [4, 2, 6, 7].map(Some));
    }

    #[test]
    fn whole_bars_of_beats_land_bar_for_bar() {
        // #267: eight bars copied into eight empty ones.
        let mut s = score(8);
        let mut h = History::new();
        for _ in 0..8 {
            let last = s.bar_count() - 1;
            run(&mut s, &mut h, at(last, 4), None, Command::AppendBar).unwrap();
        }
        let sel = Selection::between(at(0, 0), at(7, 3));
        let clip = copy(&s, at(0, 0), Some(sel)).unwrap();
        run(&mut s, &mut h, at(8, 0), None, paste(clip)).unwrap();
        assert_eq!(s.bar_count(), 16);
        for bar in 0..8 {
            assert_eq!(frets(&s, 0, bar + 8), frets(&s, 0, bar), "bar {bar}");
        }
        assert!(adds_up(&s));
    }

    #[test]
    fn a_beat_a_bar_line_cuts_goes_on_tied() {
        let mut s = score(2);
        let mut h = History::new();
        let mut half = copy(&s, at(0, 0), None).unwrap();
        if let Clip::Beats { beats, .. } = &mut half {
            beats[0].duration.value = NoteValue::Half;
        }
        run(&mut s, &mut h, at(0, 3), None, paste(half)).unwrap();
        let first = &s.tracks[0].staves[0].bars[0].voices[0].beats;
        let next = &s.tracks[0].staves[0].bars[1].voices[0].beats;
        assert_eq!(first.len(), 4);
        assert_eq!(first[3].duration.value, NoteValue::Quarter);
        assert!(!first[3].notes[0].tie);
        assert_eq!(next[0].duration.value, NoteValue::Quarter);
        assert!(next[0].notes[0].tie, "the second piece is tied");
        assert_eq!(frets(&s, 0, 1), [0, 5, 6, 7].map(Some));
        assert!(adds_up(&s));
    }

    #[test]
    fn bars_pasted_with_ctrl_v_go_in_before_the_caret() {
        let mut s = with_drums(score(3));
        s.master_bars[1].repeat_start = true;
        let mut h = History::new();
        let all = Selection::Bars {
            tracks: (0, 1),
            bars: (1, 1),
        };
        let clip = copy(&s, at(1, 0), Some(all)).unwrap();
        run(&mut s, &mut h, at(0, 0), None, paste(clip)).unwrap();
        assert_eq!(s.bar_count(), 4);
        s.check_structure().unwrap();
        assert!(s.master_bars[0].repeat_start && s.master_bars[2].repeat_start);
        assert_eq!(frets(&s, 0, 0), [4, 5, 6, 7].map(Some));
        assert_eq!(frets(&s, 0, 1), [0, 1, 2, 3].map(Some));
    }

    #[test]
    fn copy_bars_takes_every_bar_the_selection_touches() {
        let s = with_drums(score(3));
        // From bar 1's third beat to bar 2's second: bars 1 and 2, whole.
        let sel = Selection::between(at(0, 2), at(1, 1));
        let Some(Clip::Bars { masters, tracks }) = copy_bars(&s, at(1, 1), Some(sel), false) else {
            panic!("bars")
        };
        assert_eq!(masters.len(), 2);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].from, 0);
        assert_eq!(tracks[0].staves[0][0].voices[0].beats.len(), 4);
        let Some(Clip::Bars { tracks, .. }) = copy_bars(&s, at(2, 0), None, true) else {
            panic!("bars")
        };
        assert_eq!(tracks.iter().map(|t| t.from).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn paste_bars_before_and_after_into_their_own_tracks() {
        let mut s = with_drums(score(3));
        let mut h = History::new();
        let drums = Caret {
            track: 1,
            ..at(2, 0)
        };
        // The drums' bar 3 only, pasted into every track from the guitar:
        // it goes back to the drums; the guitar gets a bar of rests.
        let clip = copy_bars(&s, drums, None, false).unwrap();
        let sel = Selection::between(at(0, 1), at(1, 0));
        let e = run(
            &mut s,
            &mut h,
            at(1, 0),
            Some(sel),
            paste_bars(clip.clone(), true, true),
        )
        .unwrap();
        assert_eq!(h.undo_label(), Some("Paste Bars"));
        assert_eq!(s.bar_count(), 4);
        s.check_structure().unwrap();
        assert_eq!(frets(&s, 1, 2), [8, 9, 10, 11].map(Some), "after bar 2");
        let rests = &s.tracks[0].staves[0].bars[2].voices[0].beats;
        assert_eq!(rests.len(), 1);
        assert_eq!(rests[0].kind, BeatKind::Rest);
        assert_eq!(rests[0].duration.value, NoteValue::Whole);
        assert_eq!(frets(&s, 0, 3), [8, 9, 10, 11].map(Some));
        assert_eq!(e.caret.bar, 3);
        assert!(adds_up(&s));
        h.undo(&mut s);
        // Both tracks' bar 3 into only the caret's track, before its bar:
        // the guitar takes its own copy, the drums get rests.
        let both = copy_bars(&s, at(2, 0), None, true).unwrap();
        assert_ne!(both, clip);
        run(
            &mut s,
            &mut h,
            at(0, 0),
            None,
            paste_bars(both, false, false),
        )
        .unwrap();
        assert_eq!(frets(&s, 0, 0), [8, 9, 10, 11].map(Some));
        assert_eq!(
            s.tracks[1].staves[0].bars[0].voices[0].beats[0].kind,
            BeatKind::Rest
        );
        assert_eq!(frets(&s, 0, 1), [0, 1, 2, 3].map(Some));
        // Beats don't paste as bars.
        let beats = copy(&s, at(1, 0), None).unwrap();
        assert!(
            run(
                &mut s,
                &mut h,
                at(0, 0),
                None,
                paste_bars(beats, false, true)
            )
            .is_none()
        );
    }

    #[test]
    fn cuts_are_named_and_undone() {
        let mut s = with_drums(score(3));
        let before = s.clone();
        let mut h = History::new();
        let sel = Selection::Bars {
            tracks: (0, 0),
            bars: (0, 0),
        };
        run(&mut s, &mut h, at(0, 0), Some(sel), Command::Cut).unwrap();
        assert_eq!(s.bar_count(), 2);
        assert_eq!(h.undo_label(), Some("Cut"));
        run(&mut s, &mut h, at(0, 0), None, Command::Cut).unwrap();
        assert_eq!(frets(&s, 0, 0), [5, 6, 7].map(Some));
        // Cut Bars: the bars a run of beats touches, from every track.
        let sel = Selection::between(at(0, 2), at(1, 0));
        h.undo(&mut s);
        h.undo(&mut s);
        run(&mut s, &mut h, at(1, 0), Some(sel), Command::CutBars).unwrap();
        assert_eq!(h.undo_label(), Some("Cut Bars"));
        assert_eq!(s.bar_count(), 1);
        assert_eq!(frets(&s, 1, 0), [8, 9, 10, 11].map(Some));
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, before);
        assert!(
            copy(&s, at(0, 4), None).is_none(),
            "the place after the last beat"
        );
    }

    #[test]
    fn random_clipboard_edits_keep_the_score_whole_and_undo() {
        let mut s = with_drums(score(6));
        s.master_bars[3].time_signature = oxiguitar_model::TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        let before = s.clone();
        let mut h = History::new();
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut below = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n.max(1) as u64) as usize
        };
        let mut clip = None;
        let mut done = [0usize; 3];
        for _ in 0..600 {
            let track = below(2);
            let bar = below(s.bar_count());
            let c = Caret {
                track,
                bar,
                beat: below(5),
                ..at(0, 0)
            }
            .clamp(&s)
            .unwrap();
            let head = Caret {
                bar: (bar + below(3)).min(s.bar_count() - 1),
                beat: below(5),
                ..c
            }
            .clamp(&s)
            .unwrap();
            let sel = match below(3) {
                0 => None,
                1 => Some(Selection::between(c, head)),
                _ => Some(Selection::between(
                    c,
                    Caret {
                        track: 1 - track,
                        ..head
                    },
                )),
            };
            match below(6) {
                0 => clip = copy(&s, c, sel).or(clip),
                1 => clip = copy_bars(&s, c, sel, below(2) == 0).or(clip),
                2 => {
                    done[0] += usize::from(run(&mut s, &mut h, c, sel, Command::CutBars).is_some());
                }
                3 => {
                    if let Some(k) = clip.clone() {
                        done[1] += usize::from(run(&mut s, &mut h, c, sel, paste(k)).is_some());
                    }
                }
                4 => {
                    if let Some(k) = clip.clone() {
                        let p = paste_bars(k, below(2) == 0, below(2) == 0);
                        done[2] += usize::from(run(&mut s, &mut h, c, sel, p).is_some());
                    }
                }
                _ => {
                    h.undo(&mut s);
                }
            }
            s.check_structure().unwrap();
            if s.bar_count() > 40 {
                while s.bar_count() > 20 && h.undo(&mut s).is_some() {}
            }
        }
        assert!(
            done.iter().all(|&n| n > 20),
            "cut, paste, paste bars: {done:?}"
        );
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, before);
    }
}
