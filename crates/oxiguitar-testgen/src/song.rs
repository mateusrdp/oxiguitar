// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A song as the generator describes it, and the short text notation its
//! voices are written in.
//!
//! # The notation
//!
//! A voice is a line of tokens separated by spaces; `|` ends a bar.
//!
//! - **Durations** set the length of the beats after them: `w` whole, `h`
//!   half, `q` quarter, `e` eighth, `s` sixteenth, `t` thirty-second, `x`
//!   sixty-fourth. Dots follow (`q.`), then a tuplet: `/3` is the usual
//!   triplet (3 in the time of 2), `/5:3` any ratio. `e/3` is an eighth-note
//!   triplet.
//! - **Rests**: `r`.
//! - **Fretted notes**: `string:fret`, strings counted from the highest (1).
//!   A chord joins notes with `+`: `6:0+5:2+4:2`.
//! - **Pitched notes** (keyboards, voices, strings): `C4`, `F#3`, `Bb2`;
//!   middle C is `C4`. **Drums**: `d` and the General MIDI number, `d36`.
//! - **Note effects** follow the fret or pitch: `h` hammer-on or pull-off to
//!   the next note, `b` full bend, `B` half bend, `R` bend and release, `x`
//!   dead note, `g` ghost note, `m` palm mute, `v` vibrato, `l` let ring,
//!   `>` accent, `^` heavy accent, `.` staccato, `/` shift slide to the next
//!   note, `S` legato slide, `d` slide out downwards, `u` slide out upwards,
//!   `i` slide in from below, `I` slide in from above, `n` natural
//!   harmonic, `a` artificial harmonic, `p` pinch harmonic, `T` and a fret
//!   a trill (`T7`), `f` and a digit a left-hand finger (`f2`). A note
//!   starting with `~` is tied to the note before it on the same string or
//!   pitch.
//! - **Beat marks** start with `@` and apply to the next beat: `@c` and a
//!   number names the track's chord diagram (`@c0`); `@t` and text writes
//!   beat text, `_` for spaces (`@tlet_ring`); `@ppp` to `@fff` set the
//!   dynamic; `@grace` makes the beat a grace note before the beat;
//!   `@trem8`, `@trem16`, `@trem32` tremolo picking; `@down` and `@up`
//!   strums; `@slap`, `@pop`, `@tap`; `@fade` a fade-in; `@dive` a whammy-bar
//!   dive.

use std::fmt;

/// A whole song: header, tracks and the bars every track shares.
#[derive(Clone, Debug)]
pub struct Song {
    /// Title in the header.
    pub title: String,
    /// Subtitle.
    pub subtitle: String,
    /// Artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Words by.
    pub words: String,
    /// Music by.
    pub music: String,
    /// Copyright line.
    pub copyright: String,
    /// Tabbed by.
    pub tabber: String,
    /// Starting tempo in quarter notes a minute.
    pub tempo: u32,
    /// Starting tempo's label ("Moderato"), or empty.
    pub tempo_label: String,
    /// Tempo changes: bar index, position in the bar (0 to 1), quarter
    /// notes a minute.
    pub tempo_changes: Vec<(usize, f64, u32)>,
    /// The tracks, in score order.
    pub tracks: Vec<Track>,
    /// The bars every track shares.
    pub bars: Vec<MasterBar>,
}

impl Song {
    /// An untitled song with `bars` bars of 4/4 in C and no tracks yet.
    pub fn new(title: &str, bars: usize) -> Song {
        Song {
            title: title.to_owned(),
            subtitle: String::new(),
            artist: String::new(),
            album: String::new(),
            words: String::new(),
            music: String::new(),
            copyright: String::new(),
            tabber: String::new(),
            tempo: 100,
            tempo_label: String::new(),
            tempo_changes: Vec::new(),
            tracks: Vec::new(),
            bars: vec![MasterBar::default(); bars],
        }
    }

    /// Adds a track and returns it for its voices.
    pub fn track(&mut self, track: Track) -> &mut Track {
        self.tracks.push(track);
        self.tracks.last_mut().expect("just pushed")
    }

