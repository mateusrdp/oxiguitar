// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's WAV plugin (plugin interface 1.4; M4 slice 7, PLAY-7):
//! File → Export → WAV Audio. OxiGuitar renders the score and hands the
//! sound over in pieces; this plugin writes it as a WAV file's bytes,
//! which OxiGuitar writes to the file.
//!
//! - Its settings window picks the bit depth, 16 or 24 bits or 32-bit
//!   float, and the rate, 44.1 or 48 kHz (Mateus, 1 Nov 2026); 24 bits at
//!   48 kHz until the user picks.
//! - 16 bits get triangular dither, so quiet passages fade into noise
//!   rather than into steps; 24 bits and float are written as they are,
//!   clipped at full scale (float isn't clipped).
//! - The header names the length, so the one written first has zero there
//!   and OxiGuitar writes the one handed back at the end over it.
//! - A WAV file holds at most 4 GiB of sound (its sizes are 32 bits): about
//!   3 h 40 min at 24 bits and 48 kHz. Past it the export fails, saying so.

use std::sync::Mutex;

use oxiguitar_plugin::serde_json::{Map, Value};
use oxiguitar_plugin::{
    AudioExporter, Error, ErrorKind, ExportEnd, ExportStart, Host, Plugin, Row, Tab, TabContent,
    TabEvent, Widget,
};

/// The exporter's id in the manifest.
pub const EXPORTER: &str = "wav";

/// What a WAV file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// 16-bit integers, as CDs.
    Sixteen,
    /// 24-bit integers.
    TwentyFour,
    /// 32-bit floating point.
    Float,
}

impl Depth {
    /// Every depth, in the settings window's order.
    pub const ALL: [Depth; 3] = [Depth::Sixteen, Depth::TwentyFour, Depth::Float];

    /// Bytes a sample.
    pub fn bytes(self) -> u16 {
        match self {
            Depth::Sixteen => 2,
            Depth::TwentyFour => 3,
            Depth::Float => 4,
        }
    }

    /// The settings' value.
    fn key(self) -> u64 {
        match self {
            Depth::Sixteen => 16,
            Depth::TwentyFour => 24,
            Depth::Float => 32,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Depth::Sixteen => "16-bit (CD; smallest files)",
            Depth::TwentyFour => "24-bit (recommended)",
            Depth::Float => "32-bit float (for further mixing)",
        }
    }
}

/// The rates offered.
pub const RATES: [(u32, &str); 2] = [(44_100, "44.1 kHz (CD)"), (48_000, "48 kHz (recommended)")];

/// What the settings say, or the defaults: 24 bits at 48 kHz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The bit depth.
    pub depth: Depth,
    /// The rate, in frames a second.
    pub rate: u32,
}

impl Default for Format {
    fn default() -> Format {
        Format {
            depth: Depth::TwentyFour,
            rate: 48_000,
        }
    }
}

impl Format {
    /// The format the plugin's settings name; what they don't name (or
    /// name wrongly) is the default.
    pub fn from_settings(settings: &Map<String, Value>) -> Format {
        let default = Format::default();
        let depth = settings
            .get("depth")
            .and_then(Value::as_u64)
            .and_then(|d| Depth::ALL.into_iter().find(|x| x.key() == d))
            .unwrap_or(default.depth);
        let rate = settings
            .get("rate")
            .and_then(Value::as_u64)
            .and_then(|r| RATES.iter().find(|(x, _)| u64::from(*x) == r))
            .map_or(default.rate, |(r, _)| *r);
        Format { depth, rate }
    }

    /// A WAV header for `frames` stereo frames in this format: 44 bytes
    /// for integers; 58 for float (an 18-byte format and a `fact` chunk, as
    /// the format asks for samples that aren't integers).
    pub fn header(self, frames: u64) -> Vec<u8> {
        let float = self.depth == Depth::Float;
        let block = 2 * u32::from(self.depth.bytes());
        let data = (frames * u64::from(block)).min(u64::from(u32::MAX)) as u32;
        let format_len: u32 = if float { 18 } else { 16 };
        let fact_len: u32 = if float { 12 } else { 0 };
        let riff = 4 + (8 + format_len) + fact_len + 8 + data;
        let mut h = Vec::with_capacity(58);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&riff.to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&format_len.to_le_bytes());
        h.extend_from_slice(&(if float { 3u16 } else { 1 }).to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&self.rate.to_le_bytes());
        h.extend_from_slice(&(self.rate * block).to_le_bytes());
        h.extend_from_slice(&(block as u16).to_le_bytes());
        h.extend_from_slice(&(self.depth.bytes() * 8).to_le_bytes());
        if float {
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(b"fact");
            h.extend_from_slice(&4u32.to_le_bytes());
            h.extend_from_slice(&(frames.min(u64::from(u32::MAX)) as u32).to_le_bytes());
        }
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        h
    }
}

/// The most bytes of sound a WAV file holds: its sizes are 32 bits, and
/// the header counts too.
const MOST: u64 = u32::MAX as u64 - 64;

