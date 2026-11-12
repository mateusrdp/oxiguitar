// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score as plugins see it, version 2 (interface 1.2).
//!
//! Version 1 ([`crate::score`]) is frozen: its structs are encoded field by
//! field, so none can gain a field. Version 2 has the same tree, and each of
//! its structs ends in a list of extensions (`ext`): an enum whose variants
//! are added only at its end, one per thing a later interface version adds
//! to that part of the score. So the score never needs a version 3: a new
//! notation adds a variant, and the host leaves out the variants a plugin's
//! interface version doesn't know before sending it a score
//! ([`Score2::keep_known`]). The leaves that hold no extension (a time
//! signature, a duration, a note's effects) are version 1's own types;
//! what is added to them goes in their parent's list.
//!
//! ```text
//! Score2                  + ScoreExt
//! ├── MasterBar2 × bars   + MasterBarExt   (FreeTime, since 1.2)
//! └── Track2 × tracks     + TrackExt
//!     └── Staff2          + StaffExt
//!         └── Bar2        + BarExt
//!             └── Voice2  + VoiceExt
//!                 └── Beat2   + BeatExt
//!                     └── Note2   + NoteExt
//! ```
//!
//! A score converts to version 1 ([`Score2::to_v1`]) for plugins built for
//! interface 1.0 and 1.1, leaving out every extension, and from version 1
//! ([`Score2::from_v1`]) with none.

use serde::{Deserialize, Serialize};

use crate::score::{
    BeatEffects, BeatKind, ChordDiagram, Clef, Color, Direction, Duration, Dynamic, GraceKind,
    Instrument, KeySignature, LyricLine, Marker, Mix, NoteEffects, NotePlace, Pitch, Ratio, Score,
    ScoreInfo, StrokeDirection, TempoChange, TimeSignature, TripletFeel,
};

/// The interface minor version that first knows an extension.
pub trait Since {
    /// The minor version of interface 1 that added this variant.
    fn since(&self) -> u32;
}

/// What an extension is called in a write's losses ("hairpins").
pub trait Named {
    /// Its name, plural where it counts marks.
    fn what(&self) -> &'static str;
}

/// What later interface versions add to the whole score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScoreExt {
    /// Bars per system on the page when several tracks are shown, from the
    /// first system on (Design Mode). Since 1.6.
    Layout(Vec<u8>),
}

impl Since for ScoreExt {
    fn since(&self) -> u32 {
        match self {
            ScoreExt::Layout(_) => 6,
        }
    }
}

impl Named for ScoreExt {
    fn what(&self) -> &'static str {
        match self {
            ScoreExt::Layout(_) => "the bars set for each system",
        }
    }
}

/// What later interface versions add to a master bar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MasterBarExt {
    /// The bar is in free time: it has no time signature to add up to
    /// (a cadenza, a recitative). Since 1.2.
    FreeTime,
    /// A fermata, holding every track at its moment in the bar. Since 1.6.
    Fermata(Fermata),
}

impl Since for MasterBarExt {
    fn since(&self) -> u32 {
        match self {
            MasterBarExt::FreeTime => 2,
            MasterBarExt::Fermata(_) => 6,
        }
    }
}

impl Named for MasterBarExt {
    fn what(&self) -> &'static str {
        match self {
            MasterBarExt::FreeTime => "free time",
            MasterBarExt::Fermata(_) => "fermatas",
        }
    }
}

/// A fermata (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fermata {
    /// Where in the bar, in whole notes from its start.
    pub position: Ratio,
    /// How long it holds.
    pub kind: FermataKind,
    /// Guitar Pro's length value, kept for the round trip.
    pub length: Ratio,
}

/// How long a fermata holds (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FermataKind {
    /// Short.
    Short,
    /// Medium, the usual one.
    Medium,
    /// Long.
    Long,
}

/// What later interface versions add to a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrackExt {
    /// Bars per system on the page when the track is shown alone, from
    /// the first system on (Design Mode). Since 1.6.
    Layout(Vec<u8>),
    /// Index of another track whose chord diagrams to show above this
    /// track's staves; u32::MAX for none. Since 1.7.
    ChordsFrom(u32),
}

impl Since for TrackExt {
    fn since(&self) -> u32 {
        match self {
            TrackExt::Layout(_) => 6,
            TrackExt::ChordsFrom(_) => 7,
        }
    }
}

