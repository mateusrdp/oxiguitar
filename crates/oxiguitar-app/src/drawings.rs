// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What plugins draw on the page (plugin interface 1.8): before each
//! engraving, every plugin that draws is asked about the places that carry
//! its data or marks, and only those; the host reuses its last answer when
//! they are the same. Its drawings become the engraver's
//! [`Drawings`], which knows nothing of plugins.
//!
//! Since interface 1.13 each request says when its places and the bars are
//! played, from the player's own time map, and a drawing may run on for a
//! number of seconds of the music: the time map finds where that ends
//! ([`run`]), so a plugin never needs the timing rules itself.

use oxiguitar_display::{Paint, PathCmd, Point};
use oxiguitar_host::Plugins;
use oxiguitar_host::api::display as wire;
use oxiguitar_host::api::message::{DrawPlace, DrawTimes, DrawnRun, PlayedBar};
use oxiguitar_layout::{Anchor, Drawing, Drawings, DrawnAt, Run, Shape};
use oxiguitar_model::{Ratio, Score};
use oxiguitar_play::{TimeMap, playing_order};

/// Every plugin's drawings for `score`.
pub(crate) fn of(plugins: &Plugins, score: &Score) -> Drawings {
    let mut out = Drawings::new();
    // When the bars are played, worked out once and only if something is
    // drawn.
    let mut map: Option<TimeMap> = None;
    for (tag, plugin) in plugins.drawers().iter().enumerate() {
        let (places, at, onsets) = places(score, plugin);
        if places.is_empty() {
            // Nothing to ask; the host forgets its last answer.
            plugins.draw(plugin, places, DrawTimes::default());
            continue;
        }
        let map =
            map.get_or_insert_with(|| TimeMap::new(score, &playing_order(&score.master_bars)));
        let firsts = first_visits(map, &at, &onsets);
        let times = times(map, &at, &onsets);
        for drawn in plugins.draw(plugin, places, times) {
            let i = drawn.place as usize;
            let Some(&at) = at.get(i) else { continue };
            let first = firsts.get(i).copied().flatten();
            out.entry(at).or_default().push(Drawing {
                // Whose: its place among the drawers, for a click on it.
                tag: u32::try_from(tag).unwrap_or(u32::MAX),
                width: drawn.width,
                height: drawn.height,
                shapes: drawn.items.iter().filter_map(shape).collect(),
                run: drawn
                    .run
                    .as_ref()
                    .zip(first)
                    .map(|(r, (k, t))| run(map, k, t, r)),
            });
        }
    }
    out
}

/// For each place, the played bar it is first played in and when.
fn first_visits(map: &TimeMap, at: &[DrawnAt], onsets: &[Ratio]) -> Vec<Option<(usize, f64)>> {
    at.iter()
        .zip(onsets)
        .map(|(a, onset)| {
            let k = map.visits.iter().position(|v| v.visit.bar == a.bar)?;
            Some((k, map.time(k, *onset)))
        })
        .collect()
}

/// When each place and every bar are played ([`DrawTimes`]).
fn times(map: &TimeMap, at: &[DrawnAt], onsets: &[Ratio]) -> DrawTimes {
    let n = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    // The played bars of each bar.
    let mut of_bar: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (k, v) in map.visits.iter().enumerate() {
        of_bar.entry(v.visit.bar).or_default().push(k);
    }
    DrawTimes {
        starts: at
            .iter()
            .zip(onsets)
            .map(|(a, onset)| {
                of_bar
                    .get(&a.bar)
                    .into_iter()
                    .flatten()
                    .map(|&k| map.time(k, *onset))
                    .collect()
            })
            .collect(),
        bars: map
            .visits
            .iter()
            .map(|v| PlayedBar {
                bar: n(v.visit.bar),
                start: v.start,
                end: v.end,
            })
            .collect(),
    }
}

