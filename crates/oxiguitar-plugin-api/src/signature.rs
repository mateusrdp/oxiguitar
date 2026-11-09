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
//! { "xml_root": "score-partwise" }
//! { "container_root": "score-partwise" }
//! ```
//!
//! The last two (interface 1.7) recognise XML formats by their first
//! element, wherever the file's declaration, comments and document type
//! leave it: in the file itself, or in the root file of a ZIP container
//! (the layout of compressed MusicXML, `.mxl`, and of other programs'
//! files: `META-INF/container.xml` names the root file).
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
    /// The file must be XML whose first element has exactly this name
    /// (interface 1.7). UTF-8, with or without a byte order mark, or UTF-16
    /// with one; the XML declaration, comments, processing instructions and
    /// the document type before the element are skipped, within the file's
    /// first 64 KiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml_root: Option<String>,
    /// The file must be a ZIP container whose `META-INF/container.xml`
    /// names, as its first root file, a member whose first element has
    /// exactly this name, as for `xml_root` (interface 1.7). Compressed
    /// MusicXML (`.mxl`) is such a container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_root: Option<String>,
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
        if self.text.is_none()
            && self.hex.is_none()
            && self.zip_member.is_none()
            && self.xml_root.is_none()
            && self.container_root.is_none()
        {
            return Some(
                "a signature needs at least one of text, hex, zip_member, xml_root and \
                 container_root"
                    .into(),
            );
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
        for name in [&self.xml_root, &self.container_root].into_iter().flatten() {
            if !is_element_name(name) {
                return Some(format!("{name:?} is not an XML element's name"));
            }
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
        if let Some(root) = &self.xml_root
            && file.xml_root() != Some(root.as_str())
        {
            return false;
        }
        if let Some(root) = &self.container_root
            && file.container_root() != Some(root.as_str())
        {
            return false;
        }
        true
    }

    /// The smallest file this signature recognises: what its tests ask
    /// for and nothing else (zeros before `at`). The contract suite feeds
    /// it to a reader as "its signature alone".
    pub fn smallest_file(&self) -> Vec<u8> {
        let expected = match (&self.text, &self.hex) {
            (Some(t), _) => t.as_bytes().to_vec(),
            (None, Some(h)) => decode_hex(h).unwrap_or_default(),
            (None, None) => Vec::new(),
        };
        let at = usize::try_from(self.at).unwrap_or(0).min(1 << 16);
        let mut file = vec![0; at];
        file.extend_from_slice(&expected);
        let mut members: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(root) = &self.container_root {
            let path = "score.xml";
            members.push((
                "META-INF/container.xml".into(),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container><rootfiles>\
                     <rootfile full-path=\"{path}\"/></rootfiles></container>\n"
                )
                .into_bytes(),
            ));
            members.push((path.into(), format!("<{root}/>\n").into_bytes()));
        }
        if let Some(member) = &self.zip_member
            && !members.iter().any(|(m, _)| m == member)
        {
            members.push((member.clone(), Vec::new()));
        }
        if let Some(root) = &self.xml_root {
            // After any bytes asked for at the start (a byte order mark, or
            // the start of the XML declaration, closed here), the element.
            let element = format!("<{root}/>\n");
            let plain = [&file[..], element.as_bytes()].concat();
            let closed = [&file[..], b"?>\n", element.as_bytes()].concat();
            file = if xml_root(&plain).as_deref() == Some(root.as_str()) {
                plain
            } else {
                closed
            };
        }
        if members.is_empty() {
            return file;
        }
        // A ZIP archive starts with its first member's header: the bytes
        // asked for at its start are already in it.
        stored_zip(&members)
    }
}

/// Whether `name` can be an element's name as signatures give it: not
/// empty, no space, no markup.
fn is_element_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | '/' | '"' | '\'' | '=' | '&'))
}

/// A file being recognised. The ZIP directory, the first element and the
/// container's root file are each looked for at most once, and only when a
/// signature asks for them.
#[derive(Debug)]
pub struct Sniff<'a> {
    data: &'a [u8],
    zip: OnceCell<Vec<String>>,
    xml_root: OnceCell<Option<String>>,
    container_root: OnceCell<Option<String>>,
}

