// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The track dialog's and the Transpose dialog's fields (M3 slice 5),
//! without window code: a track's settings as the dialog shows them
//! (instrument, strings, tuning as a key and a pattern, capo, frets) and
//! back, and a transposition from its fields.

use oxiguitar_edit::{Scope, TrackSettings, Transposition};
use oxiguitar_model::tuning::{self, Pattern};
use oxiguitar_model::{Instrument, Pitch, Track};

/// The General MIDI instruments, by program, as the dialog lists them;
/// then the drum kit.
pub(crate) const INSTRUMENTS: [&str; 129] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavinet",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Nylon-string Guitar",
    "Steel-string Guitar",
    "Jazz Electric Guitar",
    "Clean Electric Guitar",
    "Muted Electric Guitar",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Fingered Electric Bass",
    "Picked Electric Bass",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Square Lead",
    "Sawtooth Lead",
    "Calliope Lead",
    "Chiff Lead",
    "Charang Lead",
    "Voice Lead",
    "Fifths Lead",
    "Bass and Lead",
    "New Age Pad",
    "Warm Pad",
    "Polysynth Pad",
    "Choir Pad",
    "Bowed Pad",
    "Metallic Pad",
    "Halo Pad",
    "Sweep Pad",
    "Rain",
    "Soundtrack",
    "Crystal",
    "Atmosphere",
    "Brightness",
    "Goblins",
    "Echoes",
    "Sci-fi",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bagpipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
    "Drum Kit",
];

/// The string counts the dialog offers: none, then 3 to 10.
pub(crate) const STRING_COUNTS: [usize; 9] = [0, 3, 4, 5, 6, 7, 8, 9, 10];

/// The track dialog's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrackForm {
    pub(crate) name: String,
    pub(crate) short_name: String,
    /// Index in [`INSTRUMENTS`]: the program, or 128 for drums.
    pub(crate) instrument: i32,
    /// Index in [`STRING_COUNTS`].
    pub(crate) strings: i32,
    /// The tuning's key, a pitch class (0 = C).
    pub(crate) key: i32,
    /// Index in [`Pattern::ALL`].
    pub(crate) pattern: i32,
    /// The notes from the lowest string, for Other ("D2 A2 D3 G3 A3 D4").
    pub(crate) notes: String,
    pub(crate) capo: i32,
    pub(crate) frets: i32,
    /// Keep the notes' pitches (else their frets) when the strings change.
    pub(crate) keep_pitches: bool,
}

impl TrackForm {
    /// The fields for a track.
    pub(crate) fn of(track: &Track) -> TrackForm {
        let s = TrackSettings::of(track);
        let bass = s.instrument.is_bass();
        let (pattern, key) = tuning::recognise(&s.tuning, bass);
        TrackForm {
            name: s.name,
            short_name: s.short_name,
            instrument: if s.instrument.percussion {
                128
            } else {
                i32::from(s.instrument.midi_program)
            },
            strings: STRING_COUNTS
                .iter()
                .position(|n| *n == s.tuning.len())
                .map_or(0, |i| i as i32),
            key: i32::from(key),
            pattern: Pattern::ALL
                .iter()
                .position(|p| *p == pattern)
                .map_or(4, |i| i as i32),
            notes: notes_text(&s.tuning),
            capo: i32::from(s.capo),
            frets: i32::from(s.fret_count),
            keep_pitches: true,
        }
    }

    fn instrument(&self) -> Instrument {
        match u8::try_from(self.instrument) {
            Ok(p) if p < 128 => Instrument {
                midi_program: p,
                percussion: false,
            },
            _ => Instrument {
                midi_program: 0,
                percussion: true,
            },
        }
    }

    /// The tuning the fields give, highest string first, or why they
    /// give none.
    pub(crate) fn tuning(&self) -> Result<Vec<Pitch>, String> {
        let instrument = self.instrument();
        let strings = STRING_COUNTS
            .get(usize::try_from(self.strings).unwrap_or(0))
            .copied()
            .unwrap_or(0);
        if instrument.percussion || strings == 0 {
            return Ok(Vec::new());
        }
        let pattern = Pattern::ALL
            .get(usize::try_from(self.pattern).unwrap_or(4))
            .copied()
            .unwrap_or(Pattern::Other);
        if pattern == Pattern::Other {
            let low_first = parse_notes(&self.notes)?;
            if low_first.len() != strings {
                return Err(format!("{} notes for {strings} strings", low_first.len()));
            }
            return Ok(low_first.into_iter().rev().collect());
        }
        let key = u8::try_from(self.key.rem_euclid(12)).unwrap_or(4);
        tuning::tuning(pattern, key, strings, instrument.is_bass()).ok_or_else(|| {
            "Open tunings are for six-string guitars: choose Other for these strings".to_owned()
        })
    }

    /// What the tuning comes to, for the dialog: its notes from the
    /// lowest string and its name, or why it is wrong.
    pub(crate) fn preview(&self) -> String {
        match self.tuning() {
            Ok(t) if t.is_empty() => "No strings: the notes are shown on the staff only".to_owned(),
            Ok(t) => format!(
                "{} · {}",
                notes_text(&t),
                tuning::name(&t, self.instrument().is_bass())
            ),
            Err(e) => e,
        }
    }