/// Where a run of `r.seconds` from `start` (in the played bar `k`) ends:
/// in the played bar its time falls in, or, cut short and with no hook, at
/// the end of the bar before a jump in the playing order or of the song.
fn run(map: &TimeMap, mut k: usize, start: f64, r: &DrawnRun) -> Run {
    let end = start + r.seconds;
    let made = |bar: usize, to: f64, hook: bool| Run {
        to_bar: bar,
        to,
        width: r.width,
        paint: paint(r.paint),
        dashed: r.dashed,
        hook,
    };
    loop {
        let Some(v) = map.visits.get(k) else {
            return made(0, 0.0, false);
        };
        let bar = v.visit.bar;
        let whole = map.bar_length(bar).to_f64();
        if end < v.end {
            let to = match map.find(end) {
                Some((at, pos)) if at == k => pos,
                _ => whole,
            };
            return made(bar, to, r.hook);
        }
        match map.visits.get(k + 1) {
            Some(next) if next.visit.bar == bar + 1 && end > v.end => k += 1,
            // It ends with the bar exactly.
            _ if end <= v.end => return made(bar, whole, r.hook),
            _ => return made(bar, whole, false),
        }
    }
}

/// The places of `score` that carry `plugin`'s data or marks, and where
/// each is.
/// The plugin that drew what has the tag `tag` ([`of`]), and the place
/// it drew it at as the score has it now, for a click on it.
pub(crate) fn clicked(
    plugins: &Plugins,
    score: &Score,
    tag: u32,
    at: DrawnAt,
) -> Option<(String, DrawPlace)> {
    let plugin = plugins.drawers().into_iter().nth(tag as usize)?;
    let (places, ats, _) = places(score, &plugin);
    let i = ats.iter().position(|a| *a == at)?;
    let place = places.into_iter().nth(i)?;
    Some((plugin, place))
}

/// The places, where each is, and where each starts in its bar (whole
/// notes, as written).
fn places(score: &Score, plugin: &str) -> (Vec<DrawPlace>, Vec<DrawnAt>, Vec<Ratio>) {
    let n = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
    let mut places = Vec::new();
    let mut at = Vec::new();
    let mut onsets = Vec::new();
    for (ti, track) in score.tracks.iter().enumerate() {
        for (si, staff) in track.staves.iter().enumerate() {
            for (bi, bar) in staff.bars.iter().enumerate() {
                if let Some(data) = bar.plugins.get(plugin) {
                    places.push(DrawPlace::Bar {
                        at: [n(ti), n(si), n(bi)],
                        data: data.clone(),
                    });
                    at.push(DrawnAt {
                        track: ti,
                        staff: si,
                        bar: bi,
                        beat: None,
                    });
                    onsets.push(Ratio::ZERO);
                }
                for (vi, voice) in bar.voices.iter().enumerate() {
                    let mut onset = Ratio::ZERO;
                    for (ei, beat) in voice.beats.iter().enumerate() {
                        let here = onset;
                        if beat.grace.is_none()
                            && let Some(len) = beat.duration.length()
                        {
                            onset = onset.checked_add(len).unwrap_or(onset);
                        }
                        let ours = beat.plugins.contains_key(plugin)
                            || beat.effects.marks.contains_key(plugin)
                            || beat
                                .notes
                                .iter()
                                .any(|note| note.plugins.contains_key(plugin));
                        if ours {
                            places.push(DrawPlace::Beat {
                                at: [n(ti), n(si), n(bi), n(vi), n(ei)],
                                beat: Box::new(oxiguitar_model::wire::beat_to_wire2(beat)),
                            });
                            at.push(DrawnAt {
                                track: ti,
                                staff: si,
                                bar: bi,
                                beat: Some((vi, ei)),
                            });
                            onsets.push(here);
                        }
                    }
                }
            }
        }
    }
    (places, at, onsets)
}

fn point(p: wire::Point) -> Point {
    Point::new(p.x, p.y)
}

fn paint(p: wire::Paint) -> Paint {
    match p {
        wire::Paint::Ink => Paint::Ink,
        wire::Paint::Page => Paint::Page,
        wire::Paint::Accent => Paint::Accent,
        wire::Paint::Muted => Paint::Muted,
        wire::Paint::Warning => Paint::Warning,
    }
}

