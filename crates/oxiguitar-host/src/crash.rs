// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Saying which plugin was running when OxiGuitar crashed (Mateus, 16 Nov
//! 2026: a plugin may crash the app, which is fair, as long as the user
//! can tell it did and has something useful to send).
//!
//! Plugins run inside OxiGuitar's process, so a plugin that writes where
//! it shouldn't takes the whole program down. Two things make such a crash
//! tell on its plugin:
//!
//! - **Who was called.** Every call the host makes into a plugin (its
//!   start, each request, each block of sound an instrument renders or an
//!   effect processes) marks its thread with the plugin, and unmarks it
//!   when the call returns. A crash on that thread, in the
//!   plugin's code, in a library it calls, or in the host's answer to it,
//!   names the plugin.
//! - **Whose code it was.** On Linux the host also notes where each
//!   plugin's library is in memory, and the crash names the plugin whose
//!   code was running, on any thread, a plugin's own threads included.
//!
//! [`install`] sets this up: a handler for the signals of a crash (invalid
//! memory access, a bad instruction, an arithmetic fault, an abort) on Linux
//! and macOS, the unhandled-exception filter on Windows. It writes
//! `last-crash.txt` in the folder it is given, and the same lines to the
//! standard error, then lets the crash go on as it would have (Rust's own
//! report of a stack overflow included). [`last`] reads such a report at
//! the next start, once.
//!
//! A signal handler may do very little: no allocation, no locks. So what a
//! report says is prepared beforehand (each plugin's description, the
//! report's path), kept in fixed tables of atomics, and put together in a
//! buffer on the handler's stack.

#![allow(unsafe_code)]

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicUsize, Ordering};

/// The report's file name in the folder [`install`] is given.
pub const REPORT: &str = "last-crash.txt";

/// How many plugins the tables can name: more than any OxiGuitar holds.
const SLOTS: usize = 512;

/// Each plugin's description, `id version (file; homepage)`, leaked, and
/// its length. Slot 0 is no plugin.
static NAMES: [AtomicPtr<u8>; SLOTS] = [const { AtomicPtr::new(std::ptr::null_mut()) }; SLOTS];
static NAME_LENS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static NEXT: AtomicU32 = AtomicU32::new(1);

/// Where plugins' code is in memory: start, end and slot of each loaded
/// segment (Linux).
const RANGES: usize = 4 * SLOTS;
static RANGE_START: [AtomicUsize; RANGES] = [const { AtomicUsize::new(0) }; RANGES];
static RANGE_END: [AtomicUsize; RANGES] = [const { AtomicUsize::new(0) }; RANGES];
static RANGE_SLOT: [AtomicU32; RANGES] = [const { AtomicU32::new(0) }; RANGES];
/// The address the library's file is loaded at, so a report can say where
/// in the file the crash was.
static RANGE_BASE: [AtomicUsize; RANGES] = [const { AtomicUsize::new(0) }; RANGES];
static RANGE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// The report's path, NUL-terminated, leaked; and the program's own name
/// and version, leaked.
static REPORT_PATH: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static PROGRAM: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static PROGRAM_LEN: AtomicUsize = AtomicUsize::new(0);

/// Set by the first crash, so a crash during the report (or the abort a
/// crash leads to) writes no second one.
static WRITTEN: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The plugin this thread is running, by slot; 0 for none.
    static RUNNING: Cell<u32> = const { Cell::new(0) };
}

/// A plugin, as a crash report names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot(u32);

/// Makes a plugin known to crash reports: `description` is how a report
/// names it, `library` the file it is loaded from. Call it once a plugin,
/// before its library is loaded (the library's ranges are noted by
/// [`Slot::loaded`]).
pub(crate) fn register(description: String) -> Slot {
    let slot = NEXT.fetch_add(1, Ordering::Relaxed);
    if slot as usize >= SLOTS {
        return Slot(0);
    }
    let bytes: &'static mut [u8] = Box::leak(description.into_bytes().into_boxed_slice());
    NAME_LENS[slot as usize].store(bytes.len(), Ordering::Relaxed);
    NAMES[slot as usize].store(bytes.as_mut_ptr(), Ordering::Release);
    Slot(slot)
}

