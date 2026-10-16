// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Planning one bar of one staff: where each note sits on the staff, which
//! way stems point, how wide each column must be (NOT-1, NOT-2, NOT-4).

use oxiguitar_model::{
    Bar, BeatKind, Clef, KeySignature, MasterBar, Note, NotePlace, NoteValue, Pitch, Ratio, Staff,
    Track, Tuplet,
};

use crate::draw::{G, Painter, glyphs};
use crate::pitch::{BarAccidentals, drum, spell, staff_step, written};
use crate::rhythm::{self, BeatRef, Column};

/// One note as it will be drawn.
#[derive(Clone, Debug)]
pub(crate) struct NoteInfo {
    /// Staff step: 0 is the bottom line.
    pub(crate) step: i32,
    /// Accidental to print: -2 to 2.
    pub(crate) accidental: Option<i8>,
    /// Column of the accidental, counted leftwards from the notehead.
    pub(crate) accidental_column: usize,
    /// Drawn on the far side of the stem (a second in a chord).
    pub(crate) shifted: bool,
    /// The sounding pitch, to follow ties.
    pub(crate) pitch: Option<Pitch>,
    /// Tied to the previous note.
    pub(crate) tie: bool,
    /// Dead note: an X notehead.
    pub(crate) dead: bool,
    /// String and tab label, for notes on strings.
    pub(crate) tab: Option<(u8, String)>,
}

/// One beat as it will be drawn.
#[derive(Clone, Debug)]
pub(crate) struct BeatInfo {
    /// Start within the bar; `None` for grace beats.
    pub(crate) onset: Option<Ratio>,
    pub(crate) kind: BeatKind,
    pub(crate) value: NoteValue,
    pub(crate) dots: u8,
    pub(crate) tuplet: Option<Tuplet>,
    /// Notes, lowest first.
    pub(crate) notes: Vec<NoteInfo>,
    /// Stem direction.
    pub(crate) up: bool,
    /// Index into the voice's beam groups, if beamed.
    pub(crate) group: Option<usize>,
    /// Chord name to show above the staff.
    pub(crate) chord: Option<String>,
    /// Free text above the staff.
    pub(crate) text: String,
}

/// Horizontal needs of one column, in points.
#[derive(Clone, Debug)]
pub(crate) struct ColPlan {
    pub(crate) column: Column,
    /// Room needed left and right of the notehead centre.
    pub(crate) left: f32,
    pub(crate) right: f32,
    /// Half the width of the widest tab number.
    pub(crate) tab_half: f32,
}

/// One bar of one staff, planned but not yet placed.
#[derive(Clone, Debug)]
pub(crate) struct BarPlan {
    /// Beats per voice, as drawn.
    pub(crate) voices: Vec<Vec<BeatInfo>>,
    /// Beam groups per voice: beat indices.
    pub(crate) groups: Vec<Vec<Vec<usize>>>,
    /// Columns and the room each needs on this staff.
    pub(crate) cols: Vec<ColPlan>,
    /// The bar's length for layout.
    pub(crate) length: Ratio,
    pub(crate) clef: Clef,
    /// Lowest and highest staff step reached by noteheads and stems.
    pub(crate) low: i32,
    pub(crate) high: i32,
    pub(crate) has_chords: bool,
    pub(crate) has_text: bool,
}

/// The distance a duration asks for, in staff spaces: grows with the
/// duration, but less than in proportion, as engravers space music.
pub(crate) fn duration_space(span: Ratio) -> f32 {
    let quarters = (span.to_f64() * 4.0).max(0.0) as f32;
    1.6 + 2.0 * quarters.powf(0.6)
}

pub(crate) struct Planner<'a> {
    pub(crate) p: &'a Painter<'a>,
    pub(crate) track: &'a Track,
    pub(crate) staff: &'a Staff,
    pub(crate) tab_gap: f32,
    pub(crate) show_std: bool,
    pub(crate) show_tab: bool,
}

