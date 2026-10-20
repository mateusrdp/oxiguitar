// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Copy, cut and paste (M3 slice 4, EDIT-6), inside OxiGuitar only: the
//! clipboard is OxiGuitar's own (Mateus, 19 Oct 2026).
//!
//! As in Guitar Pro 8: a run of beats in one voice copies as beats, which
//! paste into any voice of any track; a selection across tracks copies
//! whole bars of every selected track, with what the bars share (time and
//! key signatures, repeats, sections, tempo), and pastes as bars. Paste
//! inserts at the caret; Special Paste can paste several times, or replace
//! what is there instead. Between a drum track and a pitched one only the
//! rhythm carries over, as rests; notes on strings the track hasn't are
//! left out.

use std::sync::Arc;

use oxiguitar_model::{
    Bar, Beat, BeatKind, ChordDiagram, MasterBar, NotePlace, Ratio, Score, Voice,
};

use crate::entry::{grow_voices, rests_for, retie, tidy};
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

/// A paste: what, how many times, and whether it replaces what is there
/// instead of going in before it.
#[derive(Clone, Debug, PartialEq)]
pub struct Paste {
    /// The clipboard.
    pub clip: Arc<Clip>,
    /// How many times, from 1.
    pub times: u8,
    /// Beats: the pasted beats take the place of as much music from the
    /// caret on as they last. Bars: they take the place of the bars from
    /// the caret's on (bars are added at the end as needed).
    pub replace: bool,
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

/// Runs Cut or Paste; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    match command {
        Command::Cut => {
            let delete = match selection {
                Some(Selection::Bars { .. }) => Command::DeleteBars,
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
            } => paste_beats(
                score,
                history,
                c,
                (beats, *drums, chords),
                p,
                command.label(),
            ),
            Clip::Bars { masters, tracks } => {
                paste_bars(score, history, c, masters, tracks, p, command.label())
            }
        }),
        _ => None,
    }
}

/// A beat as it goes into a track: a rest of its length between drums and
/// pitched tracks, without the notes on strings the track hasn't. Grace
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
        NotePlace::Pitched(_) => true,
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

fn paste_beats(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    (beats, from_drums, from_chords): (&[Beat], bool, &[ChordDiagram]),
    p: &Paste,
    label: &'static str,
) -> Option<Edited> {
    let track = score.tracks.get(c.track)?;
    let to_drums = track.instrument.percussion;
    let staff = track.staves.get(c.staff)?;
    let strings = staff.string_count();
    let one: Vec<Beat> = beats
        .iter()
        .filter_map(|b| fit(b, from_drums, to_drums, strings))
        .collect();
    let mut pasted: Vec<Beat> = (0..p.times.max(1))
        .flat_map(|_| one.iter().cloned())
        .collect();
    if pasted.is_empty() {
        return None;
    }
    let mut chords = track.chords.clone();
    map_chords(pasted.iter_mut(), from_chords, &mut chords);
    let new_chords = (chords != track.chords).then_some(chords);
    let mut bar = staff.bars.get(c.bar)?.clone();
    grow_voices(&mut bar, c.voice);
    let v: &mut Voice = bar.voices.get_mut(c.voice)?;
    if v.is_empty() {
        v.beats.clear();
    }
    let at = c.beat.min(v.beats.len());
    let mut tail = v.beats.split_off(at);
    if p.replace {
        // Out goes as much as the pasted beats last; what of the last
        // beat out they don't cover becomes rests.
        let wanted = pasted
            .iter()
            .map(length)
            .try_fold(Ratio::ZERO, Ratio::checked_add)?;
        let mut removed = Ratio::ZERO;
        let mut taken = 0;
        while removed < wanted && taken < tail.len() {
            removed = removed.checked_add(length(tail.get(taken)?))?;
            taken += 1;
        }
        tail.drain(..taken);
        if removed > wanted {
            let rests = rests_for(removed.checked_sub(wanted)?).unwrap_or_default();
            for (i, d) in rests.into_iter().enumerate() {
                tail.insert(
                    i,
                    Beat {
                        kind: BeatKind::Rest,
                        duration: d,
                        ..Beat::default()
                    },
                );
            }
        }
    }
    let count = pasted.len();
    v.beats.extend(pasted);
    v.beats.extend(tail);
    tidy(&mut bar);
    let (t, s, first) = (c.track, c.staff, c.bar);
    let end = score.bar_count();
    let mut regions = vec![Region::Bars {
        track: t,
        staff: s,
        bars: first..end,
    }];
    if new_chords.is_some() {
        regions.push(Region::Track(t));
    }
    let touched = history.record(score, label, regions, |score| {
        if let (Some(tr), Some(chords)) = (score.tracks.get_mut(t), new_chords) {
            tr.chords = chords;
        }
        if let Some(st) = score.tracks.get_mut(t).and_then(|t| t.staves.get_mut(s)) {
            if let Some(b) = st.bars.get_mut(first) {
                *b = bar;
            }
            retie(st, first);
        }
    });
    let caret = Caret {
        beat: at + count,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(c),
    })
}

