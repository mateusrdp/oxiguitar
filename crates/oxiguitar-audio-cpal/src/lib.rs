// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The `audio-device` service (plugin interface 1.3) on one of cpal's
//! hosts: what OxiGuitar's four audio-device plugins (PipeWire, ALSA,
//! WASAPI, CoreAudio) share. Each plugin names its host and adds nothing
//! else.
//!
//! Streams live on a thread of their own (a cpal stream need not move
//! between threads), which sleeps until asked to open or close one. A
//! stream's callback hands the core its buffer (`AudioOut::render`) with
//! the delay to the speaker that cpal reports for it; the callback itself
//! allocates nothing and waits for nothing.
//!
//! The rate: 48 kHz when the device takes it (NAM's), else the device's
//! own; f32 frames when the device takes them, else 16- or 32-bit integers
//! converted through a buffer made when the stream opens.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{HostId, SampleFormat, StreamConfig};
use oxiguitar_plugin::api::audio_device::{CLOSE, DEVICES, OPEN, Open, Opened};
use oxiguitar_plugin::api::{decode, encode};
use oxiguitar_plugin::{Availability, Error, ErrorKind, Host, Level, Service};

/// The most frames a callback converts at once from f32 (for devices that
/// take integers): larger buffers are converted in pieces.
const CONVERT: usize = 8192;

/// An audio-device service on cpal's host `id`.
pub struct CpalDevice {
    id: HostId,
    name: &'static str,
    host: Host,
    jobs: Mutex<Option<Sender<Job>>>,
}

/// What the streams' thread is asked.
enum Job {
    Open(Open, Sender<Result<Opened, Error>>),
    Close(u64, Sender<()>),
}

impl std::fmt::Debug for CpalDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpalDevice")
            .field("host", &self.name)
            .finish()
    }
}

impl CpalDevice {
    /// The service on cpal's host `id`, called `name` in messages.
    pub fn new(host: Host, id: HostId, name: &'static str) -> CpalDevice {
        CpalDevice {
            id,
            name,
            host,
            jobs: Mutex::new(None),
        }
    }

    fn cpal_host(&self) -> Result<cpal::Host, Error> {
        cpal::host_from_id(self.id).map_err(|e| {
            Error::new(
                ErrorKind::Unsupported,
                format!("{} isn't available here ({e})", self.name),
            )
        })
    }

    /// The streams' thread, started on first use.
    fn jobs(&self) -> Result<Sender<Job>, Error> {
        let mut jobs = self
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(j) = jobs.as_ref() {
            return Ok(j.clone());
        }
        let (tx, rx) = channel();
        let host = self.host.clone();
        let id = self.id;
        let name = self.name;
        std::thread::Builder::new()
            .name(format!("oxiguitar-{}", name.to_lowercase()))
            .spawn(move || keep_streams(&rx, &host, id, name))
            .map_err(|e| {
                Error::new(ErrorKind::Failed, format!("no thread for the streams: {e}"))
            })?;
        *jobs = Some(tx.clone());
        Ok(tx)
    }
}

impl Service for CpalDevice {
    fn probe(&self) -> Availability {
        match self.cpal_host() {
            Err(e) => Availability::Unavailable(e.message),
            Ok(host) => match host.default_output_device() {
                Some(_) => Availability::Available,
                None => Availability::Unavailable(format!("{} has no output", self.name)),
            },
        }
    }

    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        let invalid = |e: oxiguitar_plugin::api::DecodeError| {
            Error::new(ErrorKind::Invalid, format!("the input can't be read ({e})"))
        };
        match method {
            OPEN => {
                let open: Open = decode(input).map_err(invalid)?;
                let (tx, rx) = channel();
                self.jobs()?.send(Job::Open(open, tx)).map_err(|_| gone())?;
                let opened = rx.recv().map_err(|_| gone())??;
                encode(&opened).map_err(unencoded)
            }
            CLOSE => {
                let stream: u64 = decode(input).map_err(invalid)?;
                let (tx, rx) = channel();
                self.jobs()?
                    .send(Job::Close(stream, tx))
                    .map_err(|_| gone())?;
                rx.recv().map_err(|_| gone())?;
                Ok(Vec::new())
            }
            DEVICES => {
                let host = self.cpal_host()?;
                let names: Vec<String> = host
                    .output_devices()
                    .map(|d| d.filter_map(|d| device_name(&d)).collect())
                    .unwrap_or_default();
                encode(&names).map_err(unencoded)
            }
            _ => Err(Error::not_found(format!("no method {method:?}"))),
        }
    }
}

/// An answer that couldn't be encoded.
fn unencoded(e: oxiguitar_plugin::api::DecodeError) -> Error {
    Error::new(ErrorKind::Failed, e.to_string())
}

fn gone() -> Error {
    Error::new(ErrorKind::Failed, "the audio streams' thread has ended")
}

fn device_name(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|d| d.name().to_owned())
}

