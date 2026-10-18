// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Beat effects, note effects and chord diagrams in Guitar Pro 3–5.

use oxiguitar_model::{
    BeatEffects, BeatTechnique, Bend, BendPoint, ChordDiagram, Harmonic, Note, NoteValue, Ratio,
    Slide, Stroke, StrokeDirection, Trill,
};

use super::{PendingGrace, Reader, dynamic_from_gp};
use crate::binary::Result;
use crate::error::ReadError;

/// Guitar Pro measures bend positions in 60ths of the note.
const BEND_POSITION_MAX: i64 = 60;
/// Guitar Pro bend heights: 50 units per semitone (100 = a whole-tone bend).
const BEND_CENTS_PER_UNIT: i32 = 2;
const MAX_BEND_POINTS: usize = 64;

/// Note effects that Guitar Pro 3 stores on the beat and that apply to every
/// note of it.
#[derive(Default)]
pub(super) struct BeatNoteEffects {
    vibrato: bool,
    harmonic: Option<Harmonic>,
}

impl BeatNoteEffects {
    pub(super) fn apply(&self, note: &mut Note) {
        if self.vibrato {
            note.effects.vibrato = true;
        }
        if self.harmonic.is_some() && note.effects.harmonic.is_none() {
            note.effects.harmonic = self.harmonic;
        }
    }
}

