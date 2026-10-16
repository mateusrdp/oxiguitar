// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Time within a bar: columns of beats that start together, and beam groups
//! (NOT-2).

use oxiguitar_model::{Bar, BeatKind, NoteValue, Ratio, TimeSignature, Voice};

/// A beat of one voice, by voice and beat index in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BeatRef {
    pub(crate) voice: usize,
    pub(crate) beat: usize,
}

/// Beats of every voice that start at the same time. Standard notation and
/// tablature share columns, so the two stay aligned (VIEW-1).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Column {
    /// Start, in whole notes from the start of the bar.
    pub(crate) onset: Ratio,
    /// Time until the next column, or the end of the bar.
    pub(crate) span: Ratio,
    /// The beats that start here (grace beats excluded).
    pub(crate) beats: Vec<BeatRef>,
}

/// When each beat of a voice starts; grace beats get `None`.
pub(crate) fn onsets(voice: &Voice) -> Vec<Option<Ratio>> {
    let mut pos = Ratio::ZERO;
    voice
        .beats
        .iter()
        .map(|b| {
            if b.grace.is_some() {
                return None;
            }
            let here = pos;
            pos = pos + b.duration.length().unwrap_or(Ratio::ZERO);
            Some(here)
        })
        .collect()
}

/// The bar's length for layout: its time signature, or longer if a voice
/// overflows it (Guitar Pro allows that).
pub(crate) fn bar_length(bar: &Bar, ts: TimeSignature) -> Ratio {
    let expected = ts.bar_length().unwrap_or(Ratio::ONE);
    bar.voices
        .iter()
        .filter_map(Voice::length)
        .fold(expected, Ratio::max)
}

/// Whether voice `vi` of a bar is drawn. Voices after the first are left
/// out when they hold no notes, as Guitar Pro does: files often fill unused
/// voices with rests.
pub(crate) fn shown(bar: &Bar, vi: usize) -> bool {
    vi == 0
        || bar
            .voices
            .get(vi)
            .is_some_and(|v| v.beats.iter().any(|b| b.kind == BeatKind::Notes))
}

/// The columns of a bar, in time order. A bar with no beats at all gets one
/// column so it still has a width.
pub(crate) fn columns(bar: &Bar, length: Ratio) -> Vec<Column> {
    let mut cols: Vec<Column> = Vec::new();
    for (vi, voice) in bar.voices.iter().enumerate() {
        if !shown(bar, vi) {
            continue;
        }
        for (bi, onset) in onsets(voice).into_iter().enumerate() {
            let Some(onset) = onset else { continue };
            let r = BeatRef {
                voice: vi,
                beat: bi,
            };
            match cols.binary_search_by(|c| c.onset.cmp(&onset)) {
                Ok(i) => {
                    if let Some(c) = cols.get_mut(i) {
                        c.beats.push(r);
                    }
                }
                Err(i) => cols.insert(
                    i,
                    Column {
                        onset,
                        span: Ratio::ZERO,
                        beats: vec![r],
                    },
                ),
            }
        }
    }
    if cols.is_empty() {
        cols.push(Column {
            onset: Ratio::ZERO,
            span: length,
            beats: Vec::new(),
        });
    }
    let ends: Vec<Ratio> = cols
        .iter()
        .skip(1)
        .map(|c| c.onset)
        .chain(std::iter::once(length))
        .collect();
    for (c, end) in cols.iter_mut().zip(ends) {
        c.span = end
            .checked_sub(c.onset)
            .unwrap_or(Ratio::ZERO)
            .max(Ratio::ZERO);
    }
    cols
}

/// The length a beam group covers: a quarter note, or a dotted quarter in
/// compound time (6/8, 9/8, 12/8).
pub(crate) fn beam_unit(ts: TimeSignature) -> Ratio {
    let compound = ts.denominator == 8 && ts.numerator.is_multiple_of(3) && ts.numerator > 3;
    let unit = match ts.denominator {
        16 => Ratio::new(1, 8),
        8 if compound || ts.numerator == 3 => Ratio::new(3, 8),
        _ => Ratio::new(1, 4),
    };
    unit.unwrap_or(Ratio::ONE)
}

