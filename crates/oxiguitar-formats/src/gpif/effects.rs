// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Beats' and notes' effects in GPIF.

use oxiguitar_model::{
    Accent, BeatEffects, BeatTechnique, Bend, BendPoint, Harmonic, Note, NoteEffects, NotePlace,
    NoteValue, Pitch, Ratio, Slide, Stroke, StrokeDirection, Trill,
};

use super::{
    MAX_FRET, Reader, StaffContext, TICKS_PER_QUARTER, TrackContext, float, num, properties,
};
use crate::xml::Element;

/// GPIF bend and whammy values: 100 is a whole tone, so one unit is 2 cents.
const CENTS_PER_UNIT: f64 = 2.0;
/// Bend positions are rounded to sixtieths of the note, as Guitar Pro 5 stores them.
const BEND_STEPS: f64 = 60.0;
/// The brush (strum) length Guitar Pro uses when the file does not say.
const DEFAULT_BRUSH_TICKS: i64 = 120;

/// Guitar Pro 6 names drum sounds by element and variation instead of MIDI
/// note. Our best General MIDI match for each, by variation 0, 1, 2. Rim
/// shots, chokes and half-open hi-hats have no General MIDI note of their
/// own and use the nearest sound.
const GP6_DRUMS: [[u8; 3]; 17] = [
    [35, 35, 35], // kick
    [38, 38, 37], // snare: hit, rim shot, side stick
    [56, 56, 56], // cowbell low
    [56, 56, 56], // cowbell medium
    [56, 56, 56], // cowbell high
    [41, 41, 41], // tom very low
    [43, 43, 43], // tom low
    [45, 45, 45], // tom medium
    [47, 47, 47], // tom high
    [50, 50, 50], // tom very high
    [42, 46, 46], // hi-hat: closed, half, open
    [44, 44, 44], // pedal hi-hat
    [57, 57, 57], // crash medium
    [49, 49, 49], // crash high
    [55, 55, 55], // splash
    [51, 51, 53], // ride: middle, edge, bell
    [52, 52, 52], // china
];

