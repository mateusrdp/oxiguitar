// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Downloadable Sounds (DLS levels 1 and 2) to SoundFont 2, in memory, for
//! rustysynth: how Windows' `gm.dls` and macOS's `gs_instruments.dls` play.
//!
//! A DLS file is a RIFF `DLS ` form: instruments (`lins`), each with
//! regions over waves in a pool (`wvpl`, found through the cue table
//! `ptbl`), and articulations (`lart`/`lar2`) of connection blocks. Each
//! wave becomes a sample, each instrument an instrument and a preset (bank
//! from the locale's MSB; drum kits at bank 128), each region a zone, and
//! each connection block that SoundFont 2 has a generator for becomes that
//! generator: the envelopes' times and levels, the LFOs, pan, attenuation,
//! the filter, pitch from the LFO and the modulation envelope, key scaling
//! of envelope times and of pitch. Blocks that need a modulator in
//! SoundFont 2 (velocity to attack time, controllers) are left out:
//! rustysynth ignores modulators anyway. The DLS 2.2 specification and
//! FluidSynth's DLS loader (`fluid_dls.c`) were the references.

use oxiguitar_plugin::Error;

use crate::riff::{self, Chunk, i32_at, invalid, u16_at, u32_at};
use crate::sf2::{self, Sf2, generator as g};

/// Connection sources and destinations (DLS 2.2, section 1.14).
mod conn {
    pub(super) const SRC_NONE: u16 = 0x0000;
    pub(super) const SRC_LFO: u16 = 0x0001;
    pub(super) const SRC_KEYNUMBER: u16 = 0x0003;
    pub(super) const SRC_EG2: u16 = 0x0005;
    pub(super) const SRC_VIBRATO: u16 = 0x0009;

    pub(super) const DST_GAIN: u16 = 0x0001;
    pub(super) const DST_PITCH: u16 = 0x0003;
    pub(super) const DST_PAN: u16 = 0x0004;
    pub(super) const DST_CHORUS: u16 = 0x0080;
    pub(super) const DST_REVERB: u16 = 0x0081;
    pub(super) const DST_LFO_FREQUENCY: u16 = 0x0104;
    pub(super) const DST_LFO_STARTDELAY: u16 = 0x0105;
    pub(super) const DST_VIB_FREQUENCY: u16 = 0x0114;
    pub(super) const DST_VIB_STARTDELAY: u16 = 0x0115;
    pub(super) const DST_EG1_ATTACKTIME: u16 = 0x0206;
    pub(super) const DST_EG1_DECAYTIME: u16 = 0x0207;
    pub(super) const DST_EG1_RELEASETIME: u16 = 0x0209;
    pub(super) const DST_EG1_SUSTAINLEVEL: u16 = 0x020A;
    pub(super) const DST_EG1_DELAYTIME: u16 = 0x020B;
    pub(super) const DST_EG1_HOLDTIME: u16 = 0x020C;
    pub(super) const DST_EG2_ATTACKTIME: u16 = 0x030A;
    pub(super) const DST_EG2_DECAYTIME: u16 = 0x030B;
    pub(super) const DST_EG2_RELEASETIME: u16 = 0x030D;
    pub(super) const DST_EG2_SUSTAINLEVEL: u16 = 0x030E;
    pub(super) const DST_EG2_DELAYTIME: u16 = 0x030F;
    pub(super) const DST_EG2_HOLDTIME: u16 = 0x0310;
    pub(super) const DST_FILTER_CUTOFF: u16 = 0x0500;
    pub(super) const DST_FILTER_Q: u16 = 0x0501;
}

/// The locale's bit for a drum kit.
const DRUMS: u32 = 0x8000_0000;

/// A connection block: source, control, destination, transform, scale.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Block {
    source: u16,
    control: u16,
    destination: u16,
    scale: i32,
}

/// A `wsmp` chunk: the sample's tuning, gain and loop.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Wsmp {
    unity: u8,
    fine: i16,
    /// In centibels, positive to make it quieter.
    attenuation: i32,
    /// Start, length and type (0 forward, 1 until released then on).
    looped: Option<(u32, u32, u32)>,
}

