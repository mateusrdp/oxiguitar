// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score header (VIEW-6), and how systems become sheets (VIEW-3): the
//! Page layout stacks them down the page and starts a new page when the next
//! one does not fit; the other layouts put them all on one long sheet.

use oxiguitar_display::{DisplayList, Paint, Path, Point};
use oxiguitar_model::{ChordDiagram, Score, Staff};

use crate::Options;
use crate::draw::Painter;
use crate::engrave::DrawnSystem;
use crate::hits::SystemHits;
use crate::system::View;

/// The header, drawn from the top of the content area.
#[derive(Default)]
pub(crate) struct Header {
    list: DisplayList,
    height: f32,
}

/// Title, subtitle, artist and album centred; words, music and transcriber
/// on the right; tuning and capo on the left.
pub(crate) fn header(
    score: &Score,
    views: &[View],
    p: &Painter<'_>,
    page_width: f32,
    margin: f32,
) -> Header {
    let info = &score.info;
    let width = page_width - 2.0 * margin;
    let mut list = DisplayList::new(width, 0.0);
    let centre = width / 2.0;

    let mut y = 0.0;
    for (text, size, gap) in [
        (&info.title, 22.0, 6.0),
        (&info.subtitle, 14.0, 4.0),
        (&info.artist, 13.0, 4.0),
        (&info.album, 11.0, 4.0),
    ] {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        y += size;
        p.text_centred(&mut list, text, size, centre, y, Paint::Ink);
        y += gap;
    }
    let centre_bottom = y;

    let mut credits = Vec::new();
    let words = info.words.trim();
    let music = info.music.trim();
    if !words.is_empty() && words == music {
        credits.push(format!("Words & Music by {words}"));
    } else {
        if !music.is_empty() {
            credits.push(format!("Music by {music}"));
        }
        if !words.is_empty() {
            credits.push(format!("Words by {words}"));
        }
    }
    if !info.transcriber.trim().is_empty() {
        credits.push(format!("Transcribed by {}", info.transcriber.trim()));
    }
    let mut tunings = Vec::new();
    for v in views.iter().filter(|v| v.show_tab || v.strings > 0) {
        let Some(track) = score.tracks.get(v.track) else {
            continue;
        };
        let Some(staff) = track.staves.get(v.staff) else {
            continue;
        };
        let mut line = tuning_text(staff, track.instrument.is_bass());
        if line.is_empty() {
            continue;
        }
        if staff.capo > 0 {
            line.push_str(&format!(", capo fret {}", staff.capo));
        }
        if views.len() > 1 {
            line = format!("{}: {line}", track.name.trim());
        }
        tunings.push(line);
    }
    tunings.dedup();

    let size = 10.0;
    let side_top = centre_bottom.max(size) + 4.0;
    let mut right_y = side_top;
    for c in &credits {
        right_y += size + 2.0;
        p.text_right(&mut list, c, size, width, right_y, Paint::Ink);
    }
    // Tunings go beside the credits, or under them when the two would meet.
    let widest = |lines: &[String]| {
        lines
            .iter()
            .map(|l| p.text_width(l, size))
            .fold(0.0, f32::max)
    };
    let mut left_y = if widest(&tunings) + widest(&credits) + 2.0 * size > width {
        right_y
    } else {
        side_top
    };
    for t in &tunings {
        left_y += size + 2.0;
        p.text(&mut list, t, size, 0.0, left_y, Paint::Ink);
    }
    let mut height = centre_bottom.max(right_y).max(left_y) + 3.0 * p.sp;
    // The chord diagrams the shown tracks use, in order of first use.
    let diagrams = used_diagrams(score, views);
    if !diagrams.is_empty() {
        height = chord_diagrams(&mut list, p, &diagrams, height, width) + 2.0 * p.sp;
    }
    list.height = height;
    Header { list, height }
}

/// Chord diagrams used by the shown tracks: unique by name and frets, in
/// the order the music first uses them.
fn used_diagrams<'s>(score: &'s Score, views: &[View]) -> Vec<&'s ChordDiagram> {
    let mut out: Vec<&ChordDiagram> = Vec::new();
    for v in views {
        let Some(track) = score.tracks.get(v.track) else {
            continue;
        };
        let Some(staff) = track.staves.get(v.staff) else {
            continue;
        };
        let used = staff
            .bars
            .iter()
            .flat_map(|b| b.voices.iter())
            .flat_map(|v| v.beats.iter())
            .filter_map(|b| b.chord.and_then(|c| track.chords.get(c)));
        for d in used {
            let drawable = d.frets.iter().any(Option::is_some) && !d.name.trim().is_empty();
            if drawable && !out.iter().any(|o| o.name == d.name && o.frets == d.frets) {
                out.push(d);
            }
        }
    }
    out
}