fn paste_bars(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    masters: &[MasterBar],
    tracks: &[ClipTrack],
    p: &Paste,
    label: &'static str,
) -> Option<Edited> {
    let times = usize::from(p.times.max(1));
    let n = masters.len().checked_mul(times)?;
    if n == 0 {
        return None;
    }
    let first = c.bar;
    let new_masters: Vec<MasterBar> = (0..times).flat_map(|_| masters.iter().cloned()).collect();
    // Each track's new bars, staff by staff: `None` for a track the clip
    // doesn't reach.
    let mut new_bars: Vec<Option<Vec<Vec<Bar>>>> = Vec::with_capacity(score.tracks.len());
    let mut new_chords: Vec<Vec<ChordDiagram>> = Vec::with_capacity(score.tracks.len());
    for (ti, t) in score.tracks.iter().enumerate() {
        let clip = ti.checked_sub(c.track).and_then(|k| tracks.get(k));
        let mut chords = t.chords.clone();
        new_bars.push(clip.map(|clip| {
            let to_drums = t.instrument.percussion;
            t.staves
                .iter()
                .enumerate()
                .map(|(si, st)| {
                    let clef = st
                        .bars
                        .get(first)
                        .or(st.bars.last())
                        .map_or_else(Default::default, |b| b.clef);
                    let strings = st.string_count();
                    let one: Vec<Bar> = match clip.staves.get(si) {
                        Some(bars) => bars
                            .iter()
                            .map(|b| Bar {
                                // A bar keeps its clef in its own track.
                                clef: if clip.from == ti { b.clef } else { clef },
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
                            .collect(),
                        None => (0..masters.len()).map(|_| empty(clef)).collect(),
                    };
                    let mut all: Vec<Bar> = (0..times).flat_map(|_| one.iter().cloned()).collect();
                    map_chords(
                        all.iter_mut()
                            .flat_map(|b| &mut b.voices)
                            .flat_map(|v| &mut v.beats),
                        &clip.chords,
                        &mut chords,
                    );
                    all
                })
                .collect()
        }));
        new_chords.push(chords);
    }
    let touched = history.record(score, label, vec![Region::Whole], |s| {
        if p.replace {
            // Bars at the end for what goes past it, as the last.
            while s.master_bars.len() < first + n {
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
            if let Some(dst) = s.master_bars.get_mut(first..first + n) {
                dst.clone_from_slice(&new_masters);
            }
        } else {
            s.master_bars
                .splice(first..first, new_masters.iter().cloned());
        }
        for (t, chords) in s.tracks.iter_mut().zip(&new_chords) {
            t.chords.clone_from(chords);
        }
        for (t, bars) in s.tracks.iter_mut().zip(&new_bars) {
            for (si, st) in t.staves.iter_mut().enumerate() {
                let clef = st
                    .bars
                    .get(first)
                    .or(st.bars.last())
                    .map_or_else(Default::default, |b| b.clef);
                let mine = bars.as_ref().and_then(|b| b.get(si));
                match (p.replace, mine) {
                    (true, Some(mine)) => {
                        if let Some(dst) = st.bars.get_mut(first..first + n) {
                            dst.clone_from_slice(mine);
                        }
                    }
                    (true, None) => {}
                    (false, Some(mine)) => {
                        st.bars.splice(first..first, mine.iter().cloned());
                    }
                    (false, None) => {
                        st.bars.splice(first..first, (0..n).map(|_| empty(clef)));
                    }
                }
                retie(st, first);
            }
        }
    });
    let caret = Caret {
        bar: (first + n).min(score.bar_count().saturating_sub(1)),
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).unwrap_or(c),
    })
}

/// An empty bar in `clef`.
fn empty(clef: oxiguitar_model::Clef) -> Bar {
    Bar {
        clef,
        voices: vec![Voice::default()],
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
                let fret = (bar * 4 + i) as u8;
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

    fn paste(clip: Clip, times: u8, replace: bool) -> Command {
        Command::Paste(Paste {
            clip: Arc::new(clip),
            times,
            replace,
        })
    }

    #[test]
    fn beats_insert_and_replace() {
        let mut s = score(2);
        let mut h = History::new();
        let sel = Selection::between(at(0, 1), at(0, 2));
        let clip = copy(&s, at(0, 2), Some(sel)).unwrap();
        let e = run(
            &mut s,
            &mut h,
            at(1, 1),
            None,
            paste(clip.clone(), 2, false),
        )
        .unwrap();
        assert_eq!(frets(&s, 0, 1), [4, 1, 2, 1, 2, 5, 6, 7].map(Some));
        assert_eq!(e.caret.beat, 5);
        h.undo(&mut s);
        // Replacing two quarters by an eighth: the rest of the second
        // quarter stays, as a rest.
        let mut short = clip.clone();
        if let Clip::Beats { beats, .. } = &mut short {
            beats.truncate(1);
            beats[0].duration.value = NoteValue::Eighth;
        }
        run(&mut s, &mut h, at(1, 1), None, paste(short, 1, true)).unwrap();
        assert_eq!(frets(&s, 0, 1), [Some(4), Some(1), None, Some(6), Some(7)]);
        assert_eq!(crate::BarCheck::of(&s).iter().count(), 0, "every bar adds up");
    }

    #[test]
    fn bars_across_tracks_insert_and_replace() {
        let mut s = score(3);
        let mut drums = s.tracks[0].clone();
        drums.instrument = Instrument {
            midi_program: 0,
            percussion: true,
        };
        s.tracks.push(Track {
            name: "Drums".into(),
            ..drums
        });
        s.master_bars[1].repeat_start = true;
        let mut h = History::new();
        let all = Selection::Bars {
            tracks: (0, 1),
            bars: (1, 1),
        };
        let clip = copy(&s, at(1, 0), Some(all)).unwrap();
        run(
            &mut s,
            &mut h,
            at(0, 0),
            None,
            paste(clip.clone(), 2, false),
        )
        .unwrap();
        assert_eq!(s.bar_count(), 5);
        s.check_structure().unwrap();
        assert!(s.master_bars[0].repeat_start && s.master_bars[1].repeat_start);
        assert_eq!(frets(&s, 0, 0), [4, 5, 6, 7].map(Some));
        assert_eq!(frets(&s, 0, 2), [0, 1, 2, 3].map(Some));
        h.undo(&mut s);
        // One track's bars into the drums: only the rhythm, as rests.
        let one = Selection::Bars {
            tracks: (0, 0),
            bars: (0, 0),
        };
        let clip = copy(&s, at(0, 0), Some(one)).unwrap();
        let at = Caret {
            track: 1,
            ..at(2, 0)
        };
        run(&mut s, &mut h, at, None, paste(clip, 2, true)).unwrap();
        assert_eq!(s.bar_count(), 4, "a bar added at the end");
        s.check_structure().unwrap();
        let bar = &s.tracks[1].staves[0].bars[3];
        assert!(bar.voices[0].beats.iter().all(|b| b.kind == BeatKind::Rest));
        assert_eq!(
            frets(&s, 0, 2),
            [8, 9, 10, 11].map(Some),
            "the guitar is untouched"
        );
    }

    #[test]
    fn cut_is_named_and_undone() {
        let mut s = score(2);
        let before = s.clone();
        let mut h = History::new();
        let sel = Selection::Bars {
            tracks: (0, 0),
            bars: (0, 0),
        };
        run(&mut s, &mut h, at(0, 0), Some(sel), Command::Cut).unwrap();
        assert_eq!(s.bar_count(), 1);
        assert_eq!(h.undo_label(), Some("Cut"));
        run(&mut s, &mut h, at(0, 0), None, Command::Cut).unwrap();
        assert_eq!(frets(&s, 0, 0), [5, 6, 7].map(Some));
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, before);
        assert!(
            copy(&s, at(0, 4), None).is_none(),
            "the place after the last beat"
        );
    }
}
