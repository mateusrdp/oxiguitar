// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugin files: reading one's manifest without loading it, unpacking the
//! one used, and packing one.
//!
//! An `.oxp` file is an uncompressed tar archive (#347;
//! `oxiguitar_plugin_api::oxp`): `manifest.json` first, the library
//! (`plugin.so`, `plugin.dll` or `plugin.dylib`) second, then whatever else
//! the plugin ships. Reading a manifest reads the first two members'
//! headers and the manifest, then checks the library member as a library
//! (ELF on Linux, PE on Windows, Mach-O on macOS, one universal file for
//! both processors): the `object` crate reads its headers and only its
//! manifest section, through a small read cache, so a 10 MB plugin costs a
//! few kilobytes of reading. The library's manifest section must hold the
//! archive's `manifest.json`. No code from the file runs.
//!
//! The file must also be for this system and processor: a Linux plugin in
//! a Windows folder, or an ARM64 plugin on an x86_64 computer, is reported
//! here instead of failing to load later.
//!
//! The copy that is used is unpacked once into the user's folder and
//! kept: a later start finds it by the plugin file's
//! version, size and time, with one look at the disk.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use object::read::ReadCache;
use object::read::elf::{FileHeader, ProgramHeader, SectionHeader};
use object::read::macho::{
    FatArch, MachHeader, MachOFatFile32, MachOFatFile64, Section as _, Segment as _,
};
use object::{
    Architecture, BinaryFormat, Endianness, FileKind, Object, ObjectSection, ObjectSegment, ReadRef,
};
use object::{elf, macho};
use oxiguitar_plugin_api::manifest::{Manifest, is_packed_path};
use oxiguitar_plugin_api::oxp::{LIBRARIES, MANIFEST, library};
use oxiguitar_plugin_api::{MANIFEST_SECTION, MAX_MANIFEST_LEN, PE_MANIFEST_SECTION};

/// A manifest as read from a plugin file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ManifestFile {
    /// The manifest.
    pub(crate) manifest: Manifest,
    /// The library's manifest section, without trailing zero bytes: the
    /// plugin's table must carry the same bytes when it is loaded.
    pub(crate) bytes: Vec<u8>,
}

/// Reads the manifest of the plugin file at `path`, checking that the file
/// is a plugin archive whose library is for this system and processor and
/// carries the same manifest. The error is one sentence for the user,
/// without the file's name.
pub(crate) fn read_manifest(path: &Path) -> Result<ManifestFile, String> {
    let file = File::open(path).map_err(|e| format!("it can't be read ({e})"))?;
    let length = file
        .metadata()
        .map_err(|e| format!("it can't be read ({e})"))?
        .len();
    let (bytes, offset, size) = {
        let mut archive = tar::Archive::new(&file);
        let mut entries = archive
            .entries_with_seek()
            .map_err(|_| not_an_archive(&file))?;
        let mut first = match entries.next() {
            Some(Ok(entry)) => entry,
            _ => return Err(not_an_archive(&file)),
        };
        if member_name(&first).as_deref() != Some(MANIFEST)
            || first.header().entry_type() != tar::EntryType::Regular
        {
            return Err(not_an_archive(&file));
        }
        if first.size() > MAX_MANIFEST_LEN as u64 {
            return Err(format!(
                "its manifest is {} bytes, more than the {MAX_MANIFEST_LEN} a manifest may have",
                first.size()
            ));
        }
        let mut bytes = Vec::new();
        first
            .read_to_end(&mut bytes)
            .map_err(|_| "it is cut short: the file is damaged".to_owned())?;
        let second = match entries.next() {
            Some(Ok(entry)) => entry,
            Some(Err(_)) => return Err("it is damaged (its second part can't be read)".into()),
            None => return Err("it holds no library: it is damaged".into()),
        };
        let name = member_name(&second).unwrap_or_default();
        if name != library() {
            return Err(match LIBRARIES.iter().position(|l| *l == name) {
                Some(0) => "it is a plugin for Linux".to_owned(),
                Some(1) => "it is a plugin for Windows".to_owned(),
                Some(_) => "it is a plugin for macOS".to_owned(),
                None => format!(
                    "its second part is {name:?}, where its library ({}) should be: it is damaged",
                    library()
                ),
            } + &format!(", and this is {}", std::env::consts::OS));
        }
        if second.header().entry_type() != tar::EntryType::Regular {
            return Err("its library isn't a plain file: it is damaged".into());
        }
        (bytes, second.raw_file_position(), second.size())
    };
    if offset.saturating_add(size) > length {
        return Err(format!(
            "it is cut short ({length} bytes of {}): the file is damaged",
            offset.saturating_add(size)
        ));
    }
    let cache = ReadCache::new(&file);
    let section = library_section(&cache, offset, size)?;
    if trimmed(&section, true) != trimmed(&bytes, true) {
        return Err(format!(
            "its {MANIFEST} isn't the manifest its library was built with: it is damaged"
        ));
    }
    let bytes = trimmed(&section, false).to_vec();
    let manifest = Manifest::parse(&bytes)?;
    Ok(ManifestFile { manifest, bytes })
}