impl Named for TrackExt {
    fn what(&self) -> &'static str {
        match self {
            TrackExt::Layout(_) => "the bars set for each system",
            TrackExt::ChordsFrom(_) => "the track whose chords are shown above this one",
        }
    }
}

/// What later interface versions add to a staff. None yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StaffExt {}

impl Since for StaffExt {
    fn since(&self) -> u32 {
        match *self {}
    }
}

impl Named for StaffExt {
    fn what(&self) -> &'static str {
        match *self {}
    }
}

/// What later interface versions add to a staff's bar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BarExt {
    /// A simile sign drawn in place of the bar's music, which is a copy of
    /// the bar or bars it repeats. Since 1.6.
    Simile(Simile),
}

impl Since for BarExt {
    fn since(&self) -> u32 {
        match self {
            BarExt::Simile(_) => 6,
        }
    }
}

impl Named for BarExt {
    fn what(&self) -> &'static str {
        match self {
            BarExt::Simile(_) => "repeat (simile) signs",
        }
    }
}

/// A simile sign (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Simile {
    /// The bar repeats the bar before.
    Simple,
    /// The first of two bars repeating the two before them.
    FirstOfDouble,
    /// The second of two bars repeating the two before them.
    SecondOfDouble,
}

/// What later interface versions add to a voice. None yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VoiceExt {}

impl Since for VoiceExt {
    fn since(&self) -> u32 {
        match *self {}
    }
}

impl Named for VoiceExt {
    fn what(&self) -> &'static str {
        match *self {}
    }
}

/// What later interface versions add to a beat. Marks that span several
/// beats (slurs, hairpins, ottavas, barres) are on each beat they cover,
/// as Guitar Pro stores them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeatExt {
    /// Slurred to the next beat of its voice. Since 1.6.
    Slur,
    /// Under a hairpin. Since 1.6.
    Hairpin(Hairpin),
    /// Under an ottava line. Since 1.6.
    Ottava(Ottava),
    /// Slash notation. Since 1.6.
    Slash,
    /// How its beam joins the next beat's, when not automatic. Since 1.6.
    Beaming(Beaming),
    /// Equal tuplets from here are bracketed as one group. Since 1.6.
    TupletGroup,
    /// Under an extended barre. Since 1.6.
    Barre {
        /// The fret, from the capo.
        fret: u8,
        /// A half barre.
        half: bool,
    },
    /// A fade out (version 1's `fade_in` holds a fade in). Since 1.6.
    FadeOut,
    /// A volume swell. Since 1.6.
    VolumeSwell,
    /// A slight vibrato with the whammy bar (version 1's `vibrato_wide`
    /// holds a wide one). Since 1.6.
    SlightBridgeVibrato,
    /// The sustain pedal goes down or up. Since 1.6.
    Pedal(Pedal),
    /// A wah-wah mark. Since 1.6.
    Wah(Wah),
    /// An arpeggio, up or down. Since 1.6.
    Arpeggio(StrokeDirection),
    /// A rasgueado. Since 1.6.
    Rasgueado(Rasgueado),
    /// A golpe. Since 1.6.
    Golpe(Golpe),
    /// A pick scrape, up (towards the neck) or down. Since 1.6.
    PickScrape(StrokeDirection),
    /// A dead slap. Since 1.6.
    DeadSlap,
}

impl Since for BeatExt {
    fn since(&self) -> u32 {
        6
    }
}

impl Named for BeatExt {
    fn what(&self) -> &'static str {
        match self {
            BeatExt::Slur => "slurs",
            BeatExt::Hairpin(_) => "hairpins",
            BeatExt::Ottava(_) => "ottava lines",
            BeatExt::Slash => "slash notation",
            BeatExt::Beaming(_) => "beaming changes",
            BeatExt::TupletGroup => "forced tuplet groups",
            BeatExt::Barre { .. } => "barres",
            BeatExt::FadeOut => "fade-outs",
            BeatExt::VolumeSwell => "volume swells",
            BeatExt::SlightBridgeVibrato => "slight bridge vibratos",
            BeatExt::Pedal(_) => "sustain pedal marks",
            BeatExt::Wah(_) => "wah-wah marks",
            BeatExt::Arpeggio(_) => "arpeggios",
            BeatExt::Rasgueado(_) => "rasgueados",
            BeatExt::Golpe(_) => "golpes",
            BeatExt::PickScrape(_) => "pick scrapes",
            BeatExt::DeadSlap => "dead slaps",
        }
    }
}