impl<'a> Sniff<'a> {
    /// Starts recognising `data`.
    pub fn new(data: &'a [u8]) -> Sniff<'a> {
        Sniff {
            data,
            zip: OnceCell::new(),
            xml_root: OnceCell::new(),
            container_root: OnceCell::new(),
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

    /// The name of the file's first element, if it is XML.
    pub fn xml_root(&self) -> Option<&str> {
        self.xml_root.get_or_init(|| xml_root(self.data)).as_deref()
    }

    /// The name of the first element of the file's root file, if it is a
    /// ZIP container.
    pub fn container_root(&self) -> Option<&str> {
        self.container_root
            .get_or_init(|| container_root(self.data))
            .as_deref()
    }
}

/// Lists a ZIP archive's member names from its central directory, without
/// decompressing anything. Anything that isn't a well-formed ZIP gives the
/// names read before the damage (often none).
pub fn zip_member_names(data: &[u8]) -> Vec<String> {
    zip_entries(data)
        .into_iter()
        .map(|e| String::from_utf8_lossy(e.name).into_owned())
        .collect()
}

/// How much of a file, or of a ZIP member, is looked at for its first
/// element.
const XML_PREFIX: usize = 64 * 1024;

/// The name of the first element of `data`, if it is XML and the element
/// starts within its first 64 KiB.
pub fn xml_root(data: &[u8]) -> Option<String> {
    let text = xml_text(data.get(..XML_PREFIX).unwrap_or(data));
    first_element(&text).map(str::to_owned)
}

/// The name of the first element of the root file of a ZIP container
/// (`META-INF/container.xml`'s first `rootfile`), if `data` is one.
pub fn container_root(data: &[u8]) -> Option<String> {
    let entries = zip_entries(data);
    let member = |name: &[u8]| {
        entries
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| e.contents(data, XML_PREFIX))
    };
    let container = xml_text(&member(b"META-INF/container.xml")?).into_owned();
    let path = first_rootfile(&container)?;
    xml_root(&member(path.as_bytes())?)
}

/// One member of a ZIP archive, as its central directory gives it.
struct ZipEntry<'a> {
    name: &'a [u8],
    method: u16,
    compressed: u32,
    local: u32,
}

impl ZipEntry<'_> {
    /// The member's first `limit` bytes or fewer, if it is stored or
    /// deflated and its data is where the directory says. A member cut
    /// short gives what could be read.
    fn contents(&self, data: &[u8], limit: usize) -> Option<Vec<u8>> {
        let local = usize::try_from(self.local).ok()?;
        if data.get(local..local.checked_add(4)?) != Some(b"PK\x03\x04") {
            return None;
        }
        let u16_at = |i: usize| {
            let b: [u8; 2] = data.get(i..i.checked_add(2)?)?.try_into().ok()?;
            Some(usize::from(u16::from_le_bytes(b)))
        };
        let start = local
            .checked_add(30)?
            .checked_add(u16_at(local + 26)?)?
            .checked_add(u16_at(local + 28)?)?;
        let end = start
            .saturating_add(usize::try_from(self.compressed).ok()?)
            .min(data.len());
        let raw = data.get(start..end)?;
        match self.method {
            0 => Some(raw.get(..limit).unwrap_or(raw).to_vec()),
            8 => Some(
                match miniz_oxide::inflate::decompress_to_vec_with_limit(raw, limit) {
                    Ok(out) => out,
                    Err(e) => e.output,
                },
            ),
            _ => None,
        }
    }
}

/// The members a ZIP archive's central directory lists, up to any damage.
fn zip_entries(data: &[u8]) -> Vec<ZipEntry<'_>> {
    let mut entries = Vec::new();
    if !data.starts_with(b"PK\x03\x04") {
        return entries;
    }
    // The end-of-central-directory record is at least 22 bytes and sits in
    // the last 64 KiB + 22 bytes (its comment is at most 65535 bytes).
    let search_from = data.len().saturating_sub(22 + 65_535);
    let Some(eocd) = (search_from..data.len().saturating_sub(21))
        .rev()
        .find(|&i| data.get(i..i + 4) == Some(b"PK\x05\x06"))
    else {
        return entries;
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
        return entries;
    };
    let Ok(mut p) = usize::try_from(offset) else {
        return entries;
    };
    for _ in 0..count {
        if data.get(p..p.saturating_add(4)) != Some(b"PK\x01\x02") {
            break;
        }
        let (
            Some(method),
            Some(compressed),
            Some(name_len),
            Some(extra_len),
            Some(comment_len),
            Some(local),
        ) = (
            u16_at(p + 10),
            u32_at(p + 20),
            u16_at(p + 28),
            u16_at(p + 30),
            u16_at(p + 32),
            u32_at(p + 42),
        )
        else {
            break;
        };
        let start = p + 46;
        let Some(name) = data.get(start..start + usize::from(name_len)) else {
            break;
        };
        entries.push(ZipEntry {
            name,
            method,
            compressed,
            local,
        });
        p = start + usize::from(name_len) + usize::from(extra_len) + usize::from(comment_len);
    }
    entries
}

