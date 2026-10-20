// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Lyrics as Guitar Pro writes them (its guide, "Lyrics Editing", checked
//! 20 Oct 2026): one text per line (verse), from a bar, whose syllables go
//! one by one on the beats the first voice sings, from that bar on.
//!
//! In the text a space or a hyphen ends a syllable (a hyphen is drawn
//! after it, the word going on); each further space or hyphen leaves a
//! beat empty, and so does one before the first syllable; a line break
//! counts as one space, however many there are in a row; a plus joins two
//! words on one beat; text in square brackets is a comment, not sung.
//!
//! The engraver draws the syllables with [`syllables`] and the editor
//! types them in with [`set_syllable`], which changes only the syllable it
//! sets and keeps the rest of the text (comments, line breaks) as it was.

use crate::{Beat, BeatKind, Staff};

/// One syllable of a line, or an empty beat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Syllable {
    /// As drawn: a plus is a space. Empty for an empty beat.
    pub text: String,
    /// A hyphen follows: the word goes on.
    pub hyphen: bool,
    /// Where it is in the line's text, in bytes: an empty beat's start and
    /// end are both where its space or hyphen is.
    pub start: usize,
    /// See `start`.
    pub end: usize,
}

/// The syllables of a line, empty beats included (trailing ones left
/// out).
pub fn syllables(text: &str) -> Vec<Syllable> {
    let mut out: Vec<Syllable> = Vec::new();
    // The syllable being read: where it started.
    let mut current: Option<usize> = None;
    let mut comment = 0u32;
    // The last thing read was a line break (a run of them is one space).
    let mut after_break = false;
    let close = |out: &mut Vec<Syllable>, start: usize, end: usize, hyphen: bool| {
        let raw = text.get(start..end).unwrap_or_default();
        out.push(Syllable {
            text: without_comments(raw).replace('+', " "),
            hyphen,
            start,
            end,
        });
    };
    for (i, ch) in text.char_indices() {
        if comment > 0 {
            if ch == ']' {
                comment -= 1;
                // Space after a comment is no empty beat.
                after_break = current.is_none();
            }
            continue;
        }
        match ch {
            '[' => {
                // A comment ends the syllable before it, as a space would
                // not: "pre[x]tty" is one syllable.
                comment += 1;
            }
            '\n' | '\r' => {
                if let Some(start) = current.take() {
                    close(&mut out, start, i, false);
                } else if !after_break {
                    out.push(empty(i));
                }
                after_break = true;
            }
            ' ' | '\t' | '-' => {
                if let Some(start) = current.take() {
                    close(&mut out, start, i, ch == '-');
                } else if !after_break {
                    out.push(empty(i));
                }
                after_break = false;
            }
            _ => {
                if current.is_none() {
                    current = Some(i);
                }
                after_break = false;
            }
        }
    }
    if let Some(start) = current {
        close(&mut out, start, text.len(), false);
    }
    while out.last().is_some_and(|s| s.text.is_empty()) {
        out.pop();
    }
    out
}

/// `text` without its bracketed comments.
fn without_comments(text: &str) -> String {
    let mut depth = 0u32;
    text.chars()
        .filter(|c| match c {
            '[' => {
                depth += 1;
                false
            }
            ']' if depth > 0 => {
                depth -= 1;
                false
            }
            _ => depth == 0,
        })
        .collect()
}

fn empty(at: usize) -> Syllable {
    Syllable {
        text: String::new(),
        hyphen: false,
        start: at,
        end: at,
    }
}

/// Whether the first voice sings on `beat`: a beat with notes, not a grace
/// note, not all of them tied (a tied note continues the syllable before).
pub fn is_sung(beat: &Beat) -> bool {
    beat.grace.is_none()
        && beat.kind == BeatKind::Notes
        && !beat.notes.is_empty()
        && !beat.notes.iter().all(|n| n.tie)
}

/// The beats a line from `start_bar` sings on, in order, as (bar, beat) of
/// the staff's first voice.
pub fn sung_beats(staff: &Staff, start_bar: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (bar, b) in staff.bars.iter().enumerate().skip(start_bar) {
        let Some(v) = b.voices.first() else {
            continue;
        };
        for (i, beat) in v.beats.iter().enumerate() {
            if is_sung(beat) {
                out.push((bar, i));
            }
        }
    }
    out
}

