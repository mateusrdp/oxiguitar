// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The containers around GPIF: Guitar Pro 6's BCFZ/BCFS file system (.gpx)
//! and Guitar Pro 7/8's ZIP archive (.gp).
//!
//! The BCFZ compression and BCFS layout follow what alphaTab (MPL-2.0)
//! documents in its `GpxFileSystem`; no code is copied. See
//! `docs/formats/guitar-pro-6-8.md`.

#[cfg(feature = "gp")]
use std::io::Read as _;

use crate::error::{ReadError, ReadErrorKind};

/// The largest file we accept from inside a container. The largest GPIF in
/// our corpus is 10 MB; anything near this limit is damage or a zip bomb.
const MAX_INNER_FILE: usize = 128 * 1024 * 1024;
#[cfg(feature = "gpx")]
/// BCFS stores files in sectors of this size.
const SECTOR: usize = 0x1000;

/// The parts of a Guitar Pro 6–8 file the reader uses.
#[derive(Debug, Default)]
pub(crate) struct GpFiles {
    /// `score.gpif`: the score as XML.
    pub(crate) score: Vec<u8>,
    /// `PartConfiguration`: which notation each track shows, if present.
    pub(crate) part_configuration: Option<Vec<u8>>,
}

#[cfg(feature = "gpx")]
/// Unpacks a Guitar Pro 6 file: BCFZ (compressed) or BCFS (plain).
pub(crate) fn gpx(data: &[u8]) -> Result<GpFiles, ReadError> {
    let fs = if data.starts_with(b"BCFZ") {
        bcfz_decompress(data)?
    } else {
        data.to_vec()
    };
    let files = bcfs_files(&fs)?;
    let mut out = GpFiles::default();
    for (name, contents) in files {
        match name.as_str() {
            "score.gpif" => out.score = contents,
            "PartConfiguration" => out.part_configuration = Some(contents),
            _ => {}
        }
    }
    if out.score.is_empty() {
        return Err(ReadError::invalid(
            "the Guitar Pro 6 file holds no score.gpif",
            0,
        ));
    }
    Ok(out)
}

#[cfg(feature = "gpx")]
/// Reads bits from the most significant bit of each byte down.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

#[cfg(feature = "gpx")]
impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.pos / 8)?;
        let b = (byte >> (7 - (self.pos % 8))) & 1;
        self.pos += 1;
        Some(u32::from(b))
    }

    /// `n` bits, first bit read is the most significant.
    fn bits(&mut self, n: u32) -> Option<u32> {
        (0..n).try_fold(0, |v, _| Some((v << 1) | self.bit()?))
    }

    /// `n` bits, first bit read is the least significant.
    fn bits_reversed(&mut self, n: u32) -> Option<u32> {
        (0..n).try_fold(0, |v, i| Some(v | (self.bit()? << i)))
    }
}

#[cfg(feature = "gpx")]
/// Decompresses BCFZ: a 4-byte tag, the uncompressed length (little-endian),
/// then a bit stream of literal runs and back-references.
pub(crate) fn bcfz_decompress(data: &[u8]) -> Result<Vec<u8>, ReadError> {
    let len_bytes: [u8; 4] = data
        .get(4..8)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| ReadError::new(ReadErrorKind::UnexpectedEnd, Some(data.len())))?;
    let expected = i32::from_le_bytes(len_bytes);
    let expected = usize::try_from(expected)
        .ok()
        .filter(|&n| n <= MAX_INNER_FILE)
        .ok_or_else(|| ReadError::invalid(format!("compressed size {expected} out of range"), 4))?;
    let mut src = Bits {
        data: data.get(8..).unwrap_or_default(),
        pos: 0,
    };
    let mut out: Vec<u8> = Vec::with_capacity(expected);
    // The stream may stop a few bytes short of the promised length (one file
    // in our corpus does); what was decompressed is still usable.
    while out.len() < expected {
        let Some(flag) = src.bit() else { break };
        if flag == 1 {
            let (Some(word), at) = (src.bits(4), src.pos / 8 + 8) else {
                break;
            };
            let (Some(offset), Some(size)) = (src.bits_reversed(word), src.bits_reversed(word))
            else {
                break;
            };
            let offset = offset as usize;
            if offset == 0 || offset > out.len() {
                return Err(ReadError::invalid(
                    "BCFZ back-reference before the start of the data",
                    at,
                ));
            }
            let start = out.len() - offset;
            let n = offset.min(size as usize);
            out.extend_from_within(start..start + n);
        } else {
            let Some(size) = src.bits_reversed(2) else {
                break;
            };
            for _ in 0..size {
                let Some(b) = src.bits(8) else { break };
                out.push(b as u8);
            }
        }
    }
    out.truncate(expected);
    Ok(out)
}

