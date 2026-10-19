// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The "Example tab" format (`.extab`): one guitar track of eighth notes in
//! 4/4, as text.
//!
//! ```text
//! EXAMPLE TAB
//! title: Open Strings
//! artist: Your Name
//! tempo: 96
//! # One bar per line: eight eighth notes, each a string and a fret
//! # (string 1 is the highest), several joined by + for a chord, or - for
//! # a rest. A bar with fewer than eight is filled with rests.
//! 6:0 5:0 4:0 3:0 2:0 1:0 - -
//! 6:3 5:2 4:0 3:0 2:0+1:3 - - -
//! ```
//!
//! The manifest recognises the file by its first line (the signature
//! `{"at": 0, "text": "EXAMPLE TAB"}`), so the host picks this plugin
//! without loading it.

use oxiguitar_plugin::score::{
    Bar, Beat, BeatEffects, BeatKind, Clef, Duration, Dynamic, Instrument, KeyMode, KeySignature,
    MasterBar, Mix, Note, NoteEffects, NotePlace, NoteValue, Score, ScoreInfo, Staff,
    TimeSignature, Track, TripletFeel, Voice,
};
use oxiguitar_plugin::{
    Error, ErrorKind, FormatReader, FormatWriter, Level, LogEntry, Read, Written,
};

use crate::notes::STANDARD;

/// The format's id in `manifest.json`.
pub const ID: &str = "extab";

/// The first line of every Example tab file.
pub const MAGIC: &str = "EXAMPLE TAB";

/// Eighth notes in a bar of 4/4.
const PER_BAR: usize = 8;

/// The highest fret the format takes.
const FRETS: u8 = 24;

/// The format, read and written.
pub struct ExampleTab;

// ANCHOR: read
impl FormatReader for ExampleTab {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        // The file is untrusted: anything wrong is an error saying where,
        // never a panic.
        let text = std::str::from_utf8(data)
            .map_err(|e| invalid("the file is not UTF-8 text", e.valid_up_to(), ""))?;
        let mut lines = lines(text);
        if lines.next().map(|(_, line)| line.trim_end()) != Some(MAGIC) {
            return Err(invalid("this is not an Example tab file", 0, ""));
        }
        let mut info = ScoreInfo::default();
        let mut tempo = 120.0;
        let mut bars = Vec::new();
        let mut log = Vec::new();
        for (at, line) in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = header(line) {
                match key {
                    "title" => info.title = value.into(),
                    "artist" => info.artist = value.into(),
                    "tempo" => {
                        tempo = value
                            .parse::<f64>()
                            .ok()
                            .filter(|t| (20.0..=400.0).contains(t))
                            .ok_or_else(|| invalid("the tempo is not 20 to 400", at, "tempo"))?;
                    }
                    _ => add(
                        &mut log,
                        Level::Info,
                        "lines the format doesn't know were ignored",
                        format!("the line \"{key}: …\""),
                    ),
                }
                continue;
            }
            bars.push(bar(line, at, bars.len() + 1, &mut log)?);
        }
        if bars.is_empty() {
            return Err(Error {
                offset: Some(data.len() as u64),
                ..Error::new(
                    ErrorKind::UnexpectedEnd,
                    "the file ends before its first bar",
                )
            });
        }
        Ok(Read {
            score: score(info, tempo, bars),
            log,
        })
    }
}
// ANCHOR_END: read

/// The lines of `text` with their offsets in bytes, without line endings.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0, |at, line| {
        let start = *at;
        *at += line.len();
        Some((start, line.trim_end_matches(['\n', '\r'])))
    })
}

/// `key: value`, where `key` is a word: a header line, not a bar.
fn header(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let word = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic());
    word.then(|| (key, value.trim()))
}

/// One bar: up to eight eighth notes.
fn bar(line: &str, at: usize, number: usize, log: &mut Vec<LogEntry>) -> Result<Vec<Beat>, Error> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() > PER_BAR {
        let place = format!("bar {number}");
        return Err(invalid(
            "a bar holds at most eight eighth notes",
            at,
            &place,
        ));
    }
    let mut beats = Vec::with_capacity(PER_BAR);
    for (i, token) in tokens.iter().enumerate() {
        let place = format!("bar {number}, note {}", i + 1);
        beats.push(beat(token).map_err(|why| invalid(&why, at, &place))?);
    }
    if beats.len() < PER_BAR {
        add(
            log,
            Level::Warning,
            "bars with fewer than eight eighth notes were filled with rests",
            format!("bar {number}"),
        );
        beats.resize_with(PER_BAR, rest);
    }
    Ok(beats)
}

