// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reader for GPIF, the XML score inside Guitar Pro 6 (.gpx), 7 and 8 (.gp)
//! files.
//!
//! A GPIF document lists every master bar, bar, voice, beat, note and rhythm
//! once, in flat sections, and links them by id: a master bar names one bar
//! per staff, a bar names up to four voices, a voice names its beats, and so
//! on. Ids can be shared (two voices may reuse the same rest beat), so the
//! reader builds a fresh model object at every reference.
//!
//! What the elements mean follows alphaTab's GPIF reader (MPL-2.0) and our
//! own study of the test corpus; no code is copied. The details are in
//! `docs/formats/guitar-pro-6-8.md`.

mod effects;
#[cfg(feature = "gp")]
pub(crate) mod write;

use std::collections::HashMap;
use std::str::FromStr;

use oxiguitar_model::{
    Bar, Beat, BeatKind, ChordDiagram, Clef, Color, Direction, Duration, Dynamic, GraceKind,
    Instrument, KeyMode, KeySignature, LyricLine, Marker, MasterBar, Mix, NotePlace, NoteValue,
    Pitch, Ratio, Score, ScoreInfo, Staff, TempoChange, TimeSignature, Track, TripletFeel, Tuplet,
    Voice,
};

use crate::container::{self, GpFiles};
use crate::error::{ImportLog, ReadError};
use crate::xml::{self, Element};

/// Upper limits that no real file reaches; anything above means damage.
const MAX_BARS: usize = 20_000;
const MAX_TRACKS: usize = 256;
const MAX_STAVES_PER_TRACK: usize = 4;
const MAX_BEATS_PER_VOICE: usize = 4_096;
const MAX_NOTES_PER_BEAT: usize = 32;
/// The editor allows 3 to 10 strings (EDIT-4); Guitar Pro allows up to 10.
const MAX_STRINGS: usize = 10;
const MAX_FRET: u8 = 99;
/// Ticks per quarter note in Guitar Pro's own timing values.
const TICKS_PER_QUARTER: i64 = 960;

/// The result of reading a GPIF document.
pub(crate) struct GpifRead {
    pub(crate) score: Score,
    pub(crate) log: ImportLog,
}

#[cfg(feature = "gpx")]
/// Reads a Guitar Pro 6 file (BCFZ or BCFS container).
pub(crate) fn read_gpx(data: &[u8]) -> Result<GpifRead, ReadError> {
    let files = container::gpx(data).map_err(|e| e.within(|| "GPX container".into()))?;
    read_files(&files)
}

#[cfg(feature = "gp")]
/// Reads a Guitar Pro 7 or 8 file (ZIP archive).
pub(crate) fn read_gp7(data: &[u8]) -> Result<GpifRead, ReadError> {
    let files = container::gp7(data).map_err(|e| e.within(|| "ZIP archive".into()))?;
    read_files(&files)
}

fn read_files(files: &GpFiles) -> Result<GpifRead, ReadError> {
    let text = xml::decode(&files.score);
    let doc = xml::parse(&text).map_err(|e| e.within(|| "score.gpif".into()))?;
    let root = doc.root();
    if root.name() != "GPIF" {
        return Err(ReadError::invalid(
            format!("score.gpif starts with <{}>, not <GPIF>", root.name()),
            root.offset(),
        )
        .within(|| "score.gpif".into()));
    }
    let notation = files
        .part_configuration
        .as_deref()
        .and_then(container::part_configuration);
    let mut r = Reader::new(root);
    let score = r
        .score(notation.as_deref())
        .map_err(|e| e.within(|| "score.gpif".into()))?;
    Ok(GpifRead { score, log: r.log })
}

/// The flat sections of a GPIF document, indexed by id.
struct Index<'a> {
    bars: HashMap<&'a str, Element<'a>>,
    voices: HashMap<&'a str, Element<'a>>,
    beats: HashMap<&'a str, Element<'a>>,
    notes: HashMap<&'a str, Element<'a>>,
    rhythms: HashMap<&'a str, Element<'a>>,
}

impl<'a> Index<'a> {
    fn new(root: Element<'a>) -> Index<'a> {
        let section = |outer: &str, inner: &'a str| -> HashMap<&'a str, Element<'a>> {
            root.child(outer)
                .map(|s| {
                    s.children_named(inner)
                        .filter_map(|e| Some((e.attr("id")?, e)))
                        .collect()
                })
                .unwrap_or_default()
        };
        Index {
            bars: section("Bars", "Bar"),
            voices: section("Voices", "Voice"),
            beats: section("Beats", "Beat"),
            notes: section("Notes", "Note"),
            rhythms: section("Rhythms", "Rhythm"),
        }
    }
}

/// What the reader needs to know about one staff while reading notes.
#[derive(Clone, Debug, Default)]
struct StaffContext {
    /// Open-string pitches, lowest string first, as GPIF numbers them.
    tuning_low_first: Vec<u8>,
    capo: u8,
    /// Chord diagram id in the file → index in the track's chord list.
    chords: HashMap<String, usize>,
}