/// The line's text with its syllable `index` (from 0) set to `syllable`
/// (empty to leave the beat empty), followed by a hyphen or not. Nothing
/// else of the text changes; a syllable past the last is reached with
/// spaces (hyphens at the start, where a file's reader trims spaces).
pub fn set_syllable(text: &str, index: usize, syllable: &str, hyphen: bool) -> String {
    let word: String = syllable
        .trim()
        .chars()
        .map(|c| match c {
            ' ' | '\t' => '+',
            '-' | '[' | ']' | '\n' | '\r' => '+',
            c => c,
        })
        .collect();
    let all = syllables(text);
    let mut out = String::with_capacity(text.len() + word.len() + 2);
    match all.get(index) {
        Some(s) => {
            out.push_str(text.get(..s.start).unwrap_or_default());
            out.push_str(&word);
            let rest = text.get(s.end..).unwrap_or_default();
            // The separator after it says whether a hyphen follows.
            let mut chars = rest.chars();
            match chars.next() {
                Some(c @ (' ' | '\t' | '-')) => {
                    // The separator after it (an empty beat's own space or
                    // hyphen) now ends the syllable.
                    let keep = if hyphen {
                        '-'
                    } else if c == '-' {
                        ' '
                    } else {
                        c
                    };
                    out.push(keep);
                    out.push_str(chars.as_str());
                }
                _ => {
                    if hyphen {
                        out.push('-');
                    } else if !rest.is_empty() && !rest.starts_with(['\n', '\r']) {
                        out.push(' ');
                    }
                    out.push_str(rest);
                }
            }
        }
        None => {
            let trimmed = text.trim_end_matches([' ', '\t']);
            out.push_str(trimmed);
            let have = all.len();
            if !trimmed.is_empty() && !trimmed.ends_with(['-', '\n', '\r']) {
                out.push(' ');
            }
            // Empty beats up to it: spaces between syllables, hyphens at
            // the start.
            let gap = if have == 0 && trimmed.trim().is_empty() {
                '-'
            } else {
                ' '
            };
            for _ in have..index {
                out.push(gap);
            }
            out.push_str(&word);
            if hyphen {
                out.push('-');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(text: &str) -> Vec<String> {
        syllables(text)
            .into_iter()
            .map(|s| {
                if s.hyphen {
                    format!("{}-", s.text)
                } else {
                    s.text
                }
            })
            .collect()
    }

    #[test]
    fn guitar_pro_s_syntax() {
        assert_eq!(drawn("Hel-lo world"), ["Hel-", "lo", "world"]);
        // Further spaces and hyphens leave beats empty.
        assert_eq!(drawn("So,    so"), ["So,", "", "", "", "so"]);
        assert_eq!(drawn("a--b"), ["a-", "", "b"]);
        assert_eq!(drawn("-a"), ["", "a"]);
        // Line breaks, however many, are one space.
        assert_eq!(
            drawn("tell \nHea-ven\n\nfrom"),
            ["tell", "", "Hea-", "ven", "from"]
        );
        // A plus joins, brackets hide.
        assert_eq!(drawn("[Verse 1]Got+to go"), ["Got to", "go"]);
        assert_eq!(drawn("[Verse 1] Got"), ["Got"]);
        assert_eq!(drawn("pre[x]tty"), ["pretty"]);
        assert_eq!(drawn("a  "), ["a"]);
        assert!(drawn("").is_empty());
    }

    #[test]
    fn a_syllable_is_set_and_nothing_else_changes() {
        // Replace one.
        assert_eq!(
            set_syllable("Hel-lo world", 2, "there", false),
            "Hel-lo there"
        );
        // A hyphen on, then off.
        assert_eq!(set_syllable("Hel lo", 0, "Hel", true), "Hel-lo");
        assert_eq!(set_syllable("Hel-lo", 0, "Hel", false), "Hel lo");
        // Into an empty beat.
        assert_eq!(set_syllable("a  b", 1, "x", false), "a x b");
        assert_eq!(drawn(&set_syllable("a  b", 1, "x", true)), ["a", "x-", "b"]);
        // Past the end: empty beats up to it.
        assert_eq!(set_syllable("a", 2, "x", false), "a  x");
        assert_eq!(set_syllable("", 0, "x", false), "x");
        assert_eq!(set_syllable("", 2, "x", false), "--x");
        assert_eq!(drawn("--x"), ["", "", "x"]);
        // Taken away: the beat is left empty.
        assert_eq!(drawn(&set_syllable("a x b", 1, "", false)), ["a", "", "b"]);
        // Comments and line breaks stay.
        assert_eq!(
            set_syllable("[V1] one\ntwo", 1, "three", false),
            "[V1] one\nthree"
        );
        // Spaces typed in a syllable join words.
        assert_eq!(set_syllable("", 0, "got to", false), "got+to");
    }
}
