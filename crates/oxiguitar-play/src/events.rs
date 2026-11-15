// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Every note as played: when it starts and ends, in seconds, how loud,
//! and what it means for the sound (PLAY-3).
//!
//! The events carry the notes' meaning rather than MIDI messages: a palm
//! mute is a palm mute, a bend a curve of cents over time, a hammer-on a
//! note that isn't picked. Each instrument turns that into its own sound
//! (the SoundFont plugin into MIDI on one channel per string; NAM's DI set
//! the same way; a modelled guitar, later, directly).
//!
//! How the written effects are timed (docs/design/player.md):
//!
//! - Tied notes extend the note they continue, which keeps its attack.
//! - Staccato plays half the written length.
//! - Grace notes before the beat end where the beat starts; on the beat,
//!   they start there and the beat's notes start after them. Their length
//!   is their written value at the tempo there.
//! - A strum spreads the beat's notes over its value, from the lowest
//!   string for a down strum, from the highest for an up strum.
//! - Tremolo picking and trills repeat the note at their value; a trill's
//!   alternations are slurred.
//! - A slide to the next note glides over the last third of the note (at
//!   most a fifth of a second); a slide in glides from three semitones away
//!   over the first quarter (at most 0.15 s); a slide out, and a slide to
//!   no note (a voice's last note, or before a rest), glides over the last
//!   half (at most a quarter of a second) down toward the open string or up
//!   toward the last fret, and fades (Mateus, 20 Oct 2026).
//! - Let ring sustains a note until its string is struck again, at most
//!   four seconds past its written end.

use oxiguitar_model::{
    Accent, Beat, BeatKind, BeatTechnique, Bend, Dynamic, Fade, Golpe, GraceKind, Hairpin,
    Harmonic, Note, NotePlace, NoteValue, Ornament, Pedal, Rasgueado, Ratio, Score, Slide, Staff,
    StrokeDirection,
};

use crate::time::TimeMap;

/// How a note sounds, beyond its pitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// As fretted.
    Normal,
    /// Muted with the fretting hand: a percussive click with no pitch.
    Dead,
    /// Muted with the side of the picking hand.
    PalmMute,
    /// A harmonic (its sounding key is the event's key).
    Harmonic(HarmonicKind),
}

/// The kinds of harmonic, as the score writes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarmonicKind {
    /// Natural.
    Natural,
    /// Artificial.
    Artificial,
    /// Tapped.
    Tapped,
    /// Pinch.
    Pinch,
    /// Semi-harmonic: the note with a little of its harmonic.
    Semi,
    /// Feedback.
    Feedback,
}

/// How a note is started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attack {
    /// Picked (or plucked, or struck).
    Picked,
    /// Not picked: a hammer-on, pull-off, legato slide or trill
    /// alternation from the note before it on the string.
    Legato,
    /// Tapped with a finger.
    Tapped,
    /// Slapped.
    Slapped,
    /// Popped.
    Popped,
}

/// Vibrato.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vibrato {
    /// None.
    None,
    /// The fretting hand's.
    Normal,
    /// Wide (the whammy bar's).
    Wide,
}

/// A point of a note's pitch curve: at `time` seconds, `cents` away from
/// its key (100 = a semitone). Between points the pitch moves in a straight
/// line; before the first and after the last it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchPoint {
    /// Seconds from the song's start.
    pub time: f64,
    /// Cents from the note's key.
    pub cents: i32,
}

/// One note as played.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteEvent {
    /// Track index.
    pub track: usize,
    /// Staff index in the track.
    pub staff: usize,
    /// Voice index in the bar.
    pub voice: usize,
    /// Index of the played bar (into the timeline's visits).
    pub visit: usize,
    /// Index of the beat in its voice.
    pub beat: usize,
    /// String, 1 for the highest; `None` for instruments without strings
    /// and drums.
    pub string: Option<u8>,
    /// The sounding MIDI key at the start.
    pub key: u8,
    /// Seconds from the song's start (at 100 % speed).
    pub start: f64,
    /// Seconds from the song's start to the end of the note.
    pub end: f64,
    /// MIDI velocity, 1 to 127.
    pub velocity: u8,
    /// Muted, harmonic or as fretted.
    pub sound: Sound,
    /// Picked, slurred, tapped…
    pub attack: Attack,
    /// A ghost note (already softer).
    pub ghost: bool,
    /// The note rings on (already lengthened).
    pub let_ring: bool,
    /// Vibrato.
    pub vibrato: Vibrato,
    /// Swells in from silence.
    pub fade_in: bool,
    /// Fades out as it ends (a slide out).
    pub fade_out: bool,
    /// A grace note.
    pub grace: bool,
    /// The pitch over time: bends, the whammy bar, slides. Empty for a
    /// steady pitch.
    pub pitch: Vec<PitchPoint>,
}

/// MIDI velocity for a dynamic, as Guitar Pro plays them: 15 for ppp to
/// 127 for fff, in steps of 16.
pub fn velocity(d: Dynamic) -> i32 {
    let step = match d {
        Dynamic::PPP => 0,
        Dynamic::PP => 1,
        Dynamic::P => 2,
        Dynamic::MP => 3,
        Dynamic::MF => 4,
        Dynamic::F => 5,
        Dynamic::FF => 6,
        Dynamic::FFF => 7,
    };
    15 + 16 * step
}

/// Semitones above the open string where a natural harmonic at `fret`
/// sounds: the 12th fret an octave, the 7th and 19th an octave and a fifth,
/// and so on; other frets an octave.
fn natural_harmonic(fret: u8) -> u8 {
    match fret {
        12 => 12,
        7 | 19 => 19,
        5 | 24 => 24,
        4 | 9 | 16 => 28,
        3 => 31,
        _ => 12,
    }
}

/// A piecewise-linear curve in cents over time, holding its end values.
#[derive(Clone, Debug, Default)]
struct Curve(Vec<(f64, f64)>);

impl Curve {
    fn at(&self, t: f64) -> f64 {
        let p = &self.0;
        match p.iter().position(|&(x, _)| x > t) {
            None => p.last().map_or(0.0, |&(_, v)| v),
            Some(0) => p[0].1,
            Some(k) => {
                let ((x0, v0), (x1, v1)) = (p[k - 1], p[k]);
                if x1 > x0 {
                    v0 + (v1 - v0) * (t - x0) / (x1 - x0)
                } else {
                    v1
                }
            }
        }
    }

    /// A bend's points, from fractions of the note's length to times.
    fn of_bend(bend: &Bend, start: f64, end: f64) -> Curve {
        Curve(
            bend.points
                .iter()
                .map(|p| {
                    (
                        start + p.offset.to_f64().clamp(0.0, 1.0) * (end - start),
                        f64::from(p.cents),
                    )
                })
                .collect(),
        )
    }

