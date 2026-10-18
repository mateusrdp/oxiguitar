// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Guitar Pro 3 format plugin (`gp3.oxp`, id `oxiguitar.gp3`): reads Guitar Pro 3 files (.gp3).
//!
//! A thin crate over `oxiguitar-formats`, built with that crate's `gp3`
//! reader only. The host recognises a file as this format from the
//! signatures in `manifest.json`, without loading the plugin.

use oxiguitar_plugin::{Error, FormatReader, Host, Plugin, Read};

/// The format's id in `manifest.json`.
pub const FORMAT: &str = "gp3";

/// The plugin.
#[derive(Debug)]
pub struct Gp3;

impl Plugin for Gp3 {
    fn start(_host: Host) -> Result<Gp3, Error> {
        Ok(Gp3)
    }

    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        (format == FORMAT).then_some(self as &dyn FormatReader)
    }
}

impl FormatReader for Gp3 {
    fn read(&self, data: &[u8]) -> Result<Read, Error> {
        oxiguitar_formats::plugin::read(data)
    }
}

oxiguitar_plugin::export_plugin!(Gp3);

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
        let plugin = Gp3::start(Host::detached()).unwrap();
        let reader = plugin.reader(FORMAT).unwrap();
        let read = reader.read(&fixture("gp3")).unwrap();
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
            assert_eq!(matched, ext == "gp3", "{ext}");
        }
    }
}
