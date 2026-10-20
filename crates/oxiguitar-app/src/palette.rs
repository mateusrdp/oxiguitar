// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The editor's palette, shown greyed out from v0.2 so people trying
//! OxiGuitar see where it is going (Mateus, 19 Oct 2026; docs/design/ui.md,
//! "Palette"). The tools work from M3.
//!
//! The groups and their tools follow Guitar Pro 8's editing palette, in its
//! order, as its user guide lists them (guitar-pro.com/docs/gp8, "Editing
//! Palette Symbols", checked 19 Oct 2026): the voice and mode buttons at its
//! top, then the bar, note, effect and notation symbols, less the automation
//! group, Timer and the stem-direction tools (Mateus, 19 Oct 2026: stems go by
//! voice only, odd voices up and even voices down). The
//! names are the guide's; every icon is our own drawing (UI-3), made with
//! our renderer from Bravura's symbols, the interface font, lines and
//! curves (`icons.rs`). Shortcuts come with the editor, checked against the
//! guide's shortcut list when they are bound.
//!
//! No window code here: the window shows [`GROUPS`] and asks [`Palette`]
//! which groups are collapsed.

use crate::editing::AtCaret;
use crate::icons::{Face, Fit, Pen, S, VALUES, glyph};
use oxiguitar_display::smufl::glyph::Glyph;
use oxiguitar_edit::{Command, Effect};
use oxiguitar_model::{Dynamic, NoteValue};

/// One tool.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tool {
    /// Stable id, for the editor to bind.
    pub(crate) id: &'static str,
    /// Guitar Pro 8's name for it.
    pub(crate) name: &'static str,
    pub(crate) face: Face,
    pub(crate) fit: Fit,
}

/// A group of tools under one heading.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Group {
    pub(crate) title: &'static str,
    pub(crate) tools: &'static [Tool],
}

/// The class of the dynamics, which share one scale like the note values.
const DYNAMICS: u8 = 1;

const fn tool(id: &'static str, name: &'static str, face: Face) -> Tool {
    let fit = match face {
        Face::Text(_) => Fit::Text,
        _ => Fit::Own,
    };
    Tool {
        id,
        name,
        face,
        fit,
    }
}

const fn value(id: &'static str, name: &'static str, face: Face) -> Tool {
    Tool {
        id,
        name,
        face,
        fit: Fit::Shared(VALUES),
    }
}

const fn dynamic(id: &'static str, name: &'static str, g: Glyph) -> Tool {
    Tool {
        id,
        name,
        face: Face::Symbol(g),
        fit: Fit::Shared(DYNAMICS),
    }
}

/// The tools that work already, with their shortcut (Guitar Pro 8's) if
/// they have one: the voice buttons, which pick the caret's voice (M3
/// slice 1), and note entry's (slice 3).
pub(crate) fn working(id: &str) -> Option<Option<&'static str>> {
    Some(Some(match id {
        "voice-1" => "Ctrl + 1",
        "voice-2" => "Ctrl + 2",
        "voice-3" => "Ctrl + 3",
        "voice-4" => "Ctrl + 4",
        "rest" => "R",
        "dotted" => "Shift + .",
        "double-dotted" => "Ctrl + .",
        "tie" => "L",
        "triplet" => "/",
        "repeat-start" => "[",
        "repeat-close" => "]",
        "free-time" => "|",
        "key-signature" => "Ctrl + K",
        "rhythmic-interpretation" => "Ctrl + /",
        "directions" => "D",
        "bend" => "B",
        "chord" => "A",
        "text" => "Shift + T",
        "lyrics" => "Ctrl + T",
        "whammy" => "Ctrl + Alt + V",
        "dead" => "X",
        "ghost" => "O",
        "accent" => ";",
        "heavy-accent" => ":",
        "staccato" => "!",
        "hammer" => "H",
        "left-hand-tapping" => "(",
        "legato-slide" => "S",
        "shift-slide" => "Alt + S",
        "slide-out-down" => "Shift + S",
        "vibrato" => "V",
        "palm-mute" => "P",
        "let-ring" => "I",
        "natural-harmonic" => "Y",
        "artificial-harmonic" => "Ctrl + Alt + Y",
        "trill" => "N",
        "tremolo-picking" => "\"",
        "wide-bridge-vibrato" => "Ctrl + Alt + W",
        "fade-in" => "<",
        "tapping" => "T",
        "slap" => "$",
        "brush-down" => "Ctrl + D",
        "brush-up" => "Ctrl + U",
        "pickstroke-down" => "Shift + D",
        "pickstroke-up" => "Shift + U",
        "grace-before" => "G",
        "grace-on" => "Ctrl + Alt + G",
        "semitone-up" => "Alt + Shift + Up",
        "semitone-down" => "Alt + Shift + Down",
        "multiple-ties" => "Shift + L",
        "section" => "Shift + Insert",
        _ if command_of(id).is_some() || dialog_of(id).is_some() => return Some(None),
        _ => return None,
    }))
}

/// The edit a note-entry tool makes at the caret.
pub(crate) fn command_of(id: &str) -> Option<Command> {
    Some(match id {
        "whole" => Command::Value(NoteValue::Whole),
        "half" => Command::Value(NoteValue::Half),
        "quarter" => Command::Value(NoteValue::Quarter),
        "eighth" => Command::Value(NoteValue::Eighth),
        "16th" => Command::Value(NoteValue::Sixteenth),
        "32nd" => Command::Value(NoteValue::ThirtySecond),
        "64th" => Command::Value(NoteValue::SixtyFourth),
        "rest" => Command::Rest,
        "dotted" => Command::Dot,
        "double-dotted" => Command::DoubleDot,
        "tie" => Command::Tie,
        "triplet" => Command::Tuplet(oxiguitar_model::Tuplet::TRIPLET),
        "repeat-start" => Command::RepeatStart,
        "repeat-close" => Command::RepeatEnd,
        "free-time" => Command::FreeTime,
        "double-bar" => Command::DoubleBar,
        "semitone-up" => Command::SemitoneUp,
        "semitone-down" => Command::SemitoneDown,
        "multiple-ties" => Command::TieBeat,
        "ppp" => Command::Dynamic(Dynamic::PPP),
        "pp" => Command::Dynamic(Dynamic::PP),
        "p" => Command::Dynamic(Dynamic::P),
        "mp" => Command::Dynamic(Dynamic::MP),
        "mf" => Command::Dynamic(Dynamic::MF),
        "f" => Command::Dynamic(Dynamic::F),
        "ff" => Command::Dynamic(Dynamic::FF),
        "fff" => Command::Dynamic(Dynamic::FFF),
        _ => return effect_of(id).map(Command::Effect),
    })
}

/// The effect an effect tool switches (M3 slice 6).
pub(crate) fn effect_of(id: &str) -> Option<Effect> {
    use oxiguitar_edit::HarmonicKind as H;
    use oxiguitar_model::{BeatTechnique, GraceKind, Slide, StrokeDirection};
    Some(match id {
        "ghost" => Effect::Ghost,
        "accent" => Effect::Accent,
        "heavy-accent" => Effect::HeavyAccent,
        "staccato" => Effect::Staccato,
        "let-ring" => Effect::LetRing,
        "palm-mute" => Effect::PalmMute,
        "dead" => Effect::Dead,
        "natural-harmonic" => Effect::Harmonic(H::Natural),
        "artificial-harmonic" => Effect::Harmonic(H::Artificial),
        "fade-in" => Effect::FadeIn,
        "grace-before" => Effect::Grace(GraceKind::BeforeBeat),
        "grace-on" => Effect::Grace(GraceKind::OnBeat),
        "hammer" => Effect::HammerPull,
        "tremolo-picking" => Effect::TremoloPicking,
        "left-hand-tapping" => Effect::LeftHandTap,
        "legato-slide" => Effect::Slide(Slide::Legato),
        "shift-slide" => Effect::Slide(Slide::Shift),
        "slide-in-below" => Effect::Slide(Slide::InFromBelow),
        "slide-in-above" => Effect::Slide(Slide::InFromAbove),
        "slide-out-down" => Effect::Slide(Slide::OutDown),
        "slide-out-up" => Effect::Slide(Slide::OutUp),
        "trill" => Effect::Trill,
        "vibrato" => Effect::Vibrato,
        "wide-bridge-vibrato" => Effect::WideVibrato,
        "brush-down" => Effect::Brush(StrokeDirection::Down),
        "brush-up" => Effect::Brush(StrokeDirection::Up),
        "pickstroke-down" => Effect::PickStroke(StrokeDirection::Down),
        "pickstroke-up" => Effect::PickStroke(StrokeDirection::Up),
        "tapping" => Effect::Technique(BeatTechnique::Tap),
        "slap" => Effect::Technique(BeatTechnique::Slap),
        "pop" => Effect::Technique(BeatTechnique::Pop),
        _ => return None,
    })
}

