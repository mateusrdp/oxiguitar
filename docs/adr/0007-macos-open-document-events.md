# 0007 — No macOS "open document" events

- Status: accepted; the gap is permanent (Mateus, 25 Oct 2026)
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

## Permanent (25 Oct 2026)

Mateus's rule for every library (CONTRIBUTING.md, "Libraries exactly as published"): nothing is planned on a change upstream. Option 1 as first written waited for winit or Slint; now the gap is accepted for good (Mateus chose this over option 2, our own `unsafe` application delegate), no upgrade is checked for it, and nothing in the roadmap depends on it.

## Consequences

- On macOS, users open files with File → Open, by dropping them on the window (winit reports those), or from the command line.
- No `unsafe` code and no Objective-C glue in the app.
- A macOS-only plugin (ADR 0008) wouldn't change this: it would still need the `unsafe` delegate.
- CONTRIBUTING.md lists this under "Known platform gaps".
