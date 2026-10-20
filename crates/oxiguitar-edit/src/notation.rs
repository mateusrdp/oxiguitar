// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Chord diagrams, text and lyrics (M3 slice 6).
//!
//! - [`Command::Chord`] puts a chord diagram on the caret's beat (a note,
//!   a chord or a rest), or takes it off. The track keeps its diagrams in
//!   one list, each once: a diagram the track has already is used again.
//!   On a rest, a diagram with frets puts its notes in the beat, as Guitar
//!   Pro 8 does when the beat is empty.
//! - [`Command::Text`] sets the free text above the caret's beat.
//! - [`Command::Syllable`] types one syllable of a line of the caret's
//!   track's lyrics at a beat, as MuseScore does (Mateus, 21 Oct 2026):
//!   the line's text changes only there (`oxiguitar_model::lyrics`). A line
//!   is Guitar Pro's: from a bar, a syllable per beat the first voice
//!   sings; Guitar Pro keeps five.

use oxiguitar_model::lyrics::{set_syllable, sung_beats, syllables};
use oxiguitar_model::{BeatKind, ChordDiagram, LyricLine, Note, Score};

use crate::{Caret, Command, Edited, History, Region};

/// Runs a chord, text or lyrics command; `None` for another command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    command: &Command,
) -> Option<Option<Edited>> {
    let label = command.label();
    let done = match command {
        Command::Chord(d) => chord(score, history, c, d.as_ref(), label),
        Command::Text(t) => text(score, history, c, t, label),
        Command::Syllable {
            line,
            at,
            text,
            hyphen,
        } => syllable(score, history, c, *line, *at, text, *hyphen, label),
        _ => return None,
    };
    Some(done.map(|touched| Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    }))
}

/// Lines of lyrics a track keeps at most, as Guitar Pro.
pub const LYRIC_LINES: usize = 5;

/// [`Command::Syllable`].
#[allow(clippy::too_many_arguments)]
fn syllable(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    line: usize,
    at: (usize, usize),
    word: &str,
    hyphen: bool,
    label: &str,
) -> Option<crate::Touched> {
    let track = score.tracks.get(c.track)?;
    let staff = track.staves.first()?;
    let beat = staff.bars.get(at.0)?.voices.first()?.beats.get(at.1)?;
    if !oxiguitar_model::lyrics::is_sung(beat) || line >= LYRIC_LINES {
        return None;
    }
    let mut lines = track.lyrics.clone();
    // A line after the last starts at this bar.
    let line = line.min(lines.len());
    if line == lines.len() {
        if word.trim().is_empty() {
            return None;
        }
        lines.push(LyricLine {
            start_bar: at.0,
            text: String::new(),
        });
    }
    let l = lines.get_mut(line)?;
    // Before the line's start: the line starts earlier, with empty beats
    // up to where it started.
    if at.0 < l.start_bar {
        let skipped = sung_beats(staff, at.0)
            .iter()
            .filter(|(bar, _)| *bar < l.start_bar)
            .count();
        l.text = format!("{}{}", "-".repeat(skipped), l.text);
        l.start_bar = at.0;
    }
    let index = sung_beats(staff, l.start_bar)
        .iter()
        .position(|b| *b == at)?;
    let text = set_syllable(&l.text, index, word, hyphen);
    // As files keep it: trimmed, and a line with nothing sung goes.
    l.text = text.trim().to_owned();
    if syllables(&l.text).iter().all(|s| s.text.is_empty()) {
        lines.remove(line);
    }
    if lines == track.lyrics {
        return None;
    }
    let t = c.track;
    Some(history.record(score, label, vec![Region::Track(t)], |s| {
        if let Some(tr) = s.tracks.get_mut(t) {
            tr.lyrics = lines;
        }
    }))
}

fn text(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    text: &str,
    label: &str,
) -> Option<crate::Touched> {
    let text = text.trim().to_owned();
    let beat = c.voice_in(score)?.beats.get(c.beat)?;
    if beat.kind == BeatKind::Empty || beat.text == text {
        return None;
    }
    let (t, st, bar, voice, b) = (c.track, c.staff, c.bar, c.voice, c.beat);
    let region = Region::Bars {
        track: t,
        staff: st,
        bars: bar..bar + 1,
    };
    Some(history.record(score, label, vec![region], |s| {
        if let Some(beat) = s
            .tracks
            .get_mut(t)
            .and_then(|tr| tr.staves.get_mut(st))
            .and_then(|staff| staff.bars.get_mut(bar))
            .and_then(|br| br.voices.get_mut(voice))
            .and_then(|v| v.beats.get_mut(b))
        {
            beat.text = text;
        }
    }))
}

