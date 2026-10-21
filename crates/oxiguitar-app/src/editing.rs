// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The editor's state in the window: the caret, the selection, and where
//! to draw them. No window code, so every key and click is tested without
//! a display.
//!
//! The caret and the selection live on the window's thread with a shared,
//! read-only copy of the score and the engraver's map, so moving them never
//! waits for the worker and never redraws a tile: the window draws them as
//! boxes over the page. Keys follow Guitar Pro 8's shortcut list (checked
//! 19 Oct 2026): the arrows move by beat and string, Shift extends the
//! selection, Ctrl + Shift + Left and Right extend it by bar, Home and End
//! go to the bar's ends, Ctrl + Home and Ctrl + End to the first and last
//! bars, Tab and Shift + Tab to the next and previous staff, Ctrl + Up and
//! Ctrl + Down to the previous and next track, Ctrl + 1 to 4 pick the voice,
//! Ctrl + A selects everything.

use std::sync::Arc;
use std::time::{Duration, Instant};

use oxiguitar_display::Rect;
use oxiguitar_edit::{Caret, Command, Selection, StaffBar};
use oxiguitar_host::api::event::{Position, Selection as PluginSelection, Selection2};
use oxiguitar_layout::{Found, Hits, Spot, pitch_at_step};
use oxiguitar_model::{BeatKind, Clef, NoteValue, Pitch, Score, Tuplet};

/// How soon a second digit must follow the first to make a two-digit
/// fret.
const SECOND_DIGIT: Duration = Duration::from_millis(1000);

/// The keys the editor takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    BackTab,
    /// Ctrl + 1 to 4: the voice, from 0.
    Voice(usize),
    /// Ctrl + A.
    SelectAll,
    /// Escape: drop the selection.
    Escape,
}

impl Key {
    /// The key the window names, as `app.slint` passes it.
    pub(crate) fn from_name(name: &str) -> Option<Key> {
        Some(match name {
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "home" => Key::Home,
            "end" => Key::End,
            "tab" => Key::Tab,
            "backtab" => Key::BackTab,
            "1" => Key::Voice(0),
            "2" => Key::Voice(1),
            "3" => Key::Voice(2),
            "4" => Key::Voice(3),
            "a" | "A" => Key::SelectAll,
            "escape" => Key::Escape,
            _ => return None,
        })
    }
}

/// The caret, the selection and what they need.
#[derive(Default)]
pub(crate) struct Editing {
    score: Option<Arc<Score>>,
    hits: Arc<Hits>,
    /// The shown tracks, top to bottom.
    shown: Vec<usize>,
    caret: Option<Caret>,
    /// Where Shift was first held, while a selection grows.
    anchor: Option<Caret>,
    selection: Option<Selection>,
    /// The last digit typed, where and when, for a two-digit fret.
    digit: Option<(Caret, u8, Instant)>,
    /// The caret's line or space on the standard staff (0 the bottom line,
    /// 8 the top line) in a clef, where Enter enters a note; `None` while
    /// the caret is on the tablature.
    height: Option<(i32, Clef)>,
    /// What was last copied or cut: OxiGuitar's own clipboard.
    clipboard: Option<Arc<oxiguitar_edit::Clip>>,
    /// Where lyrics are being typed on the page, if they are.
    lyric: Option<Lyric>,
}

/// Where a syllable is being typed (Ctrl + T, as in MuseScore; Mateus,
/// 21 Oct 2026): a line of the caret's track's lyrics, at a beat its first
/// staff's first voice sings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lyric {
    pub(crate) track: usize,
    /// The line (verse), from 0.
    pub(crate) line: usize,
    /// The bar and beat.
    pub(crate) at: (usize, usize),
}

/// What ends a syllable being typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LyricKey {
    /// Space: the next beat.
    Space,
    /// Hyphen: the next beat, the word going on.
    Hyphen,
    /// Backspace in an empty field: the beat before.
    Back,
    /// Up / Down: the line above / below, same beat.
    Up,
    Down,
    /// Enter or Escape: done.
    Done,
}

/// The clipboard's keys, as Guitar Pro 8's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipKey {
    /// Ctrl + C.
    Copy,
    /// Ctrl + X.
    Cut,
    /// Ctrl + V.
    Paste,
    /// Ctrl + Shift + V: Special Paste.
    Special,
}

/// The clipboard key `name` with these modifiers is, if any.
pub(crate) fn clip_key(name: &str, shift: bool, ctrl: bool, alt: bool) -> Option<ClipKey> {
    if !ctrl || alt {
        return None;
    }
    match (name, shift) {
        ("c" | "C", false) => Some(ClipKey::Copy),
        ("x" | "X", false) => Some(ClipKey::Cut),
        ("v" | "V", false) => Some(ClipKey::Paste),
        ("v" | "V", true) => Some(ClipKey::Special),
        _ => None,
    }
}

/// The commands of the Edit menu's note items, by the name the menu
/// passes.
pub(crate) fn menu_command(name: &str) -> Option<Command> {
    Some(match name {
        "insert-beat" => Command::InsertBeat,
        "delete-beats" => Command::DeleteBeats,
        "clear-beats" => Command::ClearBeats,
        "clear-bar" => Command::ClearBar,
        "move-left" => Command::MoveBeatLeft,
        "move-right" => Command::MoveBeatRight,
        "voice-1" => Command::MoveToVoice(0),
        "voice-2" => Command::MoveToVoice(1),
        "voice-3" => Command::MoveToVoice(2),
        "voice-4" => Command::MoveToVoice(3),
        "remove-unused-voices" => Command::RemoveUnusedVoices,
        "insert-bar" => Command::InsertBar,
        "delete-bars" => Command::DeleteBars,
        _ => return None,
    })
}

/// How many strings the caret's staff has.
fn strings(score: &Score, c: Caret) -> usize {
    score
        .tracks
        .get(c.track)
        .and_then(|t| t.staves.get(c.staff))
        .map_or(0, |s| s.string_count())
}

/// The clef of the caret's bar.
fn clef_of(score: &Score, c: Caret) -> Clef {
    score
        .tracks
        .get(c.track)
        .and_then(|t| t.staves.get(c.staff))
        .and_then(|s| s.bars.get(c.bar))
        .map_or(Clef::Treble, |b| b.clef)
}

/// A key's digit, 0 to 9.
fn digit(name: &str) -> Option<u8> {
    match name.as_bytes() {
        [d @ b'0'..=b'9'] => Some(d - b'0'),
        _ => None,
    }
}

/// What the palette shows of the caret's beat: its voice, note value, dots,
/// whether it is a rest, whether the caret's note is tied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AtCaret {
    pub(crate) voice: usize,
    pub(crate) value: Option<NoteValue>,
    pub(crate) dots: u8,
    pub(crate) rest: bool,
    pub(crate) tied: bool,
    /// The beat is a triplet.
    pub(crate) triplet: bool,
    /// The caret's bar opens a repeat.
    pub(crate) repeat_start: bool,
    /// The caret's bar closes a repeat.
    pub(crate) repeat_end: bool,
    /// The caret's bar ends with a double bar line.
    pub(crate) double_bar: bool,
    /// The caret's bar is in free time.
    pub(crate) free_time: bool,
    /// The effects of the caret's note and beat, a bit per entry of
    /// [`EFFECTS`].
    pub(crate) effects: u64,
    /// The caret's beat's dynamic.
    pub(crate) dynamic: Option<oxiguitar_model::Dynamic>,
}

impl AtCaret {
    /// Whether the caret's note (or beat) has `e`.
    pub(crate) fn has(&self, e: oxiguitar_edit::Effect) -> bool {
        EFFECTS
            .iter()
            .position(|x| *x == e)
            .is_some_and(|i| self.effects & (1 << i) != 0)
    }
}

