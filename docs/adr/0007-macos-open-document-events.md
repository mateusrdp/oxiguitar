# 0007 — No macOS "open document" events until winit or Slint deliver them

- Status: accepted (revisit on every winit or Slint upgrade)
- Date: 2026-10-18
- Note (19 Oct 2026): written against Slint 1.18; the app is now pinned to Slint 1.17.1 ([ADR 0009](0009-wayland-first.md)), on the same winit 0.30.13.
- Milestone: M1

## Context

On macOS, double-clicking a Guitar Pro file in Finder, choosing **Open With → OxiGuitar**, or dropping a file on the Dock icon does not pass the file on the command line. macOS starts the app (if it isn't running) and then sends an Apple Event, which AppKit hands to the application delegate as `application:openURLs:` (or the older `application:openFiles:`). The .dmg's `Info.plist` registers the Guitar Pro types (`CFBundleDocumentTypes`, UI-4), so Finder already offers OxiGuitar for them.

- winit 0.30.13 has no event for opened documents. Its own documentation (`src/platform/macos.rs`, "Custom `NSApplicationDelegate`") says to register an application delegate of our own with objc2 and `declare_class!`, which needs `unsafe` blocks (`unsafe impl ClassType`, `msg_send_id!`).
- Slint 1.18 forwards no open-document events either; it creates its event loop through winit and offers no hook for application-delegate methods.

So today a double-click on macOS starts OxiGuitar with an empty window. The manual says so (`docs/manual/src/installing.md`).

## Options

1. **Wait for winit or Slint.** No code of ours. Files open with the first winit release that reports opened documents (for example as an application-handler event), once Slint uses it, or when Slint offers the event itself.
2. **Our own `NSApplicationDelegate`** through objc2, as winit's docs show. It works, but it needs `unsafe` Objective-C class declarations in the app, and it has to cooperate with whatever delegate Slint or winit install in later versions. This is the same kind of workaround as the Wayland drop listener Mateus rejected in ADR 0006: `unsafe` code borrowing another library's internals to fill a gap upstream should fill.
3. **A small helper launched by Finder that re-execs OxiGuitar with the file name.** A second executable inside the bundle, still Objective-C glue, and it breaks "one running app" behaviour. Rejected.

## Decision

Option 1, the same as ADR 0006 (Mateus, 18 Oct 2026): clean code only, no `unsafe`, the gap documented.

## What needs to change to enable it

Any one of these, in the versions in our Cargo.lock:

1. **winit reports opened files or URLs on macOS** (an event or application-handler method), **and the Slint we use exposes it** to the app, for example through `slint::winit_030` or its successor.
2. **Slint offers an "open file" callback** on macOS (it would also serve iOS later, M9).

Then open the file through the same `Request::Open` as File → Open and a drop, remove the macOS note from `docs/manual/src/installing.md`, `docs/design/ui.md` and CONTRIBUTING.md ("Known platform gaps"), and mark this ADR superseded. Check with: `grep -rn -i "openurls\|open_urls\|opened" ~/.cargo/registry/src/*/winit-<version>/src/platform_impl/apple/` (or `macos/`), and the Slint changelog.

## Consequences

- On macOS, users open files with File → Open, by dropping them on the window (winit reports those), or from the command line.
- No `unsafe` code and no Objective-C glue in the app.
- When the plugin system arrives (ADR 0008), this could become a macOS-only plugin. That changes where the code lives, not the decision: it would still need the `unsafe` delegate, so it waits for upstream too unless Mateus decides otherwise.
- Each winit or Slint upgrade gets a check against the list above, next to ADR 0006's.