/// An export under way.
#[derive(Debug)]
struct Export {
    format: Format,
    frames: u64,
    /// The dither's generator (xorshift), the same for every export so a
    /// file comes out the same twice.
    noise: u32,
}

impl Export {
    fn next(&mut self) -> f32 {
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        x as f32 / u32::MAX as f32
    }

    /// `v` as a 16-bit sample, with triangular dither of one step.
    fn sixteen(&mut self, v: f32) -> i16 {
        let dither = self.next() - self.next();
        (v * 32_767.0 + dither).round().clamp(-32_768.0, 32_767.0) as i16
    }
}

/// The exporter `wav`.
#[derive(Debug)]
pub struct Wav {
    host: Host,
    export: Mutex<Option<Export>>,
}

impl Wav {
    fn export(&self) -> std::sync::MutexGuard<'_, Option<Export>> {
        self.export.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl AudioExporter for Wav {
    fn start(&self) -> Result<ExportStart, Error> {
        let format = Format::from_settings(&self.host.settings().unwrap_or_default());
        *self.export() = Some(Export {
            format,
            frames: 0,
            noise: 0x9E37_79B9,
        });
        Ok(ExportStart {
            rate: format.rate,
            data: format.header(0),
        })
    }

    fn samples(&self, interleaved: &[f32]) -> Result<Vec<u8>, Error> {
        let mut guard = self.export();
        let Some(export) = guard.as_mut() else {
            return Err(Error::new(ErrorKind::Failed, "no export was started"));
        };
        let depth = export.format.depth;
        let frames = (interleaved.len() / 2) as u64;
        if (export.frames + frames) * 2 * u64::from(depth.bytes()) > MOST {
            return Err(Error::new(
                ErrorKind::Failed,
                "a WAV file holds at most 4 GB of sound: export fewer bars, or at 16 bits",
            ));
        }
        export.frames += frames;
        let mut out = Vec::with_capacity(interleaved.len() * usize::from(depth.bytes()));
        for &v in interleaved {
            match depth {
                Depth::Sixteen => out.extend_from_slice(&export.sixteen(v).to_le_bytes()),
                Depth::TwentyFour => {
                    let s = (v.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
                    out.extend_from_slice(&s.to_le_bytes()[..3]);
                }
                Depth::Float => out.extend_from_slice(&v.to_le_bytes()),
            }
        }
        Ok(out)
    }

    fn finish(&self, cancelled: bool) -> Result<ExportEnd, Error> {
        let export = self
            .export()
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Failed, "no export was started"))?;
        Ok(ExportEnd {
            data: Vec::new(),
            start: (!cancelled).then(|| export.format.header(export.frames)),
        })
    }
}

/// The settings window: the bit depth and the rate.
#[derive(Debug)]
pub struct Window {
    host: Host,
}

impl Tab for Window {
    fn content(&self) -> TabContent {
        let format = Format::from_settings(&self.host.settings().unwrap_or_default());
        let row = |label: &str, widget: Widget| Row {
            widgets: vec![
                Widget::Label {
                    text: label.into(),
                    muted: false,
                },
                widget,
            ],
        };
        TabContent {
            rows: vec![
                row(
                    "Bit depth",
                    Widget::Choice {
                        id: "depth".into(),
                        options: Depth::ALL.iter().map(|d| d.title().to_owned()).collect(),
                        selected: Depth::ALL
                            .iter()
                            .position(|d| *d == format.depth)
                            .unwrap_or(1) as u32,
                        enabled: true,
                    },
                ),
                row(
                    "Rate",
                    Widget::Choice {
                        id: "rate".into(),
                        options: RATES.iter().map(|(_, t)| (*t).to_owned()).collect(),
                        selected: RATES
                            .iter()
                            .position(|(r, _)| *r == format.rate)
                            .unwrap_or(1) as u32,
                        enabled: true,
                    },
                ),
                Row {
                    widgets: vec![Widget::Label {
                        text:
                            "For File → Export → WAV Audio. Every program and player opens these."
                                .into(),
                        muted: true,
                    }],
                },
            ],
        }
    }

    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let TabEvent::Chosen(i) = event else {
            return Ok(());
        };
        let i = i as usize;
        let value = match widget {
            "depth" => Depth::ALL.get(i).map(|d| d.key()),
            "rate" => RATES.get(i).map(|(r, _)| u64::from(*r)),
            _ => return Err(Error::not_found(format!("no widget {widget:?}"))),
        }
        .ok_or_else(|| Error::new(ErrorKind::Failed, format!("no choice {i}")))?;
        let mut settings = self.host.settings()?;
        settings.insert(widget.into(), value.into());
        self.host.set_settings(&settings)
    }
}

/// The plugin.
#[derive(Debug)]
pub struct WavPlugin {
    wav: Wav,
    window: Window,
}

impl Plugin for WavPlugin {
    fn start(host: Host) -> Result<WavPlugin, Error> {
        Ok(WavPlugin {
            wav: Wav {
                host: host.clone(),
                export: Mutex::default(),
            },
            window: Window { host },
        })
    }

