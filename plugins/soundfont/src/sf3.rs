// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! SoundFont 3 to SoundFont 2, in memory, for rustysynth (which reads only
//! SoundFont 2): each Ogg Vorbis sample decoded with lewton and the sample
//! headers rewritten; everything else is kept as it is.
//!
//! SoundFont 3 (MuseScore's) is SoundFont 2 with `ifil` 3.x, each
//! compressed sample an Ogg Vorbis stream inside `smpl`, its header's start
//! and end the stream's byte offsets, sample type bit 0x10 set, and its
//! loop points counted from the decoded sample's start (as FluidSynth reads
//! it, `fluid_defsfont.c`). Samples are decoded on every core at once.

use std::io::Cursor;

use oxiguitar_plugin::Error;

use crate::riff::{self, Chunk, invalid, u16_at, u32_at};

/// A sample header's size in `shdr`.
const HEADER: usize = 46;

/// The sample type's bit for an Ogg Vorbis sample.
const VORBIS: u16 = 0x10;

/// The silent frames SoundFont 2 asks for after each sample.
const PAD: usize = 46;

/// Whether `head`, a SoundFont's first bytes (or all of it), says its
/// samples are compressed (SF3): its version, `ifil` in the `INFO` list
/// that starts it, is 3 or more. Only as much as that is read.
pub(crate) fn is_sf3(head: &[u8]) -> bool {
    if head.len() < 24
        || &head[..4] != b"RIFF"
        || &head[8..12] != b"sfbk"
        || &head[12..16] != b"LIST"
        || &head[20..24] != b"INFO"
    {
        return false;
    }
    let mut at = 24;
    while let (Some(id), Some(size)) = (head.get(at..at + 4), u32_at(head, at + 4)) {
        if id == b"ifil" {
            return u16_at(head, at + 8).is_some_and(|major| major >= 3);
        }
        at += 8 + size as usize + (size as usize % 2);
    }
    false
}

fn decode(data: &[u8]) -> Result<Vec<i16>, String> {
    let mut reader =
        lewton::inside_ogg::OggStreamReader::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    let channels = usize::from(reader.ident_hdr.audio_channels.max(1));
    let mut out = Vec::new();
    while let Some(packet) = reader.read_dec_packet_itl().map_err(|e| e.to_string())? {
        // SoundFont samples are mono; of more channels the first is kept.
        out.extend(packet.iter().step_by(channels));
    }
    Ok(out)
}

/// A sample, decoded or copied: its frames.
type Frames = Result<Vec<i16>, String>;

