// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Inspector's note and beat (M3 slice 6), without window code: the
//! caret's note and beat as rows of fields, and what a change to a field
//! does (Mateus, 20 Oct 2026: the Inspector edits them, and it is where
//! the effects' values change).
//!
//! A switch (a tickbox) sends the effect's own command, so it acts as its
//! key does, on the selection too; a choice or a number sets a value with
//! [`NoteChange`] or [`BeatChange`]; the bend and the whammy bar open
//! their editor.

use oxiguitar_edit::{BeatChange, Caret, Command, Effect, NoteChange};
use oxiguitar_model::{
    Accent, Beat, BeatKind, BeatTechnique, Dynamic, GraceKind, Harmonic, Note, NotePlace,
    NoteValue, Score, Slide, Stroke, StrokeDirection, Trill,
};

/// What a row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A heading.
    Heading,
    /// A tickbox.
    Switch,
    /// A drop-down.
    Choice,
    /// A number.
    Number,
    /// A button that opens an editor, with a summary beside it.
    Button,
}

/// One row of the Inspector's note or beat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    /// What a change names.
    pub(crate) id: &'static str,
    pub(crate) label: String,
    pub(crate) kind: Kind,
    /// On or off; the option picked; the number.
    pub(crate) value: i32,
    /// A choice's options; a button's summary as its only entry.
    pub(crate) options: Vec<String>,
    /// A number's range.
    pub(crate) min: i32,
    pub(crate) max: i32,
}

fn heading(label: String) -> Field {
    Field {
        id: "",
        label,
        kind: Kind::Heading,
        value: 0,
        options: Vec::new(),
        min: 0,
        max: 0,
    }
}

fn switch(id: &'static str, label: &str, on: bool) -> Field {
    Field {
        id,
        label: label.to_owned(),
        kind: Kind::Switch,
        value: i32::from(on),
        options: Vec::new(),
        min: 0,
        max: 1,
    }
}

fn choice(id: &'static str, label: &str, options: &[&str], picked: usize) -> Field {
    Field {
        id,
        label: label.to_owned(),
        kind: Kind::Choice,
        value: i32::try_from(picked).unwrap_or(0),
        options: options.iter().map(|o| (*o).to_owned()).collect(),
        min: 0,
        max: 0,
    }
}

fn number(id: &'static str, label: &str, value: u8, max: u8) -> Field {
    Field {
        id,
        label: label.to_owned(),
        kind: Kind::Number,
        value: i32::from(value),
        options: Vec::new(),
        min: 0,
        max: i32::from(max),
    }
}

fn button(id: &'static str, label: &str, summary: String) -> Field {
    Field {
        id,
        label: label.to_owned(),
        kind: Kind::Button,
        value: 0,
        options: vec![summary],
        min: 0,
        max: 0,
    }
}

const ACCENTS: [&str; 3] = ["None", "Accent", "Heavy accent"];
const SLIDES_IN: [&str; 3] = ["None", "From below", "From above"];
const SLIDES_OUT: [&str; 5] = ["None", "Shift", "Legato", "Downwards", "Upwards"];
const HARMONICS: [&str; 7] = [
    "None",
    "Natural",
    "Artificial",
    "Tapped",
    "Pinch",
    "Semi",
    "Feedback",
];
const TRILLS: [&str; 4] = ["None", "Sixteenths", "Thirty-seconds", "Sixty-fourths"];
const TRILL_SPEEDS: [NoteValue; 3] = [
    NoteValue::Sixteenth,
    NoteValue::ThirtySecond,
    NoteValue::SixtyFourth,
];
const LEFT: [&str; 6] = ["None", "Thumb", "1", "2", "3", "4"];
const RIGHT: [&str; 6] = ["None", "p", "i", "m", "a", "c"];
const DYNAMICS: [&str; 8] = ["ppp", "pp", "p", "mp", "mf", "f", "ff", "fff"];
const DYNAMIC_VALUES: [Dynamic; 8] = [
    Dynamic::PPP,
    Dynamic::PP,
    Dynamic::P,
    Dynamic::MP,
    Dynamic::MF,
    Dynamic::F,
    Dynamic::FF,
    Dynamic::FFF,
];
const TREMOLOS: [&str; 4] = ["None", "Eighths", "Sixteenths", "Thirty-seconds"];
const TREMOLO_SPEEDS: [NoteValue; 3] = [
    NoteValue::Eighth,
    NoteValue::Sixteenth,
    NoteValue::ThirtySecond,
];
const DIRECTIONS: [&str; 3] = ["None", "Down", "Up"];
const BRUSH_SPEEDS: [&str; 5] = [
    "A sixty-fourth",
    "A thirty-second",
    "A sixteenth",
    "An eighth",
    "A quarter",
];
const BRUSH_VALUES: [NoteValue; 5] = [
    NoteValue::SixtyFourth,
    NoteValue::ThirtySecond,
    NoteValue::Sixteenth,
    NoteValue::Eighth,
    NoteValue::Quarter,
];
const TECHNIQUES: [&str; 4] = ["None", "Tapping", "Slap", "Pop"];
const GRACES: [&str; 3] = ["None", "Before the beat", "On the beat"];

