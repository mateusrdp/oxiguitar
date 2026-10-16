// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Compares two scores that should hold the same music, read from different
//! formats, and counts how much agrees in each category.

// Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use oxiguitar_model::{
    Beat, BeatKind, Note, NoteEffects, NotePlace, Pitch, Ratio, Score, Staff, Voice,
};

/// How many items matched out of how many compared, per category, plus the
/// first few differences of each.
#[derive(Default)]
pub(crate) struct Report {
    pub(crate) counts: BTreeMap<&'static str, (usize, usize)>,
    pub(crate) examples: BTreeMap<&'static str, Vec<String>>,
}

impl Report {
    fn check(&mut self, category: &'static str, same: bool, describe: impl FnOnce() -> String) {
        let c = self.counts.entry(category).or_default();
        c.1 += 1;
        if same {
            c.0 += 1;
        } else {
            let ex = self.examples.entry(category).or_default();
            if ex.len() < 12 {
                ex.push(describe());
            }
        }
    }

    /// The share of items that matched in a category (1.0 if none compared).
    pub(crate) fn ratio(&self, category: &str) -> f64 {
        match self.counts.get(category) {
            Some(&(m, t)) if t > 0 => m as f64 / t as f64,
            _ => 1.0,
        }
    }

    pub(crate) fn summary(&self) -> String {
        let mut s = String::new();
        for (k, (m, t)) in &self.counts {
            let _ = writeln!(
                s,
                "  {k:<18} {m:>6} / {t:<6} ({:.1} %)",
                100.0 * *m as f64 / (*t).max(1) as f64
            );
            for e in self.examples.get(k).into_iter().flatten() {
                let _ = writeln!(s, "      e.g. {e}");
            }
        }
        s
    }
}

/// The effects both readers should agree on, as a comparable value.
fn note_fx(e: &NoteEffects) -> String {
    let mut v = Vec::new();
    if e.hammer_pull {
        v.push("hammer".to_owned());
    }
    for s in &e.slides {
        v.push(format!("{s:?}"));
    }
    if let Some(b) = &e.bend {
        let max = b.points.iter().map(|p| p.cents).max().unwrap_or(0);
        v.push(format!("bend{max}"));
    }
    if let Some(h) = &e.harmonic {
        v.push(format!("{h:?}"));
    }
    for (on, name) in [
        (e.vibrato, "vibrato"),
        (e.palm_mute, "pm"),
        (e.let_ring, "ring"),
        (e.dead, "dead"),
        (e.ghost, "ghost"),
        (e.staccato, "staccato"),
    ] {
        if on {
            v.push(name.to_owned());
        }
    }
    if let Some(a) = e.accent {
        v.push(format!("{a:?}"));
    }
    v.join(" ")
}

fn beat_fx(b: &Beat) -> String {
    let e = &b.effects;
    format!(
        "{:?} {:?} {:?} {:?} fade{} whammy{}",
        e.stroke.map(|s| s.direction),
        e.pick_stroke,
        e.technique,
        e.tremolo_picking,
        e.fade_in,
        e.whammy.is_some()
    )
}

/// The sounding pitch of a note, so that two fingerings of the same note
/// compare equal.
fn pitch(staff: &Staff, n: &Note) -> Option<Pitch> {
    match n.place {
        NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret),
        NotePlace::Pitched(p) => Some(p),
    }
}

/// The beats that carry notes, by where they start in the bar. Rests and
/// empty beats are left out: one format may fill a silent voice with rests
/// where the other leaves it empty.
fn onsets(v: &Voice) -> Vec<(Ratio, &Beat)> {
    let mut pos = Ratio::ZERO;
    let mut out = Vec::new();
    for b in &v.beats {
        if b.kind == BeatKind::Notes {
            out.push((pos, b));
        }
        if b.grace.is_none() {
            pos = pos + b.duration.length().unwrap_or(Ratio::ZERO);
        }
    }
    out
}