/// The dialog a tool opens at the caret: the bar's tools open the Bar
/// dialog, Custom N-tuplet its own.
pub(crate) fn dialog_of(id: &str) -> Option<crate::editing::Dialog> {
    use crate::editing::Dialog;
    Some(match id {
        "clef"
        | "key-signature"
        | "time-signature"
        | "rhythmic-interpretation"
        | "alternate-endings"
        | "directions"
        | "section" => Dialog::Bar,
        "tuplet" => Dialog::Tuplet,
        "bend" => Dialog::Bend,
        "chord" | "chords" => Dialog::Chord,
        "text" => Dialog::Text,
        "lyrics" => Dialog::Lyrics,
        "whammy" => Dialog::Whammy,
        _ => return None,
    })
}

/// Whether a working tool shows as on for the caret's beat `at`: the
/// caret's voice, the beat's note value, dots, rest, the caret's note
/// tied.
pub(crate) fn checked(id: &str, at: &AtCaret) -> bool {
    if let Some(v) = voice_of(id) {
        return at.voice == v;
    }
    match command_of(id) {
        Some(Command::Value(v)) => at.value == Some(v),
        Some(Command::Rest) => at.rest,
        Some(Command::Dot) => at.dots == 1,
        Some(Command::DoubleDot) => at.dots == 2,
        Some(Command::Tie) => at.tied,
        Some(Command::Tuplet(_)) => at.triplet,
        Some(Command::RepeatStart) => at.repeat_start,
        Some(Command::RepeatEnd) => at.repeat_end,
        Some(Command::DoubleBar) => at.double_bar,
        Some(Command::FreeTime) => at.free_time,
        Some(Command::Effect(e)) => at.has(e),
        Some(Command::Dynamic(d)) => at.dynamic == Some(d),
        _ => false,
    }
}

/// The voice a voice button picks, from 0.
pub(crate) fn voice_of(id: &str) -> Option<usize> {
    let n: usize = id.strip_prefix("voice-")?.parse().ok()?;
    (1..=4).contains(&n).then(|| n - 1)
}

/// A tool's tooltip: its name and shortcut once it works, else its name
/// and that it comes with the editor.
pub(crate) fn tool_tip(id: &str, name: &str) -> String {
    match working(id) {
        Some(Some(keys)) => format!("{name} ({keys})"),
        Some(None) => name.to_owned(),
        // Every tool that doesn't work yet is beyond TAB-1 and comes with
        // Guitar Pro 8's notation, in M5.
        None => format!("{name} (comes in M5)"),
    }
}

/// Whether a tool named `name` is one the palette's search for `query`
/// (lower case) finds: every word typed is in the name ("slide" finds the
/// six slides, "nat harm" the natural harmonic).
pub(crate) fn matches(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    query.split_whitespace().all(|word| name.contains(word))
}

/// Which groups are collapsed. Every group starts expanded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Palette {
    collapsed: Vec<bool>,
}

impl Default for Palette {
    fn default() -> Palette {
        Palette {
            collapsed: vec![false; GROUPS.len()],
        }
    }
}

impl Palette {
    /// A click on group `i`'s heading collapses or expands it.
    pub(crate) fn toggle(&mut self, i: usize) {
        if let Some(c) = self.collapsed.get_mut(i) {
            *c = !*c;
        }
    }

    pub(crate) fn collapsed(&self, i: usize) -> bool {
        self.collapsed.get(i).copied().unwrap_or(false)
    }
}

/// Every tool, in order, with its group's index.
pub(crate) fn tools() -> impl Iterator<Item = (usize, &'static Tool)> {
    GROUPS
        .iter()
        .enumerate()
        .flat_map(|(g, group)| group.tools.iter().map(move |t| (g, t)))
}

// ---------------------------------------------------------------------------
// Bravura's symbols. Each one's ink is checked against the bounding box the
// metadata gives for its name (tests below).

const NOTEHEAD: Glyph = glyph("noteheadBlack", '\u{E0A4}');
const NOTEHEAD_X: Glyph = glyph("noteheadXBlack", '\u{E0A9}');
const PAREN_LEFT: Glyph = glyph("noteheadParenthesisLeft", '\u{E0F5}');
const PAREN_RIGHT: Glyph = glyph("noteheadParenthesisRight", '\u{E0F6}');
const NOTE_WHOLE: Glyph = glyph("noteWhole", '\u{E1D2}');
const NOTE_HALF: Glyph = glyph("noteHalfUp", '\u{E1D3}');
const NOTE_QUARTER: Glyph = glyph("noteQuarterUp", '\u{E1D5}');
const NOTE_8TH: Glyph = glyph("note8thUp", '\u{E1D7}');
const NOTE_16TH: Glyph = glyph("note16thUp", '\u{E1D9}');
const NOTE_32ND: Glyph = glyph("note32ndUp", '\u{E1DB}');
const NOTE_64TH: Glyph = glyph("note64thUp", '\u{E1DD}');
const DOT: Glyph = glyph("augmentationDot", '\u{E1E7}');
const REST_QUARTER: Glyph = glyph("restQuarter", '\u{E4E5}');
const TUPLET_3: Glyph = glyph("tuplet3", '\u{E883}');
const G_CLEF: Glyph = glyph("gClef", '\u{E050}');
const SHARP: Glyph = glyph("accidentalSharp", '\u{E262}');
const FLAT: Glyph = glyph("accidentalFlat", '\u{E260}');
const NATURAL: Glyph = glyph("accidentalNatural", '\u{E261}');
const DOUBLE_SHARP: Glyph = glyph("accidentalDoubleSharp", '\u{E263}');
const DOUBLE_FLAT: Glyph = glyph("accidentalDoubleFlat", '\u{E264}');
const TIME_SIG_4: Glyph = glyph("timeSig4", '\u{E084}');
const REPEAT_LEFT: Glyph = glyph("repeatLeft", '\u{E040}');
const REPEAT_RIGHT: Glyph = glyph("repeatRight", '\u{E041}');
const REPEAT_1_BAR: Glyph = glyph("repeat1Bar", '\u{E500}');
const REPEAT_2_BARS: Glyph = glyph("repeat2Bars", '\u{E501}');
const SEGNO: Glyph = glyph("segno", '\u{E047}');
const OTTAVA_ALTA: Glyph = glyph("ottavaAlta", '\u{E511}');
const OTTAVA_BASSA_VB: Glyph = glyph("ottavaBassaVb", '\u{E51C}');
const QUINDICESIMA_ALTA: Glyph = glyph("quindicesimaAlta", '\u{E515}');
const QUINDICESIMA_BASSA_MB: Glyph = glyph("quindicesimaBassaMb", '\u{E51D}');
const FERMATA: Glyph = glyph("fermataAbove", '\u{E4C0}');
const PPP: Glyph = glyph("dynamicPPP", '\u{E52A}');
const PP: Glyph = glyph("dynamicPP", '\u{E52B}');
const P: Glyph = glyph("dynamicPiano", '\u{E520}');
const MP: Glyph = glyph("dynamicMP", '\u{E52C}');
const MF: Glyph = glyph("dynamicMF", '\u{E52D}');
const F: Glyph = glyph("dynamicForte", '\u{E522}');
const FF: Glyph = glyph("dynamicFF", '\u{E52F}');
const FFF: Glyph = glyph("dynamicFFF", '\u{E530}');
const ACCENT: Glyph = glyph("articAccentAbove", '\u{E4A0}');
const MARCATO: Glyph = glyph("articMarcatoAbove", '\u{E4AC}');
const STACCATO: Glyph = glyph("articStaccatoAbove", '\u{E4A2}');
const PEDAL: Glyph = glyph("keyboardPedalPed", '\u{E650}');
const GRACE_SLASH: Glyph = glyph("graceNoteSlashStemUp", '\u{E564}');
const SHORT_TRILL: Glyph = glyph("ornamentShortTrill", '\u{E56C}');
const MORDENT: Glyph = glyph("ornamentMordent", '\u{E56D}');
const TURN: Glyph = glyph("ornamentTurn", '\u{E567}');
const TURN_INVERTED: Glyph = glyph("ornamentTurnInverted", '\u{E568}');
const TRILL: Glyph = glyph("ornamentTrill", '\u{E566}');
const VIBRATO: Glyph = glyph("guitarVibratoStroke", '\u{EAB2}');
const VIBRATO_WIDE: Glyph = glyph("guitarWideVibratoStroke", '\u{EAB3}');
const MUTE_CLOSED: Glyph = glyph("brassMuteClosed", '\u{E5E5}');
const MUTE_OPEN: Glyph = glyph("brassMuteOpen", '\u{E5E7}');
const DOWN_BOW: Glyph = glyph("stringsDownBow", '\u{E610}');
const UP_BOW: Glyph = glyph("stringsUpBow", '\u{E612}');

