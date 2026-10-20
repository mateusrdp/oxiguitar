// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writer for Guitar Pro 7 and 8 files (.gp): a ZIP archive holding the
//! score as GPIF XML, in the shape Guitar Pro 7.6 saves it.
//!
//! It is the reader in reverse (`super`): everything the reader takes from a
//! file, the writer puts there, so a score read from a .gp file and written
//! again reads back the same. What a .gp file can't hold, or holds only one
//! way, is named in the losses (FMT-3), each with the first place it
//! happens; a score with no losses reads back as it was written, apart from
//! details nobody sees (an unused voice's placeholder beats, the order of a
//! note's slides).
//!
//! Guitar Pro needs more than the reader does: every track has a sound from
//! Guitar Pro's library ([`tables`]; without one Guitar Pro 7.6 crashed on a
//! voice track), and every chord diagram its chord spelled out ([`chord`];
//! Guitar Pro 7.6 drops a diagram without). The file claims no particular
//! version of Guitar Pro: `<GPVersion>7</GPVersion>`, the first that reads it.

mod chord;
mod tables;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;

use oxiguitar_model::{
    Accent, Bar, Beat, BeatKind, BeatTechnique, BendPoint, ChordDiagram, Clef, Direction, Duration,
    Dynamic, GraceKind, Harmonic, KeyMode, MasterBar, Note, NotePlace, NoteValue, Pitch, Ratio,
    Score, Slide, Staff, StrokeDirection, Track, TripletFeel,
};

use super::TICKS_PER_QUARTER;
use crate::error::ImportLog;

/// A written .gp file and what it couldn't hold.
pub(crate) struct GpWritten {
    pub(crate) data: Vec<u8>,
    pub(crate) losses: ImportLog,
}

/// Writes `score` as a .gp file. Fails only for a score with no tracks or
/// bars, or whose staves don't all have a bar for every bar of the score.
pub(crate) fn write_gp(score: &Score) -> Result<GpWritten, String> {
    if score.tracks.is_empty() {
        return Err("the score has no tracks".into());
    }
    if score.master_bars.is_empty() {
        return Err("the score has no bars".into());
    }
    score.check_structure()?;
    let mut w = Writer::new(score);
    let gpif = w.document();
    let part = part_configuration(&w.plans);
    let data = pack(&gpif, &part).map_err(|e| format!("could not pack the file: {e}"))?;
    Ok(GpWritten {
        data,
        losses: w.losses,
    })
}

/// The .gp archive: `VERSION`, the score and the notation shown per track.
fn pack(gpif: &str, part_configuration: &[u8]) -> zip::result::ZipResult<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    // A fixed date: the same score always gives the same file.
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for (name, data) in [
        ("VERSION", "7.0".as_bytes()),
        ("Content/score.gpif", gpif.as_bytes()),
        ("Content/PartConfiguration", part_configuration),
    ] {
        zip.start_file(name, opts)?;
        zip.write_all(data)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// `PartConfiguration`: one score view, with one notation byte per track
/// (1 standard notation, 2 tablature).
fn part_configuration(plans: &[TrackPlan]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 + plans.len());
    v.extend_from_slice(&1u32.to_be_bytes());
    v.push(0);
    v.extend_from_slice(&u32::try_from(plans.len()).unwrap_or(0).to_be_bytes());
    v.extend(plans.iter().map(|p| p.notation));
    v
}

/// How a track is written, worked out before its bars are.
struct TrackPlan {
    percussion: bool,
    sound: tables::Sound,
    /// The instrument set's name and type (`<InstrumentSet>`): the sound's,
    /// or an electric guitar's for an instrument without frets that has
    /// strings and shows no tablature, so the reader keeps its strings.
    set: (&'static str, &'static str),
    /// -1 when the track is written an octave above its sound (guitar,
    /// bass: the treble 8vb clef), else 0.
    octave_shift: i32,
    /// `<Transpose>` in semitones: chromatic plus 12 × octave.
    transpose: i32,
    /// The notation byte of `PartConfiguration`.
    notation: u8,
    channel: u8,
    /// Each staff's open strings, lowest first, and capo.
    staves: Vec<(Vec<u8>, u8)>,
}

/// A note's XML, kept apart until the whole score is written: a later tied
/// note marks the note it continues as a tie's origin.
struct NoteOut {
    /// Elements before the tie (let ring, vibrato, accents).
    head: String,
    origin: bool,
    destination: bool,
    /// Everything after the tie.
    tail: String,
}

struct Writer<'s> {
    score: &'s Score,
    plans: Vec<TrackPlan>,
    losses: ImportLog,
    bars: Vec<String>,
    voices: Vec<String>,
    beats: Vec<String>,
    notes: Vec<NoteOut>,
    /// Each distinct rhythm's XML (without its id) and its id.
    rhythms: HashMap<String, usize>,
    /// The last note on each string or pitch, per staff slot and voice, for
    /// ties: (slot, voice, key) → note id.
    last_note: HashMap<(usize, usize, u16), usize>,
    /// The notes that start a hammer-on or pull-off: the next note on
    /// their string in their voice is its destination, which Guitar Pro
    /// marks too (`HopoDestination`).
    hopo_origins: std::collections::HashSet<usize>,
    /// Where the writer is, for the losses: (bar, track), 0-based.
    place: (usize, usize),
}

/// Text for a CDATA section: characters XML can't hold are left out, and a
/// `]]>` is split across two sections.
fn cdata(text: &str) -> String {
    let clean: String = text.chars().filter(|c| xml_char(*c)).collect();
    format!("<![CDATA[{}]]>", clean.replace("]]>", "]]]]><![CDATA[>"))
}

/// Text for an attribute value, escaped.
fn attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().filter(|c| xml_char(*c)) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    out
}

/// Whether XML 1.0 can hold the character.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && !matches!(c, '\u{FFFE}' | '\u{FFFF}'))
}

/// A mixer value (0 to 1) as the channel strip holds it: written in f64,
/// in which the reader's arithmetic on it is exact.
fn mix_value(v: f64) -> String {
    v.to_string()
}

/// The tuning Guitar Pro 7.6 gives a staff without strings, lowest first: a
/// guitar's on the first staff, a five-string bass's on the others (as it
/// saves a piano's grand staff).
fn default_tuning(staff: usize) -> &'static [u8] {
    if staff == 0 {
        &[40, 45, 50, 55, 59, 64]
    } else {
        &[23, 28, 33, 38, 43]
    }
}

/// Where Guitar Pro puts a pitch on a staff without strings: the string
/// (from 0, the lowest) with the lowest fret not below 0, or past either
/// end the lowest or highest string with a fret out of range.
fn string_and_fret(tuning_low_first: &[u8], pitch: u8) -> (usize, i32) {
    let pitch = i32::from(pitch);
    tuning_low_first
        .iter()
        .enumerate()
        .map(|(i, open)| (i, pitch - i32::from(*open)))
        .filter(|(_, fret)| *fret >= 0)
        .min_by_key(|(_, fret)| *fret)
        .unwrap_or((
            0,
            pitch - i32::from(tuning_low_first.first().copied().unwrap_or(0)),
        ))
}

/// A MIDI pitch as GPIF spells it, with sharps. Guitar Pro numbers octaves
/// from MIDI 0 (its octave is MIDI ÷ 12: MIDI 64 is E5, MIDI 60 C5), one
/// above scientific pitch, and draws the staff from these (Mateus's check
/// in Guitar Pro 7.6, 20 Oct 2026: written one octave lower, the staff was
/// drawn an octave low).
fn pitch_xml(midi: i32) -> String {
    const STEPS: [(&str, &str); 12] = [
        ("C", ""),
        ("C", "#"),
        ("D", ""),
        ("D", "#"),
        ("E", ""),
        ("F", ""),
        ("F", "#"),
        ("G", ""),
        ("G", "#"),
        ("A", ""),
        ("A", "#"),
        ("B", ""),
    ];
    let (step, accidental) = STEPS
        .get(usize::try_from(midi.rem_euclid(12)).unwrap_or(0))
        .copied()
        .unwrap_or(("C", ""));
    format!(
        "<Pitch><Step>{step}</Step><Accidental>{accidental}</Accidental><Octave>{}</Octave></Pitch>",
        midi.div_euclid(12)
    )
}

