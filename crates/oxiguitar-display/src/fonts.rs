// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Fonts a display list refers to, and simple text placement.
//!
//! OxiGuitar bundles two fonts, both under the SIL Open Font License 1.1 (see
//! `assets/fonts/`): Bravura, the reference SMuFL music font, and Noto Sans for
//! text and tab numbers. Every backend draws from the same font bytes, so the
//! screen and the PDF show the same glyph outlines.
//!
//! [`FontSet::place_text`] maps characters straight to glyphs and adds up their
//! advances. That is enough for tab numbers, titles and Latin text. Kerning,
//! ligatures, right-to-left and complex scripts need a real shaper; M1 adds one
//! (`harfrust`, a pure-Rust port of HarfBuzz) behind the same function.

use std::sync::Arc;

use skrifa::{
    FontRef, GlyphId, MetadataProvider, instance::LocationRef, instance::Size, raw::TableProvider,
};

use crate::list::{GlyphRun, Paint, PlacedGlyph, Point};

/// Bravura, Steinberg's SMuFL reference music font (OFL-1.1).
pub static BRAVURA_OTF: &[u8] = include_bytes!("../../../assets/fonts/bravura/Bravura.otf");

/// Bravura's SMuFL metadata: engraving defaults, glyph boxes and anchors.
pub static BRAVURA_METADATA: &str = include_str!("../../../assets/fonts/bravura/Bravura.json");

/// Noto Sans Regular (OFL-1.1), for text and tab numbers.
pub static NOTO_SANS_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/noto-sans/NotoSans-Regular.ttf");

/// Index of a font in a [`FontSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontId(pub u8);

/// A font file loaded into a [`FontSet`].
#[derive(Debug, Clone)]
pub struct FontData {
    name: String,
    bytes: Arc<Vec<u8>>,
    units_per_em: u16,
}

impl FontData {
    /// A short name for logs and the PDF.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The font file's bytes, shared between backends.
    pub fn bytes(&self) -> &Arc<Vec<u8>> {
        &self.bytes
    }

    /// Font units per em, from the font's `head` table.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// A parsed view of the font.
    pub fn font_ref(&self) -> FontRef<'_> {
        // Checked in `FontSet::add`; a font that parsed once parses again.
        FontRef::new(&self.bytes).unwrap_or_else(|_| unreachable!("font was validated when added"))
    }
}

/// Why a font could not be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The bytes are not an OpenType or TrueType font skrifa can read.
    Unreadable(String),
    /// More than 256 fonts in one set.
    TooMany,
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::Unreadable(name) => write!(f, "{name} is not a font OxiGuitar can read"),
            FontError::TooMany => write!(f, "too many fonts"),
        }
    }
}

impl std::error::Error for FontError {}

/// The fonts a display list's glyph runs refer to by [`FontId`].
#[derive(Debug, Clone, Default)]
pub struct FontSet {
    fonts: Vec<FontData>,
}

impl FontSet {
    /// The music font in [`FontSet::bundled`].
    pub const MUSIC: FontId = FontId(0);
    /// The text font in [`FontSet::bundled`].
    pub const TEXT: FontId = FontId(1);

    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bravura as [`FontSet::MUSIC`] and Noto Sans as [`FontSet::TEXT`].
    pub fn bundled() -> Self {
        let mut set = Self::new();
        // Both fonts are compiled in and covered by the tests below.
        let _ = set.add("Bravura", BRAVURA_OTF.to_vec());
        let _ = set.add("Noto Sans", NOTO_SANS_REGULAR.to_vec());
        set
    }

    /// Add a font file and return its id.
    pub fn add(&mut self, name: &str, bytes: Vec<u8>) -> Result<FontId, FontError> {
        let id = u8::try_from(self.fonts.len()).map_err(|_| FontError::TooMany)?;
        let units_per_em = {
            let font = FontRef::new(&bytes).map_err(|_| FontError::Unreadable(name.to_owned()))?;
            font.head()
                .map(|h| h.units_per_em())
                .map_err(|_| FontError::Unreadable(name.to_owned()))?
        };
        self.fonts.push(FontData {
            name: name.to_owned(),
            bytes: Arc::new(bytes),
            units_per_em,
        });
        Ok(FontId(id))
    }

    /// The font with this id.
    pub fn get(&self, id: FontId) -> Option<&FontData> {
        self.fonts.get(usize::from(id.0))
    }

