// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Loading a plugin's code and calling it: the C side of the host.
//!
//! This module and the SDK's export glue are the only places in OxiGuitar
//! with `unsafe` code (docs/design/plugins.md, "Where the `unsafe` is").
//! Every `unsafe` block says why it is sound. What the loader relies on is
//! the plugin's side of the contract in `oxiguitar_plugin_api::abi`:
//!
//! - the entry symbol has the type `EntryV1` and returns a table that lives
//!   as long as the library is loaded;
//! - the table's functions may be called from any thread, at once;
//! - a callee copies nothing out of a sink after it returns, and every
//!   pointer it is given is used only during the call.
//!
//! Everything else is checked: a missing symbol, a null or misaligned
//! table, a table too small for this version, a manifest that differs from
//! the file's manifest section, a plugin built for a later interface.
//!
//! A library is never unloaded: unloading native code safely is unreliable
//! (thread-local destructors, threads the plugin started), so every loaded
//! library, its table and the host table it was given live until the
//! process ends.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicPtr, Ordering};

use oxiguitar_plugin_api::abi::{
    EntryV1, HOST_TABLE_V1_SIZE, HostTableV1, PLUGIN_TABLE_V1_0_SIZE, PLUGIN_TABLE_V1_SIZE,
    PluginTableV1, PluginTableV1_0, REALTIME_TABLE_V1_SIZE, RawBytes, RawSink, Status,
};
use oxiguitar_plugin_api::realtime::{ProcessCall, RealtimeTableV1, RenderCall};

/// What the host answers a plugin's requests with: encoded
/// `HostRequest` in, a status and the answer's bytes out.
pub(crate) type HostCalls = dyn Fn(&[u8]) -> (Status, Vec<u8>) + Send + Sync;

/// A loaded plugin: the functions of its table, valid until the process
/// ends.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Loaded {
    call: unsafe extern "C" fn(request: RawBytes, answer: *mut RawSink) -> Status,
    /// Its real-time functions (interface 1.3), if it has any.
    realtime: Option<&'static RealtimeTableV1>,
}

// SAFETY: the tables are never written once the plugin returned them, and
// the interface lets their functions be called from any thread (the
// real-time ones under the rules `Loaded::render` states).
unsafe impl Send for Loaded {}
// SAFETY: as for `Send`.
unsafe impl Sync for Loaded {}

/// The entry symbol's name, with the terminating zero `libloading` wants.
const ENTRY: &[u8] = b"oxiguitar_plugin_v1\0";

