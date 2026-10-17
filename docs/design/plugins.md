# Plugins

OxiGuitar is a small core and a set of plugins. The core opens nothing, plays nothing and exports nothing by itself: it holds the score, shows it, edits it, and drives the player's transport. Every file format, every sound, every export and every feature that depends on a platform or environment is a plugin (Mateus, 18 Oct 2026).

This document is the design. Parts marked **Proposed** wait for Mateus's answers to the questions in ADR 0008 ([docs/adr/0008-plugin-architecture.md](../adr/0008-plugin-architecture.md)); the rest is decided. The plugin developer guide (how to write one, step by step) is written with the SDK in M2 and lives in the user manual's sister book, `docs/plugins/`.

## Why

- **Small, simple core.** Read, edit, write and the player's controls. Everything else can be added, replaced or dropped without touching the core, and the mobile apps (M10) start from the same small base.
- **No dead code.** The app never carries or loads code it cannot use on the machine it runs on: no Wayland code under X11, no CoreAudio code on Linux, no reader for a format nobody opens. Less code loaded means a smaller attack surface and a faster start.
- **One format per plugin.** Each file format is its own plugin (one for .gp3, one for .gp4, one for .gp5, one for .gpx, one for .gp, and so on), so a format can be added, improved or retired on its own.
- **Partitioned testing and work.** Each plugin is its own crate with its own tests, fuzz targets and CI path filter. Contributors can own one plugin without learning the whole codebase, and several plugins can be built in parallel.

## What the core holds

| Core | Plugins |
| --- | --- |
| The score model, edit commands, undo and redo, selection | Every file format, reading and writing: Guitar Pro 3, 4, 5, 6 (.gpx), 7–8 (.gp); later MusicXML, ABC, MuseScore, MIDI files |
| Engraving and the four layouts, drawn on screen **(Proposed: core, because nobody can edit what they cannot see)** | Exports: PDF, PNG, SVG, WAV |
| The window shell: menus, toolbar, the four tabbed areas, the page, the footer | The Tools menu's tools (Infer Key Signatures moves out of the core) |
| The player's transport: play, play selection, stop, pause, rewind, count-in, loop, metronome; the cursor; turning the score into timed events (repeats, jumps, tempo, effects) | Sound: the MIDI synthesiser, NAM guitar, RSE banks, realistic drums |
| **(Proposed)** The mixer: per-track volume, pan, mute and solo, and playback speed | Audio devices (PipeWire, ALSA, CoreAudio, WASAPI) and MIDI devices |
| settings.json, the plugin folders, the plugin host | The plugin manager |
| A built-in catalogue of our own plugins (names, file signatures, where to get them), so the core can say *which* plugin a file needs | Platform gaps we may fill one day: Wayland drops (ADR 0006), macOS open-document events (ADR 0007) |

The core still compiles per target (a Windows build holds no Linux code). The one exception to "no code that can't be used" is the windowing toolkit: Slint and winit carry both X11 and Wayland backends in the Linux binary and choose one at start. Splitting them would mean shipping two Linux builds of the core. **(Proposed: accept this exception.)**

## The window

```text
┌──────────────────────────────────────────────────────────────────────┐
│ File  View  Tools  Help                                   (menu bar) │
├──────────────────────────────────────────────────────────────────────┤
│ [File ▢▢▢] [Edit ▢▢] [View ▢] [plugin section ▢▢▢]          (toolbar) │
├──────────────────────────────────────────────────────────────────────┤
│ Player | tab | tab                                        top area ▾ │
│  ⏮ ▶ ▶[] ❚❚ ■  count-in  loop  metronome     Output: (none)          │
├────────────┬────────────────────────────────────────────┬────────────┤
│ Palette|…  │                                            │ Inspector… │
│            │                 the page                   │            │
│ left area  │        (always on the dark canvas)         │ right area │
│            │                                     [zoom] │            │
├────────────┴────────────────────────────────────────────┴────────────┤
│ Tracks | Mixer | tab                                   bottom area ▾ │
│  1 Guitar        6 strings  [𝄞][TAB]                                 │
├──────────────────────────────────────────────────────────────────────┤
│ ● No output plugin found: get one from github.com/mateusrdp/… (footer)│
└──────────────────────────────────────────────────────────────────────┘
```

