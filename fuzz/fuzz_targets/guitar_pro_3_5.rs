// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 3, 4 and 5 reader (binary .gp3, .gp4, .gp5). Seeds: the
//! public test songs in those formats. Inputs that are not recognised as
//! one of them are skipped, so the time goes to the reader.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxiguitar_formats::{Format, detect, read};

fuzz_target!(|data: &[u8]| {
    if matches!(
        detect(data),
        Some(Format::GuitarPro3 { .. } | Format::GuitarPro4 { .. } | Format::GuitarPro5 { .. })
    ) {
        let _ = read(data);
    }
});
