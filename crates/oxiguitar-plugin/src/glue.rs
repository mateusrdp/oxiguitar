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
//!
//! The audio thread's side (interface 1.3): a prepared instrument or
//! effect is a `Box` the glue hands the host as a number (its address),
//! and the host's `render` and `process` calls name it. The host's side of
//! the contract (`oxiguitar_plugin_api::realtime`): it names only
//! instances this plugin made and hasn't released, never one instance from
//! two threads at once, and releases one only once it is no longer
//! rendered. The glue also counts the plugin's allocations made while it
//! renders ([`Counting`], the global allocator `export_plugin!` installs),
//! for the contract suite.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::cell::Cell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use oxiguitar_plugin_api::abi::{
    HOST_TABLE_V1_0_SIZE, HOST_TABLE_V1_SIZE, HostTableV1, PLUGIN_TABLE_V1_SIZE, PluginTableV1,
    REALTIME_TABLE_V1_SIZE, RawBytes, RawSink, Status,
};
use oxiguitar_plugin_api::message::{
    Error, ErrorKind, HostAnswer, HostRequest, Request, decode, encode, outcome,
};
use oxiguitar_plugin_api::realtime::{
    HostRender, PitchPoint, ProcessCall, RealtimeTableV1, RenderCall, RtEvent,
};

use crate::audio::{Instance, Keeper, Piece};
use crate::{Host, HostServices, Plugin, dispatch_kept};

/// The plugin, once the host has started it. One `.oxp` file holds one
/// plugin and its own copy of this crate, so one cell per library is one
/// per plugin.
static PLUGIN: OnceLock<Box<dyn Plugin>> = OnceLock::new();

/// A plugin's table, as `export_plugin!` stores it in a `static`.
#[derive(Debug)]
pub struct Table(PluginTableV1);

/// The audio thread's functions, the same for every plugin.
struct Realtime(RealtimeTableV1);

// SAFETY: a table of function pointers and its size, never written.
unsafe impl Sync for Realtime {}

static REALTIME: Realtime = Realtime(RealtimeTableV1 {
    size: REALTIME_TABLE_V1_SIZE,
    render,
    process,
    allocations,
});

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
        realtime: &REALTIME.0,
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
        Ok(request) => match dispatch_kept(plugin.as_ref(), request, &Boxes).map(|a| encode(&a)) {
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
    /// The host's `render`, for audio devices (interface 1.3).
    render: Option<HostRender>,
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
        if size < HOST_TABLE_V1_0_SIZE {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("the host's table is {size} bytes, older than interface 1.0's"),
            ));
        }
        // SAFETY: the table holds at least interface 1.0's fields, checked
        // just above, and `ctx` and `call` are among them.
        let (ctx, call) = unsafe { ((*table).ctx, (*table).call) };
        // SAFETY: `render` is read only from a table that holds it (its
        // size covers the whole of this version's table).
        let render = (size >= HOST_TABLE_V1_SIZE).then(|| unsafe { (*table).render });
        Ok(HostTable { ctx, call, render })
    }
}

impl HostServices for HostTable {
    fn audio_out(&self) -> Option<AudioOut> {
        self.render.map(|render| AudioOut {
            ctx: self.ctx,
            render,
        })
    }

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

/// An instance as the host holds it: the instance, and whether it
/// panicked (then it is silent from then on).
struct Held {
    instance: Instance,
    failed: bool,
}

/// Keeps instances as boxes whose addresses are their numbers.
struct Boxes;

impl Keeper for Boxes {
    fn keep(&self, instance: Instance) -> u64 {
        let held = Box::new(Held {
            instance,
            failed: false,
        });
        Box::into_raw(held) as usize as u64
    }