fn wsmp(data: &[u8]) -> Option<Wsmp> {
    let size = u32_at(data, 0)? as usize;
    let unity = u16_at(data, 4)?;
    let fine = u16_at(data, 6)? as i16;
    let gain = i32_at(data, 8)?;
    let loops = u32_at(data, 16)?;
    let looped = if loops > 0 {
        let at = size;
        Some((
            u32_at(data, at + 8)?,
            u32_at(data, at + 12)?,
            u32_at(data, at + 4)?,
        ))
    } else {
        None
    };
    Some(Wsmp {
        unity: unity.min(127) as u8,
        fine,
        // Relative gain is in 1/65536 centibels; DLS's is a gain, so
        // negative means quieter.
        attenuation: -(gain >> 16),
        looped,
    })
}

/// The connection blocks of an articulation list (`lart` or `lar2`).
fn blocks(list: &[u8]) -> Vec<Block> {
    let mut out = Vec::new();
    for c in riff::chunks(list).unwrap_or_default() {
        if &c.id != b"art1" && &c.id != b"art2" {
            continue;
        }
        let (Some(size), Some(count)) = (u32_at(c.data, 0), u32_at(c.data, 4)) else {
            continue;
        };
        for i in 0..count as usize {
            let at = size as usize + i * 12;
            let (Some(source), Some(control), Some(destination), Some(scale)) = (
                u16_at(c.data, at),
                u16_at(c.data, at + 2),
                u16_at(c.data, at + 4),
                i32_at(c.data, at + 8),
            ) else {
                break;
            };
            out.push(Block {
                source,
                control,
                destination,
                scale,
            });
        }
    }
    out
}

