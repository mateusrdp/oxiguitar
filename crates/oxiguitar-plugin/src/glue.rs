// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The export glue: the C side of a plugin, behind [`crate::export_plugin!`].
//!
//! This module and the host's loader are the only places in OxiGuitar with
//! `unsafe` code (docs/design/plugins.md, "Where the `unsafe` is"). Every
//! `unsafe` block says why it is sound. What the glue relies on is the
//! contract in `oxiguitar_plugin_api::abi`:
//!
//! - the host calls `start` once, before any `call`, with a host table that
//!   stays valid until the process ends;
//! - every pointer passed to a call (request bytes, answer sink) is valid
//!   for the duration of that call;
//! - a sink's `write` copies the bytes it is given before returning.
//!
//! Everything else is checked: null pointers, a host table too small for
//! this version, bytes that don't decode, a second `start`. A panic in the
//! plugin is caught here and answered as [`Status::PANICKED`]; it never
//! unwinds into the host.

#![allow(unsafe_code)]

use std::any::Any;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, OnceLock};

use oxiguitar_plugin_api::abi::{
    HOST_TABLE_V1_SIZE, HostTableV1, PLUGIN_TABLE_V1_SIZE, PluginTableV1, RawBytes, RawSink, Status,
};
use oxiguitar_plugin_api::message::{
    Error, ErrorKind, HostAnswer, HostRequest, Request, decode, encode, outcome,
};

use crate::{Host, HostServices, Plugin, dispatch};

/// The plugin, once the host has started it. One `.oxp` file holds one
/// plugin and its own copy of this crate, so one cell per library is one
/// per plugin.
static PLUGIN: OnceLock<Box<dyn Plugin>> = OnceLock::new();

/// A plugin's table, as `export_plugin!` stores it in a `static`.
#[derive(Debug)]
pub struct Table(PluginTableV1);

// SAFETY: a `Table` lives in a `static` and is never written after it is
// built. Its only raw pointer (in `manifest`) points into another `static`
// that is never written either, so sharing it between threads is sharing
// read-only memory. Function pointers are `Sync` already.
unsafe impl Sync for Table {}

impl Table {
    /// The table's address, which the entry symbol returns.
    pub fn get(&'static self) -> *const PluginTableV1 {
        &self.0
    }
}

/// The table of the plugin `P`, whose manifest section holds `manifest`.
pub const fn table<P: Plugin>(manifest: &'static [u8]) -> Table {
    Table(PluginTableV1 {
        size: PLUGIN_TABLE_V1_SIZE,
        interface_minor: oxiguitar_plugin_api::INTERFACE.1,
        manifest: RawBytes::of(manifest),
        start: start::<P>,
        call,
    })
}

/// `PluginTableV1::start`: builds the plugin.
///
/// # Safety
///
/// `host` is null or points to a host table that stays valid until the
/// process ends; `answer` is null or points to a sink valid for this call.
unsafe extern "C" fn start<P: Plugin>(host: *const HostTableV1, answer: *mut RawSink) -> Status {
    // SAFETY: `answer` is null or valid for this call (this function's
    // contract); `Sink` checks for null.
    let sink = unsafe { Sink::new(answer) };
    // SAFETY: `host` is null or valid until the process ends (this
    // function's contract); `HostTable::new` checks for null and size.
    let host = match unsafe { HostTable::new(host) } {
        Ok(h) => h,
        Err(e) => return sink.error(&e),
    };
    if PLUGIN.get().is_some() {
        return sink.error(&Error::new(ErrorKind::Failed, "it was started twice"));
    }
    match catch_unwind(AssertUnwindSafe(|| P::start(Host::new(Arc::new(host))))) {
        Ok(Ok(plugin)) => match PLUGIN.set(Box::new(plugin)) {
            Ok(()) => Status::OK,
            Err(_) => sink.error(&Error::new(ErrorKind::Failed, "it was started twice")),
        },
        Ok(Err(e)) => sink.error(&e),
        Err(panic) => sink.panicked(&*panic),
    }
}

/// `PluginTableV1::call`: answers one request.
///
/// # Safety
///
/// `request` is valid for this call; `answer` is null or points to a sink
/// valid for this call.
unsafe extern "C" fn call(request: RawBytes, answer: *mut RawSink) -> Status {
    // SAFETY: as in `start`, `answer` is null or valid for this call.
    let sink = unsafe { Sink::new(answer) };
    let Some(plugin) = PLUGIN.get() else {
        return Status::NOT_STARTED;
    };
    // SAFETY: `request` is valid for this call (this function's contract),
    // and the slice doesn't outlive it.
    let bytes = unsafe { slice(request) };
    let answered = catch_unwind(AssertUnwindSafe(|| match decode::<Request<'_>>(bytes) {
        Err(e) => Answered::Malformed(e.to_string()),
        Ok(request) => match dispatch(plugin.as_ref(), request).map(|a| encode(&a)) {
            Ok(Ok(bytes)) => Answered::Ok(bytes),
            Ok(Err(e)) => Answered::Failed(Error::new(
                ErrorKind::Failed,
                format!("its answer couldn't be encoded ({e})"),
            )),
            Err(e) => Answered::Failed(e),
        },
    }));
    match answered {
        Ok(Answered::Ok(bytes)) => {
            sink.write(&bytes);
            Status::OK
        }
        Ok(Answered::Failed(e)) => sink.error(&e),
        Ok(Answered::Malformed(why)) => {
            sink.write(why.as_bytes());
            Status::MALFORMED
        }
        Err(panic) => sink.panicked(&*panic),
    }
}

