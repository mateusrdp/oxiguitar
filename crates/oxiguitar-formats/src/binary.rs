// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A bounds-checked little-endian reader for binary formats.
//!
//! Every read checks the remaining length first, so a damaged file produces a
//! `ReadError` with the byte offset, never a panic or an oversized allocation.

use crate::error::{ReadError, ReadErrorKind};

/// A read position in a byte slice.
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

pub(crate) type Result<T> = std::result::Result<T, ReadError>;

impl<'a> Cursor<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Cursor<'a> {
        Cursor { data, pos: 0 }
    }

    /// Current offset from the start.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes left to read.
    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn eof(&self) -> ReadError {
        ReadError::new(ReadErrorKind::UnexpectedEnd, Some(self.pos))
    }

    /// The next `n` bytes.
    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| self.eof())?;
        let slice = self.data.get(self.pos..end).ok_or_else(|| self.eof())?;
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        let b = self.data.get(self.pos).copied().ok_or_else(|| self.eof())?;
        self.pos += 1;
        Ok(b)
    }

    pub(crate) fn i8(&mut self) -> Result<i8> {
        Ok(i8::from_le_bytes([self.u8()?]))
    }

    pub(crate) fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub(crate) fn i16(&mut self) -> Result<i16> {
        let b: [u8; 2] = self.bytes(2)?.try_into().map_err(|_| self.eof())?;
        Ok(i16::from_le_bytes(b))
    }

    pub(crate) fn i32(&mut self) -> Result<i32> {
        let b: [u8; 4] = self.bytes(4)?.try_into().map_err(|_| self.eof())?;
        Ok(i32::from_le_bytes(b))
    }

    pub(crate) fn f64(&mut self) -> Result<f64> {
        let b: [u8; 8] = self.bytes(8)?.try_into().map_err(|_| self.eof())?;
        Ok(f64::from_le_bytes(b))
    }

    /// Reads an `i32` count and checks it is between 0 and `max`, and that
    /// the file has at least `min_item_size` bytes left per item. This keeps a
    /// damaged count from causing a huge allocation.
    pub(crate) fn count(&mut self, what: &str, max: usize, min_item_size: usize) -> Result<usize> {
        let at = self.pos;
        let n = self.i32()?;
        let n = usize::try_from(n)
            .map_err(|_| ReadError::invalid(format!("negative {what} ({n})"), at))?;
        if n > max {
            return Err(ReadError::invalid(
                format!("{what} is {n}, more than the limit of {max}"),
                at,
            ));
        }
        if n.saturating_mul(min_item_size) > self.remaining() {
            return Err(ReadError::invalid(
                format!("{what} is {n}, more than the rest of the file can hold"),
                at,
            ));
        }
        Ok(n)
    }

    /// A string whose length is an `i32`, followed by that many bytes.
    pub(crate) fn int_size_string(&mut self) -> Result<String> {
        let at = self.pos;
        let len = self.i32()?;
        let len = usize::try_from(len)
            .map_err(|_| ReadError::invalid(format!("negative text length ({len})"), at))?;
        Ok(decode_text(self.bytes(len)?))
    }

    /// A string stored in a fixed-size field: a length byte, then `size` bytes
    /// of which the first `length` are the text.
    pub(crate) fn byte_size_string(&mut self, size: usize) -> Result<String> {
        let len = usize::from(self.u8()?);
        let field = self.bytes(size)?;
        Ok(decode_text(field.get(..len).unwrap_or(field)))
    }

    /// Guitar Pro's usual string: an `i32` with the field size (length + 1),
    /// then a length byte, then the text.
    pub(crate) fn int_byte_size_string(&mut self) -> Result<String> {
        let at = self.pos;
        let size = self.i32()?;
        let size = usize::try_from(size)
            .map_err(|_| ReadError::invalid(format!("negative text size ({size})"), at))?;
        if size == 0 {
            return Ok(String::new());
        }
        let len = usize::from(self.u8()?);
        let field = self.bytes(size - 1)?;
        Ok(decode_text(field.get(..len).unwrap_or(field)))
    }
}

/// Decodes text from an older binary format. Guitar Pro 3–5 wrote the
/// system code page, Windows-1252 for western languages; some newer tools
/// write UTF-8. Valid UTF-8 is taken as such, anything else as Windows-1252.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    let bytes = match bytes.iter().position(|&b| b == 0) {
        Some(end) => bytes.get(..end).unwrap_or(bytes),
        None => bytes,
    };
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    bytes.iter().map(|&b| cp1252(b)).collect()
}

fn cp1252(b: u8) -> char {
    // 0x80..=0x9F differ from Latin-1; everything else maps directly.
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH.get(usize::from(b - 0x80)).copied().unwrap_or('?'),
        _ => char::from(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian() {
        let data = [1, 0xff, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
        let mut c = Cursor::new(&data);
        assert_eq!(c.u8().unwrap(), 1);
        assert_eq!(c.i8().unwrap(), -1);
        assert_eq!(c.i16().unwrap(), 0x1234);
        assert_eq!(c.i32().unwrap(), 0x1234_5678);
        assert_eq!(c.remaining(), 0);
        let e = c.u8().unwrap_err();
        assert_eq!(e.kind, ReadErrorKind::UnexpectedEnd);
        assert_eq!(e.offset, Some(8));
    }

    #[test]
    fn guitar_pro_strings() {
        // int size 4, length byte 3, "abc"
        let data = [4, 0, 0, 0, 3, b'a', b'b', b'c'];
        assert_eq!(Cursor::new(&data).int_byte_size_string().unwrap(), "abc");
        // fixed field of 5 holding "hi"
        let data = [2, b'h', b'i', 0, 0, 0];
        let mut c = Cursor::new(&data);
        assert_eq!(c.byte_size_string(5).unwrap(), "hi");
        assert_eq!(c.remaining(), 0);
    }

    #[test]
    fn counts_are_bounded() {
        let huge = 1_000_000_i32.to_le_bytes();
        assert!(Cursor::new(&huge).count("bars", 10, 1).is_err());
        let neg = (-1_i32).to_le_bytes();
        assert!(Cursor::new(&neg).count("bars", 10, 1).is_err());
        // 3 items of 4 bytes need 12 bytes; only 4 follow.
        let data = [3, 0, 0, 0, 0, 0, 0, 0];
        assert!(Cursor::new(&data).count("items", 10, 4).is_err());
    }

    #[test]
    fn windows_1252_text() {
        assert_eq!(decode_text(b"Ca\xe7ador"), "Caçador");
        assert_eq!(decode_text(b"\x93hi\x94"), "“hi”");
        assert_eq!(decode_text("Itapuã".as_bytes()), "Itapuã");
        assert_eq!(decode_text(b"abc\0junk"), "abc");
    }
}