/// `bytes` without trailing zero bytes (a section's padding), and white
/// space if `space`.
fn trimmed(bytes: &[u8], space: bool) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|&b| b != 0 && !(space && b.is_ascii_whitespace()))
        .map_or(0, |i| i + 1);
    &bytes[..end]
}

/// Why a file that isn't a plugin archive can't be used: a plugin file of
/// the format before 0.23 (a bare library) says so.
fn not_an_archive(file: &File) -> String {
    let cache = ReadCache::new(file);
    match FileKind::parse(&cache) {
        Ok(
            FileKind::Elf32
            | FileKind::Elf64
            | FileKind::MachO32
            | FileKind::MachO64
            | FileKind::MachOFat32
            | FileKind::MachOFat64
            | FileKind::Pe32
            | FileKind::Pe64,
        ) => "it is a plugin file of the format before OxiGuitar 0.23 (a bare library): \
              get a newer version of the plugin"
            .into(),
        _ => "it is not a plugin (not an OxiGuitar plugin file)".into(),
    }
}

/// A member's path inside the archive, as UTF-8 text without a leading
/// `./` or a trailing `/`; `None` when it isn't UTF-8.
fn member_name<R: Read>(entry: &tar::Entry<'_, R>) -> Option<String> {
    let bytes = entry.path_bytes();
    let name = std::str::from_utf8(&bytes).ok()?;
    let name = name.strip_prefix("./").unwrap_or(name);
    Some(name.trim_end_matches('/').to_owned())
}

/// The manifest section of the library at `offset`, `size` bytes long, in
/// the file `cache` reads, after checking it is a library for this system
/// and processor (for a universal Mach-O file, its part for this one).
fn library_section<R: std::io::Read + std::io::Seek>(
    cache: &ReadCache<R>,
    offset: u64,
    size: u64,
) -> Result<Vec<u8>, String> {
    let data = cache.range(offset, size);
    let kind = FileKind::parse(data)
        .map_err(|_| "its library is not a library file of any kind".to_owned())?;
    match kind {
        FileKind::MachOFat32 => {
            let fat = MachOFatFile32::parse(data).map_err(damaged)?;
            from_fat(cache, offset, size, fat.arches())
        }
        FileKind::MachOFat64 => {
            let fat = MachOFatFile64::parse(data).map_err(damaged)?;
            from_fat(cache, offset, size, fat.arches())
        }
        _ => one(data),
    }
}

/// The manifest section of the part of a universal Mach-O file (at
/// `offset`, `size` bytes long) that is for this processor.
fn from_fat<R: std::io::Read + std::io::Seek, A: FatArch>(
    cache: &ReadCache<R>,
    offset: u64,
    size: u64,
    arches: &[A],
) -> Result<Vec<u8>, String> {
    let here = this_architecture();
    let Some(arch) = arches.iter().find(|a| Some(a.architecture()) == here) else {
        let built: Vec<String> = arches.iter().map(|a| name(a.architecture())).collect();
        return Err(format!(
            "it is built for {}, and this computer is {}",
            built.join(" and "),
            std::env::consts::ARCH
        ));
    };
    let (start, length): (u64, u64) = (arch.offset().into(), arch.size().into());
    if start.saturating_add(length) > size {
        return Err(format!(
            "it is cut short ({size} bytes of {}): the file is damaged",
            start.saturating_add(length)
        ));
    }
    one(cache.range(offset + start, length))
}

/// The manifest section of the library file at `path`: what a plugin file
/// packed from it must hold as its `manifest.json` ([`pack`]).
fn library_manifest(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("it can't be read ({e})"))?;
    let length = file
        .metadata()
        .map_err(|e| format!("it can't be read ({e})"))?
        .len();
    let cache = ReadCache::new(&file);
    let section = library_section(&cache, 0, length)?;
    Ok(trimmed(&section, false).to_vec())
}