    /// The sum of curves, at all their points.
    fn sum(curves: &[Curve]) -> Curve {
        let mut times: Vec<f64> = curves
            .iter()
            .flat_map(|c| c.0.iter().map(|p| p.0))
            .collect();
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        Curve(
            times
                .iter()
                .map(|&t| (t, curves.iter().map(|c| c.at(t)).sum()))
                .collect(),
        )
    }

    fn points(&self) -> Vec<PitchPoint> {
        // A curve that never leaves zero is no curve.
        if self.0.iter().all(|&(_, v)| v.abs() < 0.5) {
            return Vec::new();
        }
        self.0
            .iter()
            .map(|&(time, cents)| PitchPoint {
                time,
                cents: cents.round() as i32,
            })
            .collect()
    }
}

/// A note on its way to becoming an event: what the second pass needs.
#[derive(Clone, Debug)]
struct Work {
    event: NoteEvent,
    fret: Option<u8>,
    hammer: bool,
    slides: Vec<Slide>,
    /// Bend and whammy curves; slides are added in the second pass.
    curves: Vec<Curve>,
    /// The written end, before let ring.
    written_end: f64,
    /// The hairpin over its beat.
    hairpin: Option<Hairpin>,
}

/// The seconds a note value lasts at `bpm`.
fn seconds(value: NoteValue, bpm: f64) -> f64 {
    value.length().to_f64() * 240.0 / bpm
}

/// The sounding key of `note` on `staff`, with its harmonic, if it has one
/// in the MIDI range.
fn key_of(note: &Note, staff: &Staff) -> Option<(u8, Option<u8>)> {
    match note.place {
        NotePlace::Pitched(p) => Some((p.0.min(127), None)),
        NotePlace::Fretted { string, fret } => {
            let base = staff.pitch_of(string, fret)?;
            let open = staff.pitch_of(string, 0)?;
            let key = match note.effects.harmonic {
                None | Some(Harmonic::Semi) => base.0,
                Some(Harmonic::Natural) => open.0.saturating_add(natural_harmonic(fret)),
                Some(Harmonic::Artificial { semitones }) => base.0.saturating_add(semitones),
                Some(Harmonic::Tapped { fret: touched }) => {
                    let above = touched.saturating_sub(fret);
                    base.0.saturating_add(if above > 0 {
                        natural_harmonic(above)
                    } else {
                        12
                    })
                }
                Some(Harmonic::Pinch) => base.0.saturating_add(19),
                Some(Harmonic::Feedback) => base.0.saturating_add(12),
            };
            Some((key.min(127), Some(fret)))
        }
    }
}

fn harmonic_kind(h: Harmonic) -> HarmonicKind {
    match h {
        Harmonic::Natural => HarmonicKind::Natural,
        Harmonic::Artificial { .. } => HarmonicKind::Artificial,
        Harmonic::Tapped { .. } => HarmonicKind::Tapped,
        Harmonic::Pinch => HarmonicKind::Pinch,
        Harmonic::Semi => HarmonicKind::Semi,
        Harmonic::Feedback => HarmonicKind::Feedback,
    }
}

/// Where a beat's notes start and end, in seconds.
struct Span {
    start: f64,
    end: f64,
    bpm: f64,
}

/// Everything a beat's notes need from the bar around them.
struct Place {
    track: usize,
    staff: usize,
    voice: usize,
    visit: usize,
    beat: usize,
    /// The key signature's fifths, for ornaments' neighbours.
    fifths: i8,
}

/// The beat's notes as events (several for tremolo picking and trills).
fn beat_notes(
    beat: &Beat,
    staff: &Staff,
    at: &Place,
    span: &Span,
    grace: bool,
    out: &mut Vec<Work>,
) {
    let fx = &beat.effects;
    // A strum: the order the strings are struck in.
    let mut notes: Vec<&Note> = beat.notes.iter().collect();
    let spread = fx
        .stroke
        .map(|s| (s.direction, seconds(s.speed, span.bpm)))
        .or_else(|| {
            // An arpeggio rolls over the beat's first half, at most a
            // quarter note.
            fx.arpeggio.map(|d| {
                let half = (span.end - span.start) / 2.0;
                (d, half.min(seconds(NoteValue::Quarter, span.bpm)))
            })
        })
        .map(|(direction, over)| {
            // Down: from the lowest string, which has the highest number.
            notes.sort_by_key(|n| std::cmp::Reverse(n.string().unwrap_or(0)));
            if direction == StrokeDirection::Up {
                notes.reverse();
            }
            over
        });
    let count = notes.len().max(1) as f64;
    let whammy = fx.whammy.as_ref();
    for (rank, note) in notes.iter().enumerate() {
        let Some((key, fret)) = key_of(note, staff) else {
            continue;
        };
        let nfx = &note.effects;
        let offset = spread.map_or(0.0, |s| s * rank as f64 / count);
        let start = span.start + offset;
        let mut end = span.end;
        if nfx.staccato {
            end = start + 0.5 * (span.end - span.start);
        }
        if end <= start {
            end = start + 1e-3;
        }
        let mut level = velocity(note.dynamic);
        match nfx.accent {
            Some(Accent::Normal) => level += 16,
            Some(Accent::Heavy) => level += 32,
            None => {}
        }
        if nfx.ghost {
            level -= 32;
        }
        let sound = if nfx.dead {
            Sound::Dead
        } else if let Some(h) = nfx.harmonic {
            Sound::Harmonic(harmonic_kind(h))
        } else if nfx.palm_mute {
            Sound::PalmMute
        } else {
            Sound::Normal
        };
        let attack = if nfx.left_hand_tap {
            Attack::Tapped
        } else {
            match fx.technique {
                Some(BeatTechnique::Tap) => Attack::Tapped,
                Some(BeatTechnique::Slap) => Attack::Slapped,
                Some(BeatTechnique::Pop) => Attack::Popped,
                None => Attack::Picked,
            }
        };
        let vibrato = vibrato_of(fx.bridge_vibrato, nfx.vibrato);
        let mut curves = Vec::new();
        if let Some(b) = &nfx.bend {
            curves.push(Curve::of_bend(b, start, end));
        }
        if let Some(w) = whammy {
            curves.push(Curve::of_bend(w, span.start, span.end));
        }
        let event = NoteEvent {
            track: at.track,
            staff: at.staff,
            voice: at.voice,
            visit: at.visit,
            beat: at.beat,
            string: note.string(),
            key,
            start,
            end,
            velocity: level.clamp(1, 127) as u8,
            sound,
            attack,
            ghost: nfx.ghost,
            let_ring: nfx.let_ring || fx.let_ring,
            vibrato,
            fade_in: matches!(fx.fade, Some(Fade::In | Fade::Swell)),
            fade_out: matches!(fx.fade, Some(Fade::Out | Fade::Swell)),
            grace,
            pitch: Vec::new(),
        };
        let work = Work {
            event,
            fret,
            hammer: nfx.hammer_pull,
            slides: nfx.slides.clone(),
            curves,
            written_end: end,
            hairpin: beat.hairpin,
        };

        // Repeated notes: a rasgueado, tremolo picking, a trill, an
        // ornament.
        let tremolo = nfx.tremolo_picking.or(fx.tremolo_picking);
        if let Some(r) = fx.rasgueado {
            rasgueado(work, r, (rank, notes.len()), out);
        } else if let Some(o) = nfx.ornament {
            ornament(
                work,
                o,
                at.fifths,
                seconds(NoteValue::ThirtySecond, span.bpm),
                out,
            );
        } else if let (Some(trill), Some(string)) = (nfx.trill, note.string()) {
            let other = staff.pitch_of(string, trill.fret).map_or(key, |p| p.0);
            repeat(work, seconds(trill.speed, span.bpm), Some(other), out);
        } else if let Some(value) = tremolo {
            repeat(work, seconds(value, span.bpm), None, out);
        } else {
            out.push(work);
        }
    }
}

