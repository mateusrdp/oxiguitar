# The plugin file and its manifest

## The file

A plugin file is the plugin crate built as a C dynamic library, renamed to `NAME.oxp`. Inside it are the plugin's code and its manifest, in a section of its own (`oxg_manifest`; `oxgmanif` on Windows, whose section names hold eight characters). OxiGuitar reads that section with a parser of library files, so it learns what the plugin adds without running any of its code. Plugin files carry no signature: OxiGuitar checks that the file is whole (a truncated download is refused before it is loaded), that it is a library for this system and processor, and that it asks for an interface version OxiGuitar has.

You never write the section yourself: you write `manifest.json` beside `Cargo.toml`, and `export_plugin!` puts its bytes in the section when the file is built.

## manifest.json

The template's:

```json
{{#include ../template/manifest.json}}
```

| Key | Required | What it is |
| --- | --- | --- |
| `interface` | yes | The interface version the plugin was built for, `"MAJOR.MINOR"`: `"1.0"`, or `"1.1"` for tabs and events ([Interface versions](versions.md)) |
| `id` | yes | Unique among all plugins: 1 to 64 lowercase letters and digits, in parts joined by single `.` or `-`. Start it with your own name (`jane.chord-finder`); ids starting `oxiguitar.` are ours. Never change it once released: settings and the disabled list refer to it |
| `name` | yes | What users see, in the plugin manager and in footer messages |
| `version` | yes | `MAJOR.MINOR.PATCH`, optionally `-PRERELEASE` (`1.2.0-beta.1`), equal to the crate's version. Of two files with the same `id`, the higher version is used |
| `description` | no | One or two sentences for the plugin manager |
| `authors` | no | A list of names |
| `license` | no | An SPDX expression, such as `"MIT OR Apache-2.0"` |
| `homepage` | no | Where the plugin comes from |
| `conditions` | no | Where it can run (below); it is never loaded elsewhere |
| `requires` | no | The service interfaces it calls through the host ([Services](services.md)) |
| `formats` | no | The file formats it reads or writes ([Formats](formats.md)) |
| `services` | no | The services it implements ([Services](services.md)) |
| `instruments` | no | The instruments it offers the player ([Instruments](instruments.md)) |
| `tabs` | no | Its tabs; interface 1.1 ([Tabs](tabs.md)) |
| `events` | no | The kinds of event it wants: `"score"`, `"selection"`, `"playback"`; interface 1.1 ([Events](events.md)) |

**Unknown keys are refused when you build and ignored when OxiGuitar reads.** The test `export_plugin!` adds fails on a key the SDK doesn't know, so a typing mistake (`"servces"`) shows up in `cargo test`. An older OxiGuitar reading a newer plugin's manifest skips keys it doesn't know, so new keys never stop a plugin from loading.

A manifest is at most 1 MiB. Real ones are a few hundred bytes.

## Conditions

```json
"conditions": { "os": ["linux"], "arch": ["x86_64", "aarch64"], "display": ["wayland"] }
```

Every list given must hold the running system's value; an empty or missing list allows any.

| Key | Values |
| --- | --- |
| `os` | `linux`, `macos`, `windows` (later `android`, `ios`) |
| `arch` | `x86_64`, `aarch64` |
| `display` | `wayland`, `x11`, `windows`, `macos`: the display system OxiGuitar was built for. On Linux there are two builds of OxiGuitar, Wayland (the default) and X11 |

A plugin whose conditions don't hold is listed in Tools → Plugins with the reason, and never loaded. A plugin file is already built for one system and processor, so you need `os` and `arch` only for a file that would load but can't work (a universal macOS file that works on Apple Silicon only, say). `display` matters for plugins that talk to the windowing system. Services have conditions of their own, beside the plugin's.

## Rules the checks enforce

OxiGuitar refuses a manifest that breaks these, and Tools → Plugins names the first problem; `cargo test` (through `export_plugin!`) names them all:

- the ids are valid and unique: the plugin's; each format's; each instrument's; each tab's; each service's within its interface;
- names and tab titles aren't empty;
- a format reads or writes, and one that reads has at least one signature;
- extensions are lowercase and without the dot (`"gp5"`, not `".GP5"`);
- tabs and events need interface 1.1.

`cargo test` also checks what only the code can tell:

- the manifest asks for an interface this SDK has (1.0 or 1.1), and its version is the crate's;
- it has no key the SDK doesn't know;
- every format, service and tab declared is implemented, and every tab's contents can be shown.
