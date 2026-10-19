// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An example OxiGuitar plugin, to copy and change.
//!
//! It adds three things, each in its own module so you can delete the ones
//! you don't need (and their entries in `manifest.json`):
//!
//! - [`format`]: reads and writes "Example tab" files (`.extab`), a tiny
//!   text format of eighth notes on a guitar;
//! - [`fretboard`]: a tab in the bottom area, with a fretboard tuned like
//!   the open score's first track; click a string to name the note;
//! - [`service`]: the service `note-names`, which other plugins call to
//!   name a pitch.
//!
//! [`notes`] holds what the fretboard and the service share.

pub mod format;
pub mod fretboard;
pub mod notes;
pub mod service;

use oxiguitar_plugin::{Error, FormatReader, FormatWriter, Host, Plugin, Service, Tab};

// ANCHOR: plugin
/// The plugin: built once, when OxiGuitar first uses one of its
/// contributions, and kept until the program ends.
pub struct Example {
    fretboard: fretboard::Fretboard,
}

impl Plugin for Example {
    fn start(host: Host) -> Result<Example, Error> {
        Ok(Example {
            fretboard: fretboard::Fretboard::new(host),
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
        (id == fretboard::ID).then_some(&self.fretboard as &dyn Tab)
    }

    fn event(&self, event: &oxiguitar_plugin::Event) {
        self.fretboard.event(event);
    }
}

// The plugin's entry point and manifest, and a test (under `cargo test`)
// that the manifest matches the code.
oxiguitar_plugin::export_plugin!(Example);
// ANCHOR_END: plugin