/// The frames of every sample header in `shdr` (the terminal one left
/// out), decoding compressed ones on every core.
fn samples(shdr: &[u8], smpl: &[u8]) -> Result<Vec<Vec<i16>>, Error> {
    let headers: Vec<&[u8]> = shdr
        .as_chunks::<HEADER>()
        .0
        .iter()
        .map(|h| &h[..])
        .collect();
    let count = headers.len().saturating_sub(1);
    let headers = &headers[..count];
    let one = |h: &[u8]| -> Frames {
        let start = u32_at(h, 20).unwrap_or(0) as usize;
        let end = u32_at(h, 24).unwrap_or(0) as usize;
        let kind = u16_at(h, 44).unwrap_or(0);
        if kind & VORBIS != 0 {
            let bytes = smpl
                .get(start..end.min(smpl.len()))
                .ok_or("a sample past the sample data")?;
            decode(bytes)
        } else {
            Ok(smpl
                .as_chunks::<2>()
                .0
                .iter()
                .skip(start)
                .take(end.saturating_sub(start))
                .map(|b| i16::from_le_bytes(*b))
                .collect())
        }
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, 16);
    let per = count.div_ceil(threads).max(1);
    let decoded: Vec<Frames> = std::thread::scope(|scope| {
        let workers: Vec<_> = headers
            .chunks(per)
            .map(|part| scope.spawn(move || part.iter().map(|h| one(h)).collect::<Vec<_>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| {
                w.join()
                    .unwrap_or_else(|_| vec![Err("decoding stopped".into())])
            })
            .collect()
    });
    decoded
        .into_iter()
        .enumerate()
        .map(|(i, f)| f.map_err(|e| invalid(format!("sample {}: {e}", i + 1))))
        .collect()
}

fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(data.len() + 9);
    v.extend_from_slice(id);
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(data);
    if data.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn list(kind: &[u8; 4], parts: &[Vec<u8>]) -> Vec<u8> {
    let mut body = kind.to_vec();
    for p in parts {
        body.extend_from_slice(p);
    }
    chunk(b"LIST", &body)
}

/// The SoundFont 2 file `file` (a SoundFont 3) becomes: its samples
/// decoded, its headers pointing at them, `ifil` 2.4.
pub(crate) fn to_sf2(file: &[u8]) -> Result<Vec<u8>, Error> {
    let (kind, top) = riff::form(file)?;
    if kind != *b"sfbk" {
        return Err(invalid("it isn't a SoundFont"));
    }
    let info = riff::find_list(&top, b"INFO").ok_or_else(|| invalid("it has no INFO list"))?;
    let sdta = riff::find_list(&top, b"sdta").ok_or_else(|| invalid("it has no sample data"))?;
    let pdta = riff::find_list(&top, b"pdta").ok_or_else(|| invalid("it has no pdta list"))?;
    let smpl =
        riff::find(&riff::chunks(sdta)?, b"smpl").ok_or_else(|| invalid("it has no smpl chunk"))?;
    let pdta: Vec<Chunk<'_>> = riff::chunks(pdta)?;
    let shdr = riff::find(&pdta, b"shdr").ok_or_else(|| invalid("it has no sample headers"))?;
    let decoded = samples(shdr, smpl)?;

    let total: usize = decoded.iter().map(|s| s.len() + PAD).sum();
    let mut wave: Vec<u8> = Vec::with_capacity(total * 2);
    let mut headers = Vec::with_capacity(shdr.len());
    for (h, frames) in shdr.as_chunks::<HEADER>().0.iter().zip(&decoded) {
        let mut h = h.to_vec();
        let start = u32_at(&h, 20).unwrap_or(0);
        let loop_start = u32_at(&h, 28).unwrap_or(0);
        let loop_end = u32_at(&h, 32).unwrap_or(0);
        let kind = u16_at(&h, 44).unwrap_or(0);
        let at = (wave.len() / 2) as u32;
        // A compressed sample's loop counts from its own start; an
        // uncompressed one's from the start of the data.
        let (ls, le) = if kind & VORBIS != 0 {
            (loop_start, loop_end)
        } else {
            (
                loop_start.saturating_sub(start),
                loop_end.saturating_sub(start),
            )
        };
        for v in frames {
            wave.extend_from_slice(&v.to_le_bytes());
        }
        let end = (wave.len() / 2) as u32;
        wave.resize(wave.len() + PAD * 2, 0);
        h[20..24].copy_from_slice(&at.to_le_bytes());
        h[24..28].copy_from_slice(&end.to_le_bytes());
        h[28..32].copy_from_slice(&(at + ls).min(end).to_le_bytes());
        h[32..36].copy_from_slice(&(at + le).min(end).to_le_bytes());
        h[44..46].copy_from_slice(&(kind & !VORBIS).to_le_bytes());
        headers.extend_from_slice(&h);
    }
    // The terminal record, as it was.
    if let Some(last) = shdr.as_chunks::<HEADER>().0.last() {
        headers.extend_from_slice(last);
    }

    let mut info_parts = Vec::new();
    for c in riff::chunks(info)? {
        if &c.id == b"ifil" {
            info_parts.push(chunk(b"ifil", &[2, 0, 4, 0]));
        } else {
            info_parts.push(chunk(&c.id, c.data));
        }
    }
    let pdta_parts: Vec<Vec<u8>> = pdta
        .iter()
        .map(|c| {
            if &c.id == b"shdr" {
                chunk(&c.id, &headers)
            } else {
                chunk(&c.id, c.data)
            }
        })
        .collect();
    let mut body = b"sfbk".to_vec();
    body.extend(list(b"INFO", &info_parts));
    body.extend(list(b"sdta", &[chunk(b"smpl", &wave)]));
    body.extend(list(b"pdta", &pdta_parts));
    Ok(chunk(b"RIFF", &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soundfont_2_is_no_soundfont_3() {
        assert!(!is_sf3(&crate::sf2::test_set()));
        assert!(!is_sf3(b"RIFF\x04\0\0\0WAVE"));
        assert!(!is_sf3(b""));
    }

    #[test]
    fn an_uncompressed_set_comes_back_alike() {
        // Its samples copied, its loops kept, rustysynth reads it.
        let sf2 = crate::sf2::test_set();
        let again = to_sf2(&sf2).unwrap();
        let a = rustysynth::SoundFont::new(&mut Cursor::new(&sf2)).unwrap();
        let b = rustysynth::SoundFont::new(&mut Cursor::new(&again)).unwrap();
        assert_eq!(a.get_wave_data(), b.get_wave_data());
        let (x, y) = (&a.get_sample_headers()[0], &b.get_sample_headers()[0]);
        assert_eq!(
            (
                x.get_start(),
                x.get_end(),
                x.get_start_loop(),
                x.get_end_loop()
            ),
            (
                y.get_start(),
                y.get_end(),
                y.get_start_loop(),
                y.get_end_loop()
            )
        );
    }

    #[test]
    fn a_damaged_file_is_an_error_not_a_panic() {
        let sf2 = crate::sf2::test_set();
        for cut in [0, 7, 12, 40, 100, sf2.len() / 2, sf2.len() - 3] {
            let _ = to_sf2(&sf2[..cut]);
        }
        let mut bad = sf2.clone();
        bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let _ = to_sf2(&bad);
    }

    /// The system's MuseScore General, when there is one (Debian's
    /// musescore-general-soundfont-small): every sample decodes and
    /// rustysynth reads the result.
    #[test]
    fn musescore_general_decodes_when_the_system_has_it() {
        let path = std::path::Path::new("/usr/share/sounds/sf3/MuseScore_General_Lite.sf3");
        let Ok(file) = std::fs::read(path) else {
            return;
        };
        assert!(is_sf3(&file));
        let sf2 = to_sf2(&file).unwrap();
        let font = rustysynth::SoundFont::new(&mut Cursor::new(&sf2)).unwrap();
        assert!(font.get_presets().len() > 128);
        assert!(font.get_wave_data().len() > 50_000_000);
    }
}