    /// The settings the fields ask for, or why they can't be.
    pub(crate) fn to_settings(&self) -> Result<TrackSettings, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("A track needs a name".to_owned());
        }
        Ok(TrackSettings {
            name: name.to_owned(),
            short_name: self.short_name.trim().to_owned(),
            instrument: self.instrument(),
            capo: u8::try_from(self.capo.clamp(0, 24)).unwrap_or(0),
            fret_count: u8::try_from(self.frets.clamp(12, 36)).unwrap_or(24),
            tuning: self.tuning()?,
            keep_pitches: self.keep_pitches,
        })
    }
}

/// A tuning's notes from the lowest string, with octaves: "E2 A2 D3 G3
/// B3 E4".
pub(crate) fn notes_text(tuning_high_first: &[Pitch]) -> String {
    tuning_high_first
        .iter()
        .rev()
        .map(|p| format!("{}{}", tuning::key_name(p.0 % 12), i32::from(p.0 / 12) - 1))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Notes from the lowest string ("D2 A2 D3", sharps as ♯ or #, flats as
/// ♭ or b), as MIDI pitches lowest first; a note without an octave takes
/// the first one above the note before it (D2 A D G A D).
pub(crate) fn parse_notes(text: &str) -> Result<Vec<Pitch>, String> {
    let mut out: Vec<Pitch> = Vec::new();
    for word in text.split_whitespace() {
        let mut chars = word.chars().peekable();
        let letter = chars.next().ok_or("an empty note")?;
        let mut pc: i32 = match letter.to_ascii_uppercase() {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return Err(format!("\"{word}\" is not a note")),
        };
        while let Some(&c) = chars.peek() {
            match c {
                '#' | '♯' => pc += 1,
                'b' | '♭' => pc -= 1,
                _ => break,
            }
            chars.next();
        }
        let rest: String = chars.collect();
        let midi = if rest.is_empty() {
            // The first one above the note before, or in octave 2.
            match out.last() {
                Some(prev) => {
                    let p = i32::from(prev.0);
                    let mut m = p - p.rem_euclid(12) + pc.rem_euclid(12);
                    while m <= p {
                        m += 12;
                    }
                    m
                }
                None => 36 + pc,
            }
        } else {
            let octave: i32 = rest
                .parse()
                .map_err(|_| format!("\"{word}\" is not a note"))?;
            (octave + 1) * 12 + pc
        };
        let p = u8::try_from(midi)
            .ok()
            .filter(|m| *m <= 127)
            .ok_or_else(|| format!("\"{word}\" is out of range"))?;
        out.push(Pitch(p));
    }
    if out.is_empty() {
        return Err("No notes".to_owned());
    }
    Ok(out)
}

/// A transposition from the Transpose dialog's fields.
pub(crate) fn transposition(
    amount: i32,
    steps: bool,
    keep_fingering: bool,
    scope: Scope,
) -> Option<Transposition> {
    let amount = i8::try_from(amount.clamp(-48, 48))
        .ok()
        .filter(|a| *a != 0)?;
    Some(Transposition {
        amount,
        diatonic: steps,
        keep_fingering,
        scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::Staff;

    fn guitar() -> Track {
        Track {
            name: "Lead".into(),
            staves: vec![Staff {
                tuning: Staff::standard_guitar_tuning(),
                ..Staff::default()
            }],
            ..Track::default()
        }
    }

    #[test]
    fn a_guitar_round_trips_and_retunes() {
        let t = guitar();
        let f = TrackForm::of(&t);
        assert_eq!((f.instrument, f.strings, f.key, f.pattern), (25, 4, 4, 0));
        assert_eq!(f.notes, "E2 A2 D3 G3 B3 E4");
        assert_eq!(f.preview(), "E2 A2 D3 G3 B3 E4 · E Standard");
        let s = f.to_settings().unwrap();
        assert_eq!(s, TrackSettings::of(&t));
        // Drop D.
        let mut d = f.clone();
        d.pattern = 1;
        d.key = 2;
        assert_eq!(d.preview(), "D2 A2 D3 G3 B3 E4 · Drop D");
        // Other: DADGAD, octaves inferred.
        let mut o = f.clone();
        o.pattern = 4;
        o.notes = "D2 A D G A D".into();
        assert_eq!(o.preview(), "D2 A2 D3 G3 A3 D4 · D A D G A D");
        o.notes = "D2 A D".into();
        assert_eq!(o.preview(), "3 notes for 6 strings");
        // Seven strings, open: none.
        let mut seven = f.clone();
        seven.strings = 5;
        seven.pattern = 2;
        assert!(seven.to_settings().is_err());
        // Drums: no strings.
        let mut drums = f.clone();
        drums.instrument = 128;
        let s = drums.to_settings().unwrap();
        assert!(s.instrument.percussion && s.tuning.is_empty());
        let mut nameless = f;
        nameless.name = "  ".into();
        assert!(nameless.to_settings().is_err());
    }

    #[test]
    fn notes_parse() {
        let p = |s: &str| parse_notes(s).map(|v| v.iter().map(|p| p.0).collect::<Vec<_>>());
        assert_eq!(p("E2 A2 D3").unwrap(), [40, 45, 50]);
        assert_eq!(p("Eb2 Ab Db").unwrap(), [39, 44, 49]);
        assert_eq!(p("C♯3").unwrap(), [49]);
        assert!(p("H2").is_err());
        assert!(p("").is_err());
        assert_eq!(transposition(0, false, true, Scope::Track), None);
        assert_eq!(
            transposition(-3, true, false, Scope::All).unwrap().amount,
            -3
        );
    }
}
