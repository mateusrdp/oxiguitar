// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Chord names in any convention (#278): a name read into its notes, the
//! names a set of notes can take, the ways to play a name on a track's
//! strings, and names rewritten in a chord notation style.
//!
//! Names are read and notes are named by the
//! [chordparser](https://github.com/PauSala/chordparser) crate (Pau Sala,
//! MIT). It reads `Em7-5`, `Em7b5`, `Eø`, `C△7`, `CM7`, `F-`, `Fmin`,
//! `G°7`, `D+` and the like, and gives every chord one normalised spelling
//! (`Emi7(b5)`, `CMa7`, `Fmi`), which is what two names are compared by:
//! here a chord's *kind* is that spelling without its root and bass.
//!
//! A [`Style`] is a list of aliases, kind → text, read from settings.json
//! by the app (Mateus, 15 Nov 2026: compared by meaning, the whole kind
//! at once). A name whose kind has no alias in a style stays as written.

use chordparser::{inference, parsing::Parser};

use crate::harmony::{self, DEFAULT_NAME, NOTE_NAMES};

/// A chord name, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    /// The root's pitch class (0 = C).
    pub root: u8,
    /// The pitch class of a bass note written after a slash.
    pub bass: Option<u8>,
    /// The chord's kind: the library's normalised spelling of everything
    /// but the root and the bass (`mi7(b5)`; empty for a major triad).
    pub kind: String,
    /// The pitch classes it sounds, the root's first.
    classes: Vec<u8>,
    /// The pitch classes a way to play it may leave out.
    optional: Vec<u8>,
}

impl Reading {
    /// The pitch classes the chord sounds, the root's first, a bass note
    /// that is no chord note last.
    pub fn classes(&self) -> &[u8] {
        &self.classes
    }

    /// Whether `other` is the same chord: root, kind and bass.
    pub fn same(&self, other: &Reading) -> bool {
        self.root == other.root && self.kind == other.kind && self.bass == other.bass
    }

    /// Whether the notes `pitches` (MIDI) are this chord: every note it
    /// needs and no other, over its bass.
    pub fn is_played_by(&self, pitches: &[u8]) -> bool {
        let sounds = |class: &u8| pitches.iter().any(|p| p % 12 == *class);
        pitches.iter().min().map(|p| p % 12) == Some(self.bass.unwrap_or(self.root))
            && pitches.iter().all(|p| self.classes.contains(&(p % 12)))
            && self
                .classes
                .iter()
                .all(|c| sounds(c) || self.optional.contains(c))
    }

    /// Ways to play the chord on strings tuned `tuning` (MIDI pitches,
    /// highest string first) within `frets` frets: frets per string
    /// (highest first; `None` not played), easiest first, at most `limit`.
    pub fn voicings(&self, tuning: &[u8], frets: u8, limit: usize) -> Vec<Vec<Option<u8>>> {
        harmony::shapes(
            &self.classes,
            &self.optional,
            self.bass.unwrap_or(self.root),
            self.classes.len().clamp(2, 3),
            tuning,
            frets,
            limit,
        )
    }
}

/// A name's three parts as written: root, kind and the bass after a
/// slash. `None` when it doesn't start with a note name.
fn parts(name: &str) -> Option<(&str, &str, Option<&str>)> {
    let name = name.trim();
    let root = note_len(name)?;
    let (root, rest) = name.split_at(root);
    match rest.rsplit_once('/') {
        Some((kind, bass)) if note_len(bass) == Some(bass.len()) => Some((root, kind, Some(bass))),
        _ => Some((root, rest, None)),
    }
}

/// The length in bytes of the note name `text` starts with: a letter A to
/// G and one sharp or flat sign, if any.
fn note_len(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let letter = chars.next().filter(|c| ('A'..='G').contains(c))?;
    let sign = chars
        .next()
        .filter(|c| matches!(c, '#' | 'b' | '♯' | '♭'))
        .map_or(0, char::len_utf8);
    Some(letter.len_utf8() + sign)
}

/// The name a pitch class takes when OxiGuitar names a chord: sharps for
/// C♯ and F♯, flats for the rest, as Harmonize writes them.
fn spell(class: u8) -> &'static str {
    DEFAULT_NAME
        .get(usize::from(class % 12))
        .and_then(|i| NOTE_NAMES.get(*i))
        .copied()
        .unwrap_or("C")
}

