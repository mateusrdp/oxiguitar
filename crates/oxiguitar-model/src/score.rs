// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score, its header information and the bar layout shared by all tracks.

use crate::{Ratio, Track};

/// A whole piece of music: header information, the bar structure shared by
/// every track, and the tracks themselves.
///
/// Bars are split in two, as in Guitar Pro and TuxGuitar: [`MasterBar`] holds
/// what every track shares (time signature, repeats, tempo, sections), and
/// each track's [`Staff`](crate::Staff) holds one [`Bar`](crate::Bar) of
/// notes per master bar. A valid score has exactly as many bars in every
/// staff as it has master bars.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    /// Title, artist and the other header texts.
    pub info: ScoreInfo,
    /// Tempo at the start, in quarter notes per minute.
    pub initial_tempo: f64,
    /// Text shown with the initial tempo, such as "Moderate rock".
    pub tempo_label: String,
    /// The bar structure shared by every track.
    pub master_bars: Vec<MasterBar>,
    /// The tracks, in display order.
    pub tracks: Vec<Track>,
}

impl Score {
    /// An empty score with a tempo of 120.
    pub fn new() -> Score {
        Score {
            initial_tempo: 120.0,
            ..Score::default()
        }
    }

    /// Number of bars.
    pub fn bar_count(&self) -> usize {
        self.master_bars.len()
    }

    /// Checks the invariants every reader and edit must keep: each staff has one
    /// bar per master bar, and each staff has at least one voice per bar.
    /// Returns a description of the first problem found.
    pub fn check_structure(&self) -> Result<(), String> {
        let bars = self.master_bars.len();
        for (ti, track) in self.tracks.iter().enumerate() {
            if track.staves.is_empty() {
                return Err(format!("track {} has no staff", ti + 1));
            }
            for (si, staff) in track.staves.iter().enumerate() {
                if staff.bars.len() != bars {
                    return Err(format!(
                        "track {} staff {} has {} bars, expected {}",
                        ti + 1,
                        si + 1,
                        staff.bars.len(),
                        bars
                    ));
                }
                if let Some(bi) = staff.bars.iter().position(|b| b.voices.is_empty()) {
                    return Err(format!(
                        "track {} staff {} bar {} has no voice",
                        ti + 1,
                        si + 1,
                        bi + 1
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Header texts of a score. Empty strings mean "not set".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScoreInfo {
    /// Title.
    pub title: String,
    /// Subtitle.
    pub subtitle: String,
    /// Performing artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Author of the words.
    pub words: String,
    /// Composer.
    pub music: String,
    /// Copyright line.
    pub copyright: String,
    /// Who made this tablature or transcription.
    pub transcriber: String,
    /// Playing instructions.
    pub instructions: String,
    /// Free notes, one entry per line.
    pub notices: Vec<String>,
}

/// What every track shares in one bar.
#[derive(Clone, Debug, PartialEq)]
pub struct MasterBar {
    /// Time signature in force in this bar.
    pub time_signature: TimeSignature,
    /// Key signature in force in this bar.
    pub key_signature: KeySignature,
    /// A repeat sign opens here.
    pub repeat_start: bool,
    /// A repeat sign closes here; the section plays this many times in all
    /// (2 for an ordinary repeat). `None` if there is no closing repeat.
    pub repeat_end: Option<u8>,
    /// Alternate ending numbers this bar belongs to, as a bit set:
    /// bit 0 is ending 1, bit 1 ending 2, and so on.
    pub alternate_endings: u16,
    /// Section marker (rehearsal mark), if any.
    pub marker: Option<Marker>,
    /// Double bar line at the end.
    pub double_bar: bool,
    /// Swing or triplet feel applied from this bar.
    pub triplet_feel: TripletFeel,
    /// Tempo changes inside this bar.
    pub tempo_changes: Vec<TempoChange>,
    /// Navigation signs and jumps: segno, coda, D.S., D.C., fine.
    pub directions: Vec<Direction>,
}

impl Default for MasterBar {
    fn default() -> Self {
        MasterBar {
            time_signature: TimeSignature::COMMON,
            key_signature: KeySignature::default(),
            repeat_start: false,
            repeat_end: None,
            alternate_endings: 0,
            marker: None,
            double_bar: false,
            triplet_feel: TripletFeel::None,
            tempo_changes: Vec::new(),
            directions: Vec::new(),
        }
    }
}

impl MasterBar {
    /// The length this bar should have, in whole notes.
    pub fn expected_length(&self) -> Option<Ratio> {
        self.time_signature.bar_length()
    }
}

/// A time signature such as 4/4 or 7/8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeSignature {
    /// Beats per bar (top number).
    pub numerator: u8,
    /// Beat unit (bottom number): 4 for quarter notes.
    pub denominator: u8,
}

impl TimeSignature {
    /// 4/4.
    pub const COMMON: TimeSignature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };

    /// The length of a full bar in whole notes, or `None` for a zero denominator.
    pub fn bar_length(self) -> Option<Ratio> {
        Ratio::new(i64::from(self.numerator), i64::from(self.denominator))
    }
}

/// A key signature as a position on the circle of fifths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeySignature {
    /// Sharps (positive) or flats (negative), from -7 to 7.
    pub fifths: i8,
    /// Major or minor.
    pub mode: KeyMode,
}

/// Major or minor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyMode {
    /// Major key.
    #[default]
    Major,
    /// Minor key.
    Minor,
}

/// A section marker, such as "Verse" or "Solo".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// The text.
    pub text: String,
    /// Display colour.
    pub color: Color,
}

/// An RGB colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// Swing and triplet feel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TripletFeel {
    /// Played as written.
    #[default]
    None,
    /// Eighth notes swung as triplets.
    Triplet8th,
    /// Sixteenth notes swung as triplets.
    Triplet16th,
    /// Dotted eighth and sixteenth.
    Dotted8th,
    /// Dotted sixteenth and thirty-second.
    Dotted16th,
    /// Scottish snap on eighths.
    Scottish8th,
    /// Scottish snap on sixteenths.
    Scottish16th,
}

/// A tempo change at a point inside a bar.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoChange {
    /// Where it happens, in whole notes from the start of the bar.
    pub position: Ratio,
    /// New tempo in quarter notes per minute.
    pub bpm: f64,
    /// Optional text, such as "a tempo".
    pub label: String,
    /// If true, the tempo moves gradually to the next change instead of jumping.
    pub linear: bool,
}

/// Navigation signs and jumps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Segno sign.
    Segno,
    /// Double segno sign.
    SegnoSegno,
    /// Coda sign.
    Coda,
    /// Double coda sign.
    DoubleCoda,
    /// Fine.
    Fine,
    /// "To coda".
    ToCoda,
    /// "To double coda".
    ToDoubleCoda,
    /// Da capo.
    DaCapo,
    /// Da capo al coda.
    DaCapoAlCoda,
    /// Da capo al double coda.
    DaCapoAlDoubleCoda,
    /// Da capo al fine.
    DaCapoAlFine,
    /// Dal segno.
    DalSegno,
    /// Dal segno al coda.
    DalSegnoAlCoda,
    /// Dal segno al double coda.
    DalSegnoAlDoubleCoda,
    /// Dal segno al fine.
    DalSegnoAlFine,
    /// Dal segno segno.
    DalSegnoSegno,
    /// Dal segno segno al coda.
    DalSegnoSegnoAlCoda,
    /// Dal segno segno al double coda.
    DalSegnoSegnoAlDoubleCoda,
    /// Dal segno segno al fine.
    DalSegnoSegnoAlFine,
}
