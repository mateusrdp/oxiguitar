# 0006 — No file drops on Wayland until winit or Slint report them

- Status: accepted (revisit on every winit or Slint upgrade)
- Date: 2026-10-17
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

## What needs to change to enable it

Any one of these, in the versions in our Cargo.lock:

1. **winit's Wayland backend emits `WindowEvent::DroppedFile`, and the Slint we use runs on that winit.** Then drops work with no change to our code: `install_window_events` already handles the event. Check with `grep -rn DroppedFile ~/.cargo/registry/src/*/winit-<version>/src/platform_impl/linux/wayland/`; today nothing matches.
2. **Slint passes operating-system drops to the window**, for example as a `DropArea` event whose `DataTransfer` has file paths. Then open the file from that event, and drop the winit `DroppedFile` arm if Slint covers every system.
3. **Slint (or winit through Slint) lets an app add its own event source to the event loop**, for example a file descriptor that wakes the loop when it is readable. Then option 3 could be reconsidered without polling. It would still need the `unsafe` display wrap, so it needs Mateus's agreement first.

Whichever lands, test with [`scripts/wayland-dnd-test/`](../../scripts/wayland-dnd-test/README.md). It runs a Wayland compositor nested in Xvfb and a drag source that behaves like a file manager, so it shows whether a drop reaches the app. Then remove the Wayland exception from the manual (`docs/manual/src/opening.md` and `keys.md`), `docs/design/ui.md` and CONTRIBUTING.md, and mark this ADR superseded.

## Consequences

- On Wayland, users open files with File → Open, a double-click in the file manager (the packages register the Guitar Pro types), or the command line.
- No `unsafe` code and no polling in the app.
- Each winit or Slint upgrade gets a check against the list above. CONTRIBUTING.md lists this under "Known platform gaps" for reviewers.