impl Reader<'_> {
    pub(super) fn beat_effects(&mut self, el: &Element, fx: &mut BeatEffects) {
        for child in &el.children {
            match child.name.as_str() {
                "Fadding" => match child.text() {
                    "FadeIn" => fx.fade_in = true,
                    other => self.log.warn(
                        format!("the \"{other}\" volume effect is not supported yet"),
                        self.here(),
                    ),
                },
                "Tremolo" => {
                    fx.tremolo_picking = match child.text() {
                        "1/2" => Some(NoteValue::Eighth),
                        "1/4" => Some(NoteValue::Sixteenth),
                        "1/8" => Some(NoteValue::ThirtySecond),
                        _ => {
                            self.log.warn(
                                "an unknown tremolo picking speed was read as sixteenths",
                                self.here(),
                            );
                            Some(NoteValue::Sixteenth)
                        }
                    };
                }
                "Whammy" => {
                    let a = |n: &str| child.attr(n).and_then(float);
                    fx.whammy = curve([
                        (a("originOffset"), a("originValue")),
                        (a("middleOffset1"), a("middleValue")),
                        (a("middleOffset2"), a("middleValue")),
                        (a("destinationOffset"), a("destinationValue")),
                    ]);
                }
                "Vibrato" => fx.vibrato_wide = true,
                "Arpeggio" => self
                    .log
                    .warn("arpeggios are not supported yet", self.here()),
                "Hairpin" => self.log.warn("hairpins are not supported yet", self.here()),
                "Ottavia" => self
                    .log
                    .warn("ottava lines are not supported yet", self.here()),
                "Wah" => self
                    .log
                    .warn("wah marks are not supported yet (M4)", self.here()),
                "Golpe" => self
                    .log
                    .warn("golpe is not supported yet (M4)", self.here()),
                "Legato" => self
                    .log
                    .warn("slurs between beats are not supported yet", self.here()),
                "Ornament" => self
                    .log
                    .warn("ornaments are not supported yet", self.here()),
                // Read elsewhere, or not needed to show and play the score.
                "Rhythm"
                | "Notes"
                | "GraceNotes"
                | "Dynamic"
                | "FreeText"
                | "Chord"
                | "Properties"
                | "XProperties"
                | "Lyrics"
                | "WhammyExtend"
                | "Timer"
                | "ConcertPitchStemOrientation"
                | "TransposedPitchStemOrientation"
                | "Variation"
                | "Bank"
                | "StemOrientation" => {}
                other => self.log.info(
                    format!("the beat element <{other}> was ignored"),
                    self.here(),
                ),
            }
        }

        let mut whammy = WhammyProps::default();
        let mut brush_ticks = DEFAULT_BRUSH_TICKS;
        if let Some(x) = el.child("XProperties") {
            for p in x.children_named("XProperty") {
                if p.attr("id") == Some("687935489")
                    && let Some(t) = p.child_text("Int").and_then(num::<i64>).filter(|t| *t > 0)
                {
                    brush_ticks = t;
                }
            }
        }
        for (name, p) in properties(el) {
            let value = || p.child_text("Float").and_then(float);
            match name {
                "Brush" | "PickStroke" => {
                    let direction = match p.child_text("Direction") {
                        Some("Up") => StrokeDirection::Up,
                        _ => StrokeDirection::Down,
                    };
                    if name == "Brush" {
                        fx.stroke = Some(Stroke {
                            direction,
                            speed: note_value_from_ticks(brush_ticks),
                        });
                    } else {
                        fx.pick_stroke = Some(direction);
                    }
                }
                "Slapped" => fx.technique = Some(BeatTechnique::Slap),
                "Popped" => fx.technique = Some(BeatTechnique::Pop),
                "WhammyBar" => whammy.enabled = true,
                "WhammyBarOriginValue" => whammy.values[0] = value(),
                "WhammyBarMiddleValue" => whammy.values[1] = value(),
                "WhammyBarDestinationValue" => whammy.values[2] = value(),
                "WhammyBarOriginOffset" => whammy.offsets[0] = value(),
                "WhammyBarMiddleOffset1" => whammy.offsets[1] = value(),
                "WhammyBarMiddleOffset2" => whammy.offsets[2] = value(),
                "WhammyBarDestinationOffset" => whammy.offsets[3] = value(),
                "VibratoWTremBar" => fx.vibrato_wide = true,
                "Rasgueado" => self
                    .log
                    .warn("rasgueado is not supported yet (M4)", self.here()),
                "BarreFret" | "BarreString" => {
                    self.log
                        .warn("barre marks are not supported yet (M4)", self.here());
                }
                "WhammyBarExtend"
                | "PrimaryPickupTone"
                | "SecondaryPickupTone"
                | "PrimaryPickupVolume"
                | "SecondaryPickupVolume" => {}
                other => self.log.info(
                    format!("the beat property \"{other}\" was ignored"),
                    self.here(),
                ),
            }
        }
        if whammy.enabled && fx.whammy.is_none() {
            fx.whammy = curve([
                (whammy.offsets[0], whammy.values[0]),
                (whammy.offsets[1], whammy.values[1]),
                (whammy.offsets[2], whammy.values[1]),
                (whammy.offsets[3], whammy.values[2]),
            ]);
        }
    }

    pub(super) fn note(
        &mut self,
        el: &Element,
        ctx: &TrackContext,
        sctx: &StaffContext,
        beat_fx: &mut oxiguitar_model::BeatEffects,
    ) -> Option<Note> {
        let mut string: Option<usize> = None;
        let mut fret: Option<u8> = None;
        let mut midi: Option<u8> = None;
        let mut octave_tone: (Option<i32>, Option<i32>) = (None, None);
        let mut drum: (Option<usize>, usize) = (None, 0);
        let mut fx = NoteEffects::default();
        let mut bend = BendProps::default();
        let mut harmonic_type: Option<&str> = None;
        let mut harmonic_fret: Option<f64> = None;

        for (name, p) in properties(el) {
            let flag = || p.child("Enable").is_some();
            let value = || p.child_text("Float").and_then(float);
            match name {
                "String" => string = p.child_text("String").and_then(num::<usize>),
                "Fret" => fret = p.child_text("Fret").and_then(num::<u8>),
                "Midi" => {
                    midi = p
                        .child_text("Number")
                        .and_then(num::<u8>)
                        .filter(|m| *m <= 127)
                }
                "Octave" => octave_tone.0 = p.child_text("Number").and_then(num::<i32>),
                "Tone" => octave_tone.1 = p.child_text("Step").and_then(num::<i32>),
                "Element" => drum.0 = p.child_text("Element").and_then(num::<usize>),
                "Variation" => {
                    drum.1 = p
                        .child_text("Variation")
                        .and_then(num::<usize>)
                        .unwrap_or(0)
                }
                "Tapped" if flag() => beat_fx.technique = Some(BeatTechnique::Tap),
                "LeftHandTapped" if flag() => fx.left_hand_tap = true,
                "HopoOrigin" if flag() => fx.hammer_pull = true,
                "PalmMuted" if flag() => fx.palm_mute = true,
                "Muted" if flag() => fx.dead = true,
                "Slide" => {
                    let flags = p.child_text("Flags").and_then(num::<u32>).unwrap_or(0);
                    fx.slides = self.slides(flags);
                }
                "Bended" => bend.enabled = flag(),
                "BendOriginValue" => bend.values[0] = value(),
                "BendMiddleValue" => bend.values[1] = value(),
                "BendDestinationValue" => bend.values[2] = value(),
                "BendOriginOffset" => bend.offsets[0] = value(),
                "BendMiddleOffset1" => bend.offsets[1] = value(),
                "BendMiddleOffset2" => bend.offsets[2] = value(),
                "BendDestinationOffset" => bend.offsets[3] = value(),
                "HarmonicType" => harmonic_type = p.child_text("HType"),
                "HarmonicFret" => harmonic_fret = p.child_text("HFret").and_then(float),
                // Spelling and display details we work out ourselves.
                "ConcertPitch" | "TransposedPitch" | "HopoDestination" | "Harmonic"
                | "ShowStringNumber" | "Tapped" | "LeftHandTapped" | "HopoOrigin" | "PalmMuted"
                | "Muted" => {}
                other => self.log.info(
                    format!("the note property \"{other}\" was ignored"),
                    self.here(),
                ),
            }
        }

        let mut tie = false;
        for child in &el.children {
            match child.name.as_str() {
                "Tie" => tie = child.attr("destination") == Some("true"),
                "Vibrato" => fx.vibrato = true,
                "LetRing" => fx.let_ring = true,
                "AntiAccent" => fx.ghost = true,
                "Accent" => {
                    let flags = num::<u32>(child.text()).unwrap_or(0);
                    fx.staccato = flags & 0x01 != 0;
                    if flags & 0x04 != 0 {
                        fx.accent = Some(Accent::Heavy);
                    } else if flags & 0x08 != 0 {
                        fx.accent = Some(Accent::Normal);
                    }
                    if flags & 0x12 != 0 {
                        self.log.warn(
                            "tenuto and staccatissimo marks are not supported yet",
                            self.here(),
                        );
                    }
                }
                "Trill" => {
                    // The trill names the other note by its MIDI pitch.
                    if let Some(value) = num::<i32>(child.text()) {
                        let open = string
                            .and_then(|s| sctx.tuning_low_first.get(s))
                            .map_or(0, |p| i32::from(*p) + i32::from(sctx.capo));
                        let other = if value >= open { value - open } else { value };
                        if let Ok(f) = u8::try_from(other) {
                            fx.trill = Some(Trill {
                                fret: f.min(MAX_FRET),
                                speed: NoteValue::Sixteenth,
                            });
                        }
                    }
                }
                "LeftFingering" => fx.left_finger = finger(child.text()),
                "RightFingering" => fx.right_finger = finger(child.text()),
                "Accidental" => self.log.info(
                    "accidental spellings chosen in Guitar Pro are worked out again",
                    self.here(),
                ),
                "Ornament" => self
                    .log
                    .warn("ornaments are not supported yet", self.here()),
                "Properties" | "XProperties" | "InstrumentArticulation" | "Lyrics" => {}
                other => self.log.info(
                    format!("the note element <{other}> was ignored"),
                    self.here(),
                ),
            }
        }

        if bend.enabled {
            fx.bend = curve([
                (bend.offsets[0], bend.values[0]),
                (bend.offsets[1], bend.values[1]),
                (bend.offsets[2], bend.values[1]),
                (bend.offsets[3], bend.values[2]),
            ]);
        }
        if let Some(kind) = harmonic_type {
            let steps = harmonic_fret.map_or(12, |f| f.round().clamp(0.0, 48.0) as u8);
            fx.harmonic = match kind {
                "Natural" => Some(Harmonic::Natural),
                "Artificial" => Some(Harmonic::Artificial { semitones: steps }),
                "Tap" => Some(Harmonic::Tapped { fret: steps }),
                "Pinch" => Some(Harmonic::Pinch),
                "Semi" => Some(Harmonic::Semi),
                "Feedback" => Some(Harmonic::Feedback),
                _ => None,
            };
        }

        let place = self.place_of(ctx, sctx, string, fret, midi, octave_tone, drum)?;
        Some(Note {
            place,
            tie,
            dynamic: oxiguitar_model::Dynamic::default(),
            effects: fx,
        })
    }

    /// Where a note goes: on a string for stringed tracks, as a pitch
    /// otherwise. Returns `None`, logged, for a note we cannot place.
    #[allow(clippy::too_many_arguments)]
    fn place_of(
        &mut self,
        ctx: &TrackContext,
        sctx: &StaffContext,
        string: Option<usize>,
        fret: Option<u8>,
        midi: Option<u8>,
        octave_tone: (Option<i32>, Option<i32>),
        drum: (Option<usize>, usize),
    ) -> Option<NotePlace> {
        if ctx.percussion {
            let pitch = midi.or_else(|| {
                let (element, variation) = drum;
                let row = GP6_DRUMS.get(element?)?;
                row.get(variation).or(row.first()).copied()
            });
            if drum.0.is_some() && midi.is_none() {
                self.log.info(
                    "Guitar Pro 6 drum sounds were matched to General MIDI drums",
                    self.here(),
                );
            }
            let pitch = pitch.or(fret.filter(|f| *f <= 127));
            if pitch.is_none() {
                self.log
                    .warn("a drum note of unknown sound was left out", self.here());
            }
            return pitch.map(|p| NotePlace::Pitched(Pitch(p)));
        }
        let strings = sctx.tuning_low_first.len();
        if strings > 0
            && let (Some(s), Some(f)) = (string, fret)
        {
            if s >= strings {
                self.log.warn(
                    "a note on a string the track does not have was left out",
                    self.here(),
                );
                return None;
            }
            if f > MAX_FRET {
                self.log
                    .warn("a note above fret 99 was left out", self.here());
                return None;
            }
            return Some(NotePlace::Fretted {
                string: u8::try_from(strings - s).ok()?,
                fret: f,
            });
        }
        // Tracks without strings: piano, voice and the like.
        let pitch = midi.or_else(|| {
            let (Some(octave), Some(tone)) = octave_tone else {
                return None;
            };
            u8::try_from(octave * 12 + tone).ok().filter(|p| *p <= 127)
        });
        if pitch.is_none() {
            self.log
                .warn("a note without a pitch was left out", self.here());
        }
        pitch.map(|p| NotePlace::Pitched(Pitch(p)))
    }

    /// Slide flags: 1 shift, 2 legato, 4 out downwards, 8 out upwards,
    /// 16 in from below, 32 in from above, 64 and 128 pick scrapes.
    fn slides(&mut self, flags: u32) -> Vec<Slide> {
        let mut v = Vec::new();
        if flags & 16 != 0 {
            v.push(Slide::InFromBelow);
        } else if flags & 32 != 0 {
            v.push(Slide::InFromAbove);
        }
        if flags & 1 != 0 {
            v.push(Slide::Shift);
        } else if flags & 2 != 0 {
            v.push(Slide::Legato);
        } else if flags & 4 != 0 {
            v.push(Slide::OutDown);
        } else if flags & 8 != 0 {
            v.push(Slide::OutUp);
        }
        if flags & (64 | 128) != 0 {
            self.log
                .warn("pick scrapes are not supported yet (M4)", self.here());
        }
        v
    }
}