impl Slot {
    /// Marks the calling thread as running this plugin until the guard is
    /// dropped.
    pub(crate) fn running(self) -> Running {
        Running(RUNNING.with(|r| r.replace(self.0)))
    }

    /// Notes where the plugin's library, loaded from `library`, is in
    /// memory, so a crash in its code names it on any thread (Linux; a
    /// no-op elsewhere).
    pub(crate) fn loaded(self, library: &Path) {
        if self.0 != 0 {
            note_ranges(self.0, library);
        }
    }
}

/// The mark [`Slot::running`] puts on a thread; dropping it puts back what
/// was there (a plugin's call into the host that calls another plugin
/// comes back to the first).
#[must_use]
pub(crate) struct Running(u32);

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.with(|r| r.set(self.0));
    }
}

/// Sets crash reports up for the rest of the process: they are written to
/// `folder`'s [`REPORT`] (made if missing) and name the program as
/// `program` (such as "OxiGuitar 0.20.0"). Call it once, early; a second
/// call changes nothing. The error is a sentence.
pub fn install(folder: &Path, program: &str) -> Result<(), String> {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("cannot create {}: {e}", folder.display()))?;
    let path = folder.join(REPORT);
    let mut bytes = path.as_os_str().as_encoded_bytes().to_vec();
    if bytes.contains(&0) {
        return Err(format!("{} can't be a file name", path.display()));
    }
    bytes.push(0);
    REPORT_PATH.store(
        Box::leak(bytes.into_boxed_slice()).as_mut_ptr(),
        Ordering::Release,
    );
    let program: &'static mut [u8] = Box::leak(program.as_bytes().to_vec().into_boxed_slice());
    PROGRAM_LEN.store(program.len(), Ordering::Relaxed);
    PROGRAM.store(program.as_mut_ptr(), Ordering::Release);
    handlers::install();
    Ok(())
}

/// A crash report found at start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// What it says.
    pub text: String,
    /// Where it is kept now.
    pub path: PathBuf,
    /// The plugin it names, as its description (`id version (file;
    /// homepage)`), if it names one.
    pub plugin: Option<String>,
}

impl Report {
    /// The plugin's id, as its description gives it.
    pub fn plugin_id(&self) -> Option<&str> {
        self.plugin.as_deref()?.split(' ').next()
    }

    /// The homepage of the plugin it names, where its users report
    /// problems, if its manifest has one.
    pub fn homepage(&self) -> Option<&str> {
        let plugin = self.plugin.as_deref()?;
        let inside = plugin.strip_suffix(')')?.rsplit_once(" (")?.1;
        inside.rsplit_once("; ").map(|(_, home)| home)
    }

    /// What the window says about it at start: what crashed, where the
    /// report is, and what to do.
    pub fn advice(&self) -> oxiguitar_plugin_api::message::Advice {
        use oxiguitar_plugin_api::message::{Advice, Link};
        let report = self.path.display();
        let mut links = Vec::new();
        let text = match &self.plugin {
            Some(plugin) => {
                if let Some(home) = self.homepage() {
                    links.push(Link {
                        text: "The plugin's page".to_owned(),
                        url: home.to_owned(),
                    });
                }
                format!(
                    "It happened in the plugin {plugin}.\nThe report is in {report}: send it to the plugin's author, with what you were doing. If it happens again, remove the plugin's file from its folder, or start OxiGuitar with --safe-mode, which uses no plugins."
                )
            }
            None => {
                links.push(Link {
                    text: "OxiGuitar's issues".to_owned(),
                    url: ISSUES.to_owned(),
                });
                format!(
                    "No plugin was running on the thread that crashed.\nThe report is in {report}: please open an issue with it, and with what you were doing."
                )
            }
        };
        Advice {
            id: "last-crash".to_owned(),
            title: "OxiGuitar crashed the last time it ran".to_owned(),
            text,
            links,
        }
    }
}

/// Where OxiGuitar's own problems are reported.
const ISSUES: &str = "https://github.com/mateusrdp/oxiguitar/issues";

