// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A SoundFont 2 file written in memory, for rustysynth to read: what the
//! plugin turns other sets into (DLS, SF3) before rustysynth sees them, and
//! the contract suite's test set. Only what rustysynth reads: samples,
//! instruments and presets with their generators, and no modulators
//! (rustysynth ignores them).
//!
//! The layout follows the SoundFont 2.04 specification: a RIFF `sfbk`
//! form of three lists, `INFO`, `sdta` (16-bit samples, each followed by
//! 46 silent ones) and `pdta` (the nine record chunks, each closed by a
//! terminal record).

/// Generators, by their SoundFont 2 numbers and names (section 8.1.2 of
/// the specification; only those we write).
pub(crate) mod generator {
    pub(crate) const START_LOOP_OFFSET: u16 = 2;
    pub(crate) const END_LOOP_OFFSET: u16 = 3;
    pub(crate) const MOD_LFO_TO_PITCH: u16 = 5;
    pub(crate) const VIB_LFO_TO_PITCH: u16 = 6;
    pub(crate) const MOD_ENV_TO_PITCH: u16 = 7;
    pub(crate) const INITIAL_FILTER_FC: u16 = 8;
    pub(crate) const INITIAL_FILTER_Q: u16 = 9;
    pub(crate) const MOD_LFO_TO_FILTER_FC: u16 = 10;
    pub(crate) const MOD_ENV_TO_FILTER_FC: u16 = 11;
    pub(crate) const MOD_LFO_TO_VOLUME: u16 = 13;
    pub(crate) const CHORUS_SEND: u16 = 15;
    pub(crate) const REVERB_SEND: u16 = 16;
    pub(crate) const PAN: u16 = 17;
    pub(crate) const DELAY_MOD_LFO: u16 = 21;
    pub(crate) const FREQ_MOD_LFO: u16 = 22;
    pub(crate) const DELAY_VIB_LFO: u16 = 23;
    pub(crate) const FREQ_VIB_LFO: u16 = 24;
    pub(crate) const DELAY_MOD_ENV: u16 = 25;
    pub(crate) const ATTACK_MOD_ENV: u16 = 26;
    pub(crate) const HOLD_MOD_ENV: u16 = 27;
    pub(crate) const DECAY_MOD_ENV: u16 = 28;
    pub(crate) const SUSTAIN_MOD_ENV: u16 = 29;
    pub(crate) const RELEASE_MOD_ENV: u16 = 30;
    pub(crate) const KEYNUM_TO_MOD_ENV_HOLD: u16 = 31;
    pub(crate) const KEYNUM_TO_MOD_ENV_DECAY: u16 = 32;
    pub(crate) const DELAY_VOL_ENV: u16 = 33;
    pub(crate) const ATTACK_VOL_ENV: u16 = 34;
    pub(crate) const HOLD_VOL_ENV: u16 = 35;
    pub(crate) const DECAY_VOL_ENV: u16 = 36;
    pub(crate) const SUSTAIN_VOL_ENV: u16 = 37;
    pub(crate) const RELEASE_VOL_ENV: u16 = 38;
    pub(crate) const KEYNUM_TO_VOL_ENV_HOLD: u16 = 39;
    pub(crate) const KEYNUM_TO_VOL_ENV_DECAY: u16 = 40;
    pub(crate) const INSTRUMENT: u16 = 41;
    pub(crate) const KEY_RANGE: u16 = 43;
    pub(crate) const VELOCITY_RANGE: u16 = 44;
    pub(crate) const START_LOOP_COARSE_OFFSET: u16 = 45;
    pub(crate) const INITIAL_ATTENUATION: u16 = 48;
    pub(crate) const END_LOOP_COARSE_OFFSET: u16 = 50;
    pub(crate) const FINE_TUNE: u16 = 52;
    pub(crate) const SAMPLE_ID: u16 = 53;
    pub(crate) const SAMPLE_MODES: u16 = 54;
    pub(crate) const SCALE_TUNING: u16 = 56;
    pub(crate) const EXCLUSIVE_CLASS: u16 = 57;
    pub(crate) const OVERRIDING_ROOT_KEY: u16 = 58;
}