/// What the reader needs to know about one track while reading bars.
#[derive(Clone, Debug, Default)]
struct TrackContext {
    percussion: bool,
    /// Octaves the written part sits above or below the sounding one
    /// (-1 for guitar and bass).
    octave_shift: i32,
    staves: Vec<StaffContext>,
}

pub(super) struct Reader<'a> {
    root: Element<'a>,
    index: Index<'a>,
    pub(super) log: ImportLog,
    /// Where the current beat is, for log messages: (bar, track), 1-based.
    place: (usize, usize),
    /// The staff slot (track and staff, counted through the score) and voice
    /// being read, to follow ties.
    voice_key: (usize, usize),
    /// Last fret played on each string, per staff slot and voice: a tied
    /// note continues it. Guitar Pro 6 sometimes stores a stale fret on the
    /// second note of a tie.
    last_fret: HashMap<(usize, usize, u8), u8>,
}

/// Parses a number, accepting surrounding spaces.
pub(super) fn num<T: FromStr>(s: &str) -> Option<T> {
    s.trim().parse().ok()
}

/// Parses a decimal number that GPIF may write with a fractional part.
pub(super) fn float(s: &str) -> Option<f64> {
    num::<f64>(s).filter(|v| v.is_finite())
}

/// The `<Property name="…">` children of a `<Properties>` element.
pub(super) fn properties<'e>(parent: Element<'e>) -> impl Iterator<Item = (&'e str, Element<'e>)> {
    parent
        .child("Properties")
        .into_iter()
        .flat_map(|p| p.children_named("Property"))
        .filter_map(|p| Some((p.attr("name")?, p)))
}

/// One property by name.
pub(super) fn property<'e>(parent: Element<'e>, name: &str) -> Option<Element<'e>> {
    properties(parent).find(|(n, _)| *n == name).map(|(_, p)| p)
}

impl<'a> Reader<'a> {
    fn new(root: Element<'a>) -> Reader<'a> {
        Reader {
            root,
            index: Index::new(root),
            log: ImportLog::default(),
            place: (0, 0),
            voice_key: (0, 0),
            last_fret: HashMap::new(),
        }
    }

    pub(super) fn here(&self) -> String {
        format!("bar {}, track {}", self.place.0, self.place.1)
    }

    fn score(&mut self, notation: Option<&[u8]>) -> Result<Score, ReadError> {
        let root = self.root;
        let mut score = Score::new();
        if let Some(info) = root.child("Score") {
            score.info = score_info(info);
        }

        // Tracks, in the order the master track lists them.
        let tracks_el = root
            .child("Tracks")
            .ok_or_else(|| ReadError::invalid("no <Tracks> section", root.offset()))?;
        let by_id: HashMap<&str, Element<'_>> = tracks_el
            .children_named("Track")
            .filter_map(|t| Some((t.attr("id")?, t)))
            .collect();
        let order: Vec<Element<'_>> = match root.path(&["MasterTrack", "Tracks"]) {
            Some(list) => list
                .text()
                .split_whitespace()
                .filter_map(|id| by_id.get(id).copied())
                .collect(),
            None => tracks_el.children_named("Track").collect(),
        };
        if order.is_empty() {
            return Err(ReadError::invalid(
                "the score has no tracks",
                tracks_el.offset(),
            ));
        }
        if order.len() > MAX_TRACKS {
            return Err(ReadError::invalid(
                format!("{} tracks is more than any real score has", order.len()),
                tracks_el.offset(),
            ));
        }
        let mut contexts = Vec::with_capacity(order.len());
        for (ti, el) in order.iter().enumerate() {
            self.place = (0, ti + 1);
            let flags = notation.and_then(|n| n.get(ti)).copied();
            let (track, ctx) = self.track(*el, flags);
            score.tracks.push(track);
            contexts.push(ctx);
        }

        // Master bars, then each staff's bars.
        let master_bars: Vec<Element<'_>> = root
            .child("MasterBars")
            .map(|m| m.children_named("MasterBar").collect())
            .unwrap_or_default();
        if master_bars.is_empty() {
            return Err(ReadError::invalid("the score has no bars", root.offset()));
        }
        if master_bars.len() > MAX_BARS {
            return Err(ReadError::invalid(
                format!("{} bars is more than any real score has", master_bars.len()),
                root.offset(),
            ));
        }
        for (bi, el) in master_bars.iter().enumerate() {
            self.place = (bi + 1, 0);
            score.master_bars.push(self.master_bar(*el));
        }
        self.tempo(&mut score);

        // Each master bar names one bar per staff, track by track.
        let slots: Vec<(usize, usize)> = contexts
            .iter()
            .enumerate()
            .flat_map(|(ti, c)| (0..c.staves.len()).map(move |si| (ti, si)))
            .collect();
        for (bi, el) in master_bars.iter().enumerate() {
            let ids: Vec<&str> = el
                .child_text("Bars")
                .unwrap_or_default()
                .split_whitespace()
                .collect();
            if ids.len() < slots.len() {
                self.log.warn(
                    "a bar was missing for some staves and was left empty",
                    format!("bar {}", bi + 1),
                );
            }
            for (k, &(ti, si)) in slots.iter().enumerate() {
                self.place = (bi + 1, ti + 1);
                let ctx = contexts.get(ti).cloned().unwrap_or_default();
                let bar = match ids.get(k).and_then(|id| self.index.bars.get(id)) {
                    Some(bar_el) => {
                        self.voice_key.0 = k;
                        self.bar(*bar_el, &ctx, si)
                    }
                    None => {
                        if ids.get(k).is_some() {
                            self.log.warn(
                                "a bar refers to data the file does not contain and was left empty",
                                self.here(),
                            );
                        }
                        empty_bar(&ctx)
                    }
                };
                if let Some(staff) = score.tracks.get_mut(ti).and_then(|t| t.staves.get_mut(si)) {
                    staff.bars.push(bar);
                }
            }
        }
        Ok(score)
    }