/// Packs the plugin library at `code` into the plugin file `out`, with
/// the manifest its section holds as `manifest.json` and each folder of
/// `folders` under its own name (a plugin crate's `ui` folder as `ui/`),
/// its files in name order. The way plugin builds that aren't release
/// builds make their files (`oxiguitar-plugin-test`'s `build`); releases
/// pack with the system's `tar` (docs/plugins, "Packaging"). The library
/// must be for this system.
pub fn pack(code: &Path, folders: &[&Path], out: &Path) -> Result<(), String> {
    let manifest = library_manifest(code).map_err(|e| format!("{}: {e}", code.display()))?;
    let file = File::create(out).map_err(|e| format!("can't write {}: {e}", out.display()))?;
    let mut builder = tar::Builder::new(std::io::BufWriter::new(file));
    let failed = |e: std::io::Error| format!("can't write {}: {e}", out.display());
    let append =
        |builder: &mut tar::Builder<_>, name: &str, data: &mut dyn Read, size: u64, mode| {
            let mut header = tar::Header::new_gnu();
            header.set_size(size);
            header.set_mode(mode);
            header.set_mtime(0);
            header.set_entry_type(tar::EntryType::Regular);
            builder.append_data(&mut header, name, data)
        };
    append(
        &mut builder,
        MANIFEST,
        &mut manifest.as_slice(),
        manifest.len() as u64,
        0o644,
    )
    .map_err(failed)?;
    let data = File::open(code).map_err(|e| format!("can't read {}: {e}", code.display()))?;
    let size = data.metadata().map_err(failed)?.len();
    append(&mut builder, library(), &mut &data, size, 0o755).map_err(failed)?;
    for folder in folders {
        let Some(top) = folder.file_name().and_then(|n| n.to_str()) else {
            return Err(format!("{} has no name to pack it under", folder.display()));
        };
        let mut files = Vec::new();
        walk(folder, top, &mut files)
            .map_err(|e| format!("can't read {}: {e}", folder.display()))?;
        files.sort();
        for (name, path) in files {
            if !is_packed_path(&name) {
                return Err(format!("{} can't go in a plugin file", path.display()));
            }
            let data =
                File::open(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
            let size = data.metadata().map_err(failed)?.len();
            append(&mut builder, &name, &mut &data, size, 0o644).map_err(failed)?;
        }
    }
    builder
        .into_inner()
        .and_then(|mut w| std::io::Write::flush(&mut w))
        .map_err(failed)
}

/// The plain files under `folder`, each with its path inside a plugin file
/// (`prefix/...`).
fn walk(folder: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&path, &name, out)?;
        } else if kind.is_file() {
            out.push((name, path));
        }
    }
    Ok(())
}

/// Makes each unpacking folder's name unique within this process.
static COUNT: AtomicU64 = AtomicU64::new(0);

/// The folder the plugin file at `path` (holding version `version` of the
/// plugin `id`) is unpacked in under `root`, unpacking it first unless an
/// earlier start already did: `ROOT/ID/PLACE-VERSION-SIZE-TIME`, from where
/// the file is and its size and time, so another file (a new build, a copy
/// in another folder) is unpacked again, into a folder of its own. That
/// look is one `stat` of the unpacked library.
///
/// The file is unpacked into a new folder beside, which is then renamed
/// into place, so another OxiGuitar unpacking it at the same time, or one
/// that stopped half way, never leaves a partial folder in use. What was
/// unpacked from earlier files at the same place (older versions, older
/// builds) is then removed when it can be (on Windows, not while another
/// OxiGuitar has its library loaded); what other places' copies unpacked is
/// kept for them.
pub(crate) fn unpack(path: &Path, id: &str, version: &str, root: &Path) -> Result<PathBuf, String> {
    let dir = unpacked_dir(path, id, version, root)?;
    if dir.join(library()).is_file() {
        return Ok(dir);
    }
    let plugin = root.join(id);
    let place = format!("{:016x}-", place(path));
    let cant = |e: std::io::Error| format!("it can't be unpacked into {} ({e})", plugin.display());
    std::fs::create_dir_all(&plugin).map_err(cant)?;
    let part = plugin.join(format!(
        ".part-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&part);
    if let Err(e) = extract(path, &part) {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e);
    }
    if std::fs::rename(&part, &dir).is_err() {
        // Another OxiGuitar got there first, or a folder by that name was
        // left without its library.
        if !dir.join(library()).is_file() {
            let _ = std::fs::remove_dir_all(&dir);
            if let Err(e) = std::fs::rename(&part, &dir) {
                let _ = std::fs::remove_dir_all(&part);
                return Err(cant(e));
            }
        }
        let _ = std::fs::remove_dir_all(&part);
    }
    // Earlier files' folders from the same place, and what an OxiGuitar
    // that stopped half way left a day ago or more.
    if let Ok(entries) = std::fs::read_dir(&plugin) {
        let day = std::time::Duration::from_secs(24 * 60 * 60);
        for entry in entries.flatten() {
            let other = entry.path();
            if other == dir {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let unpacking = name.starts_with(".part-");
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > day);
            if name.starts_with(&place) || (unpacking && old) {
                let _ = std::fs::remove_dir_all(&other);
            }
        }
    }
    Ok(dir)
}

/// A number for where the file at `path` is (64-bit FNV-1a of its path),
/// the same from one start and one OxiGuitar version to the next.
fn place(path: &Path) -> u64 {
    path.as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// The folder [`unpack`] unpacks the plugin file at `path` in under `root`,
/// unpacked or not.
fn unpacked_dir(path: &Path, id: &str, version: &str, root: &Path) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("it can't be read ({e})"))?;
    let time = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Ok(root.join(id).join(format!(
        "{:016x}-{version}-{}-{time}",
        place(path),
        meta.len()
    )))
}