impl Reader<'_> {
    pub(super) fn beat_effects(&mut self, fx: &mut BeatEffects) -> Result<BeatNoteEffects> {
        let mut notes = BeatNoteEffects::default();
        let flags1 = self.c.u8()?;
        let flags2 = if self.since_v4() { self.c.u8()? } else { 0 };
        if self.is(3) {
            notes.vibrato = flags1 & 0x01 != 0;
            if flags1 & 0x04 != 0 {
                notes.harmonic = Some(Harmonic::Natural);
            }
            if flags1 & 0x08 != 0 {
                notes.harmonic = Some(Harmonic::Artificial { semitones: 12 });
            }
        }
        fx.vibrato_wide = flags1 & 0x02 != 0;
        fx.fade_in = flags1 & 0x10 != 0;
        if flags1 & 0x20 != 0 {
            let kind = self.c.u8()?;
            fx.technique = match kind {
                1 => Some(BeatTechnique::Tap),
                2 => Some(BeatTechnique::Slap),
                3 => Some(BeatTechnique::Pop),
                _ => None,
            };
            if self.is(3) {
                // Guitar Pro 3 follows with a whammy-bar dip depth, or an
                // unused int for tap/slap/pop.
                let value = self.c.i32()?;
                if kind == 0 && value != 0 {
                    fx.whammy = Some(Bend {
                        points: vec![
                            BendPoint {
                                offset: Ratio::ZERO,
                                cents: 0,
                            },
                            BendPoint {
                                offset: Ratio::new(1, 2).unwrap_or(Ratio::ZERO),
                                cents: value.saturating_mul(-BEND_CENTS_PER_UNIT),
                            },
                            BendPoint {
                                offset: Ratio::ONE,
                                cents: 0,
                            },
                        ],
                    });
                }
            }
        }
        if flags2 & 0x04 != 0 {
            fx.whammy = Some(self.bend()?);
        }
        if flags1 & 0x40 != 0 {
            let a = self.c.i8()?;
            let b = self.c.i8()?;
            // Version 5 swapped the order of the up and down speeds.
            let (down, up) = if self.is(5) { (b, a) } else { (a, b) };
            fx.stroke = if up > 0 {
                Some(Stroke {
                    direction: StrokeDirection::Up,
                    speed: stroke_speed(up),
                })
            } else if down > 0 {
                Some(Stroke {
                    direction: StrokeDirection::Down,
                    speed: stroke_speed(down),
                })
            } else {
                None
            };
        }
        if flags2 & 0x01 != 0 {
            self.log.warn(
                "rasgueado is not supported yet (comes with M4)",
                self.here(),
            );
        }
        if flags2 & 0x02 != 0 {
            fx.pick_stroke = match self.c.i8()? {
                1 => Some(StrokeDirection::Up),
                2 => Some(StrokeDirection::Down),
                _ => None,
            };
        }
        Ok(notes)
    }

    /// A bend or whammy curve: type, height, then points.
    pub(super) fn bend(&mut self) -> Result<Bend> {
        self.c.i8()?; // bend type: the points carry the shape
        self.c.i32()?; // overall height: the points carry it
        let n = self.c.count("number of bend points", MAX_BEND_POINTS, 9)?;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let position = self.c.i32()?;
            let value = self.c.i32()?;
            self.c.bool()?; // vibrato on this point
            let position = i64::from(position).clamp(0, BEND_POSITION_MAX);
            points.push(BendPoint {
                offset: Ratio::new(position, BEND_POSITION_MAX).unwrap_or(Ratio::ZERO),
                cents: value.saturating_mul(BEND_CENTS_PER_UNIT),
            });
        }
        Ok(Bend { points })
    }

    pub(super) fn note_effects(
        &mut self,
        note: &mut Note,
        string: u8,
    ) -> Result<Option<PendingGrace>> {
        let flags1 = self.c.u8()?;
        let flags2 = if self.since_v4() { self.c.u8()? } else { 0 };
        let mut grace = None;
        if flags1 & 0x01 != 0 {
            note.effects.bend = Some(self.bend()?);
        }
        if flags1 & 0x10 != 0 {
            grace = Some(self.grace(string)?);
        }
        let fx_hammer = flags1 & 0x02 != 0;
        let fx_let_ring = flags1 & 0x08 != 0;
        if self.is(3) && flags1 & 0x04 != 0 {
            note.effects.slides.push(Slide::Shift);
        }
        if flags2 & 0x04 != 0 {
            note.effects.tremolo_picking = Some(match self.c.i8()? {
                1 => NoteValue::Eighth,
                2 => NoteValue::Sixteenth,
                _ => NoteValue::ThirtySecond,
            });
        }
        if flags2 & 0x08 != 0 {
            note.effects.slides = self.slides()?;
        }
        if flags2 & 0x10 != 0 {
            note.effects.harmonic = self.harmonic()?;
        }
        if flags2 & 0x20 != 0 {
            let fret = self.c.i8()?;
            let period = self.c.i8()?;
            note.effects.trill = Some(Trill {
                fret: u8::try_from(fret.clamp(0, 99)).unwrap_or(0),
                speed: match period {
                    1 => NoteValue::Sixteenth,
                    2 => NoteValue::ThirtySecond,
                    _ => NoteValue::SixtyFourth,
                },
            });
        }
        let fx = &mut note.effects;
        fx.hammer_pull = fx_hammer;
        fx.let_ring = fx_let_ring;
        fx.staccato = flags2 & 0x01 != 0;
        fx.palm_mute = flags2 & 0x02 != 0;
        if flags2 & 0x40 != 0 {
            fx.vibrato = true;
        }
        Ok(grace)
    }

    fn grace(&mut self, string: u8) -> Result<PendingGrace> {
        let fret = self.c.i8()?;
        let dynamic = dynamic_from_gp(self.c.i8()?);
        let (transition, duration, flags);
        if self.is(5) {
            transition = self.c.i8()?;
            duration = self.c.u8()?;
            flags = self.c.u8()?;
        } else {
            duration = self.c.u8()?;
            transition = self.c.i8()?;
            flags = if fret < 0 { 0x01 } else { 0 };
        }
        let mut note = Note::fretted(string, u8::try_from(fret.clamp(0, 99)).unwrap_or(0));
        note.dynamic = dynamic;
        note.effects.dead = flags & 0x01 != 0;
        match transition {
            1 => note.effects.slides.push(Slide::Legato),
            3 => note.effects.hammer_pull = true,
            _ => {} // 0: none; 2: bend, drawn the same as none for now
        }
        Ok(PendingGrace {
            note,
            duration: match duration {
                1 => NoteValue::SixtyFourth,
                2 => NoteValue::ThirtySecond,
                _ => NoteValue::Sixteenth,
            },
            on_beat: flags & 0x02 != 0,
        })
    }

    fn slides(&mut self) -> Result<Vec<Slide>> {
        if self.is(5) {
            let flags = self.c.u8()?;
            let all = [
                (0x01, Slide::Shift),
                (0x02, Slide::Legato),
                (0x04, Slide::OutDown),
                (0x08, Slide::OutUp),
                (0x10, Slide::InFromBelow),
                (0x20, Slide::InFromAbove),
            ];
            return Ok(all
                .iter()
                .filter(|(bit, _)| flags & bit != 0)
                .map(|(_, s)| *s)
                .collect());
        }
        Ok(match self.c.i8()? {
            -2 => vec![Slide::InFromAbove],
            -1 => vec![Slide::InFromBelow],
            1 => vec![Slide::Shift],
            2 => vec![Slide::Legato],
            3 => vec![Slide::OutDown],
            4 => vec![Slide::OutUp],
            _ => Vec::new(),
        })
    }

    fn harmonic(&mut self) -> Result<Option<Harmonic>> {
        let at = self.c.pos();
        let kind = self.c.i8()?;
        Ok(Some(match (self.major(), kind) {
            (_, 1) => Harmonic::Natural,
            (5, 2) => {
                self.c.u8()?; // pitch class
                self.c.i8()?; // accidental
                let octave = self.c.u8()?;
                Harmonic::Artificial {
                    semitones: if octave == 0 { 12 } else { 12 * octave.min(3) },
                }
            }
            (5, 3) => Harmonic::Tapped { fret: self.c.u8()? },
            (4, 3) => Harmonic::Tapped { fret: 12 },
            (_, 4) => Harmonic::Pinch,
            (_, 5) => Harmonic::Semi,
            (4, 15) => Harmonic::Artificial { semitones: 19 },
            (4, 17) => Harmonic::Artificial { semitones: 24 },
            (4, 22) => Harmonic::Artificial { semitones: 12 },
            (_, 0) => return Ok(None),
            (_, other) => {
                return Err(ReadError::invalid(format!("harmonic type {other}"), at));
            }
        }))
    }

    /// A chord diagram. Guitar Pro 3 and 4 have an old and a new layout; Guitar
    /// Pro 5 always uses Guitar Pro 4's new layout.
    pub(super) fn chord(&mut self, string_count: usize) -> Result<ChordDiagram> {
        let new_format = self.c.bool()?;
        let mut chord = ChordDiagram::default();
        let mut frets: Vec<i32> = Vec::new();
        if !new_format {
            chord.name = self.c.int_byte_size_string()?;
            let first_fret = self.c.i32()?;
            chord.first_fret = u8::try_from(first_fret.clamp(0, 99)).unwrap_or(0);
            if first_fret != 0 {
                for _ in 0..6 {
                    frets.push(self.c.i32()?);
                }
            }
        } else if self.is(3) {
            // sharp (1), blank (3), root, type, extension, bass, tonality (ints), add (1)
            self.c.skip(1 + 3 + 5 * 4 + 1)?;
            chord.name = self.c.byte_size_string(22)?;
            self.c.skip(3 * 4)?; // fifth, ninth, eleventh alterations
            chord.first_fret = u8::try_from(self.c.i32()?.clamp(0, 99)).unwrap_or(0);
            for _ in 0..6 {
                frets.push(self.c.i32()?);
            }
            // barre count, frets, starts, ends (ints); omissions (7); blank (1)
            self.c.skip(4 + 3 * 2 * 4 + 7 + 1)?;
        } else {
            // sharp (1), blank (3), root, type, extension (bytes), bass, tonality (ints), add (1)
            self.c.skip(1 + 3 + 3 + 2 * 4 + 1)?;
            chord.name = self.c.byte_size_string(22)?;
            self.c.skip(3)?; // fifth, ninth, eleventh alterations
            chord.first_fret = u8::try_from(self.c.i32()?.clamp(0, 99)).unwrap_or(0);
            for _ in 0..7 {
                frets.push(self.c.i32()?);
            }
            // barre count (1), frets, starts, ends (5 each); omissions (7);
            // blank (1); fingerings (7); show diagram (1)
            self.c.skip(1 + 15 + 7 + 1 + 7 + 1)?;
        }
        chord.frets = frets
            .into_iter()
            .take(string_count)
            .map(|f| u8::try_from(f).ok().filter(|f| *f <= 99))
            .collect();
        Ok(chord)
    }
}

/// Guitar Pro stroke speeds, from 1 (fastest) to 6.
fn stroke_speed(v: i8) -> NoteValue {
    match v {
        1 | 2 => NoteValue::OneTwentyEighth,
        3 => NoteValue::SixtyFourth,
        4 => NoteValue::ThirtySecond,
        5 => NoteValue::Sixteenth,
        6 => NoteValue::Eighth,
        _ => NoteValue::Quarter,
    }
}