impl Planner<'_> {
    fn sp(&self) -> f32 {
        self.p.sp
    }

    /// Plans one bar of this staff.
    pub(crate) fn bar(&self, bar: &Bar, mb: &MasterBar) -> BarPlan {
        let length = rhythm::bar_length(bar, mb.time_signature);
        let columns = rhythm::columns(bar, length);
        let clef = bar.clef;
        let key = mb.key_signature;
        let mut acc = BarAccidentals::new(key);

        // Beats, spelt in time order so accidentals carry through the bar.
        let mut voices: Vec<Vec<BeatInfo>> = bar
            .voices
            .iter()
            .enumerate()
            .map(|(vi, v)| {
                let shown = rhythm::shown(bar, vi);
                v.beats
                    .iter()
                    .zip(rhythm::onsets(v))
                    .map(|(b, onset)| BeatInfo {
                        onset,
                        kind: if shown { b.kind } else { BeatKind::Empty },
                        value: b.duration.value,
                        dots: b.duration.dots,
                        tuplet: b.duration.tuplets.first().copied(),
                        notes: Vec::new(),
                        up: true,
                        group: None,
                        chord: b
                            .chord
                            .and_then(|c| self.track.chords.get(c))
                            .map(|c| c.name.clone())
                            .filter(|n| !n.is_empty()),
                        text: b.text.trim().to_owned(),
                    })
                    .collect()
            })
            .collect();
        for col in &columns {
            for r in &col.beats {
                let Some(beat) = bar.voices.get(r.voice).and_then(|v| v.beats.get(r.beat)) else {
                    continue;
                };
                if beat.kind != BeatKind::Notes {
                    continue;
                }
                let mut notes: Vec<NoteInfo> = beat
                    .notes
                    .iter()
                    .filter_map(|n| self.note(n, clef, key, &mut acc))
                    .collect();
                notes.sort_by_key(|n| n.step);
                place_accidentals(&mut notes);
                if let Some(info) = voices.get_mut(r.voice).and_then(|v| v.get_mut(r.beat)) {
                    info.notes = notes;
                }
            }
        }

        // Stems and beams.
        let active: Vec<usize> = bar
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.beats.iter().any(|b| b.kind == BeatKind::Notes))
            .map(|(i, _)| i)
            .collect();
        // Stems follow the voice number (voices 1 and 3 up, 2 and 4 down)
        // whenever a voice other than the first has notes in the bar, and
        // always on drums. Voice 1 alone points its stems by where its notes
        // sit.
        let by_voice = active.iter().any(|&v| v > 0) || self.track.instrument.percussion;
        let mut groups = Vec::new();
        for (vi, voice) in bar.voices.iter().enumerate() {
            let g = rhythm::beam_groups(voice, mb.time_signature);
            if let Some(beats) = voices.get_mut(vi) {
                let forced = by_voice.then_some(voice_stems_up(vi));
                for (gi, members) in g.iter().enumerate() {
                    let up = forced
                        .unwrap_or_else(|| stem_up(members.iter().filter_map(|&i| beats.get(i))));
                    for &i in members {
                        if let Some(b) = beats.get_mut(i) {
                            b.group = Some(gi);
                            b.up = up;
                        }
                    }
                }
                for b in beats.iter_mut().filter(|b| b.group.is_none()) {
                    b.up = forced.unwrap_or_else(|| stem_up(std::iter::once(&*b)));
                }
                for b in beats.iter_mut() {
                    place_seconds(&mut b.notes, b.up);
                }
            }
            groups.push(g);
        }

        // Vertical reach, for system spacing.
        let mut low = 0;
        let mut high = 8;
        for b in voices.iter().flatten() {
            let (Some(first), Some(last)) = (b.notes.first(), b.notes.last()) else {
                continue;
            };
            let stem = if b.value >= NoteValue::Half { 7 } else { 0 };
            if b.up {
                high = high.max(last.step + stem);
                low = low.min(first.step);
            } else {
                low = low.min(first.step - stem);
                high = high.max(last.step);
            }
        }

        // Horizontal needs of each column.
        let cols: Vec<ColPlan> = columns
            .into_iter()
            .map(|column| self.column_needs(column, &voices))
            .collect();
        BarPlan {
            has_chords: voices.iter().flatten().any(|b| b.chord.is_some()),
            has_text: voices.iter().flatten().any(|b| !b.text.is_empty()),
            voices,
            groups,
            cols,
            length,
            clef,
            low,
            high,
        }
    }

    fn note(
        &self,
        n: &Note,
        clef: Clef,
        key: KeySignature,
        acc: &mut BarAccidentals,
    ) -> Option<NoteInfo> {
        if self.track.instrument.percussion {
            let NotePlace::Pitched(p) = n.place else {
                return None;
            };
            let (step, cross) = drum(p);
            return Some(NoteInfo {
                step,
                accidental: None,
                accidental_column: 0,
                shifted: false,
                pitch: Some(p),
                tie: n.tie,
                dead: cross || n.effects.dead,
                tab: None,
            });
        }
        let (pitch, tab) = match n.place {
            NotePlace::Fretted { string, fret } => {
                let label = if n.effects.dead {
                    "X".to_owned()
                } else if n.tie || n.effects.ghost {
                    format!("({fret})")
                } else {
                    fret.to_string()
                };
                (self.staff.pitch_of(string, fret), Some((string, label)))
            }
            NotePlace::Pitched(p) => (Some(p), None),
        };
        let spelled = spell(written(pitch?, self.staff.transposition, clef), key);
        Some(NoteInfo {
            step: staff_step(spelled.diatonic, clef),
            accidental: acc.accidental(spelled, n.tie),
            accidental_column: 0,
            shifted: false,
            pitch,
            tie: n.tie,
            dead: n.effects.dead,
            tab,
        })
    }

    fn column_needs(&self, column: Column, voices: &[Vec<BeatInfo>]) -> ColPlan {
        let sp = self.sp();
        let p = self.p;
        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;
        let mut tab_half: f32 = 0.0;
        for r in &column.beats {
            let Some(b) = beat_at(voices, *r) else {
                continue;
            };
            let head = notehead(b.value, false);
            let hw = p.width(head);
            let mut l = hw / 2.0;
            let mut rt = hw / 2.0;
            if b.notes.iter().any(|n| n.shifted) {
                if b.up {
                    rt += hw;
                } else {
                    l += hw;
                }
            }
            let columns = b
                .notes
                .iter()
                .filter(|n| n.accidental.is_some())
                .map(|n| n.accidental_column + 1)
                .max()
                .unwrap_or(0);
            l += columns as f32 * (p.width(glyphs::SHARP) + 0.15 * sp);
            if b.dots > 0 {
                rt += 0.3 * sp + f32::from(b.dots) * 0.5 * sp;
            }
            if b.up
                && b.group.is_none()
                && rhythm::beam_count(b.value) > 0
                && b.kind == BeatKind::Notes
            {
                rt += p.width(glyphs::FLAGS_UP[0]);
            }
            if b.kind == BeatKind::Rest {
                let w = p.width(rest_glyph(b.value));
                l = l.max(w / 2.0);
                rt = rt.max(w / 2.0);
            }
            left = left.max(l);
            right = right.max(rt);
            if self.show_tab {
                for n in &b.notes {
                    if let Some((_, label)) = &n.tab {
                        tab_half = tab_half.max(p.tab_number_width(self.tab_gap, label) / 2.0);
                    }
                }
            }
            if let Some(name) = &b.chord {
                // Chord names may overhang a little into the next column.
                tab_half = tab_half.max(0.35 * p.text_width(name, chord_name_size(sp)));
            }
        }
        if !self.show_std {
            left = 0.0;
            right = 0.0;
        }
        ColPlan {
            column,
            left,
            right,
            tab_half,
        }
    }
}

