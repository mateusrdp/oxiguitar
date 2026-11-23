// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Tracks, staves, tunings and instruments, and the track's mixer (`Mix`:
//! saved in the file since #300, mute and solo too).
//!
//! A field added here doesn't compile in `wire.rs` until someone decides how
//! it crosses the plugin interface (a variant at the end of the matching
//! `score2` extension list, never a change to version 1), and every format
//! plugin's writer then decides whether its format can hold it; what one
//! can't is kept in the `.oxg` file beside the score if the user says so.
//! Edits to it go through `oxiguitar-edit`'s commands, so undo works.

use std::collections::BTreeMap;

use crate::{Bar, Color, Pitch};

/// One instrument part: a guitar, a bass, a drum kit, a piano.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Full name, such as "Lead Guitar".
    pub name: String,
    /// Short name shown at the start of later systems.
    pub short_name: String,
    /// Display colour.
    pub color: Color,
    /// The sound it plays with.
    pub instrument: Instrument,
    /// Mixer settings.
    pub mix: Mix,
    /// One staff for most instruments; two for a piano grand staff.
    pub staves: Vec<Staff>,
    /// Chord diagrams used by this track; beats refer to them by index.
    pub chords: Vec<ChordDiagram>,
    /// Lyrics lines.
    pub lyrics: Vec<LyricLine>,
    /// What plugins keep for this track (the sections they add to the
    /// track dialog, plugin interface 1.3): a JSON object as text, by
    /// plugin id. No score format holds it: it is saved beside the score,
    /// in `FILE.oxg` (`song.gp3.oxg`).
    pub plugins: BTreeMap<String, String>,
    /// The order the track's effects run in, by plugin id, as the user set
    /// it in the track dialog (#152); empty for the default order. Saved
    /// beside the score with [`Track::plugins`].
    pub effects: Vec<String>,
    /// Whether its chord diagrams are played, each until the next, by a
    /// copy of its instrument and effects, as a voice of its own (#151).
    /// Saved beside the score with [`Track::plugins`].
    pub play_chords: bool,
    /// Bars per system on the page when this track is shown alone, from
    /// the first system on (Design Mode); empty, or past its end, for the
    /// automatic layout.
    pub layout: Vec<u8>,
    /// Index of another track whose chord diagrams to show above this
    /// track's staves; [`usize::MAX`] for none. Purely visual — the
    /// referenced chords are not persisted as part of this track's
    /// chord data.
    pub chords_from: usize,
    /// Index of the synthetic chord voice for this track's played chords;
    /// [`usize::MAX`] when the track has no chord voice. Derived as
    /// `score.tracks.len() + track_index` when a chord voice exists.
    /// Persisted for round-trip compatibility with older files.
    pub chords_voice_index: usize,
}

/// What a track sounds like through General MIDI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Instrument {
    /// General MIDI program, 0 to 127 (25 is steel-string guitar).
    pub midi_program: u8,
    /// True for a drum kit (MIDI channel 10).
    pub percussion: bool,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            name: String::new(),
            short_name: String::new(),
            color: Color::default(),
            instrument: Instrument::default(),
            mix: Mix::default(),
            staves: Vec::new(),
            chords: Vec::new(),
            lyrics: Vec::new(),
            plugins: BTreeMap::new(),
            effects: Vec::new(),
            play_chords: false,
            layout: Vec::new(),
            chords_from: usize::MAX,
            chords_voice_index: usize::MAX,
        }
    }
}

impl Default for Instrument {
    /// A steel-string guitar (program 25), as a new track in Guitar Pro is.
    fn default() -> Self {
        Instrument {
            midi_program: 25,
            percussion: false,
        }
    }
}