/// An item of the interface's canvases as the engraver's shape; `None` for
/// a picture (interface 1.14), which a canvas shows but the page doesn't.
fn shape(item: &wire::Item) -> Option<Shape> {
    Some(match item {
        wire::Item::Line {
            from,
            to,
            width,
            paint: p,
        } => Shape::Line {
            from: point(*from),
            to: point(*to),
            width: *width,
            paint: paint(*p),
        },
        wire::Item::Rect {
            at,
            width,
            height,
            paint: p,
        } => Shape::Rect {
            at: point(*at),
            width: *width,
            height: *height,
            paint: paint(*p),
        },
        wire::Item::Path { cmds, fill, stroke } => Shape::Path {
            cmds: cmds
                .iter()
                .map(|c| match *c {
                    wire::PathCmd::MoveTo(p) => PathCmd::MoveTo(point(p)),
                    wire::PathCmd::LineTo(p) => PathCmd::LineTo(point(p)),
                    wire::PathCmd::CubicTo(a, b, c) => {
                        PathCmd::CubicTo(point(a), point(b), point(c))
                    }
                    wire::PathCmd::Close => PathCmd::Close,
                })
                .collect(),
            fill: fill.map(paint),
            stroke: stroke.map(|(w, p)| (w, paint(p))),
        },
        wire::Item::Text {
            text,
            at,
            size,
            font,
            anchor,
            paint: p,
        } => Shape::Text {
            text: text.clone(),
            at: point(*at),
            size: *size,
            music: *font == wire::Font::Music,
            anchor: match anchor {
                wire::Anchor::Start => Anchor::Start,
                wire::Anchor::Middle => Anchor::Middle,
                wire::Anchor::End => Anchor::End,
            },
            paint: paint(*p),
        },
        wire::Item::Image { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_host::api::display::Paint as WirePaint;
    use oxiguitar_model::MasterBar;

    /// `bars` bars of 4/4 at 120 (two seconds each), the first `repeated`
    /// times through a repeat.
    fn song(bars: usize, repeat_first: bool) -> TimeMap {
        let mut s = Score::new();
        s.initial_tempo = 120.0;
        s.master_bars = vec![MasterBar::default(); bars];
        if repeat_first {
            s.master_bars[0].repeat_end = Some(2);
        }
        TimeMap::new(&s, &playing_order(&s.master_bars))
    }

    fn lasting(seconds: f64) -> DrawnRun {
        DrawnRun {
            seconds,
            width: 0.1,
            paint: WirePaint::Warning,
            dashed: false,
            hook: true,
        }
    }

    #[test]
    fn a_run_ends_where_its_seconds_do() {
        let map = song(4, false);
        // From the second beat of bar 0 (0.5 s), 3 s: 3.5 s, three
        // quarters through bar 1.
        let r = run(&map, 0, 0.5, &lasting(3.0));
        assert_eq!(r.to_bar, 1);
        assert!((r.to - 0.75).abs() < 1e-6, "{}", r.to);
        assert!(r.hook);
        assert_eq!(r.paint, Paint::Warning);
        // Ending with a bar exactly: at its end, hooked.
        let r = run(&map, 0, 0.0, &lasting(4.0));
        assert_eq!((r.to_bar, r.hook), (1, true));
        assert!((r.to - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_run_is_cut_short_at_a_jump_or_the_song_s_end() {
        // Bar 0 is played twice: a run from it past its end stops there.
        let map = song(3, true);
        let r = run(&map, 0, 0.0, &lasting(3.0));
        assert_eq!((r.to_bar, r.hook), (0, false));
        assert!((r.to - 1.0).abs() < 1e-9);
        // Past the song's end: its last bar, cut short.
        let map = song(2, false);
        let r = run(&map, 0, 0.0, &lasting(60.0));
        assert_eq!((r.to_bar, r.hook), (1, false));
    }
}
