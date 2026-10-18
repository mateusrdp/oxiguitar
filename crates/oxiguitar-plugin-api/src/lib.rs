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
//! - [`score`]: the score as plugins see it. It mirrors the core's model
//!   but is versioned on its own, so the model can keep changing without
//!   breaking plugins.
//! - [`signature`] and [`class`]: how the host picks a format plugin for a
//!   file and an instrument plugin for a track, from the manifest alone.
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
//! - Until OxiGuitar 0.2 is released, version 1.0 is still being designed
//!   and may change. From 0.2 on it is frozen.
//!
//! # Licence
//!
//! MIT OR Apache-2.0, unlike the GPL-3.0 core, so plugins may use any
//! GPL-compatible licence (Mateus, 18 Oct 2026).

pub mod abi;
pub mod class;
pub mod manifest;
pub mod message;
pub mod score;
pub mod signature;

mod blob;

pub use blob::Blob;
pub use message::{DecodeError, decode, encode};

/// The interface version this crate describes: (major, minor).
pub const INTERFACE: (u32, u32) = (1, 0);

/// The symbol a plugin exports. It returns a pointer to the plugin's
/// [`abi::PluginTableV1`].
pub const ENTRY_SYMBOL: &str = "oxiguitar_plugin_v1";

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

/// The largest manifest a host reads, in bytes. Real manifests are a few
/// kilobytes; the limit stops a damaged file from costing time at start.
pub const MAX_MANIFEST_LEN: usize = 1 << 20;