/// One eighth note: `-`, or notes such as `3:5` joined by `+`.
fn beat(token: &str) -> Result<Beat, String> {
    if token == "-" {
        return Ok(rest());
    }
    let mut notes: Vec<Note> = Vec::new();
    for part in token.split('+') {
        let (string, fret) = part
            .split_once(':')
            .and_then(|(s, f)| Some((s.parse::<u8>().ok()?, f.parse::<u8>().ok()?)))
            .ok_or_else(|| format!("{part:?} is not a note such as 3:5, or a rest (-)"))?;
        if !(1..=STANDARD.len() as u8).contains(&string) || fret > FRETS {
            return Err(format!(
                "{part:?}: strings are 1 to 6 and frets 0 to {FRETS}"
            ));
        }
        if notes
            .iter()
            .any(|n| matches!(n.place, NotePlace::Fretted { string: s, .. } if s == string))
        {
            return Err(format!("{token:?} plays string {string} twice"));
        }
        notes.push(Note {
            place: NotePlace::Fretted { string, fret },
            tie: false,
            dynamic: Dynamic::MF,
            effects: NoteEffects::default(),
        });
    }
    Ok(Beat {
        kind: BeatKind::Notes,
        notes,
        ..rest()
    })
}

fn eighth() -> Duration {
    Duration {
        value: NoteValue::Eighth,
        dots: 0,
        tuplets: Vec::new(),
    }
}

fn rest() -> Beat {
    Beat {
        kind: BeatKind::Rest,
        duration: eighth(),
        notes: Vec::new(),
        grace: None,
        effects: BeatEffects::default(),
        chord: None,
        text: String::new(),
    }
}

// ANCHOR: score
/// The score: one guitar track in standard tuning, every bar in 4/4.
fn score(info: ScoreInfo, tempo: f64, bars: Vec<Vec<Beat>>) -> Score {
    let master_bar = MasterBar {
        time_signature: TimeSignature {
            numerator: 4,
            denominator: 4,
        },
        key_signature: KeySignature {
            fifths: 0,
            mode: KeyMode::Major,
        },
        repeat_start: false,
        repeat_end: None,
        alternate_endings: 0,
        marker: None,
        double_bar: false,
        triplet_feel: TripletFeel::None,
        tempo_changes: Vec::new(),
        directions: Vec::new(),
    };
    let staff = Staff {
        tuning: STANDARD.to_vec(),
        fret_count: FRETS,
        show_standard: true,
        show_tablature: true,
        // One bar of the staff for each master bar, each with at least one
        // voice: the core refuses a score shaped otherwise.
        bars: bars
            .iter()
            .map(|beats| Bar {
                clef: Clef::Treble8vb,
                voices: vec![Voice {
                    beats: beats.clone(),
                }],
            })
            .collect(),
        ..Staff::default()
    };
    Score {
        info,
        initial_tempo: tempo,
        tempo_label: String::new(),
        master_bars: vec![master_bar; bars.len()],
        tracks: vec![Track {
            name: "Guitar".into(),
            short_name: "Gtr.".into(),
            // General MIDI 25: steel-string acoustic guitar (from 0).
            instrument: Instrument {
                midi_program: 25,
                percussion: false,
            },
            mix: Mix {
                volume: 0.8,
                pan: 0.0,
                mute: false,
                solo: false,
            },
            staves: vec![staff],
            ..Track::default()
        }],
    }
}
// ANCHOR_END: score

// ANCHOR: write
impl FormatWriter for ExampleTab {
    fn write(&self, score: &Score) -> Result<Written, Error> {
        let mut out = format!("{MAGIC}\n");
        let mut losses = Vec::new();
        for (key, value) in [("title", &score.info.title), ("artist", &score.info.artist)] {
            if !value.is_empty() {
                out.push_str(&format!("{key}: {}\n", value.replace('\n', " ")));
            }
        }
        out.push_str(&format!(
            "tempo: {}\n",
            score.initial_tempo.clamp(20.0, 400.0)
        ));
        // Whatever the format can't hold is left out and listed as a loss,
        // so the user is told before anything is lost.
        if score.tracks.len() > 1 {
            let n = score.tracks.len() as u32 - 1;
            add_n(&mut losses, "only the first track is written", "", n);
        }
        let Some(staff) = score.tracks.first().and_then(|t| t.staves.first()) else {
            return Ok(Written {
                data: out.into_bytes(),
                losses,
            });
        };
        if staff.tuning != STANDARD {
            add(
                &mut losses,
                Level::Warning,
                "the track is written in standard tuning",
                String::new(),
            );
        }
        for (i, bar) in staff.bars.iter().enumerate() {
            let place = format!("bar {}", i + 1);
            let four_four = TimeSignature {
                numerator: 4,
                denominator: 4,
            };
            if score
                .master_bars
                .get(i)
                .is_some_and(|m| m.time_signature != four_four)
            {
                add_n(&mut losses, "every bar is written in 4/4", &place, 1);
            }
            if bar.voices.len() > 1 {
                add_n(&mut losses, "only the first voice is written", &place, 1);
            }
            let beats = bar.voices.first().map_or(&[][..], |v| &v.beats[..]);
            let mut tokens: Vec<String> = beats
                .iter()
                .map(|b| token(b, &place, &mut losses))
                .collect();
            if tokens.len() > PER_BAR {
                add_n(
                    &mut losses,
                    "notes after the eighth of a bar are left out",
                    &place,
                    1,
                );
                tokens.truncate(PER_BAR);
            }
            if tokens.is_empty() {
                tokens.push("-".into());
            }
            out.push_str(&tokens.join(" "));
            out.push('\n');
        }
        Ok(Written {
            data: out.into_bytes(),
            losses,
        })
    }
}
// ANCHOR_END: write