    fn track(&mut self, el: Element<'_>, notation_flags: Option<u8>) -> (Track, TrackContext) {
        let mut track = Track {
            name: el.child_text("Name").unwrap_or_default().to_owned(),
            short_name: el.child_text("ShortName").unwrap_or_default().to_owned(),
            color: el.child_text("Color").map(color).unwrap_or_default(),
            ..Track::default()
        };
        let mut ctx = TrackContext::default();

        // Instrument: GP7/8 name it by type and keep MIDI in <Sounds>; GP6
        // has <GeneralMidi> and an instrument reference.
        let set_type = el.path(&["InstrumentSet", "Type"]).map(Element::text);
        let gp6_ref = el.child("Instrument").and_then(|i| i.attr("ref"));
        let gm = el.child("GeneralMidi");
        let channel = el
            .path(&["MidiConnection", "PrimaryChannel"])
            .or_else(|| gm.and_then(|g| g.child("PrimaryChannel")))
            .and_then(|c| num::<u8>(c.text()));
        ctx.percussion = set_type.is_some_and(|t| t == "drumKit")
            || gp6_ref.is_some_and(|r| r.starts_with("drmkt"))
            || (set_type.is_none() && gp6_ref.is_none() && channel == Some(9));
        let program = el
            .path(&["Sounds", "Sound", "MIDI", "Program"])
            .or_else(|| gm.and_then(|g| g.child("Program")))
            .and_then(|p| num::<u8>(p.text()))
            .filter(|p| *p <= 127)
            .unwrap_or(25);
        track.instrument = Instrument {
            midi_program: program,
            percussion: ctx.percussion,
        };
        // The first sound automation sits at bar 0 and only names the
        // starting sound; any later one is a change.
        let sound_change = el.child("Automations").is_some_and(|a| {
            a.children_named("Automation").any(|x| {
                x.child_text("Type") == Some("Sound")
                    && x.child_text("Bar").and_then(num::<usize>).unwrap_or(0) > 0
            })
        });
        if sound_change {
            self.log.warn(
                "sound changes inside a track are not supported yet; the first sound is kept",
                format!("track {}", self.place.1),
            );
        }
        track.mix = mix(el);

        // Transposition: GP7/8 give octave and semitones; GP6 one number.
        let total: i32 = match el.child("Transpose") {
            Some(t) => {
                let chromatic = t.child_text("Chromatic").and_then(num::<i32>).unwrap_or(0);
                let octave = t.child_text("Octave").and_then(num::<i32>).unwrap_or(0);
                chromatic.saturating_add(octave.saturating_mul(12))
            }
            None => el
                .path(&["PartSounding", "TranspositionPitch"])
                .and_then(|p| num::<i32>(p.text()))
                .unwrap_or(0),
        }
        .clamp(-48, 48);
        // An octave down is what the treble 8vb clef shows (guitar, bass);
        // the rest stays as the staff's transposition, written - sounding.
        ctx.octave_shift = if (-23..=-12).contains(&total) { -1 } else { 0 };
        let transposition = i8::try_from(-total + 12 * ctx.octave_shift).unwrap_or(0);

        // Staves: GP7/8 list them; GP6 keeps one staff's properties on the track.
        let staff_els: Vec<Element<'_>> = el
            .child("Staves")
            .map(|s| s.children_named("Staff").collect())
            .unwrap_or_default();
        let staff_props: Vec<Element<'_>> = if staff_els.is_empty() {
            vec![el]
        } else {
            staff_els
        };
        if staff_props.len() > MAX_STAVES_PER_TRACK {
            self.log.warn(
                format!("only the first {MAX_STAVES_PER_TRACK} staves of a track are read"),
                format!("track {}", self.place.1),
            );
        }
        for props in staff_props.iter().take(MAX_STAVES_PER_TRACK) {
            let (staff, sctx) = self.staff(*props, &mut track, &ctx, transposition, notation_flags);
            track.staves.push(staff);
            ctx.staves.push(sctx);
        }

