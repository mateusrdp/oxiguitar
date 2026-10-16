// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Key signatures for files that do not set one (NOT-1).
//!
//! Most tabs leave every bar in C major, the format's default, whatever the
//! music. When a file sets no key anywhere, the engraver infers one per bar
//! by counting accidentals: for each of the 12 key signatures from six flats
//! to five sharps, how much of the bar's music (weighted by duration, across
//! every pitched track) falls outside that key's scale, as a share of the
//! bar's music. A best path through the song then picks one key per bar,
//! where changing key costs as much as two and a half bars of music wholly
//! out of key (less at section marks, double bars and repeats, where
//! modulations usually happen; more the further the new key is round the
//! circle of fifths), so a chromatic passage does not change the key but a
//! real modulation does. Ties go to the signature with fewer accidentals.
//!
//! The inferred keys are used only if they spare at least 2.5 % of the music
//! its accidentals; otherwise (chromatic riffs, atonal passages) the song
//! stays in C. A file that sets any key other than C is trusted as it is.

use oxiguitar_model::{BeatKind, NotePlace, Score};

/// Key signatures considered, in fifths: six flats to five sharps. G flat
/// major (E flat minor) stands for its enharmonic twin F sharp major, as
/// is usual and as songs in E flat tuning are written.
const KEYS: std::ops::RangeInclusive<i8> = -6..=5;

/// The cost of changing key, in bars whose music is wholly out of key (a
/// bar costs the share of its music outside the key).
const SWITCH: f32 = 2.5;
/// The cost of changing key where a section starts (marker, double bar,
/// repeat).
const SWITCH_AT_SECTION: f32 = 1.5;
/// A small cost per accidental of the signature, per bar, so ties go to the
/// simpler key.
const SIMPLICITY: f32 = 0.002;
/// An extra cost per step around the circle of fifths, so a change goes to a
/// nearby key rather than to its enharmonic neighbour across the circle
/// (B major and G flat major, say).
const DISTANCE: f32 = 0.2;
/// Inferred keys are used only if they spare this share of the music its
/// accidentals compared with C major; chromatic songs stay in C.
const MIN_GAIN: f32 = 0.025;

/// True if the file sets a key other than C major anywhere.
pub(crate) fn file_has_keys(score: &Score) -> bool {
    score
        .master_bars
        .iter()
        .any(|mb| mb.key_signature.fifths != 0)
}

/// The pitch classes of the major scale with `fifths` sharps (or flats if
/// negative), as a bit set.
fn scale(fifths: i8) -> u16 {
    let tonic = (7 * i32::from(fifths)).rem_euclid(12);
    [0, 2, 4, 5, 7, 9, 11]
        .iter()
        .fold(0u16, |set, step| set | 1 << ((tonic + step) % 12))
}

