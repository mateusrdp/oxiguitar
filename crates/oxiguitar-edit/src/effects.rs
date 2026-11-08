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
    Accent, Bar, Barre, Beaming, Beat, BeatKind, BeatTechnique, Bend, Duration, Dynamic, Fade,
    Golpe, GraceKind, Hairpin, Harmonic, Note, NotePlace, NoteValue, Ornament, Ottava, Rasgueado,
    Score, Slide, Stroke, StrokeDirection, Trill, Vibrato, Wah,
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
    /// Wide left-hand vibrato (v0.17, the rest of the palette).
    WideLeftVibrato,
    /// An ornament on the note.
    Ornament(Ornament),
    /// The string's number shown on the staff.
    StringNumber,
    /// Slight vibrato with the whammy bar, on the beat.
    BridgeVibrato,
    /// Fade out, on the beat.
    FadeOut,
    /// Volume swell, on the beat.
    VolumeSwell,
    /// A wah-wah mark, on the beat.
    Wah(Wah),
    /// An arpeggio up or down, on the beat.
    Arpeggio(StrokeDirection),
    /// A rasgueado (i i at first; the Inspector picks the pattern).
    Rasgueado,
    /// A golpe, on the beat.
    Golpe(Golpe),
    /// A pick scrape up or down, on the beat.
    PickScrape(StrokeDirection),
    /// A dead slap, on the beat.
    DeadSlap,
    /// A slur over the beats (the last selected beat ends it).
    Slur,
    /// A crescendo or decrescendo over the beats.
    Hairpin(Hairpin),
    /// An ottava line over the beats.
    Ottava(Ottava),
    /// Slash notation.
    Slash,
    /// An extended barre over the beats, at the lowest fret their notes use.
    Barre,
    /// How the beats' beams join the next (automatic undoes the others).
    Beaming(Beaming),
    /// Force Groups: equal tuplets from the beat on are bracketed as one.
    TupletGroup,
}

impl Effect {
    /// Whether it acts on notes (else on beats).
    pub fn on_notes(self) -> bool {
        matches!(
            self,
            Effect::Dead
                | Effect::Ghost
                | Effect::Accent
                | Effect::HeavyAccent
                | Effect::Staccato
                | Effect::HammerPull
                | Effect::LeftHandTap
                | Effect::Slide(_)
                | Effect::Vibrato
                | Effect::PalmMute
                | Effect::PalmMuteBeat
                | Effect::LetRing
                | Effect::Harmonic(_)
                | Effect::Trill
                | Effect::WideLeftVibrato
                | Effect::Ornament(_)
                | Effect::StringNumber
        )
    }

