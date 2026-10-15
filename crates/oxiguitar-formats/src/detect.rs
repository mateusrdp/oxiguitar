// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Recognising a file's format from its contents (FMT-6).
//!
//! File extensions lie: four files in our test corpus named `.gp3` are really
//! Guitar Pro 5 files. Detection therefore looks only at the bytes.

use std::fmt;

/// A file format, with the version where the contents say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    /// Guitar Pro 1 or 2 (.gtp).
    GuitarPro1to2 {
        /// Version text from the header, such as "2.21".
        version: String,
    },
    /// Guitar Pro 3 (.gp3).
    GuitarPro3 {
        /// Version text from the header, such as "3.00".
        version: String,
    },
    /// Guitar Pro 4 (.gp4).
    GuitarPro4 {
        /// Version text from the header, such as "4.06".
        version: String,
    },
    /// Guitar Pro 5 (.gp5).
    GuitarPro5 {
        /// Version text from the header, such as "5.10".
        version: String,
    },
    /// Guitar Pro 6 (.gpx): a BCFZ (compressed) or BCFS container.
    GuitarPro6,
    /// Guitar Pro 7 or 8 (.gp): a ZIP holding `Content/score.gpif`.
    GuitarPro7Plus,
    /// MuseScore compressed (.mscz): a ZIP holding a `.mscx` file.
    MuseScoreCompressed,
    /// MuseScore uncompressed (.mscx) XML.
    MuseScoreXml,
    /// Compressed MusicXML (.mxl).
    MusicXmlCompressed,
    /// MusicXML (.musicxml, .xml).
    MusicXml,
    /// ABC notation (.abc).
    Abc,
    /// Standard MIDI file (.mid).
    Midi,
}

impl Format {
    /// A short human-readable name with version, for messages and `oxiguitar-cli info`.
    pub fn describe(&self) -> String {
        match self {
            Format::GuitarPro1to2 { version } => format!("Guitar Pro {version} (.gtp)"),
            Format::GuitarPro3 { version } => format!("Guitar Pro {version} (.gp3)"),
            Format::GuitarPro4 { version } => format!("Guitar Pro {version} (.gp4)"),
            Format::GuitarPro5 { version } => format!("Guitar Pro {version} (.gp5)"),
            Format::GuitarPro6 => "Guitar Pro 6 (.gpx)".into(),
            Format::GuitarPro7Plus => "Guitar Pro 7/8 (.gp)".into(),
            Format::MuseScoreCompressed => "MuseScore (.mscz)".into(),
            Format::MuseScoreXml => "MuseScore XML (.mscx)".into(),
            Format::MusicXmlCompressed => "Compressed MusicXML (.mxl)".into(),
            Format::MusicXml => "MusicXML".into(),
            Format::Abc => "ABC notation".into(),
            Format::Midi => "Standard MIDI file".into(),
        }
    }

