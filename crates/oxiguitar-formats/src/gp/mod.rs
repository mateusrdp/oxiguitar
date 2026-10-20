// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reader for the binary Guitar Pro 3, 4 and 5 formats (.gp3, .gp4, .gp5).
//!
//! The byte layout follows what PyGuitarPro (LGPL-3.0), alphaTab (MPL-2.0)
//! and TuxGuitar (LGPL-2.1) document in their readers; no code is copied
//! from them. `docs/formats/guitar-pro-3-5.md` describes the layout and the
//! places where those sources disagree.

mod effects;
#[cfg(feature = "gp5")]
mod write;

#[cfg(feature = "gp5")]
pub(crate) use write::write as write_gp5;

use oxiguitar_model::{
    Bar, Beat, BeatKind, ChordDiagram, Clef, Color, Direction, Duration, Dynamic, GraceKind,
    Instrument, KeyMode, KeySignature, LyricLine, Marker, MasterBar, Mix, Note, NotePlace,
    NoteValue, Pitch, Ratio, Score, ScoreInfo, Staff, TempoChange, TimeSignature, Track,
    TripletFeel, Tuplet, Voice,
};

use crate::Format;
use crate::binary::{Cursor, Result};
use crate::error::{ImportLog, ReadError, ReadErrorKind};

/// Upper limits that no real file reaches; anything above means damage.
const MAX_BARS: usize = 20_000;
const MAX_TRACKS: usize = 256;
const MAX_BEATS_PER_VOICE: usize = 4_096;
const MAX_NOTICES: usize = 1_000;
const MAX_STRINGS: usize = 7;

/// A Guitar Pro version as (major, minor): "5.10" is (5, 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u8, u8);

impl Version {
    fn parse(text: &str) -> Option<Version> {
        let (major, minor) = text.split_once('.')?;
        Some(Version(
            major.trim().parse().ok()?,
            minor.trim().parse().ok()?,
        ))
    }
}

const V5_00: Version = Version(5, 0);

/// The steps of a MIDI channel's volume and balance in the channel table.
pub(crate) const CHANNEL_STEPS: f32 = 16.0;

/// What one MIDI channel of the file's 64-channel table holds.
#[derive(Clone, Copy)]
struct Channel {
    program: i32,
    volume: u8,
    balance: u8,
}

impl Default for Channel {
    /// A track whose channel the file doesn't have: Guitar Pro's usual
    /// volume (13 of 16) and the middle of the pan.
    fn default() -> Channel {
        Channel {
            program: 0,
            volume: 13,
            balance: 8,
        }
    }
}

/// A grace note waiting to be turned into a grace beat before its main beat.
struct PendingGrace {
    note: Note,
    duration: NoteValue,
    on_beat: bool,
}

/// The result of reading a file.
pub(crate) struct GpRead {
    pub(crate) score: Score,
    pub(crate) log: ImportLog,
    pub(crate) trailing_bytes: usize,
}

struct Reader<'a> {
    c: Cursor<'a>,
    version: Version,
    log: ImportLog,
    /// Last fret seen on each string of each track, to resolve tied notes,
    /// which the file stores without a fret.
    last_fret: Vec<[u8; MAX_STRINGS]>,
    /// Distinct chord diagrams found so far, per track.
    chords: Vec<Vec<ChordDiagram>>,
    /// Where the current beat is, for log messages: (bar, track), 1-based.
    place: (usize, usize),
}

/// Reads a Guitar Pro 3, 4 or 5 file.
pub(crate) fn read(data: &[u8], format: &Format) -> std::result::Result<GpRead, ReadError> {
    let (version_text, major) = match format {
        Format::GuitarPro3 { version } => (version, 3),
        Format::GuitarPro4 { version } => (version, 4),
        Format::GuitarPro5 { version } => (version, 5),
        other => {
            return Err(ReadError::new(
                ReadErrorKind::Unsupported(other.describe()),
                None,
            ));
        }
    };
    // The version's major number must be the one the header names (so
    // "5.10", not "50.1"), and one this build reads.
    let version = Version::parse(version_text)
        .filter(|v| v.0 == major && reads(major))
        .ok_or_else(|| {
            ReadError::new(
                ReadErrorKind::Invalid(format!("unknown Guitar Pro version \"{version_text}\"")),
                Some(1),
            )
        })?;
    let mut r = Reader {
        c: Cursor::new(data),
        version,
        log: ImportLog::default(),
        last_fret: Vec::new(),
        chords: Vec::new(),
        place: (0, 0),
    };
    let score = r.song()?;
    Ok(GpRead {
        score,
        trailing_bytes: r.c.remaining(),
        log: r.log,
    })
}

