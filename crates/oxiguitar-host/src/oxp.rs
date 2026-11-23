// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading a plugin file's manifest without loading the plugin.
//!
//! An `.oxp` file is a shared library: ELF on Linux, PE on Windows, Mach-O
//! on macOS (one universal file for both processors). The `object` crate
//! reads its headers and section table and only the manifest's section,
//! through a small read cache, so a 10 MB plugin costs a few kilobytes of
//! reading: ELF and Mach-O files by their headers alone (`elf_section`,
//! `macho_section`), since a full parse also reads the symbol table. No
//! code from the file runs.
//!
//! The file must also be for this system and processor: a Linux plugin in
//! a Windows folder, or an ARM64 plugin on an x86_64 computer, is reported
//! here instead of failing to load later.

use std::fs::File;
use std::path::Path;

use object::read::ReadCache;
use object::read::elf::{FileHeader, ProgramHeader, SectionHeader};
use object::read::macho::{
    FatArch, MachHeader, MachOFatFile32, MachOFatFile64, Section as _, Segment as _,
};
use object::{
    Architecture, BinaryFormat, Endianness, FileKind, Object, ObjectSection, ObjectSegment, ReadRef,
};
use object::{elf, macho};
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
        _ => one(&cache)?,
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
    one(cache.range(arch.offset().into(), arch.size().into()))
}

