// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An example OxiGuitar plugin, to copy and change.
//!
//! It adds three things, each in its own module so you can delete the ones
//! you don't need (and their entries in `manifest.json`):
//!
//! - [`format`]: reads and writes "Example tab" files (`.extab`), a tiny
//!   text format of eighth notes on a guitar;
//! - [`statistics`]: a tab in the right area counting the notes on each
//!   string of a track of the open score, as a small chart;
//! - [`service`]: the service `note-names`, which other plugins call to
//!   name a pitch.
//!
//! [`notes`] holds note names and standard tuning.

pub mod format;
pub mod notes;
pub mod service;
pub mod statistics;

use oxiguitar_plugin::{Error, FormatReader, FormatWriter, Host, Plugin, Service, Tab};

// ANCHOR: plugin
/// The plugin: built once, when OxiGuitar first uses one of its
/// contributions, and kept until the program ends.
pub struct Example {
    statistics: statistics::Statistics,
}

impl Plugin for Example {
    fn start(host: Host) -> Result<Example, Error> {
        Ok(Example {
            statistics: statistics::Statistics::new(host),
        })
    }

    // Each method hands out a contribution the manifest declares, by its id.

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == format::ID).then_some(&format::ExampleTab as &dyn FormatReader)
    }

    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        (format == format::ID).then_some(&format::ExampleTab as &dyn FormatWriter)
    }

    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        (interface == service::INTERFACE && id == service::ID)
            .then_some(&service::NoteNames as &dyn Service)
    }

    fn tab(&self, id: &str) -> Option<&dyn Tab> {
        (id == statistics::ID).then_some(&self.statistics as &dyn Tab)
    }

    fn event(&self, event: &oxiguitar_plugin::Event) {
        self.statistics.event(event);
    }
}

// The plugin's entry point and manifest, and a test (under `cargo test`)
// that the manifest matches the code.
oxiguitar_plugin::export_plugin!(Example);
// ANCHOR_END: plugin
