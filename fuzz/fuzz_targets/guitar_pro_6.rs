// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 6 reader (.gpx): the BCFZ decompressor, the BCFS file
//! system and the GPIF score inside. Seeds: the public .gpx test song.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxiguitar_formats::{Format, detect, read};

fuzz_target!(|data: &[u8]| {
    if detect(data) == Some(Format::GuitarPro6) {
        let _ = read(data);
    }
});