/// Draws chord diagrams in rows from `y`; returns the bottom of the last row.
fn chord_diagrams(
    list: &mut DisplayList,
    p: &Painter<'_>,
    diagrams: &[&ChordDiagram],
    y: f32,
    width: f32,
) -> f32 {
    let sg = 4.2; // between strings, points
    let fg = 5.2; // between frets
    let name_size = 8.0;
    let frets_shown = 4;
    let mut x = 0.0;
    let mut row_top = y;
    let mut row_bottom = y;
    for d in diagrams {
        let strings = d.frets.len().max(1);
        let grid_w = (strings - 1) as f32 * sg;
        let name_w = p.text_width(&d.name, name_size);
        let cell = grid_w.max(name_w) + 16.0;
        if x > 0.0 && x + cell > width {
            x = 0.0;
            row_top = row_bottom + 6.0;
        }
        let gx = x + (cell - grid_w) / 2.0;
        let name_base = row_top + name_size;
        p.text_centred(
            list,
            &d.name,
            name_size,
            gx + grid_w / 2.0,
            name_base,
            Paint::Ink,
        );
        let marks_y = name_base + 5.5; // centre of the x/o marks
        let grid_top = marks_y + 4.0;
        let grid_bottom = grid_top + frets_shown as f32 * fg;
        let thin = 0.5;
        // Strings, frets, and the nut when the diagram starts at the nut.
        for s in 0..strings {
            let sx = gx + s as f32 * sg;
            p.vline(list, sx, grid_top, grid_bottom, thin);
        }
        for f in 0..=frets_shown {
            let fy = grid_top + f as f32 * fg;
            p.hline(list, gx, gx + grid_w, fy, thin);
        }
        let first = d.first_fret.max(1);
        if first <= 1 {
            p.hline(list, gx, gx + grid_w, grid_top - 0.6, 1.6);
        } else {
            p.text(
                list,
                &format!("{first}fr"),
                6.5,
                gx + grid_w + 2.5,
                grid_top + fg * 0.8,
                Paint::Ink,
            );
        }
        // Frets per string: highest string first, drawn low string left.
        for (i, fret) in d.frets.iter().enumerate() {
            let sx = gx + (strings - 1 - i) as f32 * sg;
            match fret {
                None => p.text_centred(list, "x", 6.0, sx, marks_y + 2.0, Paint::Ink),
                Some(0) => circle(list, sx, marks_y, 1.5, false),
                Some(f) => {
                    let row = i32::from(*f) - i32::from(first) + 1;
                    if (1..=frets_shown).contains(&row) {
                        let cy = grid_top + (row as f32 - 0.5) * fg;
                        circle(list, sx, cy, 1.7, true);
                    }
                }
            }
        }
        row_bottom = row_bottom.max(grid_bottom);
        x += cell;
    }
    row_bottom
}

/// A circle of radius `r` centred on (`cx`, `cy`), filled or outlined.
fn circle(list: &mut DisplayList, cx: f32, cy: f32, r: f32, filled: bool) {
    let k = 0.552_284_8 * r;
    let path = Path::new()
        .move_to(cx + r, cy)
        .cubic_to(
            Point::new(cx + r, cy + k),
            Point::new(cx + k, cy + r),
            Point::new(cx, cy + r),
        )
        .cubic_to(
            Point::new(cx - k, cy + r),
            Point::new(cx - r, cy + k),
            Point::new(cx - r, cy),
        )
        .cubic_to(
            Point::new(cx - r, cy - k),
            Point::new(cx - k, cy - r),
            Point::new(cx, cy - r),
        )
        .cubic_to(
            Point::new(cx + k, cy - r),
            Point::new(cx + r, cy - k),
            Point::new(cx + r, cy),
        )
        .close();
    if filled {
        list.fill(path, Paint::Ink);
    } else {
        list.stroke(path, 0.5, Paint::Ink);
    }
}

/// "Standard tuning" for a guitar's or bass's E Standard, else the
/// tuning's compact name ("Tuning: Drop D", "Tuning: E♭ Standard",
/// "Tuning: D A D G A D"; `oxiguitar_model::tuning`).
fn tuning_text(staff: &Staff, bass: bool) -> String {
    if staff.tuning.is_empty() {
        return String::new();
    }
    let name = oxiguitar_model::tuning::name(&staff.tuning, bass);
    if name == "E Standard" {
        return "Standard tuning".to_owned();
    }
    format!("Tuning: {name}")
}

