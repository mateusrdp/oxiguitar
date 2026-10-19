// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Bytes inside messages: a file's contents, a service's own message.

use std::borrow::Cow;
use std::fmt;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A run of bytes inside a message. It is encoded as one block (not byte
/// by byte), and decoding borrows it from the encoded message when it can,
/// so a 10 MB file is copied once on its way into a plugin.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Blob<'a>(pub Cow<'a, [u8]>);

impl<'a> Blob<'a> {
    /// Borrows `bytes`.
    pub fn borrowed(bytes: &'a [u8]) -> Blob<'a> {
        Blob(Cow::Borrowed(bytes))
    }

    /// Takes `bytes`.
    pub fn owned(bytes: Vec<u8>) -> Blob<'static> {
        Blob(Cow::Owned(bytes))
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// The bytes, owned.
    pub fn into_vec(self) -> Vec<u8> {
        self.0.into_owned()
    }

    /// A copy that owns its bytes.
    pub fn into_owned(self) -> Blob<'static> {
        Blob(Cow::Owned(self.0.into_owned()))
    }
}

impl fmt::Debug for Blob<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blob({} bytes)", self.0.len())
    }
}

impl Serialize for Blob<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.0)
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Blob<'a> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Blob<'a>, D::Error> {
        d.deserialize_bytes(BlobVisitor)
    }
}

struct BlobVisitor;

impl<'de> Visitor<'de> for BlobVisitor {
    type Value = Blob<'de>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bytes")
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Blob<'de>, E> {
        Ok(Blob(Cow::Borrowed(v)))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Blob<'de>, E> {
        Ok(Blob(Cow::Owned(v.to_vec())))
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Blob<'de>, E> {
        Ok(Blob(Cow::Owned(v)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Blob<'de>, A::Error> {
        let mut v = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1 << 16));
        while let Some(b) = seq.next_element()? {
            v.push(b);
        }
        Ok(Blob(Cow::Owned(v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_is_borrowed_from_the_encoded_message() {
        let data: Vec<u8> = (0..=255).collect();
        let bytes = crate::encode(&Blob::borrowed(&data)).unwrap();
        // A length (varint, 2 bytes for 256) then the bytes as they are.
        assert_eq!(bytes.len(), 2 + 256);
        let back: Blob<'_> = crate::decode(&bytes).unwrap();
        assert!(matches!(back.0, Cow::Borrowed(_)));
        assert_eq!(back.as_slice(), &data[..]);
    }
}
