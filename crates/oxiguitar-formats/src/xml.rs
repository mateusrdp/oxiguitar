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

/// No node: the end of a list of children.
const NONE: u32 = u32::MAX;

/// A parsed document: every element in one list, in document order.
///
/// Names, attribute values and text borrow from the document text wherever
/// they can, and the elements sit in one vector rather than one allocation
/// each, so building and dropping the tree costs little: a 10 MB GPIF file
/// has close to a million elements, and the tree was most of the time it
/// took to read one (the speed budget, M1).
#[derive(Debug)]
pub(crate) struct Document<'a> {
    nodes: Vec<Node<'a>>,
    attrs: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    /// The root element's index.
    root: u32,
}

#[derive(Debug)]
struct Node<'a> {
    /// Tag name, with any namespace prefix.
    name: Cow<'a, str>,
    /// All text and CDATA directly inside the element, joined, except that
    /// blank text before the first other text is not kept.
    text: Cow<'a, str>,
    /// Byte offset of the start tag in the document.
    offset: usize,
    /// This element's attributes in [`Document::attrs`].
    attrs: (u32, u32),
    first_child: u32,
    last_child: u32,
    next_sibling: u32,
}

impl<'a> Document<'a> {
    /// The root element.
    pub(crate) fn root(&self) -> Element<'_> {
        Element {
            doc: self,
            index: self.root,
        }
    }

    fn push(&mut self, parent: u32, name: Cow<'a, str>, offset: usize, attrs: (u32, u32)) -> u32 {
        let index = self.nodes.len() as u32;
        self.nodes.push(Node {
            name,
            text: Cow::Borrowed(""),
            offset,
            attrs,
            first_child: NONE,
            last_child: NONE,
            next_sibling: NONE,
        });
        if let Some(p) = self.nodes.get_mut(parent as usize) {
            let last = p.last_child;
            p.last_child = index;
            if last == NONE {
                p.first_child = index;
            } else if let Some(l) = self.nodes.get_mut(last as usize) {
                l.next_sibling = index;
            }
        }
        index
    }
}

/// One XML element: its name, attributes, text and child elements. A
/// small handle into its [`Document`].
#[derive(Clone, Copy)]
pub(crate) struct Element<'a> {
    doc: &'a Document<'a>,
    index: u32,
}

impl std::fmt::Debug for Element<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}> at byte {}", self.name(), self.offset())
    }
}

/// Stands in for a node that is not there, which cannot happen: handles
/// are only made for nodes that exist.
static NO_NODE: Node<'static> = Node {
    name: Cow::Borrowed(""),
    text: Cow::Borrowed(""),
    offset: 0,
    attrs: (0, 0),
    first_child: NONE,
    last_child: NONE,
    next_sibling: NONE,
};

impl<'a> Element<'a> {
    fn node(self) -> &'a Node<'a> {
        self.doc.nodes.get(self.index as usize).unwrap_or(&NO_NODE)
    }

    /// Tag name, with any namespace prefix.
    pub(crate) fn name(self) -> &'a str {
        &self.node().name
    }

    /// Byte offset of the start tag in the document.
    pub(crate) fn offset(self) -> usize {
        self.node().offset
    }

    /// The child elements, in document order.
    pub(crate) fn children(self) -> impl Iterator<Item = Element<'a>> {
        let doc = self.doc;
        std::iter::successors(
            Some(self.node().first_child).filter(|i| *i != NONE),
            move |i| {
                doc.nodes
                    .get(*i as usize)
                    .map(|n| n.next_sibling)
                    .filter(|n| *n != NONE)
            },
        )
        .map(move |index| Element { doc, index })
    }

    /// Whether the element has child elements.
    pub(crate) fn has_children(self) -> bool {
        self.node().first_child != NONE
    }

    /// The first child with this name.
    pub(crate) fn child(self, name: &str) -> Option<Element<'a>> {
        self.children().find(|c| c.name() == name)
    }

    /// All children with this name.
    pub(crate) fn children_named(self, name: &str) -> impl Iterator<Item = Element<'a>> {
        self.children().filter(move |c| c.name() == name)
    }

    /// The element's own text, trimmed.
    pub(crate) fn text(self) -> &'a str {
        self.node().text.trim()
    }

    /// The element's own text, untrimmed, except that blank text before
    /// the first other text is not kept.
    pub(crate) fn raw_text(self) -> &'a str {
        &self.node().text
    }

    /// The trimmed text of the first child with this name.
    pub(crate) fn child_text(self, name: &str) -> Option<&'a str> {
        self.child(name).map(Element::text)
    }

    /// Follows a path of child names, such as `["Staves", "Staff"]`.
    pub(crate) fn path(self, names: &[&str]) -> Option<Element<'a>> {
        names.iter().try_fold(self, |e, n| e.child(n))
    }

    /// An attribute's value.
    pub(crate) fn attr(self, name: &str) -> Option<&'a str> {
        let (start, end) = self.node().attrs;
        self.doc
            .attrs
            .get(start as usize..end as usize)
            .unwrap_or_default()
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_ref())
    }
}

/// The document as text: UTF-8 with any byte-order mark removed, invalid
/// bytes replaced. [`parse`] borrows from it.
pub(crate) fn decode(data: &[u8]) -> Cow<'_, str> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    // The plain check is several times faster than the lossy one, and
    // files are nearly always valid.
    match std::str::from_utf8(data) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => String::from_utf8_lossy(data),
    }
}

