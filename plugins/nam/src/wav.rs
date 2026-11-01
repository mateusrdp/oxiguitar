// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading an impulse response's WAV file: 16-, 24- and 32-bit PCM and
//! 32-bit float, mono or the first channel of several. Untrusted input:
//! anything else is refused with a sentence, never a panic.

/// An impulse response as its file holds it.
#[derive(Debug, PartialEq)]
pub(crate) struct Response {
    /// Its sample rate.
    pub(crate) rate: u32,
    /// Its taps, the first channel's.
    pub(crate) taps: Vec<f32>,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Reads a WAV file's first channel.
pub(crate) fn read(data: &[u8]) -> Result<Response, String> {
    if data.get(0..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WAVE") {
        return Err("it isn't a WAV file".into());
    }
    let mut format = None;
    let mut samples = None;
    let mut at = 12;
    while at + 8 <= data.len() {
        let id = &data[at..at + 4];
        let len = u32_at(data, at + 4).ok_or("a damaged chunk")? as usize;
        let body = data
            .get(at + 8..(at + 8).saturating_add(len).min(data.len()))
            .ok_or("a damaged chunk")?;
        match id {
            b"fmt " => format = Some(body),
            b"data" => samples = Some(body),
            _ => {}
        }
        at = at
            .saturating_add(8)
            .saturating_add(len)
            .saturating_add(len & 1);
    }
    let format = format.ok_or("it has no format chunk")?;
    let samples = samples.ok_or("it has no sound")?;
    let mut tag = u16_at(format, 0).ok_or("a damaged format chunk")?;
    let channels = usize::from(u16_at(format, 2).ok_or("a damaged format chunk")?);
    let rate = u32_at(format, 4).ok_or("a damaged format chunk")?;
    let bits = u16_at(format, 14).ok_or("a damaged format chunk")?;
    if tag == 0xFFFE {
        // WAVE_FORMAT_EXTENSIBLE: the real tag starts the sub-format.
        tag = u16_at(format, 24).ok_or("a damaged format chunk")?;
    }
    if channels == 0 || !(8_000..=384_000).contains(&rate) {
        return Err("its format makes no sense".into());
    }
    let width = usize::from(bits / 8);
    let frame = width * channels;
    let taps: Vec<f32> = match (tag, bits) {
        (1, 16) => samples
            .chunks_exact(frame)
            .map(|f| f32::from(i16::from_le_bytes([f[0], f[1]])) / 32_768.0)
            .collect(),
        (1, 24) => samples
            .chunks_exact(frame)
            .map(|f| (i32::from_le_bytes([0, f[0], f[1], f[2]]) >> 8) as f32 / 8_388_608.0)
            .collect(),
        (1, 32) => samples
            .chunks_exact(frame)
            .map(|f| i32::from_le_bytes([f[0], f[1], f[2], f[3]]) as f32 / 2_147_483_648.0)
            .collect(),
        (3, 32) => samples
            .chunks_exact(frame)
            .map(|f| f32::from_le_bytes([f[0], f[1], f[2], f[3]]))
            .map(|v| if v.is_finite() { v } else { 0.0 })
            .collect(),
        _ => {
            return Err(format!(
                "its samples ({bits}-bit, format {tag}) aren't read here"
            ));
        }
    };
    if taps.is_empty() {
        return Err("it has no sound".into());
    }
    Ok(Response { rate, taps })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(tag: u16, channels: u16, bits: u16, rate: u32, data: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        out.extend_from_slice(b"fmt \x10\0\0\0");
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn the_first_channel_of_each_format() {
        let r = read(&wav(1, 2, 16, 44_100, &[0, 0x40, 1, 1, 0, 0xc0, 1, 1])).unwrap();
        assert_eq!(
            r,
            Response {
                rate: 44_100,
                taps: vec![0.5, -0.5]
            }
        );
        let r = read(&wav(1, 1, 24, 48_000, &[0, 0, 0x40, 0, 0, 0xc0])).unwrap();
        assert_eq!(r.taps, [0.5, -0.5]);
        let r = read(&wav(3, 1, 32, 48_000, &0.25f32.to_le_bytes())).unwrap();
        assert_eq!(r.taps, [0.25]);
    }

    #[test]
    fn damage_is_refused_with_a_sentence() {
        assert!(read(b"nonsense").is_err());
        assert!(read(&wav(1, 1, 12, 48_000, &[1, 2, 3])).is_err());
        assert!(read(&wav(1, 0, 16, 48_000, &[1, 2])).is_err());
        let mut cut = wav(1, 1, 16, 48_000, &[1, 2, 3, 4]);
        cut.truncate(30);
        assert!(read(&cut).is_err());
    }

    #[test]
    fn the_shipped_responses_read() {
        for (_, data) in crate::BUILT_IN {
            let r = read(data).unwrap();
            assert_eq!(r.rate, 44_100);
            assert!(r.taps.len() > 2000);
        }
    }
}