/// The report of a crash since the last start, if there was one: read from
/// `folder` and renamed with the time it was found (`crash-SECONDS.txt`),
/// so it is told once and kept.
pub fn last(folder: &Path) -> Option<Report> {
    let at = folder.join(REPORT);
    let text = std::fs::read_to_string(&at).ok()?;
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let kept = folder.join(format!("crash-{seconds}.txt"));
    let path = if std::fs::rename(&at, &kept).is_ok() {
        kept
    } else {
        at
    };
    let plugin = text
        .lines()
        .find_map(|l| {
            l.strip_prefix(IN_CODE)
                .or_else(|| l.strip_prefix(ON_THREAD))
        })
        .map(str::to_owned);
    Some(Report { text, path, plugin })
}

/// A report's line naming the plugin whose code crashed.
const IN_CODE: &str = "The crash was in the code of the plugin ";
/// A report's line naming the plugin the crashed thread was running.
const ON_THREAD: &str = "It happened while OxiGuitar was running the plugin ";

/// The description of the plugin in `slot`, if it has one.
fn name_of(slot: u32) -> Option<&'static [u8]> {
    let i = slot as usize;
    if slot == 0 || i >= SLOTS {
        return None;
    }
    let ptr = NAMES[i].load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    // SAFETY: a non-null entry is a leaked slice of the stored length,
    // written before the pointer was published (Release, read Acquire)
    // and never freed or changed.
    Some(unsafe { std::slice::from_raw_parts(ptr, NAME_LENS[i].load(Ordering::Relaxed)) })
}

/// The plugin whose code is at `pc`, if one's is, and the address its file
/// is loaded at; `(0, 0)` for none.
fn slot_at(pc: usize) -> (u32, usize) {
    if pc == 0 {
        return (0, 0);
    }
    let n = RANGE_COUNT.load(Ordering::Acquire).min(RANGES);
    (0..n)
        .find(|&i| {
            RANGE_START[i].load(Ordering::Relaxed) <= pc
                && pc < RANGE_END[i].load(Ordering::Relaxed)
        })
        .map_or((0, 0), |i| {
            (
                RANGE_SLOT[i].load(Ordering::Relaxed),
                RANGE_BASE[i].load(Ordering::Relaxed),
            )
        })
}

/// How long a report may be.
const REPORT_MAX: usize = 4096;

/// Where the handler puts its report together: not on its stack, which is
/// the thread's small alternate signal stack. Used once, by the first
/// crash ([`WRITTEN`] lets only that one in).
struct Space(std::cell::UnsafeCell<[u8; REPORT_MAX]>);

// SAFETY: only the one handler [`WRITTEN`] lets in touches it.
unsafe impl Sync for Space {}

static SPACE: Space = Space(std::cell::UnsafeCell::new([0; REPORT_MAX]));

/// The report's space, for the one crash that writes it.
///
/// # Safety
///
/// Only the handler that set [`WRITTEN`] may call it, once.
#[allow(clippy::mut_from_ref)]
unsafe fn space() -> &'static mut [u8] {
    // SAFETY: the caller is the only one ever to use it.
    unsafe { &mut *SPACE.0.get() }
}

/// A report put together without allocating, in space given to it.
struct Buffer<'a> {
    bytes: &'a mut [u8],
    len: usize,
}

impl<'a> Buffer<'a> {
    fn new(bytes: &'a mut [u8]) -> Buffer<'a> {
        Buffer { bytes, len: 0 }
    }

    fn push(&mut self, s: &[u8]) -> &mut Buffer<'a> {
        let n = s.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + n].copy_from_slice(&s[..n]);
        self.len += n;
        self
    }

    fn number(&mut self, mut n: u64) -> &mut Buffer<'a> {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(&digits[i..])
    }

    fn hex(&mut self, n: usize) -> &mut Buffer<'a> {
        let mut digits = [0u8; 16];
        for (i, d) in digits.iter_mut().enumerate() {
            let nibble = (n >> (4 * (15 - i))) & 0xf;
            *d = b"0123456789abcdef"[nibble];
        }
        self.push(b"0x").push(&digits)
    }
}