    /// Whether a rest can take it (a beat effect that marks time, not
    /// notes).
    fn on_rests(self) -> bool {
        matches!(
            self,
            Effect::Hairpin(_) | Effect::Ottava(_) | Effect::Beaming(_) | Effect::TupletGroup
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
                | Effect::Ornament(_)
                | Effect::FadeOut
                | Effect::VolumeSwell
                | Effect::Arpeggio(_)
                | Effect::Slur
                | Effect::Hairpin(_)
                | Effect::Ottava(_)
                | Effect::Slash
                | Effect::Beaming(_)
                | Effect::TupletGroup
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
            Effect::WideLeftVibrato => "Wide Vibrato",
            Effect::Ornament(Ornament::UpperMordent) => "Upper Mordent",
            Effect::Ornament(Ornament::LowerMordent) => "Lower Mordent",
            Effect::Ornament(Ornament::Turn) => "Turn",
            Effect::Ornament(Ornament::InvertedTurn) => "Inverted Turn",
            Effect::StringNumber => "String Number",
            Effect::BridgeVibrato => "Bridge Vibrato",
            Effect::FadeOut => "Fade Out",
            Effect::VolumeSwell => "Volume Swell",
            Effect::Wah(Wah::Open) => "Open Wah-Wah",
            Effect::Wah(Wah::Closed) => "Closed Wah-Wah",
            Effect::Arpeggio(StrokeDirection::Down) => "Downward Arpeggio",
            Effect::Arpeggio(StrokeDirection::Up) => "Upward Arpeggio",
            Effect::Rasgueado => "Rasgueado",
            Effect::Golpe(Golpe::Finger) => "Finger Golpe",
            Effect::Golpe(Golpe::Thumb) => "Thumb Golpe",
            Effect::PickScrape(StrokeDirection::Up) => "Upward Pick Scrape",
            Effect::PickScrape(StrokeDirection::Down) => "Downward Pick Scrape",
            Effect::DeadSlap => "Dead Slap",
            Effect::Slur => "Slur",
            Effect::Hairpin(Hairpin::Crescendo) => "Crescendo",
            Effect::Hairpin(Hairpin::Decrescendo) => "Decrescendo",
            Effect::Ottava(Ottava::Alta8) => "8va",
            Effect::Ottava(Ottava::Bassa8) => "8vb",
            Effect::Ottava(Ottava::Alta15) => "15ma",
            Effect::Ottava(Ottava::Bassa15) => "15mb",
            Effect::Slash => "Slash",
            Effect::Barre => "Extended Barre",
            Effect::Beaming(Beaming::Automatic) => "Automatic Beaming",
            Effect::Beaming(Beaming::Force) => "Force Beaming",
            Effect::Beaming(Beaming::Break) => "Break Beaming",
            Effect::Beaming(Beaming::BreakSecondary) => "Break Secondary Beaming",
            Effect::TupletGroup => "Force Groups",
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
            Effect::Vibrato => fx.vibrato == Some(Vibrato::Slight),
            Effect::WideLeftVibrato => fx.vibrato == Some(Vibrato::Wide),
            Effect::Ornament(o) => fx.ornament == Some(o),
            Effect::StringNumber => fx.string_number,
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
            Effect::WideVibrato => fx.bridge_vibrato == Some(Vibrato::Wide),
            Effect::BridgeVibrato => fx.bridge_vibrato == Some(Vibrato::Slight),
            Effect::FadeIn => fx.fade == Some(Fade::In),
            Effect::FadeOut => fx.fade == Some(Fade::Out),
            Effect::VolumeSwell => fx.fade == Some(Fade::Swell),
            Effect::Wah(w) => fx.wah == Some(w),
            Effect::Arpeggio(d) => fx.arpeggio == Some(d),
            Effect::Rasgueado => fx.rasgueado.is_some(),
            Effect::Golpe(g) => fx.golpe == Some(g),
            Effect::PickScrape(d) => fx.pick_scrape == Some(d),
            Effect::DeadSlap => fx.dead_slap,
            Effect::Slur => beat.slur,
            Effect::Hairpin(h) => beat.hairpin == Some(h),
            Effect::Ottava(o) => beat.ottava == Some(o),
            Effect::Slash => beat.slash,
            Effect::Barre => beat.barre.is_some(),
            Effect::Beaming(m) => beat.beaming == m,
            Effect::TupletGroup => beat.tuplet_group,
            Effect::Technique(t) => fx.technique == Some(t),
            Effect::Brush(d) => fx.stroke.is_some_and(|s| s.direction == d),
            Effect::PickStroke(d) => fx.pick_stroke == Some(d),
            Effect::Grace(k) => beat.grace == Some(k),
            _ => !beat.notes.is_empty() && beat.notes.iter().all(|n| self.on_note(beat, n)),
        }
    }
}

/// A value of the caret's note (or the selected notes) set, as the
/// Inspector sets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteChange {
    /// An accent, a heavy accent, or none.
    Accent(Option<Accent>),
    /// How the note slides in (from below or above), or not.
    SlideIn(Option<Slide>),
    /// How the note slides out (shift, legato, down or up), or not.
    SlideOut(Option<Slide>),
    /// A harmonic, or none.
    Harmonic(Option<Harmonic>),
    /// A trill, or none.
    Trill(Option<Trill>),
    /// The left hand's finger (thumb 0, index 1 … little 4), or none.
    LeftFinger(Option<u8>),
    /// The right hand's finger (p 0, i 1, m 2, a 3, c 4), or none.
    RightFinger(Option<u8>),
    /// A left-hand vibrato, slight or wide, or none.
    Vibrato(Option<Vibrato>),
    /// An ornament, or none.
    Ornament(Option<Ornament>),
}

impl NoteChange {
    /// Whether it needs a note on a string.
    fn needs_strings(self) -> bool {
        !matches!(
            self,
            NoteChange::Accent(_)
                | NoteChange::LeftFinger(_)
                | NoteChange::RightFinger(_)
                | NoteChange::Ornament(_)
        )
    }

