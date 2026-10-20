// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Effects (M3 slice 6, TAB-1): what the effect keys and the palette's
//! effect tools do at the caret.
//!
//! An [`Effect`] is switched on or off. A note effect acts on the note on
//! the caret's string, and does nothing when the caret's string has no note
//! in its beat (Mateus, 20 Oct 2026); on a staff without strings, on every
//! note of the beat. A beat effect acts on the caret's beat. With a
//! selection, either acts on every note (or beat) selected: on for all of
//! them, unless all of them have it already, then off for all (Mateus,
//! 20 Oct 2026).
//!
//! An effect with a value starts at a default, which the Inspector changes
//! (Mateus, 20 Oct 2026): a trill with the next fret up in sixteenths,
//! tremolo picking in thirty-seconds (as Guitar Pro 8's key), a brush in a
//! sixteenth, an artificial or tapped harmonic an octave (12) up. A grace
//! note becomes a thirty-second, as Guitar Pro writes them.
//!
//! Effects that need strings (slides, bends, harmonics, vibrato, palm mute,
//! let ring, trills, hammer-ons, tapping, slapping and popping, strokes,
//! the whammy bar) apply only to notes on strings, never on drum tracks.
//!
//! [`Command::Dynamic`] sets a dynamic on the caret's beat or the selected
//! beats, notes and rests; [`Command::Bend`] and [`Command::Whammy`] set a
//! curve (or none), as the bend editor makes it.

use oxiguitar_model::{
    Accent, Bar, Beat, BeatKind, BeatTechnique, Bend, Duration, Dynamic, GraceKind, Harmonic, Note,
    NotePlace, NoteValue, Score, Slide, Stroke, StrokeDirection, Trill,
};

use crate::{Caret, Command, Edited, History, Region, Selection};

/// The kinds of harmonic the keys and tools make.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HarmonicKind {
    /// Natural harmonic (Y).
    Natural,
    /// Artificial harmonic, an octave up (Ctrl + Alt + Y).
    Artificial,
    /// Tapped harmonic, 12 frets up (Alt + T).
    Tapped,
    /// Pinch harmonic (Alt + P).
    Pinch,
    /// Semi-harmonic.
    Semi,
}

impl HarmonicKind {
    /// The harmonic it starts as.
    pub fn harmonic(self) -> Harmonic {
        match self {
            HarmonicKind::Natural => Harmonic::Natural,
            HarmonicKind::Artificial => Harmonic::Artificial { semitones: 12 },
            HarmonicKind::Tapped => Harmonic::Tapped { fret: 12 },
            HarmonicKind::Pinch => Harmonic::Pinch,
            HarmonicKind::Semi => Harmonic::Semi,
        }
    }

    /// The kind of `h`, if it is one of these.
    pub fn of(h: Harmonic) -> Option<HarmonicKind> {
        Some(match h {
            Harmonic::Natural => HarmonicKind::Natural,
            Harmonic::Artificial { .. } => HarmonicKind::Artificial,
            Harmonic::Tapped { .. } => HarmonicKind::Tapped,
            Harmonic::Pinch => HarmonicKind::Pinch,
            Harmonic::Semi => HarmonicKind::Semi,
            Harmonic::Feedback => return None,
        })
    }
}

/// An effect switched on or off at the caret (TAB-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Dead note (X).
    Dead,
    /// Ghost note (O).
    Ghost,
    /// Accent (;).
    Accent,
    /// Heavy accent (:).
    HeavyAccent,
    /// Staccato (!).
    Staccato,
    /// Hammer-on or pull-off to the next note (H).
    HammerPull,
    /// Left-hand tapping (( ).
    LeftHandTap,
    /// A slide (S legato, Alt + S shift, Shift + S out downwards). A note
    /// slides in one way and out one way: another slide of the same side
    /// is replaced.
    Slide(Slide),
    /// Slight left-hand vibrato (V).
    Vibrato,
    /// Palm mute on the note (P).
    PalmMute,
    /// Palm mute on every note of the beat (Shift + P).
    PalmMuteBeat,
    /// Let ring (I).
    LetRing,
    /// A harmonic; another kind is replaced.
    Harmonic(HarmonicKind),
    /// A trill with the next fret up, in sixteenths (N).
    Trill,
    /// Tremolo picking in thirty-seconds, on the beat (").
    TremoloPicking,
    /// Wide vibrato with the whammy bar, on the beat (Ctrl + Alt + W).
    WideVibrato,
    /// Fade in (<).
    FadeIn,
    /// Tapping (T), slapping ($) or popping, on the beat.
    Technique(BeatTechnique),
    /// A brush (strum) down (Ctrl + D) or up (Ctrl + U), in a sixteenth.
    Brush(StrokeDirection),
    /// A pick stroke down (Shift + D) or up (Shift + U).
    PickStroke(StrokeDirection),
    /// The beat a grace note before the beat (G) or on it (Ctrl + Alt +
    /// G).
    Grace(GraceKind),
}