/// The beat's percussive hits: a golpe on the body (heard as a muted
/// note on the lowest string for the thumb, the highest for a finger) and a
/// dead slap (every string muted), short, at the beat's dynamic.
fn hits(beat: &Beat, staff: &Staff, at: &Place, span: &Span, out: &mut Vec<Work>) {
    let fx = &beat.effects;
    let strings = u8::try_from(staff.string_count()).unwrap_or(0);
    if strings == 0 || (fx.golpe.is_none() && !fx.dead_slap) {
        return;
    }
    let mut struck: Vec<u8> = Vec::new();
    match fx.golpe {
        Some(Golpe::Thumb) => struck.push(strings),
        Some(Golpe::Finger) => struck.push(1),
        None => {}
    }
    if fx.dead_slap {
        struck.extend(1..=strings);
    }
    struck.sort_unstable();
    struck.dedup();
    let level = velocity(beat.notes.first().map_or(Dynamic::F, |n| n.dynamic));
    let end = span.end.min(span.start + 0.12);
    for string in struck.into_iter().rev() {
        let Some(key) = staff.pitch_of(string, 0) else {
            continue;
        };
        let event = NoteEvent {
            track: at.track,
            staff: at.staff,
            voice: at.voice,
            visit: at.visit,
            beat: at.beat,
            string: Some(string),
            key: key.0,
            start: span.start,
            end,
            velocity: level.clamp(1, 127) as u8,
            sound: Sound::Dead,
            attack: if fx.dead_slap {
                Attack::Slapped
            } else {
                Attack::Tapped
            },
            ghost: false,
            let_ring: false,
            vibrato: Vibrato::None,
            fade_in: false,
            fade_out: false,
            grace: false,
            pitch: Vec::new(),
        };
        out.push(Work {
            event,
            fret: Some(0),
            hammer: false,
            slides: Vec::new(),
            curves: Vec::new(),
            written_end: end,
            hairpin: beat.hairpin,
        });
    }
}

/// How far apart a rasgueado's strings are struck within one stroke.
const RASGUEADO_STRUM: f64 = 0.006;

/// A rasgueado: one stroke for each finger of its pattern across the beat
/// (short-short-long for an anapaest, even otherwise), each a quick strum
/// down, but an index finger's second stroke (i i) comes back up. `rank` is
/// the note's place in a downstroke, lowest string first, of `count`.
fn rasgueado(work: Work, r: Rasgueado, (rank, count): (usize, usize), out: &mut Vec<Work>) {
    let fingers: Vec<char> = r.fingers().chars().collect();
    let n = fingers.len().max(1);
    let (start, end) = (work.event.start, work.event.end);
    let len = end - start;
    let shares: Vec<f64> = if r.anapaest() && n == 3 {
        vec![0.25, 0.25, 0.5]
    } else {
        vec![1.0 / n as f64; n]
    };
    let last = count.saturating_sub(1);
    let mut t = start;
    for (k, share) in shares.iter().enumerate() {
        let up = k > 0 && fingers.get(k) == Some(&'i') && fingers.get(k - 1) == Some(&'i');
        let order = if up { last.saturating_sub(rank) } else { rank };
        let mut w = work.clone();
        w.event.start = (t + order as f64 * RASGUEADO_STRUM).min(end);
        w.event.end = (t + share * len).min(end).max(w.event.start + 1e-3);
        w.written_end = w.event.end;
        if k + 1 < n {
            w.hammer = false;
            w.slides.clear();
        }
        out.push(w);
        t += share * len;
    }
}

/// The note a step above (or below) `key` in the major scale of the key
/// signature with `fifths`.
fn neighbour(key: u8, fifths: i8, up: bool) -> u8 {
    const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let tonic = (7 * i32::from(fifths)).rem_euclid(12);
    let in_scale = |p: i32| MAJOR.contains(&(p - tonic).rem_euclid(12));
    let k = i32::from(key);
    let step = if up { 1 } else { -1 };
    let mut p = k + step;
    while !in_scale(p) && (p - k).abs() < 3 {
        p += step;
    }
    u8::try_from(p.clamp(0, 127)).unwrap_or(key)
}

/// An ornament: a mordent plays the note, its neighbour and the note again,
/// the first two each `short` long (a thirty-second); a turn plays four
/// notes across the note's length (upper, note, lower, note; inverted:
/// lower first). The notes after the first are slurred.
fn ornament(work: Work, o: Ornament, fifths: i8, short: f64, out: &mut Vec<Work>) {
    let key = work.event.key;
    let (start, end) = (work.event.start, work.event.end);
    let len = end - start;
    let upper = neighbour(key, fifths, true);
    let lower = neighbour(key, fifths, false);
    let parts: Vec<(u8, f64)> = match o {
        Ornament::UpperMordent | Ornament::LowerMordent => {
            let other = if o == Ornament::UpperMordent {
                upper
            } else {
                lower
            };
            let d = short.min(len / 4.0);
            vec![(key, d), (other, d), (key, len - 2.0 * d)]
        }
        Ornament::Turn => vec![
            (upper, len / 4.0),
            (key, len / 4.0),
            (lower, len / 4.0),
            (key, len / 4.0),
        ],
        Ornament::InvertedTurn => {
            vec![
                (lower, len / 4.0),
                (key, len / 4.0),
                (upper, len / 4.0),
                (key, len / 4.0),
            ]
        }
    };
    let n = parts.len();
    let mut t = start;
    for (k, (part_key, d)) in parts.into_iter().enumerate() {
        let mut w = work.clone();
        w.event.key = part_key;
        w.event.start = t;
        w.event.end = (t + d).min(end).max(t + 1e-3);
        w.written_end = w.event.end;
        if k > 0 {
            w.event.attack = Attack::Legato;
        }
        if k + 1 < n {
            w.hammer = false;
            w.slides.clear();
        }
        out.push(w);
        t += d;
    }
}