/// A hairpin (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hairpin {
    /// Crescendo.
    Crescendo,
    /// Decrescendo.
    Decrescendo,
}

/// An ottava line (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ottava {
    /// 8va.
    Alta8,
    /// 8vb.
    Bassa8,
    /// 15ma.
    Alta15,
    /// 15mb.
    Bassa15,
}

/// A beat's beaming, when not automatic (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Beaming {
    /// Beamed to the next beat.
    Force,
    /// Not beamed to the next beat.
    Break,
    /// Only the first beam continues to the next beat.
    BreakSecondary,
}

/// The sustain pedal (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pedal {
    /// Pressed.
    Down,
    /// Released.
    Up,
}

/// A wah-wah mark (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Wah {
    /// Open.
    Open,
    /// Closed.
    Closed,
}

/// A golpe (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Golpe {
    /// With a finger.
    Finger,
    /// With the thumb.
    Thumb,
}

/// Guitar Pro's rasgueado patterns (since 1.6), by their fingers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Rasgueado {
    Ii,
    Mi,
    MiiTriplet,
    MiiAnapaest,
    PmpTriplet,
    PmpAnapaest,
    PeiTriplet,
    PeiAnapaest,
    PaiTriplet,
    PaiAnapaest,
    AmiTriplet,
    AmiAnapaest,
    Ppp,
    Amii,
    Amip,
    Eami,
    Eamii,
    Peami,
}

/// What later interface versions add to a note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteExt {
    /// A wide left-hand vibrato (version 1's `vibrato` is also set).
    /// Since 1.6.
    WideVibrato,
    /// An ornament. Since 1.6.
    Ornament(Ornament),
    /// The string's number is shown. Since 1.6.
    StringNumber,
    /// How the note is spelled on the staff. Since 1.6.
    Spelling {
        /// The step: 0 for C … 6 for B.
        step: u8,
        /// Semitones from the step's natural, -2 to 2.
        alter: i8,
    },
}

impl Since for NoteExt {
    fn since(&self) -> u32 {
        6
    }
}

impl Named for NoteExt {
    fn what(&self) -> &'static str {
        match self {
            NoteExt::WideVibrato => "wide vibratos",
            NoteExt::Ornament(_) => "ornaments",
            NoteExt::StringNumber => "string numbers",
            NoteExt::Spelling { .. } => "chosen spellings",
        }
    }
}

/// An ornament (since 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ornament {
    /// Upper mordent.
    UpperMordent,
    /// Lower mordent.
    LowerMordent,
    /// Turn.
    Turn,
    /// Inverted turn.
    InvertedTurn,
}

/// A whole piece: version 1's [`Score`], with extensions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Score2 {
    /// Title, artist and the rest.
    pub info: ScoreInfo,
    /// Tempo at the start, in quarter notes per minute.
    pub initial_tempo: f64,
    /// The text shown with the initial tempo, such as "Moderato".
    pub tempo_label: String,
    /// What every track shares in each bar.
    pub master_bars: Vec<MasterBar2>,
    /// The tracks.
    pub tracks: Vec<Track2>,
    /// What later versions add.
    pub ext: Vec<ScoreExt>,
}

/// What every track shares in one bar: version 1's
/// [`MasterBar`](crate::score::MasterBar), with extensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MasterBar2 {
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
    /// What later versions add.
    pub ext: Vec<MasterBarExt>,
}

impl MasterBar2 {
    /// Whether the bar is in free time ([`MasterBarExt::FreeTime`]).
    pub fn free_time(&self) -> bool {
        self.ext.contains(&MasterBarExt::FreeTime)
    }
}

/// One instrument's part: version 1's [`Track`](crate::score::Track), with
/// extensions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track2 {
    /// The track's name.
    pub name: String,
    /// Its short name, for the start of each system after the first.
    pub short_name: String,
    /// Its colour.
    pub color: Color,
    /// The instrument that plays it.
    pub instrument: Instrument,
    /// Volume, pan, mute and solo.
    pub mix: Mix,
    /// Its staves, one or more.
    pub staves: Vec<Staff2>,
    /// The chord diagrams its beats name, by index.
    pub chords: Vec<ChordDiagram>,
    /// Its lyrics.
    pub lyrics: Vec<LyricLine>,
    /// What later versions add.
    pub ext: Vec<TrackExt>,
}

