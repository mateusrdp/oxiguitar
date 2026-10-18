// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score as plugins see it (version 1).
//!
//! These types mirror the core's score model (`oxiguitar-model`, ADR 0002)
//! field for field, but they belong to the interface: the model can change
//! as the editor grows, and these change only through the interface's
//! versions (a later version would add a `Score2` message beside this one,
//! never change these types). The core converts between the two; a plugin
//! builds or reads these directly.
//!
//! ```text
//! Score
//! ├── ScoreInfo              title, artist, …
//! ├── MasterBar × bars       time and key signature, repeats, tempo, markers
//! └── Track × tracks         name, instrument, mixer, chord diagrams, lyrics
//!     └── Staff              tuning, capo, notation/tab visibility
//!         └── Bar × bars     clef
//!             └── Voice ×1–4
//!                 └── Beat   duration, notes or rest, beat effects
//!                     └── Note   string and fret (or pitch), note effects
//! ```
//!
//! Musical time is exact: positions and lengths are [`Ratio`]s of a whole
//! note. Strings are numbered from 1, the highest; pitches are MIDI note
//! numbers (60 = middle C).

use serde::{Deserialize, Serialize};

/// A fraction of a whole note: `num / den`, with `den` > 0. The core
/// reduces it when it converts; a zero `den` is rejected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ratio {
    /// Numerator.
    pub num: i64,
    /// Denominator, greater than zero.
    pub den: i64,
}

/// A whole piece: its metadata, the bars every track shares and the tracks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// Title, artist and the rest.
    pub info: ScoreInfo,
    /// Tempo at the start, in quarter notes per minute.
    pub initial_tempo: f64,
    /// The text shown with the initial tempo, such as "Moderato".
    pub tempo_label: String,
    /// What every track shares in each bar.
    pub master_bars: Vec<MasterBar>,
    /// The tracks.
    pub tracks: Vec<Track>,
}

/// The score's metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreInfo {
    /// Title.
    pub title: String,
    /// Subtitle.
    pub subtitle: String,
    /// Artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Who wrote the words.
    pub words: String,
    /// Who wrote the music.
    pub music: String,
    /// Copyright notice.
    pub copyright: String,
    /// Who transcribed it.
    pub transcriber: String,
    /// Performance instructions.
    pub instructions: String,
    /// Further notices, one per line.
    pub notices: Vec<String>,
}

/// What every track shares in one bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MasterBar {
    /// The time signature.
    pub time_signature: TimeSignature,
    /// The key signature.
    pub key_signature: KeySignature,
    /// A repeat starts at this bar.
    pub repeat_start: bool,
    /// A repeat ends at this bar, played this many times in all.
    pub repeat_end: Option<u8>,
    /// The endings this bar belongs to: bit 0 = first ending, and so on.
    pub alternate_endings: u16,
    /// A section marker.
    pub marker: Option<Marker>,
    /// A double bar line at the end.
    pub double_bar: bool,
    /// The bar's triplet feel.
    pub triplet_feel: TripletFeel,
    /// Tempo changes within the bar, in order.
    pub tempo_changes: Vec<TempoChange>,
    /// Navigation marks: segno, coda, D.C., D.S. and the rest.
    pub directions: Vec<Direction>,
}

/// A time signature, such as 6/8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    /// Beats per bar.
    pub numerator: u8,
    /// The note value of a beat (4 = quarter note).
    pub denominator: u8,
}

/// A key signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeySignature {
    /// Sharps (positive) or flats (negative), -7 to 7.
    pub fifths: i8,
    /// Major or minor.
    pub mode: KeyMode,
}

/// Major or minor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyMode {
    /// Major.
    Major,
    /// Minor.
    Minor,
}

/// A section marker, such as "A Verse".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// The section letter, such as "A".
    pub letter: String,
    /// The section's text, such as "Verse".
    pub text: String,
    /// Its colour.
    pub color: Color,
}