/// Every effect the palette shows as on or off.
pub(crate) const EFFECTS: &[oxiguitar_edit::Effect] = {
    use oxiguitar_edit::{Effect as E, HarmonicKind as H};
    use oxiguitar_model::{BeatTechnique as T, GraceKind as G, Slide, StrokeDirection as D};
    &[
        E::Dead,
        E::Ghost,
        E::Accent,
        E::HeavyAccent,
        E::Staccato,
        E::HammerPull,
        E::LeftHandTap,
        E::Slide(Slide::Legato),
        E::Slide(Slide::Shift),
        E::Slide(Slide::InFromBelow),
        E::Slide(Slide::InFromAbove),
        E::Slide(Slide::OutDown),
        E::Slide(Slide::OutUp),
        E::Vibrato,
        E::PalmMute,
        E::LetRing,
        E::Harmonic(H::Natural),
        E::Harmonic(H::Artificial),
        E::Harmonic(H::Tapped),
        E::Harmonic(H::Pinch),
        E::Harmonic(H::Semi),
        E::Trill,
        E::TremoloPicking,
        E::WideVibrato,
        E::FadeIn,
        E::Technique(T::Tap),
        E::Technique(T::Slap),
        E::Technique(T::Pop),
        E::Brush(D::Down),
        E::Brush(D::Up),
        E::PickStroke(D::Down),
        E::PickStroke(D::Up),
        E::Grace(G::BeforeBeat),
        E::Grace(G::OnBeat),
    ]
};

/// The bits of [`EFFECTS`] the caret's note and beat have: a note effect
/// as the note on the caret's string has it (every note, on a staff
/// without strings), a beat effect as the beat has it.
fn effects_at(beat: &oxiguitar_model::Beat, string: Option<u8>) -> u64 {
    let notes: Vec<&oxiguitar_model::Note> = beat
        .notes
        .iter()
        .filter(|n| string.is_none() || n.string() == string)
        .collect();
    let mut bits = 0;
    for (i, e) in EFFECTS.iter().enumerate() {
        let on = match e {
            oxiguitar_edit::Effect::TremoloPicking
            | oxiguitar_edit::Effect::WideVibrato
            | oxiguitar_edit::Effect::FadeIn
            | oxiguitar_edit::Effect::Technique(_)
            | oxiguitar_edit::Effect::Brush(_)
            | oxiguitar_edit::Effect::PickStroke(_)
            | oxiguitar_edit::Effect::Grace(_) => e.on_beat(beat),
            _ => !notes.is_empty() && notes.iter().all(|n| e.on_note(beat, n)),
        };
        if on {
            bits |= 1 << i;
        }
    }
    bits
}

/// A dialog the keys or the palette open at the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialog {
    /// The Bar dialog: signatures, clef, tempo, feel, repeats, endings,
    /// directions, section.
    Bar,
    /// Custom N-tuplet.
    Tuplet,
    /// The bend editor, for the caret's note.
    Bend,
    /// The whammy bar's editor, for the caret's beat.
    Whammy,
    /// The chord dialog, for the caret's beat.
    Chord,
    /// Text above the caret's beat.
    Text,
    /// Lyrics typed on the page, at the caret's beat (not a dialog).
    Lyrics,
}

/// The dialog a key opens, as Guitar Pro 8's keys open its own: Ctrl + K
/// (key signature), Ctrl + / (feel), D (directions) and Shift + Insert
/// (section) open the Bar dialog; B the bend editor, Ctrl + Alt + V the
/// whammy bar's; A the chord dialog; Shift + T the beat's text; Ctrl + T
/// (Guitar Pro's time signature key) starts typing lyrics on the page, as
/// in MuseScore (Mateus, 20 and 21 Oct 2026).
pub(crate) fn dialog_key(name: &str, shift: bool, ctrl: bool, alt: bool) -> Option<Dialog> {
    match (name, ctrl, alt) {
        ("k" | "K" | "/" | "?", true, false) => Some(Dialog::Bar),
        ("d" | "D", false, false) if !shift => Some(Dialog::Bar),
        ("insert", false, false) if shift => Some(Dialog::Bar),
        ("b", false, false) if !shift => Some(Dialog::Bend),
        ("a", false, false) if !shift => Some(Dialog::Chord),
        ("t" | "T", true, false) if !shift => Some(Dialog::Lyrics),
        ("T", false, false) if shift => Some(Dialog::Text),
        ("v" | "V", true, true) => Some(Dialog::Whammy),
        _ => None,
    }
}

/// The effect a key switches (M3 slice 6): Guitar Pro 8's keys, and keys
/// of our own for the effects it gives none, each the effect's first
/// letter with at most one modifier, the commonest guitar effects first
/// (Mateus, 20 Oct 2026): T tapping, Alt + S shift slide, Shift + S slide
/// out downwards, Alt + P pinch harmonic, Alt + T tapped harmonic.
pub(crate) fn effect_key(name: &str, shift: bool, ctrl: bool, alt: bool) -> Option<Command> {
    use oxiguitar_edit::{Effect as E, HarmonicKind as H};
    use oxiguitar_model::{BeatTechnique, GraceKind, Slide, StrokeDirection};
    let effect = match (name, shift, ctrl, alt) {
        ("x", false, false, false) => E::Dead,
        ("o", false, false, false) => E::Ghost,
        (";", _, false, false) => E::Accent,
        (":", _, false, false) => E::HeavyAccent,
        ("!", _, false, false) => E::Staccato,
        ("h", false, false, false) => E::HammerPull,
        ("(", _, false, false) => E::LeftHandTap,
        ("s", false, false, false) => E::Slide(Slide::Legato),
        ("s" | "S", false, false, true) => E::Slide(Slide::Shift),
        ("S", true, false, false) => E::Slide(Slide::OutDown),
        ("v", false, false, false) => E::Vibrato,
        ("p", false, false, false) => E::PalmMute,
        ("P", true, false, false) => E::PalmMuteBeat,
        ("i", false, false, false) => E::LetRing,
        ("y", false, false, false) => E::Harmonic(H::Natural),
        ("y" | "Y", false, true, true) => E::Harmonic(H::Artificial),
        ("p" | "P", false, false, true) => E::Harmonic(H::Pinch),
        ("t" | "T", false, false, true) => E::Harmonic(H::Tapped),
        ("n", false, false, false) => E::Trill,
        ("\"", _, false, false) => E::TremoloPicking,
        ("w" | "W", false, true, true) => E::WideVibrato,
        ("<", _, false, false) => E::FadeIn,
        ("t", false, false, false) | (")", _, false, false) => E::Technique(BeatTechnique::Tap),
        ("$", _, false, false) => E::Technique(BeatTechnique::Slap),
        ("d" | "D", false, true, false) => E::Brush(StrokeDirection::Down),
        ("u" | "U", false, true, false) => E::Brush(StrokeDirection::Up),
        ("D", true, false, false) => E::PickStroke(StrokeDirection::Down),
        ("U", true, false, false) => E::PickStroke(StrokeDirection::Up),
        ("g", false, false, false) => E::Grace(GraceKind::BeforeBeat),
        ("g" | "G", false, true, true) => E::Grace(GraceKind::OnBeat),
        _ => return None,
    };
    Some(Command::Effect(effect))
}

/// A command typed at the caret, as the worker takes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Typed {
    pub(crate) command: Command,
    pub(crate) caret: Caret,
    pub(crate) selection: Option<Selection>,
}

impl Editing {
    /// A score was opened: the caret goes to the start of `first`.
    pub(crate) fn opened(&mut self, score: Arc<Score>, first: usize) {
        self.lyric = None;
        self.caret = Caret::at_start(&score, first);
        self.height = None;
        self.anchor = None;
        self.selection = None;
        self.score = Some(score);
    }

    /// The score changed (an edit, undo, redo): the caret stays where it
    /// was, as far as the score still has that place; a selection whose
    /// bars went is dropped.
    pub(crate) fn changed(&mut self, score: Arc<Score>) {
        self.caret = self.caret.and_then(|c| c.clamp(&score));
        if let Some(sel) = self.selection {
            let (_, last_track) = sel.tracks();
            let (_, last_bar) = sel.bars();
            if last_track >= score.tracks.len() || last_bar >= score.bar_count() {
                self.selection = None;
                self.anchor = None;
            }
        }
        self.score = Some(score);
    }