/// The streams' thread: sleeps until asked, opens and closes streams.
fn keep_streams(jobs: &Receiver<Job>, host: &Host, id: HostId, name: &'static str) {
    let mut streams: Vec<(u64, cpal::Stream)> = Vec::new();
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Open(open, reply) => {
                let result = open_stream(host, id, name, &open).map(|(stream, opened)| {
                    streams.push((open.stream, stream));
                    opened
                });
                let _ = reply.send(result);
            }
            Job::Close(n, reply) => {
                // Dropping a cpal stream stops it: its callback isn't called
                // again once this returns.
                streams.retain(|(s, _)| *s != n);
                let _ = reply.send(());
            }
        }
    }
}

fn failed(what: &str, e: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::Failed, format!("{what} ({e})"))
}

fn open_stream(
    host: &Host,
    id: HostId,
    name: &'static str,
    open: &Open,
) -> Result<(cpal::Stream, Opened), Error> {
    let out = host
        .audio_out()
        .ok_or_else(|| Error::new(ErrorKind::Unsupported, "this OxiGuitar can't play sound"))?;
    let cpal_host =
        cpal::host_from_id(id).map_err(|e| failed(&format!("{name} isn't available"), e))?;
    let device = if open.device.is_empty() {
        // The system's default output, or the first that can be opened
        // (a machine whose default names no card).
        cpal_host
            .default_output_device()
            .filter(|d| d.default_output_config().is_ok())
            .or_else(|| {
                cpal_host
                    .output_devices()
                    .ok()?
                    .find(|d| d.default_output_config().is_ok())
            })
    } else {
        cpal_host
            .output_devices()
            .ok()
            .and_then(|mut d| d.find(|d| device_name(d).as_deref() == Some(open.device.as_str())))
    }
    .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("{name} has no such output")))?;
    let default = device
        .default_output_config()
        .map_err(|e| failed("the output gives no configuration", e))?;
    // The rate asked for when the device takes it, in f32 if it can; else
    // the device's own.
    let wanted = device.supported_output_configs().ok().and_then(|configs| {
        let fits: Vec<_> = configs
            .filter(|c| c.min_sample_rate() <= open.rate && open.rate <= c.max_sample_rate())
            .collect();
        fits.iter()
            .find(|c| c.sample_format() == SampleFormat::F32 && c.channels() >= 2)
            .or_else(|| fits.iter().find(|c| c.sample_format() == SampleFormat::F32))
            .or_else(|| fits.first())
            .map(|c| c.with_sample_rate(open.rate))
    });
    let chosen = wanted.unwrap_or(default);
    let format = chosen.sample_format();
    let config: StreamConfig = chosen.config();
    let channels = usize::from(config.channels);
    let stream_no = open.stream;
    let errors = host.clone();
    let on_error = move |e: cpal::Error| {
        errors.notify(Level::Error, format!("{name}: the sound stopped ({e})."));
    };
    let delay = |info: &cpal::OutputCallbackInfo| {
        let ts = info.timestamp();
        u64::try_from(ts.playback.duration_since(ts.callback).as_nanos()).unwrap_or(u64::MAX)
    };
    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                out.render(stream_no, data, channels, delay(info));
            },
            on_error,
            None,
        ),
        SampleFormat::I16 => {
            let mut scratch = vec![0.0f32; CONVERT * channels];
            device.build_output_stream(
                config,
                move |data: &mut [i16], info: &cpal::OutputCallbackInfo| {
                    let d = delay(info);
                    for chunk in data.chunks_mut(CONVERT * channels) {
                        let s = &mut scratch[..chunk.len()];
                        out.render(stream_no, s, channels, d);
                        for (o, v) in chunk.iter_mut().zip(s.iter()) {
                            *o = (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                        }
                    }
                },
                on_error,
                None,
            )
        }
        SampleFormat::I32 => {
            let mut scratch = vec![0.0f32; CONVERT * channels];
            device.build_output_stream(
                config,
                move |data: &mut [i32], info: &cpal::OutputCallbackInfo| {
                    let d = delay(info);
                    for chunk in data.chunks_mut(CONVERT * channels) {
                        let s = &mut scratch[..chunk.len()];
                        out.render(stream_no, s, channels, d);
                        for (o, v) in chunk.iter_mut().zip(s.iter()) {
                            *o = (f64::from(v.clamp(-1.0, 1.0)) * f64::from(i32::MAX)) as i32;
                        }
                    }
                },
                on_error,
                None,
            )
        }
        other => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("the output takes {other} samples, which OxiGuitar doesn't make"),
            ));
        }
    }
    .map_err(|e| failed("the output couldn't be opened", e))?;
    stream
        .play()
        .map_err(|e| failed("the output couldn't be started", e))?;
    let opened = Opened {
        rate: config.sample_rate,
        channels: u32::from(config.channels),
        frames: match config.buffer_size {
            cpal::BufferSize::Fixed(n) => n,
            cpal::BufferSize::Default => 0,
        },
        device: device_name(&device).unwrap_or_else(|| name.to_owned()),
    };
    Ok((stream, opened))
}
