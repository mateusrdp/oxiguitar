// SPDX-License-Identifier: GPL-3.0-or-later
//! SF3 → SF2 in memory: decode each Ogg Vorbis sample (lewton) and rewrite
//! the sample headers, so an SF2-only synthesiser plays an SF3 file.
//!
//! SF3 (MuseScore's format) is SF2 with `ifil` 3.x, each compressed sample
//! an Ogg Vorbis stream inside `smpl`, its header's start and end byte
//! offsets of that stream, sample type bit 0x10 set, and its loop points
//! relative to the decoded sample (FluidSynth's fluid_defsfont.c).

use std::io::Cursor;

#[derive(Debug)]
pub struct Converted {
    pub sf2: Vec<u8>,
    pub samples: usize,
    pub compressed: usize,
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// The chunks of a RIFF list body: (id, data).
fn chunks(mut body: &[u8]) -> Result<Vec<([u8; 4], &[u8])>, String> {
    let mut out = Vec::new();
    while body.len() >= 8 {
        let id: [u8; 4] = body[..4].try_into().unwrap();
        let size = u32le(body, 4) as usize;
        let data = body.get(8..8 + size).ok_or("chunk past the end")?;
        out.push((id, data));
        // RIFF pads odd chunks to an even size, but some SF3 writers
        // (MuseScore's) don't: take the pad byte only if a chunk id follows it.
        let plausible = |at: usize| {
            body.get(at..at + 8).is_some_and(|h| {
                h[..4].iter().all(|c| c.is_ascii_alphanumeric() || *c == b' ')
                    && at + 8 + u32le(h, 4) as usize <= body.len()
            })
        };
        let next = if size & 1 == 1 && !plausible(8 + size + 1) && plausible(8 + size) { 8 + size } else { 8 + size + (size & 1) };
        body = body.get(next..).unwrap_or(&[]);
    }
    Ok(out)
}

fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(data.len() + 9);
    v.extend_from_slice(id);
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(data);
    if data.len() & 1 == 1 {
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

fn decode_vorbis(data: &[u8]) -> Result<Vec<i16>, String> {
    let mut reader =
        lewton::inside_ogg::OggStreamReader::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    let channels = reader.ident_hdr.audio_channels as usize;
    let mut out = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|e| e.to_string())?
    {
        // Mono in SoundFonts; keep the first channel if not.
        out.extend(packet.iter().step_by(channels.max(1)));
    }
    Ok(out)
}

/// Converts an SF3 file to SF2 bytes. An SF2 file comes back unchanged.
pub fn to_sf2(file: &[u8]) -> Result<Converted, String> {
    if file.len() < 12 || &file[..4] != b"RIFF" || &file[8..12] != b"sfbk" {
        return Err("not a SoundFont".into());
    }
    let top = chunks(&file[12..])?;
    let mut info = None;
    let mut sdta = None;
    let mut pdta = None;
    for (id, data) in &top {
        if id == b"LIST" && data.len() >= 4 {
            match &data[..4] {
                b"INFO" => info = Some(&data[4..]),
                b"sdta" => sdta = Some(&data[4..]),
                b"pdta" => pdta = Some(&data[4..]),
                _ => {}
            }
        }
    }
    let (info, sdta, pdta) = (
        info.ok_or("no INFO")?,
        sdta.ok_or("no sdta")?,
        pdta.ok_or("no pdta")?,
    );
    let smpl = chunks(sdta)?
        .into_iter()
        .find(|(id, _)| id == b"smpl")
        .map(|(_, d)| d)
        .ok_or("no smpl")?;
    let pdta_chunks = chunks(pdta)?;
    let shdr = pdta_chunks
        .iter()
        .find(|(id, _)| id == b"shdr")
        .map(|(_, d)| *d)
        .ok_or("no shdr")?;
    if !smpl.starts_with(b"OggS") && !shdr.chunks(46).any(|h| h.len() == 46 && u16::from_le_bytes([h[44], h[45]]) & 0x10 != 0) {
        return Ok(Converted { sf2: file.to_vec(), samples: shdr.len() / 46, compressed: 0 });
    }

    let mut wave: Vec<i16> = Vec::new();
    let mut new_shdr = Vec::with_capacity(shdr.len());
    let mut compressed = 0;
    let count = shdr.len() / 46;
    for (i, h) in shdr.chunks(46).enumerate() {
        let mut h = h.to_vec();
        if i + 1 == count {
            new_shdr.extend_from_slice(&h); // the terminal "EOS" record
            break;
        }
        let start = u32le(&h, 20) as usize;
        let end = u32le(&h, 24) as usize;
        let loop_start = u32le(&h, 28);
        let loop_end = u32le(&h, 32);
        let kind = u16::from_le_bytes([h[44], h[45]]);
        let pcm: Vec<i16> = if kind & 0x10 != 0 {
            compressed += 1;
            let bytes = smpl.get(start..end.min(smpl.len())).ok_or("sample past smpl")?; // end is exclusive
            decode_vorbis(bytes)?
        } else {
            let w: Vec<i16> = smpl
                .chunks_exact(2)
                .skip(start)
                .take(end.saturating_sub(start))
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            w
        };
        let at = wave.len() as u32;
        let (ls, le) = if kind & 0x10 != 0 {
            (at + loop_start, at + loop_end)
        } else {
            (at + loop_start - start as u32, at + loop_end - start as u32)
        };
        wave.extend_from_slice(&pcm);
        let new_end = wave.len() as u32;
        wave.extend(std::iter::repeat_n(0i16, 46)); // SF2: 46 zero points after each sample
        h[20..24].copy_from_slice(&at.to_le_bytes());
        h[24..28].copy_from_slice(&new_end.to_le_bytes());
        h[28..32].copy_from_slice(&ls.to_le_bytes());
        h[32..36].copy_from_slice(&le.to_le_bytes());
        h[44..46].copy_from_slice(&(kind & !0x10).to_le_bytes());
        new_shdr.extend_from_slice(&h);
    }

    // INFO with ifil 2.1.
    let mut info_parts = Vec::new();
    for (id, data) in chunks(info)? {
        if &id == b"ifil" {
            info_parts.push(chunk(b"ifil", &[2, 0, 1, 0]));
        } else {
            info_parts.push(chunk(&id, data));
        }
    }
    let smpl_bytes: Vec<u8> = wave.iter().flat_map(|s| s.to_le_bytes()).collect();
    let pdta_parts: Vec<Vec<u8>> = pdta_chunks
        .iter()
        .map(|(id, d)| if id == b"shdr" { chunk(id, &new_shdr) } else { chunk(id, d) })
        .collect();
    let mut body = b"sfbk".to_vec();
    body.extend(list(b"INFO", &info_parts));
    body.extend(list(b"sdta", &[chunk(b"smpl", &smpl_bytes)]));
    body.extend(list(b"pdta", &pdta_parts));
    Ok(Converted { sf2: chunk(b"RIFF", &body), samples: count - 1, compressed })
}