#[derive(Default)]
struct BendProps {
    enabled: bool,
    /// Origin, middle, destination.
    values: [Option<f64>; 3],
    /// Origin, middle 1, middle 2, destination, in percent of the note.
    offsets: [Option<f64>; 4],
}

type WhammyProps = BendProps;

/// A bend or whammy curve from up to four (offset in percent, value) points.
/// Points with a missing value are skipped; a missing offset takes the
/// natural place (0, 50, 50, 100). Repeated points are merged, and a curve
/// that never leaves zero is no bend.
fn curve(points: [(Option<f64>, Option<f64>); 4]) -> Option<Bend> {
    const DEFAULT_OFFSETS: [f64; 4] = [0.0, 50.0, 50.0, 100.0];
    let mut out: Vec<BendPoint> = Vec::new();
    for ((offset, value), default) in points.into_iter().zip(DEFAULT_OFFSETS) {
        let Some(value) = value else { continue };
        let offset = offset.unwrap_or(default).clamp(0.0, 100.0);
        let steps = (offset / 100.0 * BEND_STEPS).round() as i64;
        let point = BendPoint {
            offset: Ratio::new(steps, BEND_STEPS as i64).unwrap_or(Ratio::ZERO),
            cents: (value * CENTS_PER_UNIT).round().clamp(-2400.0, 2400.0) as i32,
        };
        if out.last() == Some(&point) {
            continue;
        }
        if out.last().is_some_and(|last| last.offset > point.offset) {
            continue;
        }
        out.push(point);
    }
    // Guitar Pro 7 marks some tied notes as bent with a flat curve; that is
    // no bend at all.
    if out.iter().all(|p| p.cents == 0) {
        return None;
    }
    Some(Bend { points: out })
}