    fn audio_exporter(&self, id: &str) -> Option<&dyn AudioExporter> {
        (id == EXPORTER).then_some(&self.wav as &dyn AudioExporter)
    }

    fn settings_window(&self) -> Option<&dyn Tab> {
        Some(&self.window)
    }
}

oxiguitar_plugin::export_plugin!(WavPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_plugin::serde_json::json;

    fn settings(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn the_format_comes_from_the_settings_or_the_defaults() {
        assert_eq!(Format::from_settings(&Map::new()), Format::default());
        assert_eq!(
            Format::from_settings(&settings(json!({"depth": 16, "rate": 44100}))),
            Format {
                depth: Depth::Sixteen,
                rate: 44_100
            }
        );
        // What isn't offered is the default.
        assert_eq!(
            Format::from_settings(&settings(json!({"depth": 20, "rate": 96000}))),
            Format::default()
        );
    }

    #[test]
    fn headers_name_the_format_and_the_length() {
        let h = Format::default().header(48_000);
        assert_eq!(h.len(), 44);
        assert_eq!(&h[..4], b"RIFF");
        assert_eq!(&h[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([h[20], h[21]]), 1, "PCM");
        assert_eq!(u16::from_le_bytes([h[22], h[23]]), 2, "stereo");
        assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 48_000);
        assert_eq!(
            u32::from_le_bytes(h[28..32].try_into().unwrap()),
            48_000 * 6
        );
        assert_eq!(u16::from_le_bytes([h[32], h[33]]), 6);
        assert_eq!(u16::from_le_bytes([h[34], h[35]]), 24);
        assert_eq!(&h[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes(h[40..44].try_into().unwrap()),
            48_000 * 6
        );
        assert_eq!(
            u32::from_le_bytes(h[4..8].try_into().unwrap()),
            36 + 48_000 * 6
        );

        let f = Format {
            depth: Depth::Float,
            rate: 44_100,
        }
        .header(10);
        assert_eq!(f.len(), 58);
        assert_eq!(u16::from_le_bytes([f[20], f[21]]), 3, "IEEE float");
        assert_eq!(&f[38..42], b"fact");
        assert_eq!(u32::from_le_bytes(f[46..50].try_into().unwrap()), 10);
        assert_eq!(&f[50..54], b"data");
        assert_eq!(u32::from_le_bytes(f[54..58].try_into().unwrap()), 80);
        assert_eq!(u32::from_le_bytes(f[4..8].try_into().unwrap()), 58 - 8 + 80);
        // The header at the start and the one at the end are as long.
        assert_eq!(
            f.len(),
            Format {
                depth: Depth::Float,
                rate: 44_100
            }
            .header(0)
            .len()
        );
    }

    fn wav(format: Format) -> Wav {
        let w = Wav {
            host: Host::detached(),
            export: Mutex::default(),
        };
        *w.export() = Some(Export {
            format,
            frames: 0,
            noise: 0x9E37_79B9,
        });
        w
    }

    #[test]
    fn samples_become_bytes_at_each_depth() {
        let w = wav(Format::default());
        let b = w.samples(&[1.0, -1.0, 2.0, 0.0]).unwrap();
        assert_eq!(b.len(), 12);
        assert_eq!(&b[0..3], &[0xFF, 0xFF, 0x7F], "full scale");
        assert_eq!(&b[3..6], &[0x01, 0x00, 0x80], "minus full scale");
        assert_eq!(&b[6..9], &[0xFF, 0xFF, 0x7F], "louder is clipped");
        assert_eq!(&b[9..12], &[0, 0, 0]);
        let end = w.finish(false).unwrap();
        let h = end.start.unwrap();
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 12);
        assert!(w.finish(false).is_err(), "nothing under way any more");

        let w = wav(Format {
            depth: Depth::Float,
            rate: 48_000,
        });
        let b = w.samples(&[1.5, -0.25]).unwrap();
        assert_eq!(f32::from_le_bytes(b[0..4].try_into().unwrap()), 1.5);
        assert_eq!(f32::from_le_bytes(b[4..8].try_into().unwrap()), -0.25);
        assert_eq!(w.finish(true).unwrap().start, None, "cancelled");

        // Dither of one step at most, around the value.
        let w = wav(Format {
            depth: Depth::Sixteen,
            rate: 48_000,
        });
        let quiet: Vec<f32> = vec![0.25; 2000];
        let b = w.samples(&quiet).unwrap();
        let values: Vec<i16> = b
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert!(values.iter().all(|v| (*v - 8192).abs() <= 1), "{values:?}");
        let mean = values.iter().map(|v| f64::from(*v)).sum::<f64>() / values.len() as f64;
        assert!((mean - 8191.75).abs() < 0.1, "{mean}");
    }

    #[test]
    fn past_four_gigabytes_the_export_stops() {
        let w = wav(Format::default());
        w.export().as_mut().unwrap().frames = MOST / 6 - 1;
        assert!(w.samples(&[0.0, 0.0]).is_ok());
        let e = w.samples(&[0.0; 4]).unwrap_err();
        assert!(e.message.contains("4 GB"), "{}", e.message);
    }
}