/// The caret's beat, its note (on the caret's string; on a staff without
/// strings, the beat's first), and whether the staff has strings.
fn here(score: &Score, c: Caret) -> Option<(&Beat, Option<&Note>, bool, u8)> {
    let track = score.tracks.get(c.track)?;
    let staff = track.staves.get(c.staff)?;
    let strings = staff.string_count() > 0 && !track.instrument.percussion;
    let beat = c.voice_in(score)?.beats.get(c.beat)?;
    if beat.kind == BeatKind::Empty {
        return None;
    }
    let note = if staff.string_count() > 0 {
        beat.notes.iter().find(|n| n.string() == Some(c.string))
    } else {
        beat.notes.first()
    };
    let note = note.filter(|_| beat.kind == BeatKind::Notes);
    Some((beat, note, strings, staff.fret_count))
}

/// A bend's or whammy curve's summary: its furthest pitch.
fn curve_summary(b: Option<&oxiguitar_model::Bend>) -> String {
    let Some(far) = b.and_then(|b| b.points.iter().map(|p| p.cents).max_by_key(|c| c.abs())) else {
        return "None".to_owned();
    };
    let rows = (far / crate::bend_form::ROW_CENTS).abs();
    let names = crate::bend_form::amounts();
    let name = usize::try_from(rows - 1)
        .ok()
        .and_then(|i| names.get(i).cloned())
        .unwrap_or_else(|| format!("{far} cents"));
    if far < 0 { format!("−{name}") } else { name }
}

fn note_value_name(v: NoteValue) -> &'static str {
    match v {
        NoteValue::DoubleWhole => "double whole note",
        NoteValue::Whole => "whole note",
        NoteValue::Half => "half note",
        NoteValue::Quarter => "quarter note",
        NoteValue::Eighth => "eighth note",
        NoteValue::Sixteenth => "sixteenth note",
        NoteValue::ThirtySecond => "thirty-second note",
        NoteValue::SixtyFourth => "sixty-fourth note",
        _ => "note",
    }
}

