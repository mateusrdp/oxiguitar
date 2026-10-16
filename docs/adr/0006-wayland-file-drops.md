# 0006 — File drops on Wayland through our own data device (stopgap)

- Status: accepted (a stopgap: replace it as soon as the conditions below hold)
- Date: 2026-10-17
- Milestone: M1

## Context

Mateus asked for files dropped on the window to open (session 6). The window comes from Slint 1.18 on winit 0.30 (ADR 0004):

- winit 0.30 reports a dropped file as `WindowEvent::DroppedFile` on Windows, macOS and X11. Its Wayland backend has no drag and drop at all: `DroppedFile` appears only in winit's `x11`, `windows` and `macos` platform code.
- Slint 1.18 has drag and drop inside a window (`DragArea`, `DropArea`, `DataTransfer` with file paths), but its winit backend passes no drops from the operating system on.

So on Wayland, the desktop Mateus and most current Linux users run (GNOME and KDE default to it), a drop did nothing. On Wayland, drops arrive as `wl_data_device` events on the client's connection, and only on the connection that created the window's surface. winit owns that connection and lends it out only as a raw `wl_display` pointer (`raw-window-handle`).

## Options

1. **Wait for winit or Slint.** No code of ours, but no drops on Wayland until a winit release with Wayland drag and drop reaches a Slint release we use. That date is unknown.
2. **Run on XWayland** (force winit's X11 backend). X11 drops work, but the window loses Wayland scaling (blurry on HiDPI) and the desktop entry's icon and app id. Every user pays for one feature.
3. **Our own `wl_data_device` on winit's connection.** Wrap winit's `wl_display` with `wayland_backend::client::Backend::from_foreign_display`, bind `wl_data_device_manager` and `wl_seat`, and handle `enter`, `leave`, `drop` and the offer's `text/uri-list`. smithay-clipboard reaches winit's display the same way. Costs one `unsafe` call (the pointer must stay valid while we use it) and a 100 ms poll of our event queue.
4. **A second Wayland connection of our own.** No unsafe, but it doesn't work: the compositor sends drop events only to the connection that owns the surface.

## Decision

Option 3, kept in one file, `crates/oxiguitar-app/src/wayland_drop.rs`, built only on Unix systems other than macOS. `main.rs` starts it once the winit window exists, polls it every 100 ms, and drops it right after the event loop returns.

## How it works

- **One `unsafe` call:** `Backend::from_foreign_display(winit's wl_display)`. Its rule is that the display outlive our backend. Slint's winit backend keeps winit's event loop, which owns the connection, for the life of the process (it stores the loop again after `run` returns, so it can run again). `main.rs` drops our listener as soon as `run` returns, while the display is still open. No other thread holds a clone: the threads that read dropped lists get only a pipe and a channel.
- **Reading events:** winit's event loop reads the socket whenever it has data, and libwayland files our events in our own queue. A Slint timer calls `dispatch_pending` on that queue every 100 ms. Before the winit window exists, a one-shot timer looks for it every 100 ms for two seconds. On X11, or with no data device manager, nothing keeps running.
- **Protocol:** on `enter`, accept only offers listing `text/uri-list`, with the `copy` action (data device version 3). On `drop`, `receive` the list through a pipe read on its own thread (the source may be slow), then `finish` and `destroy` the offer on the UI thread. Open the first `file://` path, like a drop on X11. Clipboard offers (`selection`) are destroyed at once, because Slint copies and pastes through its own data device.
- **Dependencies:** `wayland-client` and `wayland-backend` (with `client_system`), the versions winit already uses. Nothing new is compiled or linked.

## Consequences

- Drops work on Wayland, X11, Windows and macOS alike, and the manual no longer needs an exception.
- The app crate has `unsafe` in one place, allowed by `#![allow(unsafe_code)]` in that file only. The crate still denies it everywhere else. CONTRIBUTING.md lists it under "Unsafe code and stopgaps".
- When the app runs on Wayland, the 100 ms timer wakes the event loop 10 times a second. The tick only looks at an empty queue, but it is a cost the window didn't have before.
- Drags into the window show no drop position or highlight. None was asked for.

### When to replace it, and how

Replace it when **either** holds for the versions in our Cargo.lock:

- winit's Wayland backend emits `WindowEvent::DroppedFile`. Check with `grep -rn DroppedFile ~/.cargo/registry/src/*/winit-<version>/src/platform_impl/linux/wayland/`; today nothing matches.
- Slint passes operating-system file drops to the window, for example as a `DropArea` event carrying `DataTransfer::file_paths`.

Then delete `src/wayland_drop.rs`, its `mod` line, `install_wayland_drops`, `try_wayland_drops`, the `WaylandListener` type and the drop after `run` in `main.rs`, and the `wayland-*` target dependencies in the app's Cargo.toml. Remove the CONTRIBUTING.md entry and mark this ADR "superseded". The X11, Windows and macOS path (`DroppedFile` in `install_window_events`) stays as it is.

Also revisit it if a winit or Slint upgrade changes how the event loop owns the Wayland connection, because the safety argument above depends on it.