/// Puts the report together in `space`: `what` the crash was, `pc` where
/// (0 when unknown), `running` the slot the crashed thread was marked
/// with. Returns its length.
fn compose(space: &mut [u8], what: &[u8], pc: usize, running: u32, seconds: u64) -> usize {
    let mut b = Buffer::new(space);
    let program = PROGRAM.load(Ordering::Acquire);
    if program.is_null() {
        b.push(b"OxiGuitar");
    } else {
        // SAFETY: a leaked slice of the stored length, published by
        // `install` before handlers were installed and never freed.
        b.push(unsafe { std::slice::from_raw_parts(program, PROGRAM_LEN.load(Ordering::Relaxed)) });
    }
    b.push(b" crashed: ").push(what).push(b".\n");
    let (in_code, base) = slot_at(pc);
    if let Some(name) = name_of(in_code) {
        b.push(IN_CODE.as_bytes()).push(name).push(b"\n");
    }
    if let Some(name) = name_of(running).filter(|_| running != in_code) {
        b.push(ON_THREAD.as_bytes()).push(name).push(b"\n");
    }
    if in_code == 0 && name_of(running).is_none() {
        b.push(b"No plugin was running on the thread that crashed.\n");
    }
    b.push(b"Where: ");
    if pc == 0 {
        b.push(b"unknown");
    } else {
        b.hex(pc);
        // An offset in the plugin's file is what `addr2line -e FILE`
        // turns into a line of its source.
        if in_code != 0 {
            b.push(b", at ")
                .hex(pc.wrapping_sub(base))
                .push(b" in the plugin's file");
        }
    }
    b.push(b"\nSystem: ")
        .push(std::env::consts::OS.as_bytes())
        .push(b" ")
        .push(std::env::consts::ARCH.as_bytes())
        .push(b"\nTime: ")
        .number(seconds)
        .push(b" (seconds since 1970, UTC)\n");
    b.len
}

#[cfg(unix)]
mod handlers {
    //! Linux and macOS: the signals of a crash.

    use super::{Ordering, RUNNING, WRITTEN, compose};
    use std::cell::UnsafeCell;

    /// The signals a crash raises, with what a report calls each.
    const SIGNALS: [(libc::c_int, &[u8]); 5] = [
        (libc::SIGSEGV, b"invalid memory access (SIGSEGV)"),
        (libc::SIGBUS, b"invalid memory access (SIGBUS)"),
        (libc::SIGILL, b"an invalid instruction (SIGILL)"),
        (libc::SIGFPE, b"an arithmetic fault (SIGFPE)"),
        (libc::SIGABRT, b"it was aborted (SIGABRT)"),
    ];

    /// The actions in place before ours, put back once a report is
    /// written, so the crash goes on as it would have.
    struct Previous(UnsafeCell<[Option<libc::sigaction>; 5]>);

    // SAFETY: written only by `install`, once, before any handler that
    // reads it is installed; read only afterwards.
    unsafe impl Sync for Previous {}

    static PREVIOUS: Previous = Previous(UnsafeCell::new([None; 5]));

    pub(super) fn install() {
        for (i, &(signal, _)) in SIGNALS.iter().enumerate() {
            // SAFETY: zeroed is a valid `sigaction` (no handler, no flags,
            // empty mask); `sigaction` reads `ours` and writes `old`, both
            // valid for the call; `PREVIOUS` is written before the handler
            // that reads it is in place, and by this one thread.
            unsafe {
                let mut ours: libc::sigaction = std::mem::zeroed();
                ours.sa_sigaction = on_signal as *const () as usize;
                ours.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
                libc::sigemptyset(&raw mut ours.sa_mask);
                let mut old: libc::sigaction = std::mem::zeroed();
                if libc::sigaction(signal, std::ptr::null(), &raw mut old) == 0 {
                    (*PREVIOUS.0.get())[i] = Some(old);
                    libc::sigaction(signal, &raw const ours, std::ptr::null_mut());
                }
            }
        }
    }