    /// The change's name, as Edit → Undo shows it.
    pub fn label(self) -> &'static str {
        match self {
            NoteChange::Accent(_) => "Accent",
            NoteChange::SlideIn(_) | NoteChange::SlideOut(_) => "Slide",
            NoteChange::Harmonic(_) => "Harmonic",
            NoteChange::Trill(_) => "Trill",
            NoteChange::LeftFinger(_) | NoteChange::RightFinger(_) => "Fingering",
            NoteChange::Vibrato(_) => "Vibrato",
            NoteChange::Ornament(_) => "Ornament",
        }
    }
}

/// A value of the caret's beat (or the selected beats) set, as the
/// Inspector sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeatChange {
    /// Tremolo picking at this speed, or none.
    Tremolo(Option<NoteValue>),
    /// A brush, its direction and how long it takes, or none.
    Stroke(Option<Stroke>),
    /// A pick stroke, or none.
    PickStroke(Option<StrokeDirection>),
    /// Tapping, slapping or popping, or none.
    Technique(Option<BeatTechnique>),
    /// A grace note (a thirty-second, as Guitar Pro writes them), or a
    /// beat of its own again.
    Grace(Option<GraceKind>),
    /// A rasgueado's pattern, or none.
    Rasgueado(Option<Rasgueado>),
    /// An extended barre's fret and shape, or none.
    Barre(Option<Barre>),
    /// A fade in, fade out or volume swell, or none.
    Fade(Option<Fade>),
    /// A vibrato with the whammy bar, slight or wide, or none.
    BridgeVibrato(Option<Vibrato>),
}

impl BeatChange {
    /// Whether it needs notes on strings.
    fn needs_strings(self) -> bool {
        !matches!(
            self,
            BeatChange::Tremolo(_) | BeatChange::Grace(_) | BeatChange::Fade(_)
        )
    }

    /// Whether a rest can take it.
    fn on_rests(self) -> bool {
        false
    }

    /// The change's name, as Edit → Undo shows it.
    pub fn label(self) -> &'static str {
        match self {
            BeatChange::Tremolo(_) => "Tremolo Picking",
            BeatChange::Stroke(_) => "Brush",
            BeatChange::PickStroke(_) => "Pickstroke",
            BeatChange::Technique(_) => "Technique",
            BeatChange::Grace(_) => "Grace Note",
            BeatChange::Rasgueado(_) => "Rasgueado",
            BeatChange::Barre(_) => "Extended Barre",
            BeatChange::Fade(_) => "Fade",
            BeatChange::BridgeVibrato(_) => "Bridge Vibrato",
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
        Command::Note(n) => Edit::Note(*n),
        Command::Beat(b) => Edit::Beat(*b),
        _ => return None,
    };
    Some(run(score, history, c, selection, command.label(), &edit))
}

enum Edit {
    Effect(Effect),
    Dynamic(Dynamic),
    Bend(Option<Bend>),
    Whammy(Option<Bend>),
    Note(NoteChange),
    Beat(BeatChange),
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
        Edit::Note(change) => {
            (strings || !change.needs_strings())
                && set_notes(&mut bars, first, &targets, c, all_notes, |n| {
                    let fretted = matches!(n.place, NotePlace::Fretted { .. });
                    (fretted || !change.needs_strings()).then_some(())?;
                    note_change(n, *change, frets)
                })
        }
        Edit::Beat(change) => {
            (strings || !change.needs_strings())
                && each_beat(&mut bars, first, &targets, c, |b| {
                    (b.kind == BeatKind::Notes || (change.on_rests() && b.kind == BeatKind::Rest))
                        .then_some(())?;
                    beat_change(b, *change);
                    Some(())
                })
        }
    };
    let staff = crate::entry::staff_of(score, c)?;
    unmark_similes(staff.bars.get(first..=last)?, &mut bars);
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

