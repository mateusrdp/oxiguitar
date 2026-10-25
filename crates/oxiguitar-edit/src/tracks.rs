// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Tracks (M3 slice 5, EDIT-4): adding, duplicating, deleting and moving
//! tracks, and a track's properties: name, instrument, capo, frets, and its
//! strings and tuning. Changing the strings, tuning or capo of a track with
//! notes keeps either their pitches (frets are found again: on the same
//! string if it can play the note, else on another string the beat leaves
//! free) or their frets (the music changes with the tuning); a note that
//! can't be kept goes, and [`notes_lost`] says how many first.

use oxiguitar_model::{Bar, Instrument, Note, NotePlace, Pitch, Score, Staff, Track, Voice};

use crate::{Caret, Command, Edited, History, Region};

/// What a track's properties change to.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackSettings {
    /// Full name.
    pub name: String,
    /// Short name, at the start of later systems.
    pub short_name: String,
    /// Its sound.
    pub instrument: Instrument,
    /// Capo fret, 0 for none.
    pub capo: u8,
    /// Frets.
    pub fret_count: u8,
    /// Open strings, highest first; empty for an instrument without
    /// strings. Every staff of the track takes it.
    pub tuning: Vec<Pitch>,
    /// When the strings, tuning or capo change: keep the notes' pitches
    /// (true) or their frets (false).
    pub keep_pitches: bool,
    /// What plugins keep for the track, by plugin id (their sections of the
    /// track dialog, plugin interface 1.3).
    pub plugins: std::collections::BTreeMap<String, String>,
}

impl TrackSettings {
    /// A track's settings as they are (its first staff's strings).
    pub fn of(track: &Track) -> TrackSettings {
        let staff = track.staves.first();
        TrackSettings {
            name: track.name.clone(),
            short_name: track.short_name.clone(),
            instrument: track.instrument,
            capo: staff.map_or(0, |s| s.capo),
            fret_count: staff.map_or(24, |s| s.fret_count),
            tuning: staff.map(|s| s.tuning.clone()).unwrap_or_default(),
            keep_pitches: true,
            plugins: track.plugins.clone(),
        }
    }
}

/// Runs a track command; `None` for a command that isn't one.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    command: &Command,
) -> Option<Option<Edited>> {
    let label = command.label();
    let t = c.track;
    let edited = match command {
        Command::AddTrack => {
            let at = (t + 1).min(score.tracks.len());
            let track = new_track(score, format!("Track {}", score.tracks.len() + 1));
            whole(score, history, label, c, at, |s| s.tracks.insert(at, track))
        }
        Command::DuplicateTrack => {
            let mut copy = score.tracks.get(t)?.clone();
            copy.name = match copy.name.trim() {
                "" => "Copy".to_owned(),
                name => format!("{name} (copy)"),
            };
            whole(score, history, label, c, t + 1, |s| {
                s.tracks.insert(t + 1, copy)
            })
        }
        Command::DeleteTrack => {
            if score.tracks.len() < 2 || t >= score.tracks.len() {
                return Some(None);
            }
            let to = t.min(score.tracks.len() - 2);
            whole(score, history, label, c, to, |s| {
                s.tracks.remove(t);
            })
        }
        Command::MoveTrackUp | Command::MoveTrackDown => {
            let other = if *command == Command::MoveTrackUp {
                t.checked_sub(1)
            } else {
                Some(t + 1).filter(|o| *o < score.tracks.len())
            };
            match other {
                Some(o) if t < score.tracks.len() => {
                    whole(score, history, label, c, o, |s| s.tracks.swap(t, o))
                }
                _ => None,
            }
        }
        Command::TrackProperties(to) => properties(score, history, label, c, to),
        _ => return None,
    };
    Some(edited)
}