        // Lyrics: one line per <Line>, starting at the bar its offset names.
        if let Some(lyrics) = el.child("Lyrics") {
            for line in lyrics.children_named("Line") {
                let text = line
                    .child("Text")
                    .map(Element::raw_text)
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }
                track.lyrics.push(LyricLine {
                    start_bar: line
                        .child_text("Offset")
                        .and_then(num::<usize>)
                        .unwrap_or(0),
                    text: text.trim().to_owned(),
                });
            }
        }
        (track, ctx)
    }

    fn staff(
        &mut self,
        props: Element<'_>,
        track: &mut Track,
        tctx: &TrackContext,
        transposition: i8,
        notation_flags: Option<u8>,
    ) -> (Staff, StaffContext) {
        let mut sctx = StaffContext::default();
        let mut staff = Staff {
            transposition,
            ..Staff::default()
        };
        if let Some(t) = property(props, "Tuning").and_then(|t| t.child("Pitches")) {
            let pitches: Vec<u8> = t
                .text()
                .split_whitespace()
                .filter_map(num::<u8>)
                .filter(|p| *p <= 127)
                .collect();
            // Drum kits store a tuning of zeros; that is no tuning at all.
            if !tctx.percussion && pitches.iter().any(|p| *p != 0) {
                if pitches.len() > MAX_STRINGS {
                    self.log.warn(
                        format!(
                            "tunings with more than {MAX_STRINGS} strings are cut to {MAX_STRINGS}"
                        ),
                        format!("track {}", self.place.1),
                    );
                }
                sctx.tuning_low_first = pitches.into_iter().take(MAX_STRINGS).collect();
            }
        }
        staff.tuning = sctx
            .tuning_low_first
            .iter()
            .rev()
            .map(|p| Pitch(*p))
            .collect();
        sctx.capo = property(props, "CapoFret")
            .and_then(|c| c.child_text("Fret"))
            .and_then(num::<u8>)
            .filter(|c| *c <= MAX_FRET)
            .unwrap_or(0);
        staff.capo = sctx.capo;
        if property(props, "PartialCapoFret")
            .and_then(|c| c.child_text("Fret"))
            .and_then(num::<u8>)
            .is_some_and(|f| f > 0)
        {
            self.log.warn(
                "partial capos are not supported yet",
                format!("track {}", self.place.1),
            );
        }
        if let Some(n) = property(props, "FretCount")
            .and_then(|c| c.child_text("Number"))
            .and_then(num::<u8>)
            .filter(|n| (1..=36).contains(n))
        {
            staff.fret_count = n;
        }

        // Which notation shows: the PartConfiguration flags if there are any.
        let stringed = !staff.tuning.is_empty();
        let flags = notation_flags.unwrap_or(if stringed { 3 } else { 1 });
        staff.show_standard = flags & 0x01 != 0 || !stringed || flags & 0x03 == 0;
        staff.show_tablature = flags & 0x02 != 0 && stringed;
        if flags & 0x0C != 0 {
            self.log.info(
                "slash and numbered notation views are shown as standard notation",
                format!("track {}", self.place.1),
            );
        }

        // Chord diagrams: GP7/8 per staff, GP6 per track (the same element).
        if let Some(items) = property(props, "DiagramCollection").and_then(|d| d.child("Items")) {
            for item in items.children_named("Item") {
                let Some(id) = item.attr("id") else { continue };
                let diagram = chord_diagram(item, sctx.tuning_low_first.len());
                let index = match track.chords.iter().position(|c| *c == diagram) {
                    Some(i) => i,
                    None => {
                        track.chords.push(diagram);
                        track.chords.len() - 1
                    }
                };
                sctx.chords.insert(id.to_owned(), index);
            }
        }
        (staff, sctx)
    }

    fn master_bar(&mut self, el: Element<'_>) -> MasterBar {
        let mut mb = MasterBar::default();
        if let Some((n, d)) = el.child_text("Time").and_then(|t| t.split_once('/')) {
            match (num::<u8>(n), num::<u8>(d)) {
                (Some(n), Some(d))
                    if n > 0 && NoteValue::from_denominator(u16::from(d)).is_some() =>
                {
                    mb.time_signature = TimeSignature {
                        numerator: n,
                        denominator: d,
                    };
                }
                _ => self.log.warn(
                    "an unreadable time signature was replaced by 4/4",
                    format!("bar {}", self.place.0),
                ),
            }
        }
        if let Some(key) = el.child("Key") {
            mb.key_signature = KeySignature {
                fifths: key
                    .child_text("AccidentalCount")
                    .and_then(num::<i8>)
                    .unwrap_or(0)
                    .clamp(-7, 7),
                mode: if key.child_text("Mode") == Some("Minor") {
                    KeyMode::Minor
                } else {
                    KeyMode::Major
                },
            };
        }
        if let Some(rep) = el.child("Repeat") {
            mb.repeat_start = rep.attr("start") == Some("true");
            if rep.attr("end") == Some("true") {
                let plays = rep.attr("count").and_then(num::<u8>).unwrap_or(2);
                mb.repeat_end = Some(plays.clamp(2, 99));
            }
        }
        if let Some(endings) = el.child_text("AlternateEndings") {
            for n in endings.split_whitespace().filter_map(num::<u32>) {
                if (1..=16).contains(&n) {
                    mb.alternate_endings |= 1 << (n - 1);
                }
            }
        }
        if let Some(section) = el.child("Section") {
            let letter = section.child_text("Letter").unwrap_or_default();
            let text = section.child_text("Text").unwrap_or_default();
            if !letter.is_empty() || !text.is_empty() {
                mb.marker = Some(Marker {
                    letter: letter.to_owned(),
                    text: text.to_owned(),
                    color: Color::default(),
                });
            }
        }
        mb.double_bar = el.child("DoubleBar").is_some();
        if let Some(feel) = el.child_text("TripletFeel") {
            mb.triplet_feel = match feel {
                "Triplet8th" => TripletFeel::Triplet8th,
                "Triplet16th" => TripletFeel::Triplet16th,
                "Dotted8th" => TripletFeel::Dotted8th,
                "Dotted16th" => TripletFeel::Dotted16th,
                "Scottish8th" => TripletFeel::Scottish8th,
                "Scottish16th" => TripletFeel::Scottish16th,
                _ => TripletFeel::None,
            };
        }
        if let Some(dirs) = el.child("Directions") {
            for d in dirs.children() {
                match direction(d.text()) {
                    Some(dir) => mb.directions.push(dir),
                    None => self.log.warn(
                        format!("the direction \"{}\" is not supported", d.text()),
                        format!("bar {}", self.place.0),
                    ),
                }
            }
        }
        if el.child("Fermatas").is_some_and(|f| f.has_children()) {
            self.log.warn(
                "fermatas are not supported yet",
                format!("bar {}", self.place.0),
            );
        }
        mb.free_time = el.child("FreeTime").is_some();
        mb
    }

    /// Tempo automations of the master track: the one at the very start is
    /// the initial tempo, the others become tempo changes.
    fn tempo(&mut self, score: &mut Score) {
        let Some(autos) = self.root.path(&["MasterTrack", "Automations"]) else {
            return;
        };
        let mut found_initial = false;
        for a in autos.children_named("Automation") {
            match a.child_text("Type") {
                Some("Tempo") => {}
                Some("SyncPoint") => {
                    self.log
                        .info("audio sync points were ignored", "master track");
                    continue;
                }
                _ => continue,
            }
            let mut parts = a.child_text("Value").unwrap_or_default().split_whitespace();
            let Some(value) = parts.next().and_then(float).filter(|v| *v > 0.0) else {
                continue;
            };
            // The second number is the beat unit the value counts.
            let unit = match parts.next().and_then(num::<u8>) {
                Some(1) => 0.5,
                Some(3) => 1.5,
                Some(4) => 2.0,
                Some(5) => 3.0,
                _ => 1.0,
            };
            let bpm = (value * unit).clamp(1.0, 2000.0);
            let bar = a.child_text("Bar").and_then(num::<usize>).unwrap_or(0);
            let fraction = a
                .child_text("Position")
                .and_then(float)
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let label = a.child_text("Text").unwrap_or_default().to_owned();
            let linear = a.child_text("Linear") == Some("true");
            if bar == 0 && fraction == 0.0 && !found_initial {
                found_initial = true;
                score.initial_tempo = bpm;
                score.tempo_label = label;
                continue;
            }
            let Some(mb) = score.master_bars.get_mut(bar) else {
                self.log.warn(
                    "a tempo change after the last bar was ignored",
                    "master track",
                );
                continue;
            };
            let length = mb.expected_length().unwrap_or(Ratio::ONE);
            let ticks =
                (fraction * length.to_f64() * 4.0 * TICKS_PER_QUARTER as f64).round() as i64;
            let position = Ratio::new(ticks, 4 * TICKS_PER_QUARTER).unwrap_or(Ratio::ZERO);
            mb.tempo_changes.push(TempoChange {
                position,
                bpm,
                label,
                linear,
            });
        }
        for mb in &mut score.master_bars {
            mb.tempo_changes.sort_by_key(|t| t.position);
        }
    }

    fn bar(&mut self, el: Element<'_>, ctx: &TrackContext, staff_index: usize) -> Bar {
        let ottava = el.child_text("Ottavia");
        let clef = self.clef(el.child_text("Clef").unwrap_or("G2"), ottava, ctx);
        if el.child("SimileMark").is_some() {
            self.log
                .warn("simile marks are not supported yet", self.here());
        }
        let sctx = ctx.staves.get(staff_index).cloned().unwrap_or_default();
        let mut bar = Bar {
            clef,
            voices: Vec::new(),
        };
        let ids: Vec<&str> = el
            .child_text("Voices")
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        for (vi, id) in ids.iter().take(4).enumerate() {
            if *id == "-1" {
                continue;
            }
            let Some(voice_el) = self.index.voices.get(id).copied() else {
                self.log.warn(
                    "a voice refers to data the file does not contain and was left out",
                    self.here(),
                );
                continue;
            };
            self.voice_key.1 = vi;
            let voice = self.voice(voice_el, ctx, &sctx);
            if vi == 0 || bar.voices.is_empty() || !voice.is_empty() {
                bar.voices.push(voice);
            }
        }
        if bar.voices.is_empty() {
            bar.voices.push(Voice::default());
        }
        bar
    }

    fn clef(&mut self, name: &str, ottava: Option<&str>, ctx: &TrackContext) -> Clef {
        if ottava.is_some_and(|o| !o.is_empty()) {
            self.log.warn(
                "octave clefs other than the guitar's treble 8vb are not supported yet",
                self.here(),
            );
        }
        let down = ctx.octave_shift == -1;
        match name {
            "G2" if down => Clef::Treble8vb,
            "G2" => Clef::Treble,
            "F4" if down => Clef::Bass8vb,
            "F4" => Clef::Bass,
            "C3" => Clef::Alto,
            "C4" => Clef::Tenor,
            "Neutral" => Clef::Neutral,
            other => {
                self.log.warn(
                    format!("the clef \"{other}\" is not supported and was read as treble"),
                    self.here(),
                );
                if down { Clef::Treble8vb } else { Clef::Treble }
            }
        }
    }

    fn voice(&mut self, el: Element<'_>, ctx: &TrackContext, sctx: &StaffContext) -> Voice {
        let mut voice = Voice::default();
        let ids: Vec<&str> = el
            .child_text("Beats")
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        if ids.len() > MAX_BEATS_PER_VOICE {
            self.log.warn(
                format!("only the first {MAX_BEATS_PER_VOICE} beats of a voice are read"),
                self.here(),
            );
        }
        for id in ids.iter().take(MAX_BEATS_PER_VOICE) {
            let Some(beat_el) = self.index.beats.get(id).copied() else {
                self.log.warn(
                    "a beat refers to data the file does not contain and was left out",
                    self.here(),
                );
                continue;
            };
            if let Some(beat) = self.beat(beat_el, ctx, sctx) {
                voice.beats.push(beat);
            }
        }
        voice
    }

    fn beat(&mut self, el: Element<'_>, ctx: &TrackContext, sctx: &StaffContext) -> Option<Beat> {
        let Some(duration) = el
            .child("Rhythm")
            .and_then(|r| r.attr("ref"))
            .and_then(|id| self.index.rhythms.get(id).copied())
            .and_then(|r| self.duration(r))
        else {
            self.log.warn(
                "a beat without a readable duration was left out",
                self.here(),
            );
            return None;
        };
        let mut beat = Beat {
            duration,
            ..Beat::default()
        };
        beat.grace = match el.child_text("GraceNotes") {
            Some("OnBeat") => Some(GraceKind::OnBeat),
            Some("BeforeBeat") => Some(GraceKind::BeforeBeat),
            _ => None,
        };
        // The beat's dynamic is an explicit marking in Guitar Pro 6-8 (mf
        // unless changed); notes also take it as their velocity.
        let marked = el.child_text("Dynamic").and_then(dynamic);
        beat.effects.dynamic = marked;
        let dynamic = marked.unwrap_or_default();
        beat.text = el
            .child("FreeText")
            .map(|t| t.text().to_owned())
            .unwrap_or_default();
        if let Some(chord) = el.child_text("Chord") {
            beat.chord = sctx.chords.get(chord).copied();
            if beat.chord.is_none() {
                self.log.warn(
                    "a chord name without a diagram in the file was left out",
                    self.here(),
                );
            }
        }
        self.beat_effects(el, &mut beat.effects);

        let ids: Vec<&str> = el
            .child_text("Notes")
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        for id in ids.iter().take(MAX_NOTES_PER_BEAT) {
            let Some(note_el) = self.index.notes.get(id).copied() else {
                self.log.warn(
                    "a note refers to data the file does not contain and was left out",
                    self.here(),
                );
                continue;
            };
            if let Some(mut note) = self.note(note_el, ctx, sctx, &mut beat.effects) {
                note.dynamic = dynamic;
                self.follow_tie(&mut note);
                // One note per string: a second one would be unplayable.
                let taken = note
                    .string()
                    .is_some_and(|s| beat.notes.iter().any(|n| n.string() == Some(s)));
                if taken {
                    self.log.warn(
                        "a second note on the same string in one beat was left out",
                        self.here(),
                    );
                } else {
                    beat.notes.push(note);
                }
            }
        }
        if beat.notes.is_empty() {
            beat.kind = BeatKind::Rest;
        }
        // Highest string first, as in the GP5 reader.
        beat.notes.sort_by_key(|n| n.string().unwrap_or(0));
        Some(beat)
    }

    fn duration(&mut self, el: Element<'_>) -> Option<Duration> {
        let value = match el.child_text("NoteValue")? {
            "DoubleWhole" => NoteValue::DoubleWhole,
            "Long" => {
                self.log.warn(
                    "quadruple whole notes are read as double whole notes",
                    self.here(),
                );
                NoteValue::DoubleWhole
            }
            "Whole" => NoteValue::Whole,
            "Half" => NoteValue::Half,
            "Quarter" => NoteValue::Quarter,
            "Eighth" => NoteValue::Eighth,
            "16th" => NoteValue::Sixteenth,
            "32nd" => NoteValue::ThirtySecond,
            "64th" => NoteValue::SixtyFourth,
            "128th" => NoteValue::OneTwentyEighth,
            "256th" => NoteValue::TwoFiftySixth,
            _ => return None,
        };
        let mut d = Duration::new(value);
        d.dots = el
            .child("AugmentationDot")
            .and_then(|a| a.attr("count"))
            .and_then(num::<u8>)
            .unwrap_or(0)
            .min(4);
        for name in ["PrimaryTuplet", "SecondaryTuplet"] {
            if let Some(t) = el.child(name) {
                let notes = t.attr("num").and_then(num::<u8>).unwrap_or(0);
                let in_time_of = t.attr("den").and_then(num::<u8>).unwrap_or(0);
                if notes > 1 && in_time_of > 0 && notes != in_time_of {
                    d.tuplets.push(Tuplet { notes, in_time_of });
                }
            }
        }
        d.length().map(|_| d)
    }
}

