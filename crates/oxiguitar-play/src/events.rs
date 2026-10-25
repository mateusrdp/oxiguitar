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
    Accent, Beat, BeatKind, BeatTechnique, Bend, Dynamic, GraceKind, Harmonic, Note, NotePlace,
    NoteValue, Ratio, Score, Slide, Staff, StrokeDirection,
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
    let spread = fx.stroke.map(|s| {
        // Down: from the lowest string, which has the highest number.
        notes.sort_by_key(|n| std::cmp::Reverse(n.string().unwrap_or(0)));
        if s.direction == StrokeDirection::Up {
            notes.reverse();
        }
        seconds(s.speed, span.bpm)
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
        let vibrato = if fx.vibrato_wide {
            Vibrato::Wide
        } else if nfx.vibrato {
            Vibrato::Normal
        } else {
            Vibrato::None
        };
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
            fade_in: fx.fade_in,
            fade_out: false,
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
        };

        // Repeated notes: tremolo picking, or a trill.
        let tremolo = nfx.tremolo_picking.or(fx.tremolo_picking);
        if let (Some(trill), Some(string)) = (nfx.trill, note.string()) {
            let other = staff.pitch_of(string, trill.fret).map_or(key, |p| p.0);
            repeat(work, seconds(trill.speed, span.bpm), Some(other), out);
        } else if let Some(value) = tremolo {
            repeat(work, seconds(value, span.bpm), None, out);
        } else {
            out.push(work);
        }
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
                for (voi, voice) in bar.voices.iter().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    voice_notes(staff, voice, (ti, si, voi, vi), map, &mut work);
                }
            }
            second_pass(&mut work, staff);
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

/// The notes of one voice of one played bar.
fn voice_notes(
    staff: &Staff,
    voice: &oxiguitar_model::Voice,
    (track, staff_index, voice_index, visit): (usize, usize, usize, usize),
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
            };
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
    if note.effects.vibrato && w.event.vibrato == Vibrato::None {
        w.event.vibrato = Vibrato::Normal;
    }
    w.hammer = note.effects.hammer_pull;
    w.slides = note.effects.slides.clone();
    w.event.let_ring |= note.effects.let_ring;
    true
}

/// Hammer-ons, slides and let ring: what a note does to the next one on its
/// string, and the pitch curves.
fn second_pass(work: &mut [Work], staff: &Staff) {
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
        if work[i].event.let_ring {
            let until = next_on_string[i].map_or(f64::INFINITY, |j| work[j].event.start);
            let ring = (work[i].written_end + 4.0).min(until);
            work[i].event.end = work[i].event.end.max(ring);
        }
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
}