// ---------------------------------------------------------------------------
// The groups.

/// Guitar Pro 8's palette, top to bottom.
pub(crate) const GROUPS: &[Group] = &[
    Group {
        title: "Voices and modes",
        tools: &[
            tool("voice-1", "Edit Voice 1", Face::Drawn(voice_1)),
            tool("voice-2", "Edit Voice 2", Face::Drawn(voice_2)),
            tool("voice-3", "Edit Voice 3", Face::Drawn(voice_3)),
            tool("voice-4", "Edit Voice 4", Face::Drawn(voice_4)),
            tool("multivoice", "Multivoice", Face::Drawn(multivoice)),
            tool("design-mode", "Design Mode", Face::Drawn(design_mode)),
            tool("lyrics", "Lyrics", Face::Drawn(lyrics)),
            tool("chords", "Chords", Face::Drawn(chord_library)),
        ],
    },
    Group {
        title: "Bar",
        tools: &[
            tool("clef", "Clef", Face::Symbol(G_CLEF)),
            tool("key-signature", "Key Signature", Face::Drawn(key_signature)),
            tool(
                "time-signature",
                "Time Signature",
                Face::Drawn(time_signature),
            ),
            tool(
                "rhythmic-interpretation",
                "Rhythmic Interpretation (Triplet Feel)",
                Face::Drawn(triplet_feel),
            ),
            tool("free-time", "Free Time", Face::Drawn(free_time)),
            tool("double-bar", "Double Bar Line", Face::Drawn(double_bar)),
            tool(
                "repeat-1-bar",
                "Single Repeat Sign",
                Face::Drawn(repeat_one_bar),
            ),
            tool(
                "repeat-2-bars",
                "Double Repeat Sign",
                Face::Drawn(repeat_two_bars),
            ),
            tool("repeat-start", "Repeat Start", Face::Drawn(repeat_start)),
            tool("repeat-close", "Repeat Close", Face::Drawn(repeat_close)),
            tool(
                "alternate-endings",
                "Alternate Endings",
                Face::Drawn(alternate_endings),
            ),
            tool("directions", "Directions", Face::Symbol(SEGNO)),
            tool("8va", "8va Octaviation", Face::Symbol(OTTAVA_ALTA)),
            tool("8vb", "8vb Octaviation", Face::Symbol(OTTAVA_BASSA_VB)),
            tool("15ma", "15ma Octaviation", Face::Symbol(QUINDICESIMA_ALTA)),
            tool(
                "15mb",
                "15mb Octaviation",
                Face::Symbol(QUINDICESIMA_BASSA_MB),
            ),
        ],
    },
    Group {
        title: "Note",
        tools: &[
            value("whole", "Whole Note", Face::Symbol(NOTE_WHOLE)),
            value("half", "Half Note", Face::Symbol(NOTE_HALF)),
            value("quarter", "Quarter Note", Face::Symbol(NOTE_QUARTER)),
            value("eighth", "Eighth Note", Face::Symbol(NOTE_8TH)),
            value("16th", "Sixteenth Note", Face::Symbol(NOTE_16TH)),
            value("32nd", "Thirty-Second Note", Face::Symbol(NOTE_32ND)),
            value("64th", "Sixty-Fourth Note", Face::Symbol(NOTE_64TH)),
            value("rest", "Rest", Face::Symbol(REST_QUARTER)),
            value("dotted", "Dotted Note", Face::Drawn(dotted)),
            value(
                "double-dotted",
                "Double-Dotted Note",
                Face::Drawn(double_dotted),
            ),
            tool("triplet", "Triplet", Face::Drawn(triplet)),
            tool("tuplet", "Custom N-tuplet", Face::Drawn(custom_tuplet)),
            tool(
                "nested-tuplet",
                "Nested N-tuplet",
                Face::Drawn(nested_tuplet),
            ),
            tool("tie", "Tie", Face::Drawn(tie)),
            tool("multiple-ties", "Multiple Ties", Face::Drawn(multiple_ties)),
            tool("fermata", "Fermata", Face::Symbol(FERMATA)),
            tool("double-flat", "Double Flat", Face::Symbol(DOUBLE_FLAT)),
            tool("flat", "Flat", Face::Symbol(FLAT)),
            tool("natural", "Natural", Face::Symbol(NATURAL)),
            tool("sharp", "Sharp", Face::Symbol(SHARP)),
            tool("double-sharp", "Double Sharp", Face::Symbol(DOUBLE_SHARP)),
            tool(
                "change-accidental",
                "Change Accidental",
                Face::Drawn(change_accidental),
            ),
            tool(
                "semitone-down",
                "Lower by a Semitone",
                Face::Drawn(semitone_down),
            ),
            tool(
                "semitone-up",
                "Raise by a Semitone",
                Face::Drawn(semitone_up),
            ),
            dynamic("ppp", "Pianississimo", PPP),
            dynamic("pp", "Pianissimo", PP),
            dynamic("p", "Piano", P),
            dynamic("mp", "Mezzo Piano", MP),
            dynamic("mf", "Mezzo Forte", MF),
            dynamic("f", "Forte", F),
            dynamic("ff", "Fortissimo", FF),
            dynamic("fff", "Fortississimo", FFF),
            tool("crescendo", "Crescendo", Face::Drawn(crescendo)),
            tool("decrescendo", "Decrescendo", Face::Drawn(decrescendo)),
        ],
    },
    Group {
        title: "Effects",
        tools: &[
            tool("ghost", "Ghost Note", Face::Drawn(ghost)),
            tool("accent", "Accented Note", Face::Drawn(accented)),
            tool(
                "heavy-accent",
                "Heavily Accented Note",
                Face::Drawn(heavily_accented),
            ),
            tool("staccato", "Staccato", Face::Drawn(staccato)),
            tool("legato", "Legato", Face::Drawn(legato)),
            tool("let-ring", "Let Ring", Face::Text("L.R.")),
            tool("sustain-pedal", "Sustain Pedal", Face::Symbol(PEDAL)),
            tool("palm-mute", "Palm Mute", Face::Text("P.M.")),
            tool("dead", "Dead Note", Face::Symbol(NOTEHEAD_X)),
            tool("natural-harmonic", "Natural Harmonic", Face::Text("N.H.")),
            tool(
                "artificial-harmonic",
                "Artificial Harmonic",
                Face::Text("A.H."),
            ),
            tool("bend", "Bend", Face::Drawn(bend)),
            tool("fade-in", "Fade-in", Face::Drawn(fade_in)),
            tool("fade-out", "Fade-out", Face::Drawn(fade_out)),
            tool("volume-swell", "Volume Swell", Face::Drawn(volume_swell)),
            tool(
                "grace-before",
                "Grace Note Before the Beat",
                Face::Drawn(grace_before),
            ),
            tool("grace-on", "Grace Note On the Beat", Face::Drawn(grace_on)),
            tool("hammer", "Hammer-on/Pull-off", Face::Text("H")),
            tool(
                "tremolo-picking",
                "Tremolo Picking",
                Face::Drawn(tremolo_picking),
            ),
            tool("upper-mordent", "Upper Mordent", Face::Symbol(SHORT_TRILL)),
            tool("lower-mordent", "Lower Mordent", Face::Symbol(MORDENT)),
            tool("turn", "Turn", Face::Symbol(TURN)),
            tool(
                "inverted-turn",
                "Inverted Turn",
                Face::Symbol(TURN_INVERTED),
            ),
            tool(
                "left-hand-tapping",
                "Left-Hand Tapping",
                Face::Drawn(left_hand_tapping),
            ),
            tool("legato-slide", "Legato Slide", Face::Drawn(legato_slide)),
            tool("shift-slide", "Shift Slide", Face::Drawn(shift_slide)),
            tool(
                "slide-in-below",
                "Slide In from Below",
                Face::Drawn(slide_in_below),
            ),
            tool(
                "slide-in-above",
                "Slide In from Above",
                Face::Drawn(slide_in_above),
            ),
            tool(
                "slide-out-down",
                "Slide Out Downwards",
                Face::Drawn(slide_out_down),
            ),
            tool(
                "slide-out-up",
                "Slide Out Upwards",
                Face::Drawn(slide_out_up),
            ),
            tool("whammy", "Whammy Bar", Face::Drawn(whammy)),
            tool("trill", "Trill", Face::Symbol(TRILL)),
            tool("vibrato", "Slight Left-Hand Vibrato", Face::Drawn(vibrato)),
            tool(
                "wide-vibrato",
                "Wide Left-Hand Vibrato",
                Face::Drawn(wide_vibrato),
            ),
            tool(
                "bridge-vibrato",
                "Slight Bridge Vibrato",
                Face::Drawn(bridge_vibrato),
            ),
            tool(
                "wide-bridge-vibrato",
                "Wide Bridge Vibrato",
                Face::Drawn(wide_bridge_vibrato),
            ),
            tool("wah-closed", "Closed Wah-Wah", Face::Symbol(MUTE_CLOSED)),
            tool("wah-open", "Open Wah-Wah", Face::Symbol(MUTE_OPEN)),
            tool(
                "arpeggio-down",
                "Downward Arpeggio",
                Face::Drawn(arpeggio_down),
            ),
            tool("arpeggio-up", "Upward Arpeggio", Face::Drawn(arpeggio_up)),
            tool("brush-down", "Downward Brush", Face::Drawn(brush_down)),
            tool("brush-up", "Upward Brush", Face::Drawn(brush_up)),
            tool("rasgueado", "Rasgueado", Face::Text("rasg.")),
            tool(
                "left-hand-fingering",
                "Left-Hand Fingering",
                Face::Drawn(left_hand_fingering),
            ),
            tool(
                "right-hand-fingering",
                "Right-Hand Fingering",
                Face::Drawn(right_hand_fingering),
            ),
            tool("string-number", "String Number", Face::Drawn(string_number)),
            tool(
                "pickstroke-down",
                "Downward Pickstroke",
                Face::Symbol(DOWN_BOW),
            ),
            tool("pickstroke-up", "Upward Pickstroke", Face::Symbol(UP_BOW)),
            tool("tapping", "Tapping", Face::Text("T")),
            tool("slap", "Slap", Face::Text("S")),
            tool("pop", "Pop", Face::Text("P")),
            tool("finger-golpe", "Finger Golpe", Face::Drawn(finger_golpe)),
            tool("thumb-golpe", "Thumb Golpe", Face::Drawn(thumb_golpe)),
            tool(
                "pick-scrape-up",
                "Upward Pick Scrape",
                Face::Drawn(pick_scrape_up),
            ),
            tool(
                "pick-scrape-down",
                "Downward Pick Scrape",
                Face::Drawn(pick_scrape_down),
            ),
            tool("dead-slap", "Dead Slap", Face::Drawn(dead_slap)),
        ],
    },
    Group {
        title: "Notation",
        tools: &[
            tool("chord", "Chord", Face::Drawn(chord)),
            tool("slash", "Slash", Face::Drawn(slash)),
            tool("barre", "Extended Barre", Face::Drawn(barre)),
            tool("text", "Text", Face::Text("Aa")),
            tool("section", "Section", Face::Drawn(section)),
            tool("beams-auto", "Automatic Beaming", Face::Drawn(beams_auto)),
            tool("beams-force", "Force Beaming", Face::Drawn(beams_force)),
            tool("beams-break", "Break Beaming", Face::Drawn(beams_break)),
            tool(
                "beams-break-secondary",
                "Break Secondary Beaming",
                Face::Drawn(beams_break_secondary),
            ),
            tool("tuplet-groups", "Force Groups", Face::Drawn(force_groups)),
        ],
    },
];