/// One staff of a track: version 1's [`Staff`](crate::score::Staff), with
/// extensions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Staff2 {
    /// Open-string pitches, highest string first; empty for a staff
    /// without strings.
    pub tuning: Vec<Pitch>,
    /// The capo's fret, 0 for none.
    pub capo: u8,
    /// How many frets the instrument has.
    pub fret_count: u8,
    /// Semitones the written part sits above the sounding one.
    pub transposition: i8,
    /// Standard notation is shown.
    pub show_standard: bool,
    /// Tablature is shown.
    pub show_tablature: bool,
    /// One bar for each of the score's bars.
    pub bars: Vec<Bar2>,
    /// What later versions add.
    pub ext: Vec<StaffExt>,
}

/// One bar of a staff: version 1's [`Bar`](crate::score::Bar), with
/// extensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar2 {
    /// The clef.
    pub clef: Clef,
    /// Its voices, one to four.
    pub voices: Vec<Voice2>,
    /// What later versions add.
    pub ext: Vec<BarExt>,
}

/// One voice of a bar: version 1's [`Voice`](crate::score::Voice), with
/// extensions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Voice2 {
    /// Its beats, in order.
    pub beats: Vec<Beat2>,
    /// What later versions add.
    pub ext: Vec<VoiceExt>,
}

/// One beat: version 1's [`Beat`](crate::score::Beat), with extensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beat2 {
    /// Notes, a rest, or an empty placeholder.
    pub kind: BeatKind,
    /// How long it lasts.
    pub duration: Duration,
    /// Its notes.
    pub notes: Vec<Note2>,
    /// A grace beat, before or on the beat.
    pub grace: Option<GraceKind>,
    /// What applies to the whole beat.
    pub effects: BeatEffects,
    /// The chord diagram it names, an index into its track's chords.
    pub chord: Option<u32>,
    /// Free text above it.
    pub text: String,
    /// What later versions add.
    pub ext: Vec<BeatExt>,
}

/// One note: version 1's [`Note`](crate::score::Note), with extensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note2 {
    /// Where it is: a string and fret, or a pitch.
    pub place: NotePlace,
    /// It continues the note before it on its string.
    pub tie: bool,
    /// How loud.
    pub dynamic: Dynamic,
    /// Its effects.
    pub effects: NoteEffects,
    /// What later versions add.
    pub ext: Vec<NoteExt>,
}

impl Score2 {
    /// Version 2 of a version-1 score, with no extensions.
    pub fn from_v1(score: Score) -> Score2 {
        use crate::score as v1;
        let Score {
            info,
            initial_tempo,
            tempo_label,
            master_bars,
            tracks,
        } = score;
        let note = |n: v1::Note| {
            let v1::Note {
                place,
                tie,
                dynamic,
                effects,
            } = n;
            Note2 {
                place,
                tie,
                dynamic,
                effects,
                ext: Vec::new(),
            }
        };
        let beat = |b: v1::Beat| {
            let v1::Beat {
                kind,
                duration,
                notes,
                grace,
                effects,
                chord,
                text,
            } = b;
            Beat2 {
                kind,
                duration,
                notes: notes.into_iter().map(note).collect(),
                grace,
                effects,
                chord,
                text,
                ext: Vec::new(),
            }
        };
        let bar = |b: v1::Bar| {
            let v1::Bar { clef, voices } = b;
            Bar2 {
                clef,
                voices: voices
                    .into_iter()
                    .map(|v| Voice2 {
                        beats: v.beats.into_iter().map(beat).collect(),
                        ext: Vec::new(),
                    })
                    .collect(),
                ext: Vec::new(),
            }
        };
        let staff = |s: v1::Staff| {
            let v1::Staff {
                tuning,
                capo,
                fret_count,
                transposition,
                show_standard,
                show_tablature,
                bars,
            } = s;
            Staff2 {
                tuning,
                capo,
                fret_count,
                transposition,
                show_standard,
                show_tablature,
                bars: bars.into_iter().map(bar).collect(),
                ext: Vec::new(),
            }
        };
        Score2 {
            info,
            initial_tempo,
            tempo_label,
            master_bars: master_bars
                .into_iter()
                .map(|m| {
                    let v1::MasterBar {
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
                    } = m;
                    MasterBar2 {
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
                        ext: Vec::new(),
                    }
                })
                .collect(),
            tracks: tracks
                .into_iter()
                .map(|t| {
                    let v1::Track {
                        name,
                        short_name,
                        color,
                        instrument,
                        mix,
                        staves,
                        chords,
                        lyrics,
                    } = t;
                    Track2 {
                        name,
                        short_name,
                        color,
                        instrument,
                        mix,
                        staves: staves.into_iter().map(staff).collect(),
                        chords,
                        lyrics,
                        ext: Vec::new(),
                    }
                })
                .collect(),
            ext: Vec::new(),
        }
    }

