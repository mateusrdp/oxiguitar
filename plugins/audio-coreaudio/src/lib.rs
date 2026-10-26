// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! OxiGuitar's audio-device plugin for Core Audio (plugin interface 1.3): the
//! service `audio-device` `coreaudio`, through cpal's Core Audio host
//! (`oxiguitar-audio-cpal`). Built only for macos; elsewhere the crate is
//! a plugin whose service is never available, so the workspace builds on
//! every system.

use oxiguitar_plugin::{Availability, Error, Host, Plugin, Service};

/// The plugin.
#[derive(Debug)]
pub struct Audio {
    #[cfg(target_os = "macos")]
    device: oxiguitar_audio_cpal::CpalDevice,
}

impl Plugin for Audio {
    fn start(host: Host) -> Result<Audio, Error> {
        #[cfg(not(target_os = "macos"))]
        let _ = host;
        Ok(Audio {
            #[cfg(target_os = "macos")]
            device: oxiguitar_audio_cpal::CpalDevice::new(
                host,
                cpal::HostId::CoreAudio,
                "Core Audio",
            ),
        })
    }

    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        (interface == "audio-device" && id == "coreaudio").then_some(self as &dyn Service)
    }
}

impl Service for Audio {
    fn probe(&self) -> Availability {
        #[cfg(target_os = "macos")]
        return self.device.probe();
        #[cfg(not(target_os = "macos"))]
        Availability::Unavailable("it runs only on macos".into())
    }

    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        #[cfg(target_os = "macos")]
        return self.device.call(method, input);
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (method, input);
            Err(Error::new(
                oxiguitar_plugin::ErrorKind::Unsupported,
                "it runs only on macos",
            ))
        }
    }
}

oxiguitar_plugin::export_plugin!(Audio);