impl Effect {
    /// Whether it acts on notes (else on beats).
    fn on_notes(self) -> bool {
        !matches!(
            self,
            Effect::TremoloPicking
                | Effect::WideVibrato
                | Effect::FadeIn
                | Effect::Technique(_)
                | Effect::Brush(_)
                | Effect::PickStroke(_)
                | Effect::Grace(_)
        )
    }

    /// Whether it needs strings.
    fn needs_strings(self) -> bool {
        !matches!(
            self,
            Effect::Dead
                | Effect::Ghost
                | Effect::Accent
                | Effect::HeavyAccent
                | Effect::Staccato
                | Effect::TremoloPicking
                | Effect::FadeIn
                | Effect::Grace(_)
        )
    }

    /// The change's name, as Edit → Undo shows it.
    pub fn label(self) -> &'static str {
        match self {
            Effect::Dead => "Dead Note",
            Effect::Ghost => "Ghost Note",
            Effect::Accent => "Accent",
            Effect::HeavyAccent => "Heavy Accent",
            Effect::Staccato => "Staccato",
            Effect::HammerPull => "Hammer-on/Pull-off",
            Effect::LeftHandTap => "Left-Hand Tapping",
            Effect::Slide(Slide::Legato) => "Legato Slide",
            Effect::Slide(Slide::Shift) => "Shift Slide",
            Effect::Slide(Slide::InFromBelow) => "Slide In from Below",
            Effect::Slide(Slide::InFromAbove) => "Slide In from Above",
            Effect::Slide(Slide::OutDown) => "Slide Out Downwards",
            Effect::Slide(Slide::OutUp) => "Slide Out Upwards",
            Effect::Vibrato => "Vibrato",
            Effect::PalmMute | Effect::PalmMuteBeat => "Palm Mute",
            Effect::LetRing => "Let Ring",
            Effect::Harmonic(HarmonicKind::Natural) => "Natural Harmonic",
            Effect::Harmonic(HarmonicKind::Artificial) => "Artificial Harmonic",
            Effect::Harmonic(HarmonicKind::Tapped) => "Tapped Harmonic",
            Effect::Harmonic(HarmonicKind::Pinch) => "Pinch Harmonic",
            Effect::Harmonic(HarmonicKind::Semi) => "Semi-Harmonic",
            Effect::Trill => "Trill",
            Effect::TremoloPicking => "Tremolo Picking",
            Effect::WideVibrato => "Wide Vibrato",
            Effect::FadeIn => "Fade In",
            Effect::Technique(BeatTechnique::Tap) => "Tapping",
            Effect::Technique(BeatTechnique::Slap) => "Slap",
            Effect::Technique(BeatTechnique::Pop) => "Pop",
            Effect::Brush(StrokeDirection::Down) => "Brush Down",
            Effect::Brush(StrokeDirection::Up) => "Brush Up",
            Effect::PickStroke(StrokeDirection::Down) => "Pickstroke Down",
            Effect::PickStroke(StrokeDirection::Up) => "Pickstroke Up",
            Effect::Grace(GraceKind::BeforeBeat) => "Grace Note",
            Effect::Grace(GraceKind::OnBeat) => "Grace Note on the Beat",
        }
    }

    /// Whether `note` (in `beat`) has it.
    pub fn on_note(self, beat: &Beat, note: &Note) -> bool {
        let fx = &note.effects;
        match self {
            Effect::Dead => fx.dead,
            Effect::Ghost => fx.ghost,
            Effect::Accent => fx.accent == Some(Accent::Normal),
            Effect::HeavyAccent => fx.accent == Some(Accent::Heavy),
            Effect::Staccato => fx.staccato,
            Effect::HammerPull => fx.hammer_pull,
            Effect::LeftHandTap => fx.left_hand_tap,
            Effect::Slide(s) => fx.slides.contains(&s),
            Effect::Vibrato => fx.vibrato,
            Effect::PalmMute | Effect::PalmMuteBeat => fx.palm_mute,
            Effect::LetRing => fx.let_ring || beat.effects.let_ring,
            Effect::Harmonic(k) => fx.harmonic.and_then(HarmonicKind::of) == Some(k),
            Effect::Trill => fx.trill.is_some(),
            _ => self.on_beat(beat),
        }
    }

    /// Whether `beat` has it (a beat effect).
    pub fn on_beat(self, beat: &Beat) -> bool {
        let fx = &beat.effects;
        match self {
            Effect::TremoloPicking => {
                fx.tremolo_picking.is_some()
                    || beat
                        .notes
                        .iter()
                        .any(|n| n.effects.tremolo_picking.is_some())
            }
            Effect::WideVibrato => fx.vibrato_wide,
            Effect::FadeIn => fx.fade_in,
            Effect::Technique(t) => fx.technique == Some(t),
            Effect::Brush(d) => fx.stroke.is_some_and(|s| s.direction == d),
            Effect::PickStroke(d) => fx.pick_stroke == Some(d),
            Effect::Grace(k) => beat.grace == Some(k),
            _ => !beat.notes.is_empty() && beat.notes.iter().all(|n| self.on_note(beat, n)),
        }
    }
}