fn compare_voice(r: &mut Report, at: &str, (sa, va): (&Staff, &Voice), (sb, vb): (&Staff, &Voice)) {
    let a = onsets(va);
    let b = onsets(vb);
    let mut j = 0;
    for (pos, x) in &a {
        // Skip beats of b that a does not have, counting them as differences.
        while let Some((pb, _)) = b.get(j).filter(|(pb, _)| pb < pos) {
            r.check("beats", false, || {
                format!("{at}: only the second has notes at {pb}")
            });
            j += 1;
        }
        let Some((pb, y)) = b.get(j).filter(|(pb, _)| pb == pos) else {
            r.check("beats", false, || {
                format!("{at}: only the first has notes at {pos}")
            });
            continue;
        };
        let _ = pb;
        j += 1;
        let same_length =
            x.duration.length() == y.duration.length() && x.grace.is_some() == y.grace.is_some();
        r.check("beats", same_length, || {
            format!("{at} at {pos}: {:?} vs {:?}", x.duration, y.duration)
        });
        let (fa, fb) = (beat_fx(x), beat_fx(y));
        r.check("beat effects", fa == fb, || {
            format!("{at} at {pos}: {fa} vs {fb}")
        });
        let mut pa: Vec<(Option<Pitch>, bool)> =
            x.notes.iter().map(|n| (pitch(sa, n), n.tie)).collect();
        let mut pb: Vec<(Option<Pitch>, bool)> =
            y.notes.iter().map(|n| (pitch(sb, n), n.tie)).collect();
        pa.sort();
        pb.sort();
        r.check("notes", pa == pb, || {
            format!("{at} at {pos}: {pa:?} vs {pb:?}")
        });
        if pa != pb {
            continue;
        }
        let places = |v: &[Note]| v.iter().map(|n| n.place).collect::<Vec<_>>();
        r.check("fingering", places(&x.notes) == places(&y.notes), || {
            format!(
                "{at} at {pos}: {:?} vs {:?}",
                places(&x.notes),
                places(&y.notes)
            )
        });
        for (p, q) in x.notes.iter().zip(&y.notes) {
            let (ea, eb) = (note_fx(&p.effects), note_fx(&q.effects));
            r.check("note effects", ea == eb, || {
                format!("{at} at {pos}: [{ea}] vs [{eb}]")
            });
        }
    }
    for (pb, _) in b.get(j..).unwrap_or_default() {
        r.check("beats", false, || {
            format!("{at}: only the second has notes at {pb}")
        });
    }
}

/// Compares `b` against `a`. Tracks are paired by position; voices and beats
/// by position within each bar.
pub(crate) fn compare(a: &Score, b: &Score) -> Report {
    let mut r = Report::default();
    r.check("bar count", a.bar_count() == b.bar_count(), || {
        format!("{} vs {}", a.bar_count(), b.bar_count())
    });
    for (i, (x, y)) in a.master_bars.iter().zip(&b.master_bars).enumerate() {
        let bar = i + 1;
        r.check(
            "time signatures",
            x.time_signature == y.time_signature,
            || {
                format!(
                    "bar {bar}: {:?} vs {:?}",
                    x.time_signature, y.time_signature
                )
            },
        );
        r.check(
            "key signatures",
            x.key_signature.fifths == y.key_signature.fifths,
            || format!("bar {bar}: {:?} vs {:?}", x.key_signature, y.key_signature),
        );
        r.check(
            "repeats",
            (x.repeat_start, x.repeat_end) == (y.repeat_start, y.repeat_end),
            || {
                format!(
                    "bar {bar}: {:?} vs {:?}",
                    (x.repeat_start, x.repeat_end),
                    (y.repeat_start, y.repeat_end)
                )
            },
        );
        r.check(
            "alternate endings",
            x.alternate_endings == y.alternate_endings,
            || {
                format!(
                    "bar {bar}: {:b} vs {:b}",
                    x.alternate_endings, y.alternate_endings
                )
            },
        );
        let mx = x.marker.as_ref().map(|m| m.text.trim());
        let my = y.marker.as_ref().map(|m| m.text.trim());
        r.check("markers", mx == my, || {
            format!("bar {bar}: {mx:?} vs {my:?}")
        });
    }
    r.check("track count", a.tracks.len() == b.tracks.len(), || {
        format!("{} vs {}", a.tracks.len(), b.tracks.len())
    });
    for (ti, (ta, tb)) in a.tracks.iter().zip(&b.tracks).enumerate() {
        let t = ti + 1;
        r.check("track names", ta.name.trim() == tb.name.trim(), || {
            format!("track {t}: {:?} vs {:?}", ta.name, tb.name)
        });
        let (Some(sa), Some(sb)) = (ta.staves.first(), tb.staves.first()) else {
            continue;
        };
        r.check(
            "tunings",
            ta.instrument.percussion == tb.instrument.percussion
                && (ta.instrument.percussion || sa.tuning == sb.tuning),
            || format!("track {t}: {:?} vs {:?}", sa.tuning, sb.tuning),
        );
        r.check("capos", sa.capo == sb.capo, || {
            format!("track {t}: {} vs {}", sa.capo, sb.capo)
        });
        for (bi, (ba, bb)) in sa.bars.iter().zip(&sb.bars).enumerate() {
            let place = format!("track {t} bar {}", bi + 1);
            for (vi, (va, vb)) in ba.voices.iter().zip(&bb.voices).enumerate() {
                let at = format!("{place} voice {}", vi + 1);
                compare_voice(&mut r, &at, (sa, va), (sb, vb));
            }
        }
    }
    r
}