    /// A new engraving: its map, and so the tracks shown. A caret on a
    /// track no longer shown goes to the first shown track, same bar.
    pub(crate) fn engraved(&mut self, hits: Arc<Hits>) {
        self.shown = hits.tracks();
        self.hits = hits;
        let (Some(score), Some(c), Some(&first)) = (&self.score, self.caret, self.shown.first())
        else {
            return;
        };
        if !self.shown.contains(&c.track) {
            self.caret = Some(c.to_staff(score, first, 0));
            self.selection = None;
            self.anchor = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn caret(&self) -> Option<Caret> {
        self.caret
    }

    pub(crate) fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// The shown tracks, top to bottom.
    #[cfg(test)]
    pub(crate) fn shown(&self) -> &[usize] {
        &self.shown
    }

    /// After an edit: the caret where the edit left it.
    pub(crate) fn edited(&mut self, caret: Caret) {
        self.caret = self
            .score
            .as_ref()
            .and_then(|s| caret.clamp(s))
            .or(Some(caret));
        self.anchor = None;
        self.selection = None;
    }

    /// Note entry (Guitar Pro 8's keys, `oxiguitar_edit::Command`): the
    /// command a key typed at the caret makes, with Shift, Ctrl and Alt as
    /// held, at `now` (a second digit soon after the first makes a two-digit
    /// fret). `None` when the key isn't one.
    pub(crate) fn typed(
        &mut self,
        name: &str,
        shift: bool,
        ctrl: bool,
        alt: bool,
        now: Instant,
    ) -> Option<Typed> {
        let caret = self.caret?;
        let score = self.score.clone()?;
        let last = self.digit.take();
        let command = match (name, ctrl, alt) {
            (d, false, false) if digit(d).is_some() => {
                let n = digit(d)?;
                let frets = score
                    .tracks
                    .get(caret.track)
                    .and_then(|t| t.staves.get(caret.staff))
                    .map_or(0, |s| s.fret_count);
                let joined = last
                    .filter(|(c, d, at)| {
                        *c == caret
                            && *d > 0
                            && now.duration_since(*at) < SECOND_DIGIT
                            && d * 10 + n <= frets
                    })
                    .map(|(_, d, _)| d * 10 + n);
                match joined {
                    Some(fret) => Command::Fret { fret, again: true },
                    None => {
                        self.digit = Some((caret, n, now));
                        Command::Fret {
                            fret: n,
                            again: false,
                        }
                    }
                }
            }
            ("\n" | "\r" | "enter", false, false) => Command::Pitch(self.pitch_here()?),
            ("r" | "R", false, false) => Command::Rest,
            ("-" | "_", false, false) => Command::Longer,
            ("+" | "=", false, false) => Command::Shorter,
            (".", false, false) if shift => Command::Dot,
            (">", false, false) => Command::Dot,
            (".", true, false) => Command::DoubleDot,
            ("l", false, false) if !shift => Command::Tie,
            ("l" | "L", false, false) => Command::TieBeat,
            ("backspace" | "delete", false, false) if !shift => Command::DeleteNote,
            ("delete", true, false) if shift => Command::ClearBeats,
            ("delete", false, false) => Command::DeleteBeats,
            ("1" | "2" | "3" | "4", false, true) => {
                Command::MoveToVoice(usize::from(digit(name)? - 1))
            }
            ("insert", true, false) => Command::InsertBeat,
            ("insert", false, false) if !shift => Command::InsertBar,
            ("delete", true, false) => Command::DeleteBars,
            ("/" | "?", false, false) => Command::Tuplet(Tuplet::TRIPLET),
            ("[" | "{", false, false) => Command::RepeatStart,
            ("]" | "}", false, false) => Command::RepeatEnd,
            ("|", false, false) => Command::FreeTime,
            ("l" | "L", false, true) if shift => Command::FreeTime,
            ("up", false, true) if shift => Command::SemitoneUp,
            ("down", false, true) if shift => Command::SemitoneDown,
            ("up", false, true) => Command::StringUp,
            ("down", false, true) => Command::StringDown,
            _ => effect_key(name, shift, ctrl, alt)?,
        };
        Some(Typed {
            command,
            caret,
            selection: self.selection,
        })
    }

    /// The pitch at the caret's line or space of the standard staff, in its
    /// bar's key and its staff's transposition.
    fn pitch_here(&self) -> Option<Pitch> {
        let (step, clef) = self.height?;
        let c = self.caret?;
        let score = self.score.as_ref()?;
        let key = score.master_bars.get(c.bar)?.key_signature;
        let transposition = score
            .tracks
            .get(c.track)?
            .staves
            .get(c.staff)?
            .transposition;
        pitch_at_step(step, clef, key, transposition)
    }

    /// What the Bar dialog opens on: the caret's bar (the selection's
    /// first), and its heading ("Bar 3", or "Bars 3–6" for a selection).
    pub(crate) fn bar_state(&self) -> Option<(oxiguitar_edit::BarState, String)> {
        let c = self.caret?;
        let score = self.score.as_ref()?;
        let (first, last) = self.selection.map_or((c.bar, c.bar), |s| s.bars());
        let state = oxiguitar_edit::BarState::at(score, Caret { bar: first, ..c })?;
        let heading = if first == last {
            format!("Bar {}", first + 1)
        } else {
            format!("Bars {}–{}", first + 1, last + 1)
        };
        Some((state, heading))
    }

    /// The score the caret is in.
    pub(crate) fn score(&self) -> Option<Arc<Score>> {
        self.score.clone()
    }

    /// The Inspector's view of the caret's bar.
    pub(crate) fn bar_view(&self) -> Option<crate::inspector::BarView> {
        crate::inspector::bar_view(self.score.as_ref()?, self.caret?)
    }

    /// What the bend editor opens on: the bend of the note on the caret's
    /// string, or none. `None` when there is nothing to bend: no note on a
    /// string there and no selection, or a drum track.
    pub(crate) fn bend_here(&self) -> Option<Option<oxiguitar_model::Bend>> {
        let c = self.caret?;
        let score = self.score.as_ref()?;
        let track = score.tracks.get(c.track)?;
        if track.instrument.percussion || strings(score, c) == 0 {
            return None;
        }
        let note = c
            .voice_in(score)
            .and_then(|v| v.beats.get(c.beat))
            .filter(|b| b.kind == BeatKind::Notes)
            .and_then(|b| b.notes.iter().find(|n| n.string() == Some(c.string)));
        match note {
            Some(n) => Some(n.effects.bend.clone()),
            None => self.selection.map(|_| None),
        }
    }

    /// What the whammy bar's editor opens on: the caret's beat's curve, or
    /// none. `None` when the beat has no notes and nothing is selected, or
    /// on a track without strings.
    pub(crate) fn whammy_here(&self) -> Option<Option<oxiguitar_model::Bend>> {
        let c = self.caret?;
        let score = self.score.as_ref()?;
        let track = score.tracks.get(c.track)?;
        if track.instrument.percussion || strings(score, c) == 0 {
            return None;
        }
        let beat = c
            .voice_in(score)
            .and_then(|v| v.beats.get(c.beat))
            .filter(|b| b.kind == BeatKind::Notes);
        match beat {
            Some(b) => Some(b.effects.whammy.clone()),
            None => self.selection.map(|_| None),
        }
    }

    /// The Inspector's rows for the caret's note and beat.
    pub(crate) fn effect_fields(&self) -> Vec<crate::effect_form::Field> {
        match (self.score.as_ref(), self.caret) {
            (Some(s), Some(c)) => crate::effect_form::fields(s, c),
            _ => Vec::new(),
        }
    }

    /// A field of the Inspector's note or beat changed.
    pub(crate) fn field_changed(&self, id: &str, value: i32) -> Option<crate::effect_form::Action> {
        crate::effect_form::changed(self.score.as_ref()?, self.caret?, id, value)
    }

    /// Starts typing lyrics (Ctrl + T): line 1 at the caret's beat, or the
    /// next beat the track's first voice sings. The syllable there, to
    /// edit. `None` when nothing is sung from there on.
    pub(crate) fn start_lyrics(&mut self) -> Option<(Lyric, String)> {
        let c = self.caret?;
        let score = self.score.clone()?;
        let staff = score.tracks.get(c.track)?.staves.first()?;
        // The caret's moment, in the first voice.
        let onset = c
            .voice_in(&score)
            .and_then(|v| oxiguitar_edit::onsets(v).get(c.beat).copied());
        let first = staff.bars.get(c.bar).and_then(|b| b.voices.first());
        let from = match (onset, first) {
            (Some(at), Some(v)) => oxiguitar_edit::onsets(v)
                .iter()
                .position(|o| *o >= at)
                .unwrap_or(v.beats.len()),
            _ => 0,
        };
        let at = oxiguitar_model::lyrics::sung_beats(staff, c.bar)
            .into_iter()
            .find(|&(bar, beat)| bar > c.bar || beat >= from)?;
        let lyric = Lyric {
            track: c.track,
            line: 0,
            at,
        };
        self.lyric = Some(lyric);
        self.follow_lyric();
        Some((lyric, self.syllable_at(lyric).0))
    }

    /// The syllable of line `l.line` at its beat, as typed, and whether a
    /// hyphen follows it.
    pub(crate) fn syllable_at(&self, l: Lyric) -> (String, bool) {
        let Some(score) = self.score.as_ref() else {
            return (String::new(), false);
        };
        let Some(track) = score.tracks.get(l.track) else {
            return (String::new(), false);
        };
        let (Some(line), Some(staff)) = (track.lyrics.get(l.line), track.staves.first()) else {
            return (String::new(), false);
        };
        let index = oxiguitar_model::lyrics::sung_beats(staff, line.start_bar)
            .iter()
            .position(|b| *b == l.at);
        index
            .and_then(|i| {
                oxiguitar_model::lyrics::syllables(&line.text)
                    .get(i)
                    .cloned()
            })
            .map_or((String::new(), false), |s| (s.text, s.hyphen))
    }

    /// A key ended the syllable `text`: the edit that sets it (when it
    /// changed), and where typing goes on (`None`: done).
    pub(crate) fn lyric_key(
        &mut self,
        key: LyricKey,
        text: &str,
    ) -> (Option<Typed>, Option<(Lyric, String)>) {
        let Some(l) = self.lyric else {
            return (None, None);
        };
        let text = text.trim();
        let (was, was_hyphen) = self.syllable_at(l);
        let hyphen = match key {
            LyricKey::Hyphen => true,
            LyricKey::Space => false,
            _ => was_hyphen,
        };
        let lines = self
            .score
            .as_ref()
            .and_then(|s| s.tracks.get(l.track))
            .map_or(0, |t| t.lyrics.len());
        let next = match key {
            LyricKey::Space | LyricKey::Hyphen => {
                self.sung_next(l, true).map(|at| Lyric { at, ..l })
            }
            LyricKey::Back => self.sung_next(l, false).map(|at| Lyric { at, ..l }),
            LyricKey::Up => Some(Lyric {
                line: l.line.saturating_sub(1),
                ..l
            }),
            // The line below, a new one under the last too (once this one
            // holds something), five at most.
            LyricKey::Down => Some(Lyric {
                line: if l.line < lines || !text.is_empty() {
                    (l.line + 1).min(oxiguitar_edit::LYRIC_LINES - 1)
                } else {
                    l.line
                },
                ..l
            }),
            LyricKey::Done => None,
        };
        // The caret goes on first, so the edit leaves it where typing goes
        // on, however late the edit is done.
        self.lyric = next;
        self.follow_lyric();
        let edit = (text != was || hyphen != was_hyphen)
            .then(|| {
                self.tool(Command::Syllable {
                    line: l.line,
                    at: l.at,
                    text: text.to_owned(),
                    hyphen,
                })
            })
            .flatten();
        let next = next.map(|n| {
            // The syllable typed is not in the score yet: on the same
            // beat, it is what the field shows again.
            let shown = if n.at == l.at && n.line == l.line {
                text.to_owned()
            } else {
                self.syllable_at(n).0
            };
            (n, shown)
        });
        (edit, next)
    }

    /// The beat after (or before) `l`'s that the first voice sings.
    fn sung_next(&self, l: Lyric, forward: bool) -> Option<(usize, usize)> {
        let staff = self.score.as_ref()?.tracks.get(l.track)?.staves.first()?;
        let sung = oxiguitar_model::lyrics::sung_beats(staff, 0);
        let i = sung.iter().position(|b| *b == l.at)?;
        if forward {
            sung.get(i + 1).copied()
        } else {
            sung.get(i.checked_sub(1)?).copied()
        }
    }

    /// The caret goes where lyrics are typed, so the view follows.
    fn follow_lyric(&mut self) {
        if let Some(l) = self.lyric {
            self.caret = Some(Caret {
                track: l.track,
                staff: 0,
                voice: 0,
                bar: l.at.0,
                beat: l.at.1,
                string: self.caret.map_or(1, |c| c.string),
            });
            self.selection = None;
            self.anchor = None;
            self.height = None;
        }
    }

    /// Where the field for lyrics goes on the page: its sheet, the beat's
    /// middle, the line's baseline and the space between lines.
    pub(crate) fn lyric_spot(&self) -> Option<(usize, f32, f32, f32)> {
        let l = self.lyric?;
        self.hits.lyric_spot(l.track, l.at.0, l.at.1, l.line)
    }

    /// The chord dialog for the caret's beat.
    pub(crate) fn chord_form(&self) -> Option<crate::chord_form::ChordForm> {
        crate::chord_form::ChordForm::open(self.score.as_ref()?, self.caret?)
    }

    /// The text above the caret's beat; `None` where there is no beat.
    pub(crate) fn beat_text(&self) -> Option<String> {
        let c = self.caret?;
        let beat = c.voice_in(self.score.as_ref()?)?.beats.get(c.beat)?;
        (beat.kind != BeatKind::Empty).then(|| beat.text.clone())
    }

    /// The tuplet of the caret's beat, if it is in one.
    pub(crate) fn beat_tuplet(&self) -> Option<Tuplet> {
        let c = self.caret?;
        let score = self.score.as_ref()?;
        c.voice_in(score)?
            .beats
            .get(c.beat)?
            .duration
            .tuplets
            .first()
            .copied()
    }

    /// The caret's line or space on the standard staff, if it is there.
    #[cfg(test)]
    pub(crate) fn height(&self) -> Option<(i32, Clef)> {
        self.height
    }

    /// Right with the caret on the last place of the last bar: a bar to add
    /// after it, where the caret goes.
    pub(crate) fn at_the_end(&mut self) -> Option<Typed> {
        let score = self.score.clone()?;
        let c = self.caret?;
        (c.bar + 1 == score.bar_count() && c.next_beat(&score) == c)
            .then(|| self.tool(Command::AppendBar))
            .flatten()
    }

    /// A palette tool's command at the caret, on the selection.
    pub(crate) fn tool(&mut self, command: Command) -> Option<Typed> {
        self.digit = None;
        Some(Typed {
            command,
            caret: self.caret?,
            selection: self.selection,
        })
    }

    /// Copies the selection, or the caret's beat. Whether there was
    /// anything to copy.
    pub(crate) fn copy(&mut self) -> bool {
        let (Some(score), Some(c)) = (self.score.as_ref(), self.caret) else {
            return false;
        };
        match oxiguitar_edit::copy(score, c, self.selection) {
            Some(clip) => {
                self.clipboard = Some(Arc::new(clip));
                true
            }
            None => false,
        }
    }

    /// Copies, then takes out what was copied.
    pub(crate) fn cut(&mut self) -> Option<Typed> {
        if !self.copy() {
            return None;
        }
        let typed = self.tool(Command::Cut);
        self.selection = None;
        self.anchor = None;
        typed
    }

    /// The clipboard pasted `times` times, replacing what is there or not,
    /// at the selection's start or else the caret.
    pub(crate) fn paste(&mut self, times: u8, replace: bool) -> Option<Typed> {
        let clip = self.clipboard.clone()?;
        let mut typed = self.tool(Command::Paste(oxiguitar_edit::Paste {
            clip,
            times,
            replace,
        }))?;
        match self.selection.take() {
            Some(Selection::Beats { from, .. }) => {
                typed.caret = Caret {
                    bar: from.0,
                    beat: from.1,
                    ..typed.caret
                };
            }
            Some(Selection::Bars { tracks, bars }) => {
                if let Some(score) = self.score.as_ref() {
                    let at = typed.caret.to_staff(score, tracks.0, 0);
                    typed.caret = Caret {
                        bar: bars.0,
                        beat: 0,
                        ..at
                    };
                }
            }
            None => {}
        }
        typed.selection = None;
        self.anchor = None;
        Some(typed)
    }

    /// The score's sections, in order: each one's bar and how the Go to
    /// Section menu names it ("A Verse — bar 5").
    pub(crate) fn sections(&self) -> Vec<(usize, String)> {
        let Some(score) = self.score.as_ref() else {
            return Vec::new();
        };
        score
            .master_bars
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let m = b.marker.as_ref()?;
                let name = format!("{} {}", m.letter.trim(), m.text.trim());
                let name = name.trim();
                let name = if name.is_empty() { "Section" } else { name };
                Some((i, format!("{name} — bar {}", i + 1)))
            })
            .collect()
    }