/// A steel-string guitar in standard tuning, its bars empty, as File →
/// New makes one.
fn new_track(score: &Score, name: String) -> Track {
    let short = name
        .split_whitespace()
        .map(|w| w.chars().next().unwrap_or('T'))
        .collect::<String>();
    Track {
        name,
        short_name: short,
        staves: vec![Staff {
            tuning: Staff::standard_guitar_tuning(),
            bars: (0..score.bar_count())
                .map(|_| Bar {
                    voices: vec![Voice::default()],
                    ..Bar::default()
                })
                .collect(),
            ..Staff::default()
        }],
        ..Track::default()
    }
}

/// A change to the list of tracks, recorded whole; the caret goes to the
/// start of the caret's bar in track `to`.
fn whole(
    score: &mut Score,
    history: &mut History,
    label: &str,
    c: Caret,
    to: usize,
    edit: impl FnOnce(&mut Score),
) -> Option<Edited> {
    let touched = history.record(score, label, vec![Region::Whole], edit);
    let caret = Caret {
        track: to,
        staff: 0,
        voice: 0,
        beat: 0,
        ..c
    };
    Some(Edited {
        touched,
        caret: caret.clamp(score).or_else(|| Caret::at_start(score, to))?,
    })
}

/// How many notes `settings` would leave out of track `track`: notes on
/// strings it no longer has, or whose pitch no string can play.
pub fn notes_lost(score: &Score, track: usize, settings: &TrackSettings) -> usize {
    let Some(t) = score.tracks.get(track) else {
        return 0;
    };
    let mut copy = t.clone();
    let before = count_notes(&copy);
    restring(&mut copy, settings);
    before - count_notes(&copy)
}

fn count_notes(t: &Track) -> usize {
    t.staves
        .iter()
        .flat_map(|s| &s.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .map(|b| b.notes.len())
        .sum()
}

fn properties(
    score: &mut Score,
    history: &mut History,
    label: &str,
    c: Caret,
    to: &TrackSettings,
) -> Option<Edited> {
    let t = c.track;
    let old = score.tracks.get(t)?;
    let same = TrackSettings {
        keep_pitches: true,
        ..to.clone()
    };
    if TrackSettings::of(old) == same
        && old
            .staves
            .iter()
            .all(|s| s.tuning == to.tuning && s.capo == to.capo)
    {
        return None;
    }
    let mut track = old.clone();
    track.name.clone_from(&to.name);
    track.short_name.clone_from(&to.short_name);
    track.instrument = to.instrument;
    track.plugins.clone_from(&to.plugins);
    restring(&mut track, to);
    let mut regions = vec![Region::Track(t)];
    let bars = score.bar_count();
    for s in 0..track.staves.len() {
        regions.push(Region::Bars {
            track: t,
            staff: s,
            bars: 0..bars,
        });
    }
    let touched = history.record(score, label, regions, |s| {
        if let Some(dst) = s.tracks.get_mut(t) {
            *dst = track;
        }
    });
    Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    })
}

/// The track's staves with `to`'s strings, capo and frets, their notes
/// kept by pitch or by fret, then their ties followed again.
fn restring(track: &mut Track, to: &TrackSettings) {
    let drums = to.instrument.percussion;
    for staff in &mut track.staves {
        // The staff's strings before and after, without its bars.
        let mut bars = std::mem::take(&mut staff.bars);
        let old = staff.clone();
        staff.capo = to.capo;
        staff.fret_count = to.fret_count;
        staff.tuning = if drums { Vec::new() } else { to.tuning.clone() };
        if staff.tuning.is_empty() {
            // No strings: show what they can.
            staff.show_tablature = false;
            staff.show_standard = true;
        }
        if drums {
            staff.bars = bars;
            continue;
        }
        for bar in &mut bars {
            for beat in bar.voices.iter_mut().flat_map(|v| &mut v.beats) {
                let notes = std::mem::take(&mut beat.notes);
                for n in notes {
                    if let Some(place) = replace(&old, staff, &beat.notes, &n, to.keep_pitches) {
                        beat.notes.push(Note { place, ..n });
                    }
                }
                beat.notes.sort_by_key(|n| n.string().unwrap_or(u8::MAX));
                if beat.notes.is_empty() && beat.kind == oxiguitar_model::BeatKind::Notes {
                    beat.kind = oxiguitar_model::BeatKind::Rest;
                    beat.effects = Default::default();
                }
            }
        }
        staff.bars = bars;
        crate::entry::retie(staff, 0);
    }
    for d in &mut track.chords {
        d.frets.resize(to.tuning.len(), None);
    }
    // Diagrams that differed only on strings now gone are one diagram (a
    // file holds each diagram once).
    let mut kept: Vec<oxiguitar_model::ChordDiagram> = Vec::new();
    let renumber: Vec<usize> = std::mem::take(&mut track.chords)
        .into_iter()
        .map(|d| match kept.iter().position(|k| *k == d) {
            Some(i) => i,
            None => {
                kept.push(d);
                kept.len() - 1
            }
        })
        .collect();
    track.chords = kept;
    for beat in track
        .staves
        .iter_mut()
        .flat_map(|s| &mut s.bars)
        .flat_map(|b| &mut b.voices)
        .flat_map(|v| &mut v.beats)
    {
        beat.chord = beat.chord.and_then(|i| renumber.get(i).copied());
    }
}

