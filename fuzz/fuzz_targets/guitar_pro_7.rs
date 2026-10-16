// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 7 and 8 reader (.gp): the ZIP archive, PartConfiguration
//! and the GPIF score. Seeds: the public .gp test song and the visual
//! references.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxiguitar_formats::{Format, detect, read};

fuzz_target!(|data: &[u8]| {
    if detect(data) == Some(Format::GuitarPro7Plus) {
        let _ = read(data);
    }
});