/// Runs an effect, a dynamic, a bend or a whammy curve; `None` for another
/// command.
pub(crate) fn apply(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    command: &Command,
) -> Option<Option<Edited>> {
    let edit: Edit = match command {
        Command::Effect(e) => Edit::Effect(*e),
        Command::Dynamic(d) => Edit::Dynamic(*d),
        Command::Bend(b) => Edit::Bend(b.clone()),
        Command::Whammy(w) => Edit::Whammy(w.clone()),
        _ => return None,
    };
    Some(run(score, history, c, selection, command.label(), &edit))
}

enum Edit {
    Effect(Effect),
    Dynamic(Dynamic),
    Bend(Option<Bend>),
    Whammy(Option<Bend>),
}

/// Whether the selection is what a command at `c` acts on: beats of the
/// caret's staff and voice, or bars of its track.
fn selected(c: Caret, selection: Option<Selection>) -> bool {
    match selection {
        Some(Selection::Beats {
            track,
            staff,
            voice,
            ..
        }) => (track, staff, voice) == (c.track, c.staff, c.voice),
        Some(Selection::Bars { tracks, .. }) => (tracks.0..=tracks.1).contains(&c.track),
        None => false,
    }
}

fn run(
    score: &mut Score,
    history: &mut History,
    c: Caret,
    selection: Option<Selection>,
    label: &str,
    edit: &Edit,
) -> Option<Edited> {
    let staff = crate::entry::staff_of(score, c)?;
    let strings = staff.string_count() > 0
        && !score
            .tracks
            .get(c.track)
            .is_some_and(|t| t.instrument.percussion);
    let all_notes = selected(c, selection) || staff.string_count() == 0;
    let targets = crate::entry::targets(score, c, selection);
    let (&(first, _), &(last, _)) = (targets.first()?, targets.last()?);
    let mut bars: Vec<Bar> = staff.bars.get(first..=last)?.to_vec();
    let frets = staff.fret_count;
    let changed = match edit {
        Edit::Effect(e) => toggle(&mut bars, first, &targets, c, *e, strings, all_notes, frets),
        Edit::Dynamic(d) => dynamic(&mut bars, first, &targets, c, *d),
        Edit::Bend(b) => {
            let b = b.clone().filter(|b| b.points.iter().any(|p| p.cents != 0));
            strings
                && set_notes(&mut bars, first, &targets, c, all_notes, |n| {
                    matches!(n.place, NotePlace::Fretted { .. }).then(|| n.effects.bend = b.clone())
                })
        }
        Edit::Whammy(w) => {
            let w = w.clone().filter(|w| w.points.iter().any(|p| p.cents != 0));
            strings
                && each_beat(&mut bars, first, &targets, c, |b| {
                    (b.kind == BeatKind::Notes).then(|| b.effects.whammy = w.clone())
                })
        }
    };
    let staff = crate::entry::staff_of(score, c)?;
    if !changed || staff.bars.get(first..=last).is_some_and(|old| old == bars) {
        return None;
    }
    let region = Region::Bars {
        track: c.track,
        staff: c.staff,
        bars: first..last + 1,
    };
    let (track, st) = (c.track, c.staff);
    let touched = history.record(score, label, vec![region], |s| {
        if let Some(dst) = s
            .tracks
            .get_mut(track)
            .and_then(|t| t.staves.get_mut(st))
            .and_then(|staff| staff.bars.get_mut(first..=last))
        {
            dst.clone_from_slice(&bars);
        }
    });
    Some(Edited {
        touched,
        caret: c.clamp(score).unwrap_or(c),
    })
}