    /// Sets the time signature from bar `from` on.
    pub fn time(&mut self, from: usize, numerator: u8, denominator: u8) -> &mut Song {
        for b in self.bars.iter_mut().skip(from) {
            b.time = (numerator, denominator);
        }
        self
    }

    /// Sets the key signature (sharps positive, flats negative) from bar
    /// `from` on.
    pub fn key(&mut self, from: usize, fifths: i8, minor: bool) -> &mut Song {
        for b in self.bars.iter_mut().skip(from) {
            b.key = fifths;
            b.minor = minor;
        }
        self
    }
}

/// What every track shares in one bar.
#[derive(Clone, Debug, PartialEq)]
pub struct MasterBar {
    /// Time signature.
    pub time: (u8, u8),
    /// Key signature: sharps positive, flats negative.
    pub key: i8,
    /// A minor key.
    pub minor: bool,
    /// A repeat starts here.
    pub repeat_start: bool,
    /// A repeat ends here, played this many times.
    pub repeat_end: Option<u8>,
    /// Alternate endings this bar belongs to (1, 2, …).
    pub endings: Vec<u8>,
    /// A section: letter and text.
    pub section: Option<(String, String)>,
    /// A double bar line at the end.
    pub double_bar: bool,
    /// Triplet feel, by its GPIF name ("Triplet8th").
    pub triplet_feel: Option<&'static str>,
    /// Directions, by their GPIF names ("Segno", "DaCapoAlFine").
    pub directions: Vec<&'static str>,
}

impl Default for MasterBar {
    fn default() -> Self {
        MasterBar {
            time: (4, 4),
            key: 0,
            minor: false,
            repeat_start: false,
            repeat_end: None,
            endings: Vec::new(),
            section: None,
            double_bar: false,
            triplet_feel: None,
            directions: Vec::new(),
        }
    }
}

/// What a track is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A fretted instrument: General MIDI program, tuning lowest string
    /// first (MIDI pitches), capo fret. Sounds an octave below the written
    /// notes, as guitars and basses do.
    Fretted {
        /// General MIDI program (0-based).
        program: u8,
        /// Open strings, lowest first.
        tuning: Vec<u8>,
        /// Capo fret, 0 for none.
        capo: u8,
        /// Clef: treble for guitars, bass for basses.
        bass_clef: bool,
    },
    /// An instrument without strings: General MIDI program and clef.
    Pitched {
        /// General MIDI program (0-based).
        program: u8,
        /// Bass clef rather than treble.
        bass_clef: bool,
    },
    /// A drum kit.
    Drums,
}

impl Kind {
    /// A six-string guitar in standard tuning (GM 27, clean electric).
    pub fn guitar() -> Kind {
        Kind::Fretted {
            program: 27,
            tuning: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
            bass_clef: false,
        }
    }

    /// A steel-string acoustic guitar (GM 25).
    pub fn acoustic() -> Kind {
        Kind::Fretted {
            program: 25,
            tuning: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
            bass_clef: false,
        }
    }

    /// A four-string bass (GM 33).
    pub fn bass() -> Kind {
        Kind::Fretted {
            program: 33,
            tuning: vec![28, 33, 38, 43],
            capo: 0,
            bass_clef: true,
        }
    }

    /// A piano (GM 0).
    pub fn piano() -> Kind {
        Kind::Pitched {
            program: 0,
            bass_clef: false,
        }
    }

    fn strings(&self) -> usize {
        match self {
            Kind::Fretted { tuning, .. } => tuning.len(),
            _ => 0,
        }
    }
}

/// A chord diagram: name, fret per string lowest first (`None` = not
/// played), first fret shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagram {
    /// The chord's name ("Am7").
    pub name: String,
    /// Frets, lowest string first; `None` for a string not played.
    pub frets: Vec<Option<u8>>,
    /// The diagram's first fret (0 or 1 for the nut).
    pub base_fret: u8,
}

