// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A chord spelled out from its name, as Guitar Pro 7 keeps it beside each
//! diagram: the key note, the bass note and the intervals above the key.
//! Guitar Pro 7.6 drops a diagram that has no spelling (19 Oct 2026,
//! `fixtures/guitar-pro-7.6/README.md`).
//!
//! The name is read the way chord symbols are written in tabs: a root
//! (`C`, `F#`, `Bb`), then what the chord is (`m`, `7`, `maj7` or `7M`,
//! `sus4`, `add9`, `°`, `Ø`, `5`, `+` and so on), then a bass note after a
//! slash. What can't be read is left out, and a name with no root is spelled
//! as Guitar Pro spells a chord it knows nothing about: C, with no intervals.

/// A note name: its letter and accidental, as GPIF writes them
/// (`step="F" accidental="Sharp"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NoteName {
    pub(super) step: char,
    /// Semitones from the letter: -2 to 2.
    pub(super) alter: i8,
}

impl NoteName {
    const C: NoteName = NoteName {
        step: 'C',
        alter: 0,
    };

    /// GPIF's name for the accidental.
    pub(super) fn accidental(self) -> &'static str {
        match self.alter {
            -2 => "DoubleFlat",
            -1 => "Flat",
            1 => "Sharp",
            2 => "DoubleSharp",
            _ => "Natural",
        }
    }

    /// The pitch class, 0 (C) to 11.
    fn pitch_class(self) -> u8 {
        let natural: i8 = match self.step {
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => 0,
        };
        u8::try_from((natural + self.alter).rem_euclid(12)).unwrap_or(0)
    }
}

/// One interval above the key note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Degree {
    /// `Second`, `Third` … `Thirteenth`.
    pub(super) interval: &'static str,
    /// `Major`, `Minor`, `Perfect`, `Diminished` or `Augmented`.
    pub(super) alteration: &'static str,
    /// Semitones above the key note, within the octave.
    semitones: u8,
    /// Whether the diagram leaves it out.
    pub(super) omitted: bool,
}

/// A chord's spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Spelling {
    pub(super) key: NoteName,
    pub(super) bass: NoteName,
    pub(super) degrees: Vec<Degree>,
}

impl Spelling {
    /// Marks the intervals that none of `pitches` (MIDI numbers of the
    /// diagram's notes) sound as left out, as Guitar Pro does. A diagram
    /// without notes leaves nothing out.
    pub(super) fn mark_omitted(&mut self, pitches: &[u8]) {
        if pitches.is_empty() {
            return;
        }
        let key = self.key.pitch_class();
        for d in &mut self.degrees {
            let pc = (key + d.semitones) % 12;
            d.omitted = !pitches.iter().any(|p| p % 12 == pc);
        }
    }
}