// ---------------------------------------------------------------------------
// Drawing. Units: a staff space is S, y points down; noteheads sit on y = 0
// unless said otherwise.

/// A black notehead's width.
const HEAD_W: f32 = 1.18 * S;
/// Stems, a little heavier than engraved so they read at button size.
const STEM: f32 = 0.14 * S;
/// Staff lines.
const STAFF_LINE: f32 = 0.12 * S;
/// Lines drawn for shapes (arrows, brackets, frames).
const LINE: f32 = 0.2 * S;
/// Text inside drawn faces.
const LABEL: f32 = 1.8 * S;

fn head(p: &mut Pen<'_>, x: f32, y: f32) {
    p.sym(NOTEHEAD, x, y);
}

/// The x of an up-stem on a notehead at `x`.
fn stem_x(x: f32) -> f32 {
    x + HEAD_W - STEM / 2.0
}

/// A notehead with its stem up, `len` long.
fn note_up(p: &mut Pen<'_>, x: f32, y: f32, len: f32) {
    head(p, x, y);
    p.line(stem_x(x), y - 0.17 * S, stem_x(x), y - len, STEM);
}

/// A beam from `x0` to `x1`, its top at `y`.
fn beam(p: &mut Pen<'_>, x0: f32, x1: f32, y: f32) {
    p.rect(x0 - STEM / 2.0, y, x1 - x0 + STEM, 0.5 * S);
}

/// `n` stem-up notes `gap` apart from `x`, under one beam (and a second
/// one when `beams` is 2).
fn beamed(p: &mut Pen<'_>, n: usize, x: f32, gap: f32, beams: u8) {
    let top = -3.0 * S;
    for i in 0..n {
        note_up(p, x + gap * i as f32, 0.0, -top);
    }
    let (x0, x1) = (stem_x(x), stem_x(x + gap * (n - 1) as f32));
    beam(p, x0, x1, top);
    if beams > 1 {
        beam(p, x0, x1, top + 0.75 * S);
    }
}