/// A sample: 16-bit mono data at its own rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Sample {
    /// Its name (up to 20 bytes are kept).
    pub(crate) name: String,
    /// The frames.
    pub(crate) data: Vec<i16>,
    /// Frames a second.
    pub(crate) rate: u32,
    /// The MIDI key it sounds at.
    pub(crate) root: u8,
    /// Cents to add to reach that key exactly.
    pub(crate) correction: i8,
    /// The loop, start included and end excluded, in frames of `data`.
    pub(crate) looped: Option<(u32, u32)>,
}

/// A zone: generators, with the range ones first and the sample or
/// instrument last, as the specification orders them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Zone {
    /// Keys, both ends included.
    pub(crate) keys: Option<(u8, u8)>,
    /// Velocities, both ends included.
    pub(crate) velocities: Option<(u8, u8)>,
    /// Other generators: number and amount.
    pub(crate) generators: Vec<(u16, i16)>,
    /// The sample (in an instrument) or the instrument (in a preset).
    pub(crate) target: u16,
}

/// An instrument: zones over samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Instrument {
    /// Its name.
    pub(crate) name: String,
    /// Its zones.
    pub(crate) zones: Vec<Zone>,
}

/// A preset: zones over instruments, at a bank and program.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Preset {
    /// Its name.
    pub(crate) name: String,
    /// The bank: 0 for melodic sounds, 128 for drum kits.
    pub(crate) bank: u16,
    /// The program, 0 to 127.
    pub(crate) program: u16,
    /// Its zones.
    pub(crate) zones: Vec<Zone>,
}

/// A whole set.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Sf2 {
    /// Its name.
    pub(crate) name: String,
    /// The samples.
    pub(crate) samples: Vec<Sample>,
    /// The instruments.
    pub(crate) instruments: Vec<Instrument>,
    /// The presets.
    pub(crate) presets: Vec<Preset>,
}

/// The silent frames after each sample, as the specification asks.
const PAD: usize = 46;

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

fn list(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    let mut inner = Vec::with_capacity(body.len() + 4);
    inner.extend_from_slice(kind);
    inner.extend_from_slice(body);
    chunk(out, b"LIST", &inner);
}

/// A name in `len` bytes, cut and padded with zeros.
fn name(out: &mut Vec<u8>, text: &str, len: usize) {
    let bytes: Vec<u8> = text
        .bytes()
        .map(|b| if b.is_ascii() && b != 0 { b } else { b'?' })
        .take(len - 1)
        .collect();
    out.extend_from_slice(&bytes);
    out.resize(out.len() + len - bytes.len(), 0);
}

/// A text chunk of the `INFO` list: zero-terminated, even length.
fn text(out: &mut Vec<u8>, id: &[u8; 4], value: &str) {
    let mut body: Vec<u8> = value.bytes().filter(|b| *b != 0).take(255).collect();
    body.push(0);
    if body.len() % 2 == 1 {
        body.push(0);
    }
    chunk(out, id, &body);
}

/// A zone's generators in the order the specification asks.
fn generators(zone: &Zone, last: u16) -> Vec<(u16, u16)> {
    let mut g = Vec::with_capacity(zone.generators.len() + 3);
    if let Some((lo, hi)) = zone.keys {
        g.push((generator::KEY_RANGE, u16::from(lo) | (u16::from(hi) << 8)));
    }
    if let Some((lo, hi)) = zone.velocities {
        g.push((
            generator::VELOCITY_RANGE,
            u16::from(lo) | (u16::from(hi) << 8),
        ));
    }
    for &(n, amount) in &zone.generators {
        g.push((n, amount as u16));
    }
    g.push((last, zone.target));
    g
}

