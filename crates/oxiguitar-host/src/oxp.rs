// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading a plugin file's manifest without loading the plugin.
//!
//! An `.oxp` file is a shared library: ELF on Linux, PE on Windows, Mach-O
//! on macOS (one universal file for both processors). The `object` crate
//! parses its headers and section table and reads only the manifest's
//! section, through a small read cache, so a 10 MB plugin costs a few
//! kilobytes of reading. No code from the file runs.
//!
//! The file must also be for this system and processor: a Linux plugin in
//! a Windows folder, or an ARM64 plugin on an x86_64 computer, is reported
//! here instead of failing to load later.

use std::fs::File;
use std::path::Path;

use object::read::ReadCache;
use object::read::macho::{FatArch, MachOFatFile32, MachOFatFile64};
use object::{Architecture, BinaryFormat, FileKind, Object, ObjectSection, ObjectSegment, ReadRef};
use oxiguitar_plugin_api::manifest::Manifest;
use oxiguitar_plugin_api::{MANIFEST_SECTION, MAX_MANIFEST_LEN, PE_MANIFEST_SECTION};

/// A manifest as read from a plugin file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ManifestFile {
    /// The manifest.
    pub(crate) manifest: Manifest,
    /// The section's bytes, without trailing zero bytes: the plugin's table
    /// must carry the same bytes when it is loaded.
    pub(crate) bytes: Vec<u8>,
}

/// Reads the manifest of the plugin file at `path`, checking that the file
/// is a library for this system and processor. The error is one sentence
/// for the user, without the file's name.
pub(crate) fn read_manifest(path: &Path) -> Result<ManifestFile, String> {
    let file = File::open(path).map_err(|e| format!("it can't be read ({e})"))?;
    let cache = ReadCache::new(file);
    let kind = FileKind::parse(&cache)
        .map_err(|_| "it is not a plugin (not a library file of any kind)".to_owned())?;
    let bytes = match kind {
        FileKind::MachOFat32 => {
            let fat = MachOFatFile32::parse(&cache).map_err(damaged)?;
            from_fat(&cache, fat.arches())?
        }
        FileKind::MachOFat64 => {
            let fat = MachOFatFile64::parse(&cache).map_err(damaged)?;
            from_fat(&cache, fat.arches())?
        }
        _ => section(&cache)?,
    };
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let bytes = bytes[..end].to_vec();
    let manifest = Manifest::parse(&bytes)?;
    Ok(ManifestFile { manifest, bytes })
}

/// The manifest section of the part of a universal Mach-O file that is for
/// this processor.
fn from_fat<A: FatArch>(cache: &ReadCache<File>, arches: &[A]) -> Result<Vec<u8>, String> {
    let here = this_architecture();
    let Some(arch) = arches.iter().find(|a| Some(a.architecture()) == here) else {
        let built: Vec<String> = arches.iter().map(|a| name(a.architecture())).collect();
        return Err(format!(
            "it is built for {}, and this computer is {}",
            built.join(" and "),
            std::env::consts::ARCH
        ));
    };
    section(cache.range(arch.offset().into(), arch.size().into()))
}

/// The manifest section of one library, after checking the library is for
/// this system and processor.
fn section<'a, R: ReadRef<'a>>(data: R) -> Result<Vec<u8>, String> {
    let length = data.len().unwrap_or(0);
    let file = object::File::parse(data).map_err(damaged)?;
    // A file cut short can still have whole headers (Mach-O and PE put them
    // first): every segment must lie inside the file, or it would fail to
    // load later.
    let end = file
        .segments()
        .map(|s| {
            let (offset, size) = s.file_range();
            offset.saturating_add(size)
        })
        .max()
        .unwrap_or(0);
    if end > length {
        return Err(format!(
            "it is cut short ({length} bytes of {end}): the file is damaged"
        ));
    }
    let (format, section_name) = match file.format() {
        BinaryFormat::Elf => ("a Linux or Android library", MANIFEST_SECTION),
        BinaryFormat::Pe => ("a Windows library", PE_MANIFEST_SECTION),
        BinaryFormat::MachO => ("a macOS or iOS library", MANIFEST_SECTION),
        _ => return Err("it is not a library file OxiGuitar can load".into()),
    };
    if file.format() != this_format() {
        return Err(format!(
            "it is {format}, and this is {}",
            std::env::consts::OS
        ));
    }
    if let Some(here) = this_architecture()
        && file.architecture() != here
    {
        return Err(format!(
            "it is built for {}, and this computer is {}",
            name(file.architecture()),
            std::env::consts::ARCH
        ));
    }
    let section = file.section_by_name(section_name).ok_or_else(|| {
        format!("it has no manifest (no {section_name} section): it is not an OxiGuitar plugin")
    })?;
    if section.size() > MAX_MANIFEST_LEN as u64 {
        return Err(format!(
            "its manifest is {} bytes, more than the {MAX_MANIFEST_LEN} a manifest may have",
            section.size()
        ));
    }
    section
        .data()
        .map(<[u8]>::to_vec)
        .map_err(|_| "its manifest can't be read (the file is damaged)".to_owned())
}

fn damaged(e: object::Error) -> String {
    format!("it is damaged or not a library ({e})")
}

/// The library format of this system.
fn this_format() -> BinaryFormat {
    if cfg!(target_os = "windows") {
        BinaryFormat::Pe
    } else if cfg!(target_vendor = "apple") {
        BinaryFormat::MachO
    } else {
        BinaryFormat::Elf
    }
}

/// This process's processor, if it is one OxiGuitar is built for.
fn this_architecture() -> Option<Architecture> {
    match std::env::consts::ARCH {
        "x86_64" => Some(Architecture::X86_64),
        "aarch64" => Some(Architecture::Aarch64),
        _ => None,
    }
}

fn name(arch: Architecture) -> String {
    match arch {
        Architecture::X86_64 => "x86_64".into(),
        Architecture::Aarch64 => "aarch64".into(),
        Architecture::I386 => "x86 (32-bit)".into(),
        Architecture::Arm => "ARM (32-bit)".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folders::tests::scratch;

    #[test]
    fn files_that_are_not_plugins_say_why() {
        let dir = scratch("oxp");
        let text = dir.join("notes.oxp");
        std::fs::write(&text, b"just some text, not a library").unwrap();
        assert!(read_manifest(&text).unwrap_err().contains("not a plugin"));
        let empty = dir.join("empty.oxp");
        std::fs::write(&empty, b"").unwrap();
        assert!(read_manifest(&empty).is_err());
        assert!(
            read_manifest(&dir.join("missing.oxp"))
                .unwrap_err()
                .contains("can't be read")
        );
        // A library, but not a plugin: this test program itself.
        let me = std::env::current_exe().unwrap();
        assert!(read_manifest(&me).unwrap_err().contains("no manifest"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
