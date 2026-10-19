// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Recognising a file's format from its contents, with no plugin loaded
//! (FMT-6).
//!
//! A format in a manifest lists one or more [`Signature`]s; a file is of
//! that format when any one of them matches. A signature matches when every
//! test it gives holds:
//!
//! ```json
//! { "at": 1, "text": "FICHIER GUITAR PRO v5" }
//! { "hex": "42434658" }
//! { "zip_member": "Content/score.gpif" }
//! ```
//!
//! File extensions are never used to recognise a file: they lie (four
//! files in our test corpus named `.gp3` are Guitar Pro 5).

use std::cell::OnceCell;

use serde::{Deserialize, Serialize};

/// One way to recognise a format. Every test given must hold; a signature
/// with no test is invalid ([`Signature::problem`]). A signature with a
/// test this version doesn't know (from a later minor version) never
/// matches, so an older host can't mistake a file for that format.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Where `text` or `hex` must start, in bytes from the start of the file.
    #[serde(default)]
    pub at: u64,
    /// UTF-8 text that must be found at `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Bytes, as hexadecimal digits, that must be found at `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    /// The file must be a ZIP archive holding a member of exactly this name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zip_member: Option<String>,
    /// Tests this version doesn't know.
    #[serde(flatten, skip_serializing)]
    pub unknown: serde_json::Map<String, serde_json::Value>,
}

impl Signature {
    /// What is wrong with this signature, if anything.
    pub fn problem(&self) -> Option<String> {
        if !self.unknown.is_empty() {
            // From a later version: it can't be checked here, only ignored.
            return None;
        }
        if self.text.is_none() && self.hex.is_none() && self.zip_member.is_none() {
            return Some("a signature needs at least one of text, hex and zip_member".into());
        }
        if self.text.is_some() && self.hex.is_some() {
            return Some("a signature gives text or hex, not both".into());
        }
        if let Some(hex) = &self.hex
            && decode_hex(hex).is_none()
        {
            return Some(format!(
                "{hex:?} is not an even number of hexadecimal digits"
            ));
        }
        if self.text.as_deref() == Some("") || self.hex.as_deref() == Some("") {
            return Some("a signature's text or hex can't be empty".into());
        }
        None
    }

    /// Whether `file` matches this signature.
    pub fn matches(&self, file: &Sniff<'_>) -> bool {
        if !self.unknown.is_empty() || self.problem().is_some() {
            return false;
        }
        let expected = match (&self.text, &self.hex) {
            (Some(t), _) => Some(t.as_bytes().to_vec()),
            (None, Some(h)) => decode_hex(h),
            (None, None) => None,
        };
        if let Some(expected) = expected {
            let Ok(at) = usize::try_from(self.at) else {
                return false;
            };
            let found = at
                .checked_add(expected.len())
                .and_then(|end| file.data.get(at..end));
            if found != Some(&expected[..]) {
                return false;
            }
        }
        if let Some(member) = &self.zip_member
            && !file.zip_members().iter().any(|m| m == member)
        {
            return false;
        }
        true
    }
}

/// A file being recognised. The ZIP directory is read at most once, and
/// only when a signature asks for it.
#[derive(Debug)]
pub struct Sniff<'a> {
    data: &'a [u8],
    zip: OnceCell<Vec<String>>,
}