/// Splits a note into strikes `step` seconds apart, alternating with
/// `other` (a trill, slurred) or repeating its key (tremolo picking).
fn repeat(work: Work, step: f64, other: Option<u8>, out: &mut Vec<Work>) {
    let (start, end) = (work.event.start, work.event.end);
    // A step of nothing (or not a number) can't repeat anything.
    if step.is_nan() || step <= 1e-4 || end - start <= step {
        out.push(work);
        return;
    }
    let mut t = start;
    let mut k = 0usize;
    while t < end - 1e-9 {
        let mut w = work.clone();
        w.event.start = t;
        w.event.end = (t + step).min(end);
        w.written_end = w.event.end;
        if let Some(other) = other
            && k > 0
        {
            // Every alternation after the first is slurred.
            if k % 2 == 1 {
                w.event.key = other;
            }
            w.event.attack = Attack::Legato;
        }
        // Only the last strike carries on to the next note.
        let last = t + step >= end - 1e-9;
        if !last {
            w.hammer = false;
            w.slides.clear();
        }
        out.push(w);
        t += step;
        k += 1;
    }
}

/// Every note of the score, played in the time map's order.
pub fn notes(score: &Score, map: &TimeMap) -> Vec<NoteEvent> {
    let mut all = Vec::new();
    for (ti, track) in score.tracks.iter().enumerate() {
        for (si, staff) in track.staves.iter().enumerate() {
            let mut work: Vec<Work> = Vec::new();
            for (vi, v) in map.visits.iter().enumerate() {
                let Some(bar) = staff.bars.get(v.visit.bar) else {
                    continue;
                };
                let fifths = score
                    .master_bars
                    .get(v.visit.bar)
                    .map_or(0, |m| m.key_signature.fifths);
                for (voi, voice) in bar.voices.iter().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    voice_notes(staff, voice, (ti, si, voi, vi, fifths), map, &mut work);
                }
            }
            let pedal = pedal_marks(staff, map);
            second_pass(&mut work, staff, &pedal);
            hairpins(&mut work);
            all.extend(work.into_iter().map(|w| w.event));
        }
    }
    all.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track.cmp(&b.track))
            .then(a.staff.cmp(&b.staff))
            .then(a.voice.cmp(&b.voice))
            .then(b.string.cmp(&a.string))
            .then(a.key.cmp(&b.key))
    });
    all
}

/// How soft a played chord is: mezzo-piano, under the track's own notes.
const CHORD_VELOCITY: u8 = 64;

/// How far apart a played chord's strings start, lowest first, in seconds:
/// a quick downstroke rather than a block.
const CHORD_STRUM: f64 = 0.008;