/// Five staff lines from `x0` to `x1`, the bottom one on y = 0.
fn staff(p: &mut Pen<'_>, x0: f32, x1: f32) {
    for i in 0..5 {
        let y = -S * i as f32;
        p.line(x0, y, x1, y, STAFF_LINE);
    }
}

/// A tuplet bracket from `x0` to `x1` at `y`, hooks down, open in the
/// middle for its number.
fn bracket(p: &mut Pen<'_>, x0: f32, x1: f32, y: f32, gap: f32) {
    let mid = (x0 + x1) / 2.0;
    let hook = 0.7 * S;
    p.polyline(&[(x0, y + hook), (x0, y), (mid - gap, y)], LINE);
    p.polyline(&[(mid + gap, y), (x1, y), (x1, y + hook)], LINE);
}

/// The tuplet number 3, centred on `x` at `y`, at `k` times its size.
fn three(p: &mut Pen<'_>, x: f32, y: f32, k: f32) {
    let w = p.advance(TUPLET_3) * k;
    p.sym_scaled(TUPLET_3, x - w / 2.0, y, k);
}

/// An arrow from (`x0`, `y0`) to its tip at (`x1`, `y1`).
fn arrow(p: &mut Pen<'_>, x0: f32, y0: f32, x1: f32, y1: f32) {
    let head = 0.9 * S;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ex, ey) = (x1 - dx / len * head * 0.8, y1 - dy / len * head * 0.8);
    p.line(x0, y0, ex, ey, LINE);
    p.arrowhead(x1, y1, dx, dy, head);
}

/// Text centred on (`x`, `y`) by its cap height.
fn label(p: &mut Pen<'_>, text: &str, x: f32, y: f32, size: f32) {
    p.text_centred(text, x, y + size * 0.357, size);
}

/// A symbol centred on `x`, its origin at `y`.
fn sym_centred(p: &mut Pen<'_>, g: Glyph, x: f32, y: f32) {
    let w = p.advance(g);
    p.sym(g, x - w / 2.0, y);
}

// Voices and modes.

fn voice(p: &mut Pen<'_>, n: &str) {
    let side = 2.6 * S;
    p.frame(0.0, 0.0, side, side, LINE);
    label(p, n, side / 2.0, side / 2.0, LABEL);
}

fn voice_1(p: &mut Pen<'_>) {
    voice(p, "1");
}
fn voice_2(p: &mut Pen<'_>) {
    voice(p, "2");
}
fn voice_3(p: &mut Pen<'_>) {
    voice(p, "3");
}
fn voice_4(p: &mut Pen<'_>) {
    voice(p, "4");
}

fn multivoice(p: &mut Pen<'_>) {
    let (side, step) = (1.15 * S, 1.45 * S);
    p.rect(0.0, 0.0, side, side);
    p.frame(step, 0.0, side, side, LINE * 0.8);
    p.frame(0.0, step, side, side, LINE * 0.8);
    p.rect(step, step, side, side);
}

fn design_mode(p: &mut Pen<'_>) {
    // A pencil, its tip at the bottom left.
    let r = std::f32::consts::FRAC_1_SQRT_2;
    let (ux, uy) = (r, -r);
    let half = 0.5 * S;
    let (nx, ny) = (r * half, r * half);
    let tip = (0.0, 4.2 * S);
    let at = |d: f32| (tip.0 + ux * d, tip.1 + uy * d);
    let (c1, c2) = (at(1.3 * S), at(4.8 * S));
    let body = oxiguitar_display::Path::new()
        .move_to(tip.0, tip.1)
        .line_to(c1.0 + nx, c1.1 + ny)
        .line_to(c2.0 + nx, c2.1 + ny)
        .line_to(c2.0 - nx, c2.1 - ny)
        .line_to(c1.0 - nx, c1.1 - ny)
        .close();
    p.stroke(body, LINE);
    p.line(c1.0 + nx, c1.1 + ny, c1.0 - nx, c1.1 - ny, LINE);
    let lead = at(0.45 * S);
    p.circle(lead.0, lead.1, 0.25 * S, 0.0);
}

fn lyrics(p: &mut Pen<'_>) {
    note_up(p, 0.0, 0.0, 2.8 * S);
    p.text_centred("la", HEAD_W / 2.0, 2.2 * S, 1.6 * S);
}

/// A chord diagram: six strings, four frets, three fingers.
fn diagram(p: &mut Pen<'_>, x: f32, y: f32) {
    let (w, fret) = (2.5 * S, 0.75 * S);
    let thin = 0.12 * S;
    for i in 0..6 {
        let sx = x + w * i as f32 / 5.0;
        p.line(sx, y, sx, y + 4.0 * fret, thin);
    }
    p.rect(x - thin / 2.0, y - 0.3 * S, w + thin, 0.3 * S);
    for i in 1..=4 {
        let fy = y + fret * i as f32;
        p.line(x, fy, x + w, fy, thin);
    }
    for (string, f) in [(1.0, 1.5), (2.0, 2.5), (4.0, 0.5)] {
        p.circle(x + w * string / 5.0, y + fret * f, 0.24 * S, 0.0);
    }
}

fn chord(p: &mut Pen<'_>) {
    diagram(p, 0.0, 0.0);
}

fn chord_library(p: &mut Pen<'_>) {
    // A second diagram behind the first: its top and right edges.
    let (w, h, d) = (2.5 * S, 3.0 * S, 0.7 * S);
    p.polyline(
        &[(d, -0.3 * S - d), (w + d, -0.3 * S - d), (w + d, h - d)],
        LINE * 0.7,
    );
    diagram(p, 0.0, 0.0);
}

// Bar.

fn key_signature(p: &mut Pen<'_>) {
    staff(p, -0.4 * S, 2.8 * S);
    p.sym(SHARP, 0.2 * S, -4.0 * S);
    p.sym(SHARP, 1.3 * S, -2.5 * S);
}

fn four_four(p: &mut Pen<'_>, x: f32) {
    p.sym(TIME_SIG_4, x, -3.0 * S);
    p.sym(TIME_SIG_4, x, -S);
}

fn time_signature(p: &mut Pen<'_>) {
    staff(p, -0.4 * S, 2.4 * S);
    four_four(p, 0.2 * S);
}

fn free_time(p: &mut Pen<'_>) {
    time_signature(p);
    p.line(-0.2 * S, 0.4 * S, 2.2 * S, -4.4 * S, 1.5 * LINE);
}

fn triplet_feel(p: &mut Pen<'_>) {
    beamed(p, 2, 0.0, 1.8 * S, 1);
    let mid = (stem_x(0.0) + stem_x(1.8 * S)) / 2.0;
    three(p, mid, -3.9 * S, 0.8);
    p.text_centred("(", mid - 0.95 * S, -3.6 * S, 1.6 * S);
    p.text_centred(")", mid + 0.95 * S, -3.6 * S, 1.6 * S);
}

fn double_bar(p: &mut Pen<'_>) {
    staff(p, -S, 1.6 * S);
    for x in [0.0, 0.6 * S] {
        p.line(x, 0.0, x, -4.0 * S, 0.18 * S);
    }
}

fn repeat_one_bar(p: &mut Pen<'_>) {
    staff(p, -0.5 * S, 2.3 * S);
    p.sym(REPEAT_1_BAR, 0.0, -2.0 * S);
}

fn repeat_two_bars(p: &mut Pen<'_>) {
    staff(p, -0.5 * S, 3.4 * S);
    p.sym(REPEAT_2_BARS, 0.0, -2.0 * S);
}

fn repeat_start(p: &mut Pen<'_>) {
    staff(p, 0.0, 2.8 * S);
    p.sym(REPEAT_LEFT, 0.0, 0.0);
}

fn repeat_close(p: &mut Pen<'_>) {
    staff(p, -S, 1.6 * S);
    let w = p.advance(REPEAT_RIGHT);
    p.sym(REPEAT_RIGHT, 1.6 * S - w, 0.0);
}

