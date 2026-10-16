// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writes a Windows resource file (.res) holding the program's icon and its
//! version information, so the build needs no resource compiler (rc.exe or
//! windres). The MSVC linker takes the file as one more input.
//!
//! Format: "Resource File Formats" and "VS_VERSIONINFO" in Microsoft's Win32
//! documentation. Each resource is a header (sizes, type, name, flags,
//! language) followed by its data, both padded to four bytes, and the file
//! starts with an empty resource. Icons are stored as PNG images, which
//! Windows has read since Vista.

/// The icon sizes linked into the program, in pixels (256 is the largest a
/// Windows icon holds).
pub(crate) const ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
/// English (United States), the language of the interface.
const LANGUAGE: u16 = 0x0409;
/// Unicode, the code page of the version strings.
const CODE_PAGE: u16 = 1200;

/// What Windows shows about the program (Explorer's Properties → Details,
/// Task Manager's name for it).
pub(crate) struct VersionInfo<'a> {
    /// Major, minor, patch, build.
    pub(crate) version: [u16; 4],
    /// The version as written, such as "0.1.0".
    pub(crate) version_text: String,
    /// Shown by Task Manager.
    pub(crate) description: &'a str,
    /// The product's name.
    pub(crate) product: &'a str,
    /// The copyright line.
    pub(crate) copyright: &'a str,
    /// The program's file name.
    pub(crate) original_filename: &'a str,
}

/// The whole .res file: the icon, as images of the given sizes (PNG data),
/// and the version information.
pub(crate) fn resource_file(icons: &[(u32, Vec<u8>)], info: &VersionInfo<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    // The empty resource every .res file starts with.
    resource(&mut out, 0, 0, 0, &[]);
    let mut group = Vec::new();
    push16(&mut group, 0); // reserved
    push16(&mut group, 1); // type: icon
    push16(&mut group, icons.len() as u16);
    for (n, (size, png)) in icons.iter().enumerate() {
        let id = n as u16 + 1;
        // Moveable and discardable, as resource compilers write icons.
        resource(&mut out, RT_ICON, id, 0x1010, png);
        let side = if *size >= 256 { 0 } else { *size as u8 };
        group.extend([side, side, 0, 0]); // width, height, colours, reserved
        push16(&mut group, 1); // planes
        push16(&mut group, 32); // bits per pixel
        push32(&mut group, png.len() as u32);
        push16(&mut group, id);
    }
    // Icon group 1 is the program's icon: Windows shows the first group.
    resource(&mut out, RT_GROUP_ICON, 1, 0x1030, &group);
    resource(&mut out, RT_VERSION, 1, 0x0030, &version_info(info));
    out
}

/// Appends one resource: a 32-byte header with numeric type and name, then
/// the data padded to four bytes.
fn resource(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    push32(out, data.len() as u32);
    push32(out, 32); // header size
    push16(out, 0xFFFF);
    push16(out, kind);
    push16(out, 0xFFFF);
    push16(out, name);
    push32(out, 0); // data version
    push16(out, flags);
    push16(out, if kind == 0 { 0 } else { LANGUAGE });
    push32(out, 0); // version
    push32(out, 0); // characteristics
    out.extend_from_slice(data);
    pad4(out);
}

/// The VS_VERSIONINFO block.
fn version_info(info: &VersionInfo<'_>) -> Vec<u8> {
    let [a, b, c, d] = info.version.map(u32::from);
    let mut fixed = Vec::new();
    for v in [
        0xFEEF_04BD, // signature
        0x0001_0000, // structure version 1.0
        (a << 16) | b,
        (c << 16) | d, // file version
        (a << 16) | b,
        (c << 16) | d, // product version
        0x3F,          // flags mask
        0,             // flags
        0x0004_0004,   // VOS_NT_WINDOWS32
        1,             // VFT_APP
        0,             // subtype
        0,
        0, // date
    ] {
        push32(&mut fixed, v);
    }
    let strings = [
        ("CompanyName", "The OxiGuitar contributors"),
        ("FileDescription", info.description),
        ("FileVersion", &info.version_text),
        ("InternalName", "oxiguitar"),
        ("LegalCopyright", info.copyright),
        ("OriginalFilename", info.original_filename),
        ("ProductName", info.product),
        ("ProductVersion", &info.version_text),
    ];
    let table: Vec<Vec<u8>> = strings
        .iter()
        .map(|(key, value)| {
            let text = utf16z(value);
            // A string's value length counts UTF-16 units, its terminator included.
            node(key, &text, (text.len() / 2) as u16, 1, &[])
        })
        .collect();
    let lang = format!("{LANGUAGE:04X}{CODE_PAGE:04X}");
    let string_table = node(&lang, &[], 0, 1, &table);
    let string_file_info = node("StringFileInfo", &[], 0, 1, &[string_table]);
    let mut translation = Vec::new();
    push16(&mut translation, LANGUAGE);
    push16(&mut translation, CODE_PAGE);
    let var = node("Translation", &translation, 4, 0, &[]);
    let var_file_info = node("VarFileInfo", &[], 0, 1, &[var]);
    node(
        "VS_VERSION_INFO",
        &fixed,
        fixed.len() as u16,
        0,
        &[string_file_info, var_file_info],
    )
}

/// One version-information block: its length, value length and type, its
/// key, its value and its children, each starting on four bytes. The
/// length covers everything but padding after the last child.
fn node(key: &str, value: &[u8], value_length: u16, kind: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0, 0];
    push16(&mut out, value_length);
    push16(&mut out, kind);
    out.extend(utf16z(key));
    pad4(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad4(&mut out);
        out.extend_from_slice(child);
    }
    let len = out.len() as u16;
    out[..2].copy_from_slice(&len.to_le_bytes());
    out
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn push16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes());
}

fn push32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
