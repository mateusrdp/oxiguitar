// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! PDF backend: krilla.
//!
//! Each display list becomes one page. Fonts are embedded as subsets holding
//! only the glyphs used, and every glyph run carries its text, so PDF readers
//! can search and copy titles, lyrics and tab numbers (EXP-1).

use std::collections::HashMap;

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, LineCap, LineJoin, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use oxiguitar_display::{
    DisplayList, FontId, FontSet, GlyphRun, Item, Paint, Path, PathCmd, Rgba, Theme,
};

/// Why a PDF could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// A page has a zero or negative size.
    BadPageSize,
    /// krilla could not read one of the fonts.
    Font(String),
    /// krilla failed while writing the file.
    Write(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::BadPageSize => write!(f, "a page has no size"),
            PdfError::Font(name) => write!(f, "font {name} could not be embedded"),
            PdfError::Write(e) => write!(f, "could not write the PDF: {e}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Write `pages` as a PDF document with the given title.
pub fn to_pdf(
    pages: &[&DisplayList],
    fonts: &FontSet,
    theme: &Theme,
    title: &str,
) -> Result<Vec<u8>, PdfError> {
    let mut pdf_fonts = HashMap::new();
    for (id, data) in fonts.iter() {
        let font = Font::new(data.bytes().clone().into(), 0)
            .ok_or_else(|| PdfError::Font(data.name().to_owned()))?;
        pdf_fonts.insert(id, font);
    }

    let mut doc = Document::new();
    doc.set_metadata(
        krilla::metadata::Metadata::new()
            .title(title.to_owned())
            .creator(format!("OxiGuitar {}", env!("CARGO_PKG_VERSION"))),
    );
    for list in pages {
        let settings =
            PageSettings::from_wh(list.width, list.height).ok_or(PdfError::BadPageSize)?;
        let mut page = doc.start_page_with(settings);
        let mut surface = page.surface();
        draw(&mut surface, list, &pdf_fonts, theme);
        surface.finish();
        page.finish();
    }
    doc.finish().map_err(|e| PdfError::Write(format!("{e:?}")))
}

fn draw(
    surface: &mut Surface<'_>,
    list: &DisplayList,
    fonts: &HashMap<FontId, Font>,
    theme: &Theme,
) {
    for item in &list.items {
        match item {
            Item::Glyphs(run) => {
                if let Some(font) = fonts.get(&run.font) {
                    draw_run(surface, run, font, theme);
                }
            }
            Item::Line {
                from,
                to,
                width,
                paint,
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(from.x, from.y);
                pb.line_to(to.x, to.y);
                if let Some(path) = pb.finish() {
                    surface.set_fill(None);
                    surface.set_stroke(Some(stroke(theme, *paint, *width, LineCap::Butt)));
                    surface.draw_path(&path);
                }
            }
            Item::Rect { rect, paint } => {
                if let Some(r) =
                    krilla::geom::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
                {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(r);
                    if let Some(path) = pb.finish() {
                        surface.set_stroke(None);
                        surface.set_fill(Some(fill(theme, *paint)));
                        surface.draw_path(&path);
                    }
                }
            }
            Item::Path {
                path,
                fill: f,
                stroke: s,
            } => {
                let Some(p) = to_pdf_path(path) else { continue };
                surface.set_fill(f.map(|paint| fill(theme, paint)));
                surface.set_stroke(s.map(|(w, paint)| stroke(theme, paint, w, LineCap::Round)));
                surface.draw_path(&p);
            }
        }
    }
    surface.set_fill(None);
    surface.set_stroke(None);
}

fn draw_run(surface: &mut Surface<'_>, run: &GlyphRun, font: &Font, theme: &Theme) {
    if run.glyphs.is_empty() || run.size <= 0.0 {
        return;
    }
    let size = run.size;
    // krilla places each glyph at the pen plus its offset, then moves the pen
    // by its advance; derive both from the absolute positions in the list.
    let mut pen = 0.0;
    let glyphs: Vec<KrillaGlyph> = run
        .glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let start = g.cluster as usize;
            let end = run
                .glyphs
                .get(i + 1)
                .map_or(run.text.len(), |n| n.cluster as usize)
                .max(start);
            let glyph = KrillaGlyph::new(
                GlyphId::new(u32::from(g.id)),
                g.advance / size,
                (g.x - pen) / size,
                0.0,
                0.0,
                start..end,
                None,
            );
            pen += g.advance;
            glyph
        })
        .collect();
    surface.set_stroke(None);
    surface.set_fill(Some(fill(theme, run.paint)));
    surface.draw_glyphs(
        Point::from_xy(run.origin.x, run.origin.y),
        &glyphs,
        font.clone(),
        &run.text,
        size,
        false,
    );
}

fn to_pdf_path(path: &Path) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    for cmd in &path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => pb.move_to(p.x, p.y),
            PathCmd::LineTo(p) => pb.line_to(p.x, p.y),
            PathCmd::CubicTo(a, b, c) => pb.cubic_to(a.x, a.y, b.x, b.y, c.x, c.y),
            PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

fn colour(c: Rgba) -> (krilla::paint::Paint, NormalizedF32) {
    let opacity = NormalizedF32::new(f32::from(c.a) / 255.0).unwrap_or(NormalizedF32::ONE);
    (rgb::Color::new(c.r, c.g, c.b).into(), opacity)
}

fn fill(theme: &Theme, paint: Paint) -> Fill {
    let (paint, opacity) = colour(theme.resolve(paint));
    Fill {
        paint,
        opacity,
        rule: FillRule::NonZero,
    }
}

fn stroke(theme: &Theme, paint: Paint, width: f32, cap: LineCap) -> Stroke {
    let (paint, opacity) = colour(theme.resolve(paint));
    Stroke {
        paint,
        width,
        opacity,
        line_cap: cap,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_display::{smufl::Metadata, specimen};

    #[test]
    fn specimen_becomes_a_pdf_with_embedded_fonts() {
        let fonts = FontSet::bundled();
        let meta = Metadata::bravura().unwrap();
        let page = specimen::guitar_excerpt(&fonts, &meta);
        let bytes = to_pdf(&[&page], &fonts, &Theme::LIGHT, "Specimen").unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        // Subsetting keeps the file small: far below the 1.5 MB of font files.
        assert!(bytes.len() < 200_000, "{} bytes", bytes.len());
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("FontFile"), "fonts must be embedded");
    }

    #[test]
    fn empty_page_size_is_an_error() {
        let fonts = FontSet::bundled();
        let page = DisplayList::new(0.0, 0.0);
        assert_eq!(
            to_pdf(&[&page], &fonts, &Theme::LIGHT, "x"),
            Err(PdfError::BadPageSize)
        );
    }
}