/// Where note `n` of the old staff goes on the new one, beside `placed`
/// (the beat's notes already placed); `None` when it can't be kept.
fn replace(
    old: &Staff,
    new: &Staff,
    placed: &[Note],
    n: &Note,
    keep_pitches: bool,
) -> Option<NotePlace> {
    let pitch = match n.place {
        NotePlace::Fretted { string, fret } => old.pitch_of(string, fret)?,
        NotePlace::Pitched(p) => p,
    };
    let strings = u8::try_from(new.string_count()).ok()?;
    if strings == 0 {
        return Some(NotePlace::Pitched(pitch));
    }
    let free = |s: u8| !placed.iter().any(|p| p.string() == Some(s));
    let fret_on = |s: u8| -> Option<u8> {
        let open = new.pitch_of(s, 0)?.0;
        pitch.0.checked_sub(open).filter(|f| *f <= new.fret_count)
    };
    if let NotePlace::Fretted { string, fret } = n.place
        && string <= strings
        && free(string)
    {
        if !keep_pitches {
            return Some(NotePlace::Fretted { string, fret });
        }
        if let Some(fret) = fret_on(string) {
            return Some(NotePlace::Fretted { string, fret });
        }
    } else if !keep_pitches && matches!(n.place, NotePlace::Fretted { .. }) {
        return None;
    }
    // Another string the beat leaves free, the fret nearest where the
    // hand was first.
    let was = match n.place {
        NotePlace::Fretted { fret, .. } => fret,
        NotePlace::Pitched(_) => 0,
    };
    (1..=strings)
        .filter(|s| free(*s))
        .filter_map(|s| fret_on(s).map(|f| (s, f)))
        .min_by_key(|&(s, f)| (f.abs_diff(was), s))
        .map(|(string, fret)| NotePlace::Fretted { string, fret })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply as run, new_score};
    use oxiguitar_model::tuning::{Pattern, tuning};

    fn at(track: usize) -> Caret {
        Caret {
            track,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    fn fret(s: &mut Score, h: &mut History, track: usize, beat: usize, string: u8, f: u8) {
        let c = Caret {
            beat,
            string,
            ..at(track)
        };
        run(
            s,
            h,
            c,
            None,
            Command::Fret {
                fret: f,
                again: false,
            },
        )
        .unwrap();
    }

    fn places(s: &Score, track: usize) -> Vec<NotePlace> {
        s.tracks[track].staves[0].bars[0].voices[0]
            .beats
            .iter()
            .flat_map(|b| &b.notes)
            .map(|n| n.place)
            .collect()
    }

    #[test]
    fn add_duplicate_move_delete() {
        let mut s = new_score();
        let mut h = History::new();
        fret(&mut s, &mut h, 0, 0, 1, 3);
        let e = run(&mut s, &mut h, at(0), None, Command::AddTrack).unwrap();
        assert_eq!((s.tracks.len(), e.caret.track), (2, 1));
        assert_eq!(s.tracks[1].name, "Track 2");
        s.check_structure().unwrap();
        run(&mut s, &mut h, at(0), None, Command::DuplicateTrack).unwrap();
        assert_eq!(s.tracks[1].name, "Track 1 (copy)");
        assert_eq!(places(&s, 1), places(&s, 0));
        let e = run(&mut s, &mut h, at(2), None, Command::MoveTrackUp).unwrap();
        assert_eq!((s.tracks[1].name.as_str(), e.caret.track), ("Track 2", 1));
        assert!(run(&mut s, &mut h, at(0), None, Command::MoveTrackUp).is_none());
        run(&mut s, &mut h, at(0), None, Command::DeleteTrack).unwrap();
        run(&mut s, &mut h, at(0), None, Command::DeleteTrack).unwrap();
        assert_eq!(s.tracks.len(), 1);
        assert!(
            run(&mut s, &mut h, at(0), None, Command::DeleteTrack).is_none(),
            "never the last"
        );
        while h.undo(&mut s).is_some() {}
        assert_eq!(s.tracks.len(), 1);
        assert_eq!(s.tracks[0].name, "Track 1");
    }

    #[test]
    fn retuning_keeps_pitches_or_frets() {
        let mut s = new_score();
        let mut h = History::new();
        fret(&mut s, &mut h, 0, 0, 6, 0); // low E
        fret(&mut s, &mut h, 0, 1, 6, 5); // A on the low string
        let drop_d = tuning(Pattern::Drop, 2, 6, false).unwrap();
        let mut to = TrackSettings::of(&s.tracks[0]);
        to.tuning = drop_d.clone();
        // Keeping pitches: low E is fret 2 on the D string; A fret 7.
        assert_eq!(notes_lost(&s, 0, &to), 0);
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::TrackProperties(Box::new(to.clone())),
        )
        .unwrap();
        assert_eq!(
            places(&s, 0),
            [
                NotePlace::Fretted { string: 6, fret: 2 },
                NotePlace::Fretted { string: 6, fret: 7 }
            ]
        );
        h.undo(&mut s);
        // Keeping frets: the music drops with the string.
        to.keep_pitches = false;
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::TrackProperties(Box::new(to.clone())),
        )
        .unwrap();
        assert_eq!(places(&s, 0)[0], NotePlace::Fretted { string: 6, fret: 0 });
        h.undo(&mut s);
        // The top four strings (D G B E), keeping pitches: E (40) and A
        // (45) are below D (50), so both go and their beats are rests.
        to.tuning = s.tracks[0].staves[0].tuning[..4].to_vec();
        to.keep_pitches = true;
        assert_eq!(notes_lost(&s, 0, &to), 2);
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::TrackProperties(Box::new(to.clone())),
        )
        .unwrap();
        let beats = &s.tracks[0].staves[0].bars[0].voices[0].beats;
        assert!(
            beats[..2]
                .iter()
                .all(|b| b.kind == oxiguitar_model::BeatKind::Rest)
        );
        h.undo(&mut s);
        // A capo on 2, keeping pitches: frets two lower, and the open low
        // E, now below every string, goes.
        let mut capo = TrackSettings::of(&s.tracks[0]);
        capo.capo = 2;
        assert_eq!(notes_lost(&s, 0, &capo), 1);
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::TrackProperties(Box::new(capo)),
        )
        .unwrap();
        assert_eq!(places(&s, 0), [NotePlace::Fretted { string: 6, fret: 3 }]);
        // Without strings: pitches.
        let mut piano = TrackSettings::of(&s.tracks[0]);
        piano.tuning.clear();
        piano.instrument.midi_program = 0;
        run(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::TrackProperties(Box::new(piano)),
        )
        .unwrap();
        assert_eq!(places(&s, 0), [NotePlace::Pitched(Pitch(45))]);
        assert!(!s.tracks[0].staves[0].show_tablature);
        // Same settings: nothing to do.
        let same = TrackSettings::of(&s.tracks[0]);
        assert!(
            run(
                &mut s,
                &mut h,
                at(0),
                None,
                Command::TrackProperties(Box::new(same))
            )
            .is_none()
        );
    }
}