/// The chords of every track whose chord diagrams are played (#151): each
/// diagram, on the beat that shows it, sounds until the next diagram of
/// its track, or until the song ends. Their notes go to a voice of their
/// own, as if a track of its own (the "chord voice" of track `t` is
/// `tracks + t`, `tracks` the score's number of tracks), so they never mix
/// with the track's own notes before its effects. Only fretted tracks with
/// strings play them: a diagram is frets on the first staff's strings.
///
/// When a track has `chords_from` set, this function also generates events
/// for the cross-referenced chords: it looks up the source track's beat/chord
/// data, finds the closest target beat by onset time, and routes events to
/// the target track's chord voice. This gives the cursor visual cues for
/// every chord change point, even when the chords come from another track.
pub fn chords(score: &Score, map: &TimeMap) -> Vec<NoteEvent> {
    let tracks = score.tracks.len();
    let mut all = Vec::new();
    for (ti, track) in score.tracks.iter().enumerate() {
        // Own chords, if any.
        if track.play_chords && !track.chords.is_empty() {
            let Some(staff) = track.staves.first() else {
                continue;
            };
            if staff.tuning.is_empty() {
                continue;
            }
            // Each diagram's start, in playing order: (time, visit, beat, diagram).
            let mut starts: Vec<(f64, usize, usize, usize)> = Vec::new();
            for (vi, v) in map.visits.iter().enumerate() {
                for staff in &track.staves {
                    let Some(bar) = staff.bars.get(v.visit.bar) else {
                        continue;
                    };
                    for voice in &bar.voices {
                        let mut pos = Ratio::ZERO;
                        for (bi, beat) in voice.beats.iter().enumerate() {
                            if beat.grace.is_some() {
                                continue;
                            }
                            if let Some(c) = beat.chord.filter(|c| *c < track.chords.len()) {
                                starts.push((map.time(vi, pos), vi, bi, c));
                            }
                            if let Some(len) = beat.duration.length() {
                                pos = pos + len;
                            }
                        }
                    }
                }
            }
            starts.sort_by(|a, b| a.0.total_cmp(&b.0));
            // One diagram an instant: several voices may show the same.
            starts.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-9);
            let song_end = map.length();
            for (k, &(start, visit, beat, c)) in starts.iter().enumerate() {
                let end = starts.get(k + 1).map_or(song_end, |n| n.0);
                if end <= start {
                    continue;
                }
                let diagram = &track.chords[c];
                // Lowest string first: a downstroke.
                let strings: Vec<(u8, u8)> = diagram
                    .frets
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| Some((u8::try_from(i + 1).ok()?, (*f)?)))
                    .collect();
                for (n, &(string, fret)) in strings.iter().rev().enumerate() {
                    let Some(key) = staff.pitch_of(string, fret) else {
                        continue;
                    };
                    let at = (start + n as f64 * CHORD_STRUM).min(end);
                    all.push(NoteEvent {
                        track: tracks + ti,
                        staff: 0,
                        voice: 0,
                        visit,
                        beat,
                        string: Some(string),
                        key: key.0,
                        start: at,
                        end,
                        velocity: CHORD_VELOCITY,
                        sound: Sound::Normal,
                        attack: Attack::Picked,
                        ghost: false,
                        let_ring: true,
                        vibrato: Vibrato::None,
                        fade_in: false,
                        fade_out: false,
                        grace: false,
                        pitch: Vec::new(),
                    });
                }
            }
        }

        // Cross-referenced chords: look up source track's beats/chords,
        // find closest target beat by onset, route to target's chord voice.
        let source_ti = track.chords_from;
        if source_ti >= tracks {
            continue;
        }
        let source = &score.tracks[source_ti];
        if source.chords.is_empty() {
            continue;
        }
        // Build (onset, visit, beat, chord_index) for source beats with chords.
        let mut source_starts: Vec<(f64, usize, usize, usize)> = Vec::new();
        for (vi, v) in map.visits.iter().enumerate() {
            for staff in &source.staves {
                let Some(bar) = staff.bars.get(v.visit.bar) else {
                    continue;
                };
                for voice in &bar.voices {
                    let mut pos = Ratio::ZERO;
                    for (bi, beat) in voice.beats.iter().enumerate() {
                        if beat.grace.is_some() {
                            continue;
                        }
                        if let Some(c) = beat.chord.filter(|c| *c < source.chords.len()) {
                            source_starts.push((map.time(vi, pos), vi, bi, c));
                        }
                        if let Some(len) = beat.duration.length() {
                            pos = pos + len;
                        }
                    }
                }
            }
        }
        source_starts.sort_by(|a, b| a.0.total_cmp(&b.0));
        // For each source start, find the closest target beat by onset time.
        let mut target_starts: Vec<(f64, usize, usize, usize)> = Vec::new();
        for &(s_time, _s_visit, _s_beat, s_chord) in &source_starts {
            let mut best_dist = f64::INFINITY;
            let mut best: Option<(f64, usize, usize)> = None;
            for (vi, v) in map.visits.iter().enumerate() {
                for staff in &track.staves {
                    let Some(bar) = staff.bars.get(v.visit.bar) else {
                        continue;
                    };
                    for voice in &bar.voices {
                        let mut pos = Ratio::ZERO;
                        for (bi, beat) in voice.beats.iter().enumerate() {
                            let t_time = map.time(vi, pos);
                            let dist = (t_time - s_time).abs();
                            if dist < best_dist {
                                best_dist = dist;
                                best = Some((t_time, vi, bi));
                            }
                            if let Some(len) = beat.duration.length() {
                                pos = pos + len;
                            }
                        }
                    }
                }
            }
            if let Some((t_time, t_visit, t_beat)) = best {
                target_starts.push((t_time, t_visit, t_beat, s_chord));
            }
        }
        // Sort by target onset, deduplicate.
        target_starts.sort_by(|a, b| a.0.total_cmp(&b.0));
        target_starts.dedup_by(|b, a| (b.0 - a.0).abs() < 1e-9);
        // Generate events for each cross-referenced chord.
        let song_end = map.length();
        for (k, &(t_time, t_visit, t_beat, s_chord)) in target_starts.iter().enumerate() {
            let end = target_starts.get(k + 1).map_or(song_end, |n| n.0);
            if end <= t_time {
                continue;
            }
            let diagram = &source.chords[s_chord];
            let Some(staff) = track.staves.first() else {
                continue;
            };
            if staff.tuning.is_empty() {
                continue;
            }
            let strings: Vec<(u8, u8)> = diagram
                .frets
                .iter()
                .enumerate()
                .filter_map(|(i, f)| Some((u8::try_from(i + 1).ok()?, (*f)?)))
                .collect();
            for (n, &(string, fret)) in strings.iter().rev().enumerate() {
                let Some(key) = staff.pitch_of(string, fret) else {
                    continue;
                };
                let at = (t_time + n as f64 * CHORD_STRUM).min(end);
                all.push(NoteEvent {
                    track: tracks + ti,
                    staff: 0,
                    voice: 0,
                    visit: t_visit,
                    beat: t_beat,
                    string: Some(string),
                    key: key.0,
                    start: at,
                    end,
                    velocity: CHORD_VELOCITY,
                    sound: Sound::Normal,
                    attack: Attack::Picked,
                    ghost: false,
                    let_ring: true,
                    vibrato: Vibrato::None,
                    fade_in: false,
                    fade_out: false,
                    grace: false,
                    pitch: Vec::new(),
                });
            }
        }
    }
    all.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.track.cmp(&b.track))
            .then(b.string.cmp(&a.string))
    });
    all
}

/// The notes of one voice of one played bar.
fn voice_notes(
    staff: &Staff,
    voice: &oxiguitar_model::Voice,
    (track, staff_index, voice_index, visit, fifths): (usize, usize, usize, usize, i8),
    map: &TimeMap,
    work: &mut Vec<Work>,
) {
    let mut pos = Ratio::ZERO;
    let mut graces: Vec<(usize, &Beat)> = Vec::new();
    for (bi, beat) in voice.beats.iter().enumerate() {
        if beat.grace.is_some() {
            graces.push((bi, beat));
            continue;
        }
        let Some(len) = beat.duration.length() else {
            graces.clear();
            continue;
        };
        let end_pos = pos + len;
        let bpm = map.bpm(visit, pos);
        let mut start = map.time(visit, pos);
        let end = map.time(visit, end_pos);

        // Grace notes before the beat end where it starts; on the beat,
        // they start there and push the beat's notes after them. Before
        // the song's very first beat there is no time before it, so there
        // they play on the beat.
        let before: f64 = graces
            .iter()
            .filter(|(_, g)| g.grace == Some(GraceKind::BeforeBeat))
            .map(|(_, g)| seconds(g.duration.value, bpm))
            .sum();
        let room = start - before >= 0.0;
        let mut t_before = start - before;
        let mut t_on = start;
        for (gi, g) in &graces {
            let d = seconds(g.duration.value, bpm);
            let on_beat = g.grace == Some(GraceKind::OnBeat) || !room;
            let t = if on_beat { &mut t_on } else { &mut t_before };
            if g.kind == BeatKind::Notes {
                let span = Span {
                    start: *t,
                    end: (*t + d).min(if on_beat { end } else { start }),
                    bpm,
                };
                let at = Place {
                    track,
                    staff: staff_index,
                    voice: voice_index,
                    visit,
                    beat: *gi,
                    fifths,
                };
                beat_notes(g, staff, &at, &span, true, work);
            }
            *t += d;
        }
        start = t_on.min(end);
        graces.clear();

        if beat.kind == BeatKind::Notes {
            let at = Place {
                track,
                staff: staff_index,
                voice: voice_index,
                visit,
                beat: bi,
                fifths,
            };
            hits(beat, staff, &at, &Span { start, end, bpm }, work);
            // Tied notes extend the note they continue.
            let mut fresh = beat.clone();
            fresh.notes.clear();
            for note in &beat.notes {
                if note.tie && extend_tie(work, note, staff, voice_index, end) {
                    continue;
                }
                fresh.notes.push(note.clone());
            }
            beat_notes(&fresh, staff, &at, &Span { start, end, bpm }, false, work);
        }
        pos = end_pos;
    }
}

