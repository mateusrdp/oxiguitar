// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Which staves are shown, the plan of every bar, and how bars are broken
//! into systems (NOT-4).

use std::collections::HashMap;

use oxiguitar_model::{Bar, BeatKind, Clef, Dynamic, Score, Staff};

use crate::draw::{Painter, glyphs};
use crate::grid::BarGrid;
use crate::hands;
use crate::plan::{BarPlan, Planner, lyric_size};

/// Tablature lines are this many staff spaces apart.
pub(crate) const TAB_GAP: f32 = 1.5;

/// One staff of one track, as shown: standard notation, tablature or both
/// (VIEW-1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct View {
    pub(crate) track: usize,
    pub(crate) staff: usize,
    pub(crate) show_std: bool,
    pub(crate) show_tab: bool,
    pub(crate) strings: usize,
    /// Which part of the staff this view shows.
    pub(crate) part: Part,
}

/// What part of a staff a view shows. A keyboard staff is shown as a grand
/// staff of three views: the right hand, the left hand and, if asked for,
/// the tablature of both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// The whole staff, as the file has it.
    Whole,
    /// The right hand of a grand staff, in the treble clef.
    Right,
    /// The left hand of a grand staff, in the bass clef.
    Left,
    /// The tablature under a grand staff, every note.
    GrandTab,
}

impl View {
    /// The first of the views that make one track's staff: lyrics and the
    /// marks a track has once go here.
    pub(crate) fn carries_shared_marks(&self) -> bool {
        matches!(self.part, Part::Whole | Part::Right)
    }

    /// The top staff of a grand staff: a brace joins it to the next view.
    pub(crate) fn opens_grand_staff(&self) -> bool {
        self.part == Part::Right
    }
}

/// The staves to show for the chosen tracks. Unknown track numbers are
/// skipped; if none is left, the first track is shown. Tablature follows
/// the user's choice for the track, if any ([`crate::Notation`]), or else
/// the file and [`crate::default_tablature`].
/// A keyboard track with one staff in standard notation becomes a grand
/// staff (Mateus, 16 Oct 2026).
pub(crate) fn views(
    score: &Score,
    tracks: &[usize],
    notation: &std::collections::BTreeMap<usize, crate::Notation>,
) -> Vec<View> {
    let mut chosen: Vec<usize> = tracks
        .iter()
        .copied()
        .filter(|&t| t < score.tracks.len())
        .collect();
    if chosen.is_empty() && !score.tracks.is_empty() {
        chosen.push(0);
    }
    let mut out = Vec::new();
    for t in chosen {
        let Some(track) = score.tracks.get(t) else {
            continue;
        };
        let percussion = track.instrument.percussion;
        let fretted = track.instrument.is_fretted();
        let choice = notation.get(&t).copied();
        let grand = track.instrument.is_keyboard() && track.staves.len() == 1;
        for (s, staff) in track.staves.iter().enumerate() {
            let strings = staff.string_count();
            let can_tab = strings > 0 && !percussion;
            let (show_std, show_tab) = match choice {
                Some(n) => {
                    let tab = n.tablature && can_tab;
                    (n.standard || !tab, tab)
                }
                None => {
                    let tab = can_tab && fretted && staff.show_tablature;
                    (staff.show_standard || !tab, tab)
                }
            };
            let view = |show_std, show_tab, part| View {
                track: t,
                staff: s,
                show_std,
                show_tab,
                strings,
                part,
            };
            if grand && show_std {
                out.push(view(true, false, Part::Right));
                out.push(view(true, false, Part::Left));
                if show_tab {
                    out.push(view(false, true, Part::GrandTab));
                }
            } else {
                out.push(view(show_std, show_tab, Part::Whole));
            }
        }
    }
    out
}

/// Every bar of every shown staff, planned, and the shared grid of each bar.
pub(crate) struct Plans {
    /// By view, then bar.
    pub(crate) bars: Vec<Vec<BarPlan>>,
    /// By bar.
    pub(crate) grids: Vec<BarGrid>,
}

