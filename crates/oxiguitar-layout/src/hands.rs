// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Keyboard parts on a grand staff: which hand plays each note.
//!
//! Guitar Pro 3–5 (and many later files) keep a piano or organ part on one
//! staff. Shown on a single treble staff, the left hand sinks under many
//! ledger lines, so keyboard tracks are shown on a grand staff (Mateus,
//! 16 Oct 2026): a treble staff for the right hand and a bass staff for the
//! left, joined by a brace.
//!
//! The notes are split by hand range, not at a fixed note, so a line that
//! wanders around middle C stays on one staff (Mateus chose this over a
//! fixed split). Each voice is followed on its own, beat by beat:
//!
//! - A chord that one hand can span (an octave and a little more) goes to
//!   one hand: the right if it lies clearly above the current split point,
//!   the left if clearly below, and otherwise the hand that played the
//!   voice's previous chord, so a melody does not jump between staves.
//! - A wider chord is cut at the gap that best separates two hands, weighing
//!   the size of the gap against its distance from the split point.
//! - The split point starts at middle C, follows the gaps of wide chords, and
//!   drifts back towards middle C; it stays between F3 and G4.
//!
//! A hand with nothing in a voice's bar gets a whole-bar rest there (the
//! engraver hides secondary voices that hold only rests).

use oxiguitar_model::{
    Bar, Beat, BeatKind, Duration, NotePlace, NoteValue, Pitch, Ratio, Staff, Voice,
};

/// Middle C.
const MIDDLE_C: f32 = 60.0;
/// The split point stays between F3 and G4.
const SPLIT_RANGE: (f32, f32) = (53.0, 67.0);
/// The widest chord one hand takes, in semitones.
const HAND_SPAN: u8 = 14;
/// How far from the split point a one-hand chord must lie to change hands.
const HYSTERESIS: f32 = 3.0;

/// Which staff of the grand staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hand {
    Right,
    Left,
}

/// The right-hand and left-hand staves of `staff`: copies holding only that
/// hand's notes, with whole-bar rests where a hand has nothing in a voice's
/// bar. Clefs are left to the caller.
pub(crate) fn split(staff: &Staff) -> (Staff, Staff) {
    let mut right = staff.clone();
    let mut left = staff.clone();
    let voices = staff.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
    for vi in 0..voices {
        let mut state = State::default();
        for (bi, bar) in staff.bars.iter().enumerate() {
            let Some(voice) = bar.voices.get(vi) else {
                continue;
            };
            let hands: Vec<Vec<Hand>> = voice
                .beats
                .iter()
                .map(|b| state.assign(&pitches(staff, b)))
                .collect();
            for (out, hand) in [(&mut right, Hand::Right), (&mut left, Hand::Left)] {
                if let Some(v) = out.bars.get_mut(bi).and_then(|b| b.voices.get_mut(vi)) {
                    *v = keep(voice, &hands, hand);
                }
            }
        }
    }
    (right, left)
}

/// The sounding pitch of each note of a beat, in the beat's note order.
/// `None` for a note with no pitch (a note on a string that does not exist).
fn pitches(staff: &Staff, beat: &Beat) -> Vec<Option<u8>> {
    beat.notes
        .iter()
        .map(|n| match n.place {
            NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret).map(|p| p.0),
            NotePlace::Pitched(Pitch(p)) => Some(p),
        })
        .collect()
}