/// The size of chord names, in points.
pub(crate) fn chord_name_size(sp: f32) -> f32 {
    2.0 * sp
}

pub(crate) fn beat_at(voices: &[Vec<BeatInfo>], r: BeatRef) -> Option<&BeatInfo> {
    voices.get(r.voice)?.get(r.beat)
}

/// The stem direction a voice takes when stems follow the voice number:
/// up for voices 1 and 3 (indices 0 and 2), down for voices 2 and 4.
pub(crate) fn voice_stems_up(voice_index: usize) -> bool {
    voice_index.is_multiple_of(2)
}

/// Stems point away from the notes' side of the middle line; down when the
/// note furthest from the middle is above it.
fn stem_up<'b>(beats: impl Iterator<Item = &'b BeatInfo>) -> bool {
    let mut lowest = i32::MAX;
    let mut highest = i32::MIN;
    for b in beats {
        for n in &b.notes {
            lowest = lowest.min(n.step);
            highest = highest.max(n.step);
        }
    }
    if lowest == i32::MAX {
        return true;
    }
    (highest - 4) <= (4 - lowest)
}

/// Accidentals in a chord are set in columns, so that two within a sixth of
/// each other do not collide. They are placed in the usual order: the top
/// one nearest the chord, then the bottom one, then the second from the
/// top, the second from the bottom, and so on. Notes are lowest first.
fn place_accidentals(notes: &mut [NoteInfo]) {
    let with: Vec<usize> = (0..notes.len())
        .filter(|&i| notes.get(i).is_some_and(|n| n.accidental.is_some()))
        .collect();
    let mut order = Vec::with_capacity(with.len());
    let (mut lo, mut hi) = (0, with.len());
    while lo < hi {
        hi -= 1;
        order.push(with[hi]);
        if lo < hi {
            order.push(with[lo]);
            lo += 1;
        }
    }
    let mut placed: Vec<(i32, usize)> = Vec::new();
    for i in order {
        let Some(n) = notes.get_mut(i) else { continue };
        let mut column = 0;
        while placed
            .iter()
            .any(|&(s, c)| c == column && (s - n.step).abs() < 6)
        {
            column += 1;
        }
        n.accidental_column = column;
        placed.push((n.step, column));
    }
}