/// An RGB colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// How evenly written notes are played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TripletFeel {
    /// As written.
    None,
    /// Eighth notes as triplets.
    Triplet8th,
    /// Sixteenth notes as triplets.
    Triplet16th,
    /// Eighth notes dotted.
    Dotted8th,
    /// Sixteenth notes dotted.
    Dotted16th,
    /// Eighth notes as Scottish snaps.
    Scottish8th,
    /// Sixteenth notes as Scottish snaps.
    Scottish16th,
}

/// A tempo change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoChange {
    /// Where in the bar, from its start.
    pub position: Ratio,
    /// The new tempo, in quarter notes per minute.
    pub bpm: f64,
    /// The text shown with it.
    pub label: String,
    /// Reached gradually from the previous tempo.
    pub linear: bool,
}

/// A navigation mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Direction {
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
}

/// A track: one instrument with its staves.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Name.
    pub name: String,
    /// Short name, shown at the start of later systems.
    pub short_name: String,
    /// Colour.
    pub color: Color,
    /// The General MIDI instrument.
    pub instrument: Instrument,
    /// Volume, pan, mute and solo.
    pub mix: Mix,
    /// The staves (two for a grand staff), each with its bars.
    pub staves: Vec<Staff>,
    /// Chord diagrams, referred to by beats by index.
    pub chords: Vec<ChordDiagram>,
    /// Lyrics.
    pub lyrics: Vec<LyricLine>,
}

/// A General MIDI instrument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    /// The program, 0–127.
    pub midi_program: u8,
    /// A drum kit.
    pub percussion: bool,
}

/// A track's mixer settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mix {
    /// Volume, 0 to 1.
    pub volume: f32,
    /// Pan, -1 (left) to 1 (right).
    pub pan: f32,
    /// Muted.
    pub mute: bool,
    /// Soloed.
    pub solo: bool,
}

/// A staff with its bars.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Staff {
    /// Open-string pitches, from string 1 (the highest) down; empty for
    /// instruments without strings.
    pub tuning: Vec<Pitch>,
    /// Capo fret.
    pub capo: u8,
    /// Number of frets.
    pub fret_count: u8,
    /// Written transposition in semitones.
    pub transposition: i8,
    /// Standard notation shown.
    pub show_standard: bool,
    /// Tablature shown.
    pub show_tablature: bool,
    /// The bars, one for each master bar.
    pub bars: Vec<Bar>,
}

/// A chord diagram.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordDiagram {
    /// The chord's name.
    pub name: String,
    /// The fret the diagram starts at.
    pub first_fret: u8,
    /// The fret on each string from string 1; `None` = not played.
    pub frets: Vec<Option<u8>>,
}

/// A line of lyrics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricLine {
    /// The bar it starts at, from 0.
    pub start_bar: u32,
    /// The words, one syllable per note.
    pub text: String,
}

/// A MIDI note number (60 = middle C).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pitch(pub u8);

/// One staff's content in one bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// The clef.
    pub clef: Clef,
    /// The voices, 1 to 4.
    pub voices: Vec<Voice>,
}

/// A clef.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Clef {
    Treble,
    Treble8vb,
    Bass,
    Bass8vb,
    Alto,
    Tenor,
    Neutral,
}

/// A voice: a sequence of beats.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    /// The beats, in order.
    pub beats: Vec<Beat>,
}

/// What a beat holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BeatKind {
    /// Notes (a chord when several).
    Notes,
    /// A rest.
    Rest,
    /// Nothing, not even a rest: an unused voice.
    Empty,
}

/// A beat: notes or a rest, with one duration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beat {
    /// What it holds.
    pub kind: BeatKind,
    /// How long it lasts.
    pub duration: Duration,
    /// The notes.
    pub notes: Vec<Note>,
    /// A grace beat, and how it is placed.
    pub grace: Option<GraceKind>,
    /// Effects on the whole beat.
    pub effects: BeatEffects,
    /// The track's chord diagram shown over it, by index.
    pub chord: Option<u32>,
    /// Text shown with it.
    pub text: String,
}