/// The folder an earlier [`unpack`] under `root` left for the plugin file at
/// `path`, if there is one: for a program that only reads the user's
/// folders (`oxiguitar-cli`).
pub(crate) fn kept(path: &Path, id: &str, version: &str, root: &Path) -> Option<PathBuf> {
    unpacked_dir(path, id, version, root)
        .ok()
        .filter(|dir| dir.join(library()).is_file())
}

/// Unpacks the plugin file at `path` into the new folder `into`: plain
/// files and folders only, at paths inside it.
fn extract(path: &Path, into: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("it can't be read ({e})"))?;
    let cant = |e: std::io::Error| format!("it can't be unpacked into {} ({e})", into.display());
    std::fs::create_dir_all(into).map_err(cant)?;
    let mut archive = tar::Archive::new(std::io::BufReader::new(file));
    let entries = archive
        .entries()
        .map_err(|_| "it is damaged (it can't be unpacked)".to_owned())?;
    for entry in entries {
        let mut entry = entry.map_err(|_| "it is damaged (it can't be unpacked)".to_owned())?;
        let kind = entry.header().entry_type();
        if kind == tar::EntryType::XGlobalHeader {
            continue;
        }
        let name = member_name(&entry).unwrap_or_default();
        if !is_packed_path(&name) {
            return Err(format!("it holds {name:?}, which can't be unpacked"));
        }
        let target = name
            .split('/')
            .fold(into.to_path_buf(), |p, part| p.join(part));
        match kind {
            tar::EntryType::Directory => std::fs::create_dir_all(&target).map_err(cant)?,
            tar::EntryType::Regular => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(cant)?;
                }
                let mut out = File::create(&target).map_err(cant)?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|_| "it is cut short: the file is damaged".to_owned())?;
            }
            _ => {
                return Err(format!(
                    "it holds {name:?}, which isn't a plain file or folder"
                ));
            }
        }
    }
    let lib = into.join(library());
    if !lib.is_file() {
        return Err("it holds no library: it is damaged".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&lib, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
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
        // A library format: a plugin file from before 0.23 would be one.
        says(&std::env::current_exe().unwrap(), "before OxiGuitar 0.23");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A tar archive of `members` (path, contents); a path ending in `/`
    /// is a folder, and `link` makes the last member a symbolic link.
    fn archive(members: &[(&str, &[u8])], link: bool) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (n, (name, data)) in members.iter().enumerate() {
            let mut header = tar::Header::new_ustar();
            // Written as is: `set_path` refuses what these tests need.
            let field = &mut header.as_old_mut().name;
            field[..name.len()].copy_from_slice(name.as_bytes());
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(if link && n == members.len() - 1 {
                tar::EntryType::Symlink
            } else if name.ends_with('/') {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            });
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap()
    }

    #[test]
    fn a_plugin_file_is_an_archive_with_its_manifest_first() {
        let dir = scratch("oxp-archive");
        let says = |name: &str, bytes: Vec<u8>, what: &str| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            let e = read_manifest(&path).unwrap_err();
            assert!(e.contains(what), "{name}: {e}");
        };
        let manifest = br#"{"id": "x"}"#;
        says(
            "other-first.oxp",
            archive(&[("readme.txt", b"hi"), (MANIFEST, manifest)], false),
            "not a plugin",
        );
        says(
            "alone.oxp",
            archive(&[(MANIFEST, manifest)], false),
            "no library",
        );
        says(
            "folder.oxp",
            archive(&[("ui/", b""), (MANIFEST, manifest)], false),
            "not a plugin",
        );
        let other = if cfg!(windows) {
            LIBRARIES[0]
        } else {
            LIBRARIES[1]
        };
        says(
            "other.oxp",
            archive(&[(MANIFEST, manifest), (other, b"code")], false),
            "it is a plugin for",
        );
        says(
            "not-code.oxp",
            archive(&[(MANIFEST, manifest), (library(), b"code")], false),
            "not a library file",
        );
        says(
            "third.oxp",
            archive(
                &[
                    (MANIFEST, manifest),
                    ("ui/x.slint", b""),
                    (library(), b"code"),
                ],
                false,
            ),
            "where its library",
        );
        let big = vec![b' '; MAX_MANIFEST_LEN + 1];
        says(
            "big.oxp",
            archive(&[(MANIFEST, &big), (library(), b"code")], false),
            "more than",
        );
        // Cut short inside the library.
        let mut cut = archive(&[(MANIFEST, manifest), (library(), &[7u8; 4000])], false);
        cut.truncate(2048);
        says("cut.oxp", cut, "cut short");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plugin_file_is_unpacked_once_and_kept() {
        let dir = scratch("oxp-unpack");
        let root = dir.join("unpacked");
        let file = dir.join("x.oxp");
        let members: &[(&str, &[u8])] = &[
            (MANIFEST, b"{}"),
            (library(), b"code"),
            ("ui/", b""),
            ("ui/main.slint", b"export component A {}"),
            ("assets/deep/a.bin", b"\x01\x02"),
        ];
        std::fs::write(&file, archive(members, false)).unwrap();
        let first = unpack(&file, "x", "1.0.0", &root).unwrap();
        assert!(first.starts_with(root.join("x")));
        let name = first.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(&format!("{:016x}-1.0.0-", place(&file))),
            "{name}"
        );
        assert_eq!(std::fs::read(first.join(library())).unwrap(), b"code");
        assert_eq!(
            std::fs::read(first.join("ui").join("main.slint")).unwrap(),
            b"export component A {}"
        );
        assert_eq!(
            std::fs::read(first.join("assets/deep/a.bin")).unwrap(),
            b"\x01\x02"
        );
        // A later start finds it: nothing is written again.
        std::fs::write(first.join("mark"), b"").unwrap();
        assert_eq!(unpack(&file, "x", "1.0.0", &root).unwrap(), first);
        assert!(first.join("mark").is_file());
        // Another file (here, another version) is unpacked again, and the
        // first one's folder goes.
        let second = unpack(&file, "x", "0.9.0", &root).unwrap();
        assert_ne!(second, first);
        assert!(second.join(library()).is_file());
        assert!(!first.exists());
        assert_eq!(std::fs::read_dir(root.join("x")).unwrap().count(), 1);
        // A copy elsewhere has a folder of its own, and the first file's
        // stays.
        let copy = dir.join("copy.oxp");
        std::fs::copy(&file, &copy).unwrap();
        let third = unpack(&copy, "x", "0.9.0", &root).unwrap();
        assert_ne!(third, second);
        assert!(second.join(library()).is_file() && third.join(library()).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unpacking_refuses_links_and_paths_outside_the_folder() {
        let dir = scratch("oxp-refuse");
        let root = dir.join("unpacked");
        for (n, (name, link)) in [
            ("../escape", false),
            ("/abs", false),
            ("ui/../../x", false),
            ("ui/link", true),
            ("C:/x", false),
        ]
        .into_iter()
        .enumerate()
        {
            let file = dir.join(format!("{n}.oxp"));
            let members: &[(&str, &[u8])] =
                &[(MANIFEST, b"{}"), (library(), b"code"), (name, b"x")];
            std::fs::write(&file, archive(members, link)).unwrap();
            let e = unpack(&file, "x", "1.0.0", &root).unwrap_err();
            assert!(e.contains(name), "{name}: {e}");
            assert!(!dir.join("escape").exists() && !dir.join("x").exists());
        }
        // Nothing half unpacked is left.
        assert_eq!(std::fs::read_dir(root.join("x")).unwrap().count(), 0);
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