/// In a chord, a note a second away from its neighbour goes on the other
/// side of the stem. Notes are lowest first.
fn place_seconds(notes: &mut [NoteInfo], up: bool) {
    for n in notes.iter_mut() {
        n.shifted = false;
    }
    if up {
        // From the bottom: the upper note of a second moves right.
        let mut prev: Option<(i32, bool)> = None;
        for n in notes.iter_mut() {
            if let Some((s, shifted)) = prev
                && n.step - s == 1
                && !shifted
            {
                n.shifted = true;
            }
            prev = Some((n.step, n.shifted));
        }
    } else {
        // From the top: the lower note of a second moves left.
        let mut prev: Option<(i32, bool)> = None;
        for n in notes.iter_mut().rev() {
            if let Some((s, shifted)) = prev
                && s - n.step == 1
                && !shifted
            {
                n.shifted = true;
            }
            prev = Some((n.step, n.shifted));
        }
    }
}

/// The notehead for a note value.
pub(crate) fn notehead(value: NoteValue, dead: bool) -> G {
    if dead {
        return glyphs::NOTEHEAD_X;
    }
    match value {
        NoteValue::DoubleWhole => glyphs::NOTEHEAD_DOUBLE_WHOLE,
        NoteValue::Whole => glyphs::NOTEHEAD_WHOLE,
        NoteValue::Half => glyphs::NOTEHEAD_HALF,
        _ => glyphs::NOTEHEAD_BLACK,
    }
}