/// How a grace beat is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraceKind {
    /// Before the beat, taking time from the previous one.
    BeforeBeat,
    /// On the beat, taking time from it.
    OnBeat,
}

/// Effects on a whole beat.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BeatEffects {
    /// A strum.
    pub stroke: Option<Stroke>,
    /// The pick's direction.
    pub pick_stroke: Option<StrokeDirection>,
    /// A whammy-bar curve.
    pub whammy: Option<Bend>,
    /// Tap, slap or pop.
    pub technique: Option<BeatTechnique>,
    /// Fade in.
    pub fade_in: bool,
    /// Wide vibrato.
    pub vibrato_wide: bool,
    /// Tremolo picking at this note value.
    pub tremolo_picking: Option<NoteValue>,
    /// A dynamic marking.
    pub dynamic: Option<Dynamic>,
    /// Let ring.
    pub let_ring: bool,
}

/// A strum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stroke {
    /// Its direction.
    pub direction: StrokeDirection,
    /// How fast, as a note value.
    pub speed: NoteValue,
}

/// Down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum StrokeDirection {
    Down,
    Up,
}

/// A right-hand technique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum BeatTechnique {
    Tap,
    Slap,
    Pop,
}

/// A written note value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum NoteValue {
    DoubleWhole,
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
    OneTwentyEighth,
    TwoFiftySixth,
}

/// A tuplet: `notes` in the time of `in_time_of`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tuplet {
    /// Notes played.
    pub notes: u8,
    /// In the time of this many.
    pub in_time_of: u8,
}

/// A beat's duration.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Duration {
    /// The note value.
    pub value: NoteValue,
    /// Dots.
    pub dots: u8,
    /// Tuplets, the outermost first.
    pub tuplets: Vec<Tuplet>,
}

/// Where a note is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotePlace {
    /// On a string and fret (strings from 1, the highest).
    Fretted {
        /// The string.
        string: u8,
        /// The fret.
        fret: u8,
    },
    /// At a pitch, on an instrument without strings.
    Pitched(Pitch),
}

/// A note.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// Where it is played.
    pub place: NotePlace,
    /// Tied to the previous note on the same string or pitch.
    pub tie: bool,
    /// How loud.
    pub dynamic: Dynamic,
    /// Its effects.
    pub effects: NoteEffects,
}

/// A dynamic, from softest to loudest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Dynamic {
    PPP,
    PP,
    P,
    MP,
    MF,
    F,
    FF,
    FFF,
}

/// A note's effects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteEffects {
    /// Hammer-on or pull-off to the next note.
    pub hammer_pull: bool,
    /// Slides.
    pub slides: Vec<Slide>,
    /// A bend.
    pub bend: Option<Bend>,
    /// A harmonic.
    pub harmonic: Option<Harmonic>,
    /// Vibrato.
    pub vibrato: bool,
    /// Palm mute.
    pub palm_mute: bool,
    /// Let ring.
    pub let_ring: bool,
    /// A dead (muted) note.
    pub dead: bool,
    /// A ghost note.
    pub ghost: bool,
    /// An accent.
    pub accent: Option<Accent>,
    /// Staccato.
    pub staccato: bool,
    /// A trill.
    pub trill: Option<Trill>,
    /// Tremolo picking at this note value.
    pub tremolo_picking: Option<NoteValue>,
    /// Left-hand finger, 1 (index) to 5.
    pub left_finger: Option<u8>,
    /// Right-hand finger, 1 (thumb) to 5.
    pub right_finger: Option<u8>,
    /// Tapped with the left hand.
    pub left_hand_tap: bool,
}

/// An accent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Accent {
    /// Accent (>).
    Normal,
    /// Heavy accent (^).
    Heavy,
}