impl Sf2 {
    /// The file's bytes.
    pub(crate) fn write(&self) -> Vec<u8> {
        let mut info = Vec::new();
        chunk(&mut info, b"ifil", &[2, 0, 4, 0]);
        text(&mut info, b"isng", "EMU8000");
        text(&mut info, b"INAM", &self.name);

        let frames: usize = self.samples.iter().map(|s| s.data.len() + PAD).sum();
        let mut smpl = Vec::with_capacity(frames * 2);
        let mut starts = Vec::with_capacity(self.samples.len());
        for s in &self.samples {
            starts.push((smpl.len() / 2) as u32);
            for v in &s.data {
                smpl.extend_from_slice(&v.to_le_bytes());
            }
            smpl.resize(smpl.len() + PAD * 2, 0);
        }
        let mut sdta = Vec::with_capacity(smpl.len() + 8);
        chunk(&mut sdta, b"smpl", &smpl);

        let mut phdr = Vec::new();
        let mut pbag = Vec::new();
        let mut pgen = Vec::new();
        let (mut bags, mut gens) = (0u16, 0u16);
        for p in &self.presets {
            name(&mut phdr, &p.name, 20);
            phdr.extend_from_slice(&p.program.to_le_bytes());
            phdr.extend_from_slice(&p.bank.to_le_bytes());
            phdr.extend_from_slice(&bags.to_le_bytes());
            phdr.extend_from_slice(&[0; 12]);
            for z in &p.zones {
                pbag.extend_from_slice(&gens.to_le_bytes());
                pbag.extend_from_slice(&0u16.to_le_bytes());
                for (n, a) in generators(z, generator::INSTRUMENT) {
                    pgen.extend_from_slice(&n.to_le_bytes());
                    pgen.extend_from_slice(&a.to_le_bytes());
                    gens += 1;
                }
                bags += 1;
            }
        }
        name(&mut phdr, "EOP", 20);
        phdr.extend_from_slice(&[0; 4]);
        phdr.extend_from_slice(&bags.to_le_bytes());
        phdr.extend_from_slice(&[0; 12]);
        pbag.extend_from_slice(&gens.to_le_bytes());
        pbag.extend_from_slice(&0u16.to_le_bytes());
        pgen.extend_from_slice(&[0; 4]);

        let mut inst = Vec::new();
        let mut ibag = Vec::new();
        let mut igen = Vec::new();
        let (mut bags, mut gens) = (0u16, 0u16);
        for i in &self.instruments {
            name(&mut inst, &i.name, 20);
            inst.extend_from_slice(&bags.to_le_bytes());
            for z in &i.zones {
                ibag.extend_from_slice(&gens.to_le_bytes());
                ibag.extend_from_slice(&0u16.to_le_bytes());
                for (n, a) in generators(z, generator::SAMPLE_ID) {
                    igen.extend_from_slice(&n.to_le_bytes());
                    igen.extend_from_slice(&a.to_le_bytes());
                    gens += 1;
                }
                bags += 1;
            }
        }
        name(&mut inst, "EOI", 20);
        inst.extend_from_slice(&bags.to_le_bytes());
        ibag.extend_from_slice(&gens.to_le_bytes());
        ibag.extend_from_slice(&0u16.to_le_bytes());
        igen.extend_from_slice(&[0; 4]);

        let mut shdr = Vec::new();
        for (s, &start) in self.samples.iter().zip(&starts) {
            let end = start + s.data.len() as u32;
            let (ls, le) = s.looped.unwrap_or((0, s.data.len() as u32));
            name(&mut shdr, &s.name, 20);
            for v in [start, end, start + ls, start + le, s.rate] {
                shdr.extend_from_slice(&v.to_le_bytes());
            }
            shdr.push(s.root);
            shdr.push(s.correction as u8);
            shdr.extend_from_slice(&0u16.to_le_bytes()); // link
            shdr.extend_from_slice(&1u16.to_le_bytes()); // mono
        }
        name(&mut shdr, "EOS", 20);
        shdr.extend_from_slice(&[0; 26]);

        let mut pdta = Vec::new();
        chunk(&mut pdta, b"phdr", &phdr);
        chunk(&mut pdta, b"pbag", &pbag);
        chunk(&mut pdta, b"pmod", &[0; 10]);
        chunk(&mut pdta, b"pgen", &pgen);
        chunk(&mut pdta, b"inst", &inst);
        chunk(&mut pdta, b"ibag", &ibag);
        chunk(&mut pdta, b"imod", &[0; 10]);
        chunk(&mut pdta, b"igen", &igen);
        chunk(&mut pdta, b"shdr", &shdr);

        let mut body = Vec::with_capacity(info.len() + sdta.len() + pdta.len() + 40);
        body.extend_from_slice(b"sfbk");
        list(&mut body, b"INFO", &info);
        list(&mut body, b"sdta", &sdta);
        list(&mut body, b"pdta", &pdta);
        let mut out = Vec::with_capacity(body.len() + 8);
        chunk(&mut out, b"RIFF", &body);
        out
    }
}

