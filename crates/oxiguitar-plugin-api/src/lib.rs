// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar plugin interface (docs/design/plugins.md, ADR 0008).
//!
//! A plugin is one native library file, `NAME.oxp`. This crate defines
//! everything that crosses between it and the OxiGuitar core:
//!
//! - [`abi`]: the two C tables, one the plugin exports through the symbol
//!   [`ENTRY_SYMBOL`] and one the host hands it at start. Every call passes
//!   a message as bytes and gets an answer as bytes.
//! - [`message`]: the messages, encoded with postcard ([`encode`],
//!   [`decode`]).
//! - [`manifest`]: what a plugin declares, as JSON in its
//!   [`MANIFEST_SECTION`], so the host learns it without loading any code.
//! - [`score`] and [`score2`]: the score as plugins see it, versions 1 and
//!   2 (interface 1.2). It mirrors the core's model but is versioned on its
//!   own, so the model can keep changing without breaking plugins; version
//!   2 grows through lists of extensions instead of new versions.
//! - [`signature`] and [`class`]: how the host picks a format plugin for a
//!   file and an instrument plugin for a track, from the manifest alone.
//! - [`view`]: the plugin's own UI, components of its `.slint` file that
//!   the core runs (#346), and [`display`]: what a plugin draws on the
//!   page.
//! - [`event`]: what the core pushes to plugins as it happens: the score
//!   opened or changed, the selection, the playback position (interface
//!   1.1).
//! - [`realtime`]: the audio thread's plain C calls to instruments and
//!   effects, with the notes as they cross (interface 1.3).
//!
//! Plugin authors don't use this crate directly: the SDK, `oxiguitar-plugin`,
//! wraps it in safe Rust. This crate holds no `unsafe` code; the tables'
//! raw pointers are used only by the SDK's export glue and the host's
//! loader.
//!
//! # Versions
//!
//! The interface has a major and a minor version ([`INTERFACE`]).
//!
//! - The major version is in the entry symbol's name
//!   (`oxiguitar_plugin_v1`) and in the manifest. A host never loads a
//!   plugin built for another major version.
//! - A minor version only adds: new messages, new variants at the *end* of
//!   an existing enum, new fields at the end of a C table (each table starts
//!   with its own size), new manifest keys. Nothing that exists changes. A
//!   plugin states the minor version it was built for; the host sends it
//!   only what that minor version knows.
//! - Interface 2.0 (OxiGuitar 0.24, mateusrdp/oxiguitar#346) is interface
//!   1.16 without the plugin UI the core drew: tabs and dialogs made of
//!   widget rows, the canvas and surfaces, track sections' widgets, and the
//!   settings windows and named shared folders 1.9 and 1.10 had already
//!   retired. A plugin's UI is its own `.slint` file ([`view`]). A host for
//!   interface 2 loads no plugin built for interface 1.
//! - What interface 2.0 holds from 1.x, by the minor version that brought
//!   it. 1.0: formats, instruments (declared), services, footer messages
//!   and settings. 1.1: pushed events, the open score. 1.2: the score
//!   version 2, a selection that names voice, string and several tracks,
//!   and edits a plugin submits as undoable commands, with plugin tools.
//!   1.3: track sections, footer messages with a link, the plugin's own
//!   folders and the shared ones, instruments, effects and audio devices
//!   on the audio thread, and the `cpu` condition. 1.4: exporters in File
//!   → Export, plugins that need or replace other plugins and advice
//!   dialogs. 1.5: effects that follow their track's settings while the
//!   song plays. 1.6 and 1.7: the score version 2's notation and XML
//!   signatures. 1.8: tools on the palette, menus and toolbar, plugin
//!   data, drawing and performing. 1.9: tools that don't edit the score.
//!   1.11: an export's own choices. 1.12 and 1.13: performing and drawing
//!   as they are now. 1.14: running tools, measuring text, copying. 1.15:
//!   views in Slint and the plugin's files. 1.16: plugin files as
//!   archives, with the `.slint` file and assets beside the manifest.
//!
//! # Licence
//!
//! MIT OR Apache-2.0, unlike the GPL-3.0 core, so plugins may use any
//! GPL-compatible licence (Mateus, 18 Oct 2026).

pub mod abi;
pub mod audio_device;
pub mod class;
pub mod display;
pub mod event;
pub mod manifest;
pub mod message;
pub mod realtime;
pub mod score;
pub mod score2;
pub mod signature;
pub mod view;

mod blob;

pub use blob::Blob;
pub use message::{DecodeError, decode, encode};

/// The interface version this crate describes: (major, minor).
pub const INTERFACE: (u32, u32) = (2, 0);

/// The symbol a plugin exports. It returns a pointer to the plugin's
/// [`abi::PluginTableV1`].
pub const ENTRY_SYMBOL: &str = "oxiguitar_plugin_v2";

/// The name of the section holding the manifest, on ELF (Linux, Android)
/// and Mach-O (macOS; in the `__DATA` segment).
pub const MANIFEST_SECTION: &str = "oxg_manifest";

/// The segment holding [`MANIFEST_SECTION`] in a Mach-O file.
pub const MACHO_MANIFEST_SEGMENT: &str = "__DATA";

/// The name of the section holding the manifest in a PE file (Windows).
/// Section names in Windows executables and libraries hold at most eight
/// characters (the PE format has no string table for longer ones), so the
/// name is shortened there.
pub const PE_MANIFEST_SECTION: &str = "oxgmanif";

/// A plugin file's extension, without the dot.
pub const PLUGIN_EXTENSION: &str = "oxp";

/// What a plugin file holds (since OxiGuitar 0.23; #347). An `.oxp` file
/// is an uncompressed tar archive (ustar or GNU format):
///
/// 1. [`oxp::MANIFEST`], always the first member, so a host reads a
///    plugin's id and version from the first few kilobytes of the file;
/// 2. [`oxp::library`], the plugin's code for the system the file is
///    built for;
/// 3. anything else the plugin ships (its `.slint` files, pictures, data),
///    under any path, as plain files and folders.
///
/// The host unpacks the copy it uses into the user's OxiGuitar folder once
/// and keeps it there, loading the library from there and giving the
/// plugin that folder ([`message::HostRequest::OwnFiles`]); it unpacks
/// again only when the file it uses changes. Links, absolute paths and
/// paths with `..` are refused.
pub mod oxp {
    /// The manifest's name in a plugin file: the archive's first member.
    pub const MANIFEST: &str = "manifest.json";

    /// The library's name in a plugin file for each system: Linux, Windows,
    /// macOS (one universal file for both processors).
    pub const LIBRARIES: [&str; 3] = ["plugin.so", "plugin.dll", "plugin.dylib"];

    /// The folders beside a plugin crate's `Cargo.toml` that go into its
    /// plugin file under their own names, when they exist: its `.slint`
    /// files and what else it ships.
    pub const FOLDERS: [&str; 2] = ["ui", "assets"];

    /// The library's name in a plugin file for the system this is built
    /// for.
    pub const fn library() -> &'static str {
        if cfg!(target_os = "windows") {
            LIBRARIES[1]
        } else if cfg!(target_vendor = "apple") {
            LIBRARIES[2]
        } else {
            LIBRARIES[0]
        }
    }
}

/// The largest manifest a host reads, in bytes. Real manifests are a few
/// kilobytes; the limit stops a damaged file from costing time at start.
pub const MAX_MANIFEST_LEN: usize = 1 << 20;