impl Diagram {
    /// A diagram from a name and frets written lowest string first, `x`
    /// for a string not played: `Diagram::new("Am", "x02210")`. Frets above
    /// 9 are not possible this way; use the fields.
    pub fn new(name: &str, frets: &str) -> Diagram {
        let frets: Vec<Option<u8>> = frets
            .chars()
            .map(|c| c.to_digit(10).map(|d| d as u8))
            .collect();
        let lowest = frets.iter().flatten().filter(|f| **f > 0).min().copied();
        Diagram {
            name: name.to_owned(),
            frets,
            base_fret: match lowest {
                Some(f) if f > 3 => f,
                _ => 0,
            },
        }
    }
}

/// One track and its music.
#[derive(Clone, Debug)]
pub struct Track {
    /// Full name.
    pub name: String,
    /// Short name, for systems after the first.
    pub short_name: String,
    /// What it is.
    pub kind: Kind,
    /// The notation Guitar Pro would show: 1 standard, 2 tablature, 3 both
    /// (the PartConfiguration flag).
    pub notation: u8,
    /// Chord diagrams, numbered from 0 for `@c`.
    pub chords: Vec<Diagram>,
    /// Lyrics: the bar each line starts at, and the line.
    pub lyrics: Vec<(usize, String)>,
    /// Up to four voices, each a list of bars of beats.
    pub voices: Vec<Vec<Vec<Beat>>>,
}

impl Track {
    /// A track of this kind, showing what Guitar Pro would by default:
    /// standard notation and tablature for fretted instruments, standard
    /// notation otherwise.
    pub fn new(name: &str, short_name: &str, kind: Kind) -> Track {
        let notation = if kind.strings() > 0 { 3 } else { 1 };
        Track {
            name: name.to_owned(),
            short_name: short_name.to_owned(),
            kind,
            notation,
            chords: Vec::new(),
            lyrics: Vec::new(),
            voices: Vec::new(),
        }
    }

    /// Sets the notation flag (1 standard, 2 tablature, 3 both).
    pub fn notation(&mut self, flag: u8) -> &mut Track {
        self.notation = flag;
        self
    }

    /// Adds a chord diagram.
    pub fn chord(&mut self, diagram: Diagram) -> &mut Track {
        self.chords.push(diagram);
        self
    }

    /// Adds a line of lyrics starting at bar `bar`.
    pub fn lyrics(&mut self, bar: usize, line: &str) -> &mut Track {
        self.lyrics.push((bar, line.to_owned()));
        self
    }

    /// Writes voice `index` (0 to 3) from the text notation. Panics on a
    /// mistake in the text, with the token that is wrong: this is test
    /// code, and a wrong reference should fail loudly.
    pub fn voice(&mut self, index: usize, text: &str) -> &mut Track {
        assert!(index < 4, "a track has four voices");
        let bars = parse(text).unwrap_or_else(|e| panic!("{}: {e}", self.name));
        if self.voices.len() <= index {
            self.voices.resize(index + 1, Vec::new());
        }
        self.voices[index] = bars;
        self
    }
}

/// A note value with dots and an optional tuplet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dur {
    /// 1 whole, 2 half, 4 quarter … 64.
    pub value: u8,
    /// Augmentation dots.
    pub dots: u8,
    /// Tuplet: so many notes in the time of so many.
    pub tuplet: Option<(u8, u8)>,
}

impl Dur {
    /// A plain note value.
    pub const fn new(value: u8) -> Dur {
        Dur {
            value,
            dots: 0,
            tuplet: None,
        }
    }

    /// Length in 1/960 of a quarter note.
    pub fn ticks(&self) -> u32 {
        let mut t = 3840 / u32::from(self.value);
        let mut add = t;
        for _ in 0..self.dots {
            add /= 2;
            t += add;
        }
        if let Some((n, m)) = self.tuplet {
            t = t * u32::from(m) / u32::from(n);
        }
        t
    }
}

/// Where a note is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// On a string (1 = highest) at a fret.
    Fret(u8, u8),
    /// A MIDI pitch (also drums).
    Pitch(u8),
}

/// A note and its effects.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// Where it is.
    pub at: At,
    /// Tied from the note before.
    pub tied: bool,
    /// Effects, in the order written.
    pub fx: Vec<Fx>,
}