/// The target beat `(bar, beat)` of the caret's voice in `bars` (which
/// start at bar `first`).
fn beat_mut(bars: &mut [Bar], first: usize, c: Caret, at: (usize, usize)) -> Option<&mut Beat> {
    bars.get_mut(at.0.checked_sub(first)?)?
        .voices
        .get_mut(c.voice)?
        .beats
        .get_mut(at.1)
}

/// Runs `edit` on each target beat; whether any did something.
fn each_beat(
    bars: &mut [Bar],
    first: usize,
    targets: &[(usize, usize)],
    c: Caret,
    mut edit: impl FnMut(&mut Beat) -> Option<()>,
) -> bool {
    let mut any = false;
    for &at in targets {
        if let Some(b) = beat_mut(bars, first, c, at) {
            any |= edit(b).is_some();
        }
    }
    any
}

/// Runs `edit` on each target note: the caret's string's note, or every
/// note of the target beats with `all`; whether any did something.
fn set_notes(
    bars: &mut [Bar],
    first: usize,
    targets: &[(usize, usize)],
    c: Caret,
    all: bool,
    mut edit: impl FnMut(&mut Note) -> Option<()>,
) -> bool {
    each_beat(bars, first, targets, c, |b| {
        if b.kind != BeatKind::Notes {
            return None;
        }
        let mut any = false;
        for n in b
            .notes
            .iter_mut()
            .filter(|n| all || n.string() == Some(c.string))
        {
            any |= edit(n).is_some();
        }
        any.then_some(())
    })
}

/// Whether a note can take `e`.
fn fits(e: Effect, note: &Note, strings: bool) -> bool {
    !e.needs_strings() || (strings && matches!(note.place, NotePlace::Fretted { .. }))
}

#[allow(clippy::too_many_arguments)]
fn toggle(
    bars: &mut [Bar],
    first: usize,
    targets: &[(usize, usize)],
    c: Caret,
    e: Effect,
    strings: bool,
    all_notes: bool,
    frets: u8,
) -> bool {
    let all = all_notes || e == Effect::PalmMuteBeat;
    // What is there now: every target note (or beat) that can take it.
    let mut have = Vec::new();
    for &at in targets {
        let Some(b) = beat_mut(bars, first, c, at) else {
            continue;
        };
        if b.kind != BeatKind::Notes {
            continue;
        }
        if e.on_notes() {
            for n in b
                .notes
                .iter()
                .filter(|n| all || n.string() == Some(c.string))
            {
                if fits(e, n, strings) {
                    have.push(e.on_note(b, n));
                }
            }
        } else if !e.needs_strings() || (strings && !b.notes.is_empty()) {
            have.push(e.on_beat(b));
        }
    }
    if have.is_empty() {
        return false;
    }
    let on = !have.iter().all(|h| *h);
    if e.on_notes() {
        each_beat(bars, first, targets, c, |b| {
            if b.kind != BeatKind::Notes {
                return None;
            }
            // Let ring held by the beat goes down to its notes, so one of
            // them can lose it.
            if e == Effect::LetRing && b.effects.let_ring {
                b.effects.let_ring = false;
                for n in &mut b.notes {
                    n.effects.let_ring = true;
                }
            }
            let mut any = false;
            for n in b
                .notes
                .iter_mut()
                .filter(|n| all || n.string() == Some(c.string))
            {
                if fits(e, n, strings) {
                    set_note(n, e, on, frets);
                    any = true;
                }
            }
            any.then_some(())
        })
    } else {
        each_beat(bars, first, targets, c, |b| {
            if b.kind != BeatKind::Notes || (e.needs_strings() && (!strings || b.notes.is_empty()))
            {
                return None;
            }
            set_beat(b, e, on);
            Some(())
        })
    }
}

