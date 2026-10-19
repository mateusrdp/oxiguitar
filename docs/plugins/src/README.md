# Writing OxiGuitar plugins

OxiGuitar is a small core with plugins. The core holds the score, shows it, exports it as a PDF and will edit and play it; everything else comes from plugins: every file format, every sound, other exports, and tabs in the window's panels. Our own Guitar Pro readers are plugins like any you write, and the app makes no distinction between ours and yours.

This guide shows how to write one, in Rust, with the SDK (`oxiguitar-plugin`). The fastest way in is to copy the [template plugin](template.md): it builds, passes the contract suite and opens in the app as it is, and each part of it is explained in the pages that follow.

## What you need

- Rust 1.97 or later ([rustup.rs](https://rustup.rs)).
- OxiGuitar 0.2 or later, to try your plugin in the app ([the user manual](https://mateusrdp.github.io/oxiguitar/installing.html)).
- Nothing else: the SDK and the contract suite come through Cargo, and you write no `unsafe` code.

## The pages

| Page | What it covers |
| --- | --- |
| [What a plugin is](what.md) | What a plugin can and can't do, and when it runs |
| [Start from the template](template.md) | Copy, rename, build, test, try it in the app |
| [The plugin file and its manifest](manifest.md) | The `.oxp` file and `manifest.json`, key by key |
| [The SDK](sdk.md) | The `Plugin` trait, `export_plugin!`, the host, errors and threads |
| [Formats](formats.md) | Reading a file into a score, writing a score into a file |
| [Tabs](tabs.md) | A tab in one of the window's areas: widgets and a canvas |
| [Events and the open score](events.md) | Following the score, the selection and playback |
| [Services](services.md) | Implementations of an interface, chosen per system |
| [Instruments](instruments.md) | What comes with the player |
| [Settings](settings.md) | Keeping a plugin's settings |
| [Testing](testing.md) | `cargo test`, the contract suite, trying it in the app |
| [Building for every system](building.md) | Six targets, one file each, from one workflow |
| [Installing and sharing](installing.md) | Where users put the file, and what to tell them |
| [Interface versions](versions.md) | What 1.0 and 1.1 hold, and what stays compatible |

The design behind all this is in [docs/design/plugins.md](https://github.com/mateusrdp/oxiguitar/blob/develop/docs/design/plugins.md) and [ADR 0008](https://github.com/mateusrdp/oxiguitar/blob/develop/docs/adr/0008-plugin-architecture.md). The SDK's own reference is its rustdoc: `cargo doc --open` in your plugin's folder.