fn alternate_endings(p: &mut Pen<'_>) {
    p.polyline(&[(0.0, 1.4 * S), (0.0, 0.0), (3.4 * S, 0.0)], LINE);
    p.text("1.", 0.4 * S, 1.75 * S, LABEL);
}

// Note.

fn dotted(p: &mut Pen<'_>) {
    p.sym(NOTE_QUARTER, 0.0, 0.0);
    p.sym(DOT, HEAD_W + 0.3 * S, 0.0);
}

fn double_dotted(p: &mut Pen<'_>) {
    dotted(p);
    p.sym(DOT, HEAD_W + 0.95 * S, 0.0);
}

fn triplet(p: &mut Pen<'_>) {
    beamed(p, 3, 0.0, 1.6 * S, 1);
    three(p, (stem_x(0.0) + stem_x(3.2 * S)) / 2.0, -3.6 * S, 0.9);
}

fn custom_tuplet(p: &mut Pen<'_>) {
    bracket(p, 0.0, 3.8 * S, 0.0, 0.7 * S);
    p.text_centred("n", 1.9 * S, 0.45 * S, 1.6 * S);
}

fn nested_tuplet(p: &mut Pen<'_>) {
    bracket(p, 0.0, 4.2 * S, 0.0, 0.7 * S);
    three(p, 2.1 * S, 0.45 * S, 0.8);
    bracket(p, 0.0, 2.4 * S, 1.6 * S, 0.55 * S);
    three(p, 1.2 * S, 2.0 * S, 0.65);
}

fn tie(p: &mut Pen<'_>) {
    let gap = 2.8 * S;
    head(p, 0.0, 0.0);
    head(p, gap, 0.0);
    p.arc((0.6 * S, 0.75 * S), (gap + 0.6 * S, 0.75 * S), 0.6 * S);
}

fn multiple_ties(p: &mut Pen<'_>) {
    let gap = 2.8 * S;
    for x in [0.0, gap] {
        head(p, x, 0.0);
        head(p, x, -S);
    }
    p.arc((0.6 * S, 0.75 * S), (gap + 0.6 * S, 0.75 * S), 0.6 * S);
    p.arc((0.6 * S, -1.75 * S), (gap + 0.6 * S, -1.75 * S), -0.6 * S);
}

fn change_accidental(p: &mut Pen<'_>) {
    p.sym(SHARP, 0.0, -0.7 * S);
    p.sym(FLAT, 1.35 * S, 1.2 * S);
}

fn semitone_down(p: &mut Pen<'_>) {
    p.sym(FLAT, 0.0, 0.0);
    arrow(p, 1.7 * S, -1.6 * S, 1.7 * S, 1.2 * S);
}

fn semitone_up(p: &mut Pen<'_>) {
    p.sym(SHARP, 0.0, 0.0);
    arrow(p, 1.8 * S, 1.4 * S, 1.8 * S, -1.6 * S);
}

/// A hairpin from `x0` to `x1`, open at `open_x`.
fn hairpin(p: &mut Pen<'_>, closed: f32, open: f32, y: f32, half: f32) {
    p.line(closed, y, open, y - half, 0.16 * S);
    p.line(closed, y, open, y + half, 0.16 * S);
}

fn crescendo(p: &mut Pen<'_>) {
    hairpin(p, 0.0, 4.2 * S, 0.0, 0.75 * S);
}

fn decrescendo(p: &mut Pen<'_>) {
    hairpin(p, 4.2 * S, 0.0, 0.0, 0.75 * S);
}

// Effects.

fn ghost(p: &mut Pen<'_>) {
    let gap = 0.12 * S;
    p.sym(PAREN_LEFT, 0.0, 0.0);
    let x = p.advance(PAREN_LEFT) + gap;
    head(p, x, 0.0);
    p.sym(PAREN_RIGHT, x + HEAD_W + gap, 0.0);
}

/// A notehead with `g` above it.
fn articulated(p: &mut Pen<'_>, g: Glyph) {
    head(p, 0.0, 0.0);
    sym_centred(p, g, HEAD_W / 2.0, -1.4 * S);
}

fn accented(p: &mut Pen<'_>) {
    articulated(p, ACCENT);
}

fn heavily_accented(p: &mut Pen<'_>) {
    articulated(p, MARCATO);
}

fn staccato(p: &mut Pen<'_>) {
    articulated(p, STACCATO);
}

fn legato(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    head(p, 2.6 * S, -S);
    p.arc((0.6 * S, -0.8 * S), (3.2 * S, -1.8 * S), -0.7 * S);
}

fn bend(p: &mut Pen<'_>) {
    let path = oxiguitar_display::Path::new()
        .move_to(0.0, 0.0)
        .line_to(1.0 * S, 0.0)
        .cubic_to(
            oxiguitar_display::Point::new(2.3 * S, 0.0),
            oxiguitar_display::Point::new(2.6 * S, -0.9 * S),
            oxiguitar_display::Point::new(2.6 * S, -2.6 * S),
        );
    p.stroke(path, LINE);
    p.arrowhead(2.6 * S, -3.6 * S, 0.0, -1.0, 1.1 * S);
}

fn fade_in(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    hairpin(p, -0.5 * S, 1.7 * S, -1.6 * S, 0.5 * S);
}

fn fade_out(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    hairpin(p, 1.7 * S, -0.5 * S, -1.6 * S, 0.5 * S);
}

fn volume_swell(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    let mid = HEAD_W / 2.0;
    hairpin(p, mid - 1.6 * S, mid, -1.6 * S, 0.45 * S);
    hairpin(p, mid + 1.6 * S, mid, -1.6 * S, 0.45 * S);
}

/// A slashed grace note, a barline and a quarter note; `before` puts the
/// grace note before the barline.
fn grace(p: &mut Pen<'_>, before: bool) {
    let small = 0.6;
    let (grace_x, bar_x) = if before {
        (0.0, 1.5 * S)
    } else {
        (0.7 * S, 0.0)
    };
    p.sym_scaled(GRACE_SLASH, grace_x, 0.5 * S, small);
    p.line(bar_x, -3.2 * S, bar_x, 1.2 * S, 0.16 * S);
    p.sym(NOTE_QUARTER, 2.2 * S, 0.0);
}

fn grace_before(p: &mut Pen<'_>) {
    grace(p, true);
}

fn grace_on(p: &mut Pen<'_>) {
    grace(p, false);
}

fn tremolo_picking(p: &mut Pen<'_>) {
    note_up(p, 0.0, 0.0, 3.4 * S);
    let x = stem_x(0.0);
    for i in 0..3 {
        let y = -1.6 * S - 0.7 * S * i as f32;
        let (w, h, slant) = (0.65 * S, 0.22 * S, 0.3 * S);
        let bar = oxiguitar_display::Path::new()
            .move_to(x - w, y + slant)
            .line_to(x + w, y - slant)
            .line_to(x + w, y - slant - h * 2.0)
            .line_to(x - w, y + slant - h * 2.0)
            .close();
        p.fill(bar);
    }
}

fn left_hand_tapping(p: &mut Pen<'_>) {
    p.circle(0.0, 0.0, 1.3 * S, LINE * 0.9);
    label(p, "T", 0.0, 0.0, LABEL);
}

/// Two notes, the second higher, joined by a slide; `slur` adds the legato.
fn slide(p: &mut Pen<'_>, slur: bool) {
    let gap = 2.8 * S;
    head(p, 0.0, 0.8 * S);
    head(p, gap, -0.8 * S);
    p.line(1.45 * S, 0.65 * S, gap - 0.25 * S, -0.65 * S, LINE);
    if slur {
        p.arc((0.6 * S, -0.1 * S), (gap + 0.6 * S, -1.7 * S), -0.8 * S);
    }
}