/// The test set: a looped sine (480 Hz, recorded at 48 kHz, starting at
/// its peak so its first frame is never silent) as a melodic preset (bank 0,
/// program 0) and a drum kit (bank 128, program 0). The contract suite puts
/// it in the `soundfonts` folder of the plugins it checks; this function
/// makes it again (see the test that compares the two).
#[cfg(test)]
pub(crate) fn test_set() -> Vec<u8> {
    let period = 100usize;
    let data: Vec<i16> = (0..period * 48)
        .map(|i| {
            let phase = std::f64::consts::TAU * (i % period) as f64 / period as f64;
            (phase.cos() * 16_000.0).round() as i16
        })
        .collect();
    let len = data.len() as u32;
    // 480 Hz is key 70.506: 49 cents flat of key 71, which a correction
    // of +49 cents makes up.
    let sine = Sample {
        name: "Sine 480 Hz".into(),
        data,
        rate: 48_000,
        root: 71,
        correction: 49,
        looped: Some((0, len)),
    };
    let zone = Zone {
        // No delay and no attack: SoundFont 2's defaults (−12000
        // timecents, about 1 ms each) would start every note a little
        // after its frame, and the suite checks it starts on time.
        generators: vec![
            (generator::DELAY_VOL_ENV, -32768),
            (generator::ATTACK_VOL_ENV, -32768),
            (generator::SAMPLE_MODES, 1),
            (generator::RELEASE_VOL_ENV, -6000),
        ],
        target: 0,
        ..Zone::default()
    };
    Sf2 {
        name: "OxiGuitar test sine".into(),
        samples: vec![sine],
        instruments: vec![Instrument {
            name: "Sine".into(),
            zones: vec![zone],
        }],
        presets: vec![
            Preset {
                name: "Sine".into(),
                bank: 0,
                program: 0,
                zones: vec![Zone::default()],
            },
            Preset {
                name: "Sine kit".into(),
                bank: 128,
                program: 0,
                zones: vec![Zone::default()],
            },
        ],
    }
    .write()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustysynth_reads_what_we_write() {
        let bytes = test_set();
        let font = rustysynth::SoundFont::new(&mut std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(font.get_presets().len(), 2);
        assert_eq!(font.get_instruments().len(), 1);
        let s = &font.get_sample_headers()[0];
        assert_eq!((s.get_start(), s.get_end()), (0, 4800));
        assert_eq!((s.get_start_loop(), s.get_end_loop()), (0, 4800));
        assert_eq!((s.get_original_pitch(), s.get_pitch_correction()), (71, 49));
        assert_eq!(font.get_wave_data().len(), 4800 + PAD);
        assert_eq!(font.get_wave_data()[0], 16_000);
    }

    #[test]
    fn the_contract_suites_test_set_is_this_one() {
        // `OXIGUITAR_BLESS=1 cargo test -p oxiguitar-soundfont-plugin` writes it.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/oxiguitar-plugin-test/src/test-sine.sf2"
        );
        let made = test_set();
        if std::env::var_os("OXIGUITAR_BLESS").is_some() {
            std::fs::write(path, &made).unwrap();
        }
        assert!(
            std::fs::read(path).is_ok_and(|kept| kept == made),
            "{path} is out of date: run with OXIGUITAR_BLESS=1"
        );
    }
}
