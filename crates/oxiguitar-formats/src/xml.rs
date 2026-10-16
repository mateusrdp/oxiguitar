// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A small XML element tree built with quick-xml.
//!
//! GPIF (and later MuseScore and MusicXML) refer from one part of the file to
//! another by id, so the readers want the whole document at hand rather than a
//! stream of events. The tree keeps element names, attributes, text and the
//! byte offset of each start tag for error messages. Entity references other
//! than XML's five predefined ones and character references are dropped; DTDs
//! are ignored, so no entity can expand (no "billion laughs").

use std::borrow::Cow;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{ReadError, ReadErrorKind};

/// Deeper nesting than this means a damaged or hostile file.
const MAX_DEPTH: usize = 256;
/// More elements than this means a damaged or hostile file. The largest GPIF
/// file in our corpus (10 MB) has about 900,000.
const MAX_ELEMENTS: usize = 20_000_000;

/// One XML element with its attributes, text and child elements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Element {
    /// Tag name, with any namespace prefix.
    pub(crate) name: String,
    attrs: Vec<(String, String)>,
    /// Child elements, in document order.
    pub(crate) children: Vec<Element>,
    /// All text and CDATA directly inside this element, joined.
    text: String,
    /// Byte offset of the start tag in the document.
    pub(crate) offset: usize,
}

impl Element {
    /// The first child with this name.
    pub(crate) fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All children with this name.
    pub(crate) fn children_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The element's own text, trimmed.
    pub(crate) fn text(&self) -> &str {
        self.text.trim()
    }

    /// The element's own text, untrimmed.
    pub(crate) fn raw_text(&self) -> &str {
        &self.text
    }

    /// The trimmed text of the first child with this name.
    pub(crate) fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(Element::text)
    }

    /// Follows a path of child names, such as `["Staves", "Staff"]`.
    pub(crate) fn path(&self, names: &[&str]) -> Option<&Element> {
        names.iter().try_fold(self, |e, n| e.child(n))
    }

    /// An attribute's value.
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Parses a whole document and returns its root element.
pub(crate) fn parse(data: &[u8]) -> Result<Element, ReadError> {
    let text: Cow<'_, str> =
        String::from_utf8_lossy(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data));
    let mut reader = Reader::from_str(&text);
    // Element stack: the last entry is the element being filled.
    let mut stack: Vec<Element> = vec![Element::default()];
    let mut count = 0usize;
    loop {
        let offset = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let event = reader.read_event().map_err(|e| {
            ReadError::new(
                ReadErrorKind::Invalid(format!("malformed XML: {e}")),
                Some(offset),
            )
        })?;
        match event {
            Event::Start(start) => {
                count += 1;
                if stack.len() > MAX_DEPTH || count > MAX_ELEMENTS {
                    return Err(ReadError::invalid(
                        "XML nested or sized beyond limits",
                        offset,
                    ));
                }
                stack.push(element(&start, offset)?);
            }
            Event::Empty(start) => {
                count += 1;
                if count > MAX_ELEMENTS {
                    return Err(ReadError::invalid("XML sized beyond limits", offset));
                }
                let e = element(&start, offset)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(e);
                }
            }
            Event::End(_) => {
                // quick-xml checks that end tags match their start tags.
                let Some(done) = stack.pop() else {
                    return Err(ReadError::invalid("unexpected end tag", offset));
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(done),
                    None => return Err(ReadError::invalid("unexpected end tag", offset)),
                }
            }
            Event::Text(t) => {
                if let Some(e) = stack.last_mut() {
                    e.text.push_str(&t.xml10_content());
                }
            }
            Event::CData(t) => {
                if let Some(e) = stack.last_mut() {
                    e.text.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    Ok(None) => quick_xml::escape::resolve_predefined_entity(&r).map(str::to_owned),
                    Err(_) => None,
                };
                if let (Some(s), Some(e)) = (resolved, stack.last_mut()) {
                    e.text.push_str(&s);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if stack.len() != 1 {
        return Err(ReadError::new(
            ReadErrorKind::UnexpectedEnd,
            Some(data.len()),
        ));
    }
    let mut document = stack.pop().unwrap_or_default();
    match document.children.len() {
        1 => Ok(document.children.remove(0)),
        0 => Err(ReadError::invalid("the XML document is empty", 0)),
        _ => Err(ReadError::invalid(
            "the XML document has more than one root element",
            0,
        )),
    }
}

fn element(start: &BytesStart<'_>, offset: usize) -> Result<Element, ReadError> {
    let name = start.name().as_ref().to_owned();
    let mut attrs = Vec::new();
    for a in start.attributes() {
        let a =
            a.map_err(|e| ReadError::invalid(format!("malformed XML attribute: {e}"), offset))?;
        let key = a.key.as_ref().to_owned();
        let value = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map(Cow::into_owned)
            .unwrap_or_else(|_| a.value.clone().into_owned());
        attrs.push((key, value));
    }
    Ok(Element {
        name,
        attrs,
        children: Vec::new(),
        text: String::new(),
        offset,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn tree_text_cdata_and_attributes() {
        let doc = br#"<?xml version="1.0"?>
<!DOCTYPE x [<!ENTITY a "boom">]>
<GPIF><Title><![CDATA[Tarde em Itapu&atilde;]]></Title>
<Note id="3" kind="a&amp;b"><Fret>5</Fret><Tie origin="true"/></Note>
<T>x &amp; y &#65; &a;</T></GPIF>"#;
        let root = parse(doc).unwrap();
        assert_eq!(root.name, "GPIF");
        assert_eq!(root.child_text("Title"), Some("Tarde em Itapu&atilde;"));
        let note = root.child("Note").unwrap();
        assert_eq!(note.attr("id"), Some("3"));
        assert_eq!(note.attr("kind"), Some("a&b"));
        assert_eq!(note.child_text("Fret"), Some("5"));
        assert_eq!(note.child("Tie").unwrap().attr("origin"), Some("true"));
        // Undeclared and DTD entities are dropped, never expanded.
        assert_eq!(root.child_text("T"), Some("x & y A"));
        assert_eq!(root.path(&["Note", "Fret"]).unwrap().text(), "5");
    }

    #[test]
    fn errors_do_not_panic() {
        for bad in [
            &b""[..],
            b"<a>",
            b"<a></b>",
            b"</a>",
            b"<a/><b/>",
            b"<a x=1/>",
            b"\xff\xfe<\x00",
        ] {
            assert!(parse(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
        }
        let deep = "<a>".repeat(MAX_DEPTH + 2);
        assert!(parse(deep.as_bytes()).is_err());
    }
}
