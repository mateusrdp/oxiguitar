// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's audio-device plugin for WASAPI (plugin interface 1.3): the
//! service `audio-device` `wasapi`, through cpal's WASAPI host
//! (`oxiguitar-audio-cpal`). Built only for windows; elsewhere the crate is
//! a plugin whose service is never available, so the workspace builds on
//! every system.

use oxiguitar_plugin::{Availability, Error, Host, Plugin, Service};

/// The plugin.
#[derive(Debug)]
pub struct Audio {
    #[cfg(target_os = "windows")]
    device: oxiguitar_audio_cpal::CpalDevice,
}

impl Plugin for Audio {
    fn start(host: Host) -> Result<Audio, Error> {
        #[cfg(not(target_os = "windows"))]
        let _ = host;
        Ok(Audio {
            #[cfg(target_os = "windows")]
            device: oxiguitar_audio_cpal::CpalDevice::new(host, cpal::HostId::Wasapi, "WASAPI"),
        })
    }

    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        (interface == "audio-device" && id == "wasapi").then_some(self as &dyn Service)
    }
}

impl Service for Audio {
    fn probe(&self) -> Availability {
        #[cfg(target_os = "windows")]
        return self.device.probe();
        #[cfg(not(target_os = "windows"))]
        Availability::Unavailable("it runs only on windows".into())
    }

    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        #[cfg(target_os = "windows")]
        return self.device.call(method, input);
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (method, input);
            Err(Error::new(
                oxiguitar_plugin::ErrorKind::Unsupported,
                "it runs only on windows",
            ))
        }
    }
}

oxiguitar_plugin::export_plugin!(Audio);