/// The voice with only `hand`'s notes. A beat that loses all its notes
/// becomes a rest; a voice left without notes becomes one whole-bar rest.
fn keep(voice: &Voice, hands: &[Vec<Hand>], hand: Hand) -> Voice {
    let mut out = voice.clone();
    let mut any = false;
    for (beat, hs) in out.beats.iter_mut().zip(hands) {
        if beat.kind != BeatKind::Notes || beat.notes.is_empty() {
            continue;
        }
        let mut i = 0;
        beat.notes.retain(|_| {
            let mine = hs.get(i).is_none_or(|h| *h == hand);
            i += 1;
            mine
        });
        if beat.notes.is_empty() {
            beat.kind = BeatKind::Rest;
            beat.effects = Default::default();
        } else {
            any = true;
        }
    }
    merge_rests(&mut out);
    // A whole-bar rest instead of beat-by-beat rests, unless a beat carries
    // a chord name or text that must stay where it is.
    let marked = out
        .beats
        .iter()
        .any(|b| b.chord.is_some() || !b.text.is_empty());
    if !any && !marked && out.beats.iter().any(|b| b.grace.is_none()) {
        out.beats = vec![Beat {
            kind: BeatKind::Rest,
            duration: Duration::new(NoteValue::Whole),
            ..Beat::default()
        }];
    }
    out
}

/// Joins runs of plain rests (no tuplet, chord name or text) into the
/// fewest rests that each start on a multiple of their own length, as a
/// copyist would write them: four sixteenth rests on a beat become a
/// quarter rest.
fn merge_rests(voice: &mut Voice) {
    let plain = |b: &Beat| {
        b.kind == BeatKind::Rest
            && b.grace.is_none()
            && b.duration.tuplets.is_empty()
            && b.chord.is_none()
            && b.text.is_empty()
    };
    let mut out: Vec<Beat> = Vec::with_capacity(voice.beats.len());
    let mut onset = Ratio::ZERO;
    let mut i = 0;
    let beats = std::mem::take(&mut voice.beats);
    while i < beats.len() {
        let Some(beat) = beats.get(i) else { break };
        if !plain(beat) {
            onset = onset + beat_length(beat);
            out.push(beat.clone());
            i += 1;
            continue;
        }
        let start = onset;
        let mut total = Ratio::ZERO;
        let mut j = i;
        while let Some(b) = beats.get(j).filter(|b| plain(b)) {
            total = total + beat_length(b);
            j += 1;
        }
        match fill(start, total) {
            Some(values) => out.extend(values.into_iter().map(|value| Beat {
                kind: BeatKind::Rest,
                duration: Duration::new(value),
                ..Beat::default()
            })),
            None => out.extend(beats[i..j].iter().cloned()),
        }
        onset = start + total;
        i = j;
    }
    voice.beats = out;
}

fn beat_length(b: &Beat) -> Ratio {
    if b.grace.is_some() {
        Ratio::ZERO
    } else {
        b.duration.length().unwrap_or(Ratio::ZERO)
    }
}

/// Rest values filling `total` from `start`, each on a multiple of its own
/// length; `None` if plain values cannot do it.
fn fill(mut at: Ratio, total: Ratio) -> Option<Vec<NoteValue>> {
    const VALUES: [NoteValue; 7] = [
        NoteValue::Whole,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
        NoteValue::ThirtySecond,
        NoteValue::SixtyFourth,
    ];
    let end = at.checked_add(total)?;
    let mut out = Vec::new();
    while at < end {
        let left = end.checked_sub(at)?;
        let value = VALUES.into_iter().find(|v| {
            let len = v.length();
            len <= left && at.checked_div(len).is_some_and(|q| q.denom() == 1)
        })?;
        out.push(value);
        at = at.checked_add(value.length())?;
        if out.len() > 64 {
            return None;
        }
    }
    Some(out)
}

/// What the split remembers from one beat of a voice to the next.
struct State {
    split: f32,
    last: Option<Hand>,
}

impl Default for State {
    fn default() -> Self {
        State {
            split: MIDDLE_C,
            last: None,
        }
    }
}