    /// The instruction a signal interrupted, where it can be told.
    ///
    /// # Safety
    ///
    /// `context` is the third argument of an `SA_SIGINFO` handler.
    #[allow(unused_variables)]
    unsafe fn pc(context: *mut libc::c_void) -> usize {
        if context.is_null() {
            return 0;
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            // SAFETY: on Linux the context is a `ucontext_t`.
            let uc = unsafe { &*context.cast::<libc::ucontext_t>() };
            return uc.uc_mcontext.gregs[libc::REG_RIP as usize] as usize;
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            // SAFETY: as above.
            let uc = unsafe { &*context.cast::<libc::ucontext_t>() };
            return uc.uc_mcontext.pc as usize;
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // SAFETY: on macOS the context is a `ucontext_t` whose
            // `uc_mcontext` points to the thread's state.
            let uc = unsafe { &*context.cast::<libc::ucontext_t>() };
            if uc.uc_mcontext.is_null() {
                return 0;
            }
            return unsafe { (*uc.uc_mcontext).__ss.__pc } as usize;
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            // SAFETY: as above.
            let uc = unsafe { &*context.cast::<libc::ucontext_t>() };
            if uc.uc_mcontext.is_null() {
                return 0;
            }
            return unsafe { (*uc.uc_mcontext).__ss.__rip } as usize;
        }
        #[allow(unreachable_code)]
        0
    }

    extern "C" fn on_signal(
        signal: libc::c_int,
        info: *mut libc::siginfo_t,
        context: *mut libc::c_void,
    ) {
        let index = SIGNALS.iter().position(|&(s, _)| s == signal);
        if !WRITTEN.swap(true, Ordering::SeqCst) {
            let what = index.map_or(&b"a signal"[..], |i| SIGNALS[i].1);
            // SAFETY: the context of this `SA_SIGINFO` handler.
            let pc = unsafe { pc(context) };
            let running = RUNNING.with(std::cell::Cell::get);
            // SAFETY: `time` with a null pointer only returns the time.
            let seconds = unsafe { libc::time(std::ptr::null_mut()) }.max(0) as u64;
            // SAFETY: this handler set `WRITTEN`: the space is its alone.
            let space = unsafe { super::space() };
            let len = compose(space, what, pc, running, seconds);
            let text = &space[..len];
            // SAFETY: `write` and `open` are async-signal-safe; the buffer
            // is valid for its length; the path is NUL-terminated (made
            // so by `install`).
            unsafe {
                libc::write(2, b"\n".as_ptr().cast(), 1);
                libc::write(2, text.as_ptr().cast(), text.len());
                let path = super::REPORT_PATH.load(Ordering::Acquire);
                if !path.is_null() {
                    let fd = libc::open(
                        path.cast(),
                        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_CLOEXEC,
                        0o644,
                    );
                    if fd >= 0 {
                        libc::write(fd, text.as_ptr().cast(), text.len());
                        libc::close(fd);
                    }
                }
            }
        }
        // The crash goes on as it would have: the action before ours (Rust's
        // report of a stack overflow, or the default: the program ends),
        // run by the fault happening again when this handler returns, or
        // raised again for a signal sent from outside, which wouldn't.
        // SAFETY: `PREVIOUS` was written before this handler was installed;
        // `sigaction` and `raise` are async-signal-safe.
        unsafe {
            let previous = index.and_then(|i| (*PREVIOUS.0.get())[i]);
            let mut action: libc::sigaction = previous.unwrap_or_else(|| std::mem::zeroed());
            if action.sa_sigaction == libc::SIG_IGN {
                action.sa_sigaction = libc::SIG_DFL;
            }
            libc::sigaction(signal, &raw const action, std::ptr::null_mut());
            if info.is_null() || (*info).si_code <= 0 || signal == libc::SIGABRT {
                libc::raise(signal);
            }
        }
    }
}

#[cfg(windows)]
mod handlers {
    //! Windows: the unhandled-exception filter.

    use super::{Ordering, RUNNING, WRITTEN, compose};
    use std::ffi::c_void;

    /// The first fields of `EXCEPTION_RECORD`, as Windows lays it out.
    #[repr(C)]
    struct ExceptionRecord {
        code: u32,
        flags: u32,
        record: *mut ExceptionRecord,
        address: *mut c_void,
    }

    /// `EXCEPTION_POINTERS`.
    #[repr(C)]
    struct ExceptionPointers {
        record: *const ExceptionRecord,
        context: *mut c_void,
    }