/// Whether this build reads Guitar Pro `major` (its Cargo feature is on).
const fn reads(major: u8) -> bool {
    (major == 3 && cfg!(feature = "gp3"))
        || (major == 4 && cfg!(feature = "gp4"))
        || (major == 5 && cfg!(feature = "gp5"))
}

impl Reader<'_> {
    fn major(&self) -> u8 {
        self.version.0
    }

    /// Whether the file is Guitar Pro `major`. Never true for a version this
    /// build doesn't read, so the compiler leaves that version's code out of
    /// a plugin built for another version.
    fn is(&self, major: u8) -> bool {
        reads(major) && self.version.0 == major
    }

    /// Guitar Pro 3 or 4.
    fn before_v5(&self) -> bool {
        self.is(3) || self.is(4)
    }

    /// Guitar Pro 4 or 5.
    fn since_v4(&self) -> bool {
        self.is(4) || self.is(5)
    }

    fn here(&self) -> String {
        format!("bar {}, track {}", self.place.0, self.place.1)
    }

    fn song(&mut self) -> Result<Score> {
        let header = self.c.byte_size_string(30)?;
        if header.starts_with("CLIPBOARD") {
            return Err(ReadError::new(
                ReadErrorKind::Unsupported("Guitar Pro clipboard data".into()),
                Some(0),
            ));
        }
        let mut score = Score::new();
        score.info = self
            .info()
            .map_err(|e| e.within(|| "score information".into()))?;

        let mut song_triplet_feel = TripletFeel::None;
        if self.before_v5() && self.c.bool()? {
            song_triplet_feel = TripletFeel::Triplet8th;
        }
        let lyrics = if self.since_v4() {
            Some(self.lyrics().map_err(|e| e.within(|| "lyrics".into()))?)
        } else {
            None
        };
        let song_key: i32;
        if self.is(5) {
            if self.version > V5_00 {
                // RSE master effect: volume, an unknown int, 11 equaliser bands.
                self.c.skip(4 + 4 + 11)?;
            }
            self.page_setup()
                .map_err(|e| e.within(|| "page setup".into()))?;
            score.tempo_label = self.c.int_byte_size_string()?;
            score.initial_tempo = f64::from(self.c.i32()?);
            if self.version > V5_00 {
                self.c.bool()?; // tempo hidden
            }
            song_key = i32::from(self.c.i8()?);
            self.c.i32()?; // octave, unused
        } else {
            score.initial_tempo = f64::from(self.c.i32()?);
            song_key = self.c.i32()?;
            if self.is(4) {
                self.c.i8()?; // octave, unused
            }
        }
        let channels = self
            .channels()
            .map_err(|e| e.within(|| "MIDI channel table".into()))?;
        let directions = if self.is(5) {
            let d = self.directions()?;
            self.c.i32()?; // master reverb
            d
        } else {
            Vec::new()
        };

        let bar_count = self.c.count("number of bars", MAX_BARS, 1)?;
        let track_count = self.c.count("number of tracks", MAX_TRACKS, 1)?;

        let first_key = KeySignature {
            fifths: i8::try_from(song_key.clamp(-7, 7)).unwrap_or(0),
            mode: KeyMode::Major,
        };
        for i in 0..bar_count {
            let mb = self
                .master_bar(i, &score.master_bars, first_key, song_triplet_feel)
                .map_err(|e| e.within(|| format!("header of bar {}", i + 1)))?;
            score.master_bars.push(mb);
        }
        for (bar, dir) in directions {
            if let Some(mb) = score.master_bars.get_mut(bar) {
                mb.directions.push(dir);
            }
        }

        self.last_fret = vec![[0; MAX_STRINGS]; track_count];
        self.chords = vec![Vec::new(); track_count];
        for i in 0..track_count {
            let track = self
                .track(i, &channels)
                .map_err(|e| e.within(|| format!("track {}", i + 1)))?;
            score.tracks.push(track);
        }
        if self.is(5) {
            self.c.skip(if self.version == V5_00 { 2 } else { 1 })?;
        }

        self.bars(&mut score)?;
        for (track, chords) in score
            .tracks
            .iter_mut()
            .zip(std::mem::take(&mut self.chords))
        {
            track.chords = chords;
        }

        if let Some((track_choice, lines)) = lyrics {
            let target = usize::try_from(track_choice)
                .ok()
                .and_then(|t| t.checked_sub(1));
            match target.and_then(|t| score.tracks.get_mut(t)) {
                Some(track) => track.lyrics = lines,
                None if !lines.is_empty() => {
                    self.log
                        .warn("lyrics belong to a track that does not exist", "lyrics");
                }
                None => {}
            }
        }
        Ok(score)
    }

    fn info(&mut self) -> Result<ScoreInfo> {
        let mut info = ScoreInfo {
            title: self.c.int_byte_size_string()?,
            subtitle: self.c.int_byte_size_string()?,
            artist: self.c.int_byte_size_string()?,
            album: self.c.int_byte_size_string()?,
            ..ScoreInfo::default()
        };
        if self.is(5) {
            info.words = self.c.int_byte_size_string()?;
            info.music = self.c.int_byte_size_string()?;
        } else {
            // Before version 5 there is one "author" field for both.
            info.music = self.c.int_byte_size_string()?;
            info.words = info.music.clone();
        }
        info.copyright = self.c.int_byte_size_string()?;
        info.transcriber = self.c.int_byte_size_string()?;
        info.instructions = self.c.int_byte_size_string()?;
        let n = self.c.count("number of notice lines", MAX_NOTICES, 4)?;
        for _ in 0..n {
            info.notices.push(self.c.int_byte_size_string()?);
        }
        Ok(info)
    }

    /// Lyrics: the track they belong to, then five lines, each with the bar
    /// it starts on.
    fn lyrics(&mut self) -> Result<(i32, Vec<LyricLine>)> {
        let track = self.c.i32()?;
        let mut lines = Vec::new();
        for _ in 0..5 {
            let start = self.c.i32()?;
            let text = self.c.int_size_string()?;
            if !text.trim().is_empty() {
                lines.push(LyricLine {
                    start_bar: usize::try_from(start.saturating_sub(1)).unwrap_or(0),
                    text,
                });
            }
        }
        Ok((track, lines))
    }

    fn page_setup(&mut self) -> Result<()> {
        // Page size (2), margins (4), score size (1); then header/footer flags.
        self.c.skip(7 * 4 + 2)?;
        // Ten header and footer templates: title, subtitle, artist, album,
        // words, music, words and music, copyright (two lines), page number.
        for _ in 0..10 {
            self.c.int_byte_size_string()?;
        }
        self.log.info(
            "page setup (paper size, margins, header templates) is not used yet",
            "page setup",
        );
        Ok(())
    }

    fn channels(&mut self) -> Result<Vec<Channel>> {
        let mut channels = Vec::with_capacity(64);
        for _ in 0..64 {
            let program = self.c.i32()?;
            let volume = self.c.u8()?;
            let balance = self.c.u8()?;
            self.c.skip(4 + 2)?; // chorus, reverb, phaser, tremolo; 2 unused
            channels.push(Channel {
                program,
                volume,
                balance,
            });
        }
        Ok(channels)
    }

    /// Guitar Pro 5 stores navigation signs as a table of 19 bar numbers.
    fn directions(&mut self) -> Result<Vec<(usize, Direction)>> {
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
        let mut out = Vec::new();
        for dir in ORDER {
            let bar = self.c.i16()?;
            if bar >= 1 {
                out.push((usize::try_from(bar - 1).unwrap_or(0), dir));
            }
        }
        Ok(out)
    }

    fn master_bar(
        &mut self,
        index: usize,
        previous: &[MasterBar],
        first_key: KeySignature,
        song_triplet_feel: TripletFeel,
    ) -> Result<MasterBar> {
        if self.is(5) && index > 0 {
            self.c.skip(1)?;
        }
        let prev = previous.last();
        let mut mb = MasterBar {
            time_signature: prev.map_or(TimeSignature::COMMON, |p| p.time_signature),
            key_signature: prev.map_or(first_key, |p| p.key_signature),
            triplet_feel: song_triplet_feel,
            ..MasterBar::default()
        };
        let flags = self.c.u8()?;
        if flags & 0x01 != 0 {
            let at = self.c.pos();
            let n = self.c.u8()?;
            if n == 0 {
                return Err(ReadError::invalid("time signature with 0 beats", at));
            }
            mb.time_signature.numerator = n;
        }
        if flags & 0x02 != 0 {
            let at = self.c.pos();
            let d = self.c.u8()?;
            if !d.is_power_of_two() {
                return Err(ReadError::invalid(
                    format!("time signature denominator {d}"),
                    at,
                ));
            }
            mb.time_signature.denominator = d;
        }
        mb.repeat_start = flags & 0x04 != 0;
        if flags & 0x08 != 0 {
            let n = self.c.i8()?;
            // Guitar Pro 5 stores how many times the section plays; earlier
            // versions store how many times it repeats.
            let plays = if self.is(5) {
                i32::from(n)
            } else {
                i32::from(n) + 1
            };
            mb.repeat_end = Some(u8::try_from(plays.clamp(2, 99)).unwrap_or(2));
        }
        if self.before_v5() && flags & 0x10 != 0 {
            mb.alternate_endings = alternate_endings_v3(self.c.u8()?, previous);
        }
        if flags & 0x20 != 0 {
            mb.marker = Some(Marker {
                letter: String::new(),
                text: self.c.int_byte_size_string()?,
                color: self.color()?,
            });
        }
        if flags & 0x40 != 0 {
            let fifths = self.c.i8()?;
            let minor = self.c.i8()?;
            mb.key_signature = KeySignature {
                fifths: fifths.clamp(-7, 7),
                mode: if minor != 0 {
                    KeyMode::Minor
                } else {
                    KeyMode::Major
                },
            };
        }
        mb.double_bar = flags & 0x80 != 0;
        if self.is(5) {
            if flags & 0x03 != 0 {
                self.c.skip(4)?; // beam grouping
            }
            // The alternate-endings byte: a bit set in version 5.
            let endings = self.c.u8()?;
            if flags & 0x10 != 0 {
                mb.alternate_endings = u16::from(endings);
            }
            mb.triplet_feel = match self.c.u8()? {
                1 => TripletFeel::Triplet8th,
                2 => TripletFeel::Triplet16th,
                _ => TripletFeel::None,
            };
        }
        Ok(mb)
    }

    fn color(&mut self) -> Result<Color> {
        let b = self.c.bytes(4)?;
        Ok(Color {
            r: b.first().copied().unwrap_or(0),
            g: b.get(1).copied().unwrap_or(0),
            b: b.get(2).copied().unwrap_or(0),
        })
    }

    fn track(&mut self, index: usize, channels: &[Channel]) -> Result<Track> {
        if self.is(5) && (index == 0 || self.version == V5_00) {
            self.c.skip(1)?;
        }
        let flags = self.c.u8()?;
        let name = self.c.byte_size_string(40)?;
        let at = self.c.pos();
        let string_count = self.c.i32()?;
        let string_count = usize::try_from(string_count)
            .ok()
            .filter(|n| (1..=MAX_STRINGS).contains(n))
            .ok_or_else(|| ReadError::invalid(format!("{string_count} strings"), at))?;
        let mut tuning = Vec::with_capacity(string_count);
        for i in 0..MAX_STRINGS {
            let t = self.c.i32()?;
            if i < string_count {
                tuning.push(oxiguitar_model::Pitch(
                    u8::try_from(t.clamp(0, 127)).unwrap_or(0),
                ));
            }
        }
        let port = self.c.i32()?;
        let channel = self.c.i32()?;
        self.c.i32()?; // effect channel
        let fret_count = self.c.i32()?;
        let capo = self.c.i32()?;
        let color = self.color()?;

        let mut staff = Staff {
            tuning,
            capo: u8::try_from(capo.clamp(0, 30)).unwrap_or(0),
            fret_count: u8::try_from(fret_count.clamp(1, 36)).unwrap_or(24),
            ..Staff::default()
        };
        if self.is(5) {
            let flags2 = self.c.i16()?;
            staff.show_tablature = flags2 & 0x01 != 0;
            staff.show_standard = flags2 & 0x02 != 0;
            if !staff.show_tablature && !staff.show_standard {
                staff.show_tablature = true;
            }
            self.c.u8()?; // automatic accentuation
            self.c.u8()?; // MIDI bank
            self.track_rse()?;
        }

        let ch_index = usize::try_from(port.saturating_sub(1))
            .ok()
            .zip(usize::try_from(channel.saturating_sub(1)).ok())
            .map(|(p, c)| p * 16 + c);
        let ch = ch_index
            .and_then(|i| channels.get(i))
            .copied()
            .unwrap_or_default();
        let percussion = flags & 0x01 != 0 || channel == 10;
        // The channel table counts in sixteenths: 16 is full volume and
        // 8 the middle of the pan (confirmed on the corpus, where a typical
        // track has volume 14 and balance 8). Mute and solo are track flags
        // in Guitar Pro 5 only.
        let mix = Mix {
            volume: (f32::from(ch.volume.min(16)) / CHANNEL_STEPS).clamp(0.0, 1.0),
            pan: ((f32::from(ch.balance.min(16)) - CHANNEL_STEPS / 2.0) / (CHANNEL_STEPS / 2.0))
                .clamp(-1.0, 1.0),
            mute: self.is(5) && flags & 0x20 != 0,
            solo: self.is(5) && flags & 0x10 != 0,
        };
        Ok(Track {
            name,
            color,
            instrument: Instrument {
                midi_program: u8::try_from(ch.program.clamp(0, 127)).unwrap_or(0),
                percussion,
            },
            mix,
            staves: vec![staff],
            ..Track::default()
        })
    }

    fn track_rse(&mut self) -> Result<()> {
        self.c.u8()?; // humanize
        self.c.skip(24)?; // unknown
        self.rse_instrument()?;
        if self.version > V5_00 {
            self.c.skip(4)?; // equaliser
            self.c.int_byte_size_string()?; // effect name
            self.c.int_byte_size_string()?; // effect category
        }
        Ok(())
    }

    fn rse_instrument(&mut self) -> Result<()> {
        self.c.skip(4 * 3)?; // instrument, unknown, sound bank
        if self.version == V5_00 {
            self.c.skip(2 + 1)?; // effect number (short) and a blank byte
        } else {
            self.c.skip(4)?; // effect number
        }
        Ok(())
    }

    fn bars(&mut self, score: &mut Score) -> Result<()> {
        let bar_count = score.master_bars.len();
        for t in &mut score.tracks {
            if let Some(staff) = t.staves.first_mut() {
                staff.bars.reserve(bar_count);
            }
        }
        for bi in 0..bar_count {
            for ti in 0..score.tracks.len() {
                self.place = (bi + 1, ti + 1);
                let mut tempo_changes = Vec::new();
                let bar = self
                    .bar(ti, score, &mut tempo_changes)
                    .map_err(|e| e.within(|| format!("bar {}, track {}", bi + 1, ti + 1)))?;
                if let Some(staff) = score.tracks.get_mut(ti).and_then(|t| t.staves.first_mut()) {
                    staff.bars.push(bar);
                }
                if let Some(mb) = score.master_bars.get_mut(bi) {
                    for tc in tempo_changes {
                        if !mb.tempo_changes.iter().any(|x| x.position == tc.position) {
                            mb.tempo_changes.push(tc);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn bar(&mut self, ti: usize, score: &Score, tempo: &mut Vec<TempoChange>) -> Result<Bar> {
        let percussion = score
            .tracks
            .get(ti)
            .is_some_and(|t| t.instrument.percussion);
        let string_count = score
            .tracks
            .get(ti)
            .and_then(|t| t.staves.first())
            .map_or(6, Staff::string_count);
        let mut bar = Bar {
            clef: score.tracks.get(ti).map_or(Clef::Treble8vb, clef_for),
            voices: Vec::new(),
        };
        let voices = if self.is(5) { 2 } else { 1 };
        for vi in 0..voices {
            let voice = self.voice(ti, string_count, vi == 0, tempo)?;
            if vi == 0 || !voice.is_empty() {
                bar.voices.push(voice);
            }
        }
        if percussion {
            // Guitar Pro 3–5 store drum notes as string and fret, the fret
            // being the General MIDI drum. The model keeps them as pitches.
            for note in bar
                .voices
                .iter_mut()
                .flat_map(|v| &mut v.beats)
                .flat_map(|b| &mut b.notes)
            {
                if let NotePlace::Fretted { fret, .. } = note.place {
                    note.place = NotePlace::Pitched(Pitch(fret.min(127)));
                }
            }
        }
        if self.is(5) {
            // Line break marker; the last bar of a file may omit it.
            if self.c.remaining() > 0 {
                self.c.u8()?;
            }
        }
        Ok(bar)
    }

    fn voice(
        &mut self,
        ti: usize,
        string_count: usize,
        first_voice: bool,
        tempo: &mut Vec<TempoChange>,
    ) -> Result<Voice> {
        let n = self.c.count("number of beats", MAX_BEATS_PER_VOICE, 2)?;
        let mut voice = Voice::default();
        let mut position = Ratio::ZERO;
        for _ in 0..n {
            let (beat, graces, bpm) = self.beat(ti, string_count)?;
            if let Some(bpm) = bpm.filter(|_| first_voice) {
                tempo.push(TempoChange {
                    position,
                    bpm: f64::from(bpm),
                    label: String::new(),
                    linear: false,
                });
            }
            if !graces.is_empty() {
                voice.beats.push(grace_beat(graces));
            }
            position = beat
                .duration
                .length()
                .and_then(|l| position.checked_add(l))
                .unwrap_or(position);
            voice.beats.push(beat);
        }
        Ok(voice)
    }

    /// Reads one beat. Returns the beat, any grace notes to place before it,
    /// and a tempo change if the beat carries one.
    fn beat(
        &mut self,
        ti: usize,
        string_count: usize,
    ) -> Result<(Beat, Vec<PendingGrace>, Option<i32>)> {
        let flags = self.c.u8()?;
        let mut beat = Beat::default();
        if flags & 0x40 != 0 {
            beat.kind = match self.c.u8()? {
                0x00 => BeatKind::Empty,
                _ => BeatKind::Rest,
            };
        }
        beat.duration = self.duration(flags)?;
        if flags & 0x02 != 0 {
            let chord = self.chord(string_count)?;
            beat.chord = Some(self.add_chord(ti, chord));
        }
        if flags & 0x04 != 0 {
            beat.text = self.c.int_byte_size_string()?;
        }
        let mut beat_note_fx = effects::BeatNoteEffects::default();
        if flags & 0x08 != 0 {
            beat_note_fx = self.beat_effects(&mut beat.effects)?;
        }
        let mut bpm = None;
        if flags & 0x10 != 0 {
            bpm = self.mix_table_change()?;
        }

        let string_flags = self.c.u8()?;
        let mut graces = Vec::new();
        for string in 1..=7u8 {
            if string_flags & (1 << (7 - string)) == 0 {
                continue;
            }
            let (mut note, grace) = self.note(ti, string)?;
            if usize::from(string) > string_count {
                self.log.warn(
                    "a note on a string the track does not have was dropped",
                    self.here(),
                );
                continue;
            }
            beat_note_fx.apply(&mut note);
            if let Some(g) = grace {
                graces.push(g);
            }
            beat.notes.push(note);
        }
        if beat.kind == BeatKind::Notes && beat.notes.is_empty() {
            beat.kind = BeatKind::Rest;
        }

        if self.is(5) {
            let flags2 = self.c.i16()?;
            if flags2 & 0x0800 != 0 {
                self.c.u8()?; // secondary beam break
            }
        }
        Ok((beat, graces, bpm))
    }

    fn duration(&mut self, flags: u8) -> Result<Duration> {
        let at = self.c.pos();
        let value = match self.c.i8()? {
            -2 => NoteValue::Whole,
            -1 => NoteValue::Half,
            0 => NoteValue::Quarter,
            1 => NoteValue::Eighth,
            2 => NoteValue::Sixteenth,
            3 => NoteValue::ThirtySecond,
            4 => NoteValue::SixtyFourth,
            5 => NoteValue::OneTwentyEighth,
            other => {
                return Err(ReadError::invalid(
                    format!("note duration code {other}"),
                    at,
                ));
            }
        };
        let mut d = Duration::new(value);
        if flags & 0x01 != 0 {
            d.dots = 1;
        }
        if flags & 0x20 != 0 {
            let at = self.c.pos();
            let notes = self.c.i32()?;
            let in_time_of = match notes {
                3 => 2,
                5..=7 => 4,
                9..=13 => 8,
                1 => 1,
                _ => {
                    return Err(ReadError::invalid(format!("tuplet of {notes}"), at));
                }
            };
            if notes > 1 {
                d.tuplets.push(Tuplet {
                    notes: u8::try_from(notes).unwrap_or(3),
                    in_time_of,
                });
            }
        }
        Ok(d)
    }

    fn add_chord(&mut self, ti: usize, chord: ChordDiagram) -> usize {
        // Chord diagrams are stored per beat in the file; we keep one copy of
        // each distinct diagram per track.
        let Some(list) = self.chords.get_mut(ti) else {
            return 0;
        };
        if let Some(i) = list.iter().position(|c| *c == chord) {
            return i;
        }
        list.push(chord);
        list.len() - 1
    }

    fn mix_table_change(&mut self) -> Result<Option<i32>> {
        let instrument = self.c.i8()?;
        if self.is(5) {
            self.rse_instrument()?;
            if self.version == V5_00 {
                self.c.skip(1)?;
            }
        }
        // volume, balance, chorus, reverb, phaser, tremolo: -1 means unchanged
        let mut values = [0i8; 6];
        for v in &mut values {
            *v = self.c.i8()?;
        }
        if self.is(5) {
            self.c.int_byte_size_string()?; // tempo name
        }
        let tempo = self.c.i32()?;
        for v in values {
            if v >= 0 {
                self.c.u8()?; // transition duration
            }
        }
        if tempo >= 0 {
            self.c.u8()?; // transition duration
            if self.version > V5_00 {
                self.c.bool()?; // tempo hidden
            }
        }
        if self.since_v4() {
            self.c.u8()?; // "apply to all tracks" flags
        }
        if self.is(5) {
            self.c.i8()?; // wah
            if self.version > V5_00 {
                self.c.int_byte_size_string()?; // RSE effect
                self.c.int_byte_size_string()?; // RSE effect category
            }
        }
        if instrument >= 0 {
            self.log.warn(
                "instrument changes inside a track are not supported yet; the first instrument is kept",
                self.here(),
            );
        }
        if values.iter().any(|v| *v >= 0) {
            self.log.warn(
                "mixer changes inside a track (volume, pan, effects) are not supported yet",
                self.here(),
            );
        }
        Ok((tempo > 0).then_some(tempo))
    }

    fn note(&mut self, ti: usize, string: u8) -> Result<(Note, Option<PendingGrace>)> {
        let flags = self.c.u8()?;
        let mut note = Note::fretted(string, 0);
        let mut kind = 1u8;
        if flags & 0x20 != 0 {
            kind = self.c.u8()?;
        }
        if self.before_v5() && flags & 0x01 != 0 {
            self.c.skip(2)?; // independent duration and tuplet, unused by Guitar Pro itself
        }
        if flags & 0x10 != 0 {
            note.dynamic = dynamic_from_gp(self.c.i8()?);
        }
        let mut fret = 0u8;
        if flags & 0x20 != 0 {
            fret = u8::try_from(self.c.i8()?.clamp(0, 99)).unwrap_or(0);
        }
        if flags & 0x80 != 0 {
            let left = self.c.i8()?;
            let right = self.c.i8()?;
            note.effects.left_finger = u8::try_from(left).ok().filter(|f| *f <= 4);
            note.effects.right_finger = u8::try_from(right).ok().filter(|f| *f <= 4);
        }
        if self.is(5) {
            if flags & 0x01 != 0 {
                self.c.f64()?; // duration percent
            }
            self.c.u8()?; // flags2: accidental spelling
        }
        let slot = usize::from(string).saturating_sub(1);
        match kind {
            2 => {
                note.tie = true;
                fret = self
                    .last_fret
                    .get(ti)
                    .and_then(|s| s.get(slot))
                    .copied()
                    .unwrap_or(0);
            }
            3 => note.effects.dead = true,
            _ => {}
        }
        if let Some(s) = self.last_fret.get_mut(ti).and_then(|s| s.get_mut(slot)) {
            *s = fret;
        }
        note.place = NotePlace::Fretted { string, fret };
        note.effects.ghost = flags & 0x04 != 0;
        if flags & 0x40 != 0 {
            note.effects.accent = Some(oxiguitar_model::Accent::Normal);
        }
        if self.is(5) && flags & 0x02 != 0 {
            note.effects.accent = Some(oxiguitar_model::Accent::Heavy);
        }
        let mut grace = None;
        if flags & 0x08 != 0 {
            grace = self.note_effects(&mut note, string)?;
        }
        Ok((note, grace))
    }
}

/// Guitar Pro 3 and 4 store an alternate ending as a count: the bar belongs to
/// every ending up to that number not already taken since the repeat start.
fn alternate_endings_v3(value: u8, previous: &[MasterBar]) -> u16 {
    let value = u32::from(value.min(16));
    let mut taken = 0u16;
    for mb in previous.iter().rev() {
        if mb.repeat_start {
            break;
        }
        taken |= mb.alternate_endings;
    }
    let all = u16::try_from((1u32 << value) - 1).unwrap_or(u16::MAX);
    all & !taken
}

/// Guitar Pro dynamics run from 1 (ppp) to 8 (fff); 6 (f) is the default.
fn dynamic_from_gp(v: i8) -> Dynamic {
    match v {
        ..=1 => Dynamic::PPP,
        2 => Dynamic::PP,
        3 => Dynamic::P,
        4 => Dynamic::MP,
        5 => Dynamic::MF,
        6 => Dynamic::F,
        7 => Dynamic::FF,
        _ => Dynamic::FFF,
    }
}

fn grace_beat(graces: Vec<PendingGrace>) -> Beat {
    let value = graces
        .first()
        .map_or(NoteValue::ThirtySecond, |g| g.duration);
    let on_beat = graces.iter().any(|g| g.on_beat);
    Beat {
        kind: BeatKind::Notes,
        duration: Duration::new(value),
        notes: graces.into_iter().map(|g| g.note).collect(),
        grace: Some(if on_beat {
            GraceKind::OnBeat
        } else {
            GraceKind::BeforeBeat
        }),
        ..Beat::default()
    }
}

/// The clef of a Guitar Pro 3–5 track, which these formats do not store.
/// Guitar Pro decides it from the instrument, and so do we (Guitar Pro 7
/// writes these clefs when it converts such a file, as Manhattan in the
/// corpus shows): basses, and any track tuned below G1, in the bass clef an
/// octave down; guitars and other fretted instruments in the treble clef an
/// octave down; everything else (keyboards, voices, winds, strings) in the
/// treble clef at sounding pitch.
fn clef_for(track: &Track) -> Clef {
    let i = track.instrument;
    let lowest = track
        .staves
        .first()
        .and_then(|s| s.tuning.iter().map(|p| p.0).min());
    if i.percussion {
        Clef::Neutral
    } else if i.is_bass() || lowest.is_some_and(|p| p <= 31) {
        Clef::Bass8vb
    } else if i.is_fretted() {
        Clef::Treble8vb
    } else {
        Clef::Treble
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clefs_come_from_the_instrument() {
        let track = |program: u8, low: u8| Track {
            instrument: Instrument {
                midi_program: program,
                percussion: false,
            },
            staves: vec![Staff {
                tuning: vec![Pitch(low + 24), Pitch(low)],
                ..Staff::default()
            }],
            ..Track::default()
        };
        assert_eq!(clef_for(&track(25, 40)), Clef::Treble8vb);
        assert_eq!(clef_for(&track(33, 28)), Clef::Bass8vb);
        // A bass part on a track with a guitar program but a bass tuning.
        assert_eq!(clef_for(&track(24, 28)), Clef::Bass8vb);
        // Keyboards and voices tabbed on guitar tuning sound as written.
        assert_eq!(clef_for(&track(0, 40)), Clef::Treble);
        assert_eq!(clef_for(&track(53, 40)), Clef::Treble);
        let mut drums = track(0, 40);
        drums.instrument.percussion = true;
        assert_eq!(clef_for(&drums), Clef::Neutral);
    }

    #[test]
    fn versions() {
        assert_eq!(Version::parse("5.10"), Some(Version(5, 10)));
        assert_eq!(Version::parse("4.06"), Some(Version(4, 6)));
        assert!(Version(5, 10) > V5_00);
        assert_eq!(Version::parse("five"), None);
    }

    #[test]
    fn gp3_alternate_endings() {
        let mut bars = vec![MasterBar {
            repeat_start: true,
            ..MasterBar::default()
        }];
        // "1" → ending 1
        let first = alternate_endings_v3(1, &bars);
        assert_eq!(first, 0b01);
        bars.push(MasterBar {
            alternate_endings: first,
            ..MasterBar::default()
        });
        // "2" after ending 1 → ending 2 only
        assert_eq!(alternate_endings_v3(2, &bars), 0b10);
    }
}