/// Lengthens the last note on the tied note's string (or key) in the same
/// voice to `end`, carrying the tied note's bend and vibrato. False if
/// there is none to continue.
fn extend_tie(work: &mut [Work], note: &Note, staff: &Staff, voice: usize, end: f64) -> bool {
    let Some((key, _)) = key_of(note, staff) else {
        return false;
    };
    let same = |w: &Work| match note.string() {
        Some(s) => w.event.string == Some(s),
        None => w.event.key == key,
    };
    let Some(w) = work
        .iter_mut()
        .rev()
        .find(|w| w.event.voice == voice && same(w))
    else {
        return false;
    };
    let tied_from = w.event.end;
    w.event.end = w.event.end.max(end);
    w.written_end = w.event.end;
    if let Some(b) = &note.effects.bend {
        w.curves.push(Curve::of_bend(b, tied_from, end));
    }
    if w.event.vibrato == Vibrato::None {
        w.event.vibrato = vibrato_of(None, note.effects.vibrato);
    }
    w.hammer = note.effects.hammer_pull;
    w.slides = note.effects.slides.clone();
    w.event.let_ring |= note.effects.let_ring;
    true
}

/// How a note's pitch wavers: the whammy bar's vibrato on its beat, or its
/// own left-hand one.
fn vibrato_of(
    bridge: Option<oxiguitar_model::Vibrato>,
    left: Option<oxiguitar_model::Vibrato>,
) -> Vibrato {
    use oxiguitar_model::Vibrato as V;
    match (bridge, left) {
        (Some(V::Wide), _) | (None, Some(V::Wide)) => Vibrato::Wide,
        (Some(V::Slight), _) | (None, Some(V::Slight)) => Vibrato::Normal,
        (None, None) => Vibrato::None,
    }
}

/// Hammer-ons, slides and let ring: what a note does to the next one on its
/// string, and the pitch curves.
fn second_pass(work: &mut [Work], staff: &Staff, pedal: &[(f64, bool)]) {
    // Each note's next note on the same string (or key) in the same voice,
    // and on the same string in any voice (let ring).
    let n = work.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| work[a].event.start.total_cmp(&work[b].event.start));
    let key = |w: &Work| {
        (
            w.event.voice,
            w.event
                .string
                .map_or(1000 + u16::from(w.event.key), u16::from),
        )
    };
    let mut next_same_voice = vec![None; n];
    let mut next_on_string = vec![None; n];
    {
        let mut last_voice: std::collections::HashMap<(usize, u16), usize> =
            std::collections::HashMap::new();
        let mut last_string: std::collections::HashMap<u16, usize> =
            std::collections::HashMap::new();
        for &i in order.iter().rev() {
            let k = key(&work[i]);
            next_same_voice[i] = last_voice.get(&k).copied();
            last_voice.insert(k, i);
            if work[i].event.string.is_some() {
                next_on_string[i] = last_string.get(&k.1).copied();
                last_string.insert(k.1, i);
            }
        }
    }

    for i in 0..n {
        let next = next_same_voice[i];
        let (start, end) = (work[i].event.start, work[i].event.end);
        let len = end - start;
        // Slurred into the next note.
        if let Some(j) = next {
            let legato_slide = work[i].slides.contains(&Slide::Legato);
            if work[i].hammer || legato_slide {
                work[j].event.attack = Attack::Legato;
            }
        }
        let fret = work[i].fret;
        let mut glides = Vec::new();
        for slide in work[i].slides.clone() {
            match slide {
                Slide::Shift | Slide::Legato => match next {
                    // The next note must follow on (not after a rest).
                    Some(j) if work[j].event.start - end < 0.05 => {
                        let cents =
                            (f64::from(work[j].event.key) - f64::from(work[i].event.key)) * 100.0;
                        let from = end - (len / 3.0).min(0.2);
                        glides.push(Curve(vec![(from, 0.0), (end, cents)]));
                    }
                    _ => glides.push(slide_out(&mut work[i], fret, staff, fret != Some(0))),
                },
                Slide::OutDown => glides.push(slide_out(&mut work[i], fret, staff, true)),
                Slide::OutUp => glides.push(slide_out(&mut work[i], fret, staff, false)),
                Slide::InFromBelow | Slide::InFromAbove => {
                    let cents = if slide == Slide::InFromBelow {
                        -300.0
                    } else {
                        300.0
                    };
                    let to = start + (len / 4.0).min(0.15);
                    glides.push(Curve(vec![(start, cents), (to, 0.0)]));
                }
            }
        }
        let mut curves = std::mem::take(&mut work[i].curves);
        curves.extend(glides);
        work[i].event.pitch = Curve::sum(&curves).points();

        // Let ring: on until the string is struck again, at most 4 s more.
        // Under the sustain pedal: until it comes up, or the string is
        // struck again.
        let until = next_on_string[i].map_or(f64::INFINITY, |j| work[j].event.start);
        if let Some(up) = pedal_up(pedal, start) {
            let ring = up.min(until);
            work[i].event.end = work[i].event.end.max(ring);
        } else if work[i].event.let_ring {
            let ring = (work[i].written_end + 4.0).min(until);
            work[i].event.end = work[i].event.end.max(ring);
        }
    }
}