/// `part` as a slice of `document` when it lies inside it, which quick-xml's
/// names and values do whenever they need no unescaping: so a slice handed
/// out for a moment can be kept as long as the document. A copy otherwise.
fn within<'a>(document: &'a str, part: &str) -> Cow<'a, str> {
    (part.as_ptr() as usize)
        .checked_sub(document.as_ptr() as usize)
        .and_then(|start| document.get(start..start.checked_add(part.len())?))
        .filter(|inside| *inside == part)
        .map_or_else(|| Cow::Owned(part.to_owned()), Cow::Borrowed)
}

/// Parses a whole document (see [`decode`]).
pub(crate) fn parse(text: &str) -> Result<Document<'_>, ReadError> {
    let mut reader = Reader::from_str(text);
    let mut doc = Document {
        nodes: Vec::with_capacity(text.len() / 16),
        attrs: Vec::new(),
        root: NONE,
    };
    // A node for the document itself holds the root element.
    doc.push(NONE, Cow::Borrowed(""), 0, (0, 0));
    // The open elements: the last is the one being filled.
    let mut stack: Vec<u32> = vec![0];
    loop {
        let offset = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let event = reader.read_event().map_err(|e| {
            ReadError::new(
                ReadErrorKind::Invalid(format!("malformed XML: {e}")),
                Some(offset),
            )
        })?;
        let parent = stack.last().copied().unwrap_or(0);
        match event {
            Event::Start(start) => {
                if stack.len() > MAX_DEPTH || doc.nodes.len() > MAX_ELEMENTS {
                    return Err(ReadError::invalid(
                        "XML nested or sized beyond limits",
                        offset,
                    ));
                }
                let (name, attrs) = element(text, &start, offset, &mut doc.attrs)?;
                stack.push(doc.push(parent, name, offset, attrs));
            }
            Event::Empty(start) => {
                if doc.nodes.len() > MAX_ELEMENTS {
                    return Err(ReadError::invalid("XML sized beyond limits", offset));
                }
                let (name, attrs) = element(text, &start, offset, &mut doc.attrs)?;
                doc.push(parent, name, offset, attrs);
            }
            Event::End(_) => {
                // quick-xml checks that end tags match their start tags.
                if stack.len() <= 1 {
                    return Err(ReadError::invalid("unexpected end tag", offset));
                }
                stack.pop();
            }
            Event::Text(t) => push_text(&mut doc, parent, t.xml10_content()),
            Event::CData(t) => push_text(&mut doc, parent, t.xml10_content()),
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    Ok(None) => quick_xml::escape::resolve_predefined_entity(&r).map(str::to_owned),
                    Err(_) => None,
                };
                if let Some(s) = resolved {
                    push_text(&mut doc, parent, Cow::Owned(s));
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if stack.len() != 1 {
        return Err(ReadError::new(
            ReadErrorKind::UnexpectedEnd,
            Some(text.len()),
        ));
    }
    let top = doc.nodes.first().unwrap_or(&NO_NODE);
    if top.first_child == NONE {
        return Err(ReadError::invalid("the XML document is empty", 0));
    }
    if top.first_child != top.last_child {
        return Err(ReadError::invalid(
            "the XML document has more than one root element",
            0,
        ));
    }
    doc.root = top.first_child;
    Ok(doc)
}

/// Text in element `index`. Text outside the root element (the document
/// node's) is dropped.
fn push_text<'a>(doc: &mut Document<'a>, index: u32, chunk: Cow<'a, str>) {
    if index == 0 {
        return;
    }
    if let Some(node) = doc.nodes.get_mut(index as usize) {
        if node.text.trim().is_empty() {
            node.text = chunk;
        } else if !chunk.is_empty() {
            node.text.to_mut().push_str(&chunk);
        }
    }
}

/// A start tag's name and attributes; the attributes go into `attrs`, and
/// their range there is returned.
fn element<'a>(
    document: &'a str,
    start: &BytesStart<'a>,
    offset: usize,
    attrs: &mut Vec<(Cow<'a, str>, Cow<'a, str>)>,
) -> Result<(Cow<'a, str>, (u32, u32)), ReadError> {
    let name = within(document, start.name().as_ref());
    let first = attrs.len() as u32;
    // No check for repeated attributes: the first one counts, as in
    // [`Element::attr`], and the check is a tenth of the time spent here.
    for a in start.attributes().with_checks(false) {
        let a =
            a.map_err(|e| ReadError::invalid(format!("malformed XML attribute: {e}"), offset))?;
        let key = within(document, a.key.as_ref());
        let value = match a.normalized_value(XmlVersion::Implicit1_0) {
            Ok(Cow::Borrowed(v)) => within(document, v),
            Ok(Cow::Owned(v)) => Cow::Owned(v),
            Err(_) => within(document, &a.value),
        };
        attrs.push((key, value));
    }
    Ok((name, (first, attrs.len() as u32)))
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
        let text = decode(doc);
        let doc = parse(&text).unwrap();
        let root = doc.root();
        assert_eq!(root.name(), "GPIF");
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
            assert!(
                parse(&decode(bad)).is_err(),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
        let deep = "<a>".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep).is_err());
    }
}
