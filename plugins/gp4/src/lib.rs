// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 4 format plugin (`gp4.oxp`, id `oxiguitar.gp4`): reads Guitar Pro 4 files (.gp4).
//!
//! A thin crate over `oxiguitar-formats`, built with that crate's `gp4`
//! reader only. The host recognises a file as this format from the
//! signatures in `manifest.json`, without loading the plugin.

use oxiguitar_plugin::{Error, FormatReader, Host, Plugin, Read};

/// The format's id in `manifest.json`.
pub const FORMAT: &str = "gp4";

/// The plugin.
#[derive(Debug)]
pub struct Gp4;

impl Plugin for Gp4 {
    fn start(_host: Host) -> Result<Gp4, Error> {
        Ok(Gp4)
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == FORMAT).then_some(self as &dyn FormatReader)
    }
}

impl FormatReader for Gp4 {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        oxiguitar_formats::plugin::read(data)
    }
}

oxiguitar_plugin::export_plugin!(Gp4);

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
        let plugin = Gp4::start(Host::detached()).unwrap();
        let reader = plugin.reader(FORMAT).unwrap();
        let read = reader.read(&fixture("gp4")).unwrap();
        assert_eq!(read.score.info.title, "OxiGuitar Test Song");
        assert!(plugin.reader("gp-something-else").is_none());
    }

    #[test]
    fn its_signatures_recognise_its_fixture_only() {
        let manifest: Manifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        let format = &manifest.formats[0];
        for ext in ["gp3", "gp4", "gp5", "gpx", "gp"] {
            let data = fixture(ext);
            let sniff = Sniff::new(&data);
            let matched = format.signatures.iter().any(|s| s.matches(&sniff));
            assert_eq!(matched, ext == "gp4", "{ext}");
        }
    }
}