    /// The caret to the start of `bar`, on its staff.
    pub(crate) fn go_to_bar(&mut self, bar: usize) -> bool {
        let Some(c) = self.caret else {
            return false;
        };
        self.go_to(StaffBar {
            track: c.track,
            staff: c.staff,
            bar,
        })
    }

    /// Something has been copied.
    pub(crate) fn has_clip(&self) -> bool {
        self.clipboard.is_some()
    }

    /// The caret's beat as the palette shows it.
    pub(crate) fn at_caret(&self) -> Option<AtCaret> {
        let c = self.caret?;
        let score = self.score.as_ref()?;
        let beat = c.voice_in(score).and_then(|v| v.beats.get(c.beat));
        let bar = score.master_bars.get(c.bar);
        Some(AtCaret {
            voice: c.voice,
            value: beat.map(|b| b.duration.value),
            dots: beat.map_or(0, |b| b.duration.dots),
            rest: beat.is_some_and(|b| b.kind == BeatKind::Rest),
            tied: beat.is_some_and(|b| {
                b.notes
                    .iter()
                    .any(|n| n.tie && n.string().is_none_or(|s| s == c.string))
            }),
            triplet: beat.is_some_and(|b| b.duration.tuplets == [Tuplet::TRIPLET]),
            repeat_start: bar.is_some_and(|b| b.repeat_start),
            repeat_end: bar.is_some_and(|b| b.repeat_end.is_some()),
            double_bar: bar.is_some_and(|b| b.double_bar),
            free_time: bar.is_some_and(|b| b.free_time),
            effects: beat.map_or(0, |b| {
                effects_at(b, (strings(score, c) > 0).then_some(c.string))
            }),
            dynamic: beat.and_then(|b| {
                b.effects.dynamic.or_else(|| {
                    (b.kind == BeatKind::Notes)
                        .then(|| b.notes.first().map(|n| n.dynamic))
                        .flatten()
                })
            }),
        })
    }