/// The rows for the caret's note and beat: none on an empty place.
pub(crate) fn fields(score: &Score, c: Caret) -> Vec<Field> {
    let Some((beat, note, strings, frets)) = here(score, c) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(n) = note {
        let fx = &n.effects;
        out.push(heading(match n.place {
            NotePlace::Fretted { string, fret } => format!("Note · string {string}, fret {fret}"),
            NotePlace::Pitched(p) => format!("Note · {}", p.name()),
        }));
        out.push(switch("dead", "Dead note", fx.dead));
        out.push(switch("ghost", "Ghost note", fx.ghost));
        out.push(choice(
            "accent",
            "Accent",
            &ACCENTS,
            match fx.accent {
                None => 0,
                Some(Accent::Normal) => 1,
                Some(Accent::Heavy) => 2,
            },
        ));
        out.push(switch("staccato", "Staccato", fx.staccato));
        let fretted = strings && matches!(n.place, NotePlace::Fretted { .. });
        if fretted {
            out.push(switch("hammer", "Hammer-on / pull-off", fx.hammer_pull));
            let slide_in = fx.slides.iter().find_map(|s| match s {
                Slide::InFromBelow => Some(1),
                Slide::InFromAbove => Some(2),
                _ => None,
            });
            let slide_out = fx.slides.iter().find_map(|s| match s {
                Slide::Shift => Some(1),
                Slide::Legato => Some(2),
                Slide::OutDown => Some(3),
                Slide::OutUp => Some(4),
                _ => None,
            });
            out.push(choice(
                "slide-in",
                "Slide in",
                &SLIDES_IN,
                slide_in.unwrap_or(0),
            ));
            out.push(choice(
                "slide-out",
                "Slide out",
                &SLIDES_OUT,
                slide_out.unwrap_or(0),
            ));
            out.push(switch("vibrato", "Vibrato", fx.vibrato));
            out.push(switch("palm-mute", "Palm mute", fx.palm_mute));
            out.push(switch(
                "let-ring",
                "Let ring",
                fx.let_ring || beat.effects.let_ring,
            ));
            out.push(switch("lh-tap", "Left-hand tapping", fx.left_hand_tap));
            let (h, value) = match fx.harmonic {
                None => (0, None),
                Some(Harmonic::Natural) => (1, None),
                Some(Harmonic::Artificial { semitones }) => (2, Some(semitones)),
                Some(Harmonic::Tapped { fret }) => (3, Some(fret)),
                Some(Harmonic::Pinch) => (4, None),
                Some(Harmonic::Semi) => (5, None),
                Some(Harmonic::Feedback) => (6, None),
            };
            out.push(choice("harmonic", "Harmonic", &HARMONICS, h));
            if let Some(v) = value {
                let label = if h == 2 {
                    "Semitones up"
                } else {
                    "Frets up, tapped"
                };
                out.push(number("harmonic-value", label, v, 48));
            }
            let speed = fx
                .trill
                .and_then(|t| TRILL_SPEEDS.iter().position(|s| *s == t.speed))
                .map_or(if fx.trill.is_some() { 1 } else { 0 }, |i| i + 1);
            out.push(choice("trill", "Trill", &TRILLS, speed));
            if let Some(t) = fx.trill {
                out.push(number("trill-fret", "With fret", t.fret, frets.min(99)));
            }
            out.push(button("bend", "Bend", curve_summary(fx.bend.as_ref())));
        }
        out.push(choice(
            "left-finger",
            "Left hand",
            &LEFT,
            fx.left_finger.map_or(0, |f| usize::from(f) + 1),
        ));
        out.push(choice(
            "right-finger",
            "Right hand",
            &RIGHT,
            fx.right_finger.map_or(0, |f| usize::from(f) + 1),
        ));
    }
    let fx = &beat.effects;
    let rest = beat.kind == BeatKind::Rest;
    out.push(heading(format!(
        "{} · {}",
        if rest { "Rest" } else { "Beat" },
        note_value_name(beat.duration.value)
    )));
    let dynamic = fx
        .dynamic
        .or_else(|| beat.notes.first().map(|n| n.dynamic))
        .unwrap_or_default();
    out.push(choice(
        "dynamic",
        "Dynamic",
        &DYNAMICS,
        DYNAMIC_VALUES
            .iter()
            .position(|d| *d == dynamic)
            .unwrap_or(5),
    ));
    let chord = beat
        .chord
        .and_then(|i| score.tracks.get(c.track)?.chords.get(i))
        .map_or_else(|| "None".to_owned(), |d| d.name.clone());
    out.push(button("chord", "Chord", chord));
    let text = if beat.text.is_empty() {
        "None".to_owned()
    } else {
        beat.text.clone()
    };
    out.push(button("text", "Text", text));
    if rest {
        return out;
    }
    let tremolo = fx
        .tremolo_picking
        .or_else(|| beat.notes.iter().find_map(|n| n.effects.tremolo_picking));
    out.push(choice(
        "tremolo",
        "Tremolo picking",
        &TREMOLOS,
        tremolo.map_or(0, |t| {
            TREMOLO_SPEEDS
                .iter()
                .position(|s| *s == t)
                .map_or(2, |i| i + 1)
        }),
    ));
    out.push(choice(
        "grace",
        "Grace note",
        &GRACES,
        match beat.grace {
            None => 0,
            Some(GraceKind::BeforeBeat) => 1,
            Some(GraceKind::OnBeat) => 2,
        },
    ));
    out.push(switch("fade-in", "Fade in", fx.fade_in));
    if strings && !beat.notes.is_empty() {
        let direction = |d: Option<StrokeDirection>| match d {
            None => 0,
            Some(StrokeDirection::Down) => 1,
            Some(StrokeDirection::Up) => 2,
        };
        out.push(choice(
            "brush",
            "Brush",
            &DIRECTIONS,
            direction(fx.stroke.map(|s| s.direction)),
        ));
        if let Some(s) = fx.stroke {
            out.push(choice(
                "brush-speed",
                "Brush over",
                &BRUSH_SPEEDS,
                BRUSH_VALUES.iter().position(|v| *v == s.speed).unwrap_or(2),
            ));
        }
        out.push(choice(
            "pick",
            "Pick stroke",
            &DIRECTIONS,
            direction(fx.pick_stroke),
        ));
        out.push(choice(
            "technique",
            "Technique",
            &TECHNIQUES,
            match fx.technique {
                None => 0,
                Some(BeatTechnique::Tap) => 1,
                Some(BeatTechnique::Slap) => 2,
                Some(BeatTechnique::Pop) => 3,
            },
        ));
        out.push(switch(
            "wide-vibrato",
            "Wide vibrato (whammy bar)",
            fx.vibrato_wide,
        ));
        out.push(button(
            "whammy",
            "Whammy bar",
            curve_summary(fx.whammy.as_ref()),
        ));
    }
    out
}