/// How a call went, before the answer is written.
enum Answered {
    Ok(Vec<u8>),
    Failed(Error),
    Malformed(String),
}

/// The bytes `raw` points to.
///
/// # Safety
///
/// `raw` is valid for the returned lifetime: `len` bytes readable from
/// `ptr`, unless `len` is 0.
unsafe fn slice<'a>(raw: RawBytes) -> &'a [u8] {
    if raw.len == 0 || raw.ptr.is_null() {
        return &[];
    }
    // SAFETY: `ptr` is non-null and `len` bytes from it are readable for
    // `'a` (this function's contract); `u8` has no alignment requirement.
    unsafe { std::slice::from_raw_parts(raw.ptr, raw.len) }
}

/// Where a call's answer goes.
struct Sink(*mut RawSink);

impl Sink {
    /// # Safety
    ///
    /// `raw` is null or points to a sink valid until this `Sink` is dropped.
    unsafe fn new(raw: *mut RawSink) -> Sink {
        Sink(raw)
    }

    /// Appends `bytes` to the answer; nothing if there is no sink.
    fn write(&self, bytes: &[u8]) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: the sink is valid while `self` lives (`Sink::new`), and
        // its `write` takes bytes valid for the call, which `bytes` is.
        unsafe {
            let sink = &*self.0;
            (sink.write)(sink.ctx, RawBytes::of(bytes));
        }
    }

    /// Answers with `error`.
    fn error(&self, error: &Error) -> Status {
        if let Ok(bytes) = encode(error) {
            self.write(&bytes);
        }
        Status::FAILED
    }

    /// Answers with a panic's text.
    fn panicked(&self, panic: &(dyn Any + Send)) -> Status {
        let text = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("no message");
        self.write(text.as_bytes());
        Status::PANICKED
    }
}

/// The host's table, as the plugin's [`Host`] reaches it.
struct HostTable {
    ctx: *const c_void,
    call: unsafe extern "C" fn(*const c_void, RawBytes, *mut RawSink) -> Status,
}

// SAFETY: the host's table and its `ctx` may be used from any thread, at
// once, until the process ends (`oxiguitar_plugin_api::abi`'s contract).
unsafe impl Send for HostTable {}
// SAFETY: as for `Send`.
unsafe impl Sync for HostTable {}

impl HostTable {
    /// Copies what this version uses out of the host's table.
    ///
    /// # Safety
    ///
    /// `table` is null or points to a host table valid until the process
    /// ends, of at least the size its own first field gives.
    unsafe fn new(table: *const HostTableV1) -> Result<HostTable, Error> {
        if table.is_null() {
            return Err(Error::new(ErrorKind::Failed, "the host gave no table"));
        }
        // SAFETY: `table` is non-null and valid (this function's contract).
        // `size` is the first field of a `repr(C)` table, present in every
        // version, and is read alone, before anything else is.
        let size = unsafe { (*table).size };
        if size < HOST_TABLE_V1_SIZE {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("the host's table is {size} bytes, older than this plugin's interface"),
            ));
        }
        // SAFETY: the table is at least as large as `HostTableV1`, checked
        // just above, so all its fields are there to read.
        let (ctx, call) = unsafe { ((*table).ctx, (*table).call) };
        Ok(HostTable { ctx, call })
    }
}

impl HostServices for HostTable {
    fn request(&self, request: HostRequest<'_>) -> Result<HostAnswer<'static>, Error> {
        let bytes = encode(&request).map_err(|e| Error::new(ErrorKind::Failed, e.to_string()))?;
        let mut answer: Vec<u8> = Vec::new();
        let mut sink = RawSink {
            ctx: (&raw mut answer).cast(),
            write: collect,
        };
        // SAFETY: the host's `call` and `ctx` are valid until the process
        // ends (`HostTable::new`). `bytes` and `sink` (with `answer`, which
        // `collect` writes through `ctx`) live until after the call
        // returns, and nothing else touches `answer` meanwhile.
        let status = unsafe { (self.call)(self.ctx, RawBytes::of(&bytes), &raw mut sink) };
        let answer = outcome(status, &answer)?;
        decode::<HostAnswer<'_>>(answer)
            .map(HostAnswer::into_owned)
            .map_err(|e| Error::new(ErrorKind::Failed, format!("the host's answer: {e}")))
    }
}

/// A sink's `write` that appends to the `Vec<u8>` behind `ctx`.
///
/// # Safety
///
/// `ctx` points to a `Vec<u8>` no one else uses during the call, and
/// `bytes` is valid for the call.
unsafe extern "C" fn collect(ctx: *mut c_void, bytes: RawBytes) {
    // SAFETY: `ctx` is the `Vec<u8>` `HostTable::request` set up, alive and
    // otherwise untouched for the call (this function's contract).
    let answer = unsafe { &mut *ctx.cast::<Vec<u8>>() };
    // SAFETY: `bytes` is valid for this call (this function's contract).
    answer.extend_from_slice(unsafe { slice(bytes) });
}