fn legato_slide(p: &mut Pen<'_>) {
    slide(p, true);
}

fn shift_slide(p: &mut Pen<'_>) {
    slide(p, false);
}

fn slide_in_below(p: &mut Pen<'_>) {
    p.line(-1.8 * S, 1.3 * S, -0.3 * S, 0.15 * S, LINE);
    head(p, 0.0, 0.0);
}

fn slide_in_above(p: &mut Pen<'_>) {
    p.line(-1.8 * S, -1.3 * S, -0.3 * S, -0.15 * S, LINE);
    head(p, 0.0, 0.0);
}

fn slide_out_down(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    p.line(HEAD_W + 0.3 * S, 0.15 * S, HEAD_W + 1.8 * S, 1.3 * S, LINE);
}

fn slide_out_up(p: &mut Pen<'_>) {
    head(p, 0.0, 0.0);
    p.line(
        HEAD_W + 0.3 * S,
        -0.15 * S,
        HEAD_W + 1.8 * S,
        -1.3 * S,
        LINE,
    );
}

fn whammy(p: &mut Pen<'_>) {
    p.polyline(
        &[
            (0.0, 0.0),
            (1.2 * S, 0.0),
            (2.2 * S, 1.7 * S),
            (3.2 * S, 0.0),
            (4.4 * S, 0.0),
        ],
        LINE,
    );
}

/// `n` copies of `g` side by side.
fn strokes(p: &mut Pen<'_>, g: Glyph, n: usize) {
    let w = p.advance(g);
    for i in 0..n {
        p.sym(g, w * i as f32, 0.0);
    }
}

fn vibrato(p: &mut Pen<'_>) {
    strokes(p, VIBRATO, 3);
}

fn wide_vibrato(p: &mut Pen<'_>) {
    strokes(p, VIBRATO_WIDE, 2);
}

/// A sawtooth line `n` teeth long.
fn sawtooth(p: &mut Pen<'_>, n: usize, step: f32, height: f32) {
    let points: Vec<(f32, f32)> = (0..=n * 2)
        .map(|i| {
            let x = step * i as f32 / 2.0;
            (x, if i % 2 == 0 { 0.0 } else { -height })
        })
        .collect();
    p.polyline(&points, LINE * 0.9);
}

fn bridge_vibrato(p: &mut Pen<'_>) {
    sawtooth(p, 4, 1.0 * S, 0.6 * S);
}

fn wide_bridge_vibrato(p: &mut Pen<'_>) {
    sawtooth(p, 3, 1.4 * S, 1.3 * S);
}

fn arpeggio_down(p: &mut Pen<'_>) {
    p.wave_down(0.0, -2.4 * S, 1.6 * S, 3, 0.4 * S, 0.18 * S);
    p.arrowhead(0.0, 2.8 * S, 0.0, 1.0, 1.1 * S);
}

fn arpeggio_up(p: &mut Pen<'_>) {
    p.wave_down(0.0, -1.6 * S, 2.4 * S, 3, 0.4 * S, 0.18 * S);
    p.arrowhead(0.0, -2.8 * S, 0.0, -1.0, 1.1 * S);
}

fn brush_down(p: &mut Pen<'_>) {
    arrow(p, 0.0, -2.4 * S, 0.0, 2.6 * S);
}

fn brush_up(p: &mut Pen<'_>) {
    arrow(p, 0.0, 2.4 * S, 0.0, -2.6 * S);
}

/// A notehead with a fingering above it.
fn fingered(p: &mut Pen<'_>, finger: &str) {
    head(p, 0.0, 0.0);
    p.text_centred(finger, HEAD_W / 2.0, -S, 1.7 * S);
}

fn left_hand_fingering(p: &mut Pen<'_>) {
    fingered(p, "1");
}

fn right_hand_fingering(p: &mut Pen<'_>) {
    fingered(p, "p");
}

fn string_number(p: &mut Pen<'_>) {
    p.circle(0.0, 0.0, 1.2 * S, LINE * 0.8);
    label(p, "3", 0.0, 0.0, 1.7 * S);
}

/// "G" with the finger that strikes as a small letter.
fn golpe(p: &mut Pen<'_>, finger: &str) {
    let size = 2.4 * S;
    p.text("G", 0.0, 0.0, size);
    let w = p.text_width("G", size);
    p.text(finger, w + 0.05 * S, 0.5 * S, 1.4 * S);
}

fn finger_golpe(p: &mut Pen<'_>) {
    golpe(p, "i");
}

fn thumb_golpe(p: &mut Pen<'_>) {
    golpe(p, "p");
}

/// A scraping zigzag along a slope, with an arrowhead where it ends.
fn scrape(p: &mut Pen<'_>, up: bool) {
    let (x1, rise) = (3.6 * S, if up { -2.8 * S } else { 2.8 * S });
    let n = 8;
    let len = (x1 * x1 + rise * rise).sqrt();
    let (nx, ny) = (-rise / len, x1 / len);
    let points: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let d = if i % 2 == 0 { 0.0 } else { 0.4 * S };
            (x1 * t + nx * d, rise * t + ny * d)
        })
        .collect();
    p.polyline(&points, 0.16 * S);
    let (ux, uy) = (x1 / len, rise / len);
    p.arrowhead(x1 + ux * 1.1 * S, rise + uy * 1.1 * S, ux, uy, 1.0 * S);
}

fn pick_scrape_up(p: &mut Pen<'_>) {
    scrape(p, true);
}

fn pick_scrape_down(p: &mut Pen<'_>) {
    scrape(p, false);
}

fn dead_slap(p: &mut Pen<'_>) {
    p.sym(NOTEHEAD_X, 0.0, 0.0);
    p.text_centred("S", HEAD_W / 2.0, -S, 1.7 * S);
}

// Notation.

/// A slash notehead centred at (`x`, `y`).
fn slash_head(p: &mut Pen<'_>, x: f32, y: f32) {
    let (w, h, t) = (0.75 * S, 1.0 * S, 0.45 * S);
    let shape = oxiguitar_display::Path::new()
        .move_to(x - w, y + h)
        .line_to(x - w + t, y + h)
        .line_to(x + w, y - h)
        .line_to(x + w - t, y - h)
        .close();
    p.fill(shape);
}

fn slash(p: &mut Pen<'_>) {
    slash_head(p, 0.0, 0.0);
    slash_head(p, 2.0 * S, 0.0);
}

fn barre(p: &mut Pen<'_>) {
    let size = 2.2 * S;
    p.text("B", 0.0, 0.0, size);
    let x0 = p.text_width("B", size) + 0.4 * S;
    let y = -0.8 * S;
    for i in 0..3 {
        let x = x0 + 0.95 * S * i as f32;
        p.line(x, y, x + 0.55 * S, y, LINE);
    }
    let end = x0 + 2.45 * S;
    p.line(end, y - LINE / 2.0, end, y + 0.8 * S, LINE);
}

fn section(p: &mut Pen<'_>) {
    let side = 2.6 * S;
    p.frame(0.0, 0.0, side, side, LINE * 0.9);
    label(p, "A", side / 2.0, side / 2.0, LABEL);
}

fn beams_auto(p: &mut Pen<'_>) {
    beamed(p, 2, 0.0, 1.5 * S, 1);
    beamed(p, 2, 3.3 * S, 1.5 * S, 1);
}

fn beams_force(p: &mut Pen<'_>) {
    beamed(p, 4, 0.0, 1.5 * S, 1);
}

fn beams_break(p: &mut Pen<'_>) {
    p.sym(NOTE_8TH, 0.0, 0.0);
    p.sym(NOTE_8TH, 2.0 * S, 0.0);
}

fn beams_break_secondary(p: &mut Pen<'_>) {
    let gap = 1.4 * S;
    beamed(p, 4, 0.0, gap, 1);
    let y = -2.25 * S;
    beam(p, stem_x(0.0), stem_x(gap), y);
    beam(p, stem_x(2.0 * gap), stem_x(3.0 * gap), y);
}