/// A note effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Fx {
    HammerPull,
    Bend,
    HalfBend,
    BendRelease,
    Dead,
    Ghost,
    PalmMute,
    Vibrato,
    LetRing,
    Accent,
    HeavyAccent,
    Staccato,
    /// GPIF slide flags.
    Slide(u32),
    NaturalHarmonic,
    ArtificialHarmonic,
    PinchHarmonic,
    /// Trill with this fret.
    Trill(u8),
    /// Left-hand finger, 0 thumb to 4.
    Finger(u8),
}

/// A beat: a duration, its notes (none for a rest) and its marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Beat {
    /// Length.
    pub dur: Dur,
    /// Notes; empty for a rest.
    pub notes: Vec<Note>,
    /// Index of the track's chord diagram.
    pub chord: Option<usize>,
    /// Beat text.
    pub text: Option<String>,
    /// Dynamic by its GPIF name ("FF"); `None` is mf.
    pub dynamic: Option<&'static str>,
    /// A grace note before the beat.
    pub grace: bool,
    /// Tremolo picking, by its GPIF name ("1/4").
    pub tremolo: Option<&'static str>,
    /// A strum: "Down" or "Up".
    pub brush: Option<&'static str>,
    /// Slap, pop or tap.
    pub technique: Option<&'static str>,
    /// Fade in.
    pub fade_in: bool,
    /// A whammy-bar dive.
    pub dive: bool,
}

impl Beat {
    fn new(dur: Dur) -> Beat {
        Beat {
            dur,
            notes: Vec::new(),
            chord: None,
            text: None,
            dynamic: None,
            grace: false,
            tremolo: None,
            brush: None,
            technique: None,
            fade_in: false,
            dive: false,
        }
    }
}

/// A mistake in the text notation.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(token: &str, why: &str) -> Result<T, ParseError> {
    Err(ParseError(format!("\"{token}\": {why}")))
}

/// Parses a voice into bars of beats.
pub fn parse(text: &str) -> Result<Vec<Vec<Beat>>, ParseError> {
    let mut bars: Vec<Vec<Beat>> = vec![Vec::new()];
    let mut dur = Dur::new(4);
    let mut marks = Beat::new(dur);
    for token in text.split_whitespace() {
        if token == "|" {
            bars.push(Vec::new());
            continue;
        }
        if let Some(mark) = token.strip_prefix('@') {
            beat_mark(&mut marks, mark, token)?;
            continue;
        }
        if let Some(d) = duration(token)? {
            dur = d;
            continue;
        }
        let mut beat = marks.clone();
        beat.dur = dur;
        marks = Beat::new(dur);
        if token != "r" {
            for part in token.split('+') {
                beat.notes.push(note(part)?);
            }
        }
        bars.last_mut().expect("never empty").push(beat);
    }
    // A trailing "|" leaves an empty last bar; drop it.
    if bars.len() > 1 && bars.last().is_some_and(Vec::is_empty) {
        bars.pop();
    }
    Ok(bars)
}

fn duration(token: &str) -> Result<Option<Dur>, ParseError> {
    let mut chars = token.chars();
    let value = match chars.next() {
        Some('w') => 1,
        Some('h') => 2,
        Some('q') => 4,
        Some('e') => 8,
        Some('s') => 16,
        Some('t') => 32,
        Some('x') => 64,
        _ => return Ok(None),
    };
    let rest = chars.as_str();
    let dots = rest.chars().take_while(|c| *c == '.').count();
    let rest = &rest[dots..];
    let tuplet = match rest.strip_prefix('/') {
        None if rest.is_empty() => None,
        None => return Ok(None),
        Some(t) => {
            let (n, m) = match t.split_once(':') {
                Some((n, m)) => (n.parse::<u8>(), m.parse::<u8>()),
                None => {
                    let n = t.parse::<u8>();
                    // The usual "in the time of": the power of two below n,
                    // except that triplets are 3:2 and duplets 2:3.
                    let m = n.clone().map(|n| match n {
                        2 => 3,
                        3 => 2,
                        n => 1 << (7 - n.leading_zeros()),
                    });
                    (n, m)
                }
            };
            match (n, m) {
                (Ok(n), Ok(m)) if n > 1 && m > 0 && n != m => Some((n, m)),
                _ => return err(token, "a tuplet is /n or /n:m"),
            }
        }
    };
    Ok(Some(Dur {
        value,
        dots: dots as u8,
        tuplet,
    }))
}