fn chord(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    diagram: Option<&ChordDiagram>,
    label: &str,
) -> Option<crate::Touched> {
    let track = score.tracks.get(c.track)?;
    let staff = track.staves.get(c.staff)?;
    let drums = track.instrument.percussion;
    let old = staff.bars.get(c.bar)?;
    let dynamic = crate::entry::dynamic_at(score, c);
    let mut chords = track.chords.clone();
    let mut bar = old.clone();
    // At the place after a voice's last beat, a new beat (a rest until
    // the diagram's notes fill it), as typing a fret there adds one.
    let beat = crate::entry::beat_at_caret(&mut bar, c, BeatKind::Rest)?;
    if beat.kind == BeatKind::Empty {
        beat.kind = BeatKind::Rest;
    }
    match diagram {
        None => beat.chord = None,
        Some(d) => {
            let strings = staff.string_count();
            if !d.frets.is_empty() && d.frets.len() != strings {
                return None;
            }
            // A fret past the neck can't be played; one below the
            // diagram's first can't be drawn.
            if d.frets.iter().flatten().any(|f| *f > staff.fret_count)
                || d.first_fret == 0
                || d.frets
                    .iter()
                    .flatten()
                    .any(|f| *f != 0 && *f < d.first_fret)
            {
                return None;
            }
            let index = match chords.iter().position(|x| x == d) {
                Some(i) => i,
                None => {
                    chords.push(d.clone());
                    chords.len() - 1
                }
            };
            beat.chord = Some(index);
            if beat.kind == BeatKind::Rest && !drums && d.frets.iter().any(Option::is_some) {
                beat.kind = BeatKind::Notes;
                beat.notes = d
                    .frets
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        let string = u8::try_from(i + 1).ok()?;
                        f.map(|f| Note::fretted(string, f))
                    })
                    .collect();
                crate::entry::settle_dynamic(beat, dynamic);
            }
        }
    }
    crate::entry::tidy(&mut bar);
    if bar == *old && chords == track.chords {
        return None;
    }
    let (t, st, b) = (c.track, c.staff, c.bar);
    let regions = vec![
        Region::Track(t),
        Region::Bars {
            track: t,
            staff: st,
            bars: b..b + 1,
        },
    ];
    Some(history.record(score, label, regions, |s| {
        let Some(tr) = s.tracks.get_mut(t) else {
            return;
        };
        tr.chords = chords;
        if let Some(dst) = tr
            .staves
            .get_mut(st)
            .and_then(|staff| staff.bars.get_mut(b))
        {
            *dst = bar;
        }
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::{apply, new_score};

    fn at(beat: usize) -> Caret {
        Caret {
            beat,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    fn am() -> ChordDiagram {
        ChordDiagram {
            name: "Am".into(),
            first_fret: 1,
            frets: vec![Some(0), Some(1), Some(2), Some(2), Some(0), None],
        }
    }

    #[test]
    fn a_chord_on_a_rest_puts_its_notes_in() {
        let mut s = new_score();
        let mut h = History::new();
        // On the empty bar's first place: a new beat with the notes.
        apply(&mut s, &mut h, at(0), None, Command::Chord(Some(am()))).unwrap();
        let beat = &s.tracks[0].staves[0].bars[0].voices[0].beats[0];
        assert_eq!(beat.kind, BeatKind::Notes);
        assert_eq!(beat.notes.len(), 5);
        assert_eq!(beat.chord, Some(0));
        assert_eq!(s.tracks[0].chords, [am()]);
        // The same diagram again, on another beat: used again.
        apply(&mut s, &mut h, at(1), None, Command::Rest).unwrap();
        apply(&mut s, &mut h, at(1), None, Command::Chord(Some(am()))).unwrap();
        assert_eq!(s.tracks[0].chords.len(), 1);
        // Off; the diagram stays in the track's list.
        apply(&mut s, &mut h, at(1), None, Command::Chord(None)).unwrap();
        assert_eq!(s.tracks[0].staves[0].bars[0].voices[0].beats[1].chord, None);
        assert_eq!(h.undo_label(), Some("Chord"));
        // A diagram for another number of strings: refused.
        let mut four = am();
        four.frets.truncate(4);
        assert!(apply(&mut s, &mut h, at(0), None, Command::Chord(Some(four))).is_none());
    }

    #[test]
    fn text() {
        let mut s = new_score();
        let mut h = History::new();
        apply(&mut s, &mut h, at(0), None, Command::Rest).unwrap();
        apply(
            &mut s,
            &mut h,
            at(0),
            None,
            Command::Text("  let ring ".into()),
        )
        .unwrap();
        assert_eq!(
            s.tracks[0].staves[0].bars[0].voices[0].beats[0].text,
            "let ring"
        );
        assert!(
            apply(
                &mut s,
                &mut h,
                at(0),
                None,
                Command::Text("let ring".into())
            )
            .is_none()
        );
    }

    fn syl(line: usize, beat: usize, text: &str, hyphen: bool) -> Command {
        Command::Syllable {
            line,
            at: (0, beat),
            text: text.into(),
            hyphen,
        }
    }

    #[test]
    fn syllables_typed_beat_by_beat() {
        let mut s = new_score();
        let mut h = History::new();
        for beat in 0..4 {
            apply(
                &mut s,
                &mut h,
                at(beat),
                None,
                Command::Fret {
                    fret: 3,
                    again: false,
                },
            )
            .unwrap();
        }
        let original = s.clone();
        let mut h = History::new();
        apply(&mut s, &mut h, at(0), None, syl(0, 0, "Hel", true)).unwrap();
        apply(&mut s, &mut h, at(0), None, syl(0, 1, "lo", false)).unwrap();
        // The fourth beat: the third is left empty.
        apply(&mut s, &mut h, at(0), None, syl(0, 3, "world", false)).unwrap();
        assert_eq!(s.tracks[0].lyrics[0].text, "Hel-lo  world");
        assert_eq!(h.undo_label(), Some("Lyrics"));
        // A second line, from the third beat.
        apply(&mut s, &mut h, at(0), None, syl(1, 2, "two", false)).unwrap();
        assert_eq!(s.tracks[0].lyrics[1].text, "--two");
        // Line 4 when there are two: the third.
        apply(&mut s, &mut h, at(0), None, syl(3, 0, "three", false)).unwrap();
        assert_eq!(s.tracks[0].lyrics.len(), 3);
        // Taking every syllable away takes the line away.
        apply(&mut s, &mut h, at(0), None, syl(2, 0, "", false)).unwrap();
        assert_eq!(s.tracks[0].lyrics.len(), 2);
        // A rest sings nothing.
        apply(&mut s, &mut h, at(3), None, Command::Rest).unwrap();
        assert!(apply(&mut s, &mut h, at(0), None, syl(0, 3, "x", false)).is_none());
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, original);
    }
}
