# 0006 — No file drops on Wayland

- Status: accepted; the gap is permanent (Mateus, 25 Oct 2026)
- Date: 2026-10-17
- Note (19 Oct 2026): written against Slint 1.18; the app is now pinned to Slint 1.17.1 ([ADR 0009](0009-wayland-first.md)), on the same winit 0.30.13.
- Milestone: M1

## Context

Dropping a file on the window opens it (session 6). The window comes from Slint 1.18 on winit 0.30 (ADR 0004). `main.rs` (`install_window_events`) opens the file from winit's `WindowEvent::DroppedFile`.

- winit 0.30 sends `DroppedFile` on Windows, macOS and X11. Its Wayland backend has no drag and drop: `DroppedFile` appears only in winit's `x11`, `windows` and `macos` platform code.
- Slint 1.18 has drag and drop inside a window (`DragArea`, `DropArea`, `DataTransfer` with file paths), but its winit backend passes no drops from the operating system on.

So on Wayland, which GNOME and KDE use by default, a drop does nothing. On Wayland, drops arrive as `wl_data_device` events, and only on the connection that created the window's surface. winit owns that connection and lends it out only as a raw `wl_display` pointer.

## Options

1. **Wait for winit or Slint.** No code of ours. Drops on Wayland arrive with the first winit release that has them, once Slint uses it, or when Slint forwards OS drops itself.
2. **Run on XWayland** (force winit's X11 backend). X11 drops work, but every user loses Wayland scaling (blurry on HiDPI) and the desktop entry's app id and icon.
3. **Our own `wl_data_device` on winit's connection.** This was built and tested on 17 Oct 2026, then rejected by Mateus. It needed an `unsafe` call to wrap winit's raw `wl_display` (`Backend::from_foreign_display`). Worse, Slint offers no way to wake its event loop on events in our own Wayland queue, so it had to poll that queue on a timer (every 100 ms). In Mateus's words, polling is unacceptable: it adds concurrency risk, an operation that runs all the time, and it is inelegant. A reader thread instead of the timer would still race winit's own reads.
4. **A second Wayland connection of our own.** It doesn't work: the compositor sends drop events only to the connection that owns the surface.

## Decision

Option 1. Drops work on Windows, macOS and X11; on Wayland they don't, and the manual says so. No workaround in the code.

## Permanent (25 Oct 2026)

Mateus's rule for every library (CONTRIBUTING.md, "Libraries exactly as published"): nothing is planned on a change upstream. Option 1 as first written waited for winit or Slint; now the gap is accepted for good, no upgrade is checked for it, and nothing in the roadmap depends on it. Options 3 and 4 stay rejected: our own code on top of the libraries can't close it without polling and borrowing winit's connection.

`install_window_events` keeps handling `WindowEvent::DroppedFile` on every system, so the Wayland build would open dropped files without a change of ours if a winit we use ever delivered them there. That is a fact about the code, not a plan.

## Consequences

- On Wayland, users open files with File → Open, a double-click in the file manager (the packages register the Guitar Pro types), or the command line.
- No `unsafe` code and no polling in the app.
- CONTRIBUTING.md lists this under "Known platform gaps".