    /// Every font, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (FontId, &FontData)> {
        self.fonts
            .iter()
            .enumerate()
            .filter_map(|(i, f)| Some((FontId(u8::try_from(i).ok()?), f)))
    }

    /// The glyph a character maps to, or 0 (`.notdef`) if the font lacks it.
    pub fn glyph_for(&self, font: FontId, ch: char) -> u16 {
        self.get(font)
            .and_then(|f| f.font_ref().charmap().map(ch))
            .and_then(|g| u16::try_from(g.to_u32()).ok())
            .unwrap_or(0)
    }

    /// Advance width of a glyph at `size` points.
    pub fn advance(&self, font: FontId, glyph: u16, size: f32) -> f32 {
        self.get(font)
            .and_then(|f| {
                f.font_ref()
                    .glyph_metrics(Size::new(size), LocationRef::default())
                    .advance_width(GlyphId::new(u32::from(glyph)))
            })
            .unwrap_or(0.0)
    }

    /// Place `text` on one line: one glyph per character, side by side.
    pub fn place_text(&self, font: FontId, size: f32, text: &str) -> Vec<PlacedGlyph> {
        let Some(data) = self.get(font) else {
            return Vec::new();
        };
        let f = data.font_ref();
        let charmap = f.charmap();
        let metrics = f.glyph_metrics(Size::new(size), LocationRef::default());
        let mut x = 0.0;
        text.char_indices()
            .map(|(i, ch)| {
                let gid = charmap.map(ch).unwrap_or(GlyphId::NOTDEF);
                let advance = metrics.advance_width(gid).unwrap_or(0.0);
                let glyph = PlacedGlyph {
                    id: u16::try_from(gid.to_u32()).unwrap_or(0),
                    x,
                    advance,
                    cluster: u32::try_from(i).unwrap_or(u32::MAX),
                };
                x += advance;
                glyph
            })
            .collect()
    }

    /// Width of `text` placed by [`FontSet::place_text`].
    pub fn text_width(&self, font: FontId, size: f32, text: &str) -> f32 {
        self.place_text(font, size, text)
            .last()
            .map_or(0.0, |g| g.x + g.advance)
    }

    /// A run of `text` starting at `origin`.
    pub fn text_run(
        &self,
        font: FontId,
        size: f32,
        origin: Point,
        text: &str,
        paint: Paint,
    ) -> GlyphRun {
        GlyphRun {
            font,
            size,
            origin,
            glyphs: self.place_text(font, size, text),
            text: text.to_owned(),
            paint,
        }
    }

    /// One SMuFL symbol from the music font. `staff_height` is four staff
    /// spaces in points, which is the music font's size by SMuFL convention.
    pub fn symbol(
        &self,
        codepoint: char,
        staff_height: f32,
        origin: Point,
        paint: Paint,
    ) -> GlyphRun {
        self.text_run(
            Self::MUSIC,
            staff_height,
            origin,
            &codepoint.to_string(),
            paint,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_load() {
        let set = FontSet::bundled();
        assert_eq!(set.get(FontSet::MUSIC).map(FontData::name), Some("Bravura"));
        assert_eq!(
            set.get(FontSet::TEXT).map(FontData::name),
            Some("Noto Sans")
        );
        assert_eq!(
            set.get(FontSet::MUSIC).map(FontData::units_per_em),
            Some(1000)
        );
    }

    #[test]
    fn bravura_has_smufl_glyphs() {
        let set = FontSet::bundled();
        // noteheadBlack, gClef8vb, 6stringTabClef
        for cp in ['\u{E0A4}', '\u{E052}', '\u{E06D}'] {
            assert_ne!(
                set.glyph_for(FontSet::MUSIC, cp),
                0,
                "missing U+{:04X}",
                u32::from(cp)
            );
        }
    }

    #[test]
    fn digits_are_placed_side_by_side() {
        let set = FontSet::bundled();
        let g = set.place_text(FontSet::TEXT, 10.0, "12");
        assert_eq!(g.len(), 2);
        assert!(g[1].x > 0.0 && (g[1].x - g[0].advance).abs() < 1e-4);
        assert_eq!(g[1].cluster, 1);
    }

    #[test]
    fn unreadable_font_is_an_error() {
        let mut set = FontSet::new();
        assert!(matches!(
            set.add("junk", vec![0; 16]),
            Err(FontError::Unreadable(_))
        ));
    }
}
