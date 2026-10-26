// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Catching system calls while a plugin renders (Linux): the real-time
//! rule says the audio thread never waits, and waiting (a contended lock,
//! a sleep, reading a file, mapping memory) is a system call.
//!
//! [`watch`] forks the test process; the child, now single-threaded, shuts
//! itself behind a seccomp filter that lets through only the two calls it
//! needs to report and end (`write` to its pipe, `exit_group`) and traps
//! every other, then runs the renders it was given. A trapped call writes
//! its number to the pipe and ends the child at once. The parent reads
//! what the child said, waiting at most a few seconds: a child that never
//! answers was stuck waiting (a lock another thread held when the process
//! forked, such as the system allocator's).
//!
//! What this can't see: a lock that nobody else holds makes no system
//! call (docs/design/player.md).
//!
//! The one module of the contract suite with `unsafe` code (fork, the
//! filter, the signal handler); every block says why it is sound.

#![allow(unsafe_code)]

use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

/// What happened while the renders ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Watched {
    /// No system call.
    Clean,
    /// A system call, by its number on this processor.
    Called(i64),
    /// The renders never ended: they waited for something.
    Stuck,
    /// The watch couldn't be set up; why.
    Unwatched(String),
}

/// How long the child may take.
const LIMIT: Duration = Duration::from_secs(10);

/// The pipe's writing end, for the signal handler.
static REPORT_FD: AtomicI32 = AtomicI32::new(-1);

/// Runs `renders` in a forked child behind a filter that traps every system
/// call, and says what it did. `renders` must use only memory made before
/// the call (it runs in a copy of the process).
pub fn watch(renders: impl FnOnce()) -> Watched {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is two writable ints, as pipe() wants.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Watched::Unwatched("no pipe".into());
    }
    let (read, write) = (fds[0], fds[1]);
    // SAFETY: fork() in a process with other threads: the child runs only
    // the code below and `renders`, which uses memory already made, then
    // ends with _exit (no destructor, no atexit handler runs in it).
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        // SAFETY: closing the pipe we made.
        unsafe {
            libc::close(read);
            libc::close(write);
        }
        return Watched::Unwatched("fork failed".into());
    }
    if pid == 0 {
        // The child.
        // SAFETY: the child's own copy of the pipe; then the handler and
        // the filter, set up with memory on this stack; `renders` runs, and
        // the child ends without unwinding or running destructors.
        unsafe {
            libc::close(read);
            REPORT_FD.store(write, Ordering::Relaxed);
            install_handler();
            if install_filter(write).is_err() {
                let _ = libc::write(write, b"F".as_ptr().cast(), 1);
                libc::_exit(3);
            }
        }
        renders();
        // SAFETY: writing one byte from a static to our pipe (allowed by
        // the filter), then ending at once.
        unsafe {
            let _ = libc::write(write, b"K".as_ptr().cast(), 1);
            libc::_exit(0);
        }
    }
    // The parent.
    // SAFETY: closing our copy of the writing end.
    unsafe { libc::close(write) };
    let started = Instant::now();
    let mut status = 0;
    let ended = loop {
        // SAFETY: waiting for our own child, without blocking.
        let r = unsafe { libc::waitpid(pid, &raw mut status, libc::WNOHANG) };
        if r == pid {
            break true;
        }
        if started.elapsed() > LIMIT {
            // SAFETY: ending our own child, then reaping it.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, &raw mut status, 0);
            }
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut said = [0u8; 16];
    // SAFETY: reading at most 16 bytes into `said` from our pipe, which is
    // closed at the child's end, so this never blocks.
    let n = unsafe { libc::read(read, said.as_mut_ptr().cast(), said.len()) };
    // SAFETY: closing the reading end.
    unsafe { libc::close(read) };
    let said = &said[..usize::try_from(n).unwrap_or(0)];
    if !ended {
        return Watched::Stuck;
    }
    match said.first() {
        Some(b'K') => Watched::Clean,
        Some(b'S') if said.len() >= 9 => {
            let mut nr = [0u8; 8];
            nr.copy_from_slice(&said[1..9]);
            Watched::Called(i64::from_ne_bytes(nr))
        }
        Some(b'F') => Watched::Unwatched("the filter couldn't be installed".into()),
        _ => Watched::Unwatched(format!("the child ended with status {status}")),
    }
}

