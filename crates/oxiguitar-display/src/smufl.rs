// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! SMuFL font metadata: the JSON file that ships beside a SMuFL font.
//!
//! Layout needs three things from it: engraving defaults (line thicknesses,
//! in staff spaces), each glyph's bounding box, and anchors such as where a
//! stem joins a notehead. All values are in staff spaces, y pointing **up**,
//! as the SMuFL specification defines them.
//!
//! Glyphs are looked up by their SMuFL name (`noteheadBlack`). The codepoints
//! we draw are the constants in [`glyph`]; M1 generates the full table from
//! the SMuFL glyph list.

use std::collections::HashMap;

use serde::Deserialize;

/// Line thicknesses and spacings recommended by the font, in staff spaces.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)] // Names follow the SMuFL specification.
pub struct EngravingDefaults {
    pub staff_line_thickness: f32,
    pub stem_thickness: f32,
    pub beam_thickness: f32,
    pub leger_line_thickness: f32,
    pub leger_line_extension: f32,
    pub thin_barline_thickness: f32,
    pub thick_barline_thickness: f32,
    pub thin_thick_barline_separation: f32,
    pub slur_endpoint_thickness: f32,
    pub slur_midpoint_thickness: f32,
}

/// A glyph's bounding box: south-west and north-east corners.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct BBox {
    /// Bottom-left corner `[x, y]`.
    #[serde(rename = "bBoxSW")]
    pub sw: [f32; 2],
    /// Top-right corner `[x, y]`.
    #[serde(rename = "bBoxNE")]
    pub ne: [f32; 2],
}

impl BBox {
    /// Width in staff spaces.
    pub fn width(&self) -> f32 {
        self.ne[0] - self.sw[0]
    }
}

/// The parts of a SMuFL metadata file OxiGuitar uses.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Font name, such as "Bravura".
    pub font_name: String,
    /// Font version.
    pub font_version: serde_json::Value,
    /// Recommended thicknesses.
    pub engraving_defaults: EngravingDefaults,
    /// Bounding boxes by glyph name.
    pub glyph_b_boxes: HashMap<String, BBox>,
    /// Anchors by glyph name, then anchor name (`stemUpSE`, …).
    #[serde(default)]
    pub glyphs_with_anchors: HashMap<String, HashMap<String, [f32; 2]>>,
}

impl Metadata {
    /// Parse a metadata file.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Bravura's metadata, bundled with OxiGuitar.
    pub fn bravura() -> Result<Self, serde_json::Error> {
        Self::parse(crate::fonts::BRAVURA_METADATA)
    }

    /// A glyph's bounding box.
    pub fn bbox(&self, name: &str) -> Option<BBox> {
        self.glyph_b_boxes.get(name).copied()
    }

    /// One anchor of a glyph, `[x, y]` in staff spaces, y up.
    pub fn anchor(&self, glyph: &str, anchor: &str) -> Option<[f32; 2]> {
        self.glyphs_with_anchors.get(glyph)?.get(anchor).copied()
    }
}

/// SMuFL glyphs OxiGuitar draws so far: name and codepoint.
#[allow(missing_docs)] // Each constant is named after its SMuFL glyph name.
pub mod glyph {
    /// A SMuFL glyph: its name in the metadata file, and its codepoint.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Glyph {
        /// SMuFL name, the key in the metadata file.
        pub name: &'static str,
        /// Private Use Area codepoint.
        pub cp: char,
    }

    const fn g(name: &'static str, cp: char) -> Glyph {
        Glyph { name, cp }
    }

    pub const G_CLEF_8VB: Glyph = g("gClef8vb", '\u{E052}');
    pub const SIX_STRING_TAB_CLEF: Glyph = g("6stringTabClef", '\u{E06D}');
    pub const TIME_SIG_4: Glyph = g("timeSig4", '\u{E084}');
    pub const NOTEHEAD_BLACK: Glyph = g("noteheadBlack", '\u{E0A4}');
    pub const NOTEHEAD_HALF: Glyph = g("noteheadHalf", '\u{E0A3}');
    pub const NOTEHEAD_X: Glyph = g("noteheadXBlack", '\u{E0A9}');
    pub const REST_QUARTER: Glyph = g("restQuarter", '\u{E4E5}');
    pub const REST_EIGHTH: Glyph = g("rest8th", '\u{E4E6}');
    pub const ACCIDENTAL_SHARP: Glyph = g("accidentalSharp", '\u{E262}');
    pub const ACCIDENTAL_FLAT: Glyph = g("accidentalFlat", '\u{E260}');
    pub const ACCIDENTAL_NATURAL: Glyph = g("accidentalNatural", '\u{E261}');
    pub const DYNAMIC_MF: Glyph = g("dynamicMF", '\u{E52D}');
    pub const FERMATA_ABOVE: Glyph = g("fermataAbove", '\u{E4C0}');
    pub const SEGNO: Glyph = g("segno", '\u{E047}');
    pub const CODA: Glyph = g("coda", '\u{E048}');
    pub const FLAG_8TH_UP: Glyph = g("flag8thUp", '\u{E240}');
    pub const VIBRATO_SEGMENT: Glyph = g("guitarVibratoStroke", '\u{EAB2}');
    pub const ARTICULATION_ACCENT: Glyph = g("articAccentAbove", '\u{E4A0}');
    pub const ARTICULATION_STACCATO: Glyph = g("articStaccatoAbove", '\u{E4A2}');
    pub const ARROWHEAD_UP: Glyph = g("arrowheadBlackUp", '\u{EB78}');
    pub const NOTE_QUARTER_UP: Glyph = g("noteQuarterUp", '\u{E1D5}');

    /// Every constant above, for tests.
    pub const ALL: &[Glyph] = &[
        G_CLEF_8VB,
        SIX_STRING_TAB_CLEF,
        TIME_SIG_4,
        NOTEHEAD_BLACK,
        NOTEHEAD_HALF,
        NOTEHEAD_X,
        REST_QUARTER,
        REST_EIGHTH,
        ACCIDENTAL_SHARP,
        ACCIDENTAL_FLAT,
        ACCIDENTAL_NATURAL,
        DYNAMIC_MF,
        FERMATA_ABOVE,
        SEGNO,
        CODA,
        FLAG_8TH_UP,
        VIBRATO_SEGMENT,
        ARTICULATION_ACCENT,
        ARTICULATION_STACCATO,
        ARROWHEAD_UP,
        NOTE_QUARTER_UP,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontSet;

    #[test]
    fn bravura_metadata_parses() {
        let m = Metadata::bravura().unwrap();
        assert_eq!(m.font_name, "Bravura");
        assert!((m.engraving_defaults.staff_line_thickness - 0.13).abs() < 1e-6);
        let stem = m.anchor("noteheadBlack", "stemUpSE").unwrap();
        assert!(stem[0] > 1.0 && stem[1] > 0.0);
    }

    #[test]
    fn every_named_glyph_is_in_font_and_metadata() {
        let m = Metadata::bravura().unwrap();
        let fonts = FontSet::bundled();
        for g in glyph::ALL {
            assert!(m.bbox(g.name).is_some(), "{} not in metadata", g.name);
            assert_ne!(
                fonts.glyph_for(FontSet::MUSIC, g.cp),
                0,
                "{} not in font",
                g.name
            );
        }
    }
}
