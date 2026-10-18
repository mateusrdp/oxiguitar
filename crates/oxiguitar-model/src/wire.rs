// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Converting between the model and the score as it crosses the plugin
//! interface (`oxiguitar_plugin_api::score`, version 1).
//!
//! The two have the same shape today, so nothing is lost either way
//! (`tests/wire.rs` in `oxiguitar-formats` checks this on every fixture).
//! Every model struct is taken apart field by field (and every wire struct
//! built field by field), so a field added to either doesn't compile here until someone decides how it crosses the
//! interface: in a new version of the wire score, never by changing
//! version 1.

use oxiguitar_plugin_api::score as w;

use crate::*;

/// The wire form of `score`, for a plugin. Indices that don't fit in 32
/// bits (no real score has 4 billion chord diagrams) become `u32::MAX`,
/// which points at nothing.
pub fn to_wire(score: &Score) -> w::Score {
    let Score {
        info,
        initial_tempo,
        tempo_label,
        master_bars,
        tracks,
    } = score;
    w::Score {
        info: info_to(info),
        initial_tempo: *initial_tempo,
        tempo_label: tempo_label.clone(),
        master_bars: master_bars.iter().map(master_bar_to).collect(),
        tracks: tracks.iter().map(track_to).collect(),
    }
}

/// The model form of a score a plugin sent. Fails only on a fraction with
/// a zero denominator, or one too large to reduce; the caller should then
/// check the score's structure ([`Score::check_structure`]) too.
pub fn from_wire(score: w::Score) -> Result<Score, String> {
    let w::Score {
        info,
        initial_tempo,
        tempo_label,
        master_bars,
        tracks,
    } = score;
    Ok(Score {
        info: info_from(info),
        initial_tempo,
        tempo_label,
        master_bars: master_bars
            .into_iter()
            .enumerate()
            .map(|(i, b)| master_bar_from(b).map_err(|e| format!("bar {}: {e}", i + 1)))
            .collect::<Result<_, _>>()?,
        tracks: tracks
            .into_iter()
            .enumerate()
            .map(|(i, t)| track_from(t).map_err(|e| format!("track {}: {e}", i + 1)))
            .collect::<Result<_, _>>()?,
    })
}

fn index_to(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn index_from(i: u32) -> usize {
    usize::try_from(i).unwrap_or(usize::MAX)
}

fn ratio_to(r: Ratio) -> w::Ratio {
    w::Ratio {
        num: r.numer(),
        den: r.denom(),
    }
}

fn ratio_from(r: w::Ratio) -> Result<Ratio, String> {
    Ratio::new(r.num, r.den).ok_or_else(|| format!("{}/{} is not a valid fraction", r.num, r.den))
}

fn info_to(i: &ScoreInfo) -> w::ScoreInfo {
    let ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    } = i;
    w::ScoreInfo {
        title: title.clone(),
        subtitle: subtitle.clone(),
        artist: artist.clone(),
        album: album.clone(),
        words: words.clone(),
        music: music.clone(),
        copyright: copyright.clone(),
        transcriber: transcriber.clone(),
        instructions: instructions.clone(),
        notices: notices.clone(),
    }
}

fn info_from(i: w::ScoreInfo) -> ScoreInfo {
    let w::ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    } = i;
    ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    }
}

fn master_bar_to(b: &MasterBar) -> w::MasterBar {
    let MasterBar {
        time_signature,
        key_signature,
        repeat_start,
        repeat_end,
        alternate_endings,
        marker,
        double_bar,
        triplet_feel,
        tempo_changes,
        directions,
    } = b;
    w::MasterBar {
        time_signature: w::TimeSignature {
            numerator: time_signature.numerator,
            denominator: time_signature.denominator,
        },
        key_signature: w::KeySignature {
            fifths: key_signature.fifths,
            mode: match key_signature.mode {
                KeyMode::Major => w::KeyMode::Major,
                KeyMode::Minor => w::KeyMode::Minor,
            },
        },
        repeat_start: *repeat_start,
        repeat_end: *repeat_end,
        alternate_endings: *alternate_endings,
        marker: marker.as_ref().map(
            |Marker {
                 letter,
                 text,
                 color,
             }| w::Marker {
                letter: letter.clone(),
                text: text.clone(),
                color: color_to(*color),
            },
        ),
        double_bar: *double_bar,
        triplet_feel: match triplet_feel {
            TripletFeel::None => w::TripletFeel::None,
            TripletFeel::Triplet8th => w::TripletFeel::Triplet8th,
            TripletFeel::Triplet16th => w::TripletFeel::Triplet16th,
            TripletFeel::Dotted8th => w::TripletFeel::Dotted8th,
            TripletFeel::Dotted16th => w::TripletFeel::Dotted16th,
            TripletFeel::Scottish8th => w::TripletFeel::Scottish8th,
            TripletFeel::Scottish16th => w::TripletFeel::Scottish16th,
        },
        tempo_changes: tempo_changes
            .iter()
            .map(
                |TempoChange {
                     position,
                     bpm,
                     label,
                     linear,
                 }| w::TempoChange {
                    position: ratio_to(*position),
                    bpm: *bpm,
                    label: label.clone(),
                    linear: *linear,
                },
            )
            .collect(),
        directions: directions.iter().map(|&d| direction_to(d)).collect(),
    }
}

