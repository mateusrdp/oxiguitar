# Testing file drops on Wayland

File drops don't work on Wayland yet: winit 0.30 and Slint 1.18 report none there ([ADR 0006](../../docs/adr/0006-wayland-file-drops.md)). These tools show whether a drop reaches the app on Wayland, without a Wayland desktop: a compositor nested in Xvfb, a drag source that behaves like a file manager, and XTEST for the mouse. Use them when a winit or Slint upgrade might add drops. A stopgap built in session 6 was tested with them on 17 Oct 2026 and then rejected (it needed `unsafe` code and polling), so the steps are known to work.

## What you need

- Xvfb, Mesa (llvmpipe), python-xlib (`pip install python-xlib`).
- Smithay's example compositor, smallvil: `git clone --depth 1 https://github.com/Smithay/smithay`, then `cargo build --release -p smallvil` in it. It links `libxkbcommon.so`; if only `libxkbcommon.so.0` is installed, link a copy as `libxkbcommon.so` into a folder and build with `RUSTFLAGS="-L that-folder"`.
- This drag source: `cargo build --release` here.
- A release build of `oxiguitar`.

## Steps

```sh
Xvfb :99 -screen 0 1400x900x24 &
export XDG_RUNTIME_DIR=$(mktemp -d)
DISPLAY=:99 path/to/smallvil &              # creates $XDG_RUNTIME_DIR/wayland-1
# OxiGuitar as a Wayland client: no DISPLAY, so winit must use Wayland.
env -u DISPLAY WAYLAND_DISPLAY=wayland-1 target/release/oxiguitar &
# smallvil puts every window at (0, 0), the newest on top: the source's
# 200x150 window sits over OxiGuitar's top-left corner.
env -u DISPLAY WAYLAND_DISPLAY=wayland-1 scripts/wayland-dnd-test/target/release/wayland-dnd-source SONG.gp &
python3 scripts/wayland-dnd-test/xdrag.py 100,100 640,450
```

The source prints what the target answered. A working drop prints:

```
source: drag started
source: action Value(DndAction(Copy))
source: drop performed
source: send text/uri-list
source: finished
```

and OxiGuitar shows the song (`DISPLAY=:99 import -window root shot.png`). Today nothing in the app accepts a drop on Wayland: the source prints `action Value(DndAction(0x0))` and `drop performed`, but no `send text/uri-list` (smallvil sends no `cancelled` either), and the window stays as it was (checked 17 Oct 2026).

Check the X11 path as well: run `oxiguitar` with `DISPLAY=:99` and no `WAYLAND_DISPLAY`, and drop with any XDND source.