/// Reads a chord name written in any convention the library knows.
/// `None` for one it can't read (kept as written wherever it is used).
pub fn read(name: &str) -> Option<Reading> {
    read_with(&mut Parser::new(), name)
}

fn read_with(parser: &mut Parser, name: &str) -> Option<Reading> {
    let chord = parser.parse(name.trim()).ok()?;
    let root = chord.root.to_semitone() % 12;
    let bass = chord.bass.map(|b| b.to_semitone() % 12);
    // The normalised name is the root, the kind, and a slash and the bass.
    let (_, mut kind, _) = parts(&chord.normalized)?;
    if bass.is_none() {
        kind = chord.normalized.get(note_len(&chord.normalized)?..)?;
    }
    let mut classes: Vec<u8> = Vec::new();
    for s in &chord.semitones {
        let class = (root + s % 12) % 12;
        if !classes.contains(&class) {
            classes.push(class);
        }
    }
    // What a guitarist leaves out first: the fifth of a chord of four
    // notes or more, and under an eleventh or thirteenth the ninth, under
    // a thirteenth the eleventh.
    let has = |s: u8| chord.semitones.contains(&s);
    let mut optional = Vec::new();
    if has(7) && classes.len() > 3 {
        optional.push((root + 7) % 12);
    }
    if has(14) && (has(17) || has(21)) {
        optional.push((root + 2) % 12);
    }
    if has(17) && has(21) {
        optional.push((root + 5) % 12);
    }
    if let Some(b) = bass
        && !classes.contains(&b)
    {
        classes.push(b);
    }
    Some(Reading {
        root,
        bass,
        kind: kind.to_owned(),
        classes,
        optional,
    })
}

/// The names the notes `pitches` (MIDI) can take: one for each of them
/// taken as the root, the lowest note's first, the others over the lowest
/// note as their bass. One note alone is named by its note. The names are
/// in the library's normalised spelling; a [`Style`] rewrites them.
pub fn names(pitches: &[u8]) -> Vec<String> {
    let mut sorted = pitches.to_vec();
    sorted.sort_unstable();
    let Some(lowest) = sorted.first().copied() else {
        return Vec::new();
    };
    if sorted.iter().all(|p| p % 12 == lowest % 12) {
        return vec![spell(lowest).to_owned()];
    }
    let mut parser = Parser::new();
    let mut out: Vec<String> = Vec::new();
    for candidate in inference::descriptors_from_midi_codes(&sorted) {
        // The library's candidates are not all chords: it says to check.
        let Some(r) = read_with(&mut parser, &candidate) else {
            continue;
        };
        let mut name = format!("{}{}", spell(r.root), r.kind);
        if let Some(b) = r.bass {
            name.push('/');
            name.push_str(spell(b));
        }
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// One alias of a style.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Alias {
    /// The key as settings.json writes it (`m7b5`).
    key: String,
    /// The kind the key means (`mi7(b5)`).
    kind: String,
    /// What the style writes for it (`ø`).
    text: String,
}

/// A chord notation style: what to write for each kind of chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    /// Its name, as the chord dialog lists it.
    pub name: String,
    aliases: Vec<Alias>,
}

/// Parentheses and spaces out of a kind as written, so `7(b9)` and `7b9`
/// compare alike.
fn bare(kind: &str) -> String {
    kind.chars()
        .filter(|c| !matches!(c, '(' | ')' | ' '))
        .collect()
}

impl Style {
    /// A style named `name` with `aliases`: for each, the kind as any
    /// convention writes it and the text this style writes instead. A key
    /// the library can't read as a kind of chord is left out.
    pub fn new(name: &str, aliases: impl IntoIterator<Item = (String, String)>) -> Style {
        let mut parser = Parser::new();
        let aliases = aliases
            .into_iter()
            .filter_map(|(key, text)| {
                let kind = read_with(&mut parser, &format!("C{key}"))
                    .filter(|r| r.root == 0 && r.bass.is_none())?
                    .kind;
                Some(Alias { key, kind, text })
            })
            .collect();
        Style {
            name: name.to_owned(),
            aliases,
        }
    }

