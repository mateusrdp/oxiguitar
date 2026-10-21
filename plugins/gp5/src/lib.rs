// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 5 format plugin (`gp5.oxp`, id `oxiguitar.gp5`): reads and writes Guitar Pro 5 files (.gp5).
//!
//! A thin crate over `oxiguitar-formats`, built with that crate's `gp5`
//! reader and writer only. The host recognises a file as this format from the
//! signatures in `manifest.json`, without loading the plugin.

use oxiguitar_plugin::{Error, FormatReader, FormatWriter2, Host, Plugin, Read, Written, score2};

/// The format's id in `manifest.json`.
pub const FORMAT: &str = "gp5";

/// The plugin.
#[derive(Debug)]
pub struct Gp5;

impl Plugin for Gp5 {
    fn start(_host: Host) -> Result<Gp5, Error> {
        Ok(Gp5)
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == FORMAT).then_some(self as &dyn FormatReader)
    }

    // Interface 1.2: the score version 2, which holds what the writer needs.
    fn writer2(&self, format: &str) -> Option<&dyn FormatWriter2> {
        (format == FORMAT).then_some(self as &dyn FormatWriter2)
    }
}

impl FormatReader for Gp5 {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        oxiguitar_formats::plugin::read(data)
    }
}

impl FormatWriter2 for Gp5 {
    fn write(&self, score: &score2::Score2) -> Result<Written, Error> {
        oxiguitar_formats::plugin::write_gp5(score)
    }
}

oxiguitar_plugin::export_plugin!(Gp5);

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::api::manifest::Manifest;
    use oxiguitar_plugin::api::signature::Sniff;

    use super::*;

    fn fixture(ext: &str) -> Vec<u8> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gp");
        std::fs::read(dir.join(format!("oxiguitar-test-song.{ext}"))).unwrap()
    }

    #[test]
    fn reads_its_own_format() {
        // That it refuses every other format holds for the plugin built
        // alone (its .oxp file), which oxiguitar-plugin-test checks: tested
        // with other plugins, the formats crate has their readers too.
        let plugin = Gp5::start(Host::detached()).unwrap();
        let reader = plugin.reader(FORMAT).unwrap();
        let read = reader.read(&fixture("gp5")).unwrap();
        assert_eq!(read.score.info.title, "OxiGuitar Test Song");
        assert!(plugin.reader("gp-something-else").is_none());
    }

    #[test]
    fn writes_what_it_reads() {
        let plugin = Gp5::start(Host::detached()).unwrap();
        let read = plugin
            .reader(FORMAT)
            .unwrap()
            .read(&fixture("gp5"))
            .unwrap();
        let score = oxiguitar_formats::plugin::read2(&fixture("gp5"))
            .unwrap()
            .score;
        let written = plugin.writer2(FORMAT).unwrap().write(&score).unwrap();
        assert!(written.losses.is_empty(), "{:?}", written.losses);
        let again = plugin.reader(FORMAT).unwrap().read(&written.data).unwrap();
        assert_eq!(again.score, read.score);
        assert!(plugin.writer2("gp").is_none());
    }

    #[test]
    fn its_signatures_recognise_its_fixture_only() {
        let manifest: Manifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        let format = &manifest.formats[0];
        for ext in ["gp3", "gp4", "gp5", "gpx", "gp"] {
            let data = fixture(ext);
            let sniff = Sniff::new(&data);
            let matched = format.signatures.iter().any(|s| s.matches(&sniff));
            assert_eq!(matched, ext == "gp5", "{ext}");
        }
    }
}
