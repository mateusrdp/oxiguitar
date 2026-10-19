// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Any file, as a format plugin answers the host's request to read it: the
//! SDK's `dispatch` with each of our five plugins, then the answer as it
//! crosses the interface (encoded, decoded) and the score converted back
//! into the model, as the core does. A file a plugin refuses is fine; a
//! crash anywhere on the way is not. Seeds: every public fixture.
//!
//! Built here as libraries, our plugins share one copy of
//! `oxiguitar-formats` with every reader in it; that each `.oxp` holds only
//! its own reader is checked by oxiguitar-plugin-test.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxiguitar_plugin::api::Blob;
use oxiguitar_plugin::api::message::{Answer, Request};
use oxiguitar_plugin::{Host, Plugin, dispatch};

fn read(plugin: &dyn Plugin, format: &str, data: &[u8]) {
    let request = Request::Read {
        format: format.to_owned(),
        data: Blob::borrowed(data),
    };
    let Ok(answer) = dispatch(plugin, request) else {
        return;
    };
    let bytes = oxiguitar_plugin::api::encode(&answer).expect("an answer encodes");
    let decoded: Answer<'_> = oxiguitar_plugin::api::decode(&bytes).expect("an answer decodes");
    // Compared as bytes, so a NaN in the score doesn't count as a change.
    let again = oxiguitar_plugin::api::encode(&decoded).expect("an answer encodes");
    assert!(again == bytes, "the answer crosses unchanged");
    if let Answer::Read { score, .. } = decoded {
        oxiguitar_model::wire::from_wire(*score).expect("the core takes the score");
    }
}

fuzz_target!(|data: &[u8]| {
    read(&oxiguitar_gp3_plugin::Gp3::start(Host::detached()).unwrap(), "gp3", data);
    read(&oxiguitar_gp4_plugin::Gp4::start(Host::detached()).unwrap(), "gp4", data);
    read(&oxiguitar_gp5_plugin::Gp5::start(Host::detached()).unwrap(), "gp5", data);
    read(&oxiguitar_gpx_plugin::Gpx::start(Host::detached()).unwrap(), "gpx", data);
    read(&oxiguitar_gp_plugin::Gp::start(Host::detached()).unwrap(), "gp", data);
});
