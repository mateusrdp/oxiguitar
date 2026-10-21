// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Inspector's view of the caret's bar (M3; Mateus, 20 Oct 2026: "it
//! greatly facilitates debugging and it is essential for composing"):
//! each voice's beats counted against the time signature, in the
//! signature's own unit (quarters in 4/4, eighths in 6/8), and whether the
//! voice is full, short or too long, and by how much. Without window code.

use oxiguitar_edit::{Caret, onsets};
use oxiguitar_model::{BeatKind, Ratio, Score};

/// How a voice fills its bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fill {
    /// Exactly the time signature.
    Full,
    /// Less than it.
    Short,
    /// More than it.
    Long,
    /// No beats (or only placeholders), or a bar in free time, or a beat
    /// whose length can't be counted: nothing to compare.
    Unknown,
}

/// One voice's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VoiceRow {
    /// "Voice 1".
    pub(crate) name: String,
    /// How much it holds against the time signature, as Guitar Pro 8 shows a
    /// bar: "1.5/4" is a quarter and an eighth in 2/4 (issue #104).
    pub(crate) count: String,
    /// What is missing or too much: "Full", "½ beat short", "1¼ beats too
    /// long", "Empty".
    pub(crate) state: String,
    pub(crate) fill: Fill,
    /// How many beats (notes, chords and rests) were entered.
    pub(crate) entered: usize,
    /// The caret is in this voice.
    pub(crate) current: bool,
}

/// What the Inspector shows of the caret's bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BarView {
    /// "Bar 5 · 4/4".
    pub(crate) heading: String,
    /// What a beat is here: "Beats are quarter notes".
    pub(crate) unit: String,
    /// One row per voice that has beats, and the caret's voice.
    pub(crate) voices: Vec<VoiceRow>,
    /// Where the caret's beat starts: "The caret's beat starts on beat 2½".
    pub(crate) caret: String,
}

/// The view of the caret's bar.
pub(crate) fn bar_view(score: &Score, c: Caret) -> Option<BarView> {
    let mb = score.master_bars.get(c.bar)?;
    let ts = mb.time_signature;
    let bar = score
        .tracks
        .get(c.track)?
        .staves
        .get(c.staff)?
        .bars
        .get(c.bar)?;
    // A beat is one of the time signature's unit.
    let unit = Ratio::new(1, i64::from(ts.denominator.max(1)))?;
    let want = Ratio::from_integer(i64::from(ts.numerator));
    let beats = |r: Ratio| r.checked_div(unit);
    let heading = if mb.free_time {
        format!("Bar {} · free time", c.bar + 1)
    } else {
        format!("Bar {} · {}/{}", c.bar + 1, ts.numerator, ts.denominator)
    };
    let mut voices = Vec::new();
    for (i, v) in bar.voices.iter().enumerate().take(4) {
        let empty = v.is_empty();
        if empty && i != c.voice {
            continue;
        }
        let entered = v.beats.iter().filter(|b| b.kind != BeatKind::Empty).count();
        let have = (!empty).then(|| v.length().and_then(beats)).flatten();
        let (count, state, fill) = match have {
            None if empty => (
                format!("0/{}", ts.denominator),
                "Empty".to_owned(),
                Fill::Unknown,
            ),
            None => (
                "?".to_owned(),
                "A beat's length can't be counted".to_owned(),
                Fill::Unknown,
            ),
            Some(n) if mb.free_time => (
                format!("{} {}", amount(n), plural(n, "beat")),
                "Free time".to_owned(),
                Fill::Unknown,
            ),
            Some(n) => {
                let count = format!("{}/{}", decimal(n), ts.denominator);
                match n.checked_sub(want) {
                    Some(d) if d.is_zero() => (count, "Full".to_owned(), Fill::Full),
                    Some(d) if d.numer() > 0 => (
                        count,
                        format!("{} {} too long", amount(d), plural(d, "beat")),
                        Fill::Long,
                    ),
                    _ => {
                        let d = want.checked_sub(n).unwrap_or(Ratio::ZERO);
                        (
                            count,
                            format!("{} {} short", amount(d), plural(d, "beat")),
                            Fill::Short,
                        )
                    }
                }
            }
        };
        voices.push(VoiceRow {
            name: format!("Voice {}", i + 1),
            count,
            state,
            fill,
            entered,
            current: i == c.voice,
        });
    }
    // An empty caret voice the bar doesn't have yet still gets its row.
    if c.voice >= bar.voices.len() {
        voices.push(VoiceRow {
            name: format!("Voice {}", c.voice + 1),
            count: format!("0/{}", ts.denominator),
            state: "Empty".to_owned(),
            fill: Fill::Unknown,
            entered: 0,
            current: true,
        });
        voices.sort_by(|a, b| a.name.cmp(&b.name));
    }
    let caret = bar
        .voices
        .get(c.voice)
        .and_then(|v| {
            let starts = onsets(v);
            let at = match starts.get(c.beat) {
                Some(at) => *at,
                // The place after the voice's last beat.
                None => v.length()?,
            };
            let n = beats(at)?;
            let one = Ratio::ONE;
            Some(format!(
                "The caret's beat starts on beat {}",
                amount(n.checked_add(one)?)
            ))
        })
        .unwrap_or_else(|| "The caret's beat starts on beat 1".to_owned());
    Some(BarView {
        heading,
        unit: format!("Beats are {}", unit_name(ts.denominator)),
        voices,
        caret,
    })
}