fn master_bar_from(b: w::MasterBar) -> Result<MasterBar, String> {
    let w::MasterBar {
        time_signature,
        key_signature,
        repeat_start,
        repeat_end,
        alternate_endings,
        marker,
        double_bar,
        triplet_feel,
        tempo_changes,
        directions,
    } = b;
    Ok(MasterBar {
        time_signature: TimeSignature {
            numerator: time_signature.numerator,
            denominator: time_signature.denominator,
        },
        key_signature: KeySignature {
            fifths: key_signature.fifths,
            mode: match key_signature.mode {
                w::KeyMode::Major => KeyMode::Major,
                w::KeyMode::Minor => KeyMode::Minor,
            },
        },
        repeat_start,
        repeat_end,
        alternate_endings,
        marker: marker.map(|m| Marker {
            letter: m.letter,
            text: m.text,
            color: color_from(m.color),
        }),
        double_bar,
        triplet_feel: match triplet_feel {
            w::TripletFeel::None => TripletFeel::None,
            w::TripletFeel::Triplet8th => TripletFeel::Triplet8th,
            w::TripletFeel::Triplet16th => TripletFeel::Triplet16th,
            w::TripletFeel::Dotted8th => TripletFeel::Dotted8th,
            w::TripletFeel::Dotted16th => TripletFeel::Dotted16th,
            w::TripletFeel::Scottish8th => TripletFeel::Scottish8th,
            w::TripletFeel::Scottish16th => TripletFeel::Scottish16th,
        },
        tempo_changes: tempo_changes
            .into_iter()
            .map(|t| {
                Ok(TempoChange {
                    position: ratio_from(t.position)?,
                    bpm: t.bpm,
                    label: t.label,
                    linear: t.linear,
                })
            })
            .collect::<Result<_, String>>()?,
        directions: directions.into_iter().map(direction_from).collect(),
    })
}