    /// The alias whose text is the kind `written`, parentheses apart.
    fn written(&self, written: &str) -> Option<&Alias> {
        let written = bare(written);
        self.aliases
            .iter()
            .find(|a| !written.is_empty() && bare(&a.text) == written)
    }
}

/// The chord notation styles of settings.json.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Styles(Vec<Style>);

impl Styles {
    /// The styles, in the order the chord dialog lists them.
    pub fn new(styles: Vec<Style>) -> Styles {
        Styles(styles)
    }

    /// Their names.
    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|s| s.name.clone()).collect()
    }

    /// The style named `name`.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.0.iter().position(|s| s.name == name)
    }

    /// Reads `name` as style `style` writes names: what the style itself
    /// writes for a kind comes first, then every convention the library
    /// knows, then what the other styles write (for texts of the user's
    /// own that the library can't read).
    pub fn read(&self, style: usize, name: &str) -> Option<Reading> {
        let (root, kind, bass) = parts(name)?;
        let through = |s: &Style| {
            let alias = s.written(kind)?;
            let slash = bass.map_or(String::new(), |b| format!("/{b}"));
            read(&format!("{root}{}{slash}", alias.key))
        };
        self.0
            .get(style)
            .and_then(through)
            .or_else(|| read(name))
            .or_else(|| self.0.iter().find_map(through))
    }

    /// `name` as style `style` writes it: its root and bass as they are,
    /// its kind by the style's alias. A name that can't be read, or whose
    /// kind the style has no alias for, comes back as it is.
    pub fn write(&self, style: usize, name: &str) -> String {
        let rewritten = || {
            let (root, _, bass) = parts(name)?;
            let kind = self.read(style, name)?.kind;
            let alias = self.0.get(style)?.aliases.iter().find(|a| a.kind == kind)?;
            let slash = bass.map_or(String::new(), |b| format!("/{b}"));
            Some(format!("{root}{}{slash}", alias.text))
        };
        rewritten().unwrap_or_else(|| name.to_owned())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    /// Standard tuning, highest string first.
    const E: [u8; 6] = [64, 59, 55, 50, 45, 40];

    fn pitches(low_first: [Option<u8>; 6]) -> Vec<u8> {
        low_first
            .iter()
            .rev()
            .zip(E)
            .filter_map(|(f, open)| f.map(|f| open + f))
            .collect()
    }

    fn styles() -> Styles {
        let style = |name: &str, aliases: &[(&str, &str)]| {
            Style::new(
                name,
                aliases
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
            )
        };
        Styles::new(vec![
            style("Custom", &[]),
            style(
                "Popular",
                &[
                    ("m", "m"),
                    ("m7b5", "m7b5"),
                    ("maj7", "maj7"),
                    ("aug", "aug"),
                ],
            ),
            style(
                "Jazz",
                &[
                    ("m", "-"),
                    ("m7b5", "ø7"),
                    ("maj7", "△7"),
                    ("aug", "+"),
                    ("7b9", "7(b9)"),
                    ("not a chord", "?"),
                    ("13", "thirteen"),
                ],
            ),
        ])
    }

    #[test]
    fn names_are_read_in_any_convention() {
        for name in ["Em7-5", "Em7b5", "Em7(b5)", "Eø", "E-7b5"] {
            let r = read(name).unwrap();
            assert_eq!((r.root, r.kind.as_str(), r.bass), (4, "mi7(b5)", None));
            assert_eq!(r.classes(), [4, 7, 10, 2], "{name}");
        }
        for name in ["C△7", "Cmaj7", "CMAJ7", "CM7"] {
            assert_eq!(read(name).unwrap().kind, "Ma7", "{name}");
        }
        let over = read("Cm7b5/Gb").unwrap();
        assert_eq!((over.root, over.bass), (0, Some(6)));
        // A bass that is no chord note is sounded too.
        assert_eq!(read("C/D").unwrap().classes(), [0, 4, 7, 2]);
        assert_eq!(read("C♯m").unwrap().root, 1);
        assert_eq!(read("C6/9").unwrap().bass, None);
        for name in ["", "H7", "G7 (my way)", "x 0 2 2 1 0"] {
            assert_eq!(read(name), None, "{name}");
        }
    }

    #[test]
    fn notes_are_named_on_every_root() {
        // #278's example: x 2 3 2 3 x.
        let n = names(&pitches([None, Some(2), Some(3), Some(2), Some(3), None]));
        assert_eq!(n[0], "Bmi7(b5)");
        assert!(n.contains(&"F6(b5)/B".to_owned()), "{n:?}");
        assert!(n.contains(&"Dmi6/B".to_owned()), "{n:?}");
        // One note is its own name; two make what they can.
        assert_eq!(names(&[47]), ["B"]);
        assert_eq!(names(&[47, 59]), ["B"]);
        assert_eq!(names(&[]), Vec::<String>::new());
        let am = names(&pitches([
            None,
            Some(0),
            Some(2),
            Some(2),
            Some(1),
            Some(0),
        ]));
        assert_eq!(am[0], "Ami");
        // Roots are spelled as Harmonize spells them.
        assert_eq!(names(&[54, 58, 61])[0], "F#");
        assert_eq!(names(&[58, 62, 65])[0], "Bb");
    }

    #[test]
    fn a_name_is_played_on_the_strings() {
        let am = read("Am").unwrap().voicings(&E, 24, 12);
        assert_eq!(am[0], [Some(0), Some(1), Some(2), Some(2), Some(0), None]);
        // The same shapes whatever the convention.
        assert_eq!(read("A-").unwrap().voicings(&E, 24, 12), am);
        let half = read("Bø").unwrap().voicings(&E, 24, 12);
        assert!(half.contains(&vec![None, Some(3), Some(2), Some(3), Some(2), None]));
        // The bass is the lowest note.
        let over = read("D/F#").unwrap().voicings(&E, 24, 4);
        assert_eq!(over[0][5], Some(2));
        // Every one of them is the chord, and other notes are not.
        let sounded = |shape: &[Option<u8>]| -> Vec<u8> {
            shape
                .iter()
                .zip(E)
                .filter_map(|(f, open)| f.map(|f| open + f))
                .collect()
        };
        let a_minor = read("Am").unwrap();
        assert!(am.iter().all(|shape| a_minor.is_played_by(&sounded(shape))));
        assert!(!a_minor.is_played_by(&[45, 52, 57, 61]));
        assert!(!a_minor.is_played_by(&[52, 57, 60]), "E is not its bass");
        assert!(read("Am/E").unwrap().is_played_by(&[52, 57, 60]));
    }

    #[test]
    fn a_style_rewrites_the_kind_and_nothing_else() {
        let s = styles();
        let (custom, popular, jazz) = (0, 1, 2);
        assert_eq!(s.write(jazz, "Em7(b5)"), "Eø7");
        assert_eq!(s.write(jazz, "F#m/C#"), "F#-/C#");
        assert_eq!(s.write(jazz, "B♭maj7"), "B♭△7");
        assert_eq!(s.write(jazz, "C7b9"), "C7(b9)");
        // Back again, and from one spelling to another.
        assert_eq!(s.write(popular, "Eø7"), "Em7b5");
        assert_eq!(s.write(popular, "E-7b5"), "Em7b5");
        assert_eq!(s.write(popular, "D+"), "Daug");
        // No alias, no change: the kind as a whole is what counts.
        assert_eq!(s.write(popular, "Cmaj7#11"), "Cmaj7#11");
        assert_eq!(s.write(jazz, "G7 (my way)"), "G7 (my way)");
        assert_eq!(s.write(jazz, "C"), "C");
        // The empty style changes nothing.
        for name in ["Eø7", "Em7-5", "nonsense"] {
            assert_eq!(s.write(custom, name), name);
        }
        assert_eq!(s.write(9, "Em"), "Em");
    }

    #[test]
    fn a_style_s_own_texts_are_read_back() {
        let s = styles();
        // "thirteen" is nothing the library reads, yet Jazz wrote it.
        assert_eq!(s.write(2, "C13"), "Cthirteen");
        assert_eq!(s.read(2, "Cthirteen").unwrap().kind, "13");
        assert_eq!(s.read(1, "Cthirteen/E").unwrap().bass, Some(4));
        // A key that is no kind of chord is left out.
        assert_eq!(s.read(2, "C?"), None);
        assert_eq!(s.names(), ["Custom", "Popular", "Jazz"]);
        assert_eq!(s.position("Jazz"), Some(2));
    }
}
