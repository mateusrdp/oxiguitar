// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writer for Guitar Pro 5 files (.gp5): the reader (`super`) in reverse.
//!
//! The file claims version 5.10, the last Guitar Pro 5 wrote, and holds
//! what that version holds: the layout is `docs/formats/guitar-pro-3-5.md`.
//! Everything the reader takes from a file the writer puts back, so a score
//! read from a .gp5 file and written again reads back the same. What the
//! format can't hold is named in the losses (FMT-3), each with the first
//! place it happens; what it holds in another shape (a dynamic marking is a
//! note's dynamic, a tuplet's chord diagram is stored with its beat) is
//! named too, as information.
//!
//! What Guitar Pro 5 does not have, among the things the model has: a
//! second staff in a track, more than two voices in a bar, more than one
//! lyrics track, clefs (they follow the instrument), pitches without a
//! string (a track without strings is written as a guitar's tab), and many
//! details of tuplets, tempo changes, harmonics and bends. The module's
//! functions name each where it happens.

use oxiguitar_model::{
    Accent, Bar, Beat, BeatKind, BeatTechnique, Bend, ChordDiagram, Direction, Dynamic, GraceKind,
    Harmonic, KeyMode, KeySignature, MasterBar, Note, NotePlace, NoteValue, Ratio, Score, Slide,
    Staff, StrokeDirection, Track, TripletFeel,
};

use super::CHANNEL_STEPS;
use crate::binary::encode_text;
use crate::error::ImportLog;

const VERSION: &str = "FICHIER GUITAR PRO v5.10";
const MAX_STRINGS: usize = 7;
/// The channels of the file's table: four ports of sixteen.
const CHANNELS: usize = 64;
/// A string whose length is a byte holds at most this many characters.
const MAX_TEXT: usize = 255;
/// Bend points kept: as many as the reader takes.
const MAX_BEND_POINTS: usize = 64;
/// Guitar Pro measures bend positions in sixtieths of the note.
const BEND_POSITION_MAX: i64 = 60;
const BEND_CENTS_PER_UNIT: i32 = 2;

/// A written .gp5 file and what it couldn't hold.
pub(crate) struct Gp5Written {
    pub(crate) data: Vec<u8>,
    pub(crate) losses: ImportLog,
}

/// Writes `score` as a .gp5 file. Fails only for a score with no tracks or
/// bars, or whose staves don't all have a bar for every bar of the score.
pub(crate) fn write(score: &Score) -> Result<Gp5Written, String> {
    if score.tracks.is_empty() {
        return Err("the score has no tracks".into());
    }
    if score.master_bars.is_empty() {
        return Err("the score has no bars".into());
    }
    score.check_structure()?;
    let mut w = Writer::new(score);
    w.song();
    Ok(Gp5Written {
        data: w.out,
        losses: w.losses,
    })
}

/// Where the writer is, for the losses.
#[derive(Clone, Copy)]
enum At {
    Score,
    Track(usize),
    /// A bar's header, 0-based.
    Header(usize),
    /// (bar, track), 0-based.
    Bar(usize, usize),
}

/// How a track's notes are written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Notes on strings.
    Strings,
    /// A track without strings: notes are placed on a guitar's by pitch.
    Pitched,
    /// Drums: the fret is the drum, the string only keeps notes apart.
    Drums,
}

/// How a track is written, worked out before its bars are.
struct TrackPlan {
    kind: Kind,
    /// The tuning as written, highest string first.
    tuning: Vec<u8>,
    capo: u8,
    /// The index in the channel table of the track's channel and of its
    /// effect channel.
    channel: usize,
    effect_channel: usize,
    /// For drums: the string each drum is written on, so that a drum stays
    /// on its string and a tied one finds the note before it.
    home: std::collections::HashMap<u8, usize>,
}

struct Writer<'s> {
    score: &'s Score,
    out: Vec<u8>,
    losses: ImportLog,
    at: At,
    plans: Vec<TrackPlan>,
    /// The last fret written on each string of each track, to see that a
    /// tied note reads back with the fret it has: the reader takes it from
    /// the note before.
    last_fret: Vec<[u8; MAX_STRINGS]>,
}

impl<'s> Writer<'s> {
    fn new(score: &'s Score) -> Writer<'s> {
        let mut w = Writer {
            score,
            out: Vec::new(),
            losses: ImportLog::default(),
            at: At::Score,
            plans: Vec::new(),
            last_fret: vec![[0; MAX_STRINGS]; score.tracks.len()],
        };
        w.plans = w.plan_tracks();
        w.at = At::Score;
        w
    }

    // ---- the losses -----------------------------------------------------

    fn place(&self) -> String {
        match self.at {
            At::Score => "score".into(),
            At::Track(t) => format!("track {}", t + 1),
            At::Header(b) => format!("bar {}", b + 1),
            At::Bar(b, t) => format!("bar {}, track {}", b + 1, t + 1),
        }
    }

    /// Something the file can't hold.
    fn loss(&mut self, message: &str) {
        let place = self.place();
        self.losses.warn(message, place);
    }

    /// Something held in another shape; reads back as it means, not as it
    /// is stored here.
    fn kept_as(&mut self, message: &str) {
        let place = self.place();
        self.losses.info(message, place);
    }

    // ---- the primitives -------------------------------------------------

    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }

    fn i8(&mut self, v: i8) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn i16(&mut self, v: i16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn bool(&mut self, v: bool) {
        self.out.push(u8::from(v));
    }

    fn zeros(&mut self, n: usize) {
        self.out.resize(self.out.len() + n, 0);
    }

    fn color(&mut self, c: oxiguitar_model::Color) {
        self.out.extend_from_slice(&[c.r, c.g, c.b, 0]);
    }

    /// Text as the file holds it: Windows-1252, at most `max` bytes.
    fn encode(&mut self, text: &str, max: usize, what: &str) -> Vec<u8> {
        let (mut bytes, replaced) = encode_text(text);
        if replaced {
            self.loss("characters outside Windows-1252 are written as ?");
        }
        if bytes.len() > max {
            bytes.truncate(max);
            self.loss(&format!("{what} is cut to {max} characters"));
        }
        bytes
    }

    /// Guitar Pro's usual string: an int with the field's size (length + 1),
    /// a length byte, the text.
    fn int_byte_size_string(&mut self, text: &str, what: &str) {
        let bytes = self.encode(text, MAX_TEXT, what);
        self.i32(i32::try_from(bytes.len() + 1).unwrap_or(1));
        self.u8(u8::try_from(bytes.len()).unwrap_or(0));
        self.out.extend_from_slice(&bytes);
    }

    /// A string in a field of a fixed size: a length byte, then the text
    /// padded with zeros to `size`.
    fn byte_size_string(&mut self, text: &str, size: usize, what: &str) {
        let bytes = self.encode(text, size, what);
        self.u8(u8::try_from(bytes.len()).unwrap_or(0));
        self.out.extend_from_slice(&bytes);
        self.zeros(size - bytes.len());
    }

    /// A string whose length is an int (lyrics).
    fn int_size_string(&mut self, text: &str) {
        let (bytes, replaced) = encode_text(text);
        if replaced {
            self.loss("characters outside Windows-1252 are written as ?");
        }
        self.i32(i32::try_from(bytes.len()).unwrap_or(0));
        self.out.extend_from_slice(&bytes);
    }

    // ---- the song -------------------------------------------------------

    fn song(&mut self) {
        let score = self.score;
        self.byte_size_string(VERSION, 30, "the version");
        self.info();
        self.lyrics();
        // The RSE master effect: volume 100, the rest unset.
        self.i32(100);
        self.i32(0);
        self.zeros(11);
        self.page_setup();

        self.int_byte_size_string(&score.tempo_label, "the tempo's name");
        let tempo = self.tempo(score.initial_tempo);
        self.i32(tempo);
        self.bool(false); // tempo hidden
        let first_key = score
            .master_bars
            .first()
            .map_or(0, |m| m.key_signature.fifths.clamp(-7, 7));
        self.i8(first_key);
        self.i32(0); // octave

        self.channels();
        self.directions();
        self.i32(0); // master reverb

        self.i32(i32::try_from(score.master_bars.len()).unwrap_or(0));
        self.i32(i32::try_from(score.tracks.len()).unwrap_or(0));
        self.master_bars(first_key);
        for ti in 0..score.tracks.len() {
            self.at = At::Track(ti);
            self.track(ti);
        }
        self.u8(0);
        self.bars();
    }

    fn info(&mut self) {
        let info = &self.score.info;
        self.int_byte_size_string(&info.title, "the title");
        self.int_byte_size_string(&info.subtitle, "the subtitle");
        self.int_byte_size_string(&info.artist, "the artist");
        self.int_byte_size_string(&info.album, "the album");
        self.int_byte_size_string(&info.words, "the words");
        self.int_byte_size_string(&info.music, "the music");
        self.int_byte_size_string(&info.copyright, "the copyright");
        self.int_byte_size_string(&info.transcriber, "the transcriber");
        self.int_byte_size_string(&info.instructions, "the instructions");
        self.i32(i32::try_from(info.notices.len()).unwrap_or(0));
        for line in &info.notices {
            self.int_byte_size_string(line, "a line of notices");
        }
    }

    /// Lyrics belong to one track; five lines.
    fn lyrics(&mut self) {
        let sung = |t: &Track| t.lyrics.iter().any(|l| !l.text.trim().is_empty());
        let chosen = self.score.tracks.iter().position(sung);
        if self.score.tracks.iter().filter(|t| sung(t)).count() > 1 {
            self.loss("only one track's lyrics are kept");
        }
        let lines: Vec<&oxiguitar_model::LyricLine> = chosen
            .and_then(|t| self.score.tracks.get(t))
            .map(|t| {
                t.lyrics
                    .iter()
                    .filter(|l| !l.text.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if chosen
            .and_then(|t| self.score.tracks.get(t))
            .is_some_and(|t| t.lyrics.len() != lines.len())
        {
            self.loss("lines of lyrics with no text are left out");
        }
        if lines.len() > 5 {
            self.loss("only five lines of lyrics are kept");
        }
        self.i32(chosen.map_or(0, |t| i32::try_from(t + 1).unwrap_or(0)));
        for i in 0..5 {
            match lines.get(i) {
                Some(line) => {
                    self.i32(i32::try_from(line.start_bar + 1).unwrap_or(1));
                    self.int_size_string(&line.text);
                }
                None => {
                    self.i32(1);
                    self.i32(0);
                }
            }
        }
    }

    /// The page: A4 with Guitar Pro's usual margins and header and footer
    /// templates; the model keeps none of it.
    fn page_setup(&mut self) {
        for v in [210, 297, 10, 10, 15, 10, 100] {
            self.i32(v);
        }
        self.i16(0x01FF);
        for t in [
            "%TITLE%",
            "%SUBTITLE%",
            "%ARTIST%",
            "%ALBUM%",
            "Words by %WORDS%",
            "Music by %MUSIC%",
            "Words & Music by %WORDS%",
            "Copyright %COPYRIGHT%",
            "All Rights Reserved - International Copyright Secured",
            "Page %N%/%P%",
        ] {
            self.int_byte_size_string(t, "a template");
        }
    }

    /// A tempo as the file holds it: a whole number of beats a minute.
    fn tempo(&mut self, bpm: f64) -> i32 {
        let rounded = bpm.round().clamp(1.0, 1000.0);
        if (rounded - bpm).abs() > 1e-9 || !bpm.is_finite() {
            self.loss("tempos are whole numbers between 1 and 1000");
        }
        // Checked above: a whole number in range.
        rounded as i32
    }

    // ---- tracks ---------------------------------------------------------

    /// Which channels of the table each track gets: two each, as Guitar Pro
    /// gives them, and the drums' on the tenth channel of a port.
    fn plan_tracks(&mut self) -> Vec<TrackPlan> {
        let mut used = [false; CHANNELS];
        let mut plans = Vec::new();
        let mut last = (0, 1);
        for (ti, track) in self.score.tracks.iter().enumerate() {
            self.at = At::Track(ti);
            let (channel, effect_channel) = match allocate(&mut used, track.instrument.percussion) {
                Some(pair) => {
                    last = pair;
                    pair
                }
                None => {
                    self.loss("tracks beyond the MIDI channels Guitar Pro 5 has share them, and their volume and pan");
                    last
                }
            };
            if track.staves.len() > 1 {
                self.loss("only the first staff of a track is kept");
            }
            let (kind, tuning, capo) = match track.staves.first() {
                Some(s) => self.tuning_of(track, s),
                None => (Kind::Strings, Vec::new(), 0),
            };
            let home = if kind == Kind::Drums {
                drum_homes(track, tuning.len())
            } else {
                Default::default()
            };
            plans.push(TrackPlan {
                kind,
                tuning,
                capo,
                channel,
                effect_channel,
                home,
            });
        }
        plans
    }

    /// The strings a track is written with.
    fn tuning_of(&mut self, track: &Track, staff: &Staff) -> (Kind, Vec<u8>, u8) {
        let to_bytes = |t: &[oxiguitar_model::Pitch]| t.iter().map(|p| p.0).collect::<Vec<u8>>();
        if track.instrument.percussion {
            let mut tuning = to_bytes(&staff.tuning);
            if tuning.len() > MAX_STRINGS {
                self.loss("a track can have seven strings at most; the others are left out");
                tuning.truncate(MAX_STRINGS);
            }
            if tuning.is_empty() {
                tuning = vec![0; 6];
            }
            return (Kind::Drums, tuning, 0);
        }
        if staff.tuning.is_empty() {
            self.loss("a track without strings is written as a guitar's tablature, its notes placed by pitch");
            // A guitar's strings, and a lower one when the music goes below.
            let mut tuning: Vec<u8> = Staff::standard_guitar_tuning()
                .iter()
                .map(|p| p.0)
                .collect();
            let lowest = staff
                .bars
                .iter()
                .flat_map(|b| &b.voices)
                .flat_map(|v| &v.beats)
                .flat_map(|b| &b.notes)
                .filter_map(|n| match n.place {
                    NotePlace::Pitched(p) => Some(p.0),
                    NotePlace::Fretted { .. } => None,
                })
                .min();
            if let Some(low) = lowest.filter(|l| *l < 40) {
                tuning.push(low);
            }
            return (Kind::Pitched, tuning, 0);
        }
        let mut tuning = to_bytes(&staff.tuning);
        if tuning.len() > MAX_STRINGS {
            self.loss("a track can have seven strings at most; the others are left out");
            tuning.truncate(MAX_STRINGS);
        }
        if staff.capo > 30 {
            self.loss("a capo above fret 30 is changed");
        }
        (Kind::Strings, tuning, staff.capo.min(30))
    }

    fn channels(&mut self) {
        // What each channel of the table holds: the track that uses it.
        let mut owner: Vec<Option<&Track>> = vec![None; CHANNELS];
        for (track, plan) in self.score.tracks.iter().zip(&self.plans) {
            for i in [plan.channel, plan.effect_channel] {
                if let Some(slot) = owner.get_mut(i) {
                    *slot = Some(track);
                }
            }
        }
        for (i, track) in owner.into_iter().enumerate() {
            match track {
                Some(t) => {
                    self.i32(i32::from(t.instrument.midi_program));
                    self.u8(volume_step(t.mix.volume));
                    self.u8(pan_step(t.mix.pan));
                }
                None => {
                    self.i32(if i % 16 == 9 { -1 } else { 25 });
                    self.u8(13);
                    self.u8(8);
                }
            }
            self.zeros(6); // chorus, reverb, phaser, tremolo; two unused
        }
        let whole = |v: f32| (v - v.round()).abs() < 1e-3;
        if self.score.tracks.iter().any(|t| {
            !whole(t.mix.volume * CHANNEL_STEPS) || !whole((t.mix.pan + 1.0) * CHANNEL_STEPS / 2.0)
        }) {
            self.kept_as("volume and pan are kept to sixteen steps");
        }
    }

    /// The signs of the score: one bar each.
    fn directions(&mut self) {
        use Direction::*;
        const ORDER: [Direction; 19] = [
            Coda,
            DoubleCoda,
            Segno,
            SegnoSegno,
            Fine,
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
            ToCoda,
            ToDoubleCoda,
        ];
        for dir in ORDER {
            let bars: Vec<usize> = self
                .score
                .master_bars
                .iter()
                .enumerate()
                .filter(|(_, m)| m.directions.contains(&dir))
                .map(|(i, _)| i)
                .collect();
            if bars.len() > 1 {
                self.at = At::Score;
                self.loss("a sign (segno, coda, da capo …) can be in one bar only");
            }
            let bar = bars
                .first()
                .and_then(|b| i16::try_from(b + 1).ok())
                .unwrap_or(-1);
            self.i16(bar);
        }
    }

    // ---- the bar headers ------------------------------------------------

    fn master_bars(&mut self, song_key: i8) {
        let mut previous_time = oxiguitar_model::TimeSignature::COMMON;
        let mut previous_key = KeySignature {
            fifths: song_key,
            mode: KeyMode::Major,
        };
        for (i, mb) in self.score.master_bars.iter().enumerate() {
            self.at = At::Header(i);
            self.master_bar(i, mb, previous_time, previous_key);
            previous_time = mb.time_signature;
            previous_key = KeySignature {
                fifths: mb.key_signature.fifths.clamp(-7, 7),
                mode: mb.key_signature.mode,
            };
        }
        self.at = At::Score;
    }

    fn master_bar(
        &mut self,
        index: usize,
        mb: &MasterBar,
        previous_time: oxiguitar_model::TimeSignature,
        previous_key: KeySignature,
    ) {
        if index > 0 {
            self.u8(0);
        }
        let time_changed = mb.time_signature != previous_time;
        let key = KeySignature {
            fifths: mb.key_signature.fifths.clamp(-7, 7),
            mode: mb.key_signature.mode,
        };
        if key.fifths != mb.key_signature.fifths {
            self.loss("key signatures have up to seven sharps or flats");
        }
        let endings = u8::try_from(mb.alternate_endings & 0xFF).unwrap_or(0);
        if mb.alternate_endings > 0xFF {
            self.loss("alternate endings go up to the eighth");
        }
        let plays = mb.repeat_end.map(|n| n.clamp(2, 99));
        if mb.repeat_end.is_some_and(|n| !(2..=99).contains(&n)) {
            self.loss("repeats are played 2 to 99 times");
        }
        let mut flags = 0u8;
        if time_changed {
            flags |= 0x03;
        }
        if mb.repeat_start {
            flags |= 0x04;
        }
        if plays.is_some() {
            flags |= 0x08;
        }
        if endings != 0 {
            flags |= 0x10;
        }
        if mb.marker.is_some() {
            flags |= 0x20;
        }
        if key != previous_key {
            flags |= 0x40;
        }
        if mb.double_bar {
            flags |= 0x80;
        }
        self.u8(flags);
        if time_changed {
            self.u8(mb.time_signature.numerator);
            self.u8(mb.time_signature.denominator);
        }
        if let Some(n) = plays {
            self.i8(i8::try_from(n).unwrap_or(2));
        }
        if let Some(marker) = &mb.marker {
            if !marker.letter.is_empty() {
                self.loss("section letters are left out");
            }
            self.int_byte_size_string(&marker.text, "a section's name");
            self.color(marker.color);
        }
        if flags & 0x40 != 0 {
            self.i8(key.fifths);
            self.i8(i8::from(key.mode == KeyMode::Minor));
        }
        if time_changed {
            let beams = beams(mb.time_signature);
            self.out.extend_from_slice(&beams);
        }
        self.u8(endings);
        let feel = match mb.triplet_feel {
            TripletFeel::None => 0,
            TripletFeel::Triplet8th => 1,
            TripletFeel::Triplet16th => 2,
            _ => {
                self.loss("only the triplet feels of eighths and sixteenths are kept");
                0
            }
        };
        self.u8(feel);
        if mb.free_time {
            self.loss("free time is left out");
        }
    }

    // ---- tracks ---------------------------------------------------------

    fn track(&mut self, ti: usize) {
        let score = self.score;
        let Some(track) = score.tracks.get(ti) else {
            return;
        };
        if ti == 0 {
            self.u8(0);
        }
        let staff = track.staves.first();
        let (plan_tuning, plan_channel, plan_effect, plan_kind) = match self.plans.get(ti) {
            Some(p) => (p.tuning.clone(), p.channel, p.effect_channel, p.kind),
            None => (Vec::new(), 0, 1, Kind::Strings),
        };
        let mut flags = 0x08u8; // shown
        if track.instrument.percussion {
            flags |= 0x01;
        }
        if track.mix.solo {
            flags |= 0x10;
        }
        if track.mix.mute {
            flags |= 0x20;
        }
        self.u8(flags);
        self.byte_size_string(&track.name, 40, "a track's name");
        if !track.short_name.is_empty() {
            self.kept_as("short track names are left out");
        }
        self.i32(i32::try_from(plan_tuning.len()).unwrap_or(6));
        for i in 0..MAX_STRINGS {
            self.i32(i32::from(plan_tuning.get(i).copied().unwrap_or(0)));
        }
        // Port and channel numbers count from 1 within a port.
        self.i32(i32::try_from(plan_channel / 16 + 1).unwrap_or(1));
        self.i32(i32::try_from(plan_channel % 16 + 1).unwrap_or(1));
        self.i32(i32::try_from(plan_effect % 16 + 1).unwrap_or(2));
        let fret_count = staff.map_or(24, |s| s.fret_count);
        if !(1..=36).contains(&fret_count) {
            self.loss("a track has 1 to 36 frets");
        }
        let fret_count = if plan_kind == Kind::Pitched {
            24
        } else {
            fret_count.clamp(1, 36)
        };
        self.i32(i32::from(fret_count));
        let capo = self.plans.get(ti).map_or(0, |p| p.capo);
        self.i32(i32::from(capo));
        self.color(track.color);

        let (tab, standard) = staff.map_or((true, true), |s| (s.show_tablature, s.show_standard));
        if staff.is_some_and(|s| s.transposition != 0) {
            self.loss("a staff's transposition is left out");
        }
        // Tablature, notation, and the chord list Guitar Pro shows.
        let notation = i16::from(tab) | i16::from(standard) << 1 | 0x40;
        self.i16(notation);
        self.u8(0); // automatic accentuation
        self.u8(0); // MIDI bank
        // The RSE block: no humanising, the clef's octave shift twice,
        // and unset sounds.
        self.u8(0);
        let low = plan_tuning.iter().copied().min().unwrap_or(40);
        let shift = if plan_kind != Kind::Drums && low < 35 {
            12
        } else {
            0
        };
        self.i32(shift);
        self.i32(shift);
        self.i32(100);
        self.zeros(12);
        for _ in 0..4 {
            self.i32(-1);
        }
        self.zeros(4); // equaliser
        self.int_byte_size_string("", "a sound");
        self.int_byte_size_string("", "a sound");
    }

    // ---- bars -----------------------------------------------------------

    fn bars(&mut self) {
        let score = self.score;
        for (bi, mb) in score.master_bars.iter().enumerate() {
            // Tempo changes are carried by the first track with a beat at
            // their place; those written so far in this bar.
            let mut written: Vec<Ratio> = Vec::new();
            for (ti, track) in score.tracks.iter().enumerate() {
                self.at = At::Bar(bi, ti);
                if let Some(bar) = track.staves.first().and_then(|s| s.bars.get(bi)) {
                    self.bar(ti, bar, mb, &mut written);
                }
            }
            self.at = At::Header(bi);
            if mb.tempo_changes.len() > written.len() {
                self.loss("a tempo change that is not at the start of a beat is left out");
            }
        }
    }

    fn bar(&mut self, ti: usize, bar: &Bar, mb: &MasterBar, written: &mut Vec<Ratio>) {
        let expected = self.score.tracks.get(ti).map(super::clef_for);
        if expected.is_some_and(|c| c != bar.clef) {
            self.loss("clefs follow the instrument, as Guitar Pro 5 has no clef of its own");
        }
        if bar.voices.len() > 2 {
            self.loss("only two voices of a bar are kept");
        }
        for vi in 0..2 {
            match bar.voices.get(vi) {
                Some(voice) if voice.beats.iter().any(|b| b.grace.is_none()) => {
                    self.voice(ti, voice, vi == 0, mb, written);
                }
                other => {
                    if other.is_some_and(|v| !v.beats.is_empty()) {
                        self.loss("a grace note needs a note after it to belong to");
                    }
                    // An unused voice: one empty beat, as Guitar Pro writes.
                    self.i32(1);
                    self.u8(0x40);
                    self.u8(0x00);
                    self.i8(0);
                    self.u8(0); // no strings
                    self.i16(0);
                }
            }
        }
        self.u8(0); // line break
    }

    fn voice(
        &mut self,
        ti: usize,
        voice: &oxiguitar_model::Voice,
        first_voice: bool,
        mb: &MasterBar,
        written: &mut Vec<Ratio>,
    ) {
        // Grace beats fold into the beat after them.
        let mut mains: Vec<(&Beat, Vec<&Beat>)> = Vec::new();
        let mut graces: Vec<&Beat> = Vec::new();
        for beat in &voice.beats {
            if beat.grace.is_some() {
                graces.push(beat);
            } else {
                mains.push((beat, std::mem::take(&mut graces)));
            }
        }
        if !graces.is_empty() {
            self.loss("a grace note needs a note after it to belong to");
        }
        self.i32(i32::try_from(mains.len()).unwrap_or(0));
        let mut position = Ratio::ZERO;
        for (beat, graces) in mains {
            let tempo = if first_voice {
                mb.tempo_changes
                    .iter()
                    .find(|t| t.position == position && !written.contains(&t.position))
            } else {
                None
            };
            if let Some(t) = tempo {
                written.push(t.position);
            }
            self.beat(ti, beat, &graces, tempo);
            position = beat
                .duration
                .length()
                .and_then(|l| position.checked_add(l))
                .unwrap_or(position);
        }
    }

    /// The placement of a note: its string (1 = highest) and the number
    /// written in the fret byte, or why it is left out.
    fn place_note(&mut self, ti: usize, note: &Note, drum_string: Option<u8>) -> Option<(u8, u8)> {
        let plan = self.plans.get(ti)?;
        match placement(plan, note, drum_string) {
            Ok(p) => Some(p),
            Err(why) => {
                self.loss(why);
                None
            }
        }
    }

    fn beat(
        &mut self,
        ti: usize,
        beat: &Beat,
        graces: &[&Beat],
        tempo: Option<&oxiguitar_model::TempoChange>,
    ) {
        let kind = match beat.kind {
            BeatKind::Empty => 0x00,
            _ => 0x02,
        };
        let has_notes = beat.kind == BeatKind::Notes && !beat.notes.is_empty();
        let status = beat.kind == BeatKind::Empty || !has_notes;
        if beat.kind == BeatKind::Rest && !beat.notes.is_empty() {
            self.loss("notes in a rest are left out");
        }
        let chord = beat
            .chord
            .and_then(|i| self.score.tracks.get(ti).and_then(|t| t.chords.get(i)));
        if beat.chord.is_some() && chord.is_none() {
            self.loss("a chord name without a diagram is left out");
        }
        let fx = self.beat_effects_flags(beat);
        if beat.effects.dynamic.is_some() {
            self.kept_as("dynamic markings are written as the notes' dynamics");
        }

        let (value, tuplet) = self.duration_codes(beat);
        let mut flags = 0u8;
        if beat.duration.dots > 0 {
            flags |= 0x01;
            if beat.duration.dots > 1 {
                self.loss("only one dot on a note is kept");
            }
        }
        if chord.is_some() {
            flags |= 0x02;
        }
        if !beat.text.is_empty() {
            flags |= 0x04;
        }
        if fx.0 != 0 || fx.1 != 0 {
            flags |= 0x08;
        }
        if tempo.is_some() {
            flags |= 0x10;
        }
        if tuplet.is_some() {
            flags |= 0x20;
        }
        if status {
            flags |= 0x40;
        }
        self.u8(flags);
        if status {
            self.u8(kind);
        }
        self.i8(value);
        if let Some(n) = tuplet {
            self.i32(n);
        }
        if let Some(c) = chord {
            self.chord(ti, c);
        }
        if flags & 0x04 != 0 {
            self.int_byte_size_string(&beat.text, "a beat's text");
        }
        if flags & 0x08 != 0 {
            self.beat_effects(beat, fx);
        }
        if let Some(t) = tempo {
            self.mix_table_change(t);
        }

        // The notes, by string.
        let mut notes: Vec<(u8, u8, &Note)> = Vec::new();
        if has_notes && beat.kind == BeatKind::Notes {
            let drum_strings = self.drum_strings(ti, beat);
            for (note, drum_string) in beat.notes.iter().zip(drum_strings) {
                if let Some((string, fret)) = self.place_note(ti, note, drum_string) {
                    if notes.iter().any(|(s, _, _)| *s == string) {
                        self.loss("a second note on the same string in one beat is left out");
                    } else {
                        notes.push((string, fret, note));
                    }
                }
            }
        }
        notes.sort_by_key(|(s, _, _)| *s);
        let mut string_flags = 0u8;
        for (string, _, _) in &notes {
            string_flags |= 1 << (7 - string);
        }
        self.u8(string_flags);
        let grace = self.grace_before(ti, graces, &notes);
        for (string, fret, note) in &notes {
            let g = grace.iter().find(|(s, _)| s == string).map(|(_, g)| g);
            self.note(ti, beat, *string, *fret, note, g);
        }
        self.i16(0); // beaming and the like
    }

    /// The strings of a beat of drums, which the model doesn't keep: each
    /// drum on its own string where the track has one for it, else the first
    /// string left in the beat, the one where the same drum was last
    /// struck for a tied drum (the file ties a note to the one before it on
    /// its string).
    fn drum_strings(&self, ti: usize, beat: &Beat) -> Vec<Option<u8>> {
        let Some(plan) = self.plans.get(ti).filter(|p| p.kind == Kind::Drums) else {
            return vec![None; beat.notes.len()];
        };
        let strings = plan.tuning.len();
        let last: [u8; MAX_STRINGS] = self.last_fret.get(ti).copied().unwrap_or([0; MAX_STRINGS]);
        let mut taken = vec![false; strings];
        let mut out: Vec<Option<usize>> = vec![None; beat.notes.len()];
        for (slot, note) in out.iter_mut().zip(&beat.notes) {
            if let NotePlace::Pitched(p) = note.place
                && let Some(s) = plan.home.get(&p.0).copied()
                && taken.get(s) == Some(&false)
            {
                if let Some(t) = taken.get_mut(s) {
                    *t = true;
                }
                *slot = Some(s);
            }
        }
        for (slot, note) in out.iter_mut().zip(&beat.notes) {
            let NotePlace::Pitched(p) = note.place else {
                continue;
            };
            if slot.is_some() {
                continue;
            }
            let free = |s: &usize| taken.get(*s) == Some(&false);
            let same = (0..strings)
                .filter(free)
                .find(|s| note.tie && last.get(*s).copied() == Some(p.0));
            if let Some(s) = same.or_else(|| (0..strings).find(free)) {
                if let Some(t) = taken.get_mut(s) {
                    *t = true;
                }
                *slot = Some(s);
            }
        }
        out.into_iter()
            .map(|s| s.and_then(|s| u8::try_from(s + 1).ok()))
            .collect()
    }

    /// The code of the duration and of the tuplet, if any.
    fn duration_codes(&mut self, beat: &Beat) -> (i8, Option<i32>) {
        let d = &beat.duration;
        let value = match d.value {
            NoteValue::DoubleWhole => {
                self.loss("a double whole note is written as a whole note");
                -2
            }
            NoteValue::Whole => -2,
            NoteValue::Half => -1,
            NoteValue::Quarter => 0,
            NoteValue::Eighth => 1,
            NoteValue::Sixteenth => 2,
            NoteValue::ThirtySecond => 3,
            NoteValue::SixtyFourth => 4,
            NoteValue::OneTwentyEighth | NoteValue::TwoFiftySixth => {
                self.loss("notes shorter than a sixty-fourth are written as sixty-fourths");
                4
            }
        };
        if d.tuplets.len() > 1 {
            self.loss("only one tuplet on a note is kept");
        }
        let tuplet = d.tuplets.first().and_then(|t| {
            let ok = matches!((t.notes, t.in_time_of), (3, 2) | (5..=7, 4) | (9..=13, 8));
            ok.then_some(i32::from(t.notes))
        });
        if !d.tuplets.is_empty() && tuplet.is_none() {
            self.loss("a tuplet other than 3:2, 5 to 7 in the time of 4, and 9 to 13 in the time of 8 is left out");
        }
        (value, tuplet)
    }

    // ---- the beat's effects ---------------------------------------------

    /// The two flag bytes of the beat's effects.
    fn beat_effects_flags(&mut self, beat: &Beat) -> (u8, u8) {
        let e = &beat.effects;
        let mut f1 = 0u8;
        let mut f2 = 0u8;
        if e.vibrato_wide {
            f1 |= 0x02;
        }
        if e.fade_in {
            f1 |= 0x10;
        }
        if e.technique.is_some() {
            f1 |= 0x20;
        }
        if e.stroke.is_some() {
            f1 |= 0x40;
        }
        if e.pick_stroke.is_some() {
            f2 |= 0x02;
        }
        if e.whammy.as_ref().is_some_and(|w| !w.points.is_empty()) {
            f2 |= 0x04;
        }
        if e.tremolo_picking.is_some() {
            self.kept_as("tremolo picking on a beat is written on each of its notes");
        }
        if e.let_ring {
            self.kept_as("let ring on a beat is written on each of its notes");
        }
        (f1, f2)
    }

    fn beat_effects(&mut self, beat: &Beat, (f1, f2): (u8, u8)) {
        let e = &beat.effects;
        self.u8(f1);
        self.u8(f2);
        if let Some(t) = e.technique {
            self.u8(match t {
                BeatTechnique::Tap => 1,
                BeatTechnique::Slap => 2,
                BeatTechnique::Pop => 3,
            });
        }
        if f2 & 0x04 != 0
            && let Some(w) = &e.whammy
        {
            self.bend(w, "a whammy bar curve");
        }
        if let Some(s) = e.stroke {
            // Version 5 writes the up speed first.
            let speed = self.stroke_code(s.speed);
            let (up, down) = match s.direction {
                StrokeDirection::Up => (speed, 0),
                StrokeDirection::Down => (0, speed),
            };
            self.i8(up);
            self.i8(down);
        }
        if let Some(p) = e.pick_stroke {
            self.i8(match p {
                StrokeDirection::Up => 1,
                StrokeDirection::Down => 2,
            });
        }
    }

    fn stroke_code(&mut self, speed: NoteValue) -> i8 {
        match speed {
            NoteValue::OneTwentyEighth => 1,
            NoteValue::SixtyFourth => 2,
            NoteValue::ThirtySecond => 3,
            NoteValue::Sixteenth => 4,
            NoteValue::Eighth => 5,
            NoteValue::Quarter => 6,
            NoteValue::TwoFiftySixth => {
                self.loss("a strum is written at the nearest speed Guitar Pro has");
                1
            }
            _ => {
                self.loss("a strum is written at the nearest speed Guitar Pro has");
                6
            }
        }
    }

    /// A bend or whammy curve: type, height, then the points.
    fn bend(&mut self, bend: &Bend, what: &str) {
        let mut points: Vec<(i32, i32)> = Vec::new();
        for p in &bend.points {
            let position = p.offset.numer() * BEND_POSITION_MAX;
            let sixtieths = (position + p.offset.denom() / 2).div_euclid(p.offset.denom());
            if position % p.offset.denom() != 0 {
                self.loss(&format!(
                    "{what}'s points are moved to the nearest sixtieth of the note"
                ));
            }
            if p.cents % BEND_CENTS_PER_UNIT != 0 {
                self.loss(&format!("{what}'s heights are kept to 2 cents"));
            }
            points.push((
                i32::try_from(sixtieths.clamp(0, BEND_POSITION_MAX)).unwrap_or(0),
                p.cents.div_euclid(BEND_CENTS_PER_UNIT)
                    + i32::from(p.cents.rem_euclid(BEND_CENTS_PER_UNIT) * 2 >= BEND_CENTS_PER_UNIT),
            ));
        }
        if points.len() > MAX_BEND_POINTS {
            self.loss(&format!(
                "{what} with more than {MAX_BEND_POINTS} points is cut"
            ));
            points.truncate(MAX_BEND_POINTS);
        }
        self.i8(1);
        let top = points.iter().map(|(_, v)| *v).max().unwrap_or(0);
        self.i32(top);
        self.i32(i32::try_from(points.len()).unwrap_or(0));
        for (position, value) in points {
            self.i32(position);
            self.i32(value);
            self.bool(false);
        }
    }

    /// A mix table change that holds only a tempo.
    fn mix_table_change(&mut self, t: &oxiguitar_model::TempoChange) {
        if !t.label.is_empty() {
            self.loss("the names of tempo changes are left out");
        }
        if t.linear {
            self.loss("a gradual tempo change is written as a sudden one");
        }
        self.i8(-1); // instrument
        for _ in 0..4 {
            self.i32(-1); // the RSE instrument
        }
        for _ in 0..6 {
            self.i8(-1); // volume, balance, chorus, reverb, phaser, tremolo
        }
        self.int_byte_size_string("", "a tempo's name");
        let bpm = self.tempo(t.bpm);
        self.i32(bpm);
        self.u8(0); // transition
        self.bool(false); // tempo hidden
        self.u8(0); // applies to all tracks
        self.i8(-1); // wah
        self.int_byte_size_string("", "a sound");
        self.int_byte_size_string("", "a sound");
    }

    // ---- chords ---------------------------------------------------------

    fn chord(&mut self, ti: usize, chord: &ChordDiagram) {
        let strings = self.plans.get(ti).map_or(6, |p| p.tuning.len());
        if !chord.frets.is_empty() && chord.frets.len() != strings {
            self.loss("a chord diagram has as many strings as its track");
        }
        self.bool(true); // the new layout
        self.bool(false); // sharp
        self.zeros(3);
        self.u8(0); // root
        self.u8(0); // type
        self.u8(0); // extension
        self.i32(0); // bass
        self.i32(0); // tonality
        self.bool(false); // add
        self.byte_size_string(&chord.name, 22, "a chord's name");
        self.zeros(3); // alterations
        self.i32(i32::from(chord.first_fret));
        for i in 0..7 {
            let fret = chord
                .frets
                .get(i)
                .filter(|_| i < strings)
                .copied()
                .flatten();
            self.i32(fret.map_or(-1, i32::from));
        }
        self.u8(0); // barres
        self.zeros(15);
        for kept in [false, false, false, true, true, true, true] {
            self.bool(kept); // the root, third and fifth
        }
        self.u8(0);
        for _ in 0..7 {
            self.i8(-1); // fingering
        }
        self.bool(true); // shown
    }

    // ---- notes ----------------------------------------------------------

    /// The grace note of each string: from the grace beat before the main
    /// one.
    fn grace_before(
        &mut self,
        ti: usize,
        graces: &[&Beat],
        notes: &[(u8, u8, &Note)],
    ) -> Vec<(u8, Grace)> {
        if graces.len() > 1 {
            self.loss("only one grace beat before a beat is kept");
        }
        let Some(beat) = graces.last() else {
            return Vec::new();
        };
        let on_beat = beat.grace == Some(GraceKind::OnBeat);
        let duration = match beat.duration.value {
            NoteValue::SixtyFourth => 1,
            NoteValue::ThirtySecond => 2,
            NoteValue::Sixteenth => 3,
            _ => {
                self.loss("a grace note is a sixteenth, thirty-second or sixty-fourth");
                2
            }
        };
        let mut out = Vec::new();
        let drum_strings = self.drum_strings(ti, beat);
        for (g, drum_string) in beat.notes.iter().zip(drum_strings) {
            let Some((string, fret)) = self.place_note(ti, g, drum_string) else {
                continue;
            };
            if !notes.iter().any(|(s, _, _)| *s == string) {
                self.loss("a grace note needs a note on its string after it");
                continue;
            }
            let fx = &g.effects;
            let mut plain = fx.clone();
            plain.dead = false;
            plain.hammer_pull = false;
            plain.slides.retain(|s| *s != Slide::Legato);
            if plain != oxiguitar_model::NoteEffects::default() {
                self.loss("a grace note holds only dead, slide and hammer-on effects");
            }
            let transition = if fx.slides.contains(&Slide::Legato) {
                1
            } else if fx.hammer_pull {
                3
            } else {
                0
            };
            out.push((
                string,
                Grace {
                    fret,
                    dynamic: g.dynamic,
                    transition,
                    duration,
                    dead: fx.dead,
                    on_beat,
                },
            ));
        }
        out
    }

    fn note(
        &mut self,
        ti: usize,
        beat: &Beat,
        string: u8,
        fret: u8,
        note: &Note,
        grace: Option<&Grace>,
    ) {
        let fx = &note.effects;
        let mut effects1 = 0u8;
        let mut effects2 = 0u8;
        let bend = fx.bend.as_ref().filter(|b| !b.points.is_empty());
        if bend.is_some() {
            effects1 |= 0x01;
        }
        if fx.hammer_pull {
            effects1 |= 0x02;
        }
        if fx.let_ring || beat.effects.let_ring {
            effects1 |= 0x08;
        }
        if grace.is_some() {
            effects1 |= 0x10;
        }
        if fx.staccato {
            effects2 |= 0x01;
        }
        if fx.palm_mute {
            effects2 |= 0x02;
        }
        let tremolo = fx.tremolo_picking.or(beat.effects.tremolo_picking);
        if tremolo.is_some() {
            effects2 |= 0x04;
        }
        if !fx.slides.is_empty() {
            effects2 |= 0x08;
        }
        let harmonic = fx.harmonic.filter(|h| {
            if *h == Harmonic::Feedback {
                self.loss("feedback harmonics are left out");
                false
            } else {
                true
            }
        });
        if harmonic.is_some() {
            effects2 |= 0x10;
        }
        if fx.trill.is_some() {
            effects2 |= 0x20;
        }
        if fx.vibrato {
            effects2 |= 0x40;
        }
        if fx.left_hand_tap {
            self.loss("left-hand tapping on a note is left out");
        }

        let left = self.finger(fx.left_finger);
        let right = self.finger(fx.right_finger);
        let mut flags = 0x20u8; // the type and the fret
        if fx.ghost {
            flags |= 0x04;
        }
        if effects1 != 0 || effects2 != 0 {
            flags |= 0x08;
        }
        if note.dynamic != Dynamic::F {
            flags |= 0x10;
        }
        match fx.accent {
            Some(Accent::Normal) => flags |= 0x40,
            Some(Accent::Heavy) => flags |= 0x02,
            None => {}
        }
        if left >= 0 || right >= 0 {
            flags |= 0x80;
        }
        self.u8(flags);
        // The file ties a note to the one before it on its string; where
        // that is not the note's own fret, the note is struck again.
        let slot = usize::from(string).saturating_sub(1);
        let before = self
            .last_fret
            .get(ti)
            .and_then(|s| s.get(slot))
            .copied()
            .unwrap_or(0);
        let tie = note.tie && before == fret;
        if note.tie && !tie {
            self.loss(
                "a tie to a note that isn't before it on its string is written as a new note",
            );
        }
        let kind = if tie {
            if fx.dead {
                self.loss("a note can't be both tied and dead");
            }
            2
        } else if fx.dead {
            3
        } else {
            1
        };
        self.u8(kind);
        if flags & 0x10 != 0 {
            self.i8(dynamic_code(note.dynamic));
        }
        self.i8(i8::try_from(fret).unwrap_or(0));
        if let Some(s) = self.last_fret.get_mut(ti).and_then(|s| s.get_mut(slot)) {
            *s = fret;
        }
        if flags & 0x80 != 0 {
            self.i8(left);
            self.i8(right);
        }
        self.u8(0); // accidental spelling
        if flags & 0x08 != 0 {
            self.u8(effects1);
            self.u8(effects2);
            if let Some(b) = bend {
                self.bend(b, "a bend");
            }
            if let Some(g) = grace {
                self.i8(i8::try_from(g.fret).unwrap_or(0));
                self.i8(dynamic_code(g.dynamic));
                self.i8(g.transition);
                self.u8(g.duration);
                self.u8(u8::from(g.dead) | if g.on_beat { 0x02 } else { 0 });
            }
            if let Some(t) = tremolo {
                let code = match t {
                    NoteValue::Eighth => 1,
                    NoteValue::Sixteenth => 2,
                    NoteValue::ThirtySecond => 3,
                    other => {
                        self.loss(
                            "tremolo picking is written in eighths, sixteenths or thirty-seconds",
                        );
                        if other < NoteValue::Eighth { 1 } else { 3 }
                    }
                };
                self.i8(code);
            }
            if effects2 & 0x08 != 0 {
                let mut bits = 0u8;
                for s in &fx.slides {
                    bits |= match s {
                        Slide::Shift => 0x01,
                        Slide::Legato => 0x02,
                        Slide::OutDown => 0x04,
                        Slide::OutUp => 0x08,
                        Slide::InFromBelow => 0x10,
                        Slide::InFromAbove => 0x20,
                    };
                }
                self.u8(bits);
            }
            if let Some(h) = harmonic {
                self.harmonic(ti, string, fret, h);
            }
            if let Some(t) = fx.trill {
                if t.fret > 99 {
                    self.loss("a trill above fret 99 is changed");
                }
                self.i8(i8::try_from(t.fret.min(99)).unwrap_or(0));
                let period = match t.speed {
                    NoteValue::Sixteenth => 1,
                    NoteValue::ThirtySecond => 2,
                    NoteValue::SixtyFourth => 3,
                    other => {
                        self.loss(
                            "trills are written in sixteenths, thirty-seconds or sixty-fourths",
                        );
                        if other < NoteValue::Sixteenth { 1 } else { 3 }
                    }
                };
                self.i8(period);
            }
        }
    }

    /// A finger as the file holds it: -1 for none, 0 (thumb) to 4.
    fn finger(&mut self, finger: Option<u8>) -> i8 {
        match finger {
            None => -1,
            Some(f) if f <= 4 => i8::try_from(f).unwrap_or(-1),
            Some(_) => {
                self.loss("a finger other than thumb to little finger is left out");
                -1
            }
        }
    }

    fn harmonic(&mut self, ti: usize, string: u8, fret: u8, h: Harmonic) {
        match h {
            Harmonic::Natural => self.i8(1),
            Harmonic::Artificial { semitones } => {
                self.i8(2);
                // The fretted note's pitch class, then the octave.
                let tuning = self.plans.get(ti).map_or(0, |p| {
                    p.tuning
                        .get(usize::from(string).saturating_sub(1))
                        .copied()
                        .unwrap_or(0)
                });
                let capo = self.plans.get(ti).map_or(0, |p| p.capo);
                let class = (u16::from(tuning) + u16::from(capo) + u16::from(fret)) % 12;
                let (just, accidental) = match class {
                    0 => (0, 0),
                    1 => (0, 1),
                    2 => (1, 0),
                    3 => (1, 1),
                    4 => (2, 0),
                    5 => (3, 0),
                    6 => (3, 1),
                    7 => (4, 0),
                    8 => (4, 1),
                    9 => (5, 0),
                    10 => (5, 1),
                    _ => (6, 0),
                };
                self.u8(just);
                self.i8(accidental);
                let octave = match semitones {
                    12 => 1,
                    24 => 2,
                    36 => 3,
                    other => {
                        self.loss("artificial harmonics are written an octave, two or three above");
                        (other / 12).clamp(1, 3)
                    }
                };
                self.u8(octave);
            }
            Harmonic::Tapped { fret } => {
                self.i8(3);
                self.u8(fret);
            }
            Harmonic::Pinch => self.i8(4),
            Harmonic::Semi => self.i8(5),
            Harmonic::Feedback => {}
        }
    }
}

/// The string (1 = highest) and fret of a note in a written track, or why it
/// is left out. A drum is written on `drum_string`, which keeps the drums
/// of a beat on different strings.
fn placement(
    plan: &TrackPlan,
    note: &Note,
    drum_string: Option<u8>,
) -> Result<(u8, u8), &'static str> {
    let strings = plan.tuning.len();
    match (plan.kind, note.place) {
        (Kind::Drums, NotePlace::Pitched(p)) => {
            if p.0 > 99 {
                return Err("a drum above 99 is left out");
            }
            drum_string
                .filter(|s| usize::from(*s) <= strings)
                .map(|s| (s, p.0))
                .ok_or("a beat of drums holds as many notes as the track has strings")
        }
        (_, NotePlace::Pitched(p)) => {
            // The string with the lowest fret that reaches the pitch.
            plan.tuning
                .iter()
                .enumerate()
                .filter_map(|(i, open)| {
                    let base = u16::from(*open) + u16::from(plan.capo);
                    Some((i, u16::from(p.0).checked_sub(base)?))
                })
                .min_by_key(|(_, fret)| *fret)
                .and_then(|(i, fret)| Some((u8::try_from(i + 1).ok()?, u8::try_from(fret).ok()?)))
                .filter(|(_, fret)| *fret <= 99)
                .ok_or("a note below the lowest string, or above fret 99, is left out")
        }
        (_, NotePlace::Fretted { string, fret }) => {
            if string == 0 || usize::from(string) > strings {
                Err(if strings == 0 {
                    "a note on a string of a track without strings is left out"
                } else {
                    "a note on a string the track does not have is left out"
                })
            } else if fret > 99 {
                Err("a note above fret 99 is left out")
            } else {
                Ok((string, fret))
            }
        }
    }
}

/// Takes a track's two channels from the table: any two of a port's, or a
/// port's tenth for the drums.
fn allocate(used: &mut [bool; CHANNELS], drums: bool) -> Option<(usize, usize)> {
    for port in 0..CHANNELS / 16 {
        let mut free = (0..16)
            .map(|c| port * 16 + c)
            .filter(|i| i % 16 != 9 && !used.get(*i).copied().unwrap_or(true));
        if drums {
            let main = port * 16 + 9;
            if used.get(main).copied().unwrap_or(true) {
                continue;
            }
            let effect = free.next();
            for i in [Some(main), effect].into_iter().flatten() {
                if let Some(u) = used.get_mut(i) {
                    *u = true;
                }
            }
            return Some((main, effect.unwrap_or(main)));
        }
        if let (Some(a), Some(b)) = (free.next(), free.next()) {
            for i in [a, b] {
                if let Some(u) = used.get_mut(i) {
                    *u = true;
                }
            }
            return Some((a, b));
        }
    }
    None
}

/// The string each drum of a track is written on: the most played drums get
/// one string each, the highest drum on the first string.
fn drum_homes(track: &Track, strings: usize) -> std::collections::HashMap<u8, usize> {
    let mut count: std::collections::HashMap<u8, usize> = Default::default();
    for p in track
        .staves
        .first()
        .into_iter()
        .flat_map(|s| &s.bars)
        .flat_map(|b| b.voices.iter().take(2))
        .flat_map(|v| &v.beats)
        .flat_map(|b| &b.notes)
        .filter_map(|n| match n.place {
            NotePlace::Pitched(p) => Some(p.0),
            NotePlace::Fretted { .. } => None,
        })
    {
        *count.entry(p).or_default() += 1;
    }
    let mut drums: Vec<(u8, usize)> = count.into_iter().collect();
    drums.sort_by_key(|(p, n)| (std::cmp::Reverse(*n), *p));
    drums.truncate(strings);
    let mut kept: Vec<u8> = drums.into_iter().map(|(p, _)| p).collect();
    kept.sort_by_key(|p| std::cmp::Reverse(*p));
    kept.into_iter().enumerate().map(|(i, p)| (p, i)).collect()
}

/// A grace note, ready to write.
struct Grace {
    fret: u8,
    dynamic: Dynamic,
    transition: i8,
    duration: u8,
    dead: bool,
    on_beat: bool,
}

/// Guitar Pro dynamics run from 1 (ppp) to 8 (fff).
fn dynamic_code(d: Dynamic) -> i8 {
    match d {
        Dynamic::PPP => 1,
        Dynamic::PP => 2,
        Dynamic::P => 3,
        Dynamic::MP => 4,
        Dynamic::MF => 5,
        Dynamic::F => 6,
        Dynamic::FF => 7,
        Dynamic::FFF => 8,
    }
}

/// A volume (0 to 1) as a step of the channel table.
fn volume_step(volume: f32) -> u8 {
    // Clamped to 0 to 16, then a whole number.
    (volume * CHANNEL_STEPS).round().clamp(0.0, CHANNEL_STEPS) as u8
}

/// A pan (-1 to 1) as a step of the channel table: 8 is the middle.
fn pan_step(pan: f32) -> u8 {
    ((pan + 1.0) * CHANNEL_STEPS / 2.0)
        .round()
        .clamp(0.0, CHANNEL_STEPS) as u8
}

/// How Guitar Pro beams a time signature, in eighths for each of four
/// groups: a beat each, or threes in the compound times.
fn beams(t: oxiguitar_model::TimeSignature) -> [u8; 4] {
    let n = u32::from(t.numerator);
    let (per, groups) = match t.denominator {
        1 => (8, n),
        2 => (4, n),
        4 => (2, n),
        8 if n >= 6 && n % 3 == 0 => (3, n / 3),
        _ => (1, n),
    };
    let mut out = [0u8; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        if u32::try_from(i).is_ok_and(|i| i < groups) {
            *slot = per;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_of_the_table() {
        assert_eq!(volume_step(0.875), 14);
        assert_eq!(volume_step(1.0), 16);
        assert_eq!(pan_step(0.0), 8);
        assert_eq!(pan_step(-1.0), 0);
        assert_eq!(pan_step(1.0), 16);
    }

    #[test]
    fn beams_follow_the_beat() {
        let t = |n, d| oxiguitar_model::TimeSignature {
            numerator: n,
            denominator: d,
        };
        assert_eq!(beams(t(4, 4)), [2, 2, 2, 2]);
        assert_eq!(beams(t(3, 4)), [2, 2, 2, 0]);
        assert_eq!(beams(t(6, 8)), [3, 3, 0, 0]);
    }
}