    /// The extension a file in this format normally has, without the dot.
    pub fn usual_extension(&self) -> &'static str {
        match self {
            Format::GuitarPro1to2 { .. } => "gtp",
            Format::GuitarPro3 { .. } => "gp3",
            Format::GuitarPro4 { .. } => "gp4",
            Format::GuitarPro5 { .. } => "gp5",
            Format::GuitarPro6 => "gpx",
            Format::GuitarPro7Plus => "gp",
            Format::MuseScoreCompressed => "mscz",
            Format::MuseScoreXml => "mscx",
            Format::MusicXmlCompressed => "mxl",
            Format::MusicXml => "musicxml",
            Format::Abc => "abc",
            Format::Midi => "mid",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

/// Recognises a file's format from its contents. Returns `None` if the
/// contents match nothing we know.
pub fn detect(data: &[u8]) -> Option<Format> {
    if let Some(f) = detect_guitar_pro_binary(data) {
        return Some(f);
    }
    if data.starts_with(b"BCFZ") || data.starts_with(b"BCFS") {
        return Some(Format::GuitarPro6);
    }
    if data.starts_with(b"MThd") {
        return Some(Format::Midi);
    }
    if data.starts_with(b"PK\x03\x04") {
        return detect_zip(data);
    }
    detect_text(data)
}

/// Guitar Pro 1–5 start with a length byte and a French version string,
/// such as "FICHIER GUITAR PRO v5.10", in a 30-byte field.
fn detect_guitar_pro_binary(data: &[u8]) -> Option<Format> {
    let len = usize::from(*data.first()?);
    let text = data.get(1..1 + len.min(30))?;
    let text = std::str::from_utf8(text).ok()?;
    let version = [
        "FICHIER GUITAR PRO ",
        "FICHIER GUITARE PRO ",
        "CLIPBOARD GUITAR PRO ",
    ]
    .iter()
    .find_map(|p| text.strip_prefix(p))?;
    let digits = version.trim_start_matches(['v', 'L']).trim().to_owned();
    let major = digits.chars().next()?;
    Some(match major {
        '1' | '2' => Format::GuitarPro1to2 { version: digits },
        '3' => Format::GuitarPro3 { version: digits },
        '4' => Format::GuitarPro4 { version: digits },
        '5' => Format::GuitarPro5 { version: digits },
        _ => return None,
    })
}

fn detect_zip(data: &[u8]) -> Option<Format> {
    let names = zip_entry_names(data);
    let has = |pred: &dyn Fn(&str) -> bool| names.iter().any(|n| pred(n));
    if has(&|n| n == "Content/score.gpif") {
        return Some(Format::GuitarPro7Plus);
    }
    if has(&|n| n.ends_with(".mscx")) {
        return Some(Format::MuseScoreCompressed);
    }
    if has(&|n| n == "META-INF/container.xml")
        && has(&|n| n.ends_with(".musicxml") || n.ends_with(".xml") && !n.starts_with("META-INF/"))
    {
        return Some(Format::MusicXmlCompressed);
    }
    None
}

/// Lists the entry names of a ZIP archive from its central directory,
/// without decompressing anything.
pub(crate) fn zip_entry_names(data: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    // The end-of-central-directory record is at least 22 bytes and sits in the
    // last 64 KiB + 22 bytes (the comment is at most 65535 bytes).
    let search_from = data.len().saturating_sub(22 + 65_535);
    let Some(eocd) = (search_from..data.len().saturating_sub(21))
        .rev()
        .find(|&i| data.get(i..i + 4) == Some(b"PK\x05\x06"))
    else {
        return names;
    };
    let u16_at = |i: usize| {
        let b: [u8; 2] = data.get(i..i + 2)?.try_into().ok()?;
        Some(u16::from_le_bytes(b))
    };
    let u32_at = |i: usize| {
        let b: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
        Some(u32::from_le_bytes(b))
    };
    let (Some(count), Some(offset)) = (u16_at(eocd + 10), u32_at(eocd + 16)) else {
        return names;
    };
    let mut p = offset as usize;
    for _ in 0..count {
        if data.get(p..p + 4) != Some(b"PK\x01\x02") {
            break;
        }
        let (Some(name_len), Some(extra_len), Some(comment_len)) =
            (u16_at(p + 28), u16_at(p + 30), u16_at(p + 32))
        else {
            break;
        };
        let start = p + 46;
        let Some(name) = data.get(start..start + usize::from(name_len)) else {
            break;
        };
        names.push(String::from_utf8_lossy(name).into_owned());
        p = start + usize::from(name_len) + usize::from(extra_len) + usize::from(comment_len);
    }
    names
}

fn detect_text(data: &[u8]) -> Option<Format> {
    // Look at the first few KiB only, skipping a UTF-8 byte-order mark.
    let head = data.get(..data.len().min(4096))?;
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let text = String::from_utf8_lossy(head);
    let trimmed = text.trim_start();
    if trimmed.starts_with('<') {
        if text.contains("<score-partwise") || text.contains("<score-timewise") {
            return Some(Format::MusicXml);
        }
        if text.contains("<museScore") {
            return Some(Format::MuseScoreXml);
        }
        return None;
    }
    // ABC: a "%abc" header, or an "X:" reference field followed by a "T:" or
    // "K:" field, possibly after comments.
    if trimmed.starts_with("%abc") {
        return Some(Format::Abc);
    }
    let mut lines = trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('%'));
    if lines.next().is_some_and(|l| l.starts_with("X:"))
        && lines
            .take(10)
            .any(|l| l.starts_with("T:") || l.starts_with("K:"))
    {
        return Some(Format::Abc);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gp_header(text: &str) -> Vec<u8> {
        let mut v = vec![text.len() as u8];
        v.extend_from_slice(text.as_bytes());
        v.resize(31, 0);
        v
    }

    #[test]
    fn guitar_pro_versions() {
        assert_eq!(
            detect(&gp_header("FICHIER GUITAR PRO v5.10")),
            Some(Format::GuitarPro5 {
                version: "5.10".into()
            })
        );
        assert_eq!(
            detect(&gp_header("FICHIER GUITAR PRO v3.00")),
            Some(Format::GuitarPro3 {
                version: "3.00".into()
            })
        );
        assert_eq!(
            detect(&gp_header("FICHIER GUITAR PRO L4.06")),
            Some(Format::GuitarPro4 {
                version: "4.06".into()
            })
        );
        assert_eq!(
            detect(&gp_header("FICHIER GUITARE PRO v1.04")),
            Some(Format::GuitarPro1to2 {
                version: "1.04".into()
            })
        );
    }

    #[test]
    fn containers_and_text() {
        assert_eq!(detect(b"BCFZ\x00\x01"), Some(Format::GuitarPro6));
        assert_eq!(detect(b"MThd\x00\x00\x00\x06"), Some(Format::Midi));
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?>\n<!DOCTYPE score-partwise>\n<score-partwise>"),
            Some(Format::MusicXml)
        );
        assert_eq!(
            detect(b"<?xml version=\"1.0\"?><museScore version=\"4.70\">"),
            Some(Format::MuseScoreXml)
        );
        assert_eq!(detect(b"X:1\nT:Tune\nK:G\nGABc|"), Some(Format::Abc));
        assert_eq!(detect(b"%abc-2.1\nX:1"), Some(Format::Abc));
        assert_eq!(detect(b"hello"), None);
        assert_eq!(detect(b""), None);
    }

    /// A minimal stored ZIP with the given entry names and no data.
    fn zip_with(names: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for name in names {
            let offset = out.len() as u32;
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&[0; 22]);
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(name.as_bytes());
            central.extend_from_slice(b"PK\x01\x02");
            central.extend_from_slice(&[0; 24]);
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        let cd_len = central.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        out.extend_from_slice(&(names.len() as u16).to_le_bytes());
        out.extend_from_slice(&(names.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_len.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn zip_contents_decide() {
        assert_eq!(
            detect(&zip_with(&["VERSION", "Content/score.gpif"])),
            Some(Format::GuitarPro7Plus)
        );
        assert_eq!(
            detect(&zip_with(&["META-INF/container.xml", "song.mscx"])),
            Some(Format::MuseScoreCompressed)
        );
        assert_eq!(
            detect(&zip_with(&["META-INF/container.xml", "score.musicxml"])),
            Some(Format::MusicXmlCompressed)
        );
        assert_eq!(detect(&zip_with(&["readme.txt"])), None);
        // Truncated archive: no panic, no format.
        let z = zip_with(&["Content/score.gpif"]);
        assert_eq!(detect(z.get(..z.len() - 10).unwrap()), None);
    }
}