#[cfg(feature = "gpx")]
fn le_u32_at(data: &[u8], at: usize) -> Option<usize> {
    let b: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
    usize::try_from(u32::from_le_bytes(b)).ok()
}

#[cfg(feature = "gpx")]
/// Lists the files of a BCFS file system: "BCFS", then 4 KiB sectors. A sector
/// that starts with the number 2 describes a file: its name, its size and the
/// list of sectors holding its contents.
pub(crate) fn bcfs_files(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ReadError> {
    let body = data
        .strip_prefix(b"BCFS")
        .ok_or_else(|| ReadError::invalid("not a BCFS file system", 0))?;
    let sectors = body.len() / SECTOR + 1;
    let mut files = Vec::new();
    let mut entry = 0usize;
    while entry + 4 <= body.len() {
        if le_u32_at(body, entry) == Some(2) {
            let name_bytes = body
                .get(entry + 4..(entry + 4 + 127).min(body.len()))
                .unwrap_or_default();
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(name_bytes.len());
            let name = String::from_utf8_lossy(name_bytes.get(..name_end).unwrap_or_default())
                .into_owned();
            let size = le_u32_at(body, entry + 0x8C).unwrap_or(0);
            if size > body.len() {
                return Err(ReadError::invalid(
                    format!("BCFS file \"{name}\" claims {size} bytes"),
                    entry + 0x8C + 4,
                ));
            }
            let mut contents = Vec::with_capacity(size);
            let mut p = entry + 0x94;
            // A file's sector list ends with 0 and cannot list more sectors
            // than the file system has.
            for _ in 0..sectors {
                let Some(s) = le_u32_at(body, p) else { break };
                if s == 0 || contents.len() >= size {
                    break;
                }
                let start = s.saturating_mul(SECTOR);
                let chunk = body
                    .get(start..start.saturating_add(SECTOR).min(body.len()))
                    .ok_or_else(|| {
                        ReadError::invalid(format!("BCFS sector {s} is outside the file"), p + 4)
                    })?;
                contents.extend_from_slice(chunk);
                p += 4;
            }
            contents.truncate(size);
            files.push((name, contents));
        }
        entry += SECTOR;
    }
    Ok(files)
}

#[cfg(feature = "gp")]
/// Unpacks a Guitar Pro 7/8 file: a ZIP archive holding `Content/score.gpif`.
pub(crate) fn gp7(data: &[u8]) -> Result<GpFiles, ReadError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| {
        ReadError::new(
            ReadErrorKind::Invalid(format!("damaged ZIP archive: {e}")),
            None,
        )
    })?;
    let score = read_zip_entry(&mut zip, "Content/score.gpif")?
        .ok_or_else(|| ReadError::invalid("the archive holds no Content/score.gpif", 0))?;
    let part_configuration = read_zip_entry(&mut zip, "Content/PartConfiguration")?;
    Ok(GpFiles {
        score,
        part_configuration,
    })
}

#[cfg(feature = "gp")]
fn read_zip_entry(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<Vec<u8>>, ReadError> {
    let mut file = match zip.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => {
            return Err(ReadError::new(
                ReadErrorKind::Invalid(format!("cannot open {name} in the archive: {e}")),
                None,
            ));
        }
    };
    let mut out = Vec::new();
    // Read one byte more than the limit, so a lying size header cannot make us
    // inflate without end.
    (&mut file)
        .take(MAX_INNER_FILE as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| {
            ReadError::new(
                ReadErrorKind::Invalid(format!("cannot decompress {name}: {e}")),
                None,
            )
        })?;
    if out.len() > MAX_INNER_FILE {
        return Err(ReadError::invalid(format!("{name} is too large"), 0));
    }
    Ok(Some(out))
}