- **Toolbar**: sections of icon buttons, side by side. The core has File, Edit and View sections; a plugin can add sections with its own icons. It can be hidden (View → Toolbar).
- **Four areas**: top and bottom span the window's width; left and right sit beside the page. Each holds tabs. Clicking the active tab's name, or the chevron, collapses the area to its tab strip; View → Panels hides it completely. Plugins add tabs to any area.
- **The page** sits in the middle on a dark canvas (the same colour as Dark Page's surround, whichever page colour is shown). It has no tabs and cannot be hidden.
- **Footer**: one line at the bottom with the latest message. Clicking it expands a scrollable list of every message this session (errors, warnings, tool reports, plugin notes); clicking again shrinks it.
- **Player controls** live in the top area's Player tab, with the choice of output plugin. With no output plugin installed the controls are greyed out and the footer says where to get one.

Before M2 the containers exist with placeholders (Palette and Inspector say what will go there; the toolbar's Edit section is greyed). `docs/design/ui.md` describes the window as built.

## Plugin files

**Proposed** (ADR 0008, question 1): a plugin is **one native library file** with a small, versioned C interface, built per target, with its manifest in a named data section.

- **One file**, named `NAME.oxp` ("OxiGuitar plugin"), for example `gp5.oxp`. Inside it is an ordinary shared library for one target: ELF on Linux, PE on Windows, Mach-O on macOS (one universal file for Intel and Apple Silicon). Downloads carry the target in their name (`oxiguitar-gp5-0.2.0-linux-x86_64.oxp`); the user can drop it in a plugin folder as it is. Icons, strings and small data are compiled into the file. Large data a plugin downloads at the user's request (SoundFonts, NAM models) goes in that plugin's data folder, not in the plugin file.
- **No unpacking.** The operating system maps the file into memory when it is loaded; nothing is copied, unzipped or compiled at start.
- **Manifest without loading.** Each plugin carries a manifest in a section named `oxg_manifest` (12 characters, so it fits Mach-O's 16-character limit). The host reads it with the `object` crate, which parses the file's headers and reads only that section, without running any of the plugin's code. Reading a manifest takes well under a millisecond.
- **Loaded only when needed.** At start the host reads manifests only. A plugin's code is loaded the first time one of its contributions is used: opening a .gp5 file loads the GP5 plugin; pressing Play loads the chosen output; showing a tab loads the plugin that owns it. A plugin whose conditions don't hold (a Wayland-only plugin under X11, say) is never loaded at all.
- **Never unloaded in a session.** Unloading native code safely is unreliable (thread-local destructors, threads a plugin started). Disabling a plugin takes effect at the next start; the plugin manager says so.

### The interface

- One exported symbol, `oxiguitar_plugin_v1`, returns a table of `extern "C"` functions. The `v1` is the interface's major version: a host refuses a plugin built for another major version, with a footer message naming the plugin and the version it needs.
- Calls pass **messages as bytes**, encoded with a stable, versioned schema (`oxiguitar-plugin-api`, serde + postcard). The score crosses as a message too, so the model's in-memory layout is never part of the interface and can keep changing.
- **Audio crosses as raw sample buffers**, with no encoding and no allocation, so instruments run at full native speed on the audio thread.
- The host hands each plugin a table of **host services** at start: send a footer message, read and write its own settings, ask for the score (whole, a track, the selection, a range of bars), submit an edit (always as an undoable command, never by changing the score directly), ask the user to approve a download (NET-2), redraw its tab.
- The **SDK** (`oxiguitar-plugin`) wraps all of this in safe Rust: a plugin implements traits (`FormatReader`, `FormatWriter`, `Instrument`, `Tool`, `Exporter`, `Panel`, …) and calls one `export_plugin!` macro. Plugin authors write no `unsafe`.
- **Where the `unsafe` is.** Loading a native library and calling through function pointers is `unsafe` in Rust by nature; no plugin system with native code can avoid it. It would live in exactly two small modules, the host's loader and the SDK's export glue, reviewed line by line and covered by an interface test suite. The rest of the workspace keeps `unsafe_code = "forbid"`. **(Proposed: Mateus to accept or reject this; see the alternatives below.)**
- **The same plugin can be built in.** A plugin crate builds both as a `.oxp` and as an ordinary Rust library. The mobile apps link their plugins in (iOS forbids loading downloaded code), and our own tests use plugins as libraries without going through the interface.

### The options considered

| | Native library (proposed) | WebAssembly component (wasmtime) | Both kinds | Separate processes | Scripts (Lua, Rhai) |
| --- | --- | --- | --- | --- | --- |
| One file per plugin | Yes, per target | Yes, one for every target | Yes | One executable per target | Yes |
| Load time | mmap, microseconds; manifest read without loading | Compiling a module takes 10s–100s of ms; cached precompiled copies make later starts fast, at the cost of disk | Both | A process start, milliseconds | Parse at load |
| Speed of DSP (NAM, drums) | Native, full SIMD | Roughly 1.5–3× slower, 128-bit SIMD only | Native where it matters | Native, plus a copy per buffer across processes | Far too slow |
| Can do platform work (audio and MIDI devices, Wayland, macOS) | Yes | No: the host would have to provide every platform API, which puts that code back in the core | Only the native kind | Yes | No |
| Isolation | None: a plugin can crash the app and do anything the user can | Sandboxed: no files or network unless granted | Only the WebAssembly kind | A crashing plugin doesn't take the app down | Partly |
| Size and risk added to the core | A loader of a few hundred lines | wasmtime with its Cranelift compiler, about 10–20 MB and a JIT in the core | Both | IPC framework | An interpreter |
| `unsafe` in our code | Two small modules | None of ours (wasmtime has its own) | Two small modules | None | None |
| iOS | Built in only | Interpreter only (no JIT on iOS) | Built in | Not allowed | Yes |

Native libraries fit Mateus's priorities best: performance, start-up time, platform plugins (which WebAssembly cannot be), and the smallest core. Their costs are a build per target (CI already builds all six), no sandbox (we already decided third-party plugins may do anything, NET-4), and the contained `unsafe` above. If isolation of untrusted third-party plugins ever matters more, WebAssembly can be added later as a second kind for portable plugins, behind the same SDK traits; nothing in this design rules it out.

## What plugins contribute

A manifest lists everything a plugin adds, so the window can show its menu entries, toolbar icons and tabs before its code is loaded.

| Contribution | What it is | Loaded when |
| --- | --- | --- |
| **Format** | Reads and/or writes one file format: name, extensions, and content signatures (bytes at an offset, a ZIP member name) so the core picks it without loading anything (FMT-6). Reading returns a score and its import log (FMT-2); writing returns bytes and a list of what the format loses (FMT-3). | A file of that format is opened or saved |
| **Instrument** (output) | Turns the player's timed events into sound for the tracks given to it: MIDI synthesiser, NAM guitar, RSE, drums. **(Proposed: chosen per track, with one default.)** | Play is pressed |
| **Service** | One implementation of an interface the core or another plugin defines, so the right code is chosen per platform or environment: an audio device (PipeWire, ALSA, CoreAudio, WASAPI), a MIDI device, Wayland drops. Each declares its conditions; settings can pin a choice; otherwise the first available wins, by priority. | First use |
| **Tool** | A Tools menu entry, the `oxiguitar-tools` shape: id, name, availability with a reason, apply → a changed score as one undoable command and a report. | Picked from the menu |
| **Exporter** | File → Export entries: PDF, PNG, SVG, WAV. | Picked |
| **Tab** | A tab in the top, bottom, left or right area, with a title and an icon. Its contents are a tree of standard widgets (buttons, toggles, sliders, lists, text, fields) that the core draws in its own style, plus, if needed, a canvas the plugin draws with OxiGuitar's display list (a fretboard, a meter) and receives pointer events on. **(Proposed; the alternative is plugins shipping Slint markup run by Slint's interpreter.)** | The tab is first shown |
| **Toolbar section** | A titled group of icon buttons, toggles and drop-downs in the toolbar. Icons are vector paths in the manifest, tinted by the core. | An item is used |
| **Settings** | A schema of the plugin's settings (names, types, defaults, ranges), from which the plugin manager draws a settings form. | Never needs code |
| **Events** | What the plugin wants to hear about: score opened or changed, selection changed, playback position. Events are pushed to it as they happen; nothing polls. | When the event first fires |

The **player** works like this: the core turns the score into a timeline of events (notes with every effect, tempo, the metronome's clicks and count-in) and runs the transport. Each instrument renders its tracks' events into sample buffers on the audio thread, the core mixes them (volume, pan, mute, solo), and the audio-device service sends the mix to the system. MIDI output to hardware is an instrument that sends MIDI instead of samples. With no instrument installed, the transport is greyed out. The rules of real-time audio (no allocation, no locks, no waiting on the audio thread) bind instrument plugins too, and the SDK's test suite checks them.

**Plugins can depend on plugins.** A plugin can define a service interface for others: the NAM plugin, for example, needs a clean guitar signal, which a "guitar DI" service provides (a plucked-string model today, sampled DI notes later). The manifest says what a plugin `provides` and `requires`.

## Folders and settings

On launch the app makes sure its user folder, `settings.json` (empty, `{}`) and the user plugin folder exist, and creates any that are missing. It never creates or writes system folders; packages install those.

| System | User folder (settings.json) | User plugins | System plugins |
| --- | --- | --- | --- |
| Linux | `~/.oxiguitar/` | `~/.oxiguitar/plugins/` | `/usr/lib/oxiguitar/plugins/` (found next to the program: `<bin>/../lib/oxiguitar/plugins/`, which also covers `/usr/local`, the AppImage and a build from source) |
| macOS | `~/Library/Application Support/OxiGuitar/` | `…/OxiGuitar/plugins/` | `OxiGuitar.app/Contents/PlugIns/`, then `/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `%APPDATA%\OxiGuitar\` | `%APPDATA%\OxiGuitar\plugins\` | `plugins\` beside `oxiguitar.exe` (the portable zip) |

- The environment variable `OXIGUITAR_HOME` replaces the user folder (tests, a portable copy on a USB stick).
- **Proposed** (question 6): on Linux, the freedesktop convention would be `~/.config/oxiguitar/settings.json` and `~/.local/share/oxiguitar/plugins/`. Mateus asked for `~/.oxiguitar/`, which is what the app does today.
- `settings.json` key `plugin_folders` (a list of paths) replaces the default list. Folders are searched in order; when two folders hold the same plugin id, the higher version wins, and the earlier folder on a tie.
- Other keys, written by the core and the plugin manager: `disabled` (plugin ids), `services` (a pinned implementation per interface), `instruments` (default and per track), and one object per plugin id for its own settings. Unknown keys are kept as they are.
- A plugin that keeps data (a downloaded SoundFont) gets its own folder, `<user folder>/data/<plugin id>/`.

## The plugin manager

A plugin of its own, on the core's host services. It lists every plugin found (name, version, ours or third-party, folder, loaded or not, why it can't load), enables and disables them (from the next start), uninstalls those in a folder the user can write to (a package manager removes the others), installs a `.oxp` file the user picks, and draws each plugin's settings form. Later, and only when the user asks (NET-2), it can download our plugins from the GitHub release, checked against SHA256SUMS.

So that a disabled plugin manager can't lock a user out, the core keeps two things of its own: `oxiguitar --safe-mode` starts with no plugins at all, and Help → About lists the plugin folders.

## Distribution

- **Our plugins live in this repository**, under `plugins/`, one crate each, so a change to the interface and the plugins that use it land in one pull request while the interface is young. They may move to their own repository once the interface reaches 1.0.
- Each release attaches every plugin for every target (`oxiguitar-gp5-0.2.0-linux-x86_64.oxp`, …) to the same GitHub release, with SHA256SUMS.
- **Proposed** (question 7): the Linux packages split into `oxiguitar` (the core, as Mateus asked) and `oxiguitar-plugins` (our plugins, into `/usr/lib/oxiguitar/plugins/`), which `oxiguitar` *Recommends*, so `apt install oxiguitar` gives a working app while the core package stays the core alone. The .dmg and the Windows zip would carry the plugins in their plugin folders the same way.
- Anyone can host plugins anywhere. The core's catalogue names only ours.

## Testing

- **Each plugin tests itself** as an ordinary Rust library: readers keep their fixture, corpus, round-trip and fuzz tests, moved with them.
- **The interface has a contract suite** (`oxiguitar-plugin-test`) that loads a built `.oxp` and checks it: manifest readable without loading, every declared contribution answers, errors cross the boundary as messages, nothing panics across it, instruments allocate nothing on the audio thread.
- **The core is tested with no plugins** (it opens nothing and says why), with test plugins, and with ours.
- The existing gates stay: the speed budget now includes reading the manifests and loading the format plugin (target: the 300-bar file still opens in under a second, and start-up with 30 plugins installed costs under 10 ms of manifest reading); visual references must not change when the readers move into plugins; NET-5 runs with our plugins installed.
- CI path filters build and test only the plugins a pull request touches, plus the contract suite.

## Privacy and trust

NET-1 to NET-5 cover the core and our plugins. A native plugin can do anything the user can, so the plugin manager marks third-party plugins plainly and the manual says the promises cover only ours (NET-4). Our plugins download only when the user asks, through the core's consent dialog.

## Debian

The core must pass Debian's acceptance criteria (Mateus, 18 Oct 2026). Checked so far:

- **Licences**: DFSG-free throughout (GPL-3.0, MIT, Apache-2.0, OFL-1.1, Unlicense), enforced by cargo-deny.
- **Policy**: binaries in `/usr/bin`, plugins in `/usr/lib/oxiguitar/plugins` (FHS allows a package's own directory under `/usr/lib`), no network at build or run time. Still to do: a manual page for each binary (`oxiguitar.1`, `oxiguitar-cli.1`; lintian requires them) and a lintian run in CI. The Maintainer field holds the GitHub issue tracker by decision (lintian flags it; a package in Debian's own archive would carry its Debian maintainer's address).
- **Debian's own archive** (Proposed, question 5) would also need every crate we use packaged in Debian. Slint is on its way: `rust-i-slint-common` 1.17.1 is in unstable but not testing, and other Slint crates are in the NEW queue. The bundled fonts would have to come from Debian font packages where they exist (Noto Sans is `fonts-noto-core`; we found no Debian package for Bravura).