/// For each bar, how much music (in quarter notes, each note counted for at
/// most a half note) falls in each pitch class, written pitch, across every
/// pitched track. Tied notes, dead notes and drums do not count.
fn weights(score: &Score) -> Vec<[f32; 12]> {
    let mut out = vec![[0.0f32; 12]; score.master_bars.len()];
    for track in score.tracks.iter().filter(|t| !t.instrument.percussion) {
        for staff in &track.staves {
            for (bar, w) in staff.bars.iter().zip(out.iter_mut()) {
                for voice in &bar.voices {
                    for beat in voice.beats.iter().filter(|b| b.kind == BeatKind::Notes) {
                        let length = beat
                            .duration
                            .length()
                            .map_or(0.0, |l| l.to_f64() as f32 * 4.0);
                        let weight = if beat.grace.is_some() {
                            0.25
                        } else {
                            length.clamp(0.0, 2.0)
                        };
                        for n in beat.notes.iter().filter(|n| !n.tie && !n.effects.dead) {
                            let pitch = match n.place {
                                NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret),
                                NotePlace::Pitched(p) => Some(p),
                            };
                            if let Some(p) = pitch {
                                let written = i32::from(p.0) + i32::from(staff.transposition);
                                if let Some(slot) = w.get_mut(written.rem_euclid(12) as usize) {
                                    *slot += weight;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// The share of a bar's music outside the key with `fifths`, from 0 to 1
/// (a bar with little music counts for less).
fn outside_share(w: &[f32; 12], fifths: i8) -> f32 {
    let s = scale(fifths);
    let total: f32 = w.iter().sum();
    let outside: f32 = (0..12)
        .filter(|pc| s & (1 << pc) == 0)
        .map(|pc| w.get(pc).copied().unwrap_or(0.0))
        .sum();
    outside / total.max(4.0)
}

/// The share of the music, over the bars that have some, that the keys
/// `keys` spare its accidentals compared with C major throughout.
fn gain(bars: &[[f32; 12]], keys: &[i8]) -> f32 {
    let with_music = bars.iter().filter(|w| w.iter().sum::<f32>() > 0.0).count();
    let spared: f32 = bars
        .iter()
        .zip(keys)
        .map(|(w, k)| outside_share(w, 0) - outside_share(w, *k))
        .sum();
    spared / with_music.max(1) as f32
}

/// The key signature (in fifths) of every bar, inferred from the notes.
#[cfg(test)]
fn infer(score: &Score) -> Vec<i8> {
    infer_bars(score, &weights(score))
}

/// The best path of keys through the bars' pitch-class weights.
fn infer_bars(score: &Score, bars: &[[f32; 12]]) -> Vec<i8> {
    let keys: Vec<i8> = KEYS.collect();
    // The share of a bar's music outside the key, so dense bars (many
    // tracks) weigh no more than sparse ones; a bar with little music
    // counts for less.
    let cost = |w: &[f32; 12], k: i8| -> f32 {
        outside_share(w, k) + SIMPLICITY * f32::from(k.unsigned_abs())
    };
    let n = bars.len();
    if n == 0 {
        return Vec::new();
    }
    // Best total cost ending in each key, and where it came from.
    let mut best: Vec<f32> = Vec::with_capacity(keys.len());
    let mut from: Vec<Vec<usize>> = vec![vec![0; keys.len()]; n];
    if let Some(w) = bars.first() {
        for k in &keys {
            best.push(cost(w, *k));
        }
    }
    for i in 1..n {
        let Some(w) = bars.get(i) else { break };
        let switch = if starts_section(score, i) {
            SWITCH_AT_SECTION
        } else {
            SWITCH
        };
        let mut next = Vec::with_capacity(keys.len());
        for (j, k) in keys.iter().enumerate() {
            let (prev, came) = best
                .iter()
                .enumerate()
                .map(|(p, total)| {
                    let step = keys.get(p).map_or(0, |q| q.abs_diff(*k));
                    let change = if p == j {
                        0.0
                    } else {
                        switch + DISTANCE * f32::from(step)
                    };
                    (total + change, p)
                })
                .fold((f32::INFINITY, j), |a, b| if b.0 < a.0 { b } else { a });
            next.push(prev + cost(w, *k));
            if let Some(slot) = from.get_mut(i).and_then(|f| f.get_mut(j)) {
                *slot = came;
            }
        }
        best = next;
    }
    // Walk back from the cheapest end.
    let mut j = best
        .iter()
        .enumerate()
        .fold(
            (0, f32::INFINITY),
            |a, (i, c)| if *c < a.1 { (i, *c) } else { a },
        )
        .0;
    let mut out = vec![0i8; n];
    for i in (0..n).rev() {
        if let Some(slot) = out.get_mut(i) {
            *slot = keys.get(j).copied().unwrap_or(0);
        }
        j = from.get(i).and_then(|f| f.get(j)).copied().unwrap_or(j);
    }
    out
}

/// Whether bar `i` starts a section, where a key change is more likely.
fn starts_section(score: &Score, i: usize) -> bool {
    let here = score.master_bars.get(i);
    let before = i.checked_sub(1).and_then(|j| score.master_bars.get(j));
    here.is_some_and(|mb| mb.marker.is_some() || mb.repeat_start)
        || before.is_some_and(|mb| mb.double_bar || mb.repeat_end.is_some())
}

/// A copy of `score` with inferred key signatures, if the file sets none;
/// `None` if the file's own keys should be used.
pub(crate) fn with_inferred_keys(score: &Score) -> Option<Score> {
    if file_has_keys(score) {
        return None;
    }
    let bars = weights(score);
    let keys = infer_bars(score, &bars);
    if keys.iter().all(|k| *k == 0) || gain(&bars, &keys) < MIN_GAIN {
        return None;
    }
    let mut s = score.clone();
    for (mb, k) in s.master_bars.iter_mut().zip(keys) {
        mb.key_signature.fifths = k;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::{
        Bar, Beat, Duration, MasterBar, Note, NoteValue, Pitch, Staff, Track, Voice,
    };

    /// A one-track score: one bar per list of MIDI pitches, quarter notes.
    fn song(bars: &[&[u8]]) -> Score {
        let mut score = Score::new();
        let mut staff = Staff::default();
        for pitches in bars {
            score.master_bars.push(MasterBar::default());
            let beats = pitches
                .iter()
                .map(|p| Beat {
                    notes: vec![Note {
                        place: NotePlace::Pitched(Pitch(*p)),
                        ..Note::fretted(1, 0)
                    }],
                    duration: Duration::new(NoteValue::Quarter),
                    ..Beat::default()
                })
                .collect();
            staff.bars.push(Bar {
                voices: vec![Voice { beats }],
                ..Bar::default()
            });
        }
        score.tracks.push(Track {
            staves: vec![staff],
            ..Track::default()
        });
        score
    }

    const G_MAJOR: &[u8] = &[67, 71, 74, 66]; // G B D F♯
    const C_MAJOR: &[u8] = &[60, 64, 67, 65]; // C E G F
    const E_FLAT: &[u8] = &[63, 67, 70, 68]; // E♭ G B♭ A♭

    #[test]
    fn scales_are_right() {
        assert_eq!(scale(0), 0b1010_1011_0101); // C D E F G A B
        assert_eq!(scale(1) & (1 << 6), 1 << 6); // G major has F♯
        assert_eq!(scale(-1) & (1 << 10), 1 << 10); // F major has B♭
    }

    #[test]
    fn a_tune_in_g_gets_one_sharp() {
        let keys = infer(&song(&[G_MAJOR; 4]));
        assert_eq!(keys, vec![1; 4]);
    }

    #[test]
    fn music_without_accidentals_stays_in_c() {
        let bar: &[u8] = &[60, 62, 64, 67]; // C D E G: fits C, F and G; C is simplest
        let s = song(&[bar; 3]);
        assert_eq!(infer(&s), vec![0; 3]);
        assert!(with_inferred_keys(&s).is_none());
    }

    #[test]
    fn a_passing_accidental_does_not_change_the_key() {
        let mut bars: Vec<&[u8]> = vec![C_MAJOR; 8];
        bars[4] = &[60, 61, 62, 63]; // a chromatic run in one bar
        assert_eq!(infer(&song(&bars)), vec![0; 8]);
    }

    #[test]
    fn a_modulation_changes_the_key_where_it_happens() {
        let c: &[u8] = &[60, 64, 67, 71]; // C E G B
        let mut bars: Vec<&[u8]> = vec![c; 8];
        bars.extend([E_FLAT; 8]);
        let keys = infer(&song(&bars));
        assert_eq!(keys[..8], [0; 8]);
        assert_eq!(keys[8..], [-3; 8]);
    }

    #[test]
    fn keys_that_spare_few_accidentals_are_not_used() {
        // Every pitch class four times and one F♯: G major spares a single
        // accidental in 49 notes, too little to be worth a signature.
        let mut bar: Vec<u8> = (0..48).map(|i| 60 + i % 12).collect();
        bar.push(66);
        let s = song(&[bar.as_slice(); 8]);
        assert_eq!(infer(&s), vec![1; 8]);
        assert!(with_inferred_keys(&s).is_none());
    }

    #[test]
    fn songs_in_six_flats_are_written_in_g_flat_not_f_sharp() {
        let e_flat_minor: &[u8] = &[63, 65, 66, 70, 61, 59, 68]; // E♭ F G♭ B♭ D♭ C♭ A♭
        assert_eq!(infer(&song(&[e_flat_minor; 4])), vec![-6; 4]);
    }

    #[test]
    fn inferred_keys_reach_the_score() {
        let s = with_inferred_keys(&song(&[G_MAJOR; 3])).expect("a key");
        assert!(s.master_bars.iter().all(|mb| mb.key_signature.fifths == 1));
    }

    #[test]
    fn the_files_own_keys_are_kept() {
        let mut s = song(&[G_MAJOR; 2]);
        s.master_bars[1].key_signature.fifths = -2;
        assert!(with_inferred_keys(&s).is_none());
    }
}