    /// Version 1 of this score, for a plugin built for interface 1.0 or
    /// 1.1: every extension left out.
    pub fn to_v1(&self) -> Score {
        use crate::score as v1;
        let note = |n: &Note2| v1::Note {
            place: n.place,
            tie: n.tie,
            dynamic: n.dynamic,
            effects: n.effects.clone(),
        };
        let beat = |b: &Beat2| v1::Beat {
            kind: b.kind,
            duration: b.duration.clone(),
            notes: b.notes.iter().map(note).collect(),
            grace: b.grace,
            effects: b.effects.clone(),
            chord: b.chord,
            text: b.text.clone(),
        };
        let bar = |b: &Bar2| v1::Bar {
            clef: b.clef,
            voices: b
                .voices
                .iter()
                .map(|v| v1::Voice {
                    beats: v.beats.iter().map(beat).collect(),
                })
                .collect(),
        };
        Score {
            info: self.info.clone(),
            initial_tempo: self.initial_tempo,
            tempo_label: self.tempo_label.clone(),
            master_bars: self
                .master_bars
                .iter()
                .map(|m| v1::MasterBar {
                    time_signature: m.time_signature,
                    key_signature: m.key_signature,
                    repeat_start: m.repeat_start,
                    repeat_end: m.repeat_end,
                    alternate_endings: m.alternate_endings,
                    marker: m.marker.clone(),
                    double_bar: m.double_bar,
                    triplet_feel: m.triplet_feel,
                    tempo_changes: m.tempo_changes.clone(),
                    directions: m.directions.clone(),
                })
                .collect(),
            tracks: self
                .tracks
                .iter()
                .map(|t| v1::Track {
                    name: t.name.clone(),
                    short_name: t.short_name.clone(),
                    color: t.color,
                    instrument: t.instrument,
                    mix: t.mix,
                    staves: t
                        .staves
                        .iter()
                        .map(|s| v1::Staff {
                            tuning: s.tuning.clone(),
                            capo: s.capo,
                            fret_count: s.fret_count,
                            transposition: s.transposition,
                            show_standard: s.show_standard,
                            show_tablature: s.show_tablature,
                            bars: s.bars.iter().map(bar).collect(),
                        })
                        .collect(),
                    chords: t.chords.clone(),
                    lyrics: t.lyrics.clone(),
                })
                .collect(),
        }
    }

    /// What [`Score2::to_v1`] leaves out, as (what, first bar from 1, how
    /// many times), for the losses of a plugin built for an earlier
    /// interface version.
    pub fn extensions_used(&self) -> Vec<(&'static str, usize, usize)> {
        self.extensions_newer_than(0)
    }

