# 0013 — Plugins' UI in interpreted Slint

- Status: accepted (Mateus, 24 Nov 2026)
- Date: 2026-11-24
- Milestone: v1.0 (#346, from #344)
- Goes on from: ADR 0012 (the core owns nothing of a plugin)

## Context

Plugins showed their UI as rows of widgets the core drew (interface 1.1), with canvases and, from 1.14, surfaces they drew themselves, and dialogs on a card with the core's OK and Cancel (1.8). The Plugin Manager (#344) needed what that widget list lacks (a scrolling table, small red buttons with tooltips, a confirmation, its own Close), and each gap meant adding to the core's widget list. Mateus, 24 Nov 2026: the core should own no plugin GUI elements, and plugins should carry no widget code of their own but use what OxiGuitar uses, with as small a footprint as possible.

Plugins can't call OxiGuitar's compiled Slint: Rust has no stable binary interface, so that would need every plugin built with the exact compiler, Slint build and dependency graph of the core, outside Cargo, and rebuilt whenever the core's build changes. A plugin's own copy of Slint would have to run its own event loop on its own thread, with its own window (which macOS forbids off the main thread), and would put a widget set in every plugin.

## Decision

A plugin's UI is a `.slint` file the plugin hands over, which the core compiles with Slint's interpreter (`slint-interpreter`, at the app's Slint version and style) and runs in its own window and event loop, on its UI thread (interface 1.15, `views`):

- Views are exported components of the plugin's one file, placed by the manifest: a window of its own, a panel in one of the window's areas, a section of the Track tab, an export's options.
- The plugin sets properties and hears callbacks, as data across the interface (`view::Value`). Callbacks reach it off the UI thread.
- Panels, track sections and export options are embedded with Slint's `ComponentContainer` and `create_embedded`, experimental in Slint 1.19 (Mateus: use it, and follow Slint on an upgrade); the interpreter keeps every exported component through its `internal` compiler configuration (`ComponentSelection::AllExported`).
- The widget rows, the dialog card with OK and Cancel, and the settings sections drawn by the core stay for plugins built before 1.15 and are retired before v1.0, once every plugin with UI has moved (#346).

## Consequences

- The core holds no plugin UI, and a plugin no widget set: its file is a few kilobytes of text. Plugins don't need OxiGuitar's compiler or Slint build, and keep working across Slint 1.x.
- Each GUI program grows by about 6.7 MB (measured with Slint 1.19 and the release profile); a plugin's file compiles in about 40 ms the first time one of its views is shown.
- A plugin's `.slint` runs on the window's thread: a badly written file can slow the window. Its mistakes show when it is compiled, which the contract suite does, so they fail the plugin's own tests.
- The experimental and internal parts of Slint are checked on every Slint upgrade.