/// Loads the plugin library at `path`, checks its table against `manifest`
/// (the bytes of its manifest section) and starts it, handing it a host
/// table whose requests `host` answers. The error is one sentence for the
/// user, without the plugin's name.
///
/// Loading runs the library's own initialisation code, and from then on the
/// plugin can do anything the user can: a plugin file is trusted like the
/// program itself (docs/design/plugins.md, "Privacy and trust").
pub(crate) fn load(path: &Path, manifest: &[u8], host: Box<HostCalls>) -> Result<Loaded, String> {
    // SAFETY: loading a library runs its initialisers, which Rust can't
    // check. The plugin folders hold code the user installed and trusts
    // like OxiGuitar itself (NET-4); nothing on our side can make this
    // safer, and the plugin is loaded only after its manifest was read.
    let library = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("it couldn't be loaded ({e})"))?;
    // Never unloaded: the library lives until the process ends, and with it
    // every pointer into it.
    let library: &'static libloading::Library = Box::leak(Box::new(library));
    // SAFETY: the interface gives the entry symbol the type `EntryV1`. A
    // library exporting this name with another type breaks the interface;
    // nothing can detect that from outside. The symbol stays valid because
    // the library is never unloaded.
    let entry: EntryV1 = unsafe { library.get::<EntryV1>(ENTRY) }
        .map(|symbol| *symbol)
        .map_err(|_| "it has no oxiguitar_plugin_v1 entry: it was built for another interface version, or isn't an OxiGuitar plugin".to_owned())?;
    // SAFETY: `entry` is the plugin's entry function, loaded just above and
    // valid for the rest of the process; it takes no arguments.
    let table = unsafe { entry() };
    if table.is_null() || !table.is_aligned() {
        return Err("its entry returned no table".into());
    }
    // SAFETY: `table` is non-null and aligned, and the interface says it
    // points to a table that lives as long as the library, which is for
    // the rest of the process. `size` is the first field of every version
    // of the `repr(C)` table and is read alone, before any other field.
    let size = unsafe { (*table).size };
    if size < PLUGIN_TABLE_V1_0_SIZE {
        return Err(format!(
            "its table is {size} bytes, smaller than interface 1.0's {PLUGIN_TABLE_V1_0_SIZE}"
        ));
    }
    // SAFETY: as above, and the table holds at least interface 1.0's fields
    // (checked just above), which `PluginTableV1_0` is: the same `repr(C)`
    // fields as the start of `PluginTableV1`. It is never written by either
    // side once returned.
    let table: &'static PluginTableV1_0 = unsafe { &*table.cast::<PluginTableV1_0>() };
    // Interface 1.3's real-time table, read only from a table that holds
    // the field and a plugin built for 1.3 or later.
    let realtime = if size >= PLUGIN_TABLE_V1_SIZE && table.interface_minor >= 3 {
        // SAFETY: the table is a whole `PluginTableV1` (its size, checked
        // just above), so its `realtime` field is there to read.
        let rt = unsafe { (*std::ptr::from_ref(table).cast::<PluginTableV1>()).realtime };
        if rt.is_null() || !rt.is_aligned() {
            None
        } else {
            // SAFETY: non-null and aligned, and the interface says it lives
            // as long as the library, which is for the rest of the process;
            // `size` comes first and is read alone.
            let rt_size = unsafe { (*rt).size };
            if rt_size < REALTIME_TABLE_V1_SIZE {
                return Err(format!(
                    "its real-time table is {rt_size} bytes, smaller than interface 1.3's {REALTIME_TABLE_V1_SIZE}"
                ));
            }
            // SAFETY: as above; the table holds every field (checked).
            Some(unsafe { &*rt })
        }
    } else {
        None
    };
    if table.manifest.len > oxiguitar_plugin_api::MAX_MANIFEST_LEN {
        return Err("its table's manifest is too large (the file is damaged)".into());
    }
    // SAFETY: the table's `manifest` points into the library's manifest
    // section, which lives as long as the library, that is, forever; its
    // length is a manifest's at most (checked just above).
    let carried = unsafe { slice(table.manifest) };
    let end = carried.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    if &carried[..end] != manifest {
        return Err(
            "its table's manifest differs from the manifest in its file (the file is damaged)"
                .into(),
        );
    }
    let host_minor = oxiguitar_plugin_api::INTERFACE.1;
    if table.interface_minor > host_minor {
        return Err(format!(
            "it needs interface 1.{}, and this OxiGuitar has 1.{host_minor}: update OxiGuitar",
            table.interface_minor
        ));
    }

    // The host's table and what answers it, for the rest of the process.
    // The box is boxed again because `ctx` is a thin pointer and a
    // `&HostCalls` is not.
    #[allow(clippy::borrowed_box)]
    let host: &'static Box<HostCalls> = Box::leak(Box::new(host));
    let host_table: &'static HostTableV1 = Box::leak(Box::new(HostTableV1 {
        size: HOST_TABLE_V1_SIZE,
        interface_minor: host_minor,
        ctx: std::ptr::from_ref::<Box<HostCalls>>(host).cast(),
        call: host_call,
        render: host_render,
    }));
    let mut answer: Vec<u8> = Vec::new();
    let mut sink = RawSink {
        ctx: (&raw mut answer).cast(),
        write: collect,
    };
    // SAFETY: `start` is the plugin's, valid for the rest of the process.
    // `host_table` is leaked, so valid for the rest of the process as the
    // interface requires; `sink` and `answer` live until after the call and
    // nothing else touches them meanwhile.
    let status = unsafe { (table.start)(host_table, &raw mut sink) };
    oxiguitar_plugin_api::message::outcome(status, &answer)
        .map_err(|e| format!("it didn't start: {}", e.message))?;
    Ok(Loaded {
        call: table.call,
        realtime,
    })
}