    fn release(&self, id: u64) -> bool {
        if id == 0 {
            return false;
        }
        // SAFETY: the host releases only a number `keep` gave it, once, and
        // only after its last `render` or `process` returned (the
        // interface's contract), so this is the box `keep` leaked and no
        // one else holds it.
        drop(unsafe { Box::from_raw(id as usize as *mut Held) });
        true
    }
}

/// The most buffers one render call hands an instrument: two for each of
/// 64 tracks. Further buffers are left silent.
pub const MAX_BUFFERS: usize = 128;

/// `RealtimeTableV1::render`.
///
/// # Safety
///
/// `instance` is one this plugin made and hasn't released, used by no
/// other thread during the call; `call` points to a render call whose
/// pointers are valid for the call: `event_count` events, `pitch_count`
/// points, `output_count` buffers of `frames` floats each, none of them
/// overlapping.
unsafe extern "C" fn render(instance: u64, call: *const RenderCall) -> Status {
    if instance == 0 || call.is_null() {
        return Status::FAILED;
    }
    // SAFETY: the number is a box `Boxes::keep` leaked, alive and used only
    // by this thread during the call (this function's contract).
    let held = unsafe { &mut *(instance as usize as *mut Held) };
    if held.failed {
        return Status::PANICKED;
    }
    let Instance::Instrument(renderer) = &mut held.instance else {
        return Status::FAILED;
    };
    // SAFETY: `call` is non-null and valid for the call; `size` comes
    // first in every version and is read alone.
    let size = unsafe { (*call).size };
    if (size as usize) < std::mem::size_of::<RenderCall>() {
        return Status::FAILED;
    }
    // SAFETY: the call is at least this version's size, checked above.
    let call = unsafe { &*call };
    // SAFETY: the events, points and buffers are valid for the call (this
    // function's contract); each slice is made only when its pointer isn't
    // null.
    let events: &[RtEvent] = unsafe { parts(call.events, call.event_count as usize) };
    // SAFETY: as above.
    let pitch: &[PitchPoint] = unsafe { parts(call.pitch, call.pitch_count as usize) };
    // SAFETY: as above.
    let pointers: &[*mut f32] = unsafe { parts(call.outputs, call.output_count as usize) };
    let frames = call.frames as usize;
    let mut buffers: [&mut [f32]; MAX_BUFFERS] = std::array::from_fn(|_| Default::default());
    let n = pointers.len().min(MAX_BUFFERS);
    for (slot, &ptr) in buffers.iter_mut().zip(pointers) {
        if !ptr.is_null() {
            // SAFETY: each buffer holds `frames` floats, valid and not
            // overlapping another for the call (this function's contract).
            *slot = unsafe { std::slice::from_raw_parts_mut(ptr, frames) };
        }
    }
    let ok = rendering(|| {
        catch_unwind(AssertUnwindSafe(|| {
            let mut piece = Piece::new(
                events,
                pitch,
                call.time,
                call.step,
                frames,
                &mut buffers[..n],
            );
            renderer.render(&mut piece);
        }))
        .is_ok()
    });
    if ok {
        Status::OK
    } else {
        held.failed = true;
        Status::PANICKED
    }
}

/// `RealtimeTableV1::process`.
///
/// # Safety
///
/// As [`render`]: an effect instance of this plugin, and a call whose two
/// buffers hold `frames` floats each, valid and apart for the call.
unsafe extern "C" fn process(instance: u64, call: *const ProcessCall) -> Status {
    if instance == 0 || call.is_null() {
        return Status::FAILED;
    }
    // SAFETY: as in `render`.
    let held = unsafe { &mut *(instance as usize as *mut Held) };
    if held.failed {
        return Status::PANICKED;
    }
    let Instance::Effect(processor) = &mut held.instance else {
        return Status::FAILED;
    };
    // SAFETY: as in `render`.
    let size = unsafe { (*call).size };
    if (size as usize) < std::mem::size_of::<ProcessCall>() {
        return Status::FAILED;
    }
    // SAFETY: as in `render`.
    let call = unsafe { &*call };
    // SAFETY: as in `render`.
    let pointers: &[*mut f32] = unsafe { parts(call.buffers, call.buffer_count as usize) };
    let (Some(&l), Some(&r)) = (pointers.first(), pointers.get(1)) else {
        return Status::FAILED;
    };
    if l.is_null() || r.is_null() || l == r {
        return Status::FAILED;
    }
    let frames = call.frames as usize;
    // SAFETY: two distinct buffers of `frames` floats, valid and apart for
    // the call (this function's contract).
    let (left, right) = unsafe {
        (
            std::slice::from_raw_parts_mut(l, frames),
            std::slice::from_raw_parts_mut(r, frames),
        )
    };
    let ok =
        rendering(|| catch_unwind(AssertUnwindSafe(|| processor.process(left, right))).is_ok());
    if ok {
        Status::OK
    } else {
        held.failed = true;
        Status::PANICKED
    }
}

/// `RealtimeTableV1::allocations`.
unsafe extern "C" fn allocations() -> u64 {
    if COUNTING.load(Ordering::Relaxed) {
        ALLOCATIONS.load(Ordering::Relaxed)
    } else {
        u64::MAX
    }
}

/// `len` values from `ptr`, or none when it is null.
///
/// # Safety
///
/// `ptr` is null or points to `len` values valid for `'a`.
unsafe fn parts<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: as this function's contract says.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

thread_local! {
    /// Whether this thread is in `render` or `process`.
    static RENDERING: Cell<bool> = const { Cell::new(false) };
}

/// Allocations made while rendering, since the plugin started.
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

/// Whether [`Counting`] is the plugin's allocator (it has been used).
static COUNTING: AtomicBool = AtomicBool::new(false);

fn rendering<T>(f: impl FnOnce() -> T) -> T {
    let _ = RENDERING.try_with(|r| r.set(true));
    let out = f();
    let _ = RENDERING.try_with(|r| r.set(false));
    out
}

fn counted() {
    COUNTING.store(true, Ordering::Relaxed);
    if RENDERING.try_with(Cell::get).unwrap_or(false) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

/// The global allocator `export_plugin!` gives every `.oxp` file: the
/// system's, counting what is allocated, reallocated or freed while the
/// plugin renders, which the real-time rule forbids (the contract suite
/// reads the count). A plugin built with `export_plugin!` sets no
/// allocator of its own.
#[derive(Debug)]
pub struct Counting;

// SAFETY: every call is forwarded unchanged to the system allocator; the
// count beside it allocates nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        counted();
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        counted();
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        counted();
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        counted();
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Where an audio device sends the core its buffers to fill (interface
/// 1.3): the host's `render`, from the device's callback.
#[derive(Clone, Copy, Debug)]
pub struct AudioOut {
    ctx: *const c_void,
    render: HostRender,
}

// SAFETY: the host's table and its `ctx` may be used from any thread until
// the process ends (the interface's contract); `render` is called under the
// rules `AudioOut::render` states.
unsafe impl Send for AudioOut {}
// SAFETY: as for `Send`.
unsafe impl Sync for AudioOut {}

impl AudioOut {
    /// Has the core fill `out`, interleaved frames of `channels` channels
    /// for the stream `stream`, which reach the speaker `delay_ns`
    /// nanoseconds from now. Call it only from the stream's audio callback,
    /// and never after the stream was closed (the interface's rules).
    pub fn render(&self, stream: u64, out: &mut [f32], channels: usize, delay_ns: u64) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        // SAFETY: the host's `render` is valid until the process ends;
        // `out` holds `frames * channels` floats, valid and ours for the
        // call; the caller keeps the stream's rules (this method's
        // contract).
        unsafe {
            (self.render)(
                self.ctx,
                stream,
                out.as_mut_ptr(),
                frames as u32,
                channels as u32,
                delay_ns,
            );
        }
    }
}