impl Reader<'_> {
    /// Gives a tied note the fret of the note it continues, and remembers
    /// each string's fret for the next tie.
    fn follow_tie(&mut self, note: &mut oxiguitar_model::Note) {
        let NotePlace::Fretted { string, fret } = &mut note.place else {
            return;
        };
        let key = (self.voice_key.0, self.voice_key.1, *string);
        if note.tie
            && let Some(&previous) = self.last_fret.get(&key)
            && previous != *fret
        {
            self.log.info(
                "a tied note's fret was corrected to the fret of the note it continues",
                self.here(),
            );
            *fret = previous;
        }
        self.last_fret.insert(key, *fret);
    }
}

fn empty_bar(ctx: &TrackContext) -> Bar {
    Bar {
        clef: if ctx.percussion {
            Clef::Neutral
        } else if ctx.octave_shift == -1 {
            Clef::Treble8vb
        } else {
            Clef::Treble
        },
        voices: vec![Voice::default()],
    }
}

fn score_info(el: Element<'_>) -> ScoreInfo {
    let t = |name: &str| el.child_text(name).unwrap_or_default().to_owned();
    let mut info = ScoreInfo {
        title: t("Title"),
        subtitle: t("SubTitle"),
        artist: t("Artist"),
        album: t("Album"),
        words: t("Words"),
        music: t("Music"),
        copyright: t("Copyright"),
        transcriber: t("Tabber"),
        instructions: t("Instructions"),
        notices: el
            .child("Notices")
            .map(|n| {
                n.raw_text()
                    .lines()
                    .map(str::trim_end)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    };
    while info.notices.last().is_some_and(String::is_empty) {
        info.notices.pop();
    }
    // "Words and music" fills whichever of the two is empty.
    let both = t("WordsAndMusic");
    if !both.is_empty() {
        if info.words.is_empty() {
            info.words.clone_from(&both);
        }
        if info.music.is_empty() {
            info.music = both;
        }
    }
    info
}

fn color(text: &str) -> Color {
    let mut v = text.split_whitespace().filter_map(num::<u8>);
    Color {
        r: v.next().unwrap_or(0),
        g: v.next().unwrap_or(0),
        b: v.next().unwrap_or(0),
    }
}

/// Volume and pan from the RSE channel strip (parameters 12 and 11, each
/// 0 to 1), mute and solo from the playback state.
fn mix(el: Element<'_>) -> Mix {
    let mut m = Mix::default();
    if let Some(params) = el.path(&["RSE", "ChannelStrip", "Parameters"]) {
        // In f64, so every f32 volume and pan has a value that reads back
        // as it (the .gp writer's).
        let p: Vec<f64> = params.text().split_whitespace().filter_map(float).collect();
        if let (Some(pan), Some(vol)) = (p.get(11), p.get(12)) {
            m.pan = ((pan * 2.0 - 1.0) as f32).clamp(-1.0, 1.0);
            m.volume = (*vol as f32).clamp(0.0, 1.0);
        }
    }
    match el.child_text("PlaybackState") {
        Some("Mute") => m.mute = true,
        Some("Solo") => m.solo = true,
        _ => {}
    }
    m
}

/// A chord diagram item. GPIF numbers strings from the lowest (0) and counts
/// frets from the diagram's base fret, 0 being an open string (an E chord
/// at the seventh fret has its low E open: base 6, fret 0).
fn chord_diagram(item: Element<'_>, track_strings: usize) -> ChordDiagram {
    let name = item.attr("name").unwrap_or_default().to_owned();
    // A diagram Guitar Pro is told not to show is a name only.
    let shown = |d: &Element<'_>| {
        !d.children_named("Property")
            .any(|p| p.attr("name") == Some("ShowDiagram") && p.attr("value") == Some("false"))
    };
    let Some(diagram) = item.child("Diagram").filter(shown) else {
        let base = item
            .child("Diagram")
            .and_then(|d| d.attr("baseFret"))
            .and_then(num::<u8>)
            .unwrap_or(0)
            .min(MAX_FRET);
        return ChordDiagram {
            name,
            first_fret: base + 1,
            frets: Vec::new(),
        };
    };
    let strings = diagram
        .attr("stringCount")
        .and_then(num::<usize>)
        .unwrap_or(track_strings)
        .min(MAX_STRINGS);
    let base = diagram
        .attr("baseFret")
        .and_then(num::<u8>)
        .unwrap_or(0)
        .min(MAX_FRET);
    let mut frets = vec![None; strings];
    for f in diagram.children_named("Fret") {
        let (Some(s), Some(fret)) = (
            f.attr("string").and_then(num::<usize>),
            f.attr("fret").and_then(num::<u8>),
        ) else {
            continue;
        };
        if s < strings
            && let Some(slot) = frets.get_mut(strings - 1 - s)
        {
            // Fret 0 is an open string wherever the diagram starts.
            *slot = Some(if fret == 0 {
                0
            } else {
                base.saturating_add(fret).min(MAX_FRET)
            });
        }
    }
    ChordDiagram {
        name,
        first_fret: base + 1,
        frets,
    }
}

fn direction(text: &str) -> Option<Direction> {
    Some(match text {
        "Segno" => Direction::Segno,
        "SegnoSegno" => Direction::SegnoSegno,
        "Coda" => Direction::Coda,
        "DoubleCoda" => Direction::DoubleCoda,
        "Fine" => Direction::Fine,
        "DaCoda" => Direction::ToCoda,
        "DaDoubleCoda" => Direction::ToDoubleCoda,
        "DaCapo" => Direction::DaCapo,
        "DaCapoAlCoda" => Direction::DaCapoAlCoda,
        "DaCapoAlDoubleCoda" => Direction::DaCapoAlDoubleCoda,
        "DaCapoAlFine" => Direction::DaCapoAlFine,
        "DaSegno" => Direction::DalSegno,
        "DaSegnoAlCoda" => Direction::DalSegnoAlCoda,
        "DaSegnoAlDoubleCoda" => Direction::DalSegnoAlDoubleCoda,
        "DaSegnoAlFine" => Direction::DalSegnoAlFine,
        "DaSegnoSegno" => Direction::DalSegnoSegno,
        "DaSegnoSegnoAlCoda" => Direction::DalSegnoSegnoAlCoda,
        "DaSegnoSegnoAlDoubleCoda" => Direction::DalSegnoSegnoAlDoubleCoda,
        "DaSegnoSegnoAlFine" => Direction::DalSegnoSegnoAlFine,
        _ => return None,
    })
}

fn dynamic(text: &str) -> Option<Dynamic> {
    Some(match text {
        "PPP" => Dynamic::PPP,
        "PP" => Dynamic::PP,
        "P" => Dynamic::P,
        "MP" => Dynamic::MP,
        "MF" => Dynamic::MF,
        "F" => Dynamic::F,
        "FF" => Dynamic::FF,
        "FFF" => Dynamic::FFF,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn chord_diagram_strings_and_base_fret() {
        let item = xml::parse(
            r#"<Item id="0" name="C7M9"><Diagram stringCount="6" baseFret="0">
            <Fret string="1" fret="3"/><Fret string="2" fret="2"/><Fret string="3" fret="4"/>
            <Fret string="4" fret="3"/><Fret string="5" fret="0"/></Diagram></Item>"#,
        )
        .unwrap();
        let c = chord_diagram(item.root(), 6);
        assert_eq!(c.name, "C7M9");
        assert_eq!(c.first_fret, 1);
        // Highest string first: x32430 read from the top.
        assert_eq!(
            c.frets,
            vec![Some(0), Some(3), Some(4), Some(2), Some(3), None]
        );
    }

    #[test]
    fn colors_and_directions() {
        assert_eq!(
            color("237 116 116"),
            Color {
                r: 237,
                g: 116,
                b: 116
            }
        );
        assert_eq!(color("nonsense"), Color::default());
        assert_eq!(direction("DaSegnoAlFine"), Some(Direction::DalSegnoAlFine));
        assert_eq!(direction("DaCoda"), Some(Direction::ToCoda));
        assert_eq!(direction("Elsewhere"), None);
    }
}