impl Loaded {
    /// Sends an encoded `Request` and returns the status and the answer's
    /// bytes.
    pub(crate) fn call(&self, request: &[u8]) -> (Status, Vec<u8>) {
        let mut answer: Vec<u8> = Vec::new();
        let mut sink = RawSink {
            ctx: (&raw mut answer).cast(),
            write: collect,
        };
        // SAFETY: the table's `call` is valid for the rest of the process
        // (`load`). `request`, `sink` and `answer` live until after the
        // call returns and nothing else touches `answer` meanwhile; the
        // plugin may be called from any thread (the interface's contract).
        let status = unsafe { (self.call)(RawBytes::of(request), &raw mut sink) };
        (status, answer)
    }

    /// Whether the plugin has real-time functions (interface 1.3).
    pub(crate) fn is_realtime(&self) -> bool {
        self.realtime.is_some()
    }

    /// Renders an instrument instance, on the audio thread.
    ///
    /// The caller keeps the interface's rules for `instance`: one this
    /// plugin made and hasn't released, rendered by one thread at a time.
    /// Every pointer in `call` is valid for the call (the safe wrapper in
    /// `realtime.rs` builds it from slices).
    pub(crate) fn render(&self, instance: u64, call: &RenderCall) -> Status {
        let Some(rt) = self.realtime else {
            return Status::FAILED;
        };
        // SAFETY: the plugin's `render` is valid for the rest of the
        // process (`load`). `call` and what it points to are valid for the
        // call, and `instance` is used as the interface requires (this
        // function's contract, kept by `realtime::Instance`, which owns the
        // instance and is not `Sync`).
        unsafe { (rt.render)(instance, call) }
    }

    /// Runs an effect instance, on the audio thread, under the same rules
    /// as [`Loaded::render`].
    pub(crate) fn process(&self, instance: u64, call: &ProcessCall) -> Status {
        let Some(rt) = self.realtime else {
            return Status::FAILED;
        };
        // SAFETY: as in `render`.
        unsafe { (rt.process)(instance, call) }
    }

    /// The plugin's allocations while rendering, or `u64::MAX` when it
    /// can't tell.
    pub(crate) fn allocations(&self) -> u64 {
        let Some(rt) = self.realtime else {
            return u64::MAX;
        };
        // SAFETY: the plugin's `allocations` is valid for the rest of the
        // process and takes nothing.
        unsafe { (rt.allocations)() }
    }
}

/// `HostTableV1::call`: answers a plugin's request with the `HostCalls`
/// behind `ctx`.
///
/// # Safety
///
/// `ctx` is the `ctx` of a host table made by [`load`]; `request` is valid
/// for this call; `answer` is null or points to a sink valid for this call.
unsafe extern "C" fn host_call(
    ctx: *const c_void,
    request: RawBytes,
    answer: *mut RawSink,
) -> Status {
    if ctx.is_null() {
        return Status::NOT_STARTED;
    }
    // SAFETY: `ctx` was made by `load` from a leaked `Box<HostCalls>`, which
    // lives for the rest of the process and is only ever read.
    let host = unsafe { &*ctx.cast::<Box<HostCalls>>() };
    // SAFETY: `request` is valid for this call (this function's contract).
    let request = unsafe { slice(request) };
    let (status, bytes) = match catch_unwind(AssertUnwindSafe(|| host(request))) {
        Ok(answered) => answered,
        Err(_) => (Status::PANICKED, b"the host failed answering it".to_vec()),
    };
    if !answer.is_null() {
        // SAFETY: `answer` is non-null and valid for this call (this
        // function's contract); its `write` copies `bytes` before it
        // returns, and `bytes` outlives the call.
        unsafe {
            let sink = &*answer;
            (sink.write)(sink.ctx, RawBytes::of(&bytes));
        }
    }
    status
}