impl<'a> Sniff<'a> {
    /// Starts recognising `data`.
    pub fn new(data: &'a [u8]) -> Sniff<'a> {
        Sniff {
            data,
            zip: OnceCell::new(),
        }
    }

    /// The file's contents.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The member names of the file, if it is a ZIP archive; none otherwise.
    pub fn zip_members(&self) -> &[String] {
        self.zip.get_or_init(|| zip_member_names(self.data))
    }
}

/// Lists a ZIP archive's member names from its central directory, without
/// decompressing anything. Anything that isn't a well-formed ZIP gives the
/// names read before the damage (often none).
pub fn zip_member_names(data: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    if !data.starts_with(b"PK\x03\x04") {
        return names;
    }
    // The end-of-central-directory record is at least 22 bytes and sits in
    // the last 64 KiB + 22 bytes (its comment is at most 65535 bytes).
    let search_from = data.len().saturating_sub(22 + 65_535);
    let Some(eocd) = (search_from..data.len().saturating_sub(21))
        .rev()
        .find(|&i| data.get(i..i + 4) == Some(b"PK\x05\x06"))
    else {
        return names;
    };
    let u16_at = |i: usize| {
        let b: [u8; 2] = data.get(i..i.checked_add(2)?)?.try_into().ok()?;
        Some(u16::from_le_bytes(b))
    };
    let u32_at = |i: usize| {
        let b: [u8; 4] = data.get(i..i.checked_add(4)?)?.try_into().ok()?;
        Some(u32::from_le_bytes(b))
    };
    let (Some(count), Some(offset)) = (u16_at(eocd + 10), u32_at(eocd + 16)) else {
        return names;
    };
    let Ok(mut p) = usize::try_from(offset) else {
        return names;
    };
    for _ in 0..count {
        if data.get(p..p.saturating_add(4)) != Some(b"PK\x01\x02") {
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

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(json: &str) -> Signature {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn text_and_hex_at_an_offset() {
        let gp5 = b"\x18FICHIER GUITAR PRO v5.10";
        let s = sig(r#"{"at": 1, "text": "FICHIER GUITAR PRO v5"}"#);
        assert!(s.matches(&Sniff::new(gp5)));
        assert!(!s.matches(&Sniff::new(b"\x18FICHIER GUITAR PRO v4.06")));
        assert!(!s.matches(&Sniff::new(b"\x18FICHIER")), "too short");
        assert!(sig(r#"{"hex": "18464943"}"#).matches(&Sniff::new(gp5)));
        assert!(!sig(r#"{"hex": "1846494"}"#).matches(&Sniff::new(gp5)));
        let far = sig(r#"{"at": 18446744073709551615, "text": "x"}"#);
        assert!(!far.matches(&Sniff::new(gp5)), "no overflow");
    }

    #[test]
    fn problems() {
        assert!(sig("{}").problem().is_some());
        assert!(sig(r#"{"text": "a", "hex": "61"}"#).problem().is_some());
        assert!(sig(r#"{"hex": "zz"}"#).problem().is_some());
        assert!(sig(r#"{"text": ""}"#).problem().is_some());
        let later = sig(r#"{"at": 0, "text": "B", "within": 100}"#);
        assert!(later.problem().is_none(), "a later version's test");
        assert!(!later.matches(&Sniff::new(b"BCFZ")), "never matches here");
        assert!(sig(r#"{"zip_member": "a"}"#).problem().is_none());
    }

    #[test]
    fn zip_members_without_decompressing() {
        // A minimal ZIP: one stored member, its central directory entry and
        // the end record.
        let name = b"Content/score.gpif";
        let mut z = Vec::new();
        z.extend_from_slice(b"PK\x03\x04");
        z.extend_from_slice(&[0; 22]);
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0, 0]);
        z.extend_from_slice(name);
        let cd = z.len() as u32;
        z.extend_from_slice(b"PK\x01\x02");
        z.extend_from_slice(&[0; 24]);
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0; 16]);
        z.extend_from_slice(name);
        z.extend_from_slice(b"PK\x05\x06");
        z.extend_from_slice(&[0; 6]);
        z.extend_from_slice(&1u16.to_le_bytes());
        z.extend_from_slice(&[0; 4]);
        z.extend_from_slice(&cd.to_le_bytes());
        z.extend_from_slice(&[0; 2]);
        assert_eq!(zip_member_names(&z), vec!["Content/score.gpif"]);
        let s = sig(r#"{"hex": "504b0304", "zip_member": "Content/score.gpif"}"#);
        assert!(s.matches(&Sniff::new(&z)));
        assert!(!sig(r#"{"zip_member": "a.mscx"}"#).matches(&Sniff::new(&z)));
        // Damaged directories give what was read, never a panic.
        for cut in 0..z.len() {
            let _ = zip_member_names(&z[..cut]);
        }
        z[cd as usize + 28] = 0xff;
        assert!(zip_member_names(&z).is_empty());
    }
}