fn note_value_name(v: NoteValue) -> &'static str {
    match v {
        NoteValue::DoubleWhole => "DoubleWhole",
        NoteValue::Whole => "Whole",
        NoteValue::Half => "Half",
        NoteValue::Quarter => "Quarter",
        NoteValue::Eighth => "Eighth",
        NoteValue::Sixteenth => "16th",
        NoteValue::ThirtySecond => "32nd",
        NoteValue::SixtyFourth => "64th",
        NoteValue::OneTwentyEighth => "128th",
        NoteValue::TwoFiftySixth => "256th",
    }
}

fn dynamic_name(d: Dynamic) -> &'static str {
    match d {
        Dynamic::PPP => "PPP",
        Dynamic::PP => "PP",
        Dynamic::P => "P",
        Dynamic::MP => "MP",
        Dynamic::MF => "MF",
        Dynamic::F => "F",
        Dynamic::FF => "FF",
        Dynamic::FFF => "FFF",
    }
}

/// A direction's name and whether it is a target (where a jump lands).
fn direction_name(d: Direction) -> (&'static str, bool) {
    match d {
        Direction::Segno => ("Segno", true),
        Direction::SegnoSegno => ("SegnoSegno", true),
        Direction::Coda => ("Coda", true),
        Direction::DoubleCoda => ("DoubleCoda", true),
        Direction::Fine => ("Fine", true),
        Direction::ToCoda => ("DaCoda", false),
        Direction::ToDoubleCoda => ("DaDoubleCoda", false),
        Direction::DaCapo => ("DaCapo", false),
        Direction::DaCapoAlCoda => ("DaCapoAlCoda", false),
        Direction::DaCapoAlDoubleCoda => ("DaCapoAlDoubleCoda", false),
        Direction::DaCapoAlFine => ("DaCapoAlFine", false),
        Direction::DalSegno => ("DaSegno", false),
        Direction::DalSegnoAlCoda => ("DaSegnoAlCoda", false),
        Direction::DalSegnoAlDoubleCoda => ("DaSegnoAlDoubleCoda", false),
        Direction::DalSegnoAlFine => ("DaSegnoAlFine", false),
        Direction::DalSegnoSegno => ("DaSegnoSegno", false),
        Direction::DalSegnoSegnoAlCoda => ("DaSegnoSegnoAlCoda", false),
        Direction::DalSegnoSegnoAlDoubleCoda => ("DaSegnoSegnoAlDoubleCoda", false),
        Direction::DalSegnoSegnoAlFine => ("DaSegnoSegnoAlFine", false),
    }
}

fn triplet_feel_name(f: TripletFeel) -> Option<&'static str> {
    Some(match f {
        TripletFeel::None => return None,
        TripletFeel::Triplet8th => "Triplet8th",
        TripletFeel::Triplet16th => "Triplet16th",
        TripletFeel::Dotted8th => "Dotted8th",
        TripletFeel::Dotted16th => "Dotted16th",
        TripletFeel::Scottish8th => "Scottish8th",
        TripletFeel::Scottish16th => "Scottish16th",
    })
}

/// Whether a clef is written an octave above its sound (treble 8vb, bass
/// 8vb), and GPIF's name for it.
fn clef_name(c: Clef) -> (&'static str, Option<bool>) {
    match c {
        Clef::Treble => ("G2", Some(false)),
        Clef::Treble8vb => ("G2", Some(true)),
        Clef::Bass => ("F4", Some(false)),
        Clef::Bass8vb => ("F4", Some(true)),
        Clef::Alto => ("C3", None),
        Clef::Tenor => ("C4", None),
        Clef::Neutral => ("Neutral", None),
    }
}

/// The written voices of a bar, up to four: the first when it has beats,
/// the others when they hold more than placeholders (`BeatKind::Empty`).
/// The rest are written as no voice.
fn written_voices(bar: &Bar) -> impl Iterator<Item = (usize, &oxiguitar_model::Voice)> {
    bar.voices.iter().enumerate().take(4).filter(|(i, v)| {
        if *i == 0 {
            !v.beats.is_empty()
        } else {
            !v.is_empty()
        }
    })
}

