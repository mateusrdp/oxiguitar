// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The service `note-names`: other plugins call it to name a pitch.
//!
//! A service is one implementation of an interface, which whoever defines
//! the interface documents: its methods and what their bytes hold. A plugin
//! that wants it lists the interface under `requires` in its manifest and
//! calls `host.call_service("note-names", "name", &[60, 0])`; the host
//! chooses among the services of every installed plugin (see
//! docs/plugins, "Services").
//!
//! This interface has one method:
//!
//! - `name`: the input is two bytes, a MIDI note number (60 = middle C) and
//!   1 for flats or 0 for sharps; the output is the note's name as UTF-8
//!   text, such as `C♯4`.

use oxiguitar_plugin::score::Pitch;
use oxiguitar_plugin::{Availability, Error, ErrorKind, Service};

use crate::notes;

/// The interface this service implements.
pub const INTERFACE: &str = "note-names";

/// The service's id in `manifest.json`.
pub const ID: &str = "example";

/// The service.
pub struct NoteNames;

// ANCHOR: service
impl Service for NoteNames {
    fn probe(&self) -> Availability {
        // Whether it can work here; a service that needs a device or a
        // server checks for it. This one always works.
        Availability::Available
    }

    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
        match (method, input) {
            ("name", &[pitch, flats]) if pitch <= 127 && flats <= 1 => {
                Ok(notes::name(Pitch(pitch), flats == 1).into_bytes())
            }
            ("name", _) => Err(Error::new(
                ErrorKind::Invalid,
                "name takes two bytes: a MIDI note number and 0 or 1 for flats",
            )),
            _ => Err(Error::not_found(format!(
                "note-names has no method {method:?}"
            ))),
        }
    }
}
// ANCHOR_END: service

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_notes() {
        assert_eq!(NoteNames.call("name", &[61, 0]).unwrap(), "C♯4".as_bytes());
        assert_eq!(NoteNames.call("name", &[61, 1]).unwrap(), "D♭4".as_bytes());
        assert_eq!(
            NoteNames.call("name", &[200, 0]).unwrap_err().kind,
            ErrorKind::Invalid
        );
        assert_eq!(
            NoteNames.call("sing", &[]).unwrap_err().kind,
            ErrorKind::NotFound
        );
    }
}