/// `data` as text: UTF-16 when it starts with that byte order mark, UTF-8
/// otherwise (without its mark), damaged characters replaced.
fn xml_text(data: &[u8]) -> std::borrow::Cow<'_, str> {
    let utf16 = |be: bool| {
        let units = data[2..].as_chunks::<2>().0.iter().map(|&pair| {
            if be {
                u16::from_be_bytes(pair)
            } else {
                u16::from_le_bytes(pair)
            }
        });
        char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>()
            .into()
    };
    match data {
        [0xff, 0xfe, ..] => utf16(false),
        [0xfe, 0xff, ..] => utf16(true),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest),
        _ => String::from_utf8_lossy(data),
    }
}

/// The name of the first element in XML `text`, after the declaration,
/// comments, processing instructions and the document type (with its
/// internal subset). Anything else first, or no element, gives none.
fn first_element(text: &str) -> Option<&str> {
    let mut s = text;
    loop {
        s = s.trim_start_matches([' ', '\t', '\r', '\n']);
        if let Some(r) = s.strip_prefix("<?") {
            s = r.split_once("?>")?.1;
        } else if let Some(r) = s.strip_prefix("<!--") {
            s = r.split_once("-->")?.1;
        } else if let Some(r) = s.strip_prefix("<!DOCTYPE") {
            s = after_doctype(r)?;
        } else {
            let r = s.strip_prefix('<')?;
            let end = r.find(|c: char| c.is_whitespace() || c == '>' || c == '/')?;
            let name = r.get(..end)?;
            return is_element_name(name).then_some(name);
        }
    }
}

/// What follows a document type declaration, given what follows its
/// `<!DOCTYPE`: its end is the first `>` outside quotes, comments and its
/// internal subset (`[ … ]`).
fn after_doctype(s: &str) -> Option<&str> {
    let mut quote = None;
    let mut subset = false;
    let mut i = 0;
    while let Some(c) = s.get(i..)?.chars().next() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '<') if s.get(i..)?.starts_with("<!--") => {
                let end = s.get(i + 4..)?.find("-->")?;
                i += 4 + end + 3;
                continue;
            }
            (None, '"' | '\'') => quote = Some(c),
            (None, '[') => subset = true,
            (None, ']') => subset = false,
            (None, '>') if !subset => return s.get(i + 1..),
            _ => {}
        }
        i += c.len_utf8();
    }
    None
}

/// The `full-path` of the first `rootfile` element in a container's XML,
/// its five predefined entities resolved.
fn first_rootfile(container: &str) -> Option<String> {
    let mut rest = container;
    loop {
        let at = rest.find("<rootfile")?;
        rest = rest.get(at + "<rootfile".len()..)?;
        // `<rootfiles>` is the list, not a root file.
        if rest.starts_with(|c: char| c.is_whitespace() || c == '/' || c == '>') {
            break;
        }
    }
    let tag = rest.get(..rest.find('>')?)?;
    let mut attrs = tag;
    while let Some(at) = attrs.find("full-path") {
        let after = attrs.get(at + "full-path".len()..)?;
        let before_ok = at == 0 || attrs.get(..at)?.ends_with(char::is_whitespace);
        let value = after.trim_start().strip_prefix('=').map(str::trim_start);
        if let (true, Some(value)) = (before_ok, value) {
            let q = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
            let inner = value.get(1..)?;
            let raw = inner.get(..inner.find(q)?)?;
            return Some(
                raw.replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'")
                    .replace("&amp;", "&"),
            );
        }
        attrs = after;
    }
    None
}