/// SIGSYS: report the trapped call's number and end the child.
extern "C" fn trapped(_signal: i32, info: *mut libc::siginfo_t, _context: *mut libc::c_void) {
    // On Linux, a SIGSYS's siginfo holds the call's address then its
    // number: after the three ints of every siginfo (and padding to 8
    // bytes on 64-bit systems) come `void *_call_addr` and `int _syscall`.
    let offset = 2 * std::mem::size_of::<usize>() + std::mem::size_of::<usize>();
    // SAFETY: the kernel hands the handler a valid siginfo for this SIGSYS,
    // whose `_syscall` field sits at `offset` (the layout above).
    let nr = unsafe { info.cast::<u8>().add(offset).cast::<i32>().read_unaligned() };
    let mut out = [0u8; 9];
    out[0] = b'S';
    out[1..].copy_from_slice(&i64::from(nr).to_ne_bytes());
    // SAFETY: async-signal-safe calls only: write to our pipe (allowed by
    // the filter) and _exit.
    unsafe {
        let _ = libc::write(
            REPORT_FD.load(Ordering::Relaxed),
            out.as_ptr().cast(),
            out.len(),
        );
        libc::_exit(2);
    }
}

/// # Safety
///
/// Only in the forked child.
unsafe fn install_handler() {
    // SAFETY: a zeroed sigaction is a valid starting point; the handler is
    // `trapped`, with SA_SIGINFO for its three arguments.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = trapped as *const () as usize;
        action.sa_flags = libc::SA_SIGINFO;
        libc::sigaction(libc::SIGSYS, &action, std::ptr::null_mut());
    }
}

/// The processor as the kernel names it in a filter.
#[cfg(target_arch = "x86_64")]
const ARCH: u32 = 0xC000_003E;
#[cfg(target_arch = "aarch64")]
const ARCH: u32 = 0xC000_00B7;

/// # Safety
///
/// Only in the forked child, which is single-threaded.
unsafe fn install_filter(report: i32) -> Result<(), ()> {
    const LD: u16 = (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16;
    const JEQ: u16 = (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16;
    const RET: u16 = (libc::BPF_RET | libc::BPF_K) as u16;
    let stmt = |code: u16, k: u32| libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    };
    let jump = |k: u32, jt: u8, jf: u8| libc::sock_filter {
        code: JEQ,
        jt,
        jf,
        k,
    };
    let allow = libc::SECCOMP_RET_ALLOW;
    let trap = libc::SECCOMP_RET_TRAP;
    // seccomp_data: nr at 0, arch at 4, instruction pointer at 8, the
    // arguments from 16 (the first one's low half first on little-endian
    // processors, which both of ours are).
    let program = [
        stmt(LD, 4),
        jump(ARCH, 1, 0),
        stmt(RET, libc::SECCOMP_RET_KILL_PROCESS),
        stmt(LD, 0),
        jump(libc::SYS_exit_group as u32, 4, 0),
        jump(libc::SYS_exit as u32, 3, 0),
        jump(libc::SYS_write as u32, 0, 3),
        stmt(LD, 16),
        jump(report as u32, 0, 1),
        stmt(RET, allow),
        stmt(RET, trap),
    ];
    // Jumps count from the next instruction: exit_group and exit go to the
    // first `allow` (9), a write to anything but the pipe to `trap` (10).
    let fprog = libc::sock_fprog {
        len: program.len() as u16,
        filter: program.as_ptr().cast_mut(),
    };
    // SAFETY: plain prctl calls with a filter program that lives on this
    // stack for the call (the kernel copies it).
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err(());
        }
        if libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &raw const fprog,
        ) != 0
        {
            return Err(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_call_is_caught_and_plain_work_is_not() {
        assert_eq!(
            watch(|| {
                let mut x = 0u64;
                for i in 0..1000 {
                    x = x.wrapping_mul(31).wrapping_add(i);
                }
                std::hint::black_box(x);
            }),
            Watched::Clean
        );
        let got = watch(|| {
            // SAFETY: getpid takes nothing.
            let _ = unsafe { libc::getpid() };
        });
        assert_eq!(got, Watched::Called(libc::SYS_getpid));
    }
}