fn beat_mark(beat: &mut Beat, mark: &str, token: &str) -> Result<(), ParseError> {
    match mark {
        "ppp" | "pp" | "p" | "mp" | "mf" | "f" | "ff" | "fff" => {
            beat.dynamic = Some(match mark {
                "ppp" => "PPP",
                "pp" => "PP",
                "p" => "P",
                "mp" => "MP",
                "mf" => "MF",
                "f" => "F",
                "ff" => "FF",
                _ => "FFF",
            });
        }
        "grace" => beat.grace = true,
        "trem8" => beat.tremolo = Some("1/2"),
        "trem16" => beat.tremolo = Some("1/4"),
        "trem32" => beat.tremolo = Some("1/8"),
        "down" => beat.brush = Some("Down"),
        "up" => beat.brush = Some("Up"),
        "slap" => beat.technique = Some("Slapped"),
        "pop" => beat.technique = Some("Popped"),
        "tap" => beat.technique = Some("Tapped"),
        "fade" => beat.fade_in = true,
        "dive" => beat.dive = true,
        m if m.starts_with('c') && m.len() > 1 => match m[1..].parse() {
            Ok(i) => beat.chord = Some(i),
            Err(_) => return err(token, "a chord mark is @c and a number"),
        },
        m if m.starts_with('t') && m.len() > 1 => beat.text = Some(m[1..].replace('_', " ")),
        _ => return err(token, "unknown beat mark"),
    }
    Ok(())
}

fn note(part: &str) -> Result<Note, ParseError> {
    let (tied, body) = match part.strip_prefix('~') {
        Some(b) => (true, b),
        None => (false, part),
    };
    let (at, effects) = if let Some((s, rest)) = body.split_once(':') {
        let string: u8 = s.parse().or_else(|_| err(part, "bad string"))?;
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let fret: u8 = rest[..digits]
            .parse()
            .or_else(|_| err(part, "a fretted note is string:fret"))?;
        if string == 0 {
            return err(part, "strings count from 1");
        }
        (At::Fret(string, fret), &rest[digits..])
    } else if let Some(rest) = body.strip_prefix('d') {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let midi: u8 = rest[..digits]
            .parse()
            .or_else(|_| err(part, "a drum note is d and a MIDI number"))?;
        (At::Pitch(midi), &rest[digits..])
    } else {
        let (pitch, used) =
            pitch(body).ok_or_else(|| ParseError(format!("\"{part}\": not a note")))?;
        (At::Pitch(pitch), &body[used..])
    };
    let mut fx = Vec::new();
    let mut chars = effects.chars().peekable();
    while let Some(c) = chars.next() {
        let number = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
            let mut n = String::new();
            while let Some(d) = chars.next_if(char::is_ascii_digit) {
                n.push(d);
            }
            n.parse::<u8>().ok()
        };
        fx.push(match c {
            'h' => Fx::HammerPull,
            'b' => Fx::Bend,
            'B' => Fx::HalfBend,
            'R' => Fx::BendRelease,
            'x' => Fx::Dead,
            'g' => Fx::Ghost,
            'm' => Fx::PalmMute,
            'v' => Fx::Vibrato,
            'l' => Fx::LetRing,
            '>' => Fx::Accent,
            '^' => Fx::HeavyAccent,
            '.' => Fx::Staccato,
            '/' => Fx::Slide(1),
            'S' => Fx::Slide(2),
            'd' => Fx::Slide(4),
            'u' => Fx::Slide(8),
            'i' => Fx::Slide(16),
            'I' => Fx::Slide(32),
            'n' => Fx::NaturalHarmonic,
            'a' => Fx::ArtificialHarmonic,
            'p' => Fx::PinchHarmonic,
            'T' => match number(&mut chars) {
                Some(f) => Fx::Trill(f),
                None => return err(part, "a trill is T and a fret"),
            },
            'f' => match number(&mut chars) {
                Some(f) if f <= 4 => Fx::Finger(f),
                _ => return err(part, "a finger is f and 0 to 4"),
            },
            _ => return err(part, "unknown note effect"),
        });
    }
    Ok(Note { at, tied, fx })
}