    /// What [`Score2::keep_known`] with `minor` leaves out, as (what, first
    /// bar from 1, how many times), for the losses of a plugin built for
    /// interface 1.`minor`.
    pub fn extensions_newer_than(&self, minor: u32) -> Vec<(&'static str, usize, usize)> {
        fn count<T: Since + Named>(
            out: &mut Vec<(&'static str, usize, usize)>,
            ext: &[T],
            bar: usize,
            minor: u32,
        ) {
            for e in ext.iter().filter(|e| e.since() > minor) {
                let what = e.what();
                match out.iter_mut().find(|o| o.0 == what) {
                    Some(o) => {
                        o.1 = o.1.min(bar);
                        o.2 += 1;
                    }
                    None => out.push((what, bar, 1)),
                }
            }
        }
        let mut out: Vec<(&'static str, usize, usize)> = Vec::new();
        count(&mut out, &self.ext, 1, minor);
        for (i, m) in self.master_bars.iter().enumerate() {
            count(&mut out, &m.ext, i + 1, minor);
        }
        for t in &self.tracks {
            count(&mut out, &t.ext, 1, minor);
            for s in &t.staves {
                count(&mut out, &s.ext, 1, minor);
                for (i, b) in s.bars.iter().enumerate() {
                    count(&mut out, &b.ext, i + 1, minor);
                    for v in &b.voices {
                        count(&mut out, &v.ext, i + 1, minor);
                        for beat in &v.beats {
                            count(&mut out, &beat.ext, i + 1, minor);
                            for n in &beat.notes {
                                count(&mut out, &n.ext, i + 1, minor);
                            }
                        }
                    }
                }
            }
        }
        // The first bar each kind is in, whatever order they were found.
        out.sort_by_key(|o| o.1);
        out
    }

    /// Leaves out every extension that interface 1.`minor` doesn't know,
    /// so a plugin built for it can read the score.
    pub fn keep_known(&mut self, minor: u32) {
        fn keep<T: Since>(ext: &mut Vec<T>, minor: u32) {
            ext.retain(|e| e.since() <= minor);
        }
        keep(&mut self.ext, minor);
        for m in &mut self.master_bars {
            keep(&mut m.ext, minor);
        }
        for t in &mut self.tracks {
            keep(&mut t.ext, minor);
            for s in &mut t.staves {
                keep(&mut s.ext, minor);
                for b in &mut s.bars {
                    keep(&mut b.ext, minor);
                    for v in &mut b.voices {
                        keep(&mut v.ext, minor);
                        for beat in &mut v.beats {
                            keep(&mut beat.ext, minor);
                            for n in &mut beat.notes {
                                keep(&mut n.ext, minor);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{KeyMode, NoteValue};

    fn small() -> Score2 {
        let v1 = Score {
            initial_tempo: 90.0,
            master_bars: vec![crate::score::MasterBar {
                time_signature: TimeSignature {
                    numerator: 3,
                    denominator: 4,
                },
                key_signature: KeySignature {
                    fifths: 1,
                    mode: KeyMode::Major,
                },
                repeat_start: false,
                repeat_end: None,
                alternate_endings: 0,
                marker: None,
                double_bar: false,
                triplet_feel: TripletFeel::None,
                tempo_changes: vec![],
                directions: vec![],
            }],
            tracks: vec![crate::score::Track {
                staves: vec![crate::score::Staff {
                    bars: vec![crate::score::Bar {
                        clef: Clef::Treble,
                        voices: vec![crate::score::Voice {
                            beats: vec![crate::score::Beat {
                                kind: BeatKind::Rest,
                                duration: Duration {
                                    value: NoteValue::Half,
                                    dots: 1,
                                    tuplets: vec![],
                                },
                                notes: vec![],
                                grace: None,
                                effects: BeatEffects::default(),
                                chord: None,
                                text: String::new(),
                            }],
                        }],
                    }],
                    ..crate::score::Staff::default()
                }],
                ..crate::score::Track::default()
            }],
            ..Score::default()
        };
        Score2::from_v1(v1)
    }

    #[test]
    fn version_1_round_trips_and_extensions_are_left_out() {
        let mut s = small();
        let v1 = s.to_v1();
        assert_eq!(Score2::from_v1(v1.clone()), s);
        s.master_bars[0].ext.push(MasterBarExt::FreeTime);
        assert!(s.master_bars[0].free_time());
        assert_eq!(s.to_v1(), v1);
        assert_eq!(s.extensions_used(), vec![("free time", 1, 1)]);
        let mut for_1_1 = s.clone();
        for_1_1.keep_known(1);
        assert!(!for_1_1.master_bars[0].free_time());
        s.keep_known(2);
        assert!(s.master_bars[0].free_time());
    }

    #[test]
    fn extensions_encode_at_their_place() {
        let mut s = small();
        s.master_bars[0].ext.push(MasterBarExt::FreeTime);
        let bytes = crate::encode(&s).unwrap();
        assert_eq!(crate::decode::<Score2>(&bytes).unwrap(), s);
        // FreeTime is the first variant: frozen at 0.
        assert_eq!(crate::encode(&MasterBarExt::FreeTime).unwrap(), [0]);
    }
}