pub(crate) fn plan_all(score: &Score, views: &[View], p: &Painter<'_>, dynamics: bool) -> Plans {
    let empty = Bar::default();
    // Grand staves, split once per track and staff.
    let mut hands: HashMap<(usize, usize), (Staff, Staff, Staff)> = HashMap::new();
    for v in views.iter().filter(|v| v.part != Part::Whole) {
        let Some(staff) = score
            .tracks
            .get(v.track)
            .and_then(|t| t.staves.get(v.staff))
        else {
            continue;
        };
        hands.entry((v.track, v.staff)).or_insert_with(|| {
            let (mut right, left) = hands::split(staff);
            let mut left = hands::without_shared_marks(&left);
            for bar in &mut right.bars {
                bar.clef = Clef::Treble;
            }
            for bar in &mut left.bars {
                bar.clef = Clef::Bass;
            }
            (right, left, hands::without_shared_marks(staff))
        });
    }
    let bars: Vec<Vec<BarPlan>> = views
        .iter()
        .filter_map(|v| {
            let track = score.tracks.get(v.track)?;
            let file_staff = track.staves.get(v.staff)?;
            let split = hands.get(&(v.track, v.staff));
            let staff = match (v.part, split) {
                (Part::Right, Some((r, _, _))) => r,
                (Part::Left, Some((_, l, _))) => l,
                (Part::GrandTab, Some((_, _, t))) => t,
                _ => file_staff,
            };
            let planner = Planner {
                p,
                track,
                staff,
                tab_gap: TAB_GAP * p.sp,
                show_std: v.show_std,
                show_tab: v.show_tab,
            };
            let mut planned: Vec<BarPlan> = score
                .master_bars
                .iter()
                .enumerate()
                .map(|(i, mb)| planner.bar(staff.bars.get(i).unwrap_or(&empty), mb))
                .collect();
            if dynamics {
                mark_dynamics(&mut planned);
            }
            if v.staff == 0 && v.carries_shared_marks() {
                for (line, lyric) in track.lyrics.iter().enumerate() {
                    place_lyrics(&mut planned, line, lyric.start_bar, &lyric.text);
                }
                measure_lyrics(&mut planned, p);
            }
            Some(planned)
        })
        .collect();
    let grids = (0..score.master_bars.len())
        .map(|i| {
            let of_bar: Vec<&BarPlan> = bars.iter().filter_map(|v| v.get(i)).collect();
            BarGrid::merge(&of_bar, p.sp)
        })
        .collect();
    Plans { bars, grids }
}

/// Marks where the first voice's dynamic marking changes, across the whole
/// staff. Only explicit markings count (Guitar Pro 6-8 beats); a note's
/// velocity is not a marking. Nothing is shown while the music stays at
/// Guitar Pro's default, mezzo-forte.
fn mark_dynamics(bars: &mut [BarPlan]) {
    let mut current = Dynamic::MF;
    for bar in bars {
        let Some(beats) = bar.voices.first_mut() else {
            continue;
        };
        for b in beats.iter_mut().filter(|b| b.grace.is_none()) {
            if let Some(d) = b.effects.dynamic
                && d != current
            {
                b.show_dynamic = Some(d);
                current = d;
            }
        }
    }
}

/// Splits Guitar Pro lyrics into syllables: words are separated by spaces
/// or line breaks, syllables by hyphens (kept on the syllable they follow,
/// so the reader sees the word continue).
pub(crate) fn syllables(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let parts: Vec<&str> = word.split('-').filter(|s| !s.is_empty()).collect();
        let last = parts.len().saturating_sub(1);
        for (i, part) in parts.into_iter().enumerate() {
            out.push(if i < last {
                format!("{part}-")
            } else {
                part.to_owned()
            });
        }
    }
    out
}

