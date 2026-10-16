// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The GPIF reader (score.gpif, the XML inside Guitar Pro 6, 7 and 8
//! files) and the XML tree under it. The input is the XML itself: packing
//! it into a .gp archive here, uncompressed, lets the fuzzer change the
//! score rather than the archive. Seeds: the score.gpif of every public .gp
//! file, with a dictionary of GPIF names (gpif.dict).

#![no_main]

use std::io::Write as _;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|xml: &[u8]| {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let packed = zip
        .start_file("Content/score.gpif", opts)
        .and_then(|()| zip.write_all(xml).map_err(Into::into))
        .and_then(|()| zip.finish());
    if let Ok(archive) = packed {
        let _ = oxiguitar_formats::read(&archive.into_inner());
    }
});