impl State {
    /// The hand of each note of a chord, in the chord's note order.
    fn assign(&mut self, pitches: &[Option<u8>]) -> Vec<Hand> {
        let mut known: Vec<u8> = pitches.iter().flatten().copied().collect();
        if known.is_empty() {
            return vec![self.last.unwrap_or(Hand::Right); pitches.len()];
        }
        known.sort_unstable();
        let (lo, hi) = (known[0], known[known.len() - 1]);
        // Drift back towards middle C.
        self.split += (MIDDLE_C - self.split) * 0.05;
        if hi - lo <= HAND_SPAN {
            let mean = known.iter().map(|&p| f32::from(p)).sum::<f32>() / known.len() as f32;
            let hand = if mean >= self.split + HYSTERESIS {
                Hand::Right
            } else if mean < self.split - HYSTERESIS {
                Hand::Left
            } else {
                self.last.unwrap_or(if mean >= self.split {
                    Hand::Right
                } else {
                    Hand::Left
                })
            };
            self.last = Some(hand);
            return vec![hand; pitches.len()];
        }
        let cut = self.best_cut(&known);
        // Notes up to the cut go to the left hand.
        let mid = (f32::from(known[cut - 1]) + f32::from(known[cut])) / 2.0;
        self.split = (0.7 * self.split + 0.3 * mid).clamp(SPLIT_RANGE.0, SPLIT_RANGE.1);
        self.last = Some(Hand::Right);
        let boundary = known[cut];
        pitches
            .iter()
            .map(|p| match p {
                Some(p) if *p < boundary => Hand::Left,
                _ => Hand::Right,
            })
            .collect()
    }

    /// Where to cut a sorted chord wider than a hand: the index of the first
    /// right-hand note. Big gaps near the split point win; a cut that leaves
    /// either hand wider than it can reach is penalised.
    fn best_cut(&self, sorted: &[u8]) -> usize {
        let n = sorted.len();
        let mut best = (f32::NEG_INFINITY, 1);
        for k in 1..n {
            let gap = f32::from(sorted[k] - sorted[k - 1]);
            let mid = (f32::from(sorted[k]) + f32::from(sorted[k - 1])) / 2.0;
            let mut score = gap - (mid - self.split).abs() * 0.35;
            if sorted[k - 1] - sorted[0] > HAND_SPAN {
                score -= 12.0;
            }
            if sorted[n - 1] - sorted[k] > HAND_SPAN {
                score -= 12.0;
            }
            if score > best.0 {
                best = (score, k);
            }
        }
        best.1
    }
}