/// What a change to a field does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Action {
    /// An edit at the caret (or on the selection).
    Edit(Command),
    /// The bend editor.
    Bend,
    /// The whammy bar's editor.
    Whammy,
    /// The chord dialog.
    Chord,
    /// The beat's text.
    Text,
}

/// The field `id` changed to `value` (on or off, the option picked, the
/// number): what to do. `None` when nothing changes.
pub(crate) fn changed(score: &Score, c: Caret, id: &str, value: i32) -> Option<Action> {
    let (beat, note, _, _) = here(score, c)?;
    let pick = usize::try_from(value).ok();
    let fx = note.map(|n| &n.effects);
    let effect = |e: Effect| Some(Action::Edit(Command::Effect(e)));
    let note_change = |n: NoteChange| Some(Action::Edit(Command::Note(n)));
    let beat_change = |b: BeatChange| Some(Action::Edit(Command::Beat(b)));
    match id {
        "dead" => effect(Effect::Dead),
        "ghost" => effect(Effect::Ghost),
        "staccato" => effect(Effect::Staccato),
        "hammer" => effect(Effect::HammerPull),
        "vibrato" => effect(Effect::Vibrato),
        "palm-mute" => effect(Effect::PalmMute),
        "let-ring" => effect(Effect::LetRing),
        "lh-tap" => effect(Effect::LeftHandTap),
        "fade-in" => effect(Effect::FadeIn),
        "wide-vibrato" => effect(Effect::WideVibrato),
        "bend" => Some(Action::Bend),
        "whammy" => Some(Action::Whammy),
        "chord" => Some(Action::Chord),
        "text" => Some(Action::Text),
        "accent" => note_change(NoteChange::Accent(match pick? {
            0 => None,
            1 => Some(Accent::Normal),
            _ => Some(Accent::Heavy),
        })),
        "slide-in" => note_change(NoteChange::SlideIn(match pick? {
            0 => None,
            1 => Some(Slide::InFromBelow),
            _ => Some(Slide::InFromAbove),
        })),
        "slide-out" => note_change(NoteChange::SlideOut(match pick? {
            0 => None,
            1 => Some(Slide::Shift),
            2 => Some(Slide::Legato),
            3 => Some(Slide::OutDown),
            _ => Some(Slide::OutUp),
        })),
        "harmonic" => {
            // A value already set carries over between artificial and
            // tapped.
            let kept = match fx?.harmonic {
                Some(Harmonic::Artificial { semitones }) => semitones,
                Some(Harmonic::Tapped { fret }) => fret,
                _ => 12,
            };
            note_change(NoteChange::Harmonic(match pick? {
                0 => None,
                1 => Some(Harmonic::Natural),
                2 => Some(Harmonic::Artificial { semitones: kept }),
                3 => Some(Harmonic::Tapped { fret: kept }),
                4 => Some(Harmonic::Pinch),
                5 => Some(Harmonic::Semi),
                _ => Some(Harmonic::Feedback),
            }))
        }
        "harmonic-value" => {
            let v = u8::try_from(value).ok()?;
            note_change(NoteChange::Harmonic(match fx?.harmonic? {
                Harmonic::Artificial { .. } => Some(Harmonic::Artificial { semitones: v }),
                Harmonic::Tapped { .. } => Some(Harmonic::Tapped { fret: v }),
                _ => return None,
            }))
        }
        "trill" => {
            let n = note?;
            let fret = match (fx?.trill, n.place) {
                (Some(t), _) => t.fret,
                (None, NotePlace::Fretted { fret, .. }) => fret.saturating_add(1),
                (None, NotePlace::Pitched(_)) => return None,
            };
            note_change(NoteChange::Trill(match pick? {
                0 => None,
                i => Some(Trill {
                    fret,
                    speed: *TRILL_SPEEDS.get(i - 1)?,
                }),
            }))
        }
        "trill-fret" => {
            let t = fx?.trill?;
            note_change(NoteChange::Trill(Some(Trill {
                fret: u8::try_from(value).ok()?,
                ..t
            })))
        }
        "left-finger" | "right-finger" => {
            let f = match pick? {
                0 => None,
                i => Some(u8::try_from(i - 1).ok()?),
            };
            note_change(if id == "left-finger" {
                NoteChange::LeftFinger(f)
            } else {
                NoteChange::RightFinger(f)
            })
        }
        "dynamic" => Some(Action::Edit(Command::Dynamic(*DYNAMIC_VALUES.get(pick?)?))),
        "tremolo" => beat_change(BeatChange::Tremolo(match pick? {
            0 => None,
            i => Some(*TREMOLO_SPEEDS.get(i - 1)?),
        })),
        "grace" => beat_change(BeatChange::Grace(match pick? {
            0 => None,
            1 => Some(GraceKind::BeforeBeat),
            _ => Some(GraceKind::OnBeat),
        })),
        "brush" => {
            let speed = beat
                .effects
                .stroke
                .map_or(NoteValue::Sixteenth, |s| s.speed);
            beat_change(BeatChange::Stroke(match pick? {
                0 => None,
                1 => Some(Stroke {
                    direction: StrokeDirection::Down,
                    speed,
                }),
                _ => Some(Stroke {
                    direction: StrokeDirection::Up,
                    speed,
                }),
            }))
        }
        "brush-speed" => {
            let s = beat.effects.stroke?;
            beat_change(BeatChange::Stroke(Some(Stroke {
                speed: *BRUSH_VALUES.get(pick?)?,
                ..s
            })))
        }
        "pick" => beat_change(BeatChange::PickStroke(match pick? {
            0 => None,
            1 => Some(StrokeDirection::Down),
            _ => Some(StrokeDirection::Up),
        })),
        "technique" => beat_change(BeatChange::Technique(match pick? {
            0 => None,
            1 => Some(BeatTechnique::Tap),
            2 => Some(BeatTechnique::Slap),
            _ => Some(BeatTechnique::Pop),
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use oxiguitar_edit::{History, apply, new_score};

    fn at(beat: usize, string: u8) -> Caret {
        Caret {
            beat,
            string,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    fn with_note() -> Score {
        let mut s = new_score();
        let mut h = History::new();
        apply(
            &mut s,
            &mut h,
            at(0, 2),
            None,
            Command::Fret {
                fret: 5,
                again: false,
            },
        )
        .unwrap();
        s
    }

    fn field<'a>(f: &'a [Field], id: &str) -> &'a Field {
        f.iter().find(|f| f.id == id).unwrap()
    }

    #[test]
    fn the_caret_s_note_and_beat_as_rows() {
        let mut s = with_note();
        let f = fields(&s, at(0, 2));
        assert_eq!(f[0].label, "Note · string 2, fret 5");
        assert_eq!(f[0].kind, Kind::Heading);
        assert_eq!(field(&f, "bend").options, ["None"]);
        assert!(f.iter().any(|f| f.label == "Beat · quarter note"));
        // No note on string 3: only the beat.
        let f = fields(&s, at(0, 3));
        assert_eq!(f[0].label, "Beat · quarter note");
        // A trill shows its fret.
        let mut h = History::new();
        let c = at(0, 2);
        let a = changed(&s, c, "trill", 2).unwrap();
        let Action::Edit(cmd) = a else { panic!() };
        apply(&mut s, &mut h, c, None, cmd).unwrap();
        let f = fields(&s, c);
        assert_eq!(field(&f, "trill").value, 2);
        assert_eq!(field(&f, "trill-fret").value, 6);
        assert_eq!(field(&f, "trill-fret").max, 24);
        // A harmonic's value.
        let Action::Edit(cmd) = changed(&s, c, "harmonic", 3).unwrap() else {
            panic!()
        };
        apply(&mut s, &mut h, c, None, cmd).unwrap();
        let f = fields(&s, c);
        assert_eq!(field(&f, "harmonic-value").label, "Frets up, tapped");
        assert_eq!(field(&f, "harmonic-value").value, 12);
        assert_eq!(
            changed(&s, c, "harmonic-value", 7),
            Some(Action::Edit(Command::Note(NoteChange::Harmonic(Some(
                Harmonic::Tapped { fret: 7 }
            )))))
        );
        // A rest: its dynamic only.
        apply(&mut s, &mut h, at(1, 2), None, Command::Rest).unwrap();
        let f = fields(&s, at(1, 2));
        assert_eq!(f[0].label, "Rest · quarter note");
        assert_eq!(f.len(), 4, "dynamic, chord and text");
    }

    #[test]
    fn changes_become_commands() {
        let s = with_note();
        let c = at(0, 2);
        assert_eq!(
            changed(&s, c, "palm-mute", 1),
            Some(Action::Edit(Command::Effect(Effect::PalmMute)))
        );
        assert_eq!(changed(&s, c, "bend", 0), Some(Action::Bend));
        assert_eq!(
            changed(&s, c, "slide-out", 2),
            Some(Action::Edit(Command::Note(NoteChange::SlideOut(Some(
                Slide::Legato
            )))))
        );
        assert_eq!(
            changed(&s, c, "brush", 2),
            Some(Action::Edit(Command::Beat(BeatChange::Stroke(Some(
                Stroke {
                    direction: StrokeDirection::Up,
                    speed: NoteValue::Sixteenth
                }
            )))))
        );
        assert_eq!(
            changed(&s, c, "dynamic", 0),
            Some(Action::Edit(Command::Dynamic(Dynamic::PPP)))
        );
        assert_eq!(
            changed(&s, c, "left-finger", 1),
            Some(Action::Edit(Command::Note(NoteChange::LeftFinger(Some(0)))))
        );
        assert_eq!(changed(&s, c, "trill-fret", 3), None, "no trill yet");
        assert_eq!(changed(&s, c, "nonsense", 0), None);
    }
}