/// A bar whose music an edit changed no longer repeats the bar before it:
/// its simile sign comes off (docs/design/palette.md).
pub(crate) fn unmark_similes(old: &[Bar], new: &mut [Bar]) {
    for (o, n) in old.iter().zip(new) {
        if n.simile.is_some() && o.simile == n.simile && o.voices != n.voices {
            n.simile = None;
        }
    }
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
    let takes = |b: &Beat| b.kind == BeatKind::Notes || (e.on_rests() && b.kind == BeatKind::Rest);
    // An extended barre lies at the lowest fret any target note uses.
    let mut barre = None;
    if e == Effect::Barre {
        for &at in targets {
            if let Some(b) = beat_mut(bars, first, c, at) {
                for n in &b.notes {
                    if let NotePlace::Fretted { fret, .. } = n.place
                        && fret > 0
                        && !n.tie
                    {
                        barre = Some(barre.map_or(fret, |f: u8| f.min(fret)));
                    }
                }
            }
        }
        if barre.is_none() {
            return false;
        }
    }
    // What is there now: every target note (or beat) that can take it.
    let mut have = Vec::new();
    for &at in targets {
        let Some(b) = beat_mut(bars, first, c, at) else {
            continue;
        };
        if !takes(b) {
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
    // A slur needs two beats: at the caret, from its beat to the next.
    if e == Effect::Slur && targets.len() > 1 {
        // The last selected beat ends the slur: it isn't slurred onward.
        let last = targets.len() - 1;
        let on = !targets
            .iter()
            .take(last)
            .all(|&at| beat_mut(bars, first, c, at).is_some_and(|b| b.slur));
        let mut any = false;
        for (i, &at) in targets.iter().enumerate() {
            if let Some(b) = beat_mut(bars, first, c, at)
                && b.kind == BeatKind::Notes
            {
                b.slur = on && i < last;
                any = true;
            }
        }
        return any;
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
            if !takes(b) || (e.needs_strings() && (!strings || b.notes.is_empty())) {
                return None;
            }
            set_beat(b, e, on);
            if e == Effect::Barre {
                b.barre = on
                    .then_some(barre.map(|fret| Barre { fret, half: false }))
                    .flatten();
            }
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
        Effect::Vibrato => fx.vibrato = on.then_some(Vibrato::Slight),
        Effect::WideLeftVibrato => fx.vibrato = on.then_some(Vibrato::Wide),
        Effect::Ornament(o) => fx.ornament = on.then_some(o),
        Effect::StringNumber => fx.string_number = on,
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
        Effect::WideVibrato => fx.bridge_vibrato = on.then_some(Vibrato::Wide),
        Effect::BridgeVibrato => fx.bridge_vibrato = on.then_some(Vibrato::Slight),
        Effect::FadeIn => fx.fade = on.then_some(Fade::In),
        Effect::FadeOut => fx.fade = on.then_some(Fade::Out),
        Effect::VolumeSwell => fx.fade = on.then_some(Fade::Swell),
        Effect::Wah(w) => fx.wah = on.then_some(w),
        Effect::Arpeggio(d) => {
            fx.arpeggio = on.then_some(d);
            // One way to spread the chord.
            if on {
                fx.stroke = None;
            }
        }
        Effect::Rasgueado => fx.rasgueado = on.then_some(Rasgueado::Ii),
        Effect::Golpe(g) => fx.golpe = on.then_some(g),
        Effect::PickScrape(d) => fx.pick_scrape = on.then_some(d),
        Effect::DeadSlap => fx.dead_slap = on,
        Effect::Hairpin(h) => b.hairpin = on.then_some(h),
        Effect::Ottava(o) => b.ottava = on.then_some(o),
        Effect::Slash => b.slash = on,
        Effect::Beaming(m) => {
            b.beaming = if on { m } else { Beaming::Automatic };
        }
        Effect::TupletGroup => b.tuplet_group = on,
        Effect::Slur => b.slur = on,
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

/// Sets a value on a note; `None` when it can't take it (a trill past the
/// last fret, a finger past the little one).
fn note_change(n: &mut Note, change: NoteChange, frets: u8) -> Option<()> {
    let fx = &mut n.effects;
    match change {
        NoteChange::Accent(a) => fx.accent = a,
        NoteChange::SlideIn(s) | NoteChange::SlideOut(s) => {
            let into = matches!(change, NoteChange::SlideIn(_));
            if s.is_some_and(|s| slide_in(s) != into) {
                return None;
            }
            fx.slides.retain(|x| slide_in(*x) != into);
            if let Some(s) = s {
                fx.slides.push(s);
                fx.slides.sort_by_key(|x| !slide_in(*x));
            }
        }
        NoteChange::Harmonic(h) => fx.harmonic = h,
        NoteChange::Trill(t) => {
            if t.is_some_and(|t| t.fret > frets.min(99)) {
                return None;
            }
            fx.trill = t;
        }
        NoteChange::LeftFinger(f) | NoteChange::RightFinger(f) => {
            if f.is_some_and(|f| f > 4) {
                return None;
            }
            if matches!(change, NoteChange::LeftFinger(_)) {
                fx.left_finger = f;
            } else {
                fx.right_finger = f;
            }
        }
        NoteChange::Vibrato(v) => fx.vibrato = v,
        NoteChange::Ornament(o) => fx.ornament = o,
    }
    Some(())
}

fn beat_change(b: &mut Beat, change: BeatChange) {
    let fx = &mut b.effects;
    match change {
        BeatChange::Tremolo(t) => {
            fx.tremolo_picking = t;
            for n in &mut b.notes {
                n.effects.tremolo_picking = None;
            }
        }
        BeatChange::Stroke(s) => fx.stroke = s,
        BeatChange::PickStroke(d) => fx.pick_stroke = d,
        BeatChange::Technique(t) => fx.technique = t,
        BeatChange::Grace(g) => {
            if g.is_some() && b.grace.is_none() {
                b.duration = Duration::new(NoteValue::ThirtySecond);
            }
            b.grace = g;
        }
        BeatChange::Rasgueado(r) => fx.rasgueado = r,
        BeatChange::Barre(barre) => b.barre = barre,
        BeatChange::Fade(f) => fx.fade = f,
        BeatChange::BridgeVibrato(v) => fx.bridge_vibrato = v,
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
    fn the_inspector_sets_values() {
        let (mut s, mut h) = song();
        let c = at(1, 3);
        let note = |s: &Score| beat(s, 1).notes[0].effects.clone();
        let trill = Trill {
            fret: 10,
            speed: NoteValue::ThirtySecond,
        };
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Note(NoteChange::Trill(Some(trill))),
        )
        .unwrap();
        assert_eq!(note(&s).trill, Some(trill));
        let past = Trill { fret: 30, ..trill };
        assert!(
            apply(
                &mut s,
                &mut h,
                c,
                None,
                Command::Note(NoteChange::Trill(Some(past)))
            )
            .is_none()
        );
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Note(NoteChange::SlideOut(Some(Slide::OutUp))),
        )
        .unwrap();
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Note(NoteChange::SlideIn(Some(Slide::InFromAbove))),
        )
        .unwrap();
        assert_eq!(note(&s).slides, [Slide::InFromAbove, Slide::OutUp]);
        // An out slide is no in slide.
        assert!(
            apply(
                &mut s,
                &mut h,
                c,
                None,
                Command::Note(NoteChange::SlideIn(Some(Slide::Shift)))
            )
            .is_none()
        );
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Note(NoteChange::SlideIn(None)),
        )
        .unwrap();
        assert_eq!(note(&s).slides, [Slide::OutUp]);
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Note(NoteChange::LeftFinger(Some(3))),
        )
        .unwrap();
        assert_eq!(note(&s).left_finger, Some(3));
        let stroke = Stroke {
            direction: StrokeDirection::Down,
            speed: NoteValue::Eighth,
        };
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Beat(BeatChange::Stroke(Some(stroke))),
        )
        .unwrap();
        assert_eq!(beat(&s, 1).effects.stroke, Some(stroke));
        apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Beat(BeatChange::Grace(Some(GraceKind::OnBeat))),
        )
        .unwrap();
        assert_eq!(beat(&s, 1).duration.value, NoteValue::ThirtySecond);
        assert_eq!(h.undo_label(), Some("Grace Note"));
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

    fn sel(from: usize, to: usize) -> Selection {
        Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (0, from),
            to: (0, to),
        }
    }

    #[test]
    fn a_slur_covers_the_selection_and_the_last_beat_ends_it() {
        let (mut s, mut h) = song();
        assert!(fx(&mut s, &mut h, at(0, 1), Some(sel(0, 2)), Effect::Slur));
        let slurred: Vec<bool> = (0..4).map(|i| beat(&s, i).slur).collect();
        assert_eq!(slurred, [true, true, false, false]);
        assert!(fx(&mut s, &mut h, at(0, 1), Some(sel(0, 2)), Effect::Slur));
        assert!((0..4).all(|i| !beat(&s, i).slur));
        // At the caret: from its beat to the next.
        assert!(fx(&mut s, &mut h, at(3, 3), None, Effect::Slur));
        assert!(beat(&s, 3).slur);
    }

    #[test]
    fn run_marks_take_rests_too_and_one_replaces_another() {
        let (mut s, mut h) = song();
        apply(&mut s, &mut h, at(2, 3), None, Command::Rest).unwrap();
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            Some(sel(0, 3)),
            Effect::Hairpin(Hairpin::Crescendo)
        ));
        assert!((0..4).all(|i| beat(&s, i).hairpin == Some(Hairpin::Crescendo)));
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            Some(sel(1, 2)),
            Effect::Ottava(Ottava::Alta8)
        ));
        assert_eq!(beat(&s, 2).ottava, Some(Ottava::Alta8));
        assert!(fx(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Effect::Ottava(Ottava::Bassa15)
        ));
        assert_eq!(beat(&s, 1).ottava, Some(Ottava::Bassa15));
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Effect::Beaming(Beaming::Break)
        ));
        assert_eq!(beat(&s, 0).beaming, Beaming::Break);
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Effect::Beaming(Beaming::Break)
        ));
        assert_eq!(beat(&s, 0).beaming, Beaming::Automatic);
    }

    #[test]
    fn a_barre_lies_at_the_lowest_fret_and_the_inspector_changes_it() {
        let (mut s, mut h) = song();
        assert!(fx(&mut s, &mut h, at(0, 1), Some(sel(0, 3)), Effect::Barre));
        assert!((0..4).all(|i| beat(&s, i).barre
            == Some(Barre {
                fret: 5,
                half: false
            })));
        let half = Barre {
            fret: 7,
            half: true,
        };
        apply(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Command::Beat(BeatChange::Barre(Some(half))),
        )
        .unwrap();
        assert_eq!(beat(&s, 1).barre, Some(half));
    }

    #[test]
    fn new_note_and_beat_marks_switch_on_and_off() {
        let (mut s, mut h) = song();
        assert!(fx(&mut s, &mut h, at(1, 3), None, Effect::WideLeftVibrato));
        assert_eq!(beat(&s, 1).notes[0].effects.vibrato, Some(Vibrato::Wide));
        // V replaces it with a slight one.
        assert!(fx(&mut s, &mut h, at(1, 3), None, Effect::Vibrato));
        assert_eq!(beat(&s, 1).notes[0].effects.vibrato, Some(Vibrato::Slight));
        assert!(fx(
            &mut s,
            &mut h,
            at(1, 3),
            None,
            Effect::Ornament(Ornament::Turn)
        ));
        assert_eq!(beat(&s, 1).notes[0].effects.ornament, Some(Ornament::Turn));
        assert!(fx(&mut s, &mut h, at(0, 1), None, Effect::FadeOut));
        assert_eq!(beat(&s, 0).effects.fade, Some(Fade::Out));
        assert!(fx(&mut s, &mut h, at(0, 1), None, Effect::FadeIn));
        assert_eq!(beat(&s, 0).effects.fade, Some(Fade::In));
        assert!(fx(&mut s, &mut h, at(0, 1), None, Effect::Rasgueado));
        assert_eq!(beat(&s, 0).effects.rasgueado, Some(Rasgueado::Ii));
        apply(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Command::Beat(BeatChange::Rasgueado(Some(Rasgueado::Peami))),
        )
        .unwrap();
        assert_eq!(beat(&s, 0).effects.rasgueado, Some(Rasgueado::Peami));
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Effect::Arpeggio(StrokeDirection::Up)
        ));
        assert_eq!(beat(&s, 0).effects.arpeggio, Some(StrokeDirection::Up));
        assert!(fx(
            &mut s,
            &mut h,
            at(0, 1),
            None,
            Effect::Golpe(Golpe::Thumb)
        ));
        assert!(fx(&mut s, &mut h, at(0, 1), None, Effect::DeadSlap));
        assert!(beat(&s, 0).effects.dead_slap);
        assert_eq!(h.undo_label(), Some("Dead Slap"));
    }
}
