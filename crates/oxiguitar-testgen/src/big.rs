// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A long band score for the speed budget: any number of bars, eight
//! tracks as a rock song has them, as dense as real files of that size.

use std::fmt::Write as _;

use crate::song::{Diagram, Kind, MasterBar, Song, Track};

/// A small, fixed pseudo-random sequence (xorshift), so the score is the
/// same on every run and every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// A chord of the progression: name, diagram (lowest string first), root
/// for the bass (string, fret), and the chord's notes for the keyboards
/// (MIDI).
type Chord = (&'static str, &'static str, (u8, u8), [u8; 3]);

/// The progression, a chord every two bars.
const CHORDS: [Chord; 4] = [
    ("Am", "x02210", (3, 0), [57, 60, 64]),
    ("F", "133211", (4, 1), [53, 57, 60]),
    ("C", "x32010", (3, 3), [55, 60, 64]),
    ("G", "320003", (4, 3), [55, 59, 62]),
];

/// A score of `bars` bars and eight tracks: lead, rhythm and acoustic
/// guitars, bass, piano, strings, voice with lyrics, and drums. Every
/// track plays in every bar, sixteenth notes in the lead.
pub fn band_song(bars: usize) -> Song {
    let mut song = Song::new("Speed Budget", bars);
    song.artist = "The OxiGuitar contributors".to_owned();
    song.album = "Generated test scores".to_owned();
    song.words = "OxiGuitar".to_owned();
    song.music = "OxiGuitar".to_owned();
    song.tabber = "oxiguitar-testgen".to_owned();
    song.tempo = 120;
    song.tempo_label = "Moderato".to_owned();
    song.key(0, 0, true);
    for (i, mb) in song.bars.iter_mut().enumerate() {
        if i % 16 == 0 {
            let n = i / 16;
            let letter = char::from(b'A' + (n % 26) as u8).to_string();
            let text = ["Verse", "Chorus", "Bridge", "Solo"][n % 4];
            mb.section = Some((letter, text.to_owned()));
        }
        if i % 32 == 8 {
            mb.repeat_start = true;
        }
        if i % 32 == 15 {
            *mb = MasterBar {
                repeat_end: Some(2),
                ..mb.clone()
            };
        }
    }
    if bars > 100 {
        song.tempo_changes.push((bars / 2, 0.0, 132));
    }

    let mut rng = Rng(0x0815_2026);
    let chord_of = |bar: usize| CHORDS[(bar / 2) % CHORDS.len()];

    let mut lead = Track::new("Lead Guitar", "Lead", Kind::guitar());
    let mut rhythm = Track::new("Rhythm Guitar", "Rhy.", Kind::guitar());
    for (name, frets, _, _) in CHORDS {
        rhythm.chord(Diagram::new(name, frets));
    }
    let mut acoustic = Track::new("Acoustic Guitar", "Ac.", Kind::acoustic());
    acoustic.notation(2);
    let mut bass = Track::new("Bass", "Bs.", Kind::bass());
    let mut piano = Track::new("Piano", "Pno.", Kind::piano());
    let mut strings = Track::new(
        "Strings",
        "Str.",
        Kind::Pitched {
            program: 48,
            bass_clef: false,
        },
    );
    let mut voice = Track::new(
        "Voice",
        "Vo.",
        Kind::Pitched {
            program: 54,
            bass_clef: false,
        },
    );
    let mut drums = Track::new("Drums", "Dr.", Kind::Drums);

    // A-minor pentatonic in fifth position, highest string first.
    const SCALE: [(u8, u8); 12] = [
        (6, 5),
        (6, 8),
        (5, 5),
        (5, 7),
        (4, 5),
        (4, 7),
        (3, 5),
        (3, 7),
        (2, 5),
        (2, 8),
        (1, 5),
        (1, 8),
    ];
    // One line of text notation per voice.
    let mut parts: [String; 9] = Default::default();
    let [l, r, a, b, p_rh, p_lh, s, v, d] = &mut parts;
    let mut step = 6;
    let mut lyric = String::new();
    for bar in 0..bars {
        let (name, _, root, keys) = chord_of(bar);
        let ci = CHORDS.iter().position(|c| c.0 == name).unwrap_or(0);
        if bar > 0 {
            for t in [
                &mut *l, &mut *r, &mut *a, &mut *b, &mut *p_rh, &mut *p_lh, &mut *s, &mut *v,
                &mut *d,
            ] {
                t.push_str(" | ");
            }
        }

        // Lead: sixteenths in solos, eighths otherwise, with effects.
        let solo = (bar / 16) % 4 == 3;
        let (count, value) = if solo { (16, "s") } else { (8, "e") };
        l.push_str(value);
        for k in 0..count {
            step = (step + rng.below(5)).saturating_sub(2).min(SCALE.len() - 1);
            let (st, fr) = SCALE[step];
            let fx = match rng.below(40) {
                0 => "h",
                1 => "b",
                2 => "v",
                3 => "/",
                4 if k == count - 1 => "R",
                _ => "",
            };
            let _ = write!(l, " {st}:{fr}{fx}");
        }

        // Rhythm: palm-muted eighths on the root, a strummed chord each
        // half bar.
        let _ = write!(r, "@c{ci} @down q ");
        let shape = chord_shape(CHORDS[ci].1);
        let _ = write!(r, "{shape} e");
        for _ in 0..4 {
            // The bass's E and A strings are the guitar's sixth and fifth.
            let _ = write!(r, " {}:{}m", root.0 + 2, root.1);
        }
        let _ = write!(r, " q {shape}");

        // Acoustic: let-ring arpeggio.
        a.push('e');
        for (k, string) in [6u8, 4, 3, 2, 1, 2, 3, 4].iter().enumerate() {
            let fret = arpeggio_fret(CHORDS[ci].1, *string);
            let fx = if k == 0 { "l" } else { "" };
            let _ = write!(a, " {string}:{fret}{fx}");
        }

        // Bass: eighths on the root, an octave jump at the end.
        b.push('e');
        for k in 0..8 {
            if k == 6 {
                let _ = write!(b, " {}:{}", root.0 - 2, root.1 + 2);
            } else {
                let _ = write!(b, " {}:{}", root.0, root.1);
            }
        }

        // Piano: chords in the right hand, roots in the left (one staff,
        // two voices; the engraver splits the hands).
        let chord: Vec<String> = keys.iter().map(|k| midi_name(k + 12)).collect();
        let _ = write!(p_rh, "q {c} {c} e {c} {c} q {c}", c = chord.join("+"));
        let low = midi_name(keys[0] - 12);
        let _ = write!(p_lh, "h {low} {low}");

        // Strings: whole-bar chords, ties across pairs of bars.
        let tie = if bar % 2 == 1 { "~" } else { "" };
        let held: Vec<String> = keys
            .iter()
            .map(|k| format!("{tie}{}", midi_name(*k)))
            .collect();
        let _ = write!(s, "w {}", held.join("+"));

        // Voice: a quarter-and-eighth melody on chord tones, with lyrics.
        let tones: Vec<String> = keys.iter().map(|k| midi_name(k + 12)).collect();
        if bar % 4 == 3 {
            let _ = write!(v, "h {} r", tones[0]);
            lyric.push_str("la. ");
        } else {
            let _ = write!(
                v,
                "q {} e {} {} q {} {}",
                tones[0], tones[1], tones[2], tones[1], tones[0]
            );
            lyric.push_str("la la la la la ");
        }

        // Drums: kick, snare and hi-hat eighths, a crash every 8 bars.
        d.push('e');
        for k in 0..8 {
            let mut hit = vec!["d42"];
            if k == 0 && bar % 8 == 0 {
                hit[0] = "d49";
            }
            if k % 4 == 0 {
                hit.push("d36");
            }
            if k % 4 == 2 {
                hit.push("d38");
            }
            if rng.chance(10) {
                hit.push("d36");
                hit.dedup();
            }
            let _ = write!(d, " {}", hit.join("+"));
        }
    }
    lead.voice(0, l);
    rhythm.voice(0, r);
    acoustic.voice(0, a);
    bass.voice(0, b);
    piano.voice(0, p_rh).voice(1, p_lh);
    strings.voice(0, s);
    voice.voice(0, v).lyrics(0, lyric.trim());
    drums.voice(0, d);
    for t in [lead, rhythm, bass, piano, strings, voice, acoustic, drums] {
        song.track(t);
    }
    song
}

/// A chord as fretted notes from a diagram written lowest string first.
fn chord_shape(frets: &str) -> String {
    let n = frets.len() as u8;
    frets
        .chars()
        .enumerate()
        .filter_map(|(i, c)| {
            let f = c.to_digit(10)?;
            Some(format!("{}:{f}", n - i as u8))
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// The diagram's fret on `string` (1 = highest), or open if not played.
fn arpeggio_fret(frets: &str, string: u8) -> u32 {
    let n = frets.len();
    frets
        .chars()
        .nth(n - usize::from(string))
        .and_then(|c| c.to_digit(10))
        .unwrap_or(0)
}

/// A MIDI pitch as the notation writes it ("C#4").
pub(crate) fn midi_name(midi: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(midi % 12)],
        i32::from(midi / 12) - 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_full_tracks() {
        let song = band_song(32);
        assert_eq!(song.tracks.len(), 8);
        for t in &song.tracks {
            for v in &t.voices {
                assert_eq!(v.len(), 32, "{}", t.name);
            }
        }
        assert_eq!(midi_name(60), "C4");
        assert_eq!(midi_name(61), "C#4");
        assert_eq!(chord_shape("x02210"), "5:0+4:2+3:2+2:1+1:0");
    }
}