/// The order a note's slides are kept in: in, then out (as files hold
/// them).
fn slide_in(s: Slide) -> bool {
    matches!(s, Slide::InFromBelow | Slide::InFromAbove)
}

fn set_note(n: &mut Note, e: Effect, on: bool, frets: u8) {
    let fret = match n.place {
        NotePlace::Fretted { fret, .. } => fret,
        NotePlace::Pitched(_) => 0,
    };
    let fx = &mut n.effects;
    match e {
        Effect::Dead => fx.dead = on,
        Effect::Ghost => fx.ghost = on,
        Effect::Accent => fx.accent = on.then_some(Accent::Normal),
        Effect::HeavyAccent => fx.accent = on.then_some(Accent::Heavy),
        Effect::Staccato => fx.staccato = on,
        Effect::HammerPull => fx.hammer_pull = on,
        Effect::LeftHandTap => fx.left_hand_tap = on,
        Effect::Slide(s) => {
            fx.slides
                .retain(|x| *x != s && (!on || slide_in(*x) != slide_in(s)));
            if on {
                fx.slides.push(s);
                fx.slides.sort_by_key(|x| !slide_in(*x));
            }
        }
        Effect::Vibrato => fx.vibrato = on,
        Effect::PalmMute | Effect::PalmMuteBeat => fx.palm_mute = on,
        Effect::LetRing => fx.let_ring = on,
        Effect::Harmonic(k) => fx.harmonic = on.then(|| k.harmonic()),
        Effect::Trill => {
            let top = frets.min(99);
            let other = if fret < top {
                fret + 1
            } else {
                fret.saturating_sub(1)
            };
            fx.trill = on.then_some(Trill {
                fret: other,
                speed: NoteValue::Sixteenth,
            });
        }
        _ => {}
    }
}

fn set_beat(b: &mut Beat, e: Effect, on: bool) {
    let fx = &mut b.effects;
    match e {
        Effect::TremoloPicking => {
            fx.tremolo_picking = on.then_some(NoteValue::ThirtySecond);
            for n in &mut b.notes {
                n.effects.tremolo_picking = None;
            }
        }
        Effect::WideVibrato => fx.vibrato_wide = on,
        Effect::FadeIn => fx.fade_in = on,
        Effect::Technique(t) => fx.technique = on.then_some(t),
        Effect::Brush(direction) => {
            let speed = fx.stroke.map_or(NoteValue::Sixteenth, |s| s.speed);
            fx.stroke = on.then_some(Stroke { direction, speed });
        }
        Effect::PickStroke(d) => fx.pick_stroke = on.then_some(d),
        Effect::Grace(k) => {
            b.grace = on.then_some(k);
            if on {
                b.duration = Duration::new(NoteValue::ThirtySecond);
            }
        }
        _ => {}
    }
}