/// Gives one line of lyrics to the first voice's beats with notes, from
/// `start_bar` on, one syllable per beat. Beats whose notes are all tied
/// continue the previous syllable and get none.
fn place_lyrics(bars: &mut [BarPlan], line: usize, start_bar: usize, text: &str) {
    let mut words = syllables(text).into_iter();
    for bar in bars.iter_mut().skip(start_bar) {
        let Some(beats) = bar.voices.first_mut() else {
            continue;
        };
        for b in beats.iter_mut() {
            let sung = b.grace.is_none()
                && b.kind == BeatKind::Notes
                && !b.notes.is_empty()
                && !b.notes.iter().all(|n| n.tie);
            if sung {
                let Some(w) = words.next() else {
                    return;
                };
                b.lyrics.push((line, w));
            }
        }
    }
}

/// Records how wide each column's lyrics are, so spacing makes room.
fn measure_lyrics(bars: &mut [BarPlan], p: &Painter<'_>) {
    let size = lyric_size(p.sp);
    for bar in bars {
        let BarPlan { voices, cols, .. } = bar;
        for c in cols.iter_mut() {
            c.lyric_half = c
                .column
                .beats
                .iter()
                .filter_map(|r| voices.get(r.voice)?.get(r.beat))
                .flat_map(|b| b.lyrics.iter())
                .map(|(_, w)| p.text_width(w, size) / 2.0)
                .fold(0.0, f32::max);
        }
    }
}

/// One line of music.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SystemSpec {
    /// Index of the system, from 0.
    pub(crate) index: usize,
    /// First bar and one past the last.
    pub(crate) first: usize,
    pub(crate) end: usize,
    /// Room on the left for track names (0 when none are shown).
    pub(crate) indent: f32,
    /// Where the bars begin: the indent, then the clef and key signature.
    pub(crate) prefix: f32,
    /// Extra room given to each bar to fill the line.
    pub(crate) extra: Vec<f32>,
}

/// The track names shown at the start of a system, one per shown track in
/// order, when more than one track is shown: full names on the first
/// system, short names after (none if a track has no short name).
pub(crate) fn track_names(
    score: &Score,
    views: &[View],
    first_system: bool,
) -> Vec<(usize, String)> {
    let mut tracks: Vec<usize> = views.iter().map(|v| v.track).collect();
    tracks.dedup();
    if tracks.len() < 2 {
        return Vec::new();
    }
    tracks
        .into_iter()
        .filter_map(|t| {
            let track = score.tracks.get(t)?;
            let name = if first_system {
                &track.name
            } else {
                &track.short_name
            };
            Some((t, name.trim().to_owned()))
        })
        .collect()
}

/// The size of track names, in points.
pub(crate) fn track_name_size(p: &Painter<'_>) -> f32 {
    1.9 * p.sp
}

/// Room for track names left of the staves: the widest name, at most a
/// fifth of the line, plus a gap. Names longer than that are shortened
/// when drawn.
pub(crate) fn names_indent(names: &[(usize, String)], p: &Painter<'_>, line: f32) -> f32 {
    let size = track_name_size(p);
    let widest = names
        .iter()
        .map(|(_, n)| p.text_width(n, size))
        .fold(0.0, f32::max);
    if widest <= 0.0 {
        0.0
    } else {
        widest.min(line / 5.0) + 1.0 * p.sp
    }
}

/// Room left of the staves for grand-staff braces (0 when there is none).
pub(crate) fn brace_room(views: &[View], p: &Painter<'_>) -> f32 {
    if views.iter().any(View::opens_grand_staff) {
        p.width(glyphs::BRACE) * 3.0 + 0.8 * p.sp
    } else {
        0.0
    }
}

/// Width of the clef and key signature that open a system.
pub(crate) fn system_prefix(p: &Painter<'_>, fifths: i8) -> f32 {
    let sp = p.sp;
    let key = if fifths == 0 {
        0.0
    } else {
        p.key_width(fifths) + 0.5 * sp
    };
    0.6 * sp + p.width(glyphs::G_CLEF_8VB).max(p.width(glyphs::TAB_CLEF_6)) + 0.8 * sp + key
}