fn color_to(c: Color) -> w::Color {
    w::Color {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

fn color_from(c: w::Color) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

/// Pairs each model direction with its wire twin; one table for both ways.
macro_rules! directions {
    ($($name:ident),* $(,)?) => {
        fn direction_to(d: Direction) -> w::Direction {
            match d {
                $(Direction::$name => w::Direction::$name,)*
            }
        }

        fn direction_from(d: w::Direction) -> Direction {
            match d {
                $(w::Direction::$name => Direction::$name,)*
            }
        }
    };
}

directions!(
    Segno,
    SegnoSegno,
    Coda,
    DoubleCoda,
    Fine,
    ToCoda,
    ToDoubleCoda,
    DaCapo,
    DaCapoAlCoda,
    DaCapoAlDoubleCoda,
    DaCapoAlFine,
    DalSegno,
    DalSegnoAlCoda,
    DalSegnoAlDoubleCoda,
    DalSegnoAlFine,
    DalSegnoSegno,
    DalSegnoSegnoAlCoda,
    DalSegnoSegnoAlDoubleCoda,
    DalSegnoSegnoAlFine,
);

fn track_to(t: &Track) -> w::Track {
    let Track {
        name,
        short_name,
        color,
        instrument,
        mix,
        staves,
        chords,
        lyrics,
    } = t;
    let Instrument {
        midi_program,
        percussion,
    } = *instrument;
    let Mix {
        volume,
        pan,
        mute,
        solo,
    } = *mix;
    w::Track {
        name: name.clone(),
        short_name: short_name.clone(),
        color: color_to(*color),
        instrument: w::Instrument {
            midi_program,
            percussion,
        },
        mix: w::Mix {
            volume,
            pan,
            mute,
            solo,
        },
        staves: staves.iter().map(staff_to).collect(),
        chords: chords
            .iter()
            .map(
                |ChordDiagram {
                     name,
                     first_fret,
                     frets,
                 }| w::ChordDiagram {
                    name: name.clone(),
                    first_fret: *first_fret,
                    frets: frets.clone(),
                },
            )
            .collect(),
        lyrics: lyrics
            .iter()
            .map(|LyricLine { start_bar, text }| w::LyricLine {
                start_bar: index_to(*start_bar),
                text: text.clone(),
            })
            .collect(),
    }
}

fn track_from(t: w::Track) -> Result<Track, String> {
    let w::Track {
        name,
        short_name,
        color,
        instrument,
        mix,
        staves,
        chords,
        lyrics,
    } = t;
    Ok(Track {
        name,
        short_name,
        color: color_from(color),
        instrument: Instrument {
            midi_program: instrument.midi_program,
            percussion: instrument.percussion,
        },
        mix: Mix {
            volume: mix.volume,
            pan: mix.pan,
            mute: mix.mute,
            solo: mix.solo,
        },
        staves: staves
            .into_iter()
            .map(staff_from)
            .collect::<Result<_, _>>()?,
        chords: chords
            .into_iter()
            .map(|c| ChordDiagram {
                name: c.name,
                first_fret: c.first_fret,
                frets: c.frets,
            })
            .collect(),
        lyrics: lyrics
            .into_iter()
            .map(|l| LyricLine {
                start_bar: index_from(l.start_bar),
                text: l.text,
            })
            .collect(),
    })
}

fn staff_to(s: &Staff) -> w::Staff {
    let Staff {
        tuning,
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars,
    } = s;
    w::Staff {
        tuning: tuning.iter().map(|p| w::Pitch(p.0)).collect(),
        capo: *capo,
        fret_count: *fret_count,
        transposition: *transposition,
        show_standard: *show_standard,
        show_tablature: *show_tablature,
        bars: bars
            .iter()
            .map(|Bar { clef, voices }| w::Bar {
                clef: clef_to(*clef),
                voices: voices
                    .iter()
                    .map(|Voice { beats }| w::Voice {
                        beats: beats.iter().map(beat_to).collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn staff_from(s: w::Staff) -> Result<Staff, String> {
    let w::Staff {
        tuning,
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars,
    } = s;
    Ok(Staff {
        tuning: tuning.into_iter().map(|p| Pitch(p.0)).collect(),
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars: bars
            .into_iter()
            .enumerate()
            .map(|(i, b)| {
                Ok(Bar {
                    clef: clef_from(b.clef),
                    voices: b
                        .voices
                        .into_iter()
                        .map(|v| {
                            Ok(Voice {
                                beats: v
                                    .beats
                                    .into_iter()
                                    .map(beat_from)
                                    .collect::<Result<_, String>>()?,
                            })
                        })
                        .collect::<Result<_, String>>()
                        .map_err(|e| format!("bar {}: {e}", i + 1))?,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn clef_to(c: Clef) -> w::Clef {
    match c {
        Clef::Treble => w::Clef::Treble,
        Clef::Treble8vb => w::Clef::Treble8vb,
        Clef::Bass => w::Clef::Bass,
        Clef::Bass8vb => w::Clef::Bass8vb,
        Clef::Alto => w::Clef::Alto,
        Clef::Tenor => w::Clef::Tenor,
        Clef::Neutral => w::Clef::Neutral,
    }
}

fn clef_from(c: w::Clef) -> Clef {
    match c {
        w::Clef::Treble => Clef::Treble,
        w::Clef::Treble8vb => Clef::Treble8vb,
        w::Clef::Bass => Clef::Bass,
        w::Clef::Bass8vb => Clef::Bass8vb,
        w::Clef::Alto => Clef::Alto,
        w::Clef::Tenor => Clef::Tenor,
        w::Clef::Neutral => Clef::Neutral,
    }
}

fn beat_to(b: &Beat) -> w::Beat {
    let Beat {
        kind,
        duration,
        notes,
        grace,
        effects,
        chord,
        text,
    } = b;
    let BeatEffects {
        stroke,
        pick_stroke,
        whammy,
        technique,
        fade_in,
        vibrato_wide,
        tremolo_picking,
        dynamic,
        let_ring,
    } = effects;
    w::Beat {
        kind: match kind {
            BeatKind::Notes => w::BeatKind::Notes,
            BeatKind::Rest => w::BeatKind::Rest,
            BeatKind::Empty => w::BeatKind::Empty,
        },
        duration: duration_to(duration),
        notes: notes.iter().map(note_to).collect(),
        grace: grace.map(|g| match g {
            GraceKind::BeforeBeat => w::GraceKind::BeforeBeat,
            GraceKind::OnBeat => w::GraceKind::OnBeat,
        }),
        effects: w::BeatEffects {
            stroke: stroke.map(|Stroke { direction, speed }| w::Stroke {
                direction: stroke_direction_to(direction),
                speed: value_to(speed),
            }),
            pick_stroke: pick_stroke.map(stroke_direction_to),
            whammy: whammy.as_ref().map(bend_to),
            technique: technique.map(|t| match t {
                BeatTechnique::Tap => w::BeatTechnique::Tap,
                BeatTechnique::Slap => w::BeatTechnique::Slap,
                BeatTechnique::Pop => w::BeatTechnique::Pop,
            }),
            fade_in: *fade_in,
            vibrato_wide: *vibrato_wide,
            tremolo_picking: tremolo_picking.map(value_to),
            dynamic: dynamic.map(dynamic_to),
            let_ring: *let_ring,
        },
        chord: chord.map(index_to),
        text: text.clone(),
    }
}

fn beat_from(b: w::Beat) -> Result<Beat, String> {
    let w::Beat {
        kind,
        duration,
        notes,
        grace,
        effects,
        chord,
        text,
    } = b;
    let w::BeatEffects {
        stroke,
        pick_stroke,
        whammy,
        technique,
        fade_in,
        vibrato_wide,
        tremolo_picking,
        dynamic,
        let_ring,
    } = effects;
    Ok(Beat {
        kind: match kind {
            w::BeatKind::Notes => BeatKind::Notes,
            w::BeatKind::Rest => BeatKind::Rest,
            w::BeatKind::Empty => BeatKind::Empty,
        },
        duration: duration_from(duration),
        notes: notes.into_iter().map(note_from).collect::<Result<_, _>>()?,
        grace: grace.map(|g| match g {
            w::GraceKind::BeforeBeat => GraceKind::BeforeBeat,
            w::GraceKind::OnBeat => GraceKind::OnBeat,
        }),
        effects: BeatEffects {
            stroke: stroke.map(|s| Stroke {
                direction: stroke_direction_from(s.direction),
                speed: value_from(s.speed),
            }),
            pick_stroke: pick_stroke.map(stroke_direction_from),
            whammy: whammy.map(bend_from).transpose()?,
            technique: technique.map(|t| match t {
                w::BeatTechnique::Tap => BeatTechnique::Tap,
                w::BeatTechnique::Slap => BeatTechnique::Slap,
                w::BeatTechnique::Pop => BeatTechnique::Pop,
            }),
            fade_in,
            vibrato_wide,
            tremolo_picking: tremolo_picking.map(value_from),
            dynamic: dynamic.map(dynamic_from),
            let_ring,
        },
        chord: chord.map(index_from),
        text,
    })
}

fn stroke_direction_to(d: StrokeDirection) -> w::StrokeDirection {
    match d {
        StrokeDirection::Down => w::StrokeDirection::Down,
        StrokeDirection::Up => w::StrokeDirection::Up,
    }
}

fn stroke_direction_from(d: w::StrokeDirection) -> StrokeDirection {
    match d {
        w::StrokeDirection::Down => StrokeDirection::Down,
        w::StrokeDirection::Up => StrokeDirection::Up,
    }
}

fn value_to(v: NoteValue) -> w::NoteValue {
    match v {
        NoteValue::DoubleWhole => w::NoteValue::DoubleWhole,
        NoteValue::Whole => w::NoteValue::Whole,
        NoteValue::Half => w::NoteValue::Half,
        NoteValue::Quarter => w::NoteValue::Quarter,
        NoteValue::Eighth => w::NoteValue::Eighth,
        NoteValue::Sixteenth => w::NoteValue::Sixteenth,
        NoteValue::ThirtySecond => w::NoteValue::ThirtySecond,
        NoteValue::SixtyFourth => w::NoteValue::SixtyFourth,
        NoteValue::OneTwentyEighth => w::NoteValue::OneTwentyEighth,
        NoteValue::TwoFiftySixth => w::NoteValue::TwoFiftySixth,
    }
}

fn value_from(v: w::NoteValue) -> NoteValue {
    match v {
        w::NoteValue::DoubleWhole => NoteValue::DoubleWhole,
        w::NoteValue::Whole => NoteValue::Whole,
        w::NoteValue::Half => NoteValue::Half,
        w::NoteValue::Quarter => NoteValue::Quarter,
        w::NoteValue::Eighth => NoteValue::Eighth,
        w::NoteValue::Sixteenth => NoteValue::Sixteenth,
        w::NoteValue::ThirtySecond => NoteValue::ThirtySecond,
        w::NoteValue::SixtyFourth => NoteValue::SixtyFourth,
        w::NoteValue::OneTwentyEighth => NoteValue::OneTwentyEighth,
        w::NoteValue::TwoFiftySixth => NoteValue::TwoFiftySixth,
    }
}

fn duration_to(d: &Duration) -> w::Duration {
    let Duration {
        value,
        dots,
        tuplets,
    } = d;
    w::Duration {
        value: value_to(*value),
        dots: *dots,
        tuplets: tuplets
            .iter()
            .map(|&Tuplet { notes, in_time_of }| w::Tuplet { notes, in_time_of })
            .collect(),
    }
}

fn duration_from(d: w::Duration) -> Duration {
    Duration {
        value: value_from(d.value),
        dots: d.dots,
        tuplets: d
            .tuplets
            .into_iter()
            .map(|t| Tuplet {
                notes: t.notes,
                in_time_of: t.in_time_of,
            })
            .collect(),
    }
}

fn dynamic_to(d: Dynamic) -> w::Dynamic {
    match d {
        Dynamic::PPP => w::Dynamic::PPP,
        Dynamic::PP => w::Dynamic::PP,
        Dynamic::P => w::Dynamic::P,
        Dynamic::MP => w::Dynamic::MP,
        Dynamic::MF => w::Dynamic::MF,
        Dynamic::F => w::Dynamic::F,
        Dynamic::FF => w::Dynamic::FF,
        Dynamic::FFF => w::Dynamic::FFF,
    }
}

fn dynamic_from(d: w::Dynamic) -> Dynamic {
    match d {
        w::Dynamic::PPP => Dynamic::PPP,
        w::Dynamic::PP => Dynamic::PP,
        w::Dynamic::P => Dynamic::P,
        w::Dynamic::MP => Dynamic::MP,
        w::Dynamic::MF => Dynamic::MF,
        w::Dynamic::F => Dynamic::F,
        w::Dynamic::FF => Dynamic::FF,
        w::Dynamic::FFF => Dynamic::FFF,
    }
}

fn bend_to(Bend { points }: &Bend) -> w::Bend {
    w::Bend {
        points: points
            .iter()
            .map(|&BendPoint { offset, cents }| w::BendPoint {
                offset: ratio_to(offset),
                cents,
            })
            .collect(),
    }
}

fn bend_from(b: w::Bend) -> Result<Bend, String> {
    Ok(Bend {
        points: b
            .points
            .into_iter()
            .map(|p| {
                Ok(BendPoint {
                    offset: ratio_from(p.offset)?,
                    cents: p.cents,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn note_to(n: &Note) -> w::Note {
    let Note {
        place,
        tie,
        dynamic,
        effects,
    } = n;
    let NoteEffects {
        hammer_pull,
        slides,
        bend,
        harmonic,
        vibrato,
        palm_mute,
        let_ring,
        dead,
        ghost,
        accent,
        staccato,
        trill,
        tremolo_picking,
        left_finger,
        right_finger,
        left_hand_tap,
    } = effects;
    w::Note {
        place: match *place {
            NotePlace::Fretted { string, fret } => w::NotePlace::Fretted { string, fret },
            NotePlace::Pitched(p) => w::NotePlace::Pitched(w::Pitch(p.0)),
        },
        tie: *tie,
        dynamic: dynamic_to(*dynamic),
        effects: w::NoteEffects {
            hammer_pull: *hammer_pull,
            slides: slides
                .iter()
                .map(|s| match s {
                    Slide::Shift => w::Slide::Shift,
                    Slide::Legato => w::Slide::Legato,
                    Slide::InFromBelow => w::Slide::InFromBelow,
                    Slide::InFromAbove => w::Slide::InFromAbove,
                    Slide::OutDown => w::Slide::OutDown,
                    Slide::OutUp => w::Slide::OutUp,
                })
                .collect(),
            bend: bend.as_ref().map(bend_to),
            harmonic: harmonic.map(|h| match h {
                Harmonic::Natural => w::Harmonic::Natural,
                Harmonic::Artificial { semitones } => w::Harmonic::Artificial { semitones },
                Harmonic::Tapped { fret } => w::Harmonic::Tapped { fret },
                Harmonic::Pinch => w::Harmonic::Pinch,
                Harmonic::Semi => w::Harmonic::Semi,
                Harmonic::Feedback => w::Harmonic::Feedback,
            }),
            vibrato: *vibrato,
            palm_mute: *palm_mute,
            let_ring: *let_ring,
            dead: *dead,
            ghost: *ghost,
            accent: accent.map(|a| match a {
                Accent::Normal => w::Accent::Normal,
                Accent::Heavy => w::Accent::Heavy,
            }),
            staccato: *staccato,
            trill: trill.map(|Trill { fret, speed }| w::Trill {
                fret,
                speed: value_to(speed),
            }),
            tremolo_picking: tremolo_picking.map(value_to),
            left_finger: *left_finger,
            right_finger: *right_finger,
            left_hand_tap: *left_hand_tap,
        },
    }
}

fn note_from(n: w::Note) -> Result<Note, String> {
    let w::Note {
        place,
        tie,
        dynamic,
        effects,
    } = n;
    let w::NoteEffects {
        hammer_pull,
        slides,
        bend,
        harmonic,
        vibrato,
        palm_mute,
        let_ring,
        dead,
        ghost,
        accent,
        staccato,
        trill,
        tremolo_picking,
        left_finger,
        right_finger,
        left_hand_tap,
    } = effects;
    Ok(Note {
        place: match place {
            w::NotePlace::Fretted { string, fret } => NotePlace::Fretted { string, fret },
            w::NotePlace::Pitched(p) => NotePlace::Pitched(Pitch(p.0)),
        },
        tie,
        dynamic: dynamic_from(dynamic),
        effects: NoteEffects {
            hammer_pull,
            slides: slides
                .into_iter()
                .map(|s| match s {
                    w::Slide::Shift => Slide::Shift,
                    w::Slide::Legato => Slide::Legato,
                    w::Slide::InFromBelow => Slide::InFromBelow,
                    w::Slide::InFromAbove => Slide::InFromAbove,
                    w::Slide::OutDown => Slide::OutDown,
                    w::Slide::OutUp => Slide::OutUp,
                })
                .collect(),
            bend: bend.map(bend_from).transpose()?,
            harmonic: harmonic.map(|h| match h {
                w::Harmonic::Natural => Harmonic::Natural,
                w::Harmonic::Artificial { semitones } => Harmonic::Artificial { semitones },
                w::Harmonic::Tapped { fret } => Harmonic::Tapped { fret },
                w::Harmonic::Pinch => Harmonic::Pinch,
                w::Harmonic::Semi => Harmonic::Semi,
                w::Harmonic::Feedback => Harmonic::Feedback,
            }),
            vibrato,
            palm_mute,
            let_ring,
            dead,
            ghost,
            accent: accent.map(|a| match a {
                w::Accent::Normal => Accent::Normal,
                w::Accent::Heavy => Accent::Heavy,
            }),
            staccato,
            trill: trill.map(|t| Trill {
                fret: t.fret,
                speed: value_from(t.speed),
            }),
            tremolo_picking: tremolo_picking.map(value_from),
            left_finger,
            right_finger,
            left_hand_tap,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_denominator_is_refused_and_fractions_are_reduced() {
        let mut score = Score::new();
        let mut bar = MasterBar::default();
        bar.tempo_changes.push(TempoChange {
            position: Ratio::new(1, 2).unwrap(),
            bpm: 90.0,
            label: String::new(),
            linear: false,
        });
        score.master_bars.push(bar);
        let mut wire = to_wire(&score);
        assert_eq!(from_wire(wire.clone()).unwrap(), score);
        wire.master_bars[0].tempo_changes[0].position = w::Ratio { num: 2, den: 4 };
        assert_eq!(
            from_wire(wire.clone()).unwrap(),
            score,
            "2/4 reduces to 1/2"
        );
        wire.master_bars[0].tempo_changes[0].position = w::Ratio { num: 1, den: 0 };
        assert_eq!(
            from_wire(wire).unwrap_err(),
            "bar 1: 1/0 is not a valid fraction"
        );
    }
}
