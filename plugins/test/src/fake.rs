// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The audio device `fake` (interface 1.3): an output with no speaker, so
//! the contract tests can drive a whole stream (the host's `render`, the
//! engine, an instrument) without a sound card.
//!
//! `open` starts a thread that asks the core for [`BUFFERS`] buffers of
//! [`FRAMES`] stereo frames, as fast as it can, as a device's callback
//! would, then stops. `close` waits for it. `totals` answers
//! `frames rendered, sum of every sample's size` as text, once the thread
//! is done; `devices` names the one output, `Nowhere`.

use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use oxiguitar_plugin::api::audio_device::{CLOSE, DEVICES, OPEN, Open, Opened};
use oxiguitar_plugin::{Error, ErrorKind, Host, Service};

/// The frames in each buffer.
pub const FRAMES: usize = 256;
/// The buffers each stream asks for.
pub const BUFFERS: usize = 200;

/// The device.
#[derive(Debug)]
pub struct Fake {
    host: Host,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    thread: Option<(u64, JoinHandle<(u64, f64)>)>,
    totals: Option<(u64, f64)>,
}

impl Fake {
    /// The device, asking `host` for its buffers.
    pub fn new(host: Host) -> Fake {
        Fake {
            host,
            state: Mutex::default(),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn join(&self, stream: Option<u64>) {
        let mut state = self.state();
        if state
            .thread
            .as_ref()
            .is_some_and(|(id, _)| stream.is_none_or(|s| s == *id))
            && let Some((_, thread)) = state.thread.take()
        {
            state.totals = thread.join().ok();
        }
    }
}

fn failed(e: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::Failed, e.to_string())
}

impl Service for Fake {
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        match method {
            OPEN => {
                let open: Open = oxiguitar_plugin::api::decode(input).map_err(failed)?;
                let out = self
                    .host
                    .audio_out()
                    .ok_or_else(|| failed("the host has no render (interface 1.3)"))?;
                self.join(None);
                let stream = open.stream;
                let thread = std::thread::spawn(move || {
                    let mut buffer = vec![0.0f32; FRAMES * 2];
                    let (mut frames, mut sum) = (0u64, 0.0f64);
                    for _ in 0..BUFFERS {
                        out.render(stream, &mut buffer, 2, 0);
                        frames += FRAMES as u64;
                        sum += buffer.iter().map(|v| f64::from(v.abs())).sum::<f64>();
                    }
                    (frames, sum)
                });
                let mut state = self.state();
                state.thread = Some((stream, thread));
                state.totals = None;
                oxiguitar_plugin::api::encode(&Opened {
                    rate: open.rate,
                    channels: 2,
                    frames: FRAMES as u32,
                    device: "Nowhere".into(),
                })
                .map_err(failed)
            }
            CLOSE => {
                let stream: u64 = oxiguitar_plugin::api::decode(input).map_err(failed)?;
                self.join(Some(stream));
                Ok(Vec::new())
            }
            DEVICES => oxiguitar_plugin::api::encode(&vec!["Nowhere".to_owned()]).map_err(failed),
            "totals" => {
                self.join(None);
                let (frames, sum) = self
                    .state()
                    .totals
                    .ok_or_else(|| failed("no stream has played"))?;
                Ok(format!("{frames} {sum}").into_bytes())
            }
            _ => Err(Error::not_found(format!("no method {method:?}"))),
        }
    }
}