/// [`Command::Dynamic`]: the marking on each target beat (notes and
/// rests), its notes played at it.
fn dynamic(
    bars: &mut [Bar],
    first: usize,
    targets: &[(usize, usize)],
    c: Caret,
    d: Dynamic,
) -> bool {
    each_beat(bars, first, targets, c, |b| {
        if b.kind == BeatKind::Empty {
            return None;
        }
        let before = (
            b.effects.dynamic,
            b.notes.iter().map(|n| n.dynamic).collect::<Vec<_>>(),
        );
        b.effects.dynamic = Some(d);
        for n in &mut b.notes {
            n.dynamic = d;
        }
        (before
            != (
                b.effects.dynamic,
                b.notes.iter().map(|n| n.dynamic).collect(),
            ))
            .then_some(())
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use oxiguitar_model::{BendPoint, Ratio};

    use super::*;
    use crate::{apply, new_score};

    fn at(beat: usize, string: u8) -> Caret {
        Caret {
            track: 0,
            staff: 0,
            voice: 0,
            bar: 0,
            beat,
            string,
        }
    }

    /// A bar of four quarter notes: a chord on strings 1 and 2, then
    /// single notes on string 3.
    fn song() -> (Score, History) {
        let mut s = new_score();
        let mut h = History::new();
        for (beat, string, fret) in [(0, 1, 5), (0, 2, 6), (1, 3, 7), (2, 3, 9), (3, 3, 7)] {
            apply(
                &mut s,
                &mut h,
                at(beat, string),
                None,
                Command::Fret { fret, again: false },
            )
            .unwrap();
        }
        (s, History::new())
    }

    fn beat(s: &Score, i: usize) -> &Beat {
        &s.tracks[0].staves[0].bars[0].voices[0].beats[i]
    }

    fn fx(s: &mut Score, h: &mut History, c: Caret, sel: Option<Selection>, e: Effect) -> bool {
        apply(s, h, c, sel, Command::Effect(e)).is_some()
    }

    #[test]
    fn a_note_effect_takes_the_caret_s_note_and_goes_off_again() {
        let (mut s, mut h) = song();
        let original = s.clone();
        assert!(fx(&mut s, &mut h, at(0, 2), None, Effect::PalmMute));
        assert!(!beat(&s, 0).notes[0].effects.palm_mute);
        assert!(beat(&s, 0).notes[1].effects.palm_mute);
        assert_eq!(h.undo_label(), Some("Palm Mute"));
        // No note on string 4: nothing.
        assert!(!fx(&mut s, &mut h, at(0, 4), None, Effect::PalmMute));
        // Shift + P: the whole beat, on (one note has it), then off.
        assert!(fx(&mut s, &mut h, at(0, 4), None, Effect::PalmMuteBeat));
        assert!(beat(&s, 0).notes.iter().all(|n| n.effects.palm_mute));
        assert!(fx(&mut s, &mut h, at(0, 4), None, Effect::PalmMuteBeat));
        assert!(beat(&s, 0).notes.iter().all(|n| !n.effects.palm_mute));
        while h.undo(&mut s).is_some() {}
        assert_eq!(s, original);
    }

    #[test]
    fn a_selection_takes_it_unless_all_have_it() {
        let (mut s, mut h) = song();
        let sel = Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (0, 1),
            to: (0, 3),
        };
        fx(&mut s, &mut h, at(2, 3), None, Effect::Accent);
        assert!(fx(&mut s, &mut h, at(1, 3), Some(sel), Effect::Accent));
        assert!((1..4).all(|i| beat(&s, i).notes[0].effects.accent == Some(Accent::Normal)));
        assert!(fx(&mut s, &mut h, at(1, 3), Some(sel), Effect::Accent));
        assert!((1..4).all(|i| beat(&s, i).notes[0].effects.accent.is_none()));
        // A heavy accent replaces an accent.
        fx(&mut s, &mut h, at(1, 3), None, Effect::Accent);
        fx(&mut s, &mut h, at(1, 3), None, Effect::HeavyAccent);
        assert_eq!(beat(&s, 1).notes[0].effects.accent, Some(Accent::Heavy));
    }

    #[test]
    fn slides_keep_one_in_and_one_out() {
        let (mut s, mut h) = song();
        let c = at(1, 3);
        fx(&mut s, &mut h, c, None, Effect::Slide(Slide::Legato));
        fx(&mut s, &mut h, c, None, Effect::Slide(Slide::InFromBelow));
        assert_eq!(
            beat(&s, 1).notes[0].effects.slides,
            [Slide::InFromBelow, Slide::Legato]
        );
        fx(&mut s, &mut h, c, None, Effect::Slide(Slide::Shift));
        assert_eq!(
            beat(&s, 1).notes[0].effects.slides,
            [Slide::InFromBelow, Slide::Shift]
        );
        fx(&mut s, &mut h, c, None, Effect::Slide(Slide::InFromBelow));
        assert_eq!(beat(&s, 1).notes[0].effects.slides, [Slide::Shift]);
    }

    #[test]
    fn values_start_at_their_defaults() {
        let (mut s, mut h) = song();
        fx(&mut s, &mut h, at(1, 3), None, Effect::Trill);
        let t = beat(&s, 1).notes[0].effects.trill.unwrap();
        assert_eq!((t.fret, t.speed), (8, NoteValue::Sixteenth));
        fx(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Effect::Harmonic(HarmonicKind::Artificial),
        );
        assert_eq!(
            beat(&s, 1).notes[0].effects.harmonic,
            Some(Harmonic::Artificial { semitones: 12 })
        );
        fx(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Effect::Harmonic(HarmonicKind::Pinch),
        );
        assert_eq!(beat(&s, 1).notes[0].effects.harmonic, Some(Harmonic::Pinch));
        fx(&mut s, &mut h, at(1, 3), None, Effect::TremoloPicking);
        assert_eq!(
            beat(&s, 1).effects.tremolo_picking,
            Some(NoteValue::ThirtySecond)
        );
        fx(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Effect::Brush(StrokeDirection::Up),
        );
        assert_eq!(
            beat(&s, 1).effects.stroke,
            Some(Stroke {
                direction: StrokeDirection::Up,
                speed: NoteValue::Sixteenth
            })
        );
        fx(
            &mut s,
            &mut h,
            at(2, 3),
            None,
            Effect::Grace(GraceKind::BeforeBeat),
        );
        assert_eq!(beat(&s, 2).grace, Some(GraceKind::BeforeBeat));
        assert_eq!(beat(&s, 2).duration.value, NoteValue::ThirtySecond);
    }

    #[test]
    fn rests_take_no_effect_but_a_dynamic() {
        let (mut s, mut h) = song();
        apply(&mut s, &mut h, at(3, 3), None, Command::Rest).unwrap();
        assert!(!fx(&mut s, &mut h, at(3, 3), None, Effect::FadeIn));
        assert!(!fx(&mut s, &mut h, at(3, 3), None, Effect::Dead));
        apply(
            &mut s,
            &mut h,
            at(3, 3),
            None,
            Command::Dynamic(Dynamic::PP),
        )
        .unwrap();
        assert_eq!(beat(&s, 3).effects.dynamic, Some(Dynamic::PP));
        apply(&mut s, &mut h, at(0, 1), None, Command::Dynamic(Dynamic::P)).unwrap();
        assert!(beat(&s, 0).notes.iter().all(|n| n.dynamic == Dynamic::P));
        assert!(apply(&mut s, &mut h, at(0, 1), None, Command::Dynamic(Dynamic::P)).is_none());
    }

    #[test]
    fn drums_take_accents_but_no_bends() {
        let (mut s, mut h) = song();
        s.tracks[0].instrument.percussion = true;
        assert!(!fx(&mut s, &mut h, at(1, 3), None, Effect::Vibrato));
        assert!(fx(&mut s, &mut h, at(1, 3), None, Effect::Ghost));
        let bend = Bend {
            points: vec![
                BendPoint {
                    offset: Ratio::ZERO,
                    cents: 0,
                },
                BendPoint {
                    offset: Ratio::ONE,
                    cents: 200,
                },
            ],
        };
        assert!(
            apply(
                &mut s,
                &mut h,
                at(1, 3),
                None,
                Command::Bend(Some(bend.clone()))
            )
            .is_none()
        );
        s.tracks[0].instrument.percussion = false;
        apply(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Command::Bend(Some(bend.clone())),
        )
        .unwrap();
        assert_eq!(beat(&s, 1).notes[0].effects.bend, Some(bend));
        apply(&mut s, &mut h, at(1, 3), None, Command::Bend(None)).unwrap();
        assert_eq!(beat(&s, 1).notes[0].effects.bend, None);
    }
}