/// A copy of `staff` without the marks that belong only once to a grand
/// staff (on its top staff): chord names, beat text and dynamics.
pub(crate) fn without_shared_marks(staff: &Staff) -> Staff {
    let mut out = staff.clone();
    for beat in out
        .bars
        .iter_mut()
        .flat_map(|b: &mut Bar| b.voices.iter_mut())
        .flat_map(|v| v.beats.iter_mut())
    {
        beat.chord = None;
        beat.text.clear();
        beat.effects.dynamic = None;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::Note;

    fn chord(pitches: &[u8]) -> Beat {
        Beat {
            kind: BeatKind::Notes,
            duration: Duration::new(NoteValue::Quarter),
            notes: pitches
                .iter()
                .map(|&p| Note {
                    place: NotePlace::Pitched(Pitch(p)),
                    ..Note::fretted(1, 0)
                })
                .collect(),
            ..Beat::default()
        }
    }

    fn staff(bars: &[&[&[u8]]]) -> Staff {
        Staff {
            bars: bars
                .iter()
                .map(|beats| Bar {
                    voices: vec![Voice {
                        beats: beats.iter().map(|p| chord(p)).collect(),
                    }],
                    ..Bar::default()
                })
                .collect(),
            ..Staff::default()
        }
    }

    fn notes(s: &Staff, bar: usize) -> Vec<Vec<u8>> {
        s.bars[bar].voices[0]
            .beats
            .iter()
            .map(|b| {
                b.notes
                    .iter()
                    .filter_map(|n| match n.place {
                        NotePlace::Pitched(p) => Some(p.0),
                        NotePlace::Fretted { .. } => None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_wide_chord_splits_between_the_hands() {
        // C2 E2 G2 under C5 E5 G5.
        let (r, l) = split(&staff(&[&[&[36, 40, 43, 72, 76, 79]]]));
        assert_eq!(notes(&r, 0), vec![vec![72, 76, 79]]);
        assert_eq!(notes(&l, 0), vec![vec![36, 40, 43]]);
    }

    #[test]
    fn a_melody_around_middle_c_stays_on_one_staff() {
        // E4 D4 C4 B3 C4 D4: within a few notes of middle C.
        let line: &[&[u8]] = &[&[64], &[62], &[60], &[59], &[60], &[62]];
        let (r, l) = split(&staff(&[line]));
        assert_eq!(notes(&r, 0).iter().filter(|n| !n.is_empty()).count(), 6);
        // The left hand rests the whole bar.
        assert_eq!(l.bars[0].voices[0].beats.len(), 1);
        assert_eq!(l.bars[0].voices[0].beats[0].kind, BeatKind::Rest);
    }

    #[test]
    fn a_low_bass_line_goes_to_the_left_hand() {
        let line: &[&[u8]] = &[&[36], &[43], &[40], &[45]];
        let (r, l) = split(&staff(&[line]));
        assert!(notes(&l, 0).iter().all(|n| n.len() == 1));
        assert_eq!(r.bars[0].voices[0].beats[0].kind, BeatKind::Rest);
    }

    #[test]
    fn a_beat_one_hand_leaves_becomes_a_rest() {
        // Low then high: each hand rests on the other's beat.
        let line: &[&[u8]] = &[&[36], &[84], &[38], &[86]];
        let (r, l) = split(&staff(&[line]));
        let kinds = |s: &Staff| -> Vec<BeatKind> {
            s.bars[0].voices[0].beats.iter().map(|b| b.kind).collect()
        };
        assert_eq!(
            kinds(&r),
            vec![
                BeatKind::Rest,
                BeatKind::Notes,
                BeatKind::Rest,
                BeatKind::Notes
            ]
        );
        assert_eq!(
            kinds(&l),
            vec![
                BeatKind::Notes,
                BeatKind::Rest,
                BeatKind::Notes,
                BeatKind::Rest
            ]
        );
    }

    #[test]
    fn rests_left_by_the_other_hand_merge() {
        // One right-hand quarter, then the left hand plays twelve sixteenths.
        let mut beats: Vec<&[u8]> = vec![&[72]];
        beats.extend(std::iter::repeat_n(&[36u8][..], 12));
        let mut s = staff(&[&beats]);
        s.bars[0].voices[0].beats[0].duration = Duration::new(NoteValue::Quarter);
        for b in s.bars[0].voices[0].beats.iter_mut().skip(1) {
            b.duration = Duration::new(NoteValue::Sixteenth);
        }
        let (r, _) = split(&s);
        let values: Vec<NoteValue> = r.bars[0].voices[0]
            .beats
            .iter()
            .map(|b| b.duration.value)
            .collect();
        // After the quarter: a quarter rest and a half rest (beats 2 and 3–4).
        assert_eq!(
            values,
            vec![NoteValue::Quarter, NoteValue::Quarter, NoteValue::Half]
        );
    }

    #[test]
    fn rests_fill_on_their_own_beats() {
        let r = |n, d| Ratio::new(n, d).unwrap();
        assert_eq!(
            fill(r(1, 8), r(7, 8)),
            Some(vec![NoteValue::Eighth, NoteValue::Quarter, NoteValue::Half])
        );
        assert_eq!(fill(Ratio::ZERO, r(1, 1)), Some(vec![NoteValue::Whole]));
        assert_eq!(fill(Ratio::ZERO, r(1, 3)), None);
    }

    #[test]
    fn shared_marks_are_taken_off() {
        let mut s = staff(&[&[&[60]]]);
        s.bars[0].voices[0].beats[0].chord = Some(0);
        s.bars[0].voices[0].beats[0].text = "rit.".to_owned();
        let bare = without_shared_marks(&s);
        assert_eq!(bare.bars[0].voices[0].beats[0].chord, None);
        assert!(bare.bars[0].voices[0].beats[0].text.is_empty());
    }
}
