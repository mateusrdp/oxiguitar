// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Any file: format detection from the contents (FMT-6), then whichever
//! reader it picks, including the refusals of formats no reader here
//! reads. Seeds: every public fixture.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = oxiguitar_formats::detect(data);
    let _ = oxiguitar_formats::read(data);
});