/// The note value closest to a length in ticks (960 to the quarter note).
fn note_value_from_ticks(ticks: i64) -> NoteValue {
    let mut best = NoteValue::ThirtySecond;
    let mut best_diff = i64::MAX;
    for d in [1u16, 2, 4, 8, 16, 32, 64, 128] {
        let len = TICKS_PER_QUARTER * 4 / i64::from(d);
        let diff = (len - ticks).abs();
        if diff < best_diff {
            best_diff = diff;
            best = NoteValue::from_denominator(d).unwrap_or(NoteValue::ThirtySecond);
        }
    }
    best
}

/// Finger names: P (thumb), I, M, A, C (little finger), as 0 to 4.
fn finger(text: &str) -> Option<u8> {
    match text {
        "P" => Some(0),
        "I" => Some(1),
        "M" => Some(2),
        "A" => Some(3),
        "C" => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn bend_curve_points() {
        // A full bend and release: 0 → 100 at 50 % → 0.
        let b = curve([
            (Some(0.0), Some(0.0)),
            (Some(25.0), Some(100.0)),
            (Some(75.0), Some(100.0)),
            (Some(100.0), Some(0.0)),
        ])
        .unwrap();
        let cents: Vec<i32> = b.points.iter().map(|p| p.cents).collect();
        assert_eq!(cents, vec![0, 200, 200, 0]);
        assert_eq!(b.points[1].offset, Ratio::new(1, 4).unwrap());
        // A plain bend without middle values: two points.
        let b = curve([
            (None, Some(0.0)),
            (None, None),
            (None, None),
            (None, Some(100.0)),
        ])
        .unwrap();
        assert_eq!(b.points.len(), 2);
        assert_eq!(b.points[1].offset, Ratio::ONE);
        assert!(curve([(None, None); 4]).is_none());
        assert!(
            curve([
                (None, Some(0.0)),
                (None, Some(0.0)),
                (None, Some(0.0)),
                (None, Some(0.0))
            ])
            .is_none()
        );
    }

    #[test]
    fn brush_lengths() {
        assert_eq!(note_value_from_ticks(60), NoteValue::SixtyFourth);
        assert_eq!(note_value_from_ticks(240), NoteValue::Sixteenth);
        assert_eq!(note_value_from_ticks(131), NoteValue::ThirtySecond);
        assert_eq!(note_value_from_ticks(480), NoteValue::Eighth);
    }
}