impl<'s> Writer<'s> {
    fn new(score: &'s Score) -> Writer<'s> {
        let mut w = Writer {
            score,
            plans: Vec::new(),
            losses: ImportLog::default(),
            bars: Vec::new(),
            voices: Vec::new(),
            beats: Vec::new(),
            notes: Vec::new(),
            rhythms: HashMap::new(),
            last_note: HashMap::new(),
            hopo_origins: std::collections::HashSet::new(),
            place: (0, 0),
        };
        // MIDI channels as Guitar Pro hands them out: in track order,
        // skipping the drums' channel 10 (9 here).
        let mut next_channel = 0u8;
        for (ti, track) in score.tracks.iter().enumerate() {
            w.place = (0, ti);
            let channel = if track.instrument.percussion {
                9
            } else {
                let c = next_channel;
                next_channel = (next_channel + 1) % 16;
                if next_channel == 9 {
                    next_channel = 10;
                }
                c
            };
            let plan = w.plan(track, channel);
            w.plans.push(plan);
        }
        w
    }

    fn here(&self) -> String {
        format!("bar {}, track {}", self.place.0 + 1, self.place.1 + 1)
    }

    fn track_place(&self) -> String {
        format!("track {}", self.place.1 + 1)
    }

    fn loss(&mut self, message: &str) {
        let place = self.here();
        self.losses.warn(message, place);
    }

    /// Something Guitar Pro does differently with what is written, though
    /// it reads back the same here.
    fn note_for_guitar_pro(&mut self, message: &str) {
        let place = self.here();
        self.losses.info(message, place);
    }

    fn track_loss(&mut self, message: &str) {
        let place = self.track_place();
        self.losses.warn(message, place);
    }

    /// How a track is written: its sound, transposition, notation and
    /// strings. What one track of a .gp file can't hold for several staves
    /// is taken from the first.
    fn plan(&mut self, track: &Track, channel: u8) -> TrackPlan {
        let percussion = track.instrument.percussion;
        let sound = if percussion {
            tables::DRUMS
        } else {
            tables::GM_SOUNDS
                .get(usize::from(track.instrument.midi_program))
                .copied()
                .unwrap_or(tables::DRUMS)
        };
        let first = track.staves.first();
        if track.staves.len() > 4 {
            self.track_loss("only the first four staves of a track are kept");
        }

        // An octave clef (treble or bass 8vb) is the track's: GPIF writes
        // the octave once, as part of the track's transposition.
        let octave_clefs = first.map_or(0, |s| {
            s.bars
                .iter()
                .filter(|b| clef_name(b.clef).1 == Some(true))
                .count()
        });
        let plain_clefs = first.map_or(0, |s| {
            s.bars
                .iter()
                .filter(|b| clef_name(b.clef).1 == Some(false))
                .count()
        });
        let mut octave_shift = if !percussion && octave_clefs > 0 && octave_clefs >= plain_clefs {
            -1
        } else {
            0
        };
        let transposition = i32::from(first.map_or(0, |s| s.transposition));
        // The reader takes the octave from where the transposition falls
        // (-23 to -12 semitones); keep the clefs or the transposition.
        let mut transpose = -transposition + 12 * octave_shift;
        if octave_shift == -1 && !(-23..=-12).contains(&transpose) {
            octave_shift = 0;
            transpose = -transposition;
            self.track_loss("octave clefs can't be kept with this track's transposition");
        } else if octave_shift == 0 && (-23..=-12).contains(&transpose) {
            self.track_loss("this track's transposition can't be kept without octave clefs");
        }
        if !(-48..=48).contains(&transpose) {
            self.track_loss("transpositions of more than four octaves can't be kept");
        }
        if track
            .staves
            .iter()
            .any(|s| i32::from(s.transposition) != transposition)
        {
            self.track_loss("the staves of a track can't have different transpositions");
        }

        // Notation: one byte per track.
        // Drums show standard notation only, whatever their strings.
        let notation_of = |s: &Staff| {
            if percussion {
                1
            } else {
                u8::from(s.show_standard) | (u8::from(s.show_tablature) << 1)
            }
        };
        let notation = first.map_or(1, notation_of);
        if track.staves.iter().any(|s| notation_of(s) != notation) {
            self.track_loss("the staves of a track can't show different notation");
        }
        let stringed = !percussion && first.is_some_and(|s| !s.tuning.is_empty());
        if stringed && notation == 0 {
            self.track_loss(
                "a track showing neither notation is written showing standard notation",
            );
        } else if !stringed && notation & 0x02 != 0 {
            self.track_loss("tablature can't be shown for a track without strings");
        }
        // What the reader makes of it (`gpif::NOT_FRETTED`): an instrument
        // without frets shown without tablature has no strings; any other
        // instrument has the strings its tuning gives.
        let not_fretted = crate::gpif::NOT_FRETTED.contains(&sound.set_type);
        // A track with strings whose sound has no frets (a piano part with
        // frets, from a Guitar Pro 5 file) shown without tablature keeps
        // its strings as Guitar Pro files hold it: an electric guitar's
        // instrument set with the instrument's sound.
        let set = if !percussion && stringed && not_fretted && notation & 0x02 == 0 {
            ("Electric Guitar", "electricGuitar")
        } else {
            (sound.set_name, sound.set_type)
        };
        if !percussion && !stringed && !not_fretted {
            self.track_loss(
                "a fretted instrument without strings is written with Guitar Pro's six strings",
            );
        }

        let staves = track
            .staves
            .iter()
            .take(4)
            .map(|s| (s.tuning.iter().rev().map(|p| p.0).collect(), s.capo))
            .collect();
        TrackPlan {
            percussion,
            sound,
            set,
            octave_shift,
            transpose,
            notation,
            channel,
            staves,
        }
    }

    /// The whole score.gpif.
    fn document(&mut self) -> String {
        let score = self.score;
        let mut x = String::with_capacity(1 << 16);
        x.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<GPIF>\n");
        x.push_str("<GPVersion>7</GPVersion>\n");
        x.push_str("<GPRevision required=\"12021\" recommended=\"12023\">12025</GPRevision>\n");
        x.push_str("<Encoding>\n<EncodingDescription>GP7</EncodingDescription>\n</Encoding>\n");
        self.info(&mut x);
        self.master_track(&mut x);
        x.push_str("<AudioTracks>\n</AudioTracks>\n<Tracks>\n");
        let plans = std::mem::take(&mut self.plans);
        for (ti, (track, plan)) in score.tracks.iter().zip(&plans).enumerate() {
            self.place = (0, ti);
            self.track(&mut x, ti, track, plan);
        }
        x.push_str("</Tracks>\n");

        // The bars of each staff, then the master bars naming them.
        let slots: Vec<(usize, &TrackPlan, usize, &Staff)> = score
            .tracks
            .iter()
            .zip(&plans)
            .enumerate()
            .flat_map(|(ti, (t, plan))| {
                t.staves
                    .iter()
                    .take(4)
                    .enumerate()
                    .map(move |(si, staff)| (ti, plan, si, staff))
            })
            .collect();
        let mut master = String::new();
        master.push_str("<MasterBars>\n<!-- order is important here -->\n");
        for (bi, mb) in score.master_bars.iter().enumerate() {
            let mut ids = Vec::with_capacity(slots.len());
            for (slot, &(ti, plan, si, staff)) in slots.iter().enumerate() {
                self.place = (bi, ti);
                if let Some(bar) = staff.bars.get(bi) {
                    ids.push(self.bar(slot, plan, si, bar).to_string());
                }
            }
            self.place = (bi, 0);
            self.master_bar(&mut master, mb, &ids);
        }
        master.push_str("</MasterBars>\n");
        drop(slots);
        self.plans = plans;
        x.push_str(&master);

        x.push_str("<Bars>\n");
        for b in &self.bars {
            x.push_str(b);
        }
        x.push_str("</Bars>\n<Voices>\n");
        for v in &self.voices {
            x.push_str(v);
        }
        x.push_str("</Voices>\n<Beats>\n");
        for b in &self.beats {
            x.push_str(b);
        }
        x.push_str("</Beats>\n<Notes>\n");
        for (id, n) in self.notes.iter().enumerate() {
            let _ = write!(x, "<Note id=\"{id}\">\n{}", n.head);
            if n.origin || n.destination {
                let _ = writeln!(
                    x,
                    "<Tie origin=\"{}\" destination=\"{}\" />",
                    n.origin, n.destination
                );
            }
            x.push_str(&n.tail);
            x.push_str("</Note>\n");
        }
        x.push_str("</Notes>\n<Rhythms>\n");
        let mut rhythms: Vec<(&String, &usize)> = self.rhythms.iter().collect();
        rhythms.sort_by_key(|(_, id)| **id);
        for (body, id) in rhythms {
            let _ = writeln!(x, "<Rhythm id=\"{id}\">\n{body}</Rhythm>");
        }
        x.push_str("</Rhythms>\n</GPIF>\n");
        x
    }

    fn info(&mut self, x: &mut String) {
        let info = &self.score.info;
        let trimmed = [
            &info.title,
            &info.subtitle,
            &info.artist,
            &info.album,
            &info.words,
            &info.music,
            &info.copyright,
            &info.transcriber,
            &info.instructions,
        ];
        if info
            .notices
            .iter()
            .chain(trimmed)
            .any(|t| t.chars().any(|c| !xml_char(c)))
        {
            self.losses.warn(
                "control characters in the song's information are left out",
                "score information",
            );
        }
        x.push_str("<Score>\n");
        for (tag, text) in [
            ("Title", &info.title),
            ("SubTitle", &info.subtitle),
            ("Artist", &info.artist),
            ("Album", &info.album),
            ("Words", &info.words),
            ("Music", &info.music),
            ("WordsAndMusic", &String::new()),
            ("Copyright", &info.copyright),
            ("Tabber", &info.transcriber),
            ("Instructions", &info.instructions),
        ] {
            let _ = writeln!(x, "<{tag}>{}</{tag}>", cdata(text));
        }
        let _ = writeln!(x, "<Notices>{}</Notices>", cdata(&info.notices.join("\n")));
        x.push_str("</Score>\n");
    }

    fn master_track(&mut self, x: &mut String) {
        let score = self.score;
        let ids: Vec<String> = (0..score.tracks.len()).map(|i| i.to_string()).collect();
        let _ = writeln!(
            x,
            "<MasterTrack>\n<Tracks>{}</Tracks>\n<Automations>",
            ids.join(" ")
        );
        let tempo = |x: &mut String,
                     bar: usize,
                     position: f64,
                     bpm: f64,
                     linear: bool,
                     label: &str| {
            let _ = write!(
                x,
                "<Automation>\n<Type>Tempo</Type>\n<Linear>{linear}</Linear>\n<Bar>{bar}</Bar>\n\
                 <Position>{position}</Position>\n<Visible>true</Visible>\n<Value>{bpm} 2</Value>\n"
            );
            if !label.is_empty() {
                let _ = writeln!(x, "<Text>{}</Text>", cdata(label));
            }
            x.push_str("</Automation>\n");
        };
        let initial = score.initial_tempo.clamp(1.0, 2000.0);
        if initial != score.initial_tempo {
            self.losses.warn(
                "tempos outside 1 to 2,000 beats a minute are changed",
                "bar 1",
            );
        }
        tempo(x, 0, 0.0, initial, false, &score.tempo_label);
        for (bi, mb) in score.master_bars.iter().enumerate() {
            let length = mb.expected_length().unwrap_or(Ratio::ONE);
            for t in &mb.tempo_changes {
                let place = format!("bar {}", bi + 1);
                let fraction = t.position.checked_div(length).map_or(0.0, Ratio::to_f64);
                // The reader rounds to Guitar Pro's 960 ticks a quarter.
                let ticks = t
                    .position
                    .checked_mul(Ratio::from_integer(4 * TICKS_PER_QUARTER));
                if !(0.0..=1.0).contains(&fraction) || ticks.is_none_or(|r| r.denom() != 1) {
                    self.losses.warn(
                        "a tempo change is moved to the nearest place Guitar Pro can hold",
                        place.clone(),
                    );
                }
                let bpm = t.bpm.clamp(1.0, 2000.0);
                if bpm != t.bpm {
                    self.losses.warn(
                        "tempos outside 1 to 2,000 beats a minute are changed",
                        place,
                    );
                }
                tempo(x, bi, fraction.clamp(0.0, 1.0), bpm, t.linear, &t.label);
            }
        }
        x.push_str("</Automations>\n</MasterTrack>\n");
    }

    fn track(&mut self, x: &mut String, ti: usize, track: &Track, plan: &TrackPlan) {
        let _ = writeln!(x, "<Track id=\"{ti}\">");
        let _ = writeln!(x, "<Name>{}</Name>", cdata(&track.name));
        let _ = writeln!(x, "<ShortName>{}</ShortName>", cdata(&track.short_name));
        let c = track.color;
        let _ = writeln!(x, "<Color>{} {} {}</Color>", c.r, c.g, c.b);
        x.push_str("<SystemsDefautLayout>3</SystemsDefautLayout>\n<PalmMute>0</PalmMute>\n<PlayingStyle>Default</PlayingStyle>\n");
        let s = plan.sound;
        let _ = write!(
            x,
            "<InstrumentSet>\n<Name>{}</Name>\n<Type>{}</Type>\n<LineCount>5</LineCount>\n<Elements>\n",
            plan.set.0, plan.set.1
        );
        if plan.percussion {
            drum_elements(x);
        }
        x.push_str("</Elements>\n</InstrumentSet>\n");
        let octave = plan.transpose / 12;
        let chromatic = plan.transpose % 12;
        let _ = writeln!(
            x,
            "<Transpose>\n<Chromatic>{chromatic}</Chromatic>\n<Octave>{octave}</Octave>\n</Transpose>"
        );

        // Volume and pan in the channel strip (parameters 12 and 11, 0 to 1).
        let mix = track.mix;
        let volume = mix_value(f64::from(mix.volume.clamp(0.0, 1.0)));
        let pan = mix_value((f64::from(mix.pan.clamp(-1.0, 1.0)) + 1.0) / 2.0);
        let params: Vec<String> = (0..16)
            .map(|i| match i {
                11 => pan.clone(),
                12 => volume.clone(),
                _ => "0.5".to_owned(),
            })
            .collect();
        let _ = writeln!(
            x,
            "<RSE>\n<ChannelStrip version=\"E56\">\n<Parameters>{}</Parameters>\n</ChannelStrip>\n</RSE>\n<ForcedSound>-1</ForcedSound>",
            params.join(" ")
        );
        let _ = write!(
            x,
            "<Sounds>\n<Sound>\n<Name>{}</Name>\n<Label>{}</Label>\n<Path>{}</Path>\n<Role>User</Role>\n\
             <MIDI>\n<LSB>0</LSB>\n<MSB>0</MSB>\n<Program>{}</Program>\n</MIDI>\n\
             <RSE>\n<SoundbankPatch>{}</SoundbankPatch>\n\
             <Pickups>\n<OverloudPosition>0</OverloudPosition>\n<Volumes>1 1</Volumes>\n<Tones>1 1</Tones>\n</Pickups>\n\
             <EffectChain>\n<Effect />\n<Effect />\n<Effect />\n<Effect />\n<Effect />\n<Effect />\n</EffectChain>\n\
             </RSE>\n</Sound>\n</Sounds>\n",
            cdata(""),
            cdata(""),
            s.path,
            track.instrument.midi_program.min(127),
            s.patch
        );
        let _ = writeln!(
            x,
            "<MidiConnection>\n<Port>0</Port>\n<PrimaryChannel>{0}</PrimaryChannel>\n<SecondaryChannel>{0}</SecondaryChannel>\n<ForeOneChannelPerString>false</ForeOneChannelPerString>\n</MidiConnection>",
            plan.channel
        );
        let state = if mix.solo {
            "Solo"
        } else if mix.mute {
            "Mute"
        } else {
            "Default"
        };
        let _ = writeln!(
            x,
            "<PlaybackState>{state}</PlaybackState>\n<AudioEngineState>RSE</AudioEngineState>"
        );
        let both = mix.solo && mix.mute;

        // Lyrics: Guitar Pro writes five lines, used or not.
        x.push_str("<Lyrics dispatched=\"true\">\n");
        let mut lines: Vec<(usize, &str)> = track
            .lyrics
            .iter()
            .map(|l| (l.start_bar, l.text.as_str()))
            .collect();
        let too_many = lines.len() > 5;
        lines.resize(5.max(lines.len()), (0, ""));
        for (offset, text) in lines.iter().take(5) {
            let _ = writeln!(
                x,
                "<Line>\n<Text>{}</Text>\n<Offset>{offset}</Offset>\n</Line>",
                cdata(text)
            );
        }
        x.push_str("</Lyrics>\n<Staves>\n");
        for si in 0..track.staves.len().min(4) {
            self.staff(x, ti, track, plan, si);
        }
        x.push_str("</Staves>\n");
        let _ = writeln!(
            x,
            "<Automations>\n<Automation>\n<Type>Sound</Type>\n<Linear>false</Linear>\n<Bar>0</Bar>\n<Position>0</Position>\n<Visible>true</Visible>\n<Value>{}</Value>\n</Automation>\n</Automations>\n</Track>",
            cdata(&format!("{};;User", s.path))
        );
        if both {
            self.track_loss("a track can't be both muted and soloed; it is soloed");
        }
        if too_many {
            self.track_loss("only five lines of lyrics are kept");
        }
    }

    fn staff(&mut self, x: &mut String, ti: usize, track: &Track, plan: &TrackPlan, si: usize) {
        let Some(staff) = track.staves.get(si) else {
            return;
        };
        let low_first: &[u8] = plan.staves.get(si).map_or(&[], |s| s.0.as_slice());
        let strings = low_first.len();
        x.push_str("<Staff>\n<Properties>\n");
        let _ = writeln!(
            x,
            "<Property name=\"CapoFret\">\n<Fret>{}</Fret>\n</Property>\n<Property name=\"FretCount\">\n<Number>{}</Number>\n</Property>",
            staff.capo, staff.fret_count
        );
        let _ = writeln!(
            x,
            "<Property name=\"PartialCapoFret\">\n<Fret>0</Fret>\n</Property>\n<Property name=\"PartialCapoStringFlags\">\n<Bitset>{}</Bitset>\n</Property>",
            "0".repeat(strings.max(6))
        );
        // Drums have a tuning of zeros, which the reader takes for none. A
        // staff without strings (a piano, a violin) has Guitar Pro's own
        // default, as Guitar Pro 7.6 writes it, and shows no tablature.
        let pitches: Vec<String> = if plan.percussion {
            vec!["0".to_owned(); 6]
        } else if strings == 0 {
            default_tuning(si).iter().map(ToString::to_string).collect()
        } else {
            low_first.iter().map(ToString::to_string).collect()
        };
        let _ = writeln!(
            x,
            "<Property name=\"Tuning\">\n<Pitches>{}</Pitches>\n<Instrument>Undefined</Instrument>\n<Label>{}</Label>\n<LabelVisible>true</LabelVisible>\n</Property>",
            pitches.join(" "),
            cdata("")
        );
        if (staff.capo > 99) || !(1..=36).contains(&staff.fret_count) {
            self.track_loss("a capo above fret 99, or a fret count outside 1 to 36, is changed");
        }
        x.push_str("<Property name=\"ChordCollection\">\n<Items/>\n</Property>\n<Property name=\"ChordWorkingSet\">\n<Items/>\n</Property>\n");
        x.push_str("<Property name=\"DiagramCollection\">\n<Items>\n");
        let tuning = &staff.tuning;
        for (i, d) in track.chords.iter().enumerate() {
            diagram_item(x, i, d, strings, tuning, |m| {
                self.losses.warn(m, format!("track {}", ti + 1));
            });
        }
        x.push_str(
            "</Items>\n</Property>\n<Property name=\"DiagramWorkingSet\">\n<Items/>\n</Property>\n",
        );
        let _ = writeln!(x, "<Name>{}</Name>\n</Properties>\n</Staff>", cdata(""));
    }

    fn master_bar(&mut self, x: &mut String, mb: &MasterBar, ids: &[String]) {
        x.push_str("<MasterBar>\n");
        let k = mb.key_signature;
        let _ = writeln!(
            x,
            "<Key>\n<AccidentalCount>{}</AccidentalCount>\n<Mode>{}</Mode>\n<TransposeAs>{}</TransposeAs>\n</Key>",
            k.fifths.clamp(-7, 7),
            if k.mode == KeyMode::Minor {
                "Minor"
            } else {
                "Major"
            },
            if k.fifths > 0 { "Sharps" } else { "Flats" }
        );
        let t = mb.time_signature;
        let _ = writeln!(x, "<Time>{}/{}</Time>", t.numerator, t.denominator);
        if mb.repeat_start || mb.repeat_end.is_some() {
            let count = mb.repeat_end.unwrap_or(0);
            let _ = writeln!(
                x,
                "<Repeat start=\"{}\" end=\"{}\" count=\"{count}\" />",
                mb.repeat_start,
                mb.repeat_end.is_some()
            );
            if mb.repeat_end.is_some_and(|c| !(2..=99).contains(&c)) {
                self.loss("repeats are played 2 to 99 times");
            }
        }
        if mb.alternate_endings != 0 {
            let endings: Vec<String> = (0..16)
                .filter(|i| mb.alternate_endings & (1 << i) != 0)
                .map(|i| (i + 1).to_string())
                .collect();
            let _ = writeln!(
                x,
                "<AlternateEndings>{}</AlternateEndings>",
                endings.join(" ")
            );
        }
        if mb.double_bar {
            x.push_str("<DoubleBar />\n");
        }
        if mb.free_time {
            x.push_str("<FreeTime />\n");
        }
        if let Some(m) = &mb.marker {
            let _ = writeln!(
                x,
                "<Section>\n<Letter>{}</Letter>\n<Text>{}</Text>\n</Section>",
                cdata(&m.letter),
                cdata(&m.text)
            );
            if m.color != oxiguitar_model::Color::default() {
                self.loss("section colours are left out");
            }
        }
        if !mb.directions.is_empty() {
            x.push_str("<Directions>\n");
            for d in &mb.directions {
                let (name, target) = direction_name(*d);
                let tag = if target { "Target" } else { "Jump" };
                let _ = writeln!(x, "<{tag}>{name}</{tag}>");
            }
            x.push_str("</Directions>\n");
        }
        if let Some(feel) = triplet_feel_name(mb.triplet_feel) {
            let _ = writeln!(x, "<TripletFeel>{feel}</TripletFeel>");
        }
        let _ = writeln!(x, "<Bars>{}</Bars>\n</MasterBar>", ids.join(" "));
    }

    /// Writes one staff's bar and returns its id.
    fn bar(&mut self, slot: usize, plan: &TrackPlan, si: usize, bar: &Bar) -> usize {
        let score = self.score;
        let track = score.tracks.get(self.place.1);
        let (clef, octave) = if plan.percussion {
            ("Neutral", None)
        } else {
            clef_name(bar.clef)
        };
        if plan.percussion && bar.clef != Clef::Neutral {
            self.loss("a drum track's clef is written as the drums' own");
        } else if let Some(down) = octave
            && down != (plan.octave_shift == -1)
        {
            self.loss("a bar's octave clef is written as its track's");
        }
        if bar.voices.len() > 4 {
            self.loss("only four voices of a bar are kept");
        }
        let mut voice_ids = [
            "-1".to_owned(),
            "-1".to_owned(),
            "-1".to_owned(),
            "-1".to_owned(),
        ];
        // A voice nobody uses is written as no voice ("-1"), as Guitar Pro
        // 6 wrote it, so the voices after it keep their places, and a bar
        // without beats has no voice at all (MuseScore fills an empty
        // voice with notes from elsewhere). The reader reads them back as
        // empty placeholders, or leaves them out after the last used one.
        for (vi, voice) in written_voices(bar) {
            let mut beat_ids = Vec::with_capacity(voice.beats.len());
            for beat in &voice.beats {
                beat_ids.push(self.beat(slot, track, plan, si, vi, beat).to_string());
            }
            let id = self.voices.len();
            self.voices.push(format!(
                "<Voice id=\"{id}\">\n<Beats>{}</Beats>\n</Voice>\n",
                beat_ids.join(" ")
            ));
            if let Some(slot) = voice_ids.get_mut(vi) {
                *slot = id.to_string();
            }
        }
        let id = self.bars.len();
        self.bars.push(format!(
            "<Bar id=\"{id}\">\n<Clef>{clef}</Clef>\n<Voices>{}</Voices>\n</Bar>\n",
            voice_ids.join(" ")
        ));
        id
    }

    /// The id of a rhythm, written once for every beat that has it.
    fn rhythm(&mut self, d: &Duration) -> usize {
        let mut body = format!("<NoteValue>{}</NoteValue>\n", note_value_name(d.value));
        if d.dots > 0 {
            let _ = writeln!(body, "<AugmentationDot count=\"{}\" />", d.dots.min(4));
        }
        let tuplets: Vec<_> = d
            .tuplets
            .iter()
            .filter(|t| t.notes > 1 && t.in_time_of > 0 && t.notes != t.in_time_of)
            .collect();
        for (tag, t) in ["PrimaryTuplet", "SecondaryTuplet"].iter().zip(&tuplets) {
            let _ = writeln!(
                body,
                "<{tag} num=\"{}\" den=\"{}\" />",
                t.notes, t.in_time_of
            );
        }
        if tuplets.len() > 2 {
            self.loss("only two tuplets inside one another are kept");
        }
        if d.dots > 4 {
            self.loss("only four dots on a note are kept");
        }
        let next = self.rhythms.len();
        *self.rhythms.entry(body).or_insert(next)
    }

    /// Writes one beat and its notes, and returns its id.
    fn beat(
        &mut self,
        slot: usize,
        track: Option<&Track>,
        plan: &TrackPlan,
        si: usize,
        vi: usize,
        beat: &Beat,
    ) -> usize {
        let chords = track.map_or(0, |t| t.chords.len());
        let rhythm = self.rhythm(&beat.duration);
        let fx = &beat.effects;
        let mut b = String::new();
        match beat.grace {
            Some(GraceKind::OnBeat) => b.push_str("<GraceNotes>OnBeat</GraceNotes>\n"),
            Some(GraceKind::BeforeBeat) => b.push_str("<GraceNotes>BeforeBeat</GraceNotes>\n"),
            None => {}
        }

        // The beat's dynamic is its notes' too. A beat with no marking whose
        // notes share a dynamic other than the default gets it as its marking.
        let notes: Vec<&Note> = if beat.kind == BeatKind::Notes {
            beat.notes.iter().collect()
        } else {
            Vec::new()
        };
        let shared = notes.first().map(|n| n.dynamic);
        let dynamic = fx.dynamic.or(shared.filter(|d| *d != Dynamic::default()));
        if notes
            .iter()
            .any(|n| n.dynamic != dynamic.unwrap_or_default())
        {
            self.loss("notes of one beat with different dynamics take the beat's dynamic");
        }
        if let Some(d) = dynamic {
            let _ = writeln!(b, "<Dynamic>{}</Dynamic>", dynamic_name(d));
        }
        let _ = writeln!(
            b,
            "<Rhythm ref=\"{rhythm}\" />\n<TransposedPitchStemOrientation>Undefined</TransposedPitchStemOrientation>\n<ConcertPitchStemOrientation>Undefined</ConcertPitchStemOrientation>"
        );
        if let Some(c) = beat.chord {
            if c < chords {
                let _ = writeln!(b, "<Chord>{c}</Chord>");
            } else {
                self.loss("a chord name without its diagram is left out");
            }
        }
        if !notes.is_empty() && beat.kind != BeatKind::Notes {
            self.loss("notes in a rest are left out");
        }

        // Tremolo picking: per beat in GPIF; a note's moves to its beat.
        let tremolo = fx
            .tremolo_picking
            .or_else(|| notes.iter().find_map(|n| n.effects.tremolo_picking));
        if notes.iter().any(|n| {
            n.effects
                .tremolo_picking
                .is_some_and(|t| Some(t) != tremolo)
        }) {
            self.loss("notes of one beat with different tremolo picking take the first one's");
        }
        let tapped = fx.technique == Some(BeatTechnique::Tap);
        if tapped && notes.is_empty() {
            self.loss("tapping on a rest is left out");
        }

        let mut note_ids = Vec::with_capacity(notes.len());
        let mut seen_strings = Vec::new();
        for note in &notes {
            if let Some(s) = note.string() {
                if seen_strings.contains(&s) {
                    self.loss("a second note on the same string in one beat is left out");
                    continue;
                }
                seen_strings.push(s);
            }
            if let Some(id) = self.note(slot, plan, si, vi, note, tapped, fx.let_ring) {
                note_ids.push(id.to_string());
            }
        }
        if !note_ids.is_empty() {
            let _ = writeln!(b, "<Notes>{}</Notes>", note_ids.join(" "));
        }
        if !beat.text.is_empty() {
            let _ = writeln!(b, "<FreeText>{}</FreeText>", cdata(&beat.text));
        }
        if fx.fade_in {
            b.push_str("<Fadding>FadeIn</Fadding>\n");
        }
        if let Some(t) = tremolo {
            let speed = match t {
                NoteValue::Eighth => "1/2",
                NoteValue::Sixteenth => "1/4",
                NoteValue::ThirtySecond => "1/8",
                _ => {
                    self.loss(
                        "tremolo picking is written in eighths, sixteenths or thirty-seconds",
                    );
                    "1/4"
                }
            };
            let _ = writeln!(b, "<Tremolo>{speed}</Tremolo>");
        }
        if fx.vibrato_wide {
            b.push_str("<Vibrato>Wide</Vibrato>\n");
        }

        let mut props = String::new();
        if !plan.percussion && plan.staves.get(si).is_some_and(|s| !s.0.is_empty()) {
            props.push_str("<Property name=\"PrimaryPickupVolume\">\n<Float>0.500000</Float>\n</Property>\n<Property name=\"PrimaryPickupTone\">\n<Float>0.500000</Float>\n</Property>\n");
        }
        let mut brush_ticks = None;
        if let Some(s) = fx.stroke {
            let dir = if s.direction == StrokeDirection::Up {
                "Up"
            } else {
                "Down"
            };
            let _ = writeln!(
                props,
                "<Property name=\"Brush\">\n<Direction>{dir}</Direction>\n</Property>"
            );
            let d = i64::from(s.speed.denominator());
            if (1..=128).contains(&d) {
                brush_ticks = Some(4 * TICKS_PER_QUARTER / d);
            } else {
                self.loss("a strum is written at the nearest speed Guitar Pro has");
                brush_ticks = Some(if d == 0 { 4 * TICKS_PER_QUARTER } else { 30 });
            }
        }
        if let Some(dir) = fx.pick_stroke {
            let dir = if dir == StrokeDirection::Up {
                "Up"
            } else {
                "Down"
            };
            let _ = writeln!(
                props,
                "<Property name=\"PickStroke\">\n<Direction>{dir}</Direction>\n</Property>"
            );
        }
        match fx.technique {
            Some(BeatTechnique::Slap) => {
                props.push_str("<Property name=\"Slapped\">\n<Enable />\n</Property>\n")
            }
            Some(BeatTechnique::Pop) => {
                props.push_str("<Property name=\"Popped\">\n<Enable />\n</Property>\n")
            }
            _ => {}
        }
        if let Some(w) = fx.whammy.as_ref().filter(|w| !w.points.is_empty()) {
            let names = [
                "WhammyBarOriginValue",
                "WhammyBarMiddleValue",
                "WhammyBarDestinationValue",
                "WhammyBarOriginOffset",
                "WhammyBarMiddleOffset1",
                "WhammyBarMiddleOffset2",
                "WhammyBarDestinationOffset",
            ];
            let (points, fits) = fit_curve(&w.points);
            if !fits {
                self.loss(
                    "a whammy bar curve with more points than Guitar Pro holds is simplified",
                );
            }
            if !on_sixtieths(&points) {
                self.loss(
                    "a whammy bar curve's points are moved to the nearest sixtieth of the beat",
                );
            }
            if points.iter().any(|p| p.cents != 0) {
                props.push_str("<Property name=\"WhammyBar\">\n<Enable />\n</Property>\n");
                bend_props(&mut props, &points, names);
            }
        }
        if !props.is_empty() {
            let _ = write!(b, "<Properties>\n{props}</Properties>\n");
        }
        if let Some(t) = brush_ticks {
            let _ = writeln!(
                b,
                "<XProperties>\n<XProperty id=\"687935489\">\n<Int>{t}</Int>\n</XProperty>\n</XProperties>"
            );
        }
        let id = self.beats.len();
        self.beats.push(format!("<Beat id=\"{id}\">\n{b}</Beat>\n"));
        id
    }

    /// Writes one note and returns its id, or `None` for a note a .gp file
    /// can't place.
    #[allow(clippy::too_many_arguments)]
    fn note(
        &mut self,
        slot: usize,
        plan: &TrackPlan,
        si: usize,
        vi: usize,
        note: &Note,
        tapped: bool,
        let_ring: bool,
    ) -> Option<usize> {
        let (low_first, capo) = plan
            .staves
            .get(si)
            .map_or((&[][..], 0), |s| (s.0.as_slice(), s.1));
        let strings = low_first.len();
        let fx = &note.effects;
        let percussion = plan.percussion;
        let transpose = plan.transpose;

        // Where the note is, and its sounding pitch.
        let (place_props, sounding, key, open): (String, i32, u16, Option<i32>) = match note.place {
            NotePlace::Fretted { string, fret } => {
                if percussion || strings == 0 {
                    self.loss("a note on a string of a track without strings is left out");
                    return None;
                }
                let s = usize::from(string);
                if s == 0 || s > strings {
                    self.loss("a note on a string the track does not have is left out");
                    return None;
                }
                if fret > 99 {
                    self.loss("a note above fret 99 is left out");
                    return None;
                }
                let open = i32::from(*low_first.get(strings - s)?) + i32::from(capo);
                let sounding = open + i32::from(fret);
                (
                    format!(
                        "<Property name=\"Fret\">\n<Fret>{fret}</Fret>\n</Property>\n\
                         <Property name=\"Midi\">\n<Number>{}</Number>\n</Property>\n\
                         <Property name=\"String\">\n<String>{}</String>\n</Property>\n",
                        sounding.clamp(0, 127),
                        strings - s
                    ),
                    sounding,
                    u16::from(string),
                    Some(open),
                )
            }
            NotePlace::Pitched(Pitch(p)) => {
                if p > 127 {
                    self.loss("a note above MIDI 127 is left out");
                    return None;
                }
                let mut props = String::new();
                if percussion {
                    match tables::drum_articulation(p) {
                        Some(_) => {}
                        None => self.note_for_guitar_pro(
                            "a drum sound Guitar Pro's kit doesn't have plays as a snare in Guitar Pro",
                        ),
                    }
                }
                let _ = writeln!(
                    props,
                    "<Property name=\"Midi\">\n<Number>{p}</Number>\n</Property>"
                );
                if !percussion {
                    // Guitar Pro gives every note a string and fret, on a
                    // staff without strings too, from its default tuning,
                    // out of range as need be (Guitar Pro 7.6: fret -40 on
                    // the lowest string for MIDI 0, 63 on the highest for
                    // MIDI 127). Without them it drew frets of -2³¹.
                    let (string, fret) = string_and_fret(default_tuning(si), p);
                    let _ = writeln!(
                        props,
                        "<Property name=\"Fret\">\n<Fret>{fret}</Fret>\n</Property>\n\
                         <Property name=\"String\">\n<String>{string}</String>\n</Property>"
                    );
                    // Readers that don't look at Midi on a track without
                    // strings (alphaTab) take the pitch from these, as
                    // Guitar Pro 6 wrote it.
                    let _ = writeln!(
                        props,
                        "<Property name=\"Octave\">\n<Number>{}</Number>\n</Property>\n\
                         <Property name=\"Tone\">\n<Step>{}</Step>\n</Property>",
                        p / 12,
                        p % 12
                    );
                }
                (props, i32::from(p), 256 + u16::from(p), None)
            }
        };

        let mut head = String::new();
        if fx.let_ring || let_ring {
            head.push_str("<LetRing />\n");
        }
        if fx.vibrato {
            head.push_str("<Vibrato>Slight</Vibrato>\n");
        }
        if fx.ghost {
            head.push_str("<AntiAccent>Normal</AntiAccent>\n");
        }
        let accent = u32::from(fx.staccato)
            | match fx.accent {
                Some(Accent::Heavy) => 0x04,
                Some(Accent::Normal) => 0x08,
                None => 0,
            };
        if accent != 0 {
            let _ = writeln!(head, "<Accent>{accent}</Accent>");
        }

        let mut tail = String::new();
        if let Some(t) = fx.trill {
            // The trill names its other note by MIDI pitch; a note without a
            // string by the number itself.
            let value = open.map_or(i32::from(t.fret), |o| o + i32::from(t.fret));
            let _ = writeln!(tail, "<Trill>{value}</Trill>");
            if t.speed != NoteValue::Sixteenth {
                self.loss("trills are written in sixteenths");
            }
            if t.fret > 99 {
                self.loss("a trill above fret 99 is changed");
            }
        }
        for (tag, finger) in [
            ("LeftFingering", fx.left_finger),
            ("RightFingering", fx.right_finger),
        ] {
            if let Some(f) = finger {
                match ["P", "I", "M", "A", "C"].get(usize::from(f)) {
                    Some(name) => {
                        let _ = writeln!(tail, "<{tag}>{name}</{tag}>");
                    }
                    None => self.loss("a finger other than thumb to little finger is left out"),
                }
            }
        }
        let articulation = if percussion {
            match note.place {
                NotePlace::Pitched(Pitch(p)) => tables::drum_articulation(p).unwrap_or(0),
                NotePlace::Fretted { .. } => 0,
            }
        } else {
            0
        };
        let _ = writeln!(
            tail,
            "<InstrumentArticulation>{articulation}</InstrumentArticulation>"
        );

        let mut props = String::new();
        let bend = fx.bend.as_ref().map(|b| fit_curve(&b.points));
        if let Some((points, fits)) = bend.filter(|(p, _)| p.iter().any(|p| p.cents != 0)) {
            if !fits {
                self.loss("a bend with more points than Guitar Pro holds is simplified");
            }
            if !on_sixtieths(&points) {
                self.loss("a bend's points are moved to the nearest sixtieth of the note");
            }
            props.push_str("<Property name=\"Bended\">\n<Enable />\n</Property>\n");
            let names = [
                "BendOriginValue",
                "BendMiddleValue",
                "BendDestinationValue",
                "BendOriginOffset",
                "BendMiddleOffset1",
                "BendMiddleOffset2",
                "BendDestinationOffset",
            ];
            bend_props(&mut props, &points, names);
        }
        let _ = writeln!(
            props,
            "<Property name=\"ConcertPitch\">\n{}\n</Property>",
            pitch_xml(sounding)
        );
        props.push_str(&place_props);
        if let Some(h) = fx.harmonic {
            let (kind, fret) = match h {
                Harmonic::Natural => ("Natural", None),
                Harmonic::Artificial { semitones } => ("Artificial", Some(semitones)),
                Harmonic::Tapped { fret } => ("Tap", Some(fret)),
                Harmonic::Pinch => ("Pinch", None),
                Harmonic::Semi => ("Semi", None),
                Harmonic::Feedback => ("Feedback", None),
            };
            // The reader takes 12 for a harmonic without a fret.
            let fret = fret.unwrap_or(12);
            if fret > 48 {
                self.loss("a harmonic more than four octaves up is changed");
            }
            props.push_str("<Property name=\"Harmonic\">\n<Enable />\n</Property>\n");
            let _ = writeln!(
                props,
                "<Property name=\"HarmonicFret\">\n<HFret>{fret}</HFret>\n</Property>\n<Property name=\"HarmonicType\">\n<HType>{kind}</HType>\n</Property>"
            );
        }
        if self
            .last_note
            .get(&(slot, vi, key))
            .is_some_and(|p| self.hopo_origins.contains(p))
        {
            props.push_str("<Property name=\"HopoDestination\">\n<Enable />\n</Property>\n");
        }
        if fx.hammer_pull {
            props.push_str("<Property name=\"HopoOrigin\">\n<Enable />\n</Property>\n");
        }
        if fx.left_hand_tap {
            props.push_str("<Property name=\"LeftHandTapped\">\n<Enable />\n</Property>\n");
        }
        if fx.dead {
            props.push_str("<Property name=\"Muted\">\n<Enable />\n</Property>\n");
        }
        if fx.palm_mute {
            props.push_str("<Property name=\"PalmMuted\">\n<Enable />\n</Property>\n");
        }
        // A slide from a voice's last note, to no note, is kept: players
        // slide off a note to nothing (Mateus, 20 Oct 2026). MuseScore 4
        // crashes on it; the interop check works around that.
        let slides = &fx.slides;
        if !slides.is_empty() {
            let (flags, fits) = slide_flags(slides);
            if !fits {
                self.loss("a note can slide in one way and out one way; the others are left out");
            }
            let _ = writeln!(
                props,
                "<Property name=\"Slide\">\n<Flags>{flags}</Flags>\n</Property>"
            );
        }
        if tapped {
            props.push_str("<Property name=\"Tapped\">\n<Enable />\n</Property>\n");
        }
        let _ = writeln!(
            props,
            "<Property name=\"TransposedPitch\">\n{}\n</Property>",
            pitch_xml(sounding - transpose)
        );
        let _ = write!(tail, "<Properties>\n{props}</Properties>\n");

        // Ties: this note continues the last one on its string (or pitch)
        // in the same voice.
        let id = self.notes.len();
        let key3 = (slot, vi, key);
        if note.tie
            && let Some(&previous) = self.last_note.get(&key3)
            && let Some(prev) = self.notes.get_mut(previous)
        {
            prev.origin = true;
        }
        self.last_note.insert(key3, id);
        if fx.hammer_pull {
            self.hopo_origins.insert(id);
        }
        self.notes.push(NoteOut {
            head,
            origin: false,
            destination: note.tie,
            tail,
        });
        Some(id)
    }
}

/// The slide flags of a note (1 shift, 2 legato, 4 out down, 8 out up,
/// 16 in from below, 32 in from above), and whether they hold every slide:
/// a note slides in one way and out one way.
fn slide_flags(slides: &[Slide]) -> (u32, bool) {
    let mut into = None;
    let mut out = None;
    let mut fits = true;
    for s in slides {
        let (group, flag) = match s {
            Slide::InFromBelow => (&mut into, 16),
            Slide::InFromAbove => (&mut into, 32),
            Slide::Shift => (&mut out, 1),
            Slide::Legato => (&mut out, 2),
            Slide::OutDown => (&mut out, 4),
            Slide::OutUp => (&mut out, 8),
        };
        if group.is_some() {
            fits = false;
        } else {
            *group = Some(flag);
        }
    }
    (into.unwrap_or(0) | out.unwrap_or(0), fits)
}

/// Whether every point of a curve sits on a sixtieth of its note, where the
/// reader puts them, and within two octaves.
fn on_sixtieths(points: &[BendPoint]) -> bool {
    points.iter().all(|p| {
        p.offset
            .checked_mul(Ratio::from_integer(60))
            .is_some_and(|r| r.denom() == 1)
            && (Ratio::ZERO..=Ratio::ONE).contains(&p.offset)
            && p.cents.abs() <= 2400
    })
}

/// Whether GPIF holds a curve as it is: an origin, a middle with one value
/// (at one or two places) and a destination, in order, no point repeated.
fn curve_fits(p: &[BendPoint]) -> bool {
    let distinct = p.windows(2).all(|w| matches!(w, [a, b] if a != b));
    let ordered = p
        .windows(2)
        .all(|w| matches!(w, [a, b] if a.offset <= b.offset));
    let shape = match p {
        [_, b, c, _] => b.cents == c.cents,
        _ => p.len() <= 3,
    };
    distinct && ordered && shape
}

/// A curve as GPIF can hold it, and whether nothing changed that shows or
/// sounds. One that doesn't fit loses its points that change nothing:
/// repeats, points inside a hold, and a hold at zero before the bend or
/// after it (as Guitar Pro 3–5 write them). One that still doesn't fit
/// keeps its first, furthest and last points.
fn fit_curve(points: &[BendPoint]) -> (Vec<BendPoint>, bool) {
    if curve_fits(points) {
        return (points.to_vec(), true);
    }
    let mut v = points.to_vec();
    v.dedup();
    let mut i = 1;
    while i + 1 < v.len() {
        let flat = matches!(v.get(i - 1..=i + 1), Some([a, b, c]) if a.cents == b.cents && b.cents == c.cents);
        if flat {
            v.remove(i);
        } else {
            i += 1;
        }
    }
    if matches!(v.as_slice(), [a, b, ..] if a.cents == 0 && b.cents == 0) {
        v.remove(0);
    }
    if matches!(v.as_slice(), [.., a, b] if a.cents == 0 && b.cents == 0) {
        v.pop();
    }
    if curve_fits(&v) {
        return (v, true);
    }
    match points {
        [first, middle @ .., last] => {
            let far = middle
                .iter()
                .max_by_key(|q| q.cents.abs())
                .copied()
                .unwrap_or(*first);
            (vec![*first, far, far, *last], false)
        }
        _ => (v, false),
    }
}

/// Writes a bend or whammy curve as GPIF's origin, middle and destination
/// (the middle has one value and two places), named by `names`: origin,
/// middle and destination values, then the four places. The curve is one
/// [`fit_curve`] made.
fn bend_props(props: &mut String, points: &[BendPoint], names: [&str; 7]) {
    let zero = BendPoint {
        offset: Ratio::ZERO,
        cents: 0,
    };
    let (origin, m1, m2, dest) = match points {
        [] => (zero, zero, zero, zero),
        &[a] => (a, a, a, a),
        &[a, b] => (a, b, b, b),
        &[a, b, c] => (a, b, b, c),
        &[a, b, c, d, ..] => (a, b, c, d),
    };
    let value = |q: BendPoint| f64::from(q.cents) / 2.0;
    let offset = |q: BendPoint| q.offset.to_f64() * 100.0;
    let values = [
        value(origin),
        value(m1),
        value(dest),
        offset(origin),
        offset(m1),
        offset(m2),
        offset(dest),
    ];
    for (name, v) in names.iter().zip(values) {
        let _ = writeln!(
            props,
            "<Property name=\"{name}\">\n<Float>{v}</Float>\n</Property>"
        );
    }
}

/// A chord diagram and its spelling, as an item of a staff's
/// `DiagramCollection`. `tuning` is the staff's, highest string first.
fn diagram_item(
    x: &mut String,
    id: usize,
    d: &ChordDiagram,
    strings: usize,
    tuning: &[Pitch],
    mut loss: impl FnMut(&str),
) {
    let _ = writeln!(x, "<Item id=\"{id}\" name=\"{}\">", attr(&d.name));
    let base = d.first_fret.saturating_sub(1);
    let count = if d.frets.is_empty() {
        strings
    } else {
        d.frets.len()
    };
    let highest = d.frets.iter().flatten().max().copied().unwrap_or(0);
    let span = highest.saturating_sub(base).max(5);
    let _ = writeln!(
        x,
        "<Diagram stringCount=\"{count}\" fretCount=\"{span}\" baseFret=\"{base}\" barsStates=\"1 1 1 1 1\">"
    );
    let n = d.frets.len();
    for (i, f) in d.frets.iter().enumerate() {
        if let Some(f) = *f {
            // Fret 0 is an open string; the others count from the base.
            let relative = if f == 0 {
                0
            } else if f <= base {
                loss("a diagram's fret below its first fret is moved up to it");
                1
            } else {
                f - base
            };
            let _ = writeln!(x, "<Fret string=\"{}\" fret=\"{relative}\" />", n - 1 - i);
        }
    }
    let shown = !d.frets.is_empty();
    let _ = writeln!(
        x,
        "<Property name=\"ShowName\" type=\"bool\" value=\"true\" />\n<Property name=\"ShowDiagram\" type=\"bool\" value=\"{shown}\" />\n<Property name=\"ShowFingering\" type=\"bool\" value=\"false\" />\n</Diagram>"
    );
    if d.frets.len() > 10 {
        loss("a diagram with more than ten strings is cut to ten");
    }
    let mut spelling = chord::spell(&d.name);
    let pitches: Vec<u8> = d
        .frets
        .iter()
        .zip(tuning)
        .filter_map(|(f, open)| f.map(|f| open.0.saturating_add(f)))
        .collect();
    spelling.mark_omitted(&pitches);
    let _ = writeln!(
        x,
        "<Chord>\n<KeyNote step=\"{}\" accidental=\"{}\" />\n<BassNote step=\"{}\" accidental=\"{}\" />",
        spelling.key.step,
        spelling.key.accidental(),
        spelling.bass.step,
        spelling.bass.accidental()
    );
    for g in &spelling.degrees {
        let _ = writeln!(
            x,
            "<Degree interval=\"{}\" alteration=\"{}\" omitted=\"{}\" />",
            g.interval, g.alteration, g.omitted
        );
    }
    x.push_str("</Chord>\n</Item>\n");
}

/// Guitar Pro 7's drum kit, as a drum track's instrument elements.
fn drum_elements(x: &mut String) {
    for e in tables::DRUM_KIT {
        let _ = write!(
            x,
            "<Element>\n<Name>{}</Name>\n<Type>{}</Type>\n<SoundbankName>{}</SoundbankName>\n<Articulations>\n",
            e.name, e.kind, e.soundbank
        );
        for a in e.articulations {
            let _ = write!(
                x,
                "<Articulation>\n<Name>{}</Name>\n<StaffLine>{}</StaffLine>\n<Noteheads>{}</Noteheads>\n\
                 <TechniquePlacement>{}</TechniquePlacement>\n<TechniqueSymbol>{}</TechniqueSymbol>\n\
                 <InputMidiNumbers>{}</InputMidiNumbers>\n<OutputRSESound>{}</OutputRSESound>\n\
                 <OutputMidiNumber>{}</OutputMidiNumber>\n</Articulation>\n",
                a.name, a.staff_line, a.noteheads, a.placement, a.symbol, a.input, a.rse, a.output
            );
        }
        x.push_str("</Articulations>\n</Element>\n");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn text_is_escaped() {
        assert_eq!(cdata("a]]>b"), "<![CDATA[a]]]]><![CDATA[>b]]>");
        assert_eq!(cdata("x\u{1}y"), "<![CDATA[xy]]>");
        assert_eq!(attr("A&B \"7\" <x>"), "A&amp;B &quot;7&quot; &lt;x&gt;");
    }

    #[test]
    fn mixer_values_come_back_exactly() {
        for pan in [-1.0f32, -0.3, 0.0, 0.1, 0.7, 1.0, 0.123_456_79] {
            let p: f64 = mix_value((f64::from(pan) + 1.0) / 2.0).parse().unwrap();
            assert_eq!(((p * 2.0 - 1.0) as f32).clamp(-1.0, 1.0), pan, "{pan}");
        }
    }

    fn fixture(path: &str) -> Score {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path);
        crate::read(&std::fs::read(path).unwrap()).unwrap().score
    }

    #[test]
    fn written_for_other_readers_too() {
        // A note without a string carries its pitch as octave and tone.
        let piano = fixture("fixtures/reference/piano-grand-staff.gp");
        let gpif = Writer::new(&piano).document();
        // C5 (MIDI 72): fret 8 on the first staff's high E string of
        // Guitar Pro's default tuning, and octave and tone for alphaTab.
        assert!(gpif.contains(
            "<Property name=\"Midi\">\n<Number>72</Number>\n</Property>\n\
             <Property name=\"Fret\">\n<Fret>8</Fret>\n</Property>\n\
             <Property name=\"String\">\n<String>5</String>\n</Property>\n\
             <Property name=\"Octave\">\n<Number>6</Number>\n</Property>\n\
             <Property name=\"Tone\">\n<Step>0</Step>\n</Property>"
        ));
        assert!(gpif.contains("<Pitches>40 45 50 55 59 64</Pitches>"));

        // A bar without beats has no voice; a slide from a voice's last note,
        // to no note, is kept (players slide off to nothing).
        let mut song = fixture("fixtures/gp/oxiguitar-test-song.gp");
        let bars = &mut song.tracks[0].staves[0].bars;
        bars[1].voices = vec![oxiguitar_model::Voice::default()];
        let last = bars
            .iter_mut()
            .rev()
            .flat_map(|b| b.voices[0].beats.iter_mut().rev())
            .find(|b| !b.notes.is_empty())
            .unwrap();
        last.notes[0].effects.slides = vec![Slide::Shift];
        let mut w = Writer::new(&song);
        let gpif = w.document();
        assert!(gpif.contains("<Voices>-1 -1 -1 -1</Voices>"));
        assert_eq!(w.losses.warning_count(), 0);
        let back = crate::read(&write_gp(&song).unwrap().data).unwrap().score;
        assert_eq!(
            back.tracks[0].staves[0].bars[1],
            song.tracks[0].staves[0].bars[1]
        );
        assert_eq!(back.tracks[0].staves[0].bars, song.tracks[0].staves[0].bars);
    }

    #[test]
    fn slides_in_one_way_and_out_one_way() {
        assert_eq!(slide_flags(&[Slide::InFromBelow, Slide::Shift]), (17, true));
        assert_eq!(slide_flags(&[Slide::OutUp]), (8, true));
        assert_eq!(slide_flags(&[Slide::Shift, Slide::Legato]), (1, false));
    }

    #[test]
    fn every_program_has_a_sound_and_the_kit_its_drums() {
        assert_eq!(tables::GM_SOUNDS[54].path, "Orchestra/Other/Singer");
        assert_eq!(tables::GM_SOUNDS[30].patch, "Classic-Guitar");
        assert_eq!(
            tables::DRUM_KIT
                .iter()
                .map(|e| e.articulations.len())
                .sum::<usize>(),
            95
        );
        // Crash high is the 23rd articulation, as Guitar Pro numbers it.
        assert_eq!(tables::drum_articulation(49), Some(22));
        assert_eq!(tables::drum_articulation(38), Some(0));
        assert_eq!(tables::drum_articulation(27), None);
    }
}