fn force_groups(p: &mut Pen<'_>) {
    let gap = 1.1 * S;
    beamed(p, 6, 0.0, gap, 1);
    three(p, stem_x(gap), -3.6 * S, 0.75);
    three(p, stem_x(4.0 * gap), -3.6 * S, 0.75);
}

/// A loudspeaker facing right, centred on `y`, `k` times its size.
pub(crate) fn speaker(p: &mut Pen<'_>, x: f32, y: f32, k: f32) {
    let s = S * k;
    p.rect(x, y - 0.5 * s, 0.7 * s, 1.0 * s);
    let cone = oxiguitar_display::Path::new()
        .move_to(x + 0.6 * s, y - 0.5 * s)
        .line_to(x + 1.5 * s, y - 1.3 * s)
        .line_to(x + 1.5 * s, y + 1.3 * s)
        .line_to(x + 0.6 * s, y + 0.5 * s)
        .close();
    p.fill(cone);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::{Fill, IconSet, glyphs_of, tests as icon_tests};

    fn set() -> IconSet {
        let faces: Vec<(Face, Fit)> = tools().map(|(_, t)| (t.face, t.fit)).collect();
        IconSet::new(&faces, Fill::PALETTE)
    }

    #[test]
    fn guitar_pro_8s_groups_and_counts() {
        let titles: Vec<&str> = GROUPS.iter().map(|g| g.title).collect();
        assert_eq!(
            titles,
            ["Voices and modes", "Bar", "Note", "Effects", "Notation"]
        );
        // As many tools as the GP8 guide lists for each group, less the
        // automation group, Timer and both stem-direction tools.
        let counts: Vec<usize> = GROUPS.iter().map(|g| g.tools.len()).collect();
        assert_eq!(counts, [8, 16, 34, 56, 10]);
        let mut ids: Vec<&str> = tools().map(|(_, t)| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), tools().count(), "tool ids are unique");
        assert_eq!(
            tool_tip("voice-2", "Edit Voice 2"),
            "Edit Voice 2 (Ctrl + 2)"
        );
        assert_eq!(tool_tip("rest", "Rest"), "Rest (R)");
        assert_eq!(tool_tip("half", "Half Note"), "Half Note");
        assert_eq!(tool_tip("bend", "Bend"), "Bend (B)");
        let slides = tools().filter(|(_, t)| matches(t.name, "slide")).count();
        assert_eq!(slides, 6);
        assert!(matches("Natural Harmonic", "nat harm"));
        assert!(!matches("Natural Harmonic", "pinch"));
        assert_eq!(tool_tip("chord", "Chord"), "Chord (A)");
        assert_eq!(tool_tip("lyrics", "Lyrics"), "Lyrics (Ctrl + T)");
        assert_eq!(tool_tip("fermata", "Fermata"), "Fermata (comes in M5)");
        assert_eq!(command_of("ff"), Some(Command::Dynamic(Dynamic::FF)));
        assert_eq!(
            command_of("shift-slide"),
            Some(Command::Effect(Effect::Slide(
                oxiguitar_model::Slide::Shift
            )))
        );
        let at = AtCaret {
            voice: 1,
            value: Some(NoteValue::Eighth),
            dots: 1,
            rest: false,
            tied: true,
            repeat_end: true,
            ..AtCaret::default()
        };
        for (id, on) in [
            ("voice-2", true),
            ("voice-1", false),
            ("eighth", true),
            ("quarter", false),
            ("dotted", true),
            ("double-dotted", false),
            ("rest", false),
            ("tie", true),
            ("bend", false),
            ("triplet", false),
            ("repeat-close", true),
            ("repeat-start", false),
        ] {
            assert_eq!(checked(id, &at), on, "{id}");
        }
        assert_eq!(voice_of("voice-4"), Some(3));
        assert_eq!(voice_of("voice-5"), None);
        assert_eq!(voice_of("multivoice"), None);
        // Every working tool is in the palette.
        for id in [
            "voice-1",
            "voice-2",
            "voice-3",
            "voice-4",
            "whole",
            "half",
            "quarter",
            "eighth",
            "16th",
            "32nd",
            "64th",
            "rest",
            "dotted",
            "double-dotted",
            "tie",
            "triplet",
            "tuplet",
            "repeat-start",
            "repeat-close",
            "free-time",
            "double-bar",
            "clef",
            "key-signature",
            "time-signature",
            "rhythmic-interpretation",
            "alternate-endings",
            "directions",
        ] {
            assert!(tools().any(|(_, t)| t.id == id), "{id}");
            assert!(working(id).is_some(), "{id} works");
        }
        assert_eq!(tool_tip("triplet", "Triplet"), "Triplet (/)");
        assert_eq!(dialog_of("tuplet"), Some(crate::editing::Dialog::Tuplet));
    }

    #[test]
    fn every_symbol_is_the_one_named() {
        let mut glyphs: Vec<Glyph> = tools().flat_map(|(_, t)| glyphs_of(t.face)).collect();
        glyphs.sort_by_key(|g| g.cp);
        glyphs.dedup();
        icon_tests::check_glyphs(&glyphs);
    }

    #[test]
    fn every_icon_is_inside_and_centred() {
        let names: Vec<&str> = tools().map(|(_, t)| t.name).collect();
        // 32 px buttons at 100 %, 125 %, 150 % and 200 %.
        icon_tests::check_inside_and_centred(&set(), &names, &[32, 40, 48, 64]);
    }

    #[test]
    fn groups_collapse_and_expand() {
        let mut p = Palette::default();
        assert!(!p.collapsed(2));
        p.toggle(2);
        assert!(p.collapsed(2) && !p.collapsed(1));
        p.toggle(2);
        assert!(!p.collapsed(2));
        p.toggle(99);
        assert!(!p.collapsed(99));
    }

    /// Writes every icon at 32 and 64 px into one picture, to look at:
    /// `OXIGUITAR_ICON_SHEET=icons.png cargo test -p oxiguitar-app icon_sheet`.
    #[test]
    fn icon_sheet() {
        let Some(path) = std::env::var_os("OXIGUITAR_ICON_SHEET") else {
            return;
        };
        let set = set();
        let cols = 16u32;
        let rows = (set.len() as u32).div_ceil(cols);
        for (suffix, px) in [("", 32u32), ("-64", 64)] {
            let cell = px + 8;
            let mut sheet = tiny_skia::Pixmap::new(cols * cell, rows * cell).unwrap();
            sheet.fill(tiny_skia::Color::from_rgba8(0x2b, 0x2b, 0x2e, 255));
            for (i, icon) in set.draw(px).into_iter().enumerate() {
                let img = tiny_skia::Pixmap::from_vec(
                    icon.rgba,
                    tiny_skia::IntSize::from_wh(px, px).unwrap(),
                )
                .unwrap();
                let (c, r) = (i as u32 % cols, i as u32 / cols);
                let mut frame = tiny_skia::Paint::default();
                frame.set_color_rgba8(0x3a, 0x3a, 0x3e, 255);
                sheet.fill_rect(
                    tiny_skia::Rect::from_xywh(
                        (c * cell + 4) as f32,
                        (r * cell + 4) as f32,
                        px as f32,
                        px as f32,
                    )
                    .unwrap(),
                    &frame,
                    tiny_skia::Transform::identity(),
                    None,
                );
                sheet.draw_pixmap(
                    (c * cell + 4) as i32,
                    (r * cell + 4) as i32,
                    img.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
            let mut out = std::path::PathBuf::from(&path);
            let stem = out.file_stem().unwrap().to_string_lossy().into_owned();
            out.set_file_name(format!("{stem}{suffix}.png"));
            sheet.save_png(&out).unwrap();
        }
    }
}