/// Which notation each track shows, from the `PartConfiguration` file.
///
/// The file is big-endian: a count of views, then per view a "multi-rest"
/// byte, a count of tracks and one flag byte per track (1 standard notation,
/// 2 tablature, 4 slash, 8 numbered). The first view is the full score.
pub(crate) fn part_configuration(data: &[u8]) -> Option<Vec<u8>> {
    let be = |at: usize| -> Option<usize> {
        let b: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
        usize::try_from(u32::from_be_bytes(b)).ok()
    };
    let views = be(0)?;
    if views == 0 {
        return None;
    }
    let tracks = be(5)?;
    let flags = data.get(9..9usize.checked_add(tracks)?)?;
    Some(flags.to_vec())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    /// Writes bits most significant first.
    #[derive(Default)]
    struct BitWriter {
        out: Vec<u8>,
        n: usize,
    }

    impl BitWriter {
        fn bit(&mut self, b: u32) {
            if self.n.is_multiple_of(8) {
                self.out.push(0);
            }
            if b != 0 {
                *self.out.last_mut().unwrap() |= 1 << (7 - self.n % 8);
            }
            self.n += 1;
        }
        fn bits(&mut self, v: u32, n: u32) {
            for i in (0..n).rev() {
                self.bit((v >> i) & 1);
            }
        }
        fn bits_reversed(&mut self, v: u32, n: u32) {
            for i in 0..n {
                self.bit((v >> i) & 1);
            }
        }
    }

    #[cfg(feature = "gpx")]
    #[test]
    fn bcfz_literals_and_back_references() {
        let mut w = BitWriter::default();
        // Literal "abc", then copy 3 bytes from 3 back: "abcabc".
        w.bit(0);
        w.bits_reversed(3, 2);
        for b in b"abc" {
            w.bits(u32::from(*b), 8);
        }
        w.bit(1);
        w.bits(4, 4);
        w.bits_reversed(3, 4);
        w.bits_reversed(3, 4);
        let mut data = b"BCFZ".to_vec();
        data.extend_from_slice(&6i32.to_le_bytes());
        data.extend_from_slice(&w.out);
        assert_eq!(bcfz_decompress(&data).unwrap(), b"abcabc");

        // A reference past the start is an error, not a panic.
        let mut w = BitWriter::default();
        w.bit(1);
        w.bits(4, 4);
        w.bits_reversed(9, 4);
        w.bits_reversed(3, 4);
        let mut bad = b"BCFZ".to_vec();
        bad.extend_from_slice(&6i32.to_le_bytes());
        bad.extend_from_slice(&w.out);
        assert!(bcfz_decompress(&bad).is_err());
        // Absurd sizes are refused before allocating.
        let mut huge = b"BCFZ".to_vec();
        huge.extend_from_slice(&i32::MAX.to_le_bytes());
        assert!(bcfz_decompress(&huge).is_err());
        assert!(bcfz_decompress(b"BCFZ\x01").is_err());
    }

    #[cfg(feature = "gpx")]
    #[test]
    fn bcfs_file_system() {
        let mut fs = vec![0u8; 3 * SECTOR];
        // Sector 1: the entry for "score.gpif", 5 bytes in sector 2.
        let e = SECTOR;
        fs[e..e + 4].copy_from_slice(&2u32.to_le_bytes());
        fs[e + 4..e + 14].copy_from_slice(b"score.gpif");
        fs[e + 0x8C..e + 0x90].copy_from_slice(&5u32.to_le_bytes());
        fs[e + 0x94..e + 0x98].copy_from_slice(&2u32.to_le_bytes());
        fs[2 * SECTOR..2 * SECTOR + 5].copy_from_slice(b"<a/>\n");
        let mut data = b"BCFS".to_vec();
        data.extend_from_slice(&fs);
        let files = bcfs_files(&data).unwrap();
        assert_eq!(files, vec![("score.gpif".to_owned(), b"<a/>\n".to_vec())]);
        assert_eq!(gpx(&data).unwrap().score, b"<a/>\n");

        // A sector index past the end is an error.
        fs[e + 0x94..e + 0x98].copy_from_slice(&900u32.to_le_bytes());
        let mut data = b"BCFS".to_vec();
        data.extend_from_slice(&fs);
        assert!(bcfs_files(&data).is_err());
    }

    #[test]
    fn part_configuration_flags() {
        let data = [0, 0, 0, 2, 0, 0, 0, 0, 2, 3, 2, 0, 0, 0, 0, 1, 3];
        assert_eq!(part_configuration(&data), Some(vec![3, 2]));
        assert_eq!(part_configuration(&[0, 0, 0, 1, 0, 0, 0, 0, 9]), None);
        assert_eq!(part_configuration(&[]), None);
    }
}