/// A pitch name such as `C4`, `F#3` or `Bb2`: its MIDI number and how many
/// bytes it took.
fn pitch(text: &str) -> Option<(u8, usize)> {
    let b = text.as_bytes();
    let step: i32 = match b.first()? {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut alter = 0;
    match b.get(i) {
        Some(b'#') => {
            alter = 1;
            i += 1;
        }
        Some(b'b') => {
            alter = -1;
            i += 1;
        }
        _ => {}
    }
    let octave = i32::from(*b.get(i)?) - i32::from(b'0');
    if !(0..=9).contains(&octave) {
        return None;
    }
    let midi = 12 * (octave + 1) + step + alter;
    u8::try_from(midi)
        .ok()
        .filter(|m| *m <= 127)
        .map(|m| (m, i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_tuplets() {
        let bars = parse("q 1:0 e. 1:2 s/3 r | h/5:3 C4").unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0][0].dur, Dur::new(4));
        assert_eq!(bars[0][1].dur.dots, 1);
        assert_eq!(bars[0][2].dur.tuplet, Some((3, 2)));
        assert!(bars[0][2].notes.is_empty());
        assert_eq!(bars[1][0].dur.tuplet, Some((5, 3)));
        assert_eq!(parse("e/5 r").unwrap()[0][0].dur.tuplet, Some((5, 4)));
        assert_eq!(parse("s/6 r").unwrap()[0][0].dur.tuplet, Some((6, 4)));
        assert_eq!(Dur::new(4).ticks(), 960);
        assert_eq!(parse("e/3 r").unwrap()[0][0].dur.ticks(), 320);
        assert_eq!(parse("q. r").unwrap()[0][0].dur.ticks(), 1440);
    }

    #[test]
    fn notes_effects_and_marks() {
        let bars = parse("@c0 @tlet_ring @ff q 6:0+5:2hm ~3:12b Bb2>. d36 F#5T7").unwrap();
        let b = &bars[0];
        assert_eq!(b[0].chord, Some(0));
        assert_eq!(b[0].text.as_deref(), Some("let ring"));
        assert_eq!(b[0].dynamic, Some("FF"));
        assert_eq!(b[0].notes[1].fx, vec![Fx::HammerPull, Fx::PalmMute]);
        assert_eq!(b[1].chord, None, "marks apply to one beat");
        assert!(b[1].notes[0].tied);
        assert_eq!(b[1].notes[0].at, At::Fret(3, 12));
        assert_eq!(b[2].notes[0].at, At::Pitch(46));
        assert_eq!(b[2].notes[0].fx, vec![Fx::Accent, Fx::Staccato]);
        assert_eq!(b[3].notes[0].at, At::Pitch(36));
        assert_eq!(b[4].notes[0].at, At::Pitch(78));
        assert_eq!(b[4].notes[0].fx, vec![Fx::Trill(7)]);
        assert_eq!(pitch("C4"), Some((60, 2)));
    }

    #[test]
    fn mistakes_are_reported() {
        assert!(parse("q 1:z").is_err());
        assert!(parse("q 1:0k").is_err());
        assert!(parse("@nope q r").is_err());
        assert!(parse("q H4").is_err());
        assert!(parse("e/1 r").is_err());
    }

    #[test]
    fn diagrams_from_text() {
        let am = Diagram::new("Am", "x02210");
        assert_eq!(am.frets[0], None);
        assert_eq!(am.frets[2], Some(2));
        assert_eq!(am.base_fret, 0);
        assert_eq!(Diagram::new("A", "577655").base_fret, 5);
    }
}