/// A sink's `write` that appends to the `Vec<u8>` behind `ctx`.
///
/// # Safety
///
/// `ctx` points to a `Vec<u8>` no one else uses during the call, and
/// `bytes` is valid for the call.
unsafe extern "C" fn collect(ctx: *mut c_void, bytes: RawBytes) {
    // SAFETY: `ctx` is the `Vec<u8>` that `load` or `Loaded::call` set up,
    // alive and otherwise untouched for the call (this function's
    // contract).
    let answer = unsafe { &mut *ctx.cast::<Vec<u8>>() };
    // SAFETY: `bytes` is valid for this call (this function's contract).
    answer.extend_from_slice(unsafe { slice(bytes) });
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

/// What fills an audio stream's buffers: interleaved frames, their
/// channels, and the delay to the speaker in nanoseconds. Called on the
/// device's audio thread.
pub(crate) type StreamTarget = Box<dyn FnMut(&mut [f32], usize, u64) + Send>;

/// The most streams open at once.
const STREAMS: usize = 16;

/// The open streams' targets, by number less one: a stream's slot holds a
/// leaked `Box<StreamTarget>` from `register_stream` until
/// `unregister_stream`.
static TARGETS: [AtomicPtr<StreamTarget>; STREAMS] =
    [const { AtomicPtr::new(std::ptr::null_mut()) }; STREAMS];

/// Keeps `target` for a new stream; its number, or `None` when every slot
/// is taken.
pub(crate) fn register_stream(target: StreamTarget) -> Option<u64> {
    let boxed = Box::into_raw(Box::new(target));
    for (i, slot) in TARGETS.iter().enumerate() {
        if slot
            .compare_exchange(
                std::ptr::null_mut(),
                boxed,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            return Some(i as u64 + 1);
        }
    }
    // SAFETY: `boxed` was made just above and no slot took it.
    drop(unsafe { Box::from_raw(boxed) });
    None
}

/// Takes the target of stream `stream` back, once its device has closed it
/// (so no callback will use it again).
pub(crate) fn unregister_stream(stream: u64) -> Option<StreamTarget> {
    let slot = TARGETS.get(usize::try_from(stream).ok()?.checked_sub(1)?)?;
    let ptr = slot.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the slot held a box `register_stream` leaked, now taken out
    // of the slot, so no one else can reach it; the device closed the
    // stream before this was called, so its callback no longer uses it.
    Some(*unsafe { Box::from_raw(ptr) })
}

/// `HostTableV1::render`: fills a stream's buffer with what its target
/// makes, or silence for a stream that isn't open.
///
/// # Safety
///
/// `out` points to `frames * channels` floats, valid and the caller's for
/// the call; the call comes from the stream's own audio callback (never
/// two at once for one stream) and not after the stream was closed.
unsafe extern "C" fn host_render(
    _ctx: *const c_void,
    stream: u64,
    out: *mut f32,
    frames: u32,
    channels: u32,
    delay_ns: u64,
) {
    if out.is_null() {
        return;
    }
    let len = frames as usize * channels as usize;
    // SAFETY: `out` holds `len` floats for the call (this function's
    // contract).
    let out = unsafe { std::slice::from_raw_parts_mut(out, len) };
    let slot = usize::try_from(stream)
        .ok()
        .and_then(|s| s.checked_sub(1))
        .and_then(|s| TARGETS.get(s));
    let ptr = slot.map_or(std::ptr::null_mut(), |s| s.load(Ordering::Acquire));
    if ptr.is_null() {
        out.fill(0.0);
        return;
    }
    // SAFETY: the slot holds a live target (`register_stream`), used only
    // by this stream's callback, one call at a time, until it is closed
    // (this function's contract), so this is the only reference.
    let target = unsafe { &mut *ptr };
    if catch_unwind(AssertUnwindSafe(|| {
        target(out, channels as usize, delay_ns)
    }))
    .is_err()
    {
        out.fill(0.0);
    }
}