/// Reads a note name at the start of `s`: the note and what follows it.
fn note_name(s: &str) -> Option<(NoteName, &str)> {
    let mut chars = s.chars();
    let step = chars.next()?.to_ascii_uppercase();
    if !('A'..='G').contains(&step) {
        return None;
    }
    let mut rest = chars.as_str();
    let mut alter = 0i8;
    loop {
        if let Some(r) = rest.strip_prefix('#').or_else(|| rest.strip_prefix('♯')) {
            alter += 1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('♭') {
            alter -= 1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('b').filter(|_| alter <= 0) {
            alter -= 1;
            rest = r;
        } else {
            break;
        }
        if alter.abs() >= 2 {
            break;
        }
    }
    Some((NoteName { step, alter }, rest))
}

const fn degree(interval: &'static str, alteration: &'static str, semitones: u8) -> Degree {
    Degree {
        interval,
        alteration,
        semitones,
        omitted: false,
    }
}

const MAJOR_THIRD: Degree = degree("Third", "Major", 4);
const MINOR_THIRD: Degree = degree("Third", "Minor", 3);
const SECOND: Degree = degree("Second", "Major", 2);
const FOURTH: Degree = degree("Fourth", "Perfect", 5);
const FIFTH: Degree = degree("Fifth", "Perfect", 7);
const FLAT_FIFTH: Degree = degree("Fifth", "Diminished", 6);
const SHARP_FIFTH: Degree = degree("Fifth", "Augmented", 8);
const SIXTH: Degree = degree("Sixth", "Major", 9);
const MINOR_SEVENTH: Degree = degree("Seventh", "Minor", 10);
const MAJOR_SEVENTH: Degree = degree("Seventh", "Major", 11);
const DIMINISHED_SEVENTH: Degree = degree("Seventh", "Diminished", 9);

/// An extension (9, 11, 13) with its alteration: -1 flat, 0, 1 sharp.
fn extension(number: u8, alter: i8) -> Option<Degree> {
    let (interval, natural) = match number {
        9 => ("Ninth", 2),
        11 => ("Eleventh", 5),
        13 => ("Thirteenth", 9),
        _ => return None,
    };
    let alteration = match alter {
        -1 => "Diminished",
        1 => "Augmented",
        _ => "Perfect",
    };
    let semitones = u8::try_from((natural + alter).rem_euclid(12)).ok()?;
    Some(degree(interval, alteration, semitones))
}

/// Spells the chord named `name`.
pub(super) fn spell(name: &str) -> Spelling {
    let unknown = Spelling {
        key: NoteName::C,
        bass: NoteName::C,
        degrees: Vec::new(),
    };
    let name = name.trim();
    let Some((key, rest)) = note_name(name) else {
        return unknown;
    };
    // The bass note after the last slash, if it is a note; "6/9" is not.
    let (quality, bass) = match rest.rsplit_once('/') {
        Some((q, b)) => match note_name(b.trim()) {
            Some((n, tail)) if tail.trim().is_empty() => (q, n),
            _ => (rest, key),
        },
        None => (rest, key),
    };
    let mut q: String = quality
        .chars()
        .filter(|c| !matches!(c, '(' | ')' | ' ' | ','))
        .collect();

    let mut third = Some(MAJOR_THIRD);
    let mut fifth = Some(FIFTH);
    let mut sixth = None;
    let mut seventh = None;
    let mut extensions: Vec<Degree> = Vec::new();
    let mut added: Vec<Degree> = Vec::new();

    // What the chord is, in the order chord symbols put it.
    if take(&mut q, "Ø") || take(&mut q, "ø") || take(&mut q, "m7b5") {
        third = Some(MINOR_THIRD);
        fifth = Some(FLAT_FIFTH);
        seventh = Some(MINOR_SEVENTH);
    } else if take(&mut q, "dim7") || take(&mut q, "°7") || take(&mut q, "o7") {
        third = Some(MINOR_THIRD);
        fifth = Some(FLAT_FIFTH);
        seventh = Some(DIMINISHED_SEVENTH);
    } else if take(&mut q, "dim") || take(&mut q, "°") {
        third = Some(MINOR_THIRD);
        fifth = Some(FLAT_FIFTH);
    } else if take(&mut q, "aug") || take(&mut q, "+") {
        fifth = Some(SHARP_FIFTH);
    } else if q.starts_with("maj") || q.starts_with("Maj") || q.starts_with('M') {
        // Major: "M", or a major seventh such as "maj7" or "M9", read below.
    } else if take(&mut q, "min") || take(&mut q, "mi") || take(&mut q, "m") || take(&mut q, "-") {
        third = Some(MINOR_THIRD);
    }
    if q == "5" {
        // A power chord: the root and fifth only.
        third = None;
        q.clear();
    }
    let major_seventh = take(&mut q, "maj") || take(&mut q, "Maj") || take(&mut q, "M");
    if take(&mut q, "6/9") || take(&mut q, "69") {
        sixth = Some(SIXTH);
        added.extend(extension(9, 0));
    } else if take(&mut q, "6") {
        sixth = Some(SIXTH);
    }
    // Sevenths and the extensions that imply them; "7M" and "7+" (as
    // written in Brazil) are major sevenths.
    for number in [13u8, 11, 9, 7] {
        if take(&mut q, &number.to_string()) {
            let major = major_seventh || take(&mut q, "M") || take(&mut q, "+");
            seventh.get_or_insert(if major { MAJOR_SEVENTH } else { MINOR_SEVENTH });
            // A ninth chord holds the ninth; an eleventh the ninth and
            // eleventh; a thirteenth the ninth and thirteenth.
            if number >= 9 {
                extensions.extend(extension(9, 0));
            }
            if number >= 11 {
                extensions.extend(extension(number, 0));
            }
            break;
        }
    }
    // Suspensions, additions and alterations, in any order.
    let mut guard = 0;
    while !q.is_empty() && guard < 16 {
        guard += 1;
        if take(&mut q, "sus2") {
            third = Some(SECOND);
        } else if take(&mut q, "sus4") || take(&mut q, "sus") {
            third = Some(FOURTH);
        } else if take(&mut q, "add") {
            let n = leading_number(&mut q);
            match n {
                Some(2) => added.push(SECOND),
                Some(4) | Some(11) => added.extend(extension(11, 0)),
                Some(n) => added.extend(extension(n, 0)),
                None => {}
            }
        } else if let Some((alter, number)) = alteration(&mut q) {
            match number {
                5 => {
                    fifth = Some(match alter {
                        -1 => FLAT_FIFTH,
                        1 => SHARP_FIFTH,
                        _ => FIFTH,
                    });
                }
                n => {
                    if let Some(d) = extension(n, alter) {
                        extensions.retain(|e| e.interval != d.interval);
                        extensions.push(d);
                    }
                }
            }
        } else if take(&mut q, "M") || take(&mut q, "+") {
            // "7M" or "7+" after the number: the seventh is major.
            if seventh.is_some() {
                seventh = Some(MAJOR_SEVENTH);
            } else {
                fifth = Some(SHARP_FIFTH);
            }
        } else {
            // Something we can't read: skip a character.
            let mut chars = q.chars();
            chars.next();
            q = chars.as_str().to_owned();
        }
    }

    let mut degrees: Vec<Degree> = Vec::new();
    let mut push = |d: Option<Degree>| {
        if let Some(d) = d
            && !degrees.iter().any(|x| x.interval == d.interval)
        {
            degrees.push(d);
        }
    };
    // Guitar Pro lists a suspended second as the Second, in interval order.
    push(third.filter(|t| t.interval == "Second"));
    push(third.filter(|t| t.interval == "Third"));
    push(third.filter(|t| t.interval == "Fourth"));
    for a in added.iter().filter(|a| a.interval == "Second") {
        push(Some(*a));
    }
    push(fifth);
    push(sixth);
    push(seventh);
    let mut rest: Vec<Degree> = extensions;
    rest.extend(added.into_iter().filter(|a| a.interval != "Second"));
    for name in ["Ninth", "Eleventh", "Thirteenth"] {
        push(rest.iter().find(|d| d.interval == name).copied());
    }
    Spelling { key, bass, degrees }
}

/// Takes `prefix` off the start of `q`, if it is there.
fn take(q: &mut String, prefix: &str) -> bool {
    if q.starts_with(prefix) {
        q.drain(..prefix.len());
        true
    } else {
        false
    }
}

/// A number at the start of `q`, taken off it.
fn leading_number(q: &mut String) -> Option<u8> {
    let digits: String = q.chars().take_while(char::is_ascii_digit).take(2).collect();
    if digits.is_empty() {
        return None;
    }
    q.drain(..digits.len());
    digits.parse().ok()
}

/// An alteration such as `b9`, `#11`, `b5`, `5-` or `-5`, or a plain `9`:
/// (-1, 0 or 1, number).
fn alteration(q: &mut String) -> Option<(i8, u8)> {
    let sign = |c: char| match c {
        'b' | '♭' | '-' => Some(-1i8),
        '#' | '♯' | '+' => Some(1),
        _ => None,
    };
    let first = q.chars().next()?;
    if let Some(alter) = sign(first) {
        let mut rest = q[first.len_utf8()..].to_owned();
        if let Some(n) = leading_number(&mut rest).filter(|n| matches!(n, 5 | 9 | 11 | 13)) {
            *q = rest;
            return Some((alter, n));
        }
        return None;
    }
    // A number followed by its sign ("5-", "5+", "13-"), or a plain
    // extension ("9" in "7M9").
    let mut rest = q.clone();
    let n = leading_number(&mut rest).filter(|n| matches!(n, 5 | 9 | 11 | 13))?;
    let alter = match rest.chars().next().and_then(sign) {
        Some(alter) => {
            rest.drain(..rest.chars().next().map_or(0, char::len_utf8));
            alter
        }
        None if n == 5 => return None,
        None => 0,
    };
    *q = rest;
    Some((alter, n))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    fn short(name: &str) -> String {
        let s = spell(name);
        let note = |n: NoteName| {
            let acc = match n.alter {
                -1 => "b",
                1 => "#",
                _ => "",
            };
            format!("{}{acc}", n.step)
        };
        let degrees: Vec<String> = s
            .degrees
            .iter()
            .map(|d| format!("{} {}", d.alteration, d.interval))
            .collect();
        format!("{} /{}: {}", note(s.key), note(s.bass), degrees.join(", "))
    }

    #[test]
    fn spellings_guitar_pro_agrees_with() {
        // As Guitar Pro 7.6 spells these, read from files of the corpus.
        assert_eq!(short("C"), "C /C: Major Third, Perfect Fifth");
        assert_eq!(short("Am"), "A /A: Minor Third, Perfect Fifth");
        assert_eq!(
            short("Am7"),
            "A /A: Minor Third, Perfect Fifth, Minor Seventh"
        );
        assert_eq!(
            short("Bb7M"),
            "Bb /Bb: Major Third, Perfect Fifth, Major Seventh"
        );
        assert_eq!(
            short("C7M9"),
            "C /C: Major Third, Perfect Fifth, Major Seventh, Perfect Ninth"
        );
        assert_eq!(
            short("F#Ø"),
            "F# /F#: Minor Third, Diminished Fifth, Minor Seventh"
        );
        assert_eq!(short("D/F#"), "D /F#: Major Third, Perfect Fifth");
        assert_eq!(
            short("E7/G#"),
            "E /G#: Major Third, Perfect Fifth, Minor Seventh"
        );
        assert_eq!(short("Gm/F"), "G /F: Minor Third, Perfect Fifth");
        assert_eq!(short("Dsus2"), "D /D: Major Second, Perfect Fifth");
        assert_eq!(short("Gsus4"), "G /G: Perfect Fourth, Perfect Fifth");
        assert_eq!(
            short("A7sus4"),
            "A /A: Perfect Fourth, Perfect Fifth, Minor Seventh"
        );
        assert_eq!(
            short("Gadd9"),
            "G /G: Major Third, Perfect Fifth, Perfect Ninth"
        );
    }

    #[test]
    fn other_spellings() {
        assert_eq!(short("E5"), "E /E: Perfect Fifth");
        assert_eq!(short("Bdim"), "B /B: Minor Third, Diminished Fifth");
        assert_eq!(
            short("Bdim7"),
            "B /B: Minor Third, Diminished Fifth, Diminished Seventh"
        );
        assert_eq!(short("Caug"), "C /C: Major Third, Augmented Fifth");
        assert_eq!(
            short("Cmaj7"),
            "C /C: Major Third, Perfect Fifth, Major Seventh"
        );
        assert_eq!(
            short("C7(b9)"),
            "C /C: Major Third, Perfect Fifth, Minor Seventh, Diminished Ninth"
        );
        assert_eq!(
            short("Am7(b5)"),
            "A /A: Minor Third, Diminished Fifth, Minor Seventh"
        );
        assert_eq!(short("F#m/5+"), "F# /F#: Minor Third, Augmented Fifth");
        assert_eq!(short("G6"), "G /G: Major Third, Perfect Fifth, Major Sixth");
        assert_eq!(
            short("C13"),
            "C /C: Major Third, Perfect Fifth, Minor Seventh, Perfect Ninth, Perfect Thirteenth"
        );
        assert_eq!(
            short("Dmadd9"),
            "D /D: Minor Third, Perfect Fifth, Perfect Ninth"
        );
        // Nothing to read: spelled as Guitar Pro spells an unknown chord.
        assert_eq!(short("N.C."), "C /C: ");
        assert_eq!(short(""), "C /C: ");
    }

    #[test]
    fn omitted_intervals_follow_the_diagram() {
        // C7M9 as x32430: C E B D, no G.
        let mut s = spell("C7M9");
        s.mark_omitted(&[48, 52, 59, 62]);
        let omitted: Vec<&str> = s
            .degrees
            .iter()
            .filter(|d| d.omitted)
            .map(|d| d.interval)
            .collect();
        assert_eq!(omitted, vec!["Fifth"]);
    }
}