/// Groups of two or more beats of one voice that share a beam: consecutive
/// notes of an eighth or shorter, inside one beam unit. Returns beat indices.
pub(crate) fn beam_groups(voice: &Voice, ts: TimeSignature) -> Vec<Vec<usize>> {
    let unit = beam_unit(ts);
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_unit: Option<i64> = None;
    for (i, (beat, onset)) in voice.beats.iter().zip(onsets(voice)).enumerate() {
        let Some(onset) = onset else { continue };
        let beamable = beat.kind == BeatKind::Notes && beat.duration.value >= NoteValue::Eighth;
        let unit_index = onset
            .checked_div(unit)
            .map(|q| q.numer().div_euclid(q.denom()));
        if !beamable || unit_index != current_unit {
            if current.len() >= 2 {
                groups.push(std::mem::take(&mut current));
            }
            current.clear();
        }
        if beamable {
            current.push(i);
            current_unit = unit_index;
        } else {
            current_unit = None;
        }
    }
    if current.len() >= 2 {
        groups.push(current);
    }
    groups
}

/// Number of beams (or flags) a note value has: 1 for an eighth.
pub(crate) fn beam_count(value: NoteValue) -> usize {
    match value {
        NoteValue::Eighth => 1,
        NoteValue::Sixteenth => 2,
        NoteValue::ThirtySecond => 3,
        NoteValue::SixtyFourth => 4,
        NoteValue::OneTwentyEighth => 5,
        NoteValue::TwoFiftySixth => 6,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::{Beat, Duration, GraceKind};

    fn beat(v: NoteValue, kind: BeatKind) -> Beat {
        Beat {
            kind,
            duration: Duration::new(v),
            ..Beat::default()
        }
    }

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap_or(Ratio::ZERO)
    }

    const TWO_FOUR: TimeSignature = TimeSignature {
        numerator: 2,
        denominator: 4,
    };

    #[test]
    fn columns_merge_voices() {
        let v1 = Voice {
            beats: vec![
                beat(NoteValue::Eighth, BeatKind::Notes),
                beat(NoteValue::Eighth, BeatKind::Notes),
                beat(NoteValue::Quarter, BeatKind::Notes),
            ],
        };
        let v2 = Voice {
            beats: vec![
                beat(NoteValue::Quarter, BeatKind::Notes),
                beat(NoteValue::Quarter, BeatKind::Rest),
            ],
        };
        let bar = Bar {
            voices: vec![v1, v2],
            ..Bar::default()
        };
        let cols = columns(&bar, bar_length(&bar, TWO_FOUR));
        let onsets: Vec<Ratio> = cols.iter().map(|c| c.onset).collect();
        assert_eq!(onsets, vec![r(0, 1), r(1, 8), r(1, 4)]);
        assert_eq!(cols[0].beats.len(), 2);
        assert_eq!(cols[1].span, r(1, 8));
        assert_eq!(cols[2].span, r(1, 4));
    }

    #[test]
    fn beams_follow_the_beat() {
        let mut v = Voice {
            beats: vec![
                beat(NoteValue::Sixteenth, BeatKind::Notes),
                beat(NoteValue::Sixteenth, BeatKind::Notes),
                beat(NoteValue::Eighth, BeatKind::Notes),
                beat(NoteValue::Eighth, BeatKind::Notes),
                beat(NoteValue::Eighth, BeatKind::Rest),
            ],
        };
        assert_eq!(beam_groups(&v, TWO_FOUR), vec![vec![0, 1, 2]]);
        // A grace beat neither breaks nor joins a group.
        let mut grace = beat(NoteValue::Sixteenth, BeatKind::Notes);
        grace.grace = Some(GraceKind::BeforeBeat);
        v.beats.insert(1, grace);
        assert_eq!(beam_groups(&v, TWO_FOUR), vec![vec![0, 2, 3]]);
        // 6/8: three eighths per group.
        let six = Voice {
            beats: vec![beat(NoteValue::Eighth, BeatKind::Notes); 6],
        };
        let ts = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        assert_eq!(beam_groups(&six, ts), vec![vec![0, 1, 2], vec![3, 4, 5]]);
    }
}
