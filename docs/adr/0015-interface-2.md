# 0015 — Interface 2.0: the core draws nothing of a plugin

- Status: accepted (Mateus, 25 Nov 2026)
- Date: 2026-11-25
- Milestone: v1.0 (#346)
- Goes on from: ADR 0013 (plugins' UI in interpreted Slint), ADR 0012 (the core names no plugin)

## Context

Interface 1.15 let plugins bring their own UI as a `.slint` file (ADR 0013), and by 25 Nov 2026 every plugin with UI used it: MIDI, wah-wah, NAM, equalizer, temperament, export, audio clips and AI. What the core still drew of plugins (tabs and dialogs made of widget rows, the canvas, surfaces, links among widgets, track sections' widgets) was there only for plugins built before 1.15, about 3,000 lines across the interface, the SDK, the host and the app. #346 said it would go before v1.0.

Interface 1 had retired things before (settings windows in 1.9, named shared folders in 1.10) by keeping their messages as placeholders that are never sent. Asked whether to do the same as 1.17, to make interface 2.0, or to wait, Mateus chose "Interface 2.0 now".

## Decision

- **Interface 2.0** is 1.16 without the plugin UI the core drew: the messages, the module `tab`, the manifest's `tabs` and the placeholders 1.9 and 1.10 left are removed from the wire, and variant numbers are pinned again. The entry symbol is `oxiguitar_plugin_v2`; the manifest asks for `"2.0"`; the SDK crates are at 2.0.0.
- **A host for interface 2 loads no plugin built for interface 1**, and says it needs updating; a plugin built for a later major version is told to wait for a newer OxiGuitar.
- **Everything else of interface 1 is in 2.0.** Checks of a plugin's minor version up to 1.16 go: every 2.0 plugin knows all of it. A later 2.x minor version only adds, as interface 1's did.
- **A plugin's UI is its views**: windows, panels, track sections (a view of the section's id) and an export's options (a view whose place is `export`).
- The score version 1, interface 1.1's selection event, `Request::Tool` and `Notify` stay: superseded, but still used by format plugins, and removing them is a change to those plugins of its own.

## Consequences

- Every plugin is rebuilt with the 2.0 SDK and released again, its manifest asking for `"2.0"`. Until a plugin is, it doesn't load in OxiGuitar 0.24 and later, and Tools → Plugins says so.
- The core's tests that load released plugins (`scripts/released-plugins.sh`) take the latest release built for the core's interface major version, so releases (or pre-releases) of the Guitar Pro, MIDI and export plugins built for 2.0 come before the core's.
- The app has no tab thread, no plugin dialog card and no canvas or surface drawing; the template's tab is a panel in Slint, and the fuzz target that read tab contents reads a plugin's drawings instead.