    /// A key, with Shift and Ctrl as held. Returns whether it was the
    /// editor's (the caret or the selection changed, or would have).
    pub(crate) fn key(&mut self, key: Key, shift: bool, ctrl: bool) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let s = &*score;
        let shown = self.shown.clone();
        let moved = match (key, ctrl) {
            (Key::Left, false) => c.prev_beat(s),
            (Key::Right, false) => c.next_beat(s),
            // Ctrl + Shift + arrows extend by bar; Ctrl alone with Left and
            // Right is the player's (Guitar Pro 8: rewind, fast forward).
            (Key::Left, true) if shift => c.prev_beat(s).bar_start(),
            (Key::Right, true) if shift => {
                if c.bar + 1 < s.bar_count() {
                    c.to_bar(s, c.bar + 1).bar_end(s)
                } else {
                    c.bar_end(s)
                }
            }
            (Key::Left | Key::Right, true) => return false,
            (Key::Up, false) if shift => c,
            (Key::Down, false) if shift => c,
            (Key::Up | Key::Down, false) if self.height.is_some() || strings(s, c) == 0 => {
                // On the standard staff: a line or space up or down.
                let (step, clef) = self.height.unwrap_or((4, clef_of(s, c)));
                let step = if key == Key::Up { step + 1 } else { step - 1 };
                self.height = Some((step.clamp(-8, 16), clef));
                c
            }
            (Key::Up, false) => c.string_up(),
            (Key::Down, false) => c.string_down(s),
            (Key::Up, true) => c.prev_track(s, &shown),
            (Key::Down, true) => c.next_track(s, &shown),
            (Key::Home, false) => c.bar_start(),
            (Key::End, false) => c.bar_end(s),
            (Key::Home, true) => c.first_bar(s),
            (Key::End, true) => c.last_bar(s).bar_end(s),
            (Key::Tab, _) => {
                self.height = None;
                c.next_staff(s, &shown)
            }
            (Key::BackTab, _) => {
                self.height = None;
                c.prev_staff(s, &shown)
            }
            (Key::Voice(v), true) => c.with_voice(s, v),
            (Key::Voice(_), false) => return false,
            (Key::SelectAll, true) => {
                self.selection = Selection::all(s);
                self.anchor = None;
                return true;
            }
            (Key::SelectAll, false) => return false,
            (Key::Escape, _) => {
                let had = self.selection.is_some();
                self.selection = None;
                self.anchor = None;
                return had;
            }
        };
        // Ctrl + End goes to the last bar's start in Guitar Pro; its last
        // beat is where a selection to the end must reach. Without Shift,
        // the start of the bar.
        let moved = if key == Key::End && ctrl && !shift {
            moved.bar_start()
        } else {
            moved
        };
        let extends = shift && !matches!(key, Key::Tab | Key::BackTab | Key::Voice(_));
        if extends {
            let anchor = *self.anchor.get_or_insert(c);
            self.selection = Some(Selection::between(anchor, moved));
        } else {
            self.anchor = None;
            self.selection = None;
        }
        self.caret = Some(moved);
        true
    }

    /// A press on the page at `spot` (from [`Hits::hit`]), with Shift as
    /// held: the caret goes there, in its voice, onto the string pressed
    /// (if on the tablature); with Shift, the selection grows to it.
    pub(crate) fn press(&mut self, spot: Spot, shift: bool) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let to = Caret {
            track: spot.track,
            staff: spot.staff,
            bar: spot.bar,
            beat: spot.beat.unwrap_or(0),
            string: spot.string.unwrap_or(c.string),
            ..c
        };
        let Some(to) = to.clamp(&score) else {
            return false;
        };
        self.height = spot.step;
        if shift {
            let anchor = *self.anchor.get_or_insert(c);
            self.selection = Some(Selection::between(anchor, to));
        } else {
            self.anchor = None;
            self.selection = None;
        }
        self.caret = Some(to);
        true
    }

    /// A double-click at (`x`, `y`) on sheet `sheet`: on a time signature,
    /// the caret goes to the start of its bar. Whether it was on one.
    pub(crate) fn time_signature_at(&mut self, sheet: usize, x: f32, y: f32) -> bool {
        let Some(spot) = self.hits.time_signature_at(sheet, x, y) else {
            return false;
        };
        self.go_to(StaffBar {
            track: spot.track,
            staff: spot.staff,
            bar: spot.bar,
        })
    }

    /// A press at (`x`, `y`) on sheet `sheet`, in the sheet's points: the
    /// place there in the caret's voice, as [`Editing::press`] takes it.
    pub(crate) fn press_at(&mut self, sheet: usize, x: f32, y: f32, shift: bool) -> bool {
        let voice = self.caret.map_or(0, |c| c.voice);
        match self.hits.hit(sheet, x, y, voice) {
            Some(spot) => self.press(spot, shift),
            None => false,
        }
    }

    /// A right-click at (`x`, `y`) on sheet `sheet`, before the page's
    /// menu opens: inside the selection it stays, so its commands act on
    /// it; elsewhere the caret goes there as for a press.
    pub(crate) fn menu_press_at(&mut self, sheet: usize, x: f32, y: f32) -> bool {
        let inside = self
            .selection_boxes()
            .iter()
            .any(|(i, r)| *i == sheet && x >= r.x && x < r.right() && y >= r.y && y < r.bottom());
        !inside && self.press_at(sheet, x, y, false)
    }

    /// A click on a track's row: the track is selected, and the caret goes
    /// to it (same bar) when it is shown.
    pub(crate) fn select_track(&mut self, track: usize) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        if c.track == track || track >= score.tracks.len() {
            return false;
        }
        self.caret = Some(c.to_staff(&score, track, 0));
        self.selection = None;
        self.anchor = None;
        true
    }

    /// The track selected in the track list: the caret's.
    pub(crate) fn track(&self) -> Option<usize> {
        self.caret.map(|c| c.track)
    }

    /// Puts the caret on the first beat of `at`, in voice 1, and drops the
    /// selection (Tools → Next Bar That Doesn't Add Up).
    pub(crate) fn go_to(&mut self, at: StaffBar) -> bool {
        let (Some(score), Some(c)) = (self.score.clone(), self.caret) else {
            return false;
        };
        let to = Caret {
            track: at.track,
            staff: at.staff,
            bar: at.bar,
            voice: 0,
            beat: 0,
            ..c
        }
        .clamp(&score);
        self.caret = to.or(self.caret);
        self.selection = None;
        self.anchor = None;
        true
    }

    /// Tools → Next Bar That Doesn't Add Up (F4): of `bad`, the first bar
    /// of a shown track after the caret's, in score order (bar by bar,
    /// then top to bottom), going round to the start.
    pub(crate) fn next_bad(&self, bad: &[StaffBar]) -> Option<StaffBar> {
        let from = self.caret?.staff_bar();
        let key = |b: &StaffBar| (b.bar, b.track, b.staff);
        let mut shown: Vec<StaffBar> = bad
            .iter()
            .copied()
            .filter(|b| self.shown.contains(&b.track))
            .collect();
        shown.sort_by_key(key);
        shown
            .iter()
            .find(|b| key(b) > key(&from))
            .or_else(|| shown.first())
            .copied()
    }

    /// Whether any of `bad` is on a shown track.
    pub(crate) fn any_shown(&self, bad: &[StaffBar]) -> bool {
        bad.iter().any(|b| self.shown.contains(&b.track))
    }

    /// Where to draw the caret, from the engraver's map.
    pub(crate) fn caret_box(&self) -> Option<Found> {
        let c = self.caret?;
        self.hits.find(
            c.track,
            c.staff,
            c.bar,
            c.voice,
            c.beat,
            c.string,
            self.height,
        )
    }

    /// Where to draw the selection: a box per bar of each staff, with its
    /// sheet.
    pub(crate) fn selection_boxes(&self) -> Vec<(usize, Rect)> {
        let (Some(sel), Some(score)) = (self.selection, &self.score) else {
            return Vec::new();
        };
        sel.spans(score)
            .into_iter()
            .filter_map(|sp| {
                self.hits
                    .span(sp.at.track, sp.at.staff, sp.at.bar, sp.beats)
            })
            .collect()
    }

    /// The selection as plugins see it (interface 1.1): one track, beats
    /// counted in the bar's first voice. The caret alone is a selection of
    /// one beat; a selection across tracks is given for its first track.
    pub(crate) fn for_plugins(&self) -> Option<PluginSelection> {
        let score = self.score.as_ref()?;
        let c = self.caret?;
        let pos = |bar: usize, beat: usize| Position {
            bar: u32::try_from(bar).unwrap_or(u32::MAX),
            beat: u32::try_from(beat).unwrap_or(u32::MAX),
        };
        let first_voice = |track: usize, staff: usize, voice: usize, bar: usize, beat: usize| {
            Caret {
                track,
                staff,
                voice,
                bar,
                beat,
                string: 1,
            }
            .beat_in_first_voice(score)
        };
        let (track, from, to) = match self.selection {
            None => {
                let b = c.beat_in_first_voice(score);
                (c.track, pos(c.bar, b), pos(c.bar, b))
            }
            Some(Selection::Beats {
                track,
                staff,
                voice,
                from,
                to,
            }) => (
                track,
                pos(from.0, first_voice(track, staff, voice, from.0, from.1)),
                pos(to.0, first_voice(track, staff, voice, to.0, to.1)),
            ),
            Some(Selection::Bars { tracks, bars }) => {
                let last = score
                    .tracks
                    .get(tracks.0)
                    .and_then(|t| t.staves.first())
                    .and_then(|s| s.bars.get(bars.1))
                    .and_then(|b| b.voices.first())
                    .map_or(0, |v| v.beats.len().saturating_sub(1));
                (tracks.0, pos(bars.0, 0), pos(bars.1, last))
            }
        };
        Some(PluginSelection {
            track: u32::try_from(track).unwrap_or(u32::MAX),
            from,
            to,
        })
    }

    /// The selection as plugins built for interface 1.2 see it: the caret
    /// with its voice and string, a run of beats of one voice, or bars
    /// across tracks.
    pub(crate) fn for_plugins2(&self) -> Option<Selection2> {
        let score = self.score.as_ref()?;
        let c = self.caret?;
        let n = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
        let pos = |(bar, beat): (usize, usize)| Position {
            bar: n(bar),
            beat: n(beat),
        };
        Some(match self.selection {
            None => {
                let stringed = score
                    .tracks
                    .get(c.track)
                    .and_then(|t| t.staves.get(c.staff))
                    .is_some_and(|s| s.string_count() > 0);
                Selection2::Caret {
                    track: n(c.track),
                    staff: n(c.staff),
                    voice: n(c.voice),
                    bar: n(c.bar),
                    beat: n(c.beat),
                    string: if stringed { c.string } else { 0 },
                }
            }
            Some(Selection::Beats {
                track,
                staff,
                voice,
                from,
                to,
            }) => Selection2::Beats {
                track: n(track),
                staff: n(staff),
                voice: n(voice),
                from: pos(from),
                to: pos(to),
            },
            Some(Selection::Bars { tracks, bars }) => Selection2::Bars {
                first_track: n(tracks.0),
                last_track: n(tracks.1),
                first_bar: n(bars.0),
                last_bar: n(bars.1),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_layout::{BarHits, BeatHit, StaffHits, SystemHits};
    use oxiguitar_model::{Bar, Beat, Duration, MasterBar, Note, NoteValue, Staff, Track, Voice};

    fn score(tracks: usize, bars: usize) -> Arc<Score> {
        let beat = Beat {
            duration: Duration::new(NoteValue::Quarter),
            notes: vec![Note::fretted(1, 0)],
            ..Beat::default()
        };
        let bar = Bar {
            voices: vec![Voice {
                beats: vec![beat; 4],
            }],
            ..Bar::default()
        };
        Arc::new(Score {
            master_bars: vec![MasterBar::default(); bars],
            tracks: (0..tracks)
                .map(|_| Track {
                    staves: vec![Staff {
                        tuning: Staff::standard_guitar_tuning(),
                        bars: vec![bar.clone(); bars],
                        ..Staff::default()
                    }],
                    ..Track::default()
                })
                .collect(),
            ..Score::new()
        })
    }

    /// One system, the given tracks, every bar 100 points wide.
    fn hits(tracks: &[usize], bars: usize) -> Arc<Hits> {
        let staves = tracks
            .iter()
            .enumerate()
            .map(|(k, &track)| {
                let top = 100.0 + 120.0 * k as f32;
                StaffHits {
                    track,
                    staff: 0,
                    standard: None,
                    tablature: Some((top, 6.0, 6)),
                    top,
                    bottom: top + 30.0,
                    lyrics: (top + 45.0, 12.0),
                    bars: (0..bars)
                        .map(|bar| {
                            let start = 100.0 * bar as f32;
                            BarHits {
                                bar,
                                start,
                                notes: start + 5.0,
                                end: start + 100.0,
                                clef: Clef::Treble8vb,
                                time_signature: None,
                                beats: (0..4)
                                    .map(|beat| BeatHit {
                                        voice: 0,
                                        beat,
                                        x: start + 10.0 + 25.0 * beat as f32,
                                        width: 25.0,
                                    })
                                    .collect(),
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        Arc::new(Hits {
            systems: vec![SystemHits {
                sheet: 0,
                top: 90.0,
                bottom: 600.0,
                staves,
            }],
        })
    }

    fn editing(tracks: usize, shown: &[usize]) -> Editing {
        let mut e = Editing::default();
        e.opened(score(tracks, 3), shown[0]);
        e.engraved(hits(shown, 3));
        e
    }

    #[test]
    fn keys_move_the_caret_as_guitar_pro_does() {
        let mut e = editing(3, &[0, 2]);
        assert!(e.key(Key::Right, false, false));
        assert_eq!(e.caret().unwrap().beat, 1);
        e.key(Key::Down, false, false);
        e.key(Key::Down, false, false);
        assert_eq!(e.caret().unwrap().string, 3);
        e.key(Key::Down, false, true);
        assert_eq!(e.caret().unwrap().track, 2, "the next shown track");
        e.key(Key::End, false, true);
        assert_eq!((e.caret().unwrap().bar, e.caret().unwrap().beat), (2, 0));
        e.key(Key::End, false, false);
        assert_eq!(e.caret().unwrap().beat, 3);
        e.key(Key::Home, false, true);
        assert_eq!(e.caret().unwrap().bar, 0);
        assert!(e.key(Key::Voice(1), false, true));
        assert_eq!(e.caret().unwrap().voice, 1);
        assert!(!e.key(Key::Voice(1), false, false), "1 alone is not ours");
        assert!(
            !e.key(Key::Right, false, true),
            "Ctrl + Right is the player's"
        );
        let b = e.caret_box().unwrap();
        assert_eq!(b.sheet, 0);
        assert!(b.string.is_some());
    }

    #[test]
    fn shift_selects_and_a_plain_move_drops_it() {
        let mut e = editing(2, &[0, 1]);
        e.key(Key::Right, true, false);
        e.key(Key::Right, true, false);
        assert_eq!(
            e.selection(),
            Some(Selection::Beats {
                track: 0,
                staff: 0,
                voice: 0,
                from: (0, 0),
                to: (0, 2)
            })
        );
        assert_eq!(e.selection_boxes().len(), 1);
        e.key(Key::Right, true, true);
        assert_eq!(
            e.selection().unwrap().bars(),
            (0, 1),
            "to the next bar's end"
        );
        let sel = e.for_plugins().unwrap();
        assert_eq!(
            (sel.from.bar, sel.from.beat, sel.to.bar, sel.to.beat),
            (0, 0, 1, 3)
        );
        e.key(Key::Left, false, false);
        assert_eq!(e.selection(), None);
        assert!(e.key(Key::SelectAll, false, true));
        assert_eq!(e.selection_boxes().len(), 6, "two tracks, three bars");
        assert!(e.key(Key::Escape, false, false));
        assert!(!e.key(Key::Escape, false, false), "nothing left to drop");
    }

    #[test]
    fn a_press_puts_the_caret_on_the_string_and_shift_press_selects() {
        let mut e = editing(2, &[0, 1]);
        let spot = e.hits.hit(0, 136.0, 112.0, 0).unwrap();
        assert!(e.press(spot, false));
        let c = e.caret().unwrap();
        assert_eq!((c.track, c.bar, c.beat, c.string), (0, 1, 1, 3));
        let spot = e.hits.hit(0, 260.0, 230.0, 0).unwrap();
        e.press(spot, true);
        assert!(matches!(e.selection(), Some(Selection::Bars { .. })));
    }

    #[test]
    fn a_right_click_keeps_the_selection_it_falls_in() {
        let mut e = editing(2, &[0, 1]);
        e.key(Key::Right, true, false);
        e.key(Key::Right, true, false);
        let (sheet, r) = e.selection_boxes()[0];
        let (x, y) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
        assert!(!e.menu_press_at(sheet, x, y), "inside: nothing moves");
        assert!(e.selection().is_some());
        assert!(e.menu_press_at(0, 236.0, 112.0), "outside: a press");
        assert_eq!(e.selection(), None);
        assert_eq!(e.caret().unwrap().bar, 2);
    }

    #[test]
    fn tracks_hidden_and_selected() {
        let mut e = editing(3, &[0, 1, 2]);
        assert!(e.select_track(2));
        assert_eq!(e.track(), Some(2));
        assert!(!e.select_track(2), "already");
        // Track 3 is hidden: the caret goes to the first shown.
        e.engraved(hits(&[1], 3));
        assert_eq!(e.track(), Some(1));
        assert_eq!(e.shown(), [1]);
        assert!(e.go_to(StaffBar {
            track: 1,
            staff: 0,
            bar: 2
        }));
        assert_eq!(e.caret().unwrap().bar, 2);
        // The score shrank under the caret.
        e.changed(score(1, 1));
        let c = e.caret().unwrap();
        assert_eq!((c.track, c.bar), (0, 0));
    }

    #[test]
    fn f4_walks_the_shown_bad_bars() {
        let mut e = editing(3, &[0, 2]);
        let at = |track, bar| StaffBar {
            track,
            staff: 0,
            bar,
        };
        let bad = [at(1, 0), at(2, 2), at(0, 2), at(2, 1)];
        assert!(e.any_shown(&bad));
        assert_eq!(
            e.next_bad(&bad),
            Some(at(2, 1)),
            "track 1 is hidden, so its bar 1 is skipped"
        );
        e.go_to(at(2, 1));
        assert_eq!(e.next_bad(&bad), Some(at(0, 2)));
        e.go_to(at(2, 2));
        assert_eq!(e.next_bad(&bad), Some(at(2, 1)), "round to the start");
        assert!(!e.any_shown(&[at(1, 0)]));
        assert_eq!(e.next_bad(&[]), None);
    }

    #[test]
    fn keys_typed_at_the_caret_are_note_entry() {
        let mut e = editing(1, &[0]);
        let t0 = Instant::now();
        let typed = |e: &mut Editing, name: &str, shift, ctrl, alt, ms| {
            e.typed(
                name,
                shift,
                ctrl,
                alt,
                t0 + std::time::Duration::from_millis(ms),
            )
            .map(|t| t.command)
        };
        assert_eq!(
            typed(&mut e, "1", false, false, false, 0),
            Some(Command::Fret {
                fret: 1,
                again: false
            })
        );
        assert_eq!(
            typed(&mut e, "2", false, false, false, 400),
            Some(Command::Fret {
                fret: 12,
                again: true
            }),
            "a second digit soon after: fret 12"
        );
        assert_eq!(
            typed(&mut e, "3", false, false, false, 600),
            Some(Command::Fret {
                fret: 3,
                again: false
            }),
            "a third digit starts again"
        );
        assert_eq!(
            typed(&mut e, "5", false, false, false, 2000),
            Some(Command::Fret {
                fret: 5,
                again: false
            }),
            "too late for a second digit"
        );
        assert_eq!(
            typed(&mut e, "9", false, false, false, 2100),
            Some(Command::Fret {
                fret: 9,
                again: false
            }),
            "59 is past the frets"
        );
        for (name, shift, ctrl, alt, command) in [
            ("r", false, false, false, Command::Rest),
            ("-", false, false, false, Command::Longer),
            ("+", false, false, false, Command::Shorter),
            ("=", false, false, false, Command::Shorter),
            (".", true, false, false, Command::Dot),
            (".", false, true, false, Command::DoubleDot),
            ("l", false, false, false, Command::Tie),
            ("L", true, false, false, Command::TieBeat),
            ("backspace", false, false, false, Command::DeleteNote),
            ("delete", true, false, false, Command::DeleteBeats),
            ("insert", false, true, false, Command::InsertBeat),
            ("up", true, false, true, Command::SemitoneUp),
            ("delete", true, true, false, Command::ClearBeats),
            ("delete", false, true, false, Command::DeleteBars),
            ("insert", false, false, false, Command::InsertBar),
            ("/", false, false, false, Command::Tuplet(Tuplet::TRIPLET)),
            ("?", true, false, false, Command::Tuplet(Tuplet::TRIPLET)),
            ("[", false, false, false, Command::RepeatStart),
            ("]", false, false, false, Command::RepeatEnd),
            ("|", true, false, false, Command::FreeTime),
            ("L", true, false, true, Command::FreeTime),
            ("3", false, false, true, Command::MoveToVoice(2)),
            ("down", false, false, true, Command::StringDown),
        ] {
            assert_eq!(
                typed(&mut e, name, shift, ctrl, alt, 0),
                Some(command),
                "{name}"
            );
        }
        assert_eq!(typed(&mut e, "z", false, false, false, 0), None);
        assert_eq!(
            typed(&mut e, "1", false, true, false, 0),
            None,
            "Ctrl + 1 is a voice"
        );
        let at = e.at_caret().unwrap();
        assert_eq!(
            (at.voice, at.value, at.rest),
            (0, Some(NoteValue::Quarter), false)
        );
    }

    #[test]
    fn enter_types_the_note_at_the_caret_s_line_or_space() {
        let mut e = editing(1, &[0]);
        let now = Instant::now();
        assert_eq!(
            e.typed("\n", false, false, false, now),
            None,
            "on the tablature"
        );
        let spot = Spot {
            track: 0,
            staff: 0,
            bar: 1,
            beat: Some(2),
            string: None,
            step: Some((8, Clef::Treble8vb)),
        };
        assert!(e.press(spot, false));
        assert_eq!(e.height(), Some((8, Clef::Treble8vb)));
        // The guitar's top line, F5 written, sounds F4.
        assert_eq!(
            e.typed("\n", false, false, false, now).map(|t| t.command),
            Some(Command::Pitch(Pitch(65)))
        );
        // Up: a space higher, G.
        e.key(Key::Up, false, false);
        assert_eq!(e.height(), Some((9, Clef::Treble8vb)));
        assert_eq!(e.caret().unwrap().string, 1, "the string stays");
        assert_eq!(
            e.typed("\n", false, false, false, now).map(|t| t.command),
            Some(Command::Pitch(Pitch(67)))
        );
        // Tab leaves the standard staff.
        e.key(Key::Tab, false, false);
        assert_eq!(e.height(), None);
    }

    #[test]
    fn copy_cut_and_paste() {
        let mut e = editing(1, &[0]);
        assert!(e.paste(1, false).is_none(), "nothing copied yet");
        assert!(e.copy());
        let t = e.paste(2, true).unwrap();
        assert!(matches!(
            &t.command,
            Command::Paste(p) if p.times == 2 && p.replace
        ));
        // With a selection, the paste goes to its start and the selection
        // goes.
        e.key(Key::Right, false, false);
        e.key(Key::Right, true, false);
        e.key(Key::Right, true, false);
        let t = e.paste(1, false).unwrap();
        assert_eq!((t.caret.bar, t.caret.beat), (0, 1));
        assert!(e.selection().is_none());
        e.key(Key::Right, true, false);
        let t = e.cut().unwrap();
        assert_eq!(t.command, Command::Cut);
        assert!(t.selection.is_some(), "the cut takes the selection");
        assert!(e.selection().is_none());
        assert_eq!(clip_key("v", true, true, false), Some(ClipKey::Special));
        assert_eq!(clip_key("c", false, false, false), None);
    }

    #[test]
    fn sections_to_go_to() {
        let mut e = editing(1, &[0]);
        let mut s = (*e.score.clone().unwrap()).clone();
        s.master_bars[2].marker = Some(oxiguitar_model::Marker {
            letter: "B".into(),
            text: "Chorus".into(),
            ..Default::default()
        });
        e.changed(Arc::new(s));
        assert_eq!(e.sections(), [(2, "B Chorus — bar 3".to_owned())]);
        assert!(e.go_to_bar(2));
        assert_eq!(e.caret().unwrap().bar, 2);
    }

    #[test]
    fn effect_keys() {
        use oxiguitar_edit::{Effect as E, HarmonicKind as H};
        use oxiguitar_model::{BeatTechnique, GraceKind, Slide, StrokeDirection};
        for (name, shift, ctrl, alt, e) in [
            ("x", false, false, false, E::Dead),
            ("o", false, false, false, E::Ghost),
            (";", false, false, false, E::Accent),
            (":", true, false, false, E::HeavyAccent),
            ("!", true, false, false, E::Staccato),
            ("h", false, false, false, E::HammerPull),
            ("(", true, false, false, E::LeftHandTap),
            ("s", false, false, false, E::Slide(Slide::Legato)),
            ("s", false, false, true, E::Slide(Slide::Shift)),
            ("S", true, false, false, E::Slide(Slide::OutDown)),
            ("v", false, false, false, E::Vibrato),
            ("p", false, false, false, E::PalmMute),
            ("P", true, false, false, E::PalmMuteBeat),
            ("i", false, false, false, E::LetRing),
            ("y", false, false, false, E::Harmonic(H::Natural)),
            ("y", false, true, true, E::Harmonic(H::Artificial)),
            ("p", false, false, true, E::Harmonic(H::Pinch)),
            ("t", false, false, true, E::Harmonic(H::Tapped)),
            ("n", false, false, false, E::Trill),
            ("\"", true, false, false, E::TremoloPicking),
            ("w", false, true, true, E::WideVibrato),
            ("<", true, false, false, E::FadeIn),
            ("t", false, false, false, E::Technique(BeatTechnique::Tap)),
            (")", true, false, false, E::Technique(BeatTechnique::Tap)),
            ("$", true, false, false, E::Technique(BeatTechnique::Slap)),
            ("d", false, true, false, E::Brush(StrokeDirection::Down)),
            ("u", false, true, false, E::Brush(StrokeDirection::Up)),
            (
                "D",
                true,
                false,
                false,
                E::PickStroke(StrokeDirection::Down),
            ),
            ("U", true, false, false, E::PickStroke(StrokeDirection::Up)),
            ("g", false, false, false, E::Grace(GraceKind::BeforeBeat)),
            ("g", false, true, true, E::Grace(GraceKind::OnBeat)),
        ] {
            assert_eq!(
                effect_key(name, shift, ctrl, alt),
                Some(Command::Effect(e)),
                "{name}"
            );
        }
        // Keys taken by something else.
        assert_eq!(effect_key("t", false, true, false), None);
        assert_eq!(effect_key("d", false, false, false), None);
        assert_eq!(effect_key("s", false, true, false), None);
        // The palette shows the caret's note's effects.
        let mut e = editing(1, &[0]);
        let t = e
            .typed("x", false, false, false, Instant::now())
            .map(|t| t.command);
        assert_eq!(t, Some(Command::Effect(E::Dead)));
        let mut s = (*e.score().unwrap()).clone();
        let c = e.caret().unwrap();
        let mut h = oxiguitar_edit::History::new();
        oxiguitar_edit::apply(
            &mut s,
            &mut h,
            c,
            None,
            Command::Fret {
                fret: 3,
                again: false,
            },
        )
        .unwrap();
        oxiguitar_edit::apply(&mut s, &mut h, c, None, Command::Effect(E::Dead)).unwrap();
        e.changed(Arc::new(s));
        let at = e.at_caret().unwrap();
        assert!(at.has(E::Dead) && !at.has(E::Ghost));
        assert!(EFFECTS.len() <= 64);
    }

    #[test]
    fn lyrics_typed_on_the_page() {
        let mut e = editing(1, &[0]);
        let mut score = (*e.score().unwrap()).clone();
        let mut h = oxiguitar_edit::History::new();
        // Type, and the edit is made as the worker would.
        let mut type_ = |e: &mut Editing, key, text: &str| {
            let (edit, next) = e.lyric_key(key, text);
            if let Some(t) = edit {
                oxiguitar_edit::apply(&mut score, &mut h, t.caret, t.selection, t.command).unwrap();
                e.changed(Arc::new(score.clone()));
            }
            next
        };
        e.key(Key::Right, false, false);
        let (l, shown) = e.start_lyrics().unwrap();
        assert_eq!((l.line, l.at, shown.as_str()), (0, (0, 1), ""));
        let n = type_(&mut e, LyricKey::Hyphen, "Hel").unwrap();
        assert_eq!(n.0.at, (0, 2));
        assert_eq!(e.caret().unwrap().beat, 2, "the caret follows");
        type_(&mut e, LyricKey::Space, "lo").unwrap();
        // Across the bar line.
        type_(&mut e, LyricKey::Space, "big").unwrap();
        let n = type_(&mut e, LyricKey::Done, "world");
        assert!(n.is_none());
        // The line starts at the bar; its first beat is left empty.
        assert_eq!(
            e.score().unwrap().tracks[0].lyrics[0].text,
            "-Hel-lo big world"
        );
        assert_eq!(e.score().unwrap().tracks[0].lyrics[0].start_bar, 0);
        assert_eq!((e.caret().unwrap().bar, e.caret().unwrap().beat), (1, 0));
        // Back on a syllable: it shows; Backspace in an empty field goes back.
        e.key(Key::Left, false, false);
        e.key(Key::Left, false, false);
        let (_, shown) = e.start_lyrics().unwrap();
        assert_eq!(shown, "lo");
        let n = type_(&mut e, LyricKey::Back, "").unwrap();
        assert_eq!((n.0.at, n.1.as_str()), ((0, 1), "Hel"));
        // Down: the next line, at the same beat; not past a line still empty.
        let n = type_(&mut e, LyricKey::Down, "Hel").unwrap();
        assert_eq!(n.0.line, 1);
        let n = type_(&mut e, LyricKey::Down, "").unwrap();
        assert_eq!(n.0.line, 1);
    }

    #[test]
    fn dialog_keys() {
        assert_eq!(dialog_key("t", false, true, false), Some(Dialog::Lyrics));
        assert_eq!(dialog_key("T", true, false, false), Some(Dialog::Text));
        assert_eq!(dialog_key("t", false, false, false), None, "T is tapping");
        assert_eq!(dialog_key("a", false, false, false), Some(Dialog::Chord));
        assert_eq!(
            dialog_key("a", false, true, false),
            None,
            "Ctrl + A selects all"
        );
        assert_eq!(dialog_key("b", false, false, false), Some(Dialog::Bend));
        assert_eq!(dialog_key("v", false, true, true), Some(Dialog::Whammy));
        assert_eq!(dialog_key("k", false, true, false), Some(Dialog::Bar));
        assert_eq!(dialog_key("d", false, false, false), Some(Dialog::Bar));
        assert_eq!(dialog_key("insert", true, false, false), Some(Dialog::Bar));
        assert_eq!(dialog_key("insert", false, false, false), None);
        assert_eq!(dialog_key("/", false, false, false), None);
    }

    #[test]
    fn key_names() {
        assert_eq!(Key::from_name("right"), Some(Key::Right));
        assert_eq!(Key::from_name("3"), Some(Key::Voice(2)));
        assert_eq!(Key::from_name("x"), None);
    }
}