impl Instrument {
    /// A fretted string instrument, by its General MIDI program: the
    /// guitars (24–31), the fretted basses (32–34, 36–37; not fretless 35
    /// or the synth basses), sitar (104) and banjo (105). Only these show
    /// tablature unless the user asks for it (Mateus, 16 Oct 2026).
    pub fn is_fretted(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 24..=34 | 36 | 37 | 104 | 105)
    }

    /// A bass: the bass programs (32–39), fretless and synth basses too.
    pub fn is_bass(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 32..=39)
    }

    /// A keyboard played with two hands, shown on a grand staff: pianos and
    /// electric pianos, harpsichord and clavinet (0–7), organs and
    /// accordions (16–21, 23). Not harmonica (22).
    pub fn is_keyboard(&self) -> bool {
        !self.percussion && matches!(self.midi_program, 0..=7 | 16..=21 | 23)
    }
}

/// Mixer settings for a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    /// Volume, 0.0 to 1.0.
    pub volume: f32,
    /// Pan, -1.0 (left) to 1.0 (right).
    pub pan: f32,
    /// Track is muted.
    pub mute: bool,
    /// Track is soloed.
    pub solo: bool,
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            volume: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

/// One staff of a track, with its strings and its bars of music.
#[derive(Clone, Debug, PartialEq)]
pub struct Staff {
    /// Open-string pitches, highest string first (string 1 is the top line of
    /// the tablature). Empty for instruments without strings.
    pub tuning: Vec<Pitch>,
    /// Capo fret, 0 for none. Fret numbers in notes count from the capo, as in
    /// Guitar Pro: fret 0 with a capo on 2 sounds two semitones above the open
    /// string.
    pub capo: u8,
    /// Number of frets.
    pub fret_count: u8,
    /// Semitones from sounding pitch to written pitch in standard notation.
    /// 0 for no transposition. (The guitar's usual octave is shown by the
    /// treble 8vb clef, not here.)
    pub transposition: i8,
    /// Show standard notation.
    pub show_standard: bool,
    /// Show tablature.
    pub show_tablature: bool,
    /// One bar per master bar of the score.
    pub bars: Vec<Bar>,
}

impl Default for Staff {
    fn default() -> Self {
        Staff {
            tuning: Vec::new(),
            capo: 0,
            fret_count: 24,
            transposition: 0,
            show_standard: true,
            show_tablature: true,
            bars: Vec::new(),
        }
    }
}

impl Staff {
    /// Standard six-string guitar tuning, E A D G B E, highest string first.
    pub fn standard_guitar_tuning() -> Vec<Pitch> {
        [64, 59, 55, 50, 45, 40].into_iter().map(Pitch).collect()
    }

    /// Number of strings (0 for instruments without strings).
    pub fn string_count(&self) -> usize {
        self.tuning.len()
    }

    /// The sounding pitch of `fret` on `string` (1 = highest), counting the capo,
    /// or `None` if the string does not exist or the pitch leaves the MIDI range.
    pub fn pitch_of(&self, string: u8, fret: u8) -> Option<Pitch> {
        let open = self.tuning.get(usize::from(string).checked_sub(1)?)?;
        let p = u16::from(open.0) + u16::from(self.capo) + u16::from(fret);
        u8::try_from(p).ok().filter(|p| *p <= 127).map(Pitch)
    }
}

/// A chord diagram: name plus fret positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordDiagram {
    /// Chord name, such as "Am7".
    pub name: String,
    /// Lowest fret shown in the diagram.
    pub first_fret: u8,
    /// Fret per string, highest string first; `None` for a string not played,
    /// `Some(0)` for an open string.
    pub frets: Vec<Option<u8>>,
}

/// One line of lyrics, attached to a track.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricLine {
    /// Index of the bar where the line starts (0-based).
    pub start_bar: usize,
    /// The text, syllables separated by spaces or hyphens as in Guitar Pro.
    pub text: String,
}

/// What plugins keep at one place of a track, by plugin id: their data
/// (`plugins` on a staff's bar, a beat or a note) and, on a beat, the marks
/// they draw there ([`BeatEffects::marks`](crate::BeatEffects)).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    /// Each plugin's data, a JSON object as text.
    pub plugins: BTreeMap<String, String>,
    /// Each plugin's mark (beats only).
    pub marks: BTreeMap<String, String>,
}

impl Kept {
    /// Whether nothing is kept.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty() && self.marks.is_empty()
    }
}