/// The rest glyph for a note value.
pub(crate) fn rest_glyph(value: NoteValue) -> G {
    match value {
        NoteValue::DoubleWhole => glyphs::REST_DOUBLE_WHOLE,
        NoteValue::Whole => glyphs::REST_WHOLE,
        NoteValue::Half => glyphs::REST_HALF,
        NoteValue::Quarter => glyphs::REST_QUARTER,
        NoteValue::Eighth => glyphs::REST_8TH,
        NoteValue::Sixteenth => glyphs::REST_16TH,
        NoteValue::ThirtySecond => glyphs::REST_32ND,
        NoteValue::SixtyFourth => glyphs::REST_64TH,
        _ => glyphs::REST_128TH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(step: i32) -> NoteInfo {
        NoteInfo {
            step,
            accidental: None,
            accidental_column: 0,
            shifted: false,
            pitch: None,
            tie: false,
            dead: false,
            tab: None,
        }
    }

    #[test]
    fn odd_voices_up_even_voices_down() {
        let up: Vec<bool> = (0..4).map(voice_stems_up).collect();
        assert_eq!(up, vec![true, false, true, false]);
    }

    /// Plans one bar whose voices hold one quarter note each (or nothing),
    /// and returns each voice's stem direction.
    fn stems(voices: [Option<u8>; 4]) -> Vec<Option<bool>> {
        use oxiguitar_display::FontSet;
        use oxiguitar_display::smufl::Metadata;
        use oxiguitar_model::{Beat, Duration, Voice};
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap_or_else(|e| panic!("{e}"));
        let p = Painter {
            fonts: &fonts,
            meta: &meta,
            sp: 5.0,
        };
        let staff = Staff {
            tuning: Staff::standard_guitar_tuning(),
            ..Staff::default()
        };
        let track = Track::default();
        let planner = Planner {
            p: &p,
            track: &track,
            staff: &staff,
            tab_gap: 7.5,
            show_std: true,
            show_tab: true,
        };
        let bar = Bar {
            voices: voices
                .iter()
                .map(|fret| Voice {
                    beats: fret
                        .map(|f| Beat {
                            notes: vec![Note::fretted(3, f)],
                            duration: Duration::new(NoteValue::Whole),
                            ..Beat::default()
                        })
                        .into_iter()
                        .collect(),
                })
                .collect(),
            ..Bar::default()
        };
        let plan = planner.bar(&bar, &MasterBar::default());
        plan.voices
            .iter()
            .map(|v| v.first().map(|b| b.up))
            .collect()
    }

    #[test]
    fn stems_follow_the_voice_number() {
        // G string, fret 12: written G5, above the middle line, so a lone
        // first voice points its stem down by itself.
        let high = Some(12);
        assert_eq!(
            stems([high, None, None, None]),
            vec![Some(false), None, None, None]
        );
        // Voices 1 and 3: both up, wherever the notes are.
        assert_eq!(
            stems([high, None, high, None]),
            vec![Some(true), None, Some(true), None]
        );
        // Voice 2 alone in the bar: still down; with voice 4: both down.
        assert_eq!(
            stems([None, Some(0), None, None]),
            vec![None, Some(false), None, None]
        );
        assert_eq!(
            stems([high, Some(0), high, Some(0)]),
            vec![Some(true), Some(false), Some(true), Some(false)]
        );
    }

    #[test]
    fn seconds_go_beside_the_stem() {
        let mut chord = vec![n(0), n(1), n(2), n(4)];
        place_seconds(&mut chord, true);
        let shifted: Vec<bool> = chord.iter().map(|n| n.shifted).collect();
        assert_eq!(shifted, vec![false, true, false, false]);
        place_seconds(&mut chord, false);
        let shifted: Vec<bool> = chord.iter().map(|n| n.shifted).collect();
        assert_eq!(shifted, vec![false, true, false, false]);
    }

    #[test]
    fn accidental_columns() {
        let mut chord = vec![n(0), n(2), n(9)];
        for c in &mut chord {
            c.accidental = Some(1);
        }
        place_accidentals(&mut chord);
        let cols: Vec<usize> = chord.iter().map(|n| n.accidental_column).collect();
        // The top one stands alone; the lower two are a third apart.
        assert_eq!(cols, vec![0, 1, 0]);
    }

    #[test]
    fn spacing_grows_slower_than_duration() {
        let q = duration_space(Ratio::new(1, 4).unwrap_or(Ratio::ONE));
        let h = duration_space(Ratio::new(1, 2).unwrap_or(Ratio::ONE));
        assert!(h > q && h < 2.0 * q);
    }
}
