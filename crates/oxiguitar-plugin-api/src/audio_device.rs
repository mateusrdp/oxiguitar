// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The `audio-device` service interface (1.3): a plugin that sends the
//! core's sound to the system (PipeWire, ALSA, WASAPI, CoreAudio).
//!
//! The host chooses one service of this interface (the one settings.json
//! pins under `services`, else the highest priority whose probe says it
//! can work) and calls its methods with postcard-encoded inputs:
//!
//! - [`OPEN`] with an [`Open`]: open a stream on the system's default
//!   output (or the device named), at the rate asked for if the device
//!   takes it, and start it. Answered with an [`Opened`]. From then on the
//!   device's callback calls the host table's `render` with the stream's
//!   number, for every buffer, until the stream is closed.
//! - [`CLOSE`] with the stream's number (a `u64`): stop and close it.
//!   After it returns, the plugin never calls `render` for it again.
//! - [`DEVICES`] with nothing: the names of the outputs, as a `Vec<String>`.
//!
//! A stream that breaks (the device unplugged, the sound server gone) is
//! said in the footer by the plugin, and the host closes it.

use serde::{Deserialize, Serialize};

/// The interface's name, as manifests write it.
pub const INTERFACE: &str = "audio-device";

/// Opens a stream: input [`Open`], output [`Opened`].
pub const OPEN: &str = "open";
/// Closes a stream: input its number (`u64`), output nothing.
pub const CLOSE: &str = "close";
/// Lists the outputs: no input, output `Vec<String>`.
pub const DEVICES: &str = "devices";

/// What the host asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Open {
    /// The stream's number, which the device's `render` calls give back.
    pub stream: u64,
    /// The rate wanted, in frames a second: 48 000 (NAM's).
    pub rate: u32,
    /// The frames per buffer wanted, or 0 for the device's own.
    pub frames: u32,
    /// The device's name, or empty for the system's default output.
    pub device: String,
}

/// What the device opened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opened {
    /// The rate it runs at: the one asked for, or the device's own when it
    /// refuses that one.
    pub rate: u32,
    /// Its channels (interleaved in `render`'s buffer).
    pub channels: u32,
    /// The frames per buffer, or 0 when they vary.
    pub frames: u32,
    /// The output's name, for the user.
    pub device: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inputs_round_trip() {
        let open = Open {
            stream: 3,
            rate: 48_000,
            frames: 0,
            device: String::new(),
        };
        assert_eq!(
            crate::decode::<Open>(&crate::encode(&open).unwrap()).unwrap(),
            open
        );
        let opened = Opened {
            rate: 44_100,
            channels: 2,
            frames: 512,
            device: "Built-in".into(),
        };
        assert_eq!(
            crate::decode::<Opened>(&crate::encode(&opened).unwrap()).unwrap(),
            opened
        );
    }
}