/// A beat as the format writes it.
fn token(beat: &Beat, place: &str, losses: &mut Vec<LogEntry>) -> String {
    if beat.duration != eighth() {
        add_n(losses, "every note is written as an eighth note", place, 1);
    }
    if beat.effects != BeatEffects::default()
        || beat
            .notes
            .iter()
            .any(|n| n.tie || n.effects != NoteEffects::default())
    {
        add_n(losses, "effects and ties are left out", place, 1);
    }
    let notes: Vec<String> = beat
        .notes
        .iter()
        .filter_map(|n| match n.place {
            NotePlace::Fretted { string, fret } if (1..=6).contains(&string) && fret <= FRETS => {
                Some(format!("{string}:{fret}"))
            }
            _ => {
                add_n(
                    losses,
                    "notes off a six-string guitar's 24 frets are left out",
                    place,
                    1,
                );
                None
            }
        })
        .collect();
    if beat.kind != BeatKind::Notes || notes.is_empty() {
        "-".into()
    } else {
        notes.join("+")
    }
}

/// A damaged file: what is wrong, the offset of its line, and where.
fn invalid(message: &str, at: usize, place: &str) -> Error {
    Error {
        offset: Some(at as u64),
        context: place.into(),
        ..Error::new(ErrorKind::Invalid, message)
    }
}

/// Adds `message` to `log`, or counts it again if it is there already, so
/// a log never repeats itself. `place` is where it happened first.
fn add(log: &mut Vec<LogEntry>, level: Level, message: &str, place: String) {
    match log.iter_mut().find(|e| e.message == message) {
        Some(entry) => entry.count += 1,
        None => log.push(LogEntry {
            level,
            message: message.into(),
            place,
            count: 1,
        }),
    }
}

/// A loss counted `n` times.
fn add_n(losses: &mut Vec<LogEntry>, message: &str, place: &str, n: u32) {
    add(losses, Level::Warning, message, place.into());
    if let Some(entry) = losses.iter_mut().find(|e| e.message == message) {
        entry.count += n - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../samples/open-strings.extab");

    #[test]
    fn reads_the_sample() {
        let read = ExampleTab.read(SAMPLE.as_bytes()).unwrap();
        assert_eq!(read.score.info.title, "Open Strings");
        assert_eq!(read.score.initial_tempo, 96.0);
        assert_eq!(read.score.master_bars.len(), 4);
        let bars = &read.score.tracks[0].staves[0].bars;
        assert_eq!(bars.len(), 4);
        assert!(bars.iter().all(|b| b.voices[0].beats.len() == PER_BAR));
        let chord = &bars[1].voices[0].beats[5];
        assert_eq!(chord.notes.len(), 2);
        // The last bar holds four notes: filled with rests, and logged.
        assert_eq!(read.log.len(), 1);
        assert_eq!(read.log[0].place, "bar 4");
    }

    #[test]
    fn writes_what_it_reads() {
        let read = ExampleTab.read(SAMPLE.as_bytes()).unwrap();
        let written = ExampleTab.write(&read.score).unwrap();
        assert!(written.losses.is_empty(), "{:?}", written.losses);
        let again = ExampleTab.read(&written.data).unwrap();
        assert_eq!(again.score, read.score);
        assert!(again.log.is_empty());
    }

    #[test]
    fn damaged_files_say_where() {
        let err = |text: &str| ExampleTab.read(text.as_bytes()).unwrap_err();
        assert_eq!(err("GUITAR PRO").offset, Some(0));
        let e = err("EXAMPLE TAB\n3:5 7:0\n");
        assert_eq!((e.kind, e.offset), (ErrorKind::Invalid, Some(12)));
        assert_eq!(e.context, "bar 1, note 2");
        assert_eq!(err("EXAMPLE TAB\n- - - - - - - - -\n").context, "bar 1");
        assert_eq!(err("EXAMPLE TAB\n1:0+1:2\n").kind, ErrorKind::Invalid);
        assert_eq!(err("EXAMPLE TAB\r\ntempo: fast\r\n").context, "tempo");
        assert_eq!(err("EXAMPLE TAB\n").kind, ErrorKind::UnexpectedEnd);
        assert_eq!(err("EXAMPLE TAB\n\u{0}\u{0}").kind, ErrorKind::Invalid);
    }

    #[test]
    fn says_what_it_cannot_write() {
        let mut score = ExampleTab.read(SAMPLE.as_bytes()).unwrap().score;
        score.tracks.push(score.tracks[0].clone());
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .duration
            .value = NoteValue::Quarter;
        let losses = ExampleTab.write(&score).unwrap().losses;
        let messages: Vec<&str> = losses.iter().map(|l| l.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "only the first track is written",
                "every note is written as an eighth note"
            ]
        );
        assert!(
            ExampleTab
                .write(&Score::default())
                .unwrap()
                .data
                .starts_with(MAGIC.as_bytes())
        );
    }
}
