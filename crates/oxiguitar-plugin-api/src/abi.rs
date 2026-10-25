// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The C tables between a plugin and the host (version 1).
//!
//! ```text
//!  host                                      plugin (.oxp)
//!  ────                                      ─────────────
//!  oxiguitar_plugin_v1()  ─────────────────▶ &PluginTableV1
//!  table.start(&HostTableV1, sink) ────────▶ builds the plugin
//!  table.call(request, sink)  ─────────────▶ answers through sink.write
//!                    ◀─────────────────────  host.call(ctx, request, sink)
//! ```
//!
//! Every call carries one message ([`crate::message`]) as bytes and gets
//! its answer through a [`RawSink`]: the callee hands the answer's bytes to
//! `sink.write`, which copies them into memory the caller owns. No memory
//! allocated on one side is ever freed on the other, so the two sides may
//! use different allocators.
//!
//! Both tables start with their own size. A later minor version adds
//! fields at the end only; a reader uses a field only when the table's size
//! covers it.
//!
//! Plugins are called from any thread, and from several threads at once.
//! The host's table may be called from any thread too, and for as long as
//! the process runs: plugins are never unloaded.

use std::ffi::c_void;

/// A borrowed run of bytes: valid for the duration of the call it is
/// passed to, unless the call's documentation says otherwise.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawBytes {
    /// The first byte; may be dangling (but not null) when `len` is 0.
    pub ptr: *const u8,
    /// The number of bytes.
    pub len: usize,
}

impl RawBytes {
    /// Borrows `bytes`.
    pub const fn of(bytes: &[u8]) -> RawBytes {
        RawBytes {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        }
    }
}

/// Where a callee writes its answer. The callee calls `write` any number
/// of times, each call appending bytes to the answer, and never after it
/// has returned.
#[repr(C)]
#[derive(Debug)]
pub struct RawSink {
    /// The caller's state; passed back to `write` unchanged.
    pub ctx: *mut c_void,
    /// Appends `bytes` (valid for this call only) to the answer.
    pub write: unsafe extern "C" fn(ctx: *mut c_void, bytes: RawBytes),
}

/// How a call ended, and what the bytes written to its sink are.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Status(pub u32);

impl Status {
    /// The call succeeded; the answer is the encoded answer message.
    pub const OK: Status = Status(0);
    /// The call failed; the answer is an encoded [`crate::message::Error`].
    pub const FAILED: Status = Status(1);
    /// The callee panicked and caught it; the answer is the panic's text
    /// (UTF-8). The callee is still usable.
    pub const PANICKED: Status = Status(2);
    /// The request couldn't be decoded; the answer is why (UTF-8 text).
    pub const MALFORMED: Status = Status(3);
    /// `call` came before a successful `start`; the answer is empty.
    pub const NOT_STARTED: Status = Status(4);
}

/// What a plugin exports, through the symbol [`crate::ENTRY_SYMBOL`]:
/// `extern "C" fn oxiguitar_plugin_v1() -> *const PluginTableV1`. The
/// table and everything it points to live as long as the library is
/// loaded, which is until the process ends.
#[repr(C)]
#[derive(Debug)]
pub struct PluginTableV1 {
    /// `size_of::<PluginTableV1>()` in the plugin's build.
    pub size: u32,
    /// The interface's minor version the plugin was built for.
    pub interface_minor: u32,
    /// The manifest: the same bytes as the plugin's manifest section.
    pub manifest: RawBytes,
    /// Builds the plugin. The host calls it once, before any `call`, with
    /// its own table, which stays valid until the process ends. On failure
    /// the plugin writes an encoded [`crate::message::Error`] to `answer`
    /// and returns [`Status::FAILED`]; the host then never calls it again.
    pub start: unsafe extern "C" fn(host: *const HostTableV1, answer: *mut RawSink) -> Status,
    /// Answers one encoded [`crate::message::Request`].
    pub call: unsafe extern "C" fn(request: RawBytes, answer: *mut RawSink) -> Status,
    /// The functions the audio thread calls (interface 1.3): null for a
    /// plugin with no instrument or effect. Read only when `size` covers
    /// it and `interface_minor` is 3 or more.
    pub realtime: *const crate::realtime::RealtimeTableV1,
}

/// Interface 1.0's plugin table: [`PluginTableV1`] up to `call`, which a
/// host reads from a plugin built before interface 1.3 (whose table ends
/// there).
#[repr(C)]
#[derive(Debug)]
pub struct PluginTableV1_0 {
    /// As [`PluginTableV1::size`].
    pub size: u32,
    /// As [`PluginTableV1::interface_minor`].
    pub interface_minor: u32,
    /// As [`PluginTableV1::manifest`].
    pub manifest: RawBytes,
    /// As [`PluginTableV1::start`].
    pub start: unsafe extern "C" fn(host: *const HostTableV1, answer: *mut RawSink) -> Status,
    /// As [`PluginTableV1::call`].
    pub call: unsafe extern "C" fn(request: RawBytes, answer: *mut RawSink) -> Status,
}

/// What the host hands a plugin at start: the host's services, which the
/// plugin calls with encoded [`crate::message::HostRequest`]s.
#[repr(C)]
#[derive(Debug)]
pub struct HostTableV1 {
    /// `size_of::<HostTableV1>()` in the host's build.
    pub size: u32,
    /// The interface's minor version the host was built for.
    pub interface_minor: u32,
    /// The host's state for this plugin; passed back to `call` unchanged.
    pub ctx: *const c_void,
    /// Answers one encoded [`crate::message::HostRequest`].
    pub call:
        unsafe extern "C" fn(ctx: *const c_void, request: RawBytes, answer: *mut RawSink) -> Status,
}

/// The type of the entry symbol.
pub type EntryV1 = unsafe extern "C" fn() -> *const PluginTableV1;

/// `size_of::<PluginTableV1>()` as a `u32`, for the table's `size` field.
pub const PLUGIN_TABLE_V1_SIZE: u32 = std::mem::size_of::<PluginTableV1>() as u32;

/// The size of interface 1.0's plugin table, the smallest a host takes:
/// everything before `realtime`.
pub const PLUGIN_TABLE_V1_0_SIZE: u32 = std::mem::offset_of!(PluginTableV1, realtime) as u32;

/// `size_of::<RealtimeTableV1>()` as a `u32`.
pub const REALTIME_TABLE_V1_SIZE: u32 =
    std::mem::size_of::<crate::realtime::RealtimeTableV1>() as u32;

/// `size_of::<HostTableV1>()` as a `u32`, for the table's `size` field.
pub const HOST_TABLE_V1_SIZE: u32 = std::mem::size_of::<HostTableV1>() as u32;
