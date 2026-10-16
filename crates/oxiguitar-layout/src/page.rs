// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The score header (VIEW-6) and the Page layout: systems stacked down the
//! page, a new page when the next system does not fit (VIEW-3).

use oxiguitar_display::{DisplayList, Paint};
use oxiguitar_model::{Pitch, Score, Staff};

use crate::Options;
use crate::draw::Painter;
use crate::engrave::DrawnSystem;
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
        let mut line = tuning_text(staff);
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
    let mut left_y = side_top;
    for t in &tunings {
        left_y += size + 2.0;
        p.text(&mut list, t, size, 0.0, left_y, Paint::Ink);
    }
    let height = centre_bottom.max(right_y).max(left_y) + 3.0 * p.sp;
    list.height = height;
    Header { list, height }
}

/// "Standard tuning", or the open strings from the lowest, such as
/// "Tuning: D A D G B E".
fn tuning_text(staff: &Staff) -> String {
    if staff.tuning.is_empty() {
        return String::new();
    }
    if staff.tuning == Staff::standard_guitar_tuning() {
        return "Standard tuning".to_owned();
    }
    let names: Vec<String> = staff.tuning.iter().rev().map(|p| letter(*p)).collect();
    format!("Tuning: {}", names.join(" "))
}

/// A pitch's letter name without octave, sharps for black keys.
fn letter(p: Pitch) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES
        .get(usize::from(p.0 % 12))
        .copied()
        .unwrap_or("C")
        .to_owned()
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
) -> Vec<DisplayList> {
    let bottom = o.page_height - o.margin - FOOTER;
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
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunings() {
        let mut staff = Staff {
            tuning: Staff::standard_guitar_tuning(),
            ..Staff::default()
        };
        assert_eq!(tuning_text(&staff), "Standard tuning");
        if let Some(low) = staff.tuning.last_mut() {
            *low = Pitch(38);
        }
        assert_eq!(tuning_text(&staff), "Tuning: D A D G B E");
    }
}