/// The manifest section of one library: ELF and Mach-O files by their
/// headers alone ([`elf_section`], [`macho_section`]), others through
/// [`section`].
fn one<'a, R: ReadRef<'a>>(data: R) -> Result<Vec<u8>, String> {
    match FileKind::parse(data).map_err(damaged)? {
        FileKind::Elf64 => elf_section::<elf::FileHeader64<Endianness>, _>(data),
        FileKind::Elf32 => elf_section::<elf::FileHeader32<Endianness>, _>(data),
        FileKind::MachO64 => macho_section::<macho::MachHeader64<Endianness>, _>(data),
        FileKind::MachO32 => macho_section::<macho::MachHeader32<Endianness>, _>(data),
        _ => section(data),
    }
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

/// The manifest section of an ELF library, after the same checks as
/// [`section`], from the file's headers alone: its program headers, its
/// section headers and the section names. `object::File::parse` also reads
/// an ELF file's symbol table and its strings, megabytes in a plugin that
/// isn't stripped, which finding a manifest never needs and which made
/// reading a folder of plugins cost in proportion to their code.
fn elf_section<'a, Elf, R>(data: R) -> Result<Vec<u8>, String>
where
    Elf: FileHeader<Endian = Endianness>,
    R: ReadRef<'a>,
{
    let length = data.len().unwrap_or(0);
    let header = Elf::parse(data).map_err(damaged)?;
    let endian = header.endian().map_err(damaged)?;
    let end = header
        .program_headers(endian, data)
        .map_err(damaged)?
        .iter()
        .filter(|p| p.p_type(endian) == elf::PT_LOAD)
        .map(|p| {
            let (offset, size) = p.file_range(endian);
            offset.saturating_add(size)
        })
        .max()
        .unwrap_or(0);
    if end > length {
        return Err(format!(
            "it is cut short ({length} bytes of {end}): the file is damaged"
        ));
    }
    if this_format() != BinaryFormat::Elf {
        return Err(format!(
            "it is a Linux or Android library, and this is {}",
            std::env::consts::OS
        ));
    }
    let built = match (header.e_machine(endian), header.is_class_64()) {
        (elf::EM_X86_64, true) => Architecture::X86_64,
        (elf::EM_AARCH64, true) => Architecture::Aarch64,
        (elf::EM_386, false) => Architecture::I386,
        (elf::EM_ARM, false) => Architecture::Arm,
        (elf::EM_RISCV, true) => Architecture::Riscv64,
        (elf::EM_RISCV, false) => Architecture::Riscv32,
        _ => Architecture::Unknown,
    };
    if let Some(here) = this_architecture()
        && built != here
    {
        return Err(format!(
            "it is built for {}, and this computer is {}",
            name(built),
            std::env::consts::ARCH
        ));
    }
    let sections = header.sections(endian, data).map_err(damaged)?;
    let Some((_, section)) = sections.section_by_name(endian, MANIFEST_SECTION.as_bytes()) else {
        return Err(format!(
            "it has no manifest (no {MANIFEST_SECTION} section): it is not an OxiGuitar plugin"
        ));
    };
    let size: u64 = section.sh_size(endian).into();
    if size > MAX_MANIFEST_LEN as u64 {
        return Err(format!(
            "its manifest is {size} bytes, more than the {MAX_MANIFEST_LEN} a manifest may have"
        ));
    }
    section
        .data(endian, data)
        .map(<[u8]>::to_vec)
        .map_err(|_| "its manifest can't be read (the file is damaged)".to_owned())
}

/// The manifest section of a Mach-O library, after the same checks as
/// [`section`], from its header and load commands alone ([`macho_parts`]).
fn macho_section<'a, Mach, R>(data: R) -> Result<Vec<u8>, String>
where
    Mach: MachHeader<Endian = Endianness>,
    R: ReadRef<'a>,
{
    let length = data.len().unwrap_or(0);
    let parts = macho_parts::<Mach, R>(data)?;
    if parts.end > length {
        return Err(format!(
            "it is cut short ({length} bytes of {}): the file is damaged",
            parts.end
        ));
    }
    if this_format() != BinaryFormat::MachO {
        return Err(format!(
            "it is a macOS or iOS library, and this is {}",
            std::env::consts::OS
        ));
    }
    if let Some(here) = this_architecture()
        && parts.built != here
    {
        return Err(format!(
            "it is built for {}, and this computer is {}",
            name(parts.built),
            std::env::consts::ARCH
        ));
    }
    parts.manifest.unwrap_or_else(|| {
        Err(format!(
            "it has no manifest (no {MANIFEST_SECTION} section): it is not an OxiGuitar plugin"
        ))
    })
}

/// What a Mach-O library's header and load commands say.
#[derive(Debug)]
struct MachOParts {
    /// Where its last segment ends in the file.
    end: u64,
    /// Its processor.
    built: Architecture,
    /// Its manifest section's bytes, if it has one, or why they can't be
    /// had.
    manifest: Option<Result<Vec<u8>, String>>,
}

/// Reads a Mach-O library's header and load commands, and its manifest
/// section, and nothing else. Upstream (object 0.41): as for ELF,
/// `object::File::parse` reads the whole symbol table first (measured with
/// 0.37), which made a folder of plugins cost in
/// proportion to their code (159 ms for 30 debug builds on CI's macOS
/// runner, over the 100 ms budget).
fn macho_parts<'a, Mach, R>(data: R) -> Result<MachOParts, String>
where
    Mach: MachHeader<Endian = Endianness>,
    R: ReadRef<'a>,
{
    let header = Mach::parse(data, 0).map_err(damaged)?;
    let endian = header.endian().map_err(damaged)?;
    let mut end = 0u64;
    let mut found = None;
    let mut commands = header.load_commands(endian, data, 0).map_err(damaged)?;
    while let Some(command) = commands.next().map_err(damaged)? {
        let Some((segment, section_data)) =
            Mach::Segment::from_command(command).map_err(damaged)?
        else {
            continue;
        };
        let (offset, size) = segment.file_range(endian);
        end = end.max(offset.saturating_add(size));
        let sections = segment.sections(endian, section_data).map_err(damaged)?;
        // Upstream (object 0.41): a section's data is read at the offset
        // this iterator gives, not the header's 32-bit one (which wraps past
        // 4 GB); `Section::data` takes it since 0.40.
        for item in segment.section_offsets(endian, sections) {
            let (s, offset) = item.map_err(damaged)?;
            if s.name() == MANIFEST_SECTION.as_bytes() {
                found = Some((*s, offset));
            }
        }
    }
    let built = match header.cputype(endian) {
        macho::CPU_TYPE_X86_64 => Architecture::X86_64,
        macho::CPU_TYPE_ARM64 => Architecture::Aarch64,
        macho::CPU_TYPE_X86 => Architecture::I386,
        macho::CPU_TYPE_ARM => Architecture::Arm,
        _ => Architecture::Unknown,
    };
    let manifest = found.map(|(section, offset)| {
        let size: u64 = section.size(endian).into();
        if size > MAX_MANIFEST_LEN as u64 {
            return Err(format!(
                "its manifest is {size} bytes, more than the {MAX_MANIFEST_LEN} a manifest may have"
            ));
        }
        section
            .data(endian, data, offset)
            .map(<[u8]>::to_vec)
            .map_err(|e| format!("its manifest can't be read (the file is damaged: {e})"))
    });
    Ok(MachOParts {
        end,
        built,
        manifest,
    })
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
        let says = |path: &Path, what: &str| {
            let e = read_manifest(path).unwrap_err();
            assert!(e.contains(what), "{}: {e}", path.display());
        };
        let text = dir.join("notes.oxp");
        std::fs::write(&text, b"just some text, not a library").unwrap();
        says(&text, "not a plugin");
        let empty = dir.join("empty.oxp");
        std::fs::write(&empty, b"").unwrap();
        assert!(read_manifest(&empty).is_err());
        says(&dir.join("missing.oxp"), "can't be read");
        // A library format, but not a plugin: this test program itself.
        says(&std::env::current_exe().unwrap(), "no manifest");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A 64-bit little-endian Mach-O library for `cputype` with one
    /// segment holding one section `name` of `contents`.
    fn tiny_macho(cputype: macho::CpuType, name: &str, contents: &[u8]) -> Vec<u8> {
        let mut f = Vec::new();
        let u32le = |f: &mut Vec<u8>, v: u32| f.extend_from_slice(&v.to_le_bytes());
        let u64le = |f: &mut Vec<u8>, v: u64| f.extend_from_slice(&v.to_le_bytes());
        let name16 = |f: &mut Vec<u8>, n: &str| {
            let mut b = [0u8; 16];
            b[..n.len()].copy_from_slice(n.as_bytes());
            f.extend_from_slice(&b);
        };
        let commands = 72 + 80;
        let data_at = (32 + commands) as u64;
        // mach_header_64
        u32le(&mut f, macho::MH_MAGIC_64);
        u32le(&mut f, cputype.0);
        u32le(&mut f, 0);
        u32le(&mut f, macho::MH_DYLIB.0);
        u32le(&mut f, 1);
        u32le(&mut f, commands);
        u32le(&mut f, 0);
        u32le(&mut f, 0);
        // segment_command_64
        u32le(&mut f, macho::LC_SEGMENT_64.0);
        u32le(&mut f, commands);
        name16(&mut f, "__DATA");
        u64le(&mut f, 0);
        u64le(&mut f, 0x1000);
        u64le(&mut f, data_at);
        u64le(&mut f, contents.len() as u64);
        u32le(&mut f, 3);
        u32le(&mut f, 3);
        u32le(&mut f, 1);
        u32le(&mut f, 0);
        // section_64
        name16(&mut f, name);
        name16(&mut f, "__DATA");
        u64le(&mut f, 0);
        u64le(&mut f, contents.len() as u64);
        u32le(&mut f, data_at as u32);
        for _ in 0..7 {
            u32le(&mut f, 0);
        }
        assert_eq!(f.len() as u64, data_at);
        f.extend_from_slice(contents);
        f
    }

    #[test]
    fn a_macho_librarys_manifest_is_read_from_its_headers() {
        type Mach = macho::MachHeader64<Endianness>;
        let file = tiny_macho(
            macho::CPU_TYPE_ARM64,
            MANIFEST_SECTION,
            b"{\"id\": \"x\"}\0\0",
        );
        assert_eq!(FileKind::parse(&*file).unwrap(), FileKind::MachO64);
        let parts = macho_parts::<Mach, _>(&*file).unwrap();
        assert_eq!(parts.built, Architecture::Aarch64);
        assert_eq!(parts.end, file.len() as u64);
        assert_eq!(parts.manifest, Some(Ok(b"{\"id\": \"x\"}\0\0".to_vec())));
        // The same as `object`'s full reading finds.
        let full = object::File::parse(&*file).unwrap();
        assert_eq!(
            full.section_by_name(MANIFEST_SECTION)
                .unwrap()
                .data()
                .unwrap(),
            b"{\"id\": \"x\"}\0\0"
        );
        let other = tiny_macho(macho::CPU_TYPE_X86_64, "__text", b"code");
        let parts = macho_parts::<Mach, _>(&*other).unwrap();
        assert_eq!(parts.built, Architecture::X86_64);
        assert_eq!(parts.manifest, None);
        let big = tiny_macho(
            macho::CPU_TYPE_ARM64,
            MANIFEST_SECTION,
            &vec![b' '; MAX_MANIFEST_LEN + 1],
        );
        let parts = macho_parts::<Mach, _>(&*big).unwrap();
        assert!(matches!(parts.manifest, Some(Err(e)) if e.contains("more than")));
        // Cut short: the segment runs past the file's end.
        let cut = &file[..file.len() - 4];
        assert_eq!(macho_parts::<Mach, _>(cut).unwrap().end, file.len() as u64);
        let e = one(cut).unwrap_err();
        assert!(e.contains("cut short"), "{e}");
        // A whole one, on a system that isn't macOS, says what it is.
        if !cfg!(target_vendor = "apple") {
            let e = one(&*file).unwrap_err();
            assert!(e.contains("macOS"), "{e}");
        }
    }

    #[test]
    fn system_plugins_manifests_are_readable() {
        let dir = std::path::PathBuf::from("/usr/lib/oxiguitar/plugins");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            println!("System plugins directory not found, skipping");
            return;
        };
        let plugins: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        println!("Found {} system plugin files", plugins.len());
        for p in &plugins {
            println!("\n--- {} ---", p.file_name().unwrap().to_string_lossy());
            let result = read_manifest(p);
            match result {
                Ok(mf) => {
                    println!(
                        "  OK: id={}, name={}, version={}, interface={}",
                        mf.manifest.id,
                        mf.manifest.name,
                        mf.manifest.version,
                        mf.manifest.interface
                    );
                    println!(
                        "  OS: {:?}, Arch: {:?}, Display: {:?}, CPU: {:?}",
                        mf.manifest.conditions.os,
                        mf.manifest.conditions.arch,
                        mf.manifest.conditions.display,
                        mf.manifest.conditions.cpu
                    );
                }
                Err(e) => println!("  ERROR: {}", e),
            }
        }
    }
}