/// The articulation lists among `chunks` (DLS 1's `lart`, DLS 2's `lar2`).
fn articulation(chunks: &[Chunk<'_>]) -> Vec<Block> {
    chunks
        .iter()
        .filter_map(Chunk::list)
        .filter(|(k, _)| k == b"lart" || k == b"lar2")
        .flat_map(|(_, body)| blocks(body))
        .collect()
}

/// A 32-bit DLS value in 1/65536 units as a SoundFont 2 amount.
fn whole(v: i32) -> i32 {
    v >> 16
}

/// A time in absolute timecents; DLS writes the shortest time as
/// `0x80000000`.
fn time(v: i32) -> i16 {
    if v == i32::MIN {
        -32768
    } else {
        whole(v).clamp(-32768, 32767) as i16
    }
}

fn amount(v: i32) -> i16 {
    v.clamp(-32768, 32767) as i16
}

/// What the generators of one zone come to, from its articulation.
#[derive(Default)]
struct Generators {
    list: Vec<(u16, i16)>,
    /// Envelope times to shift for key scaling, applied once all blocks
    /// are read: (generator, timecents to add).
    shifts: Vec<(u16, i32)>,
}

impl Generators {
    fn set(&mut self, n: u16, v: i16) {
        match self.list.iter_mut().find(|(k, _)| *k == n) {
            Some(slot) => slot.1 = v,
            None => self.list.push((n, v)),
        }
    }

    fn get(&self, n: u16) -> Option<i16> {
        self.list.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
    }

    /// Takes `b` if SoundFont 2 has a generator for it; whether it did.
    fn take(&mut self, b: Block) -> bool {
        use conn::*;
        let v = b.scale;
        match (b.source, b.control, b.destination) {
            (SRC_NONE, SRC_NONE, d) => {
                let n = match d {
                    DST_EG1_ATTACKTIME => g::ATTACK_VOL_ENV,
                    DST_EG1_DECAYTIME => g::DECAY_VOL_ENV,
                    DST_EG1_RELEASETIME => g::RELEASE_VOL_ENV,
                    DST_EG1_DELAYTIME => g::DELAY_VOL_ENV,
                    DST_EG1_HOLDTIME => g::HOLD_VOL_ENV,
                    DST_EG2_ATTACKTIME => g::ATTACK_MOD_ENV,
                    DST_EG2_DECAYTIME => g::DECAY_MOD_ENV,
                    DST_EG2_RELEASETIME => g::RELEASE_MOD_ENV,
                    DST_EG2_DELAYTIME => g::DELAY_MOD_ENV,
                    DST_EG2_HOLDTIME => g::HOLD_MOD_ENV,
                    DST_LFO_STARTDELAY => g::DELAY_MOD_LFO,
                    DST_VIB_STARTDELAY => g::DELAY_VIB_LFO,
                    DST_EG1_SUSTAINLEVEL => {
                        // Tenths of a percent of full level; SoundFont 2
                        // wants the drop, in centibels.
                        let level = f64::from(whole(v)) / 1000.0;
                        let cb = if level >= 1.0 {
                            0.0
                        } else if level <= 0.0 {
                            1440.0
                        } else {
                            -200.0 * level.log10()
                        };
                        self.set(g::SUSTAIN_VOL_ENV, cb.round() as i16);
                        return true;
                    }
                    DST_EG2_SUSTAINLEVEL => {
                        // SoundFont 2 wants the drop, in tenths of a percent.
                        self.set(g::SUSTAIN_MOD_ENV, amount(1000 - whole(v).clamp(0, 1000)));
                        return true;
                    }
                    DST_LFO_FREQUENCY => {
                        self.set(g::FREQ_MOD_LFO, amount(whole(v)));
                        return true;
                    }
                    DST_VIB_FREQUENCY => {
                        self.set(g::FREQ_VIB_LFO, amount(whole(v)));
                        return true;
                    }
                    DST_PAN => {
                        self.set(g::PAN, amount(whole(v).clamp(-500, 500)));
                        return true;
                    }
                    DST_GAIN => {
                        let now = i32::from(self.get(g::INITIAL_ATTENUATION).unwrap_or(0));
                        self.set(g::INITIAL_ATTENUATION, amount(now - whole(v)));
                        return true;
                    }
                    DST_CHORUS => {
                        self.set(g::CHORUS_SEND, amount(whole(v).clamp(0, 1000)));
                        return true;
                    }
                    DST_REVERB => {
                        self.set(g::REVERB_SEND, amount(whole(v).clamp(0, 1000)));
                        return true;
                    }
                    DST_FILTER_CUTOFF => {
                        // 0x7FFFFFFF: no filter.
                        if v != i32::MAX {
                            self.set(g::INITIAL_FILTER_FC, amount(whole(v).clamp(1500, 13500)));
                        }
                        return true;
                    }
                    DST_FILTER_Q => {
                        self.set(g::INITIAL_FILTER_Q, amount(whole(v).clamp(0, 960)));
                        return true;
                    }
                    _ => return false,
                };
                self.set(n, time(v));
                true
            }
            (SRC_LFO, SRC_NONE, DST_PITCH) => {
                self.set(g::MOD_LFO_TO_PITCH, amount(whole(v)));
                true
            }
            (SRC_VIBRATO, SRC_NONE, DST_PITCH) => {
                self.set(g::VIB_LFO_TO_PITCH, amount(whole(v)));
                true
            }
            (SRC_LFO, SRC_NONE, DST_GAIN) => {
                self.set(g::MOD_LFO_TO_VOLUME, amount(whole(v)));
                true
            }
            (SRC_LFO, SRC_NONE, DST_FILTER_CUTOFF) => {
                self.set(g::MOD_LFO_TO_FILTER_FC, amount(whole(v)));
                true
            }
            (SRC_EG2, SRC_NONE, DST_PITCH) => {
                self.set(g::MOD_ENV_TO_PITCH, amount(whole(v)));
                true
            }
            (SRC_EG2, SRC_NONE, DST_FILTER_CUTOFF) => {
                self.set(g::MOD_ENV_TO_FILTER_FC, amount(whole(v)));
                true
            }
            (SRC_KEYNUMBER, SRC_NONE, DST_PITCH) => {
                // DLS scales over 128 keys, SoundFont 2 per key.
                self.set(g::SCALE_TUNING, amount(whole(v) / 128));
                true
            }
            (SRC_KEYNUMBER, SRC_NONE, d) => {
                // DLS adds `scale × key / 128` timecents; SoundFont 2 adds
                // `amount × (60 − key)`: the same line with the opposite
                // slope, moved by its value at key 60.
                let (n, base) = match d {
                    DST_EG1_DECAYTIME => (g::KEYNUM_TO_VOL_ENV_DECAY, g::DECAY_VOL_ENV),
                    DST_EG1_HOLDTIME => (g::KEYNUM_TO_VOL_ENV_HOLD, g::HOLD_VOL_ENV),
                    DST_EG2_DECAYTIME => (g::KEYNUM_TO_MOD_ENV_DECAY, g::DECAY_MOD_ENV),
                    DST_EG2_HOLDTIME => (g::KEYNUM_TO_MOD_ENV_HOLD, g::HOLD_MOD_ENV),
                    _ => return false,
                };
                let per_key = f64::from(whole(v)) / 128.0;
                self.set(n, amount((-per_key).round() as i32));
                self.shifts.push((base, (per_key * 60.0).round() as i32));
                true
            }
            _ => false,
        }
    }

    fn finish(mut self) -> Vec<(u16, i16)> {
        for (n, by) in std::mem::take(&mut self.shifts) {
            let now = i32::from(self.get(n).unwrap_or(-12000));
            self.set(n, amount(now + by));
        }
        self.list
    }
}

/// A wave of the pool, as a sample.
struct Wave {
    sample: sf2::Sample,
    wsmp: Option<Wsmp>,
}

fn wave(body: &[u8], index: usize) -> Result<Wave, Error> {
    let chunks = riff::chunks(body)?;
    let fmt = riff::find(&chunks, b"fmt ")
        .ok_or_else(|| invalid(format!("wave {index} has no format")))?;
    let data =
        riff::find(&chunks, b"data").ok_or_else(|| invalid(format!("wave {index} has no data")))?;
    let tag = u16_at(fmt, 0).unwrap_or(0);
    let channels = usize::from(u16_at(fmt, 2).unwrap_or(1).max(1));
    let rate = u32_at(fmt, 4).unwrap_or(0);
    let bits = u16_at(fmt, 14).unwrap_or(16);
    if tag != 1 {
        return Err(invalid(format!(
            "wave {index} isn't plain PCM (format {tag})"
        )));
    }
    if rate == 0 {
        return Err(invalid(format!("wave {index} has no sample rate")));
    }
    let bytes = usize::from(bits.div_ceil(8)).max(1);
    let frame = bytes * channels;
    // The first channel, as 16 bits.
    let frames: Vec<i16> = data
        .chunks_exact(frame)
        .map(|f| match bytes {
            1 => (i16::from(f[0]) - 128) << 8,
            _ => i16::from_le_bytes([f[bytes - 2], f[bytes - 1]]),
        })
        .collect();
    let wsmp = riff::find(&chunks, b"wsmp").and_then(wsmp);
    let w = wsmp.unwrap_or(Wsmp {
        unity: 60,
        ..Wsmp::default()
    });
    let len = frames.len() as u32;
    Ok(Wave {
        sample: sf2::Sample {
            name: format!("Wave {index}"),
            data: frames,
            rate,
            root: w.unity,
            correction: w.fine.clamp(-99, 99) as i8,
            looped: w
                .looped
                .map(|(s, l, _)| (s.min(len), s.saturating_add(l).min(len))),
        },
        wsmp,
    })
}

/// The SoundFont 2 file the DLS file `file` becomes, and how many of its
/// articulation blocks SoundFont 2 has no generator for (left out).
pub(crate) fn to_sf2(file: &[u8]) -> Result<(Vec<u8>, usize), Error> {
    let (kind, top) = riff::form(file)?;
    if kind != *b"DLS " {
        return Err(invalid("it isn't a DLS file"));
    }
    // The wave pool, by each wave's offset in it.
    let pool = riff::find_list(&top, b"wvpl").ok_or_else(|| invalid("it has no wave pool"))?;
    // A wave it can't read (compressed, damaged) leaves its regions out.
    let mut offsets = Vec::new();
    let mut waves: Vec<Option<Wave>> = Vec::new();
    for c in riff::chunks(pool)? {
        if let Some((k, body)) = c.list()
            && &k == b"wave"
        {
            offsets.push(c.at as u32);
            waves.push(wave(body, waves.len()).ok());
        }
    }
    // Each wave's sample number in the SoundFont.
    let mut numbers = Vec::with_capacity(waves.len());
    let mut samples = Vec::new();
    for w in &waves {
        numbers.push(w.as_ref().map(|w| {
            samples.push(w.sample.clone());
            (samples.len() - 1) as u16
        }));
    }
    let ptbl = riff::find(&top, b"ptbl").ok_or_else(|| invalid("it has no cue table"))?;
    let cues: Vec<u32> = {
        let size = u32_at(ptbl, 0).unwrap_or(8) as usize;
        let count = u32_at(ptbl, 4).unwrap_or(0) as usize;
        (0..count)
            .map_while(|i| u32_at(ptbl, size + i * 4))
            .collect()
    };
    let wave_of = |cue: u32| -> Option<usize> {
        let offset = *cues.get(cue as usize)?;
        offsets.iter().position(|&o| o == offset)
    };

    let mut sf = Sf2 {
        name: "DLS".into(),
        samples,
        ..Sf2::default()
    };
    if let Some(info) = riff::find_list(&top, b"INFO").and_then(|i| riff::chunks(i).ok())
        && let Some(name) = riff::find(&info, b"INAM")
    {
        sf.name = String::from_utf8_lossy(name)
            .trim_end_matches('\0')
            .to_owned();
    }
    let mut left_out = 0;
    let lins = riff::find_list(&top, b"lins").ok_or_else(|| invalid("it has no instruments"))?;
    for ins in riff::chunks(lins)? {
        let Some((k, body)) = ins.list() else {
            continue;
        };
        if &k != b"ins " {
            continue;
        }
        let chunks = riff::chunks(body)?;
        let Some(insh) = riff::find(&chunks, b"insh") else {
            continue;
        };
        let bank = u32_at(insh, 4).unwrap_or(0);
        let program = (u32_at(insh, 8).unwrap_or(0) & 0x7F) as u16;
        let (bank, drums) = if bank & DRUMS != 0 {
            (128, true)
        } else {
            (((bank >> 8) & 0x7F) as u16, false)
        };
        let name = riff::find_list(&chunks, b"INFO")
            .and_then(|i| riff::chunks(i).ok())
            .and_then(|i| {
                riff::find(&i, b"INAM")
                    .map(|n| String::from_utf8_lossy(n).trim_end_matches('\0').to_owned())
            })
            .unwrap_or_else(|| format!("{}{program}", if drums { "Kit " } else { "Program " }));
        let global = articulation(&chunks);
        let mut instrument = sf2::Instrument {
            name: name.clone(),
            zones: Vec::new(),
        };
        let Some(lrgn) = riff::find_list(&chunks, b"lrgn") else {
            continue;
        };
        for rgn in riff::chunks(lrgn)? {
            let Some((k, body)) = rgn.list() else {
                continue;
            };
            if &k != b"rgn " && &k != b"rgn2" {
                continue;
            }
            let rc = riff::chunks(body)?;
            let (Some(rgnh), Some(wlnk)) = (riff::find(&rc, b"rgnh"), riff::find(&rc, b"wlnk"))
            else {
                continue;
            };
            let Some(w) = u32_at(wlnk, 8).and_then(wave_of) else {
                continue;
            };
            let (Some(wave), Some(number)) = (&waves[w], numbers[w]) else {
                continue;
            };
            let keys = (
                u16_at(rgnh, 0).unwrap_or(0).min(127) as u8,
                u16_at(rgnh, 2).unwrap_or(127).min(127) as u8,
            );
            // DLS level 1 has no velocity ranges, and files of it often
            // leave them 0 to 0: every velocity, then.
            let velocities = match (u16_at(rgnh, 4), u16_at(rgnh, 6)) {
                (Some(lo), Some(hi)) if hi > 0 && lo <= hi => {
                    (lo.min(127) as u8, hi.min(127) as u8)
                }
                _ => (0, 127),
            };
            let group = u16_at(rgnh, 10).unwrap_or(0);
            let mut gens = Generators::default();
            // The region's own articulation over the instrument's.
            let own = articulation(&rc);
            for b in global.iter().chain(&own) {
                if !gens.take(*b) {
                    left_out += 1;
                }
            }
            let region = riff::find(&rc, b"wsmp").and_then(wsmp);
            let tuning = region.or(wave.wsmp).unwrap_or(Wsmp {
                unity: 60,
                ..Wsmp::default()
            });
            if tuning.unity != wave.sample.root {
                gens.set(g::OVERRIDING_ROOT_KEY, i16::from(tuning.unity));
            }
            let fine = i32::from(tuning.fine) - i32::from(wave.sample.correction);
            if fine != 0 {
                gens.set(g::FINE_TUNE, amount(fine));
            }
            if tuning.attenuation != 0 {
                let now = i32::from(gens.get(g::INITIAL_ATTENUATION).unwrap_or(0));
                gens.set(
                    g::INITIAL_ATTENUATION,
                    amount((now + tuning.attenuation).max(0)),
                );
            }
            let len = wave.sample.data.len() as u32;
            match tuning.looped {
                Some((start, length, kind)) => {
                    let (s, e) = (start.min(len), start.saturating_add(length).min(len));
                    let (ws, we) = wave.sample.looped.unwrap_or((0, len));
                    let (ds, de) = (s as i32 - ws as i32, e as i32 - we as i32);
                    for (fine, coarse, d) in [
                        (g::START_LOOP_OFFSET, g::START_LOOP_COARSE_OFFSET, ds),
                        (g::END_LOOP_OFFSET, g::END_LOOP_COARSE_OFFSET, de),
                    ] {
                        if d != 0 {
                            gens.set(coarse, amount(d.div_euclid(32768)));
                            gens.set(fine, amount(d.rem_euclid(32768)));
                        }
                    }
                    gens.set(g::SAMPLE_MODES, if kind == 1 { 3 } else { 1 });
                }
                None => gens.set(g::SAMPLE_MODES, 0),
            }
            if group != 0 {
                gens.set(g::EXCLUSIVE_CLASS, amount(i32::from(group)));
            }
            instrument.zones.push(sf2::Zone {
                keys: Some(keys),
                velocities: Some(velocities),
                generators: gens.finish(),
                target: number,
            });
        }
        if instrument.zones.is_empty()
            || sf
                .presets
                .iter()
                .any(|p| p.bank == bank && p.program == program)
        {
            continue;
        }
        let index = sf.instruments.len() as u16;
        sf.instruments.push(instrument);
        sf.presets.push(sf2::Preset {
            name,
            bank,
            program,
            zones: vec![sf2::Zone {
                target: index,
                ..sf2::Zone::default()
            }],
        });
    }
    if sf.presets.is_empty() {
        return Err(invalid("it has no instrument with a sound"));
    }
    Ok((sf.write(), left_out))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
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

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn halves(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn wsmp_chunk(unity: u16, fine: i16, gain: i32, looped: Option<(u32, u32)>) -> Vec<u8> {
        let mut d = words(&[20]);
        d.extend(halves(&[unity, fine as u16]));
        d.extend(gain.to_le_bytes());
        d.extend(words(&[0, u32::from(looped.is_some())]));
        if let Some((s, l)) = looped {
            d.extend(words(&[16, 0, s, l]));
        }
        chunk(b"wsmp", &d)
    }

    fn art(blocks: &[(u16, u16, u16, i32)]) -> Vec<u8> {
        let mut d = words(&[8, blocks.len() as u32]);
        for &(s, c, dst, scale) in blocks {
            d.extend(halves(&[s, c, dst, 0]));
            d.extend(scale.to_le_bytes());
        }
        list(b"lart", &[chunk(b"art1", &d)])
    }

    /// A small DLS file: one 16-bit wave (a 441 Hz sine at 44.1 kHz,
    /// looped, unity note 69), a melodic instrument (bank 0, program 24)
    /// with an articulation, and a drum kit (program 0) whose region plays
    /// key 36 only.
    pub(crate) fn small() -> Vec<u8> {
        let sine: Vec<u8> = (0..4410)
            .flat_map(|i| {
                let v = (std::f64::consts::TAU * f64::from(i) / 100.0).cos() * 12000.0;
                (v.round() as i16).to_le_bytes()
            })
            .collect();
        let fmt = {
            let mut f = halves(&[1, 1]);
            f.extend(words(&[44_100, 88_200]));
            f.extend(halves(&[2, 16]));
            f
        };
        let wave = list(
            b"wave",
            &[
                chunk(b"fmt ", &fmt),
                wsmp_chunk(69, 0, 0, Some((0, 4410))),
                chunk(b"data", &sine),
            ],
        );
        let pool = list(b"wvpl", &[wave]);
        let ptbl = chunk(b"ptbl", &words(&[8, 1, 0]));
        let region = |lo: u16, hi: u16, extra: Vec<Vec<u8>>| {
            let mut parts = vec![
                chunk(b"rgnh", &halves(&[lo, hi, 0, 127, 0, 0])),
                chunk(b"wlnk", &{
                    let mut d = halves(&[0, 0]);
                    d.extend(words(&[1, 0]));
                    d
                }),
            ];
            parts.extend(extra);
            list(b"rgn ", &parts)
        };
        let guitar = list(
            b"ins ",
            &[
                chunk(b"insh", &words(&[1, 0, 24])),
                list(b"lrgn", &[region(0, 127, vec![])]),
                art(&[
                    (0, 0, conn::DST_EG1_RELEASETIME, -2400 << 16),
                    (0, 0, conn::DST_EG1_SUSTAINLEVEL, 500 << 16),
                    (0, 0, conn::DST_PAN, 100 << 16),
                    // Velocity to attack: no generator for it.
                    (0x0002, 0, conn::DST_EG1_ATTACKTIME, 1200 << 16),
                ]),
                list(b"INFO", &[chunk(b"INAM", b"Nylon\0")]),
            ],
        );
        let kit = list(
            b"ins ",
            &[
                chunk(b"insh", &words(&[1, DRUMS, 0])),
                list(
                    b"lrgn",
                    &[region(36, 36, vec![wsmp_chunk(60, 0, -(60 << 16), None)])],
                ),
            ],
        );
        let lins = list(b"lins", &[guitar, kit]);
        let mut body = b"DLS ".to_vec();
        body.extend(chunk(b"colh", &words(&[2])));
        body.extend(lins);
        body.extend(ptbl);
        body.extend(pool);
        chunk(b"RIFF", &body)
    }

    #[test]
    fn a_small_dls_becomes_a_soundfont_rustysynth_reads() {
        let (sf2, left_out) = to_sf2(&small()).unwrap();
        assert_eq!(left_out, 1);
        let font = rustysynth::SoundFont::new(&mut std::io::Cursor::new(&sf2)).unwrap();
        let presets = font.get_presets();
        assert_eq!(presets.len(), 2);
        assert_eq!(
            (presets[0].get_bank_number(), presets[0].get_patch_number()),
            (0, 24)
        );
        assert_eq!(presets[0].get_name(), "Nylon");
        assert_eq!(
            (presets[1].get_bank_number(), presets[1].get_patch_number()),
            (128, 0)
        );
        let s = &font.get_sample_headers()[0];
        assert_eq!((s.get_original_pitch(), s.get_sample_rate()), (69, 44_100));
        assert_eq!((s.get_start_loop(), s.get_end_loop()), (0, 4410));
        let regions = font.get_instruments()[0].get_regions();
        assert_eq!(regions.len(), 1);
        let r = &regions[0];
        assert_eq!(r.get_pan(), 10.0);
        assert!((r.get_release_volume_envelope() - 0.25).abs() < 1e-3);
        // 50 % sustain is 6 dB down.
        assert!(
            (r.get_sustain_volume_envelope() - 6.0).abs() < 0.1,
            "{}",
            r.get_sustain_volume_envelope()
        );
        let drum = &font.get_instruments()[1].get_regions()[0];
        assert_eq!(
            (drum.get_key_range_start(), drum.get_key_range_end()),
            (36, 36)
        );
        assert_eq!(drum.get_root_key(), 60);
        assert_eq!(drum.get_initial_attenuation(), 6.0);
    }

    #[test]
    fn its_notes_sound_at_their_pitch() {
        let (sf2, _) = to_sf2(&small()).unwrap();
        let font = std::sync::Arc::new(
            rustysynth::SoundFont::new(&mut std::io::Cursor::new(&sf2)).unwrap(),
        );
        let mut synth =
            rustysynth::Synthesizer::new(&font, &rustysynth::SynthesizerSettings::new(48_000))
                .unwrap();
        synth.process_midi_message(0, 0xC0, 24, 0);
        synth.note_on(0, 69, 100);
        let (mut l, mut r) = (vec![0.0; 9600], vec![0.0; 9600]);
        synth.render(&mut l, &mut r);
        // 441 Hz: count rising zero crossings over the last 0.1 s.
        let tail = &l[4800..];
        let crossings = tail
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        assert!((43..=45).contains(&crossings), "{crossings}");
    }

    #[test]
    fn damaged_files_are_errors_not_panics() {
        let file = small();
        for cut in [
            0,
            11,
            12,
            30,
            100,
            file.len() / 3,
            file.len() / 2,
            file.len() - 5,
        ] {
            let _ = to_sf2(&file[..cut]);
        }
        let mut bad = file.clone();
        for i in (12..bad.len()).step_by(37) {
            bad[i] = 0xFF;
            let _ = to_sf2(&bad);
        }
    }

    /// The system's own set, where there is one (Windows' and macOS's CI
    /// runners have theirs): it converts, rustysynth reads it, and its
    /// piano plays A4 at 440 Hz.
    #[test]
    fn the_systems_dls_set_plays_when_there_is_one() {
        for path in crate::sets::system_places() {
            if crate::sets::Kind::of(&path) != Some(crate::sets::Kind::Dls) {
                continue;
            }
            let Ok(file) = std::fs::read(&path) else {
                eprintln!("{}: not here", path.display());
                continue;
            };
            let (sf2, left_out) = to_sf2(&file).unwrap();
            let font = std::sync::Arc::new(
                rustysynth::SoundFont::new(&mut std::io::Cursor::new(&sf2)).unwrap(),
            );
            let presets = font.get_presets();
            println!(
                "{}: {} presets, {} instruments, {} samples, {} blocks left out",
                path.display(),
                presets.len(),
                font.get_instruments().len(),
                font.get_sample_headers().len(),
                left_out
            );
            assert!(presets.len() >= 128, "{}", presets.len());
            let mut synth =
                rustysynth::Synthesizer::new(&font, &rustysynth::SynthesizerSettings::new(48_000))
                    .unwrap();
            synth.note_on(0, 69, 100);
            let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
            synth.render(&mut l, &mut r);
            let peak = l.iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.01, "silent: {peak}");
            // The strongest of the first harmonics, by a plain DFT over
            // 0.25 s from 0.1 s.
            let at = |hz: f64| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, v) in l[4800..16_800].iter().enumerate() {
                    let ph = std::f64::consts::TAU * hz * i as f64 / 48_000.0;
                    re += f64::from(*v) * ph.cos();
                    im += f64::from(*v) * ph.sin();
                }
                (re * re + im * im).sqrt()
            };
            let fundamental = at(440.0);
            let off = at(415.3).max(at(466.2));
            assert!(
                fundamental > off * 3.0,
                "440 Hz {fundamental}, a semitone off {off}"
            );
        }
    }
}