/// A slide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slide {
    /// To the next note, picking it.
    Shift,
    /// To the next note, without picking it.
    Legato,
    /// Into the note from below.
    InFromBelow,
    /// Into the note from above.
    InFromAbove,
    /// Out of the note, downwards.
    OutDown,
    /// Out of the note, upwards.
    OutUp,
}

/// A bend or whammy curve.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bend {
    /// Its points, in order.
    pub points: Vec<BendPoint>,
}

/// A point of a bend curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BendPoint {
    /// Where in the note, from 0 (its start) to 1 (its end).
    pub offset: Ratio,
    /// How far, in cents (100 = a semitone).
    pub cents: i32,
}

/// A harmonic.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Harmonic {
    /// Natural.
    Natural,
    /// Artificial, this many semitones above the fretted note.
    Artificial {
        /// Semitones above.
        semitones: u8,
    },
    /// Tapped at a fret.
    Tapped {
        /// The fret.
        fret: u8,
    },
    /// Pinch.
    Pinch,
    /// Semi.
    Semi,
    /// Feedback.
    Feedback,
}

/// A trill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Trill {
    /// The other fret.
    pub fret: u8,
    /// How fast, as a note value.
    pub speed: NoteValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small score whose encoding is pinned below. If this test fails,
    /// a change altered version 1 of the score as it crosses the
    /// interface, which would break every built format plugin: put the
    /// change in a new version instead.
    #[test]
    fn the_encoding_of_version_1_is_frozen() {
        let note = Note {
            place: NotePlace::Fretted { string: 1, fret: 3 },
            tie: false,
            dynamic: Dynamic::F,
            effects: NoteEffects {
                bend: Some(Bend {
                    points: vec![BendPoint {
                        offset: Ratio { num: 1, den: 2 },
                        cents: 100,
                    }],
                }),
                ..NoteEffects::default()
            },
        };
        let beat = Beat {
            kind: BeatKind::Notes,
            duration: Duration {
                value: NoteValue::Quarter,
                dots: 1,
                tuplets: vec![Tuplet {
                    notes: 3,
                    in_time_of: 2,
                }],
            },
            notes: vec![note],
            grace: None,
            effects: BeatEffects::default(),
            chord: Some(0),
            text: "x".into(),
        };
        let score = Score {
            info: ScoreInfo {
                title: "T".into(),
                ..ScoreInfo::default()
            },
            initial_tempo: 120.0,
            tempo_label: String::new(),
            master_bars: vec![MasterBar {
                time_signature: TimeSignature {
                    numerator: 4,
                    denominator: 4,
                },
                key_signature: KeySignature {
                    fifths: -1,
                    mode: KeyMode::Minor,
                },
                repeat_start: true,
                repeat_end: Some(2),
                alternate_endings: 1,
                marker: None,
                double_bar: false,
                triplet_feel: TripletFeel::None,
                tempo_changes: vec![],
                directions: vec![Direction::Fine],
            }],
            tracks: vec![Track {
                name: "G".into(),
                instrument: Instrument {
                    midi_program: 25,
                    percussion: false,
                },
                staves: vec![Staff {
                    tuning: vec![Pitch(64)],
                    fret_count: 24,
                    show_tablature: true,
                    bars: vec![Bar {
                        clef: Clef::Treble8vb,
                        voices: vec![Voice { beats: vec![beat] }],
                    }],
                    ..Staff::default()
                }],
                ..Track::default()
            }],
        };
        let bytes = crate::encode(&score).unwrap();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "01540000000000000000000000000000005e40000104\
             04ff0101010201000000000104010147000000001900\
             00000000000000000000010140001800000101010101\
             000301010302010001030005000001010204c8010000\
             00000000000000000000000000000000000000000001\
             0001780000"
        );
        assert_eq!(crate::decode::<Score>(&bytes).unwrap(), score);
    }
}
