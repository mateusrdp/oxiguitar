// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 7–8 format plugin (`gp.oxp`, id `oxiguitar.gp`): reads
//! and writes Guitar Pro 7 and 8 files (.gp).
//!
//! A thin crate over `oxiguitar-formats`, built with that crate's `gp`
//! reader and writer only. The host recognises a file as this format from the
//! signatures in `manifest.json`, without loading the plugin.

use oxiguitar_plugin::{Error, FormatReader, FormatWriter, Host, Plugin, Read, Written, score};

/// The format's id in `manifest.json`.
pub const FORMAT: &str = "gp";

/// The plugin.
#[derive(Debug)]
pub struct Gp;

impl Plugin for Gp {
    fn start(_host: Host) -> Result<Gp, Error> {
        Ok(Gp)
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == FORMAT).then_some(self as &dyn FormatReader)
    }

    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        (format == FORMAT).then_some(self as &dyn FormatWriter)
    }
}

impl FormatReader for Gp {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        oxiguitar_formats::plugin::read(data)
    }
}

impl FormatWriter for Gp {
    fn write(&self, score: &score::Score) -> Result<Written, Error> {
        oxiguitar_formats::plugin::write_gp(score)
    }
}

oxiguitar_plugin::export_plugin!(Gp);

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
        let plugin = Gp::start(Host::detached()).unwrap();
        let reader = plugin.reader(FORMAT).unwrap();
        let read = reader.read(&fixture("gp")).unwrap();
        assert_eq!(read.score.info.title, "OxiGuitar Test Song");
        assert!(plugin.reader("gp-something-else").is_none());
    }

    #[test]
    fn writes_what_it_reads() {
        let plugin = Gp::start(Host::detached()).unwrap();
        let read = plugin.reader(FORMAT).unwrap().read(&fixture("gp")).unwrap();
        let written = plugin.writer(FORMAT).unwrap().write(&read.score).unwrap();
        assert!(written.losses.is_empty(), "{:?}", written.losses);
        let again = plugin.reader(FORMAT).unwrap().read(&written.data).unwrap();
        assert_eq!(again.score, read.score);
        assert!(plugin.writer("gp5").is_none());
    }

    #[test]
    fn its_signatures_recognise_its_fixture_only() {
        let manifest: Manifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        let format = &manifest.formats[0];
        for ext in ["gp3", "gp4", "gp5", "gpx", "gp"] {
            let data = fixture(ext);
            let sniff = Sniff::new(&data);
            let matched = format.signatures.iter().any(|s| s.matches(&sniff));
            assert_eq!(matched, ext == "gp", "{ext}");
        }
    }
}