/// A number of beats as a decimal: 4, 1.5, 3.33.
fn decimal(n: Ratio) -> String {
    if n.denom() == 1 {
        return n.numer().to_string();
    }
    let x = (n.numer() as f64 / n.denom() as f64 * 100.0).round() / 100.0;
    let text = format!("{x:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// "beat" or "beats", as an amount wants.
fn plural(n: Ratio, word: &str) -> String {
    if n == Ratio::ONE || (n.numer() < n.denom() && n.numer() > 0) {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// What a time signature's bottom number counts.
fn unit_name(denominator: u8) -> &'static str {
    match denominator {
        1 => "whole notes",
        2 => "half notes",
        4 => "quarter notes",
        8 => "eighth notes",
        16 => "sixteenth notes",
        32 => "thirty-second notes",
        64 => "sixty-fourth notes",
        _ => "the time signature's unit",
    }
}

/// A number of beats as a musician writes it: 4, 3½, 2¾, ⅓, or 7/5 where
/// no fraction sign fits.
fn amount(n: Ratio) -> String {
    let (num, den) = (n.numer(), n.denom());
    if den == 1 {
        return num.to_string();
    }
    let whole = num.div_euclid(den);
    let rest = num.rem_euclid(den);
    let sign = match (rest, den) {
        (1, 2) => "½",
        (1, 3) => "⅓",
        (2, 3) => "⅔",
        (1, 4) => "¼",
        (3, 4) => "¾",
        (1, 5) => "⅕",
        (2, 5) => "⅖",
        (3, 5) => "⅗",
        (4, 5) => "⅘",
        (1, 6) => "⅙",
        (5, 6) => "⅚",
        (1, 8) => "⅛",
        (3, 8) => "⅜",
        (5, 8) => "⅝",
        (7, 8) => "⅞",
        _ => "",
    };
    match (whole, sign) {
        (0, "") => format!("{rest}/{den}"),
        (_, "") => format!("{whole} {rest}/{den}"),
        (0, s) => s.to_owned(),
        (w, s) => format!("{w}{s}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_edit::{Command, History, apply, new_score};
    use oxiguitar_model::{NoteValue, TimeSignature, Tuplet};

    fn at(bar: usize, beat: usize, voice: usize) -> Caret {
        Caret {
            bar,
            beat,
            voice,
            ..Caret::at_start(&new_score(), 0).unwrap()
        }
    }

    #[test]
    fn amounts() {
        let r = |a, b| Ratio::new(a, b).unwrap();
        assert_eq!(amount(r(4, 1)), "4");
        assert_eq!(amount(r(7, 2)), "3½");
        assert_eq!(amount(r(1, 3)), "⅓");
        assert_eq!(amount(r(11, 4)), "2¾");
        assert_eq!(amount(r(7, 16)), "7/16");
        assert_eq!(amount(r(23, 16)), "1 7/16");
    }

    #[test]
    fn each_voice_against_the_time_signature() {
        let mut s = new_score();
        let mut h = History::new();
        // Voice 1: three quarters of 4/4.
        for i in 0..3 {
            apply(
                &mut s,
                &mut h,
                at(0, i, 0),
                None,
                Command::Fret {
                    fret: 3,
                    again: false,
                },
            )
            .unwrap();
        }
        // Voice 2: a half, a dotted half: a quarter too long.
        apply(
            &mut s,
            &mut h,
            at(0, 0, 1),
            None,
            Command::Fret {
                fret: 5,
                again: false,
            },
        )
        .unwrap();
        apply(
            &mut s,
            &mut h,
            at(0, 0, 1),
            None,
            Command::Value(NoteValue::Half),
        )
        .unwrap();
        apply(
            &mut s,
            &mut h,
            at(0, 1, 1),
            None,
            Command::Fret {
                fret: 7,
                again: false,
            },
        )
        .unwrap();
        apply(&mut s, &mut h, at(0, 1, 1), None, Command::Dot).unwrap();
        let v = bar_view(&s, at(0, 3, 0)).unwrap();
        assert_eq!(v.heading, "Bar 1 · 4/4");
        assert_eq!(v.unit, "Beats are quarter notes");
        assert_eq!(v.voices.len(), 2);
        assert_eq!(
            (v.voices[0].count.as_str(), v.voices[0].state.as_str()),
            ("3 of 4", "1 beat short")
        );
        assert_eq!(v.voices[0].fill, Fill::Short);
        assert!(v.voices[0].current);
        assert_eq!(v.voices[0].entered, 3);
        assert_eq!(
            (v.voices[1].count.as_str(), v.voices[1].state.as_str()),
            ("5 of 4", "1 beat too long")
        );
        assert_eq!(v.caret, "The caret's beat starts on beat 4");

        // A triplet eighth: ⅓ of a beat.
        apply(
            &mut s,
            &mut h,
            at(0, 3, 0),
            None,
            Command::Fret {
                fret: 0,
                again: false,
            },
        )
        .unwrap();
        apply(
            &mut s,
            &mut h,
            at(0, 3, 0),
            None,
            Command::Value(NoteValue::Eighth),
        )
        .unwrap();
        apply(
            &mut s,
            &mut h,
            at(0, 3, 0),
            None,
            Command::Tuplet(Tuplet::TRIPLET),
        )
        .unwrap();
        let v = bar_view(&s, at(0, 3, 0)).unwrap();
        assert_eq!(v.voices[0].count, "3⅓ of 4");
        assert_eq!(v.voices[0].state, "⅔ beat short");

        // 6/8 counts eighths; an empty voice under the caret has its row.
        s.master_bars[0].time_signature = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        let v = bar_view(&s, at(0, 0, 2)).unwrap();
        assert_eq!(v.unit, "Beats are eighth notes");
        assert_eq!(v.voices.len(), 3);
        assert_eq!(v.voices[2].state, "Empty");
        assert!(v.voices[2].current);
        assert_eq!(v.voices[0].count, "6⅔ of 6");
        s.master_bars[0].free_time = true;
        assert_eq!(
            bar_view(&s, at(0, 0, 0)).unwrap().voices[0].state,
            "Free time"
        );
    }
}
