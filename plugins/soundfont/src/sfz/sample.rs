// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! SFZ instruments' samples, read whole into memory before anything plays
//! (Mateus, 26 Oct 2026: whole, in memory, at full quality), each at its
//! own bit depth: 16-bit samples as 16 bits, 24-bit ones as 24 (three
//! bytes), anything finer as 32-bit floats. So nothing is lost and memory
//! is what the files hold uncompressed.
//!
//! - WAV: our own reading (it copies the data as it is, and reads the
//!   `smpl` chunk's loop and unity note); 8-, 16-, 24- and 32-bit PCM and
//!   32- and 64-bit floats, plain or `WAVE_FORMAT_EXTENSIBLE`.
//! - FLAC: claxon (Apache-2.0), as published.
//! - Ogg Vorbis: lewton (MIT OR Apache-2.0), as published, decoded to
//!   floats.
//!
//! A sample with more than two channels keeps its first two.

use std::path::Path;

/// Sample data, interleaved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pcm {
    I16(Box<[i16]>),
    /// Three little-endian bytes a value.
    I24(Box<[u8]>),
    F32(Box<[f32]>),
}

/// A sample, in memory.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sample {
    /// Frames a second.
    pub(crate) rate: u32,
    /// 1 or 2.
    pub(crate) channels: usize,
    pub(crate) frames: usize,
    pub(crate) pcm: Pcm,
    /// The file's own loop (WAV's `smpl` chunk): its first and last frame.
    pub(crate) looped: Option<(u32, u32)>,
    /// The key the sample sounds at unchanged, from the file.
    pub(crate) unity: Option<u8>,
}

const I16_SCALE: f32 = 1.0 / 32_768.0;
const I24_SCALE: f32 = 1.0 / 8_388_608.0;

impl Sample {
    /// The value of `channel` (0 or 1; a mono sample has the one) at
    /// `frame`, from −1 to 1. Frames past the end are silent.
    #[inline]
    pub(crate) fn at(&self, frame: usize, channel: usize) -> f32 {
        if frame >= self.frames {
            return 0.0;
        }
        let i = frame * self.channels + channel.min(self.channels - 1);
        match &self.pcm {
            Pcm::I16(d) => f32::from(d[i]) * I16_SCALE,
            Pcm::I24(d) => {
                let b = &d[i * 3..i * 3 + 3];
                let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                v as f32 * I24_SCALE
            }
            Pcm::F32(d) => d[i],
        }
    }

    /// The memory it takes.
    pub(crate) fn bytes(&self) -> usize {
        match &self.pcm {
            Pcm::I16(d) => d.len() * 2,
            Pcm::I24(d) => d.len(),
            Pcm::F32(d) => d.len() * 4,
        }
    }
}

/// Reads the sample at `path`, whatever its kind: what its first bytes say.
pub(crate) fn read(path: &Path) -> Result<Sample, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Decodes a WAV, FLAC or Ogg Vorbis file's bytes.
pub(crate) fn decode(bytes: &[u8]) -> Result<Sample, String> {
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        wav(bytes)
    } else if bytes.starts_with(b"fLaC") {
        flac(bytes)
    } else if bytes.starts_with(b"OggS") {
        ogg(bytes)
    } else {
        Err("not a WAV, FLAC or Ogg Vorbis file".into())
    }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// A WAV file's chunks, leniently: a chunk longer than the file is taken
/// to its end (some writers leave a streaming size there).
fn wav_chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let mut id = [0; 4];
        id.copy_from_slice(&bytes[at..at + 4]);
        let size = u32_at(bytes, at + 4).unwrap_or(0) as usize;
        let start = at + 8;
        let end = start.saturating_add(size).min(bytes.len());
        out.push((id, &bytes[start..end]));
        at = start.saturating_add(size).saturating_add(size % 2);
    }
    out
}