/// A ZIP archive of `members`, stored (not compressed), the first one's
/// header at its start.
fn stored_zip(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut z = Vec::new();
    let mut directory = Vec::new();
    for (name, contents) in members {
        let local = u32::try_from(z.len()).unwrap_or(u32::MAX);
        let crc = crc32(contents).to_le_bytes();
        let size = u32::try_from(contents.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes();
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX).to_le_bytes();
        // Version 2.0, no flags, stored, no date.
        let common = [
            &[20, 0, 0, 0, 0, 0, 0, 0, 0, 0][..],
            &crc,
            &size,
            &size,
            &name_len,
            &[0, 0],
        ]
        .concat();
        z.extend_from_slice(b"PK\x03\x04");
        z.extend_from_slice(&common);
        z.extend_from_slice(name.as_bytes());
        z.extend_from_slice(contents);
        directory.extend_from_slice(b"PK\x01\x02\x14\x00");
        directory.extend_from_slice(&common);
        // No comment, disk 0, no attributes, then the local header's offset.
        directory.extend_from_slice(&[0; 10]);
        directory.extend_from_slice(&local.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let offset = u32::try_from(z.len()).unwrap_or(u32::MAX).to_le_bytes();
    let size = u32::try_from(directory.len())
        .unwrap_or(u32::MAX)
        .to_le_bytes();
    let count = u16::try_from(members.len())
        .unwrap_or(u16::MAX)
        .to_le_bytes();
    z.extend_from_slice(&directory);
    z.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00");
    z.extend_from_slice(&count);
    z.extend_from_slice(&count);
    z.extend_from_slice(&size);
    z.extend_from_slice(&offset);
    z.extend_from_slice(&[0, 0]);
    z
}

/// The CRC-32 of `data` (ISO-HDLC, as ZIP uses it).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
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
        assert!(sig(r#"{"xml_root": "score-partwise"}"#).problem().is_none());
        assert!(
            sig(r#"{"container_root": "museScore"}"#)
                .problem()
                .is_none()
        );
        for bad in ["", "a b", "<a>", "a/b"] {
            let json = format!(r#"{{"xml_root": {bad:?}}}"#);
            assert!(sig(&json).problem().is_some(), "{bad:?}");
        }
    }

    const PARTWISE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \
        \"http://www.musicxml.org/dtds/partwise.dtd\">\n\
        <score-partwise version=\"4.0\">\n  <part-list/>\n</score-partwise>\n";

    #[test]
    fn the_first_element_past_what_comes_before_it() {
        let partwise = sig(r#"{"xml_root": "score-partwise"}"#);
        let timewise = sig(r#"{"xml_root": "score-timewise"}"#);
        assert!(partwise.matches(&Sniff::new(PARTWISE.as_bytes())));
        assert!(!timewise.matches(&Sniff::new(PARTWISE.as_bytes())));
        let busy = "\u{feff}<?xml version='1.0'?>\r\n<!-- by a > program -->\
            <?app keep?><!DOCTYPE score-partwise [ <!ENTITY a '>'> <!-- ] --> ]>\
            \n\t<score-partwise>";
        assert_eq!(xml_root(busy.as_bytes()).as_deref(), Some("score-partwise"));
        for be in [false, true] {
            let mut utf16 = if be {
                vec![0xfe, 0xff]
            } else {
                vec![0xff, 0xfe]
            };
            for u in PARTWISE.encode_utf16() {
                utf16.extend(if be { u.to_be_bytes() } else { u.to_le_bytes() });
            }
            assert!(
                partwise.matches(&Sniff::new(&utf16)),
                "UTF-16, big endian {be}"
            );
        }
        assert_eq!(
            xml_root(b"<museScore version=\"4.60\">").as_deref(),
            Some("museScore")
        );
        assert_eq!(xml_root(b"<a/>").as_deref(), Some("a"));
        // Not XML, or no element where one was looked for.
        for not in [
            &b""[..],
            b"hello <a>",
            b"<?xml",
            b"<!-- open",
            b"<",
            b"\0\0<a>",
            b"PK\x03\x04",
        ] {
            assert_eq!(xml_root(not), None, "{not:?}");
        }
        let far = format!("<!--{}-->\n<a/>", " ".repeat(XML_PREFIX));
        assert_eq!(xml_root(far.as_bytes()), None, "past the first 64 KiB");
        // Text at an offset and the first element, together.
        let both = sig(r#"{"text": "<?xml", "xml_root": "score-partwise"}"#);
        assert!(both.matches(&Sniff::new(PARTWISE.as_bytes())));
    }

    /// A ZIP archive of `members`, each deflated when `deflate` says so.
    fn zip(members: &[(&str, &str)], deflate: bool) -> Vec<u8> {
        let stored: Vec<(String, Vec<u8>)> = members
            .iter()
            .map(|(n, c)| ((*n).to_owned(), c.as_bytes().to_vec()))
            .collect();
        let mut z = stored_zip(&stored);
        if deflate {
            // Rebuild with every member deflated: same layout, method 8.
            let mut out = Vec::new();
            let mut directory = Vec::new();
            for (name, contents) in &stored {
                let packed = miniz_oxide::deflate::compress_to_vec(contents, 6);
                let local = (out.len() as u32).to_le_bytes();
                let common = [
                    &[20, 0, 0, 0, 8, 0, 0, 0, 0, 0][..],
                    &crc32(contents).to_le_bytes(),
                    &(packed.len() as u32).to_le_bytes(),
                    &(contents.len() as u32).to_le_bytes(),
                    &(name.len() as u16).to_le_bytes(),
                    &[0, 0],
                ]
                .concat();
                out.extend_from_slice(b"PK\x03\x04");
                out.extend_from_slice(&common);
                out.extend_from_slice(name.as_bytes());
                out.extend_from_slice(&packed);
                directory.extend_from_slice(b"PK\x01\x02\x14\x00");
                directory.extend_from_slice(&common);
                directory.extend_from_slice(&[0; 10]);
                directory.extend_from_slice(&local);
                directory.extend_from_slice(name.as_bytes());
            }
            let offset = (out.len() as u32).to_le_bytes();
            let count = (stored.len() as u16).to_le_bytes();
            let size = (directory.len() as u32).to_le_bytes();
            out.extend_from_slice(&directory);
            out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
            out.extend_from_slice(&count);
            out.extend_from_slice(&count);
            out.extend_from_slice(&size);
            out.extend_from_slice(&offset);
            out.extend_from_slice(&[0, 0]);
            z = out;
        }
        z
    }

    #[test]
    fn the_root_file_of_a_container() {
        let mxl_container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container>\n\
            <rootfiles>\n<rootfile full-path=\"Tom &amp; Vin.xml\" \
            media-type=\"application/vnd.recordare.musicxml+xml\"/>\n</rootfiles>\n</container>";
        // MuseScore 4 lists its style file first; its first element is
        // museScore too.
        let mscz_container = "<container><rootfiles><rootfile full-path='score_style.mss'/>\
            <rootfile full-path='Song.mscx'/></rootfiles></container>";
        let mxl = sig(r#"{"container_root": "score-partwise"}"#);
        let mscz = sig(r#"{"container_root": "museScore"}"#);
        for deflate in [false, true] {
            let a = zip(
                &[
                    ("mimetype", "application/vnd.recordare.musicxml"),
                    ("META-INF/container.xml", mxl_container),
                    ("Tom & Vin.xml", PARTWISE),
                ],
                deflate,
            );
            let b = zip(
                &[
                    (
                        "score_style.mss",
                        "<?xml version=\"1.0\"?>\n<museScore version=\"4.60\"><Style/></museScore>",
                    ),
                    ("Song.mscx", "<museScore version=\"4.60\"/>"),
                    ("META-INF/container.xml", mscz_container),
                ],
                deflate,
            );
            assert!(mxl.matches(&Sniff::new(&a)), "deflated {deflate}");
            assert!(!mscz.matches(&Sniff::new(&a)), "deflated {deflate}");
            assert!(mscz.matches(&Sniff::new(&b)), "deflated {deflate}");
            assert!(!mxl.matches(&Sniff::new(&b)), "deflated {deflate}");
            // The archive's own first bytes are not an element.
            assert!(!sig(r#"{"xml_root": "score-partwise"}"#).matches(&Sniff::new(&a)));
            // Damaged anywhere: an answer, never a panic.
            for cut in 0..a.len() {
                let _ = container_root(&a[..cut]);
            }
            let mut flipped = a.clone();
            for i in (0..flipped.len()).step_by(7) {
                flipped[i] ^= 0x5a;
                let _ = container_root(&flipped);
            }
        }
        // No container, or a root file the archive doesn't hold.
        assert_eq!(
            container_root(&zip(&[("score.xml", PARTWISE)], false)),
            None
        );
        let missing = zip(&[("META-INF/container.xml", mxl_container)], true);
        assert_eq!(container_root(&missing), None);
        assert_eq!(first_rootfile("<rootfiles></rootfiles>"), None);
        assert_eq!(
            first_rootfile("<rootfiles><rootfile media-type=\"x\" full-path = \"a/b.xml\"/>")
                .as_deref(),
            Some("a/b.xml")
        );
    }

    #[test]
    fn every_signature_recognises_its_smallest_file() {
        for json in [
            r#"{"at": 1, "text": "FICHIER GUITAR PRO v5"}"#,
            r#"{"hex": "42434658"}"#,
            r#"{"zip_member": "Content/score.gpif"}"#,
            r#"{"hex": "504b0304", "zip_member": "Content/score.gpif"}"#,
            r#"{"xml_root": "score-partwise"}"#,
            r#"{"text": "<?xml", "xml_root": "score-partwise"}"#,
            r#"{"hex": "efbbbf", "xml_root": "score-timewise"}"#,
            r#"{"container_root": "score-partwise"}"#,
            r#"{"zip_member": "META-INF/container.xml", "container_root": "museScore"}"#,
        ] {
            let s = sig(json);
            assert!(s.matches(&Sniff::new(&s.smallest_file())), "{json}");
        }
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