/// Width of what opens bar `i` before its notes: a key change (not at the
/// start of a system, where the system prefix shows the key), a time
/// signature, a repeat sign.
pub(crate) fn bar_prefix(score: &Score, p: &Painter<'_>, i: usize, first_in_system: bool) -> f32 {
    let sp = p.sp;
    let Some(mb) = score.master_bars.get(i) else {
        return 0.0;
    };
    let previous = i.checked_sub(1).and_then(|j| score.master_bars.get(j));
    let mut w = 0.0;
    if !first_in_system {
        w += 0.6 * sp;
        if let Some(prev) = previous
            && prev.key_signature.fifths != mb.key_signature.fifths
        {
            w += p.key_change_width(prev.key_signature.fifths, mb.key_signature.fifths) + sp;
        }
    }
    if previous.is_none_or(|prev| prev.time_signature != mb.time_signature) {
        let ts = mb.time_signature;
        w += p.time_width(ts.numerator, ts.denominator) + sp;
    }
    if mb.repeat_start {
        let ed = p.ed();
        w += (ed.thick_barline_thickness + 0.35) * sp
            + p.thin_thick_step()
            + p.width(glyphs::REPEAT_DOT)
            + 0.3 * sp;
    }
    w
}

/// Extra width after bar `i`'s notes for an end-repeat sign.
pub(crate) fn bar_suffix(score: &Score, p: &Painter<'_>, i: usize) -> f32 {
    let repeat_end = score
        .master_bars
        .get(i)
        .is_some_and(|mb| mb.repeat_end.is_some());
    if repeat_end { 1.2 * p.sp } else { 0.0 }
}

/// Breaks the bars into systems of `width` points: as many bars as fit at
/// natural spacing, then stretched to the full width. The last system is
/// stretched only if it is already more than two thirds full.
pub(crate) fn break_systems(
    score: &Score,
    views: &[View],
    plans: &Plans,
    p: &Painter<'_>,
    width: f32,
) -> Vec<SystemSpec> {
    let n = score.master_bars.len();
    let mut systems = Vec::new();
    let mut first = 0;
    while first < n {
        let fifths = score
            .master_bars
            .get(first)
            .map_or(0, |mb| mb.key_signature.fifths);
        let indent = names_indent(&track_names(score, views, systems.is_empty()), p, width)
            + brace_room(views, p);
        let prefix = indent + system_prefix(p, fifths);
        let mut used = prefix;
        let mut end = first;
        while end < n {
            let natural = bar_prefix(score, p, end, end == first)
                + plans.grids.get(end).map_or(4.0 * p.sp, BarGrid::natural)
                + bar_suffix(score, p, end);
            if end > first && used + natural > width {
                break;
            }
            used += natural;
            end += 1;
        }
        let last = end >= n;
        let spare = width - used;
        // Overfull (one bar wider than the line): squeeze. Otherwise
        // stretch, except a last system less than two thirds full.
        let stretch = spare < 0.0 || !(last && used < width * 2.0 / 3.0);
        let weights: Vec<f32> = (first..end)
            .map(|i| plans.grids.get(i).map_or(1.0, BarGrid::weight))
            .collect();
        let total: f32 = weights.iter().sum::<f32>().max(f32::EPSILON);
        let extra = weights
            .iter()
            .map(|w| if stretch { spare * w / total } else { 0.0 })
            .collect();
        systems.push(SystemSpec {
            index: systems.len(),
            first,
            end,
            indent,
            prefix,
            extra,
        });
        first = end;
    }
    systems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyrics_split_into_syllables() {
        assert_eq!(
            syllables("Tar-de em  Ita-pu-ã\nnum"),
            vec!["Tar-", "de", "em", "Ita-", "pu-", "ã", "num"]
        );
        assert!(syllables("  ").is_empty());
    }
}