/// The sustain pedal's marks on a staff, as played: (time, down).
fn pedal_marks(staff: &Staff, map: &TimeMap) -> Vec<(f64, bool)> {
    let mut out = Vec::new();
    for (vi, v) in map.visits.iter().enumerate() {
        let Some(bar) = staff.bars.get(v.visit.bar) else {
            continue;
        };
        for voice in &bar.voices {
            let mut pos = Ratio::ZERO;
            for b in &voice.beats {
                if b.grace.is_some() {
                    continue;
                }
                if let Some(p) = b.effects.pedal {
                    out.push((map.time(vi, pos), p == Pedal::Down));
                }
                if let Some(len) = b.duration.length() {
                    pos = pos + len;
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// When the pedal comes up, if it is down at `t`: the next release after
/// the last mark at or before `t` (the song's end if it never does).
fn pedal_up(pedal: &[(f64, bool)], t: f64) -> Option<f64> {
    let k = pedal.partition_point(|m| m.0 <= t + 1e-9);
    let (_, down) = *pedal.get(k.checked_sub(1)?)?;
    if !down {
        return None;
    }
    Some(
        pedal
            .get(k..)?
            .iter()
            .find(|m| !m.1)
            .map_or(f64::INFINITY, |m| m.0),
    )
}

/// Hairpins: across each run of a voice's notes under one hairpin, the
/// level moves from the run's first note to the next note after it, or a
/// dynamic's step (16) up or down when that one isn't louder (softer).
fn hairpins(work: &mut [Work]) {
    let mut order: Vec<usize> = (0..work.len()).collect();
    order.sort_by(|&a, &b| {
        (work[a].event.voice, work[a].event.track)
            .cmp(&(work[b].event.voice, work[b].event.track))
            .then(work[a].event.start.total_cmp(&work[b].event.start))
    });
    let mut k = 0;
    while k < order.len() {
        let Some(h) = work[order[k]].hairpin else {
            k += 1;
            continue;
        };
        let voice = work[order[k]].event.voice;
        let start = k;
        while order
            .get(k + 1)
            .is_some_and(|&j| work[j].hairpin == Some(h) && work[j].event.voice == voice)
        {
            k += 1;
        }
        let run: Vec<usize> = order[start..=k].to_vec();
        let after = order
            .get(k + 1)
            .copied()
            .filter(|&j| work[j].event.voice == voice);
        let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
            k += 1;
            continue;
        };
        let from = i32::from(work[first].event.velocity);
        let up = h == Hairpin::Crescendo;
        let step = if up { 16 } else { -16 };
        let to = after
            .map(|j| i32::from(work[j].event.velocity))
            .filter(|v| if up { *v > from } else { *v < from })
            .unwrap_or(from + step);
        let (t0, t1) = (work[first].event.start, work[last].written_end);
        for &i in &run {
            let frac = if t1 > t0 {
                ((work[i].event.start - t0) / (t1 - t0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let delta = (f64::from(to - from) * frac).round() as i32;
            let v = i32::from(work[i].event.velocity) + delta;
            work[i].event.velocity = v.clamp(1, 127) as u8;
        }
        k += 1;
    }
}

/// A slide out of a note: down toward the open string or up toward the
/// last fret (at least three semitones, at most an octave), over the last
/// half of the note, fading.
fn slide_out(w: &mut Work, fret: Option<u8>, staff: &Staff, down: bool) -> Curve {
    let (start, end) = (w.event.start, w.event.end);
    let frets = match (fret, down) {
        (Some(f), true) => f,
        (Some(f), false) => staff.fret_count.saturating_sub(f),
        (None, _) => 5,
    };
    let semitones = f64::from(frets.clamp(3, 12));
    let cents = if down {
        -100.0 * semitones
    } else {
        100.0 * semitones
    };
    w.event.fade_out = true;
    let from = end - ((end - start) / 2.0).min(0.25);
    Curve(vec![(from, 0.0), (end, cents)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_are_steps_of_the_key() {
        // In C: above E is F, below C is B; in G, above E is F sharp.
        assert_eq!(neighbour(64, 0, true), 65);
        assert_eq!(neighbour(60, 0, false), 59);
        assert_eq!(neighbour(64, 1, true), 66);
        assert_eq!(neighbour(67, 0, true), 69);
    }

    #[test]
    fn the_pedal_holds_until_it_comes_up() {
        let marks = [(1.0, true), (3.0, false), (5.0, true)];
        assert_eq!(pedal_up(&marks, 0.5), None);
        assert_eq!(pedal_up(&marks, 1.0), Some(3.0));
        assert_eq!(pedal_up(&marks, 2.0), Some(3.0));
        assert_eq!(pedal_up(&marks, 4.0), None);
        assert_eq!(pedal_up(&marks, 6.0), Some(f64::INFINITY));
    }

    #[test]
    fn velocities_follow_guitar_pro() {
        assert_eq!(velocity(Dynamic::PPP), 15);
        assert_eq!(velocity(Dynamic::F), 95);
        assert_eq!(velocity(Dynamic::FFF), 127);
    }

    #[test]
    fn curves_add_and_hold() {
        let a = Curve(vec![(0.0, 0.0), (1.0, 200.0)]);
        let b = Curve(vec![(0.5, 0.0), (1.5, -100.0)]);
        let s = Curve::sum(&[a, b]);
        assert_eq!(s.0.len(), 4);
        assert!((s.at(1.0) - (200.0 - 50.0)).abs() < 1e-9);
        assert!((s.at(2.0) - 100.0).abs() < 1e-9);
        assert!(Curve(vec![(0.0, 0.0), (1.0, 0.2)]).points().is_empty());
    }

    #[test]
    fn natural_harmonics() {
        assert_eq!(natural_harmonic(12), 12);
        assert_eq!(natural_harmonic(7), 19);
        assert_eq!(natural_harmonic(5), 24);
    }

    /// A guitar of two 4/4 bars at 120 bpm (two seconds each): a G chord
    /// on the first beat, a rest, then a D chord over the whole second bar.
    fn strummed(play: bool, capo: u8) -> Score {
        use oxiguitar_model::{Bar, ChordDiagram, Duration, MasterBar, Track, Voice};
        let beat = |value, chord| Beat {
            kind: BeatKind::Rest,
            duration: Duration {
                value,
                ..Duration::default()
            },
            chord,
            ..Beat::default()
        };
        let bar = |beats| Bar {
            voices: vec![Voice { beats }],
            ..Bar::default()
        };
        Score {
            master_bars: vec![MasterBar::default(), MasterBar::default()],
            tracks: vec![
                Track {
                    name: "Bass".into(),
                    ..Track::default()
                },
                Track {
                    name: "Rhythm".into(),
                    play_chords: play,
                    chords: vec![
                        ChordDiagram {
                            name: "G".into(),
                            first_fret: 1,
                            frets: vec![Some(3), Some(0), Some(0), Some(0), Some(2), Some(3)],
                        },
                        ChordDiagram {
                            name: "D".into(),
                            first_fret: 1,
                            frets: vec![Some(2), Some(3), Some(2), Some(0), None, None],
                        },
                    ],
                    staves: vec![Staff {
                        tuning: Staff::standard_guitar_tuning(),
                        capo,
                        bars: vec![
                            bar(vec![
                                beat(NoteValue::Half, Some(0)),
                                beat(NoteValue::Half, None),
                            ]),
                            bar(vec![beat(NoteValue::Whole, Some(1))]),
                        ],
                        ..Staff::default()
                    }],
                    ..Track::default()
                },
            ],
            ..Score::new()
        }
    }

    fn chords_of(score: &Score) -> Vec<NoteEvent> {
        let order = crate::order::playing_order(&score.master_bars);
        chords(score, &TimeMap::new(score, &order))
    }

    /// Each diagram sounds from its beat until the next one, or the song's
    /// end, as a quick downstroke on the strings it shows, on the track's
    /// chord voice (the score's two tracks, then the second's: 2 + 1).
    #[test]
    fn chords_sound_until_the_next_one_on_the_chord_voice() {
        let notes = chords_of(&strummed(true, 0));
        assert_eq!(notes.len(), 6 + 4);
        assert!(
            notes
                .iter()
                .all(|n| n.track == 3 && n.velocity == CHORD_VELOCITY)
        );
        let (g, d): (Vec<_>, Vec<_>) = notes.iter().partition(|n| n.start < 1.0);
        // G: E2 + 3 = G2 first (the lowest string), then up to G4.
        assert_eq!(g[0].key, 43);
        assert_eq!(g[0].string, Some(6));
        assert_eq!(g.last().unwrap().key, 67);
        for (k, n) in g.iter().enumerate() {
            assert!((n.start - k as f64 * CHORD_STRUM).abs() < 1e-9);
            assert!((n.end - 2.0).abs() < 1e-9, "{}", n.end);
        }
        // D: four strings, until the song's end.
        assert_eq!(d.len(), 4);
        assert!((d[0].start - 2.0).abs() < 1e-9);
        assert!(d.iter().all(|n| (n.end - 4.0).abs() < 1e-9));
        assert_eq!(d.last().unwrap().key, 66); // F♯4 on the first string.
    }

    #[test]
    fn chords_play_only_when_asked_and_with_the_capo() {
        assert!(chords_of(&strummed(false, 0)).is_empty());
        let capo = chords_of(&strummed(true, 2));
        assert_eq!(capo[0].key, 45);
    }

    /// One guitar bar at 120 of four quarter notes (string 2, frets 1 to 4,
    /// two strings on the first beat), each beat changed by `mark`; its
    /// notes as played.
    fn played(mark: impl Fn(usize, &mut Beat)) -> Vec<NoteEvent> {
        use oxiguitar_model::{Bar, Duration, MasterBar, Track, Voice};
        let beats = (0..4)
            .map(|k| {
                let mut b = Beat {
                    duration: Duration::new(NoteValue::Quarter),
                    notes: vec![Note::fretted(2, 1 + k as u8)],
                    ..Beat::default()
                };
                if k == 0 {
                    b.notes.push(Note::fretted(3, 2));
                }
                mark(k, &mut b);
                b
            })
            .collect();
        let score = Score {
            master_bars: vec![MasterBar::default()],
            tracks: vec![Track {
                staves: vec![Staff {
                    tuning: Staff::standard_guitar_tuning(),
                    bars: vec![Bar {
                        voices: vec![Voice { beats }],
                        ..Bar::default()
                    }],
                    ..Staff::default()
                }],
                ..Track::default()
            }],
            ..Score::new()
        };
        let order = crate::order::playing_order(&score.master_bars);
        notes(&score, &TimeMap::new(&score, &order))
    }

    #[test]
    fn an_arpeggio_rolls_over_half_the_beat() {
        let e = played(|k, b| {
            if k == 0 {
                b.effects.arpeggio = Some(StrokeDirection::Down);
            }
        });
        // String 3 (lower) first, string 2 a quarter of the beat later.
        let first: Vec<(Option<u8>, f64)> = e.iter().take(2).map(|n| (n.string, n.start)).collect();
        assert_eq!(first[0], (Some(3), 0.0));
        assert_eq!(first[1].0, Some(2));
        assert!((first[1].1 - 0.125).abs() < 1e-9, "{first:?}");
    }

    #[test]
    fn a_rasgueado_strikes_once_for_each_finger() {
        let e = played(|k, b| {
            if k == 1 {
                b.effects.rasgueado = Some(Rasgueado::MiiAnapaest);
            }
        });
        let strokes: Vec<f64> = e.iter().filter(|n| n.beat == 1).map(|n| n.start).collect();
        // m, i, i: short, short, long, over the half second of the beat.
        assert_eq!(strokes.len(), 3);
        assert!((strokes[0] - 0.5).abs() < 0.01);
        assert!((strokes[1] - 0.625).abs() < 0.01);
        assert!((strokes[2] - 0.75).abs() < 0.01);
    }

    #[test]
    fn ornaments_play_their_neighbours_in_the_key() {
        // String 2 fret 3: D 4 (62). Upper mordent: D E D.
        let e = played(|k, b| {
            if k == 2 {
                b.notes[0].effects.ornament = Some(Ornament::UpperMordent);
            }
        });
        let keys: Vec<u8> = e.iter().filter(|n| n.beat == 2).map(|n| n.key).collect();
        assert_eq!(keys, [62, 64, 62]);
        let turn = played(|k, b| {
            if k == 2 {
                b.notes[0].effects.ornament = Some(Ornament::Turn);
            }
        });
        let keys: Vec<u8> = turn.iter().filter(|n| n.beat == 2).map(|n| n.key).collect();
        assert_eq!(keys, [64, 62, 60, 62]);
    }

    #[test]
    fn golpe_and_dead_slap_are_muted_hits() {
        let e = played(|k, b| match k {
            1 => b.effects.golpe = Some(Golpe::Thumb),
            3 => b.effects.dead_slap = true,
            _ => {}
        });
        let golpe: Vec<&NoteEvent> = e
            .iter()
            .filter(|n| n.beat == 1 && n.sound == Sound::Dead)
            .collect();
        assert_eq!(golpe.len(), 1);
        assert_eq!(golpe[0].string, Some(6));
        let slap = e
            .iter()
            .filter(|n| n.beat == 3 && n.sound == Sound::Dead)
            .count();
        assert_eq!(slap, 6);
    }

    #[test]
    fn a_crescendo_rises_to_the_next_dynamic() {
        let e = played(|k, b| {
            if k < 3 {
                b.hairpin = Some(Hairpin::Crescendo);
            } else {
                for n in &mut b.notes {
                    n.dynamic = Dynamic::FF;
                }
            }
        });
        let v: Vec<u8> = e
            .iter()
            .filter(|n| n.string == Some(2))
            .map(|n| n.velocity)
            .collect();
        // From forte (95) towards fortissimo (111), rising.
        assert_eq!(v[0], 95);
        assert!(v[1] > v[0] && v[2] > v[1] && v[2] < 111, "{v:?}");
        assert_eq!(v[3], 111);
    }

    #[test]
    fn notes_under_the_pedal_ring_until_it_comes_up() {
        let e = played(|k, b| match k {
            0 => b.effects.pedal = Some(Pedal::Down),
            2 => b.effects.pedal = Some(Pedal::Up),
            _ => {}
        });
        // String 3's note (beat 1) rings to the release at 1 s; string 2's
        // is struck again at 0.5 s.
        let low = e.iter().find(|n| n.string == Some(3)).unwrap();
        assert!((low.end - 1.0).abs() < 1e-9, "{}", low.end);
        let high = e
            .iter()
            .find(|n| n.string == Some(2) && n.beat == 0)
            .unwrap();
        assert!((high.end - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fades_out_and_swells_shape_the_volume() {
        let e = played(|k, b| match k {
            1 => b.effects.fade = Some(Fade::Out),
            2 => b.effects.fade = Some(Fade::Swell),
            _ => {}
        });
        let at = |beat: usize| e.iter().find(|n| n.beat == beat).unwrap();
        assert!(!at(1).fade_in && at(1).fade_out);
        assert!(at(2).fade_in && at(2).fade_out);
    }
}