    type Filter = unsafe extern "system" fn(*const ExceptionPointers) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    }

    static PREVIOUS: std::sync::OnceLock<Option<Filter>> = std::sync::OnceLock::new();

    pub(super) fn install() {
        // SAFETY: `on_exception` has the filter's signature and stays valid
        // for the rest of the process.
        let previous = unsafe { SetUnhandledExceptionFilter(Some(on_exception)) };
        let _ = PREVIOUS.set(previous);
    }

    unsafe extern "system" fn on_exception(pointers: *const ExceptionPointers) -> i32 {
        if !WRITTEN.swap(true, Ordering::SeqCst) {
            // SAFETY: Windows passes valid exception pointers to the filter.
            let (code, address) = unsafe {
                match pointers.as_ref().and_then(|p| p.record.as_ref()) {
                    Some(r) => (r.code, r.address as usize),
                    None => (0, 0),
                }
            };
            let what: &[u8] = match code {
                0xC000_0005 => b"invalid memory access (access violation)",
                0xC000_001D => b"an invalid instruction",
                0xC000_0094 | 0xC000_008E => b"an arithmetic fault",
                0xC000_00FD => b"a stack overflow",
                0xC000_0409 => b"it was aborted (fail fast)",
                _ => b"an unhandled exception",
            };
            let seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            // SAFETY: this filter set `WRITTEN`: the space is its alone.
            let space = unsafe { super::space() };
            let len = compose(
                space,
                what,
                address,
                RUNNING.with(std::cell::Cell::get),
                seconds,
            );
            let text = &space[..len];
            let path = super::REPORT_PATH.load(Ordering::Acquire);
            if !path.is_null() {
                // SAFETY: a leaked NUL-terminated path made by `install`.
                let path = unsafe { std::ffi::CStr::from_ptr(path.cast()) };
                if let Ok(path) = path.to_str() {
                    let _ = std::fs::write(path, text);
                }
            }
            let _ = std::io::Write::write_all(&mut std::io::stderr(), text);
        }
        match PREVIOUS.get().copied().flatten() {
            // SAFETY: the filter that was there before ours, with the
            // pointers Windows gave this one.
            Some(previous) => unsafe { previous(pointers) },
            // EXCEPTION_CONTINUE_SEARCH: Windows ends the program.
            None => 0,
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod handlers {
    pub(super) fn install() {}
}

/// Notes the loaded segments of the library at `path` (Linux).
#[cfg(target_os = "linux")]
fn note_ranges(slot: u32, path: &Path) {
    struct Wanted<'a> {
        path: &'a [u8],
        slot: u32,
    }
    unsafe extern "C" fn each(
        info: *mut libc::dl_phdr_info,
        _size: libc::size_t,
        data: *mut libc::c_void,
    ) -> libc::c_int {
        // SAFETY: `dl_iterate_phdr` passes valid info, and `data` is the
        // `Wanted` given below, valid for the iteration.
        let (info, wanted) = unsafe { (&*info, &*data.cast::<Wanted<'_>>()) };
        if info.dlpi_name.is_null() {
            return 0;
        }
        // SAFETY: a loaded object's name is a NUL-terminated string.
        let name = unsafe { std::ffi::CStr::from_ptr(info.dlpi_name) }.to_bytes();
        if name != wanted.path {
            return 0;
        }
        for i in 0..usize::from(info.dlpi_phnum) {
            // SAFETY: the object has `dlpi_phnum` program headers.
            let ph = unsafe { &*info.dlpi_phdr.add(i) };
            if ph.p_type != libc::PT_LOAD {
                continue;
            }
            let start = (info.dlpi_addr as usize).wrapping_add(ph.p_vaddr as usize);
            let end = start.wrapping_add(ph.p_memsz as usize);
            let n = RANGE_COUNT.load(Ordering::Relaxed);
            if n >= RANGES {
                break;
            }
            RANGE_START[n].store(start, Ordering::Relaxed);
            RANGE_END[n].store(end, Ordering::Relaxed);
            RANGE_SLOT[n].store(wanted.slot, Ordering::Relaxed);
            RANGE_BASE[n].store(info.dlpi_addr as usize, Ordering::Relaxed);
            RANGE_COUNT.store(n + 1, Ordering::Release);
        }
        1
    }
    // Loading happens one plugin at a time (the host's `OnceLock` per
    // plugin and its starting list), but two plugins may load on two
    // threads: one lock keeps the table's count right.
    static NOTING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = NOTING.lock();
    let wanted = Wanted {
        path: path.as_os_str().as_encoded_bytes(),
        slot,
    };
    // SAFETY: `each` matches the callback's type and only reads what it is
    // given; `wanted` lives until the iteration ends.
    unsafe {
        libc::dl_iterate_phdr(Some(each), (&raw const wanted).cast_mut().cast());
    }
}

#[cfg(not(target_os = "linux"))]
fn note_ranges(_slot: u32, _path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_names_the_plugin_running_and_the_one_whose_code_it_was() {
        let a = register("me.a 1.0 (a.oxp; https://example.org/a)".into());
        let b = register("me.b 2.0 (b.oxp)".into());
        // Code of `b` at 0x1000..0x2000.
        let n = RANGE_COUNT.load(Ordering::Relaxed);
        RANGE_START[n].store(0x1000, Ordering::Relaxed);
        RANGE_END[n].store(0x2000, Ordering::Relaxed);
        RANGE_SLOT[n].store(b.0, Ordering::Relaxed);
        RANGE_BASE[n].store(0x800, Ordering::Relaxed);
        RANGE_COUNT.store(n + 1, Ordering::Release);

        let text = |pc, running| {
            let mut space = vec![0; REPORT_MAX];
            let len = compose(&mut space, b"a test", pc, running, 42);
            String::from_utf8(space[..len].to_vec()).unwrap()
        };
        let both = text(0x1800, a.0);
        assert!(both.contains("crashed: a test.\n"), "{both}");
        assert!(
            both.contains(&format!("{IN_CODE}me.b 2.0 (b.oxp)\n")),
            "{both}"
        );
        assert!(
            both.contains(&format!(
                "{ON_THREAD}me.a 1.0 (a.oxp; https://example.org/a)\n"
            )),
            "{both}"
        );
        assert!(
            both.contains(
                "Where: 0x0000000000001800, at 0x0000000000001000 in the plugin's file\n"
            ),
            "{both}"
        );
        assert!(both.contains("Time: 42 (seconds"), "{both}");
        // The same plugin both ways is named once.
        let one = text(0x1800, b.0);
        assert_eq!(one.matches("me.b").count(), 1, "{one}");
        // Neither.
        let none = text(0, 0);
        assert!(none.contains("No plugin was running"), "{none}");
        assert!(none.contains("Where: unknown"), "{none}");

        // The guard marks the thread and puts back what was there.
        assert_eq!(RUNNING.with(Cell::get), 0);
        {
            let _outer = a.running();
            {
                let _inner = b.running();
                assert_eq!(RUNNING.with(Cell::get), b.0);
            }
            assert_eq!(RUNNING.with(Cell::get), a.0);
        }
        assert_eq!(RUNNING.with(Cell::get), 0);
    }

    #[test]
    fn the_last_report_is_read_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("oxiguitar-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(last(&dir), None);
        std::fs::write(
            dir.join(REPORT),
            format!("OxiGuitar crashed: x.\n{ON_THREAD}me.a 1.0 (a.oxp)\n"),
        )
        .unwrap();
        let report = last(&dir).unwrap();
        assert_eq!(report.plugin.as_deref(), Some("me.a 1.0 (a.oxp)"));
        assert_eq!(report.plugin_id(), Some("me.a"));
        assert_eq!(report.homepage(), None);
        let advice = report.advice();
        assert!(
            advice
                .text
                .starts_with("It happened in the plugin me.a 1.0"),
            "{}",
            advice.text
        );
        assert!(advice.links.is_empty());
        let with_home = Report {
            plugin: Some("me.a 1.0 (/p/a.oxp; https://example.org/a)".into()),
            ..report.clone()
        };
        assert_eq!(with_home.homepage(), Some("https://example.org/a"));
        assert_eq!(with_home.advice().links[0].url, "https://example.org/a");
        let core = Report {
            plugin: None,
            ..report.clone()
        };
        assert_eq!(core.advice().links[0].url, ISSUES);
        assert!(
            report
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("crash-")
        );
        assert!(report.path.exists() && !dir.join(REPORT).exists());
        assert_eq!(last(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