fn wav(bytes: &[u8]) -> Result<Sample, String> {
    let chunks = wav_chunks(bytes);
    let find = |id: &[u8; 4]| chunks.iter().find(|(c, _)| c == id).map(|(_, d)| *d);
    let fmt = find(b"fmt ").ok_or("no fmt chunk")?;
    let data = find(b"data").ok_or("no data chunk")?;
    let mut tag = u16_at(fmt, 0).ok_or("a short fmt chunk")?;
    let channels = usize::from(u16_at(fmt, 2).ok_or("a short fmt chunk")?);
    let rate = u32_at(fmt, 4).ok_or("a short fmt chunk")?;
    let bits = u16_at(fmt, 14).ok_or("a short fmt chunk")?;
    if tag == 0xFFFE {
        // WAVE_FORMAT_EXTENSIBLE: the real format is the subformat's first
        // two bytes.
        tag = u16_at(fmt, 24).ok_or("a short extensible fmt chunk")?;
    }
    if channels == 0 || rate == 0 {
        return Err("no channels, or no rate".into());
    }
    let width = usize::from(bits.div_ceil(8));
    let frame_bytes = width * channels;
    if frame_bytes == 0 {
        return Err(format!("{bits}-bit samples"));
    }
    let frames = data.len() / frame_bytes;
    let data = &data[..frames * frame_bytes];
    let kept = channels.min(2);
    // Keeps the first two channels of each frame.
    let each = |f: &mut dyn FnMut(&[u8])| {
        for frame in data.chunks_exact(frame_bytes) {
            for c in 0..kept {
                f(&frame[c * width..(c + 1) * width]);
            }
        }
    };
    let pcm = match (tag, bits) {
        (1, 16) if channels <= 2 => Pcm::I16(
            data.chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect(),
        ),
        (1, 16) => {
            let mut v = Vec::with_capacity(frames * kept);
            each(&mut |b| v.push(i16::from_le_bytes([b[0], b[1]])));
            Pcm::I16(v.into())
        }
        (1, 24) if channels <= 2 => Pcm::I24(data.into()),
        (1, 24) => {
            let mut v = Vec::with_capacity(frames * kept * 3);
            each(&mut |b| v.extend_from_slice(b));
            Pcm::I24(v.into())
        }
        (1, 8) => {
            let mut v = Vec::with_capacity(frames * kept);
            each(&mut |b| v.push((i16::from(b[0]) - 128) << 8));
            Pcm::I16(v.into())
        }
        (1, 32) => {
            let mut v = Vec::with_capacity(frames * kept);
            each(&mut |b| {
                v.push(i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0);
            });
            Pcm::F32(v.into())
        }
        (3, 32) => {
            let mut v = Vec::with_capacity(frames * kept);
            each(&mut |b| v.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
            Pcm::F32(v.into())
        }
        (3, 64) => {
            let mut v = Vec::with_capacity(frames * kept);
            each(&mut |b| {
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                v.push(f64::from_le_bytes(a) as f32);
            });
            Pcm::F32(v.into())
        }
        _ => {
            return Err(format!(
                "WAV format {tag} at {bits} bits isn't one it reads"
            ));
        }
    };
    let (looped, unity) = find(b"smpl").map_or((None, None), smpl);
    Ok(Sample {
        rate,
        channels: kept,
        frames,
        pcm,
        looped,
        unity,
    })
}

/// A `smpl` chunk's first loop and unity note.
fn smpl(d: &[u8]) -> (Option<(u32, u32)>, Option<u8>) {
    let unity = u32_at(d, 12)
        .filter(|&n| n <= 127)
        .and_then(|n| u8::try_from(n).ok());
    let loops = u32_at(d, 28).unwrap_or(0);
    let looped = if loops > 0 {
        match (u32_at(d, 36 + 8), u32_at(d, 36 + 12)) {
            (Some(start), Some(end)) if end > start => Some((start, end)),
            _ => None,
        }
    } else {
        None
    };
    (looped, unity)
}

fn flac(bytes: &[u8]) -> Result<Sample, String> {
    let mut reader =
        claxon::FlacReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let info = reader.streaminfo();
    let channels = info.channels as usize;
    let kept = channels.min(2);
    let bits = info.bits_per_sample;
    if channels == 0 || info.sample_rate == 0 {
        return Err("no channels, or no rate".into());
    }
    let expected = info.samples.unwrap_or(0) as usize;
    let mut i16s: Vec<i16> = Vec::new();
    let mut i24s: Vec<u8> = Vec::new();
    let mut f32s: Vec<f32> = Vec::new();
    match bits {
        ..=16 => i16s.reserve_exact(expected * kept),
        17..=24 => i24s.reserve_exact(expected * kept * 3),
        _ => f32s.reserve_exact(expected * kept),
    }
    let mut frames = 0usize;
    let mut blocks = reader.blocks();
    let mut buffer = Vec::new();
    loop {
        let block = match blocks.read_next_or_eof(buffer) {
            Ok(Some(block)) => block,
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        };
        let n = block.duration() as usize;
        for f in 0..n {
            for c in 0..kept {
                let v = block.sample(c as u32, f as u32);
                match bits {
                    ..=16 => i16s.push((v << (16 - bits)) as i16),
                    17..=24 => {
                        let v = v << (24 - bits);
                        i24s.extend_from_slice(&v.to_le_bytes()[..3]);
                    }
                    _ => f32s.push(v as f32 / 2_147_483_648.0),
                }
            }
        }
        frames += n;
        buffer = block.into_buffer();
    }
    let pcm = match bits {
        ..=16 => Pcm::I16(i16s.into()),
        17..=24 => Pcm::I24(i24s.into()),
        _ => Pcm::F32(f32s.into()),
    };
    Ok(Sample {
        rate: info.sample_rate,
        channels: kept,
        frames,
        pcm,
        looped: None,
        unity: None,
    })
}

fn ogg(bytes: &[u8]) -> Result<Sample, String> {
    use lewton::inside_ogg::OggStreamReader;
    use lewton::samples::InterleavedSamples;
    let mut reader =
        OggStreamReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let channels = usize::from(reader.ident_hdr.audio_channels);
    let rate = reader.ident_hdr.audio_sample_rate;
    if channels == 0 || rate == 0 {
        return Err("no channels, or no rate".into());
    }
    let kept = channels.min(2);
    let mut out: Vec<f32> = Vec::new();
    loop {
        match reader.read_dec_packet_generic::<InterleavedSamples<f32>>() {
            Ok(Some(packet)) => {
                for frame in packet.samples.chunks_exact(channels) {
                    out.extend_from_slice(&frame[..kept]);
                }
            }
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    let frames = out.len() / kept;
    Ok(Sample {
        rate,
        channels: kept,
        frames,
        pcm: Pcm::F32(out.into()),
        looped: None,
        unity: None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A WAV file: `channels` interleaved `values` at `bits` (16 or 24),
    /// with a loop and unity note if given.
    pub(crate) fn wav_file(
        rate: u32,
        channels: u16,
        bits: u16,
        values: &[i32],
        smpl: Option<((u32, u32), u8)>,
    ) -> Vec<u8> {
        let width = bits / 8;
        let mut data = Vec::new();
        for &v in values {
            data.extend_from_slice(&v.to_le_bytes()[..usize::from(width)]);
        }
        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * u32::from(channels * width)).to_le_bytes());
        fmt.extend_from_slice(&(channels * width).to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        for (id, chunk) in [(b"fmt ", fmt), (b"data", data)] {
            body.extend_from_slice(id);
            body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            body.extend_from_slice(&chunk);
            if chunk.len() % 2 == 1 {
                body.push(0);
            }
        }
        if let Some(((start, end), unity)) = smpl {
            let mut s = vec![0u8; 36 + 24];
            s[12..16].copy_from_slice(&u32::from(unity).to_le_bytes());
            s[28..32].copy_from_slice(&1u32.to_le_bytes());
            s[44..48].copy_from_slice(&start.to_le_bytes());
            s[48..52].copy_from_slice(&end.to_le_bytes());
            body.extend_from_slice(b"smpl");
            body.extend_from_slice(&(s.len() as u32).to_le_bytes());
            body.extend_from_slice(&s);
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&body);
        file
    }

    #[test]
    fn wav_at_16_and_24_bits_keeps_its_depth_loop_and_unity_note() {
        let s = decode(&wav_file(
            44_100,
            1,
            16,
            &[0, 16_384, -32_768],
            Some(((1, 2), 64)),
        ))
        .unwrap();
        assert_eq!((s.rate, s.channels, s.frames), (44_100, 1, 3));
        assert!(matches!(s.pcm, Pcm::I16(_)));
        assert_eq!(
            [s.at(0, 0), s.at(1, 0), s.at(2, 0), s.at(3, 0)],
            [0.0, 0.5, -1.0, 0.0]
        );
        assert_eq!((s.looped, s.unity), (Some((1, 2)), Some(64)));
        assert_eq!(s.bytes(), 6);

        let s = decode(&wav_file(
            48_000,
            2,
            24,
            &[4_194_304, -8_388_608, -1, 1],
            None,
        ))
        .unwrap();
        assert_eq!((s.channels, s.frames), (2, 2));
        assert!(matches!(s.pcm, Pcm::I24(_)));
        assert_eq!([s.at(0, 0), s.at(0, 1)], [0.5, -1.0]);
        assert_eq!(s.at(1, 0), -1.0 / 8_388_608.0);
        assert_eq!(s.bytes(), 12);
    }

    #[test]
    fn more_than_two_channels_keep_the_first_two() {
        let s = decode(&wav_file(48_000, 3, 16, &[1, 2, 3, 4, 5, 6], None)).unwrap();
        assert_eq!((s.channels, s.frames), (2, 2));
        assert_eq!(s.pcm, Pcm::I16(vec![1, 2, 4, 5].into()));
    }

    #[test]
    fn a_data_chunk_longer_than_the_file_is_read_to_its_end() {
        let mut file = wav_file(48_000, 1, 16, &[100, 200, 300], None);
        let data = file.windows(4).position(|w| w == b"data").unwrap();
        file[data + 4..data + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        let s = decode(&file).unwrap();
        assert_eq!(s.frames, 3);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(decode(b"MThd....").is_err());
        assert!(decode(b"RIFF\x04\0\0\0WAVE").is_err());
    }

    /// The FLAC and Ogg fixtures: a 440 Hz sine, 0.1 s at 44.1 kHz, made
    /// with ffmpeg (`fixtures/sfz/README.md`).
    fn fixture(name: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sfz");
        std::fs::read(root.join(name)).unwrap()
    }

    fn sine_like(s: &Sample) {
        assert_eq!(s.rate, 44_100);
        // Ogg Vorbis may add a little padding at the end.
        assert!((4_410..=4_800).contains(&s.frames), "{}", s.frames);
        let peak = (0..s.frames)
            .map(|f| s.at(f, 0).abs())
            .fold(0.0f32, f32::max);
        assert!((0.4..0.6).contains(&peak), "{peak}");
        // A 440 Hz sine crosses zero about 88 times in 0.1 s.
        let crossings = (1..s.frames)
            .filter(|&f| (s.at(f - 1, 0) < 0.0) != (s.at(f, 0) < 0.0))
            .count();
        assert!((80..=96).contains(&crossings), "{crossings}");
    }

    #[test]
    fn flac_at_16_and_24_bits() {
        let s = decode(&fixture("sine-16.flac")).unwrap();
        assert!(matches!(s.pcm, Pcm::I16(_)));
        sine_like(&s);
        let s = decode(&fixture("sine-24.flac")).unwrap();
        assert!(matches!(s.pcm, Pcm::I24(_)));
        sine_like(&s);
    }

    #[test]
    fn ogg_vorbis() {
        let s = decode(&fixture("sine.ogg")).unwrap();
        assert!(matches!(s.pcm, Pcm::F32(_)));
        sine_like(&s);
    }
}
