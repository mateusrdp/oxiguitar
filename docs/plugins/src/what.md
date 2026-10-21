# What a plugin is

A plugin is **one file**, `NAME.oxp`: a native library for one system (a `.so` on Linux, a `.dylib` on macOS, a `.dll` on Windows, renamed), with a manifest inside it that says what the plugin adds. The user drops the file in a plugin folder; OxiGuitar finds it at the next start.

## What a plugin can add

| Contribution | What it is | Since |
| --- | --- | --- |
| [A format](formats.md) | Reads one file format into a score, writes a score into it, or both | 1.0 |
| [A tab](tabs.md) | A tab in the window's top, bottom, left or right area, made of standard widgets and, if it needs one, a canvas it draws | 1.1 |
| [Events](events.md) | Being told when a score is opened, changed or closed, when the selection changes and when playback moves | 1.1 |
| [A service](services.md) | An implementation of an interface, chosen per system: an audio device, a MIDI device, anything another plugin needs | 1.0 |
| [An instrument](instruments.md) | Sound for the player's tracks. Declared in 1.0; the interface itself comes with the player | (later) |

One plugin can add any mix of these. Tools for the Tools menu, other exports (PNG, SVG, WAV) and toolbar sections join the interface when the first plugin needs them; if you want to write one, [open an issue](https://github.com/mateusrdp/oxiguitar/issues/new) and say what it needs.

## What a plugin is not

- **Not sandboxed.** A plugin is native code running inside OxiGuitar, with the user's rights: it can read files, open network connections, start threads. OxiGuitar's privacy promises (no telemetry, never upload, never download) cover the app and the plugins we publish, not yours. Make your own promises, keep them, and say them plainly where people download your plugin.
- **Not a way to change the score behind the core's back.** A plugin reads the score the core gives it. Edits will go through the core as undoable commands, when the editor comes (OxiGuitar 0.5).
- **Not a script.** It is compiled Rust, built for each system you want to support ([Building for every system](building.md)).
- **Not unloaded.** Once loaded, it stays in memory until OxiGuitar quits. Disabling it in Tools → Plugins takes effect at the next start.

## When it runs

1. **At start**, OxiGuitar reads the manifest of every `.oxp` file in the plugin folders, without loading any code. Your plugin's tabs, formats and services are known from that alone, and reading 30 manifests takes about a millisecond.
2. **On first use**, it loads the plugin and calls its `start`: when a file of its format is opened, its tab is first shown, one of its services is first needed, or an event it subscribes to first fires.
3. **Then** it calls the plugin whenever one of its contributions is used, from any thread, possibly several at once.
4. **At the end of the program**, the plugin goes with it.

An error a plugin answers with (a damaged file, say) is shown to the user, and the plugin goes on. A plugin that **fails** (it can't start, it panics, or it breaks the interface's contract) is withdrawn for the rest of the session: OxiGuitar carries on as if it weren't installed, so a file it would have read goes to the next plugin that recognises it and its tabs leave the window. The footer says so once, and Tools → Plugins shows why.

## In Rust

A plugin is a library crate that depends on the SDK, `oxiguitar-plugin`. It implements the trait `Plugin` and one trait per kind of contribution, writes its manifest as `manifest.json` beside `Cargo.toml`, and calls `export_plugin!` once. The SDK turns that into the C interface OxiGuitar loads, with the only `unsafe` code inside the SDK.

The SDK and the interface crate are MIT OR Apache-2.0, so your plugin may use any licence compatible with the GPL (OxiGuitar itself is GPL-3.0-or-later, and a plugin runs inside it).