impl Track {
    /// What plugins keep in this track (plugin interface 1.8), each with
    /// its place: `[staff, bar]` for a staff's bar, `[staff, bar, voice,
    /// beat]` for a beat, `[staff, bar, voice, beat, note]` for a note,
    /// counted from 0. What the file beside the score keeps, since no score
    /// format holds it.
    pub fn kept(&self) -> Vec<(Vec<usize>, Kept)> {
        let mut out = Vec::new();
        for (si, staff) in self.staves.iter().enumerate() {
            for (bi, bar) in staff.bars.iter().enumerate() {
                if !bar.plugins.is_empty() {
                    out.push((
                        vec![si, bi],
                        Kept {
                            plugins: bar.plugins.clone(),
                            marks: BTreeMap::new(),
                        },
                    ));
                }
                for (vi, voice) in bar.voices.iter().enumerate() {
                    for (ei, beat) in voice.beats.iter().enumerate() {
                        if !beat.plugins.is_empty() || !beat.effects.marks.is_empty() {
                            out.push((
                                vec![si, bi, vi, ei],
                                Kept {
                                    plugins: beat.plugins.clone(),
                                    marks: beat.effects.marks.clone(),
                                },
                            ));
                        }
                        for (ni, note) in beat.notes.iter().enumerate() {
                            if !note.plugins.is_empty() {
                                out.push((
                                    vec![si, bi, vi, ei, ni],
                                    Kept {
                                        plugins: note.plugins.clone(),
                                        marks: BTreeMap::new(),
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Gives the place `at` ([`Track::kept`]'s) what plugins kept there.
    /// False, and nothing changes, when there is no such place (the score
    /// was changed elsewhere since) or `at` has another length. Marks are
    /// only kept on beats.
    pub fn keep(&mut self, at: &[usize], kept: Kept) -> bool {
        let (Some(&staff), Some(&bar)) = (at.first(), at.get(1)) else {
            return false;
        };
        let Some(bar) = self.staves.get_mut(staff).and_then(|s| s.bars.get_mut(bar)) else {
            return false;
        };
        if at.len() == 2 {
            bar.plugins = kept.plugins;
            return true;
        }
        let Some(beat) = at
            .get(2)
            .and_then(|&v| bar.voices.get_mut(v))
            .and_then(|v| v.beats.get_mut(*at.get(3)?))
        else {
            return false;
        };
        match at.len() {
            4 => {
                beat.plugins = kept.plugins;
                beat.effects.marks = kept.marks;
                true
            }
            5 => match beat.notes.get_mut(at[4]) {
                Some(note) => {
                    note.plugins = kept.plugins;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_families() {
        let gm = |p| Instrument {
            midi_program: p,
            percussion: false,
        };
        assert!(gm(25).is_fretted() && gm(33).is_fretted() && gm(105).is_fretted());
        assert!(!gm(35).is_fretted(), "fretless bass");
        assert!(!gm(40).is_fretted() && !gm(0).is_fretted() && !gm(38).is_fretted());
        assert!(gm(35).is_bass() && gm(38).is_bass() && !gm(30).is_bass());
        assert!(gm(0).is_keyboard() && gm(18).is_keyboard() && !gm(22).is_keyboard());
        let drums = Instrument {
            midi_program: 0,
            percussion: true,
        };
        assert!(
            Instrument::default().is_fretted(),
            "a new track is a guitar"
        );
        assert!(!drums.is_keyboard() && !drums.is_fretted());
    }

    #[test]
    fn pitch_with_capo() {
        let mut staff = Staff {
            tuning: Staff::standard_guitar_tuning(),
            ..Staff::default()
        };
        assert_eq!(staff.pitch_of(6, 0), Some(Pitch(40)));
        assert_eq!(staff.pitch_of(1, 5), Some(Pitch(69)));
        staff.capo = 2;
        assert_eq!(staff.pitch_of(1, 0), Some(Pitch(66)));
        assert_eq!(staff.pitch_of(0, 0), None);
        assert_eq!(staff.pitch_of(7, 0), None);
        assert_eq!(staff.pitch_of(1, 200), None);
    }
}