/// Room kept at the foot of each page for the page number, in points.
pub(crate) const FOOTER: f32 = 14.0;

/// Stacks the header and the systems into pages.
pub(crate) fn assemble(
    score: &Score,
    header: Header,
    systems: Vec<DrawnSystem>,
    p: &Painter<'_>,
    o: &Options,
) -> (Vec<DisplayList>, Vec<SystemHits>) {
    let bottom = o.page_height - o.margin - FOOTER;
    let mut hits = Vec::with_capacity(systems.len());
    let gap = 3.5 * p.sp;
    let new_page = || DisplayList::new(o.page_width, o.page_height);
    let mut pages = vec![new_page()];
    let mut y = o.margin;
    if header.height > 0.0 {
        if let Some(page) = pages.last_mut() {
            page.items.extend(
                header
                    .list
                    .items
                    .iter()
                    .map(|i| i.offset(o.margin, o.margin)),
            );
        }
        y += header.height;
    }
    let mut on_page = 0;
    for s in systems {
        if (on_page > 0 || y > o.margin) && y + s.height > bottom {
            pages.push(new_page());
            y = o.margin;
            on_page = 0;
        }
        if let Some(page) = pages.last_mut() {
            page.items
                .extend(s.list.items.iter().map(|i| i.offset(o.margin, y)));
        }
        hits.push(s.hits.placed(pages.len() - 1, o.margin, y));
        y += s.height + gap;
        on_page += 1;
    }

    let count = pages.len();
    let copyright = score.info.copyright.trim();
    for (n, page) in pages.iter_mut().enumerate() {
        let base = o.page_height - o.margin;
        if count > 1 {
            let label = format!("{}/{count}", n + 1);
            p.text_centred(page, &label, 9.0, o.page_width / 2.0, base, Paint::Muted);
        }
        if n == 0 && !copyright.is_empty() {
            p.text_centred(
                page,
                copyright,
                8.0,
                o.page_width / 2.0,
                base - 12.0,
                Paint::Muted,
            );
        }
    }
    (pages, hits)
}

/// Puts the header and every system on one long sheet: stacked down it
/// (Vertical, Parchment) or, for the single line of the Horizontal layout,
/// as wide as the line. No page numbers; the copyright, if any, closes the
/// sheet.
pub(crate) fn sheet(
    score: &Score,
    header: Header,
    systems: Vec<DrawnSystem>,
    p: &Painter<'_>,
    o: &Options,
    width: f32,
) -> (DisplayList, Vec<SystemHits>) {
    let gap = 3.5 * p.sp;
    let mut hits = Vec::with_capacity(systems.len());
    let widest = systems.iter().map(|s| s.list.width).fold(0.0, f32::max);
    let width = if o.layout == crate::Layout::Horizontal {
        widest + 2.0 * o.margin
    } else if widest + 2.0 * o.margin > width + 0.5 {
        // Only a bar wider than the whole line widens the sheet; rounding
        // in the stretched systems does not.
        widest + 2.0 * o.margin
    } else {
        width
    };
    let mut list = DisplayList::new(width, 0.0);
    let mut y = o.margin;
    if header.height > 0.0 {
        list.items.extend(
            header
                .list
                .items
                .iter()
                .map(|i| i.offset(o.margin, o.margin)),
        );
        y += header.height;
    }
    let count = systems.len();
    for (n, s) in systems.into_iter().enumerate() {
        list.items
            .extend(s.list.items.iter().map(|i| i.offset(o.margin, y)));
        hits.push(s.hits.placed(0, o.margin, y));
        y += s.height;
        if n + 1 < count {
            y += gap;
        }
    }
    let copyright = score.info.copyright.trim();
    if o.layout != crate::Layout::Horizontal && !copyright.is_empty() {
        y += 2.0 * gap;
        p.text_centred(&mut list, copyright, 8.0, width / 2.0, y, Paint::Muted);
    }
    list.height = y + o.margin;
    (list, hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::Pitch;

    #[test]
    fn tunings() {
        let mut staff = Staff {
            tuning: Staff::standard_guitar_tuning(),
            ..Staff::default()
        };
        assert_eq!(tuning_text(&staff, false), "Standard tuning");
        if let Some(low) = staff.tuning.last_mut() {
            *low = Pitch(38);
        }
        assert_eq!(tuning_text(&staff, false), "Tuning: Drop D");
    }
}
