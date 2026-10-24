# Plugins

OxiGuitar is a small core and a set of plugins. The core opens nothing and plays nothing by itself: it holds the score, shows it (and exports what it shows as a PDF), edits it (Infer Key Signatures included), and drives the player's transport. Every file format, every sound, every other export and tool, and every feature that depends on a platform or environment is a plugin (Mateus, 18 Oct 2026).

This document is the design, accepted by Mateus on 18 Oct 2026 with his answers to the questions in ADR 0008 ([docs/adr/0008-plugin-architecture.md](../adr/0008-plugin-architecture.md)), and amended the same day: PDF export and Infer Key Signatures stay in the core, and the core has no catalogue of formats (ADR 0008, "Amendments"). The plugin developer guide (how to write one, step by step) lives in the user manual's sister book, `docs/plugins/` ("Writing a plugin", below).

## Why

- **Small, simple core.** Read, edit, write, show (with PDF export) and the player's controls. Everything else can be added, replaced or dropped without touching the core, and the mobile apps (M10) start from the same small base.
- **No dead code.** The app never carries or loads code it cannot use on the machine it runs on: no Wayland code under X11, no CoreAudio code on Linux, no reader for a format nobody opens. Less code loaded means a smaller attack surface and a faster start.
- **One format per plugin.** Each file format is its own plugin (one for .gp3, one for .gp4, one for .gp5, one for .gpx, one for .gp, and so on), so a format can be added, improved or retired on its own.
- **Partitioned testing and work.** Each plugin is its own crate with its own tests, fuzz targets and CI path filter. Contributors can own one plugin without learning the whole codebase, and several plugins can be built in parallel.

## What the core holds

| Core | Plugins |
| --- | --- |
| The score model, edit commands, undo and redo, selection | Every file format, reading and writing: Guitar Pro 3, 4, 5, 6 (.gpx), 7–8 (.gp); later MusicXML, ABC, MuseScore, MIDI files |
| Engraving and the four layouts, drawn on screen (nobody can edit what they cannot see), and **PDF export** of what is shown, a feature of the viewer (Mateus, 18 Oct 2026) | Other exports: PNG, SVG, WAV |
| The window shell: menus, toolbar, the four tabbed areas, the page, the footer; **Infer Key Signatures** in the Tools menu, a feature of the editor (Mateus, 18 Oct 2026) | Other tools for the Tools menu |
| The player's transport: play, play selection, stop, pause, rewind, count-in, loop, metronome; the cursor; turning the score into timed events (repeats, jumps, tempo, effects) | Sound: the MIDI synthesiser, NAM guitar, RSE banks, realistic drums |
| The mixer: per-track volume, pan, mute and solo, and playback speed (instruments from several plugins are mixed here) | Audio devices (PipeWire, ALSA, CoreAudio, WASAPI) and MIDI devices |
| settings.json, the plugin folders, the plugin host, and the **plugin manager** (Tools → Plugins; Mateus, 18 Oct 2026) | |
| No format detection and no catalogue: what can be opened is what the installed plugins recognise (Mateus, 18 Oct 2026; "How the host loads and chooses") | Platform gaps we may fill one day: Wayland drops (ADR 0006), macOS open-document events (ADR 0007) |

The core compiles per target, so a Windows build holds no Linux code. On Linux the windowing toolkit would carry both X11 and Wayland in one binary, so **there are two Linux builds of the core** (Mateus, 18 Oct 2026): a Wayland build, the main one, and an X11 build for desktops that still need it. X11 is deprecated from the start, as the desktops are leaving it, and the X11 build will be dropped. Each build enables only its own Slint and winit backend (`backend-winit-wayland` or `backend-winit-x11`), through a Cargo feature of `oxiguitar-app` (`wayland`, the default, or `x11`).

| Build | Linux packages | Notes |
| --- | --- | --- |
| Wayland | `oxiguitar` (.deb, .rpm), `OxiGuitar-VERSION-x86_64.AppImage` | The default. Headless tests run it in a private headless Weston (`scripts/package/in-wayland.sh`; NET-5 and the package tests). |
| X11 (deprecated) | `oxiguitar-x11`, which Provides, Conflicts with and Replaces `oxiguitar`; `OxiGuitar-x11-VERSION-x86_64.AppImage` | Same program, X11 backend only. Its window says nothing different; the manual says it will be dropped. |

As built (slice 4):

- `oxiguitar-app` has the features `wayland` (default: `slint/backend-winit-wayland`, `rfd/wayland`) and `x11` (`slint/backend-winit-x11`); a build with neither doesn't compile. Slint's own `unstable-winit-030` feature would turn on both display systems, so the app takes the raw window events (the wheel, Ctrl + wheel, file drops) from `i-slint-backend-winit` directly: the crate `slint::winit_030` re-exports, with no features of its own, at the exact version Slint pins.
- The app tells the host its build's display system for plugins' conditions (`wayland` or `x11`), instead of guessing from the environment. A build started in a session it can't use (the Wayland build without a Wayland compositor, the X11 build without an X server) says which build to install instead, and exits.
- **Wayland first** (Mateus, 19 Oct 2026; [ADR 0009](../adr/0009-wayland-first.md)): the Wayland build takes the Wayland path, and workarounds belong to the X11 build until it is dropped. So Slint is pinned to 1.17.1: Slint 1.18's clipboard (arboard) always carries X11 code and goes through XWayland on GNOME, while 1.17.1 uses smithay-clipboard on the window's own Wayland connection. The Wayland build holds no X11 crate at all.
- `scripts/check-display-crates.sh` (CI's "Lint and docs") fails if the Wayland build holds any X11 crate (or turns on the keyboard maps' X11 support) or the X11 build any Wayland crate, on x86_64 and ARM64. CI also lints the X11 build, runs NET-5 on both builds (the Wayland one in a headless Weston, the X11 one under Xvfb), and the release workflow packages, installs and starts both (in Debian sid, Fedora and as AppImages; installing `oxiguitar-x11` replaces `oxiguitar`).

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
│ Tracks | tab                                           bottom area ▾ │
│  1 Guitar        6 strings  [𝄞][TAB]  vol ──●── pan ◐  [M][S]       │
├──────────────────────────────────────────────────────────────────────┤
│ ● No output plugin found: get one from github.com/mateusrdp/… (footer)│
└──────────────────────────────────────────────────────────────────────┘
```

- **Toolbar**: sections of icon buttons, side by side. The core has File, Edit and View sections; a plugin can add sections with its own icons. It can be hidden (View → Toolbar).
- **Four areas**: top and bottom span the window's width; left and right sit beside the page. Each holds tabs. Clicking the active tab's name, or the chevron, collapses the area to its tab strip; View → Panels hides it completely. Plugins add tabs to any area.
- **The page** sits in the middle on a dark canvas (the same colour as Dark Page's surround, whichever page colour is shown). It has no tabs and cannot be hidden.
- **Footer**: one line at the bottom with the latest message. Clicking it expands a scrollable list of this session's errors and warnings (it isn't a log: Mateus, 19 Oct 2026); clicking again shrinks it.
- **Player controls** live in the top area's Player tab, with the choice of output plugin. With no output plugin installed the controls are greyed out and the footer says where to get one.

Before M2 the containers exist with placeholders (Palette and Inspector say what will go there; the toolbar's Edit section is greyed). `docs/design/ui.md` describes the window as built.

## Plugin files

A plugin is **one native library file** (accepted, 18 Oct 2026) with a small, versioned C interface, built per target, with its manifest in a named data section.

- **One file**, named `NAME.oxp` ("OxiGuitar plugin"), for example `gp5.oxp`. Inside it is an ordinary shared library for one target: ELF on Linux, PE on Windows, Mach-O on macOS (one universal file for Intel and Apple Silicon). A plugin published on its own carries the target in its download's name (`example-0.1.0-linux-x86_64.oxp`); the user can drop it in a plugin folder as it is. Ours come together, in one zip per system (below). Icons, strings and small data are compiled into the file. Large data a plugin downloads at the user's request (SoundFonts, NAM models) goes in that plugin's data folder, not in the plugin file.
- **No unpacking.** The operating system maps the file into memory when it is loaded; nothing is copied, unzipped or compiled at start.
- **Manifest without loading.** Each plugin carries a manifest, JSON text, in a section named `oxg_manifest` (in the `__DATA` segment on macOS; on Windows `oxgmanif`, because section names in PE executables and libraries hold at most eight characters). The author writes it as `manifest.json` beside the plugin's `Cargo.toml`, and the SDK's `export_plugin!` puts the file's bytes in the section. The host reads it with the `object` crate, which parses the file's headers and reads only that section, without running any of the plugin's code. Reading a manifest takes well under a millisecond. Keys a host doesn't know are ignored, so a newer plugin's manifest still reads in an older host; the SDK's checks reject unknown keys when the plugin is built, so typing mistakes are caught there.
- **Loaded only when needed.** At start the host reads manifests only. A plugin's code is loaded the first time one of its contributions is used: opening a .gp5 file loads the GP5 plugin; pressing Play loads the chosen output; showing a tab loads the plugin that owns it. A plugin whose conditions don't hold (a Wayland-only plugin under X11, say) is never loaded at all.
- **Never unloaded in a session.** Unloading native code safely is unreliable (thread-local destructors, threads a plugin started). Disabling a plugin takes effect at the next start; the plugin manager says so.

### The interface

- One exported symbol, `oxiguitar_plugin_v1`, returns a table of `extern "C"` functions. The `v1` is the interface's major version: a host refuses a plugin built for another major version, with a footer message naming the plugin and the version it needs.
- Calls pass **messages as bytes**, encoded with a stable, versioned schema (`oxiguitar-plugin-api`, serde + postcard). Answers come back through a "sink" the caller provides: the callee hands it bytes and the caller copies them, so memory is never freed on the other side of the interface and the two sides may use different allocators.
- **The score crosses as its own types**, `oxiguitar_plugin_api::score`, which mirror the model field for field but belong to the interface (version 1, its encoding pinned by a test). The core converts between the two (`oxiguitar-model`'s `wire` feature; every fixture, reference and corpus file crosses without loss). So the model can keep changing as the editor grows; a change that plugins must see goes into a new version of the wire score, beside version 1.
- **Versions**: the interface's major version is in the entry symbol's name and the manifest; a minor version only adds (new messages, new variants at the end of an enum, new fields at the end of a C table, new manifest keys), and the host sends a plugin only what its minor version knows. Version 1.0 has formats, instruments (declared), services, footer messages and settings; **1.1** adds tabs, the canvas's display list, pushed events and the open score (slice 3), each at the end of its enum; **1.2** (M3 slice 2, frozen with v0.5) adds the score version 2 (version 1's tree with a list of extensions on each part, which later minor versions extend at its end, so no version 3; free time first), the editor's selection (`Selection2`: caret with voice and string, beats of a voice, bars across tracks), tools in the Tools menu (manifest `tools`, `Request::Tool`) and changes to the score (`HostRequest::Score2` gives it with its revision, `HostRequest::Apply { revision, name, score }` hands it back, recorded as one undoable command unless the score changed since). The host sends a 1.2 plugin `Write2`, `Selection2` and the score without extensions newer than its version; older plugins get version 1, and what version 1 can't hold is named in a write's losses. Interface 1 is frozen from the OxiGuitar 0.2 release: later versions only add.
- **Audio crosses as raw sample buffers**, with no encoding and no allocation, so instruments run at full native speed on the audio thread.
- The host hands each plugin a table of **host services** at start. Built: send a footer message, read and write its own settings, call a service it `requires`, and (1.1) ask for the open score and redraw its tab. To come, each when a real plugin needs it: parts of the score (a track, the selection, a range of bars), submitting an edit (always as an undoable command, never by changing the score directly, M3), asking the user to approve a download (NET-2).
- The **SDK** (`oxiguitar-plugin`) wraps all of this in safe Rust: a plugin implements traits (`FormatReader`, `FormatWriter`, `Service`, `Tab`; since 1.2 `FormatReader2`, `FormatWriter2` and `Tool`; `Instrument` and `Exporter` when they join the interface) and calls one `export_plugin!` macro. Plugin authors write no `unsafe`.
- **Licences**: the interface (`oxiguitar-plugin-api`) and the SDK (`oxiguitar-plugin`) are **MIT OR Apache-2.0** (Mateus, 18 Oct 2026), so a plugin, which compiles the SDK into itself, may use any GPL-compatible licence. The core stays GPL-3.0-or-later, and our own plugins are GPL-3.0-or-later like it. A plugin that is not GPL-compatible isn't possible in practice: it runs inside the app, which includes Slint under its GPL-3.0-only option.
- **Where the `unsafe` is.** Loading a native library and calling through function pointers is `unsafe` in Rust by nature; no plugin system with native code can avoid it. It lives in exactly two small modules, the host's loader (`crates/oxiguitar-host/src/loader.rs`) and the SDK's export glue (`crates/oxiguitar-plugin/src/glue.rs`, plus the two attributes `export_plugin!` generates: the manifest's section and the entry symbol), reviewed line by line, every block with a `// SAFETY:` comment, and covered by the contract suite on every system CI tests. Their crates, and plugin crates, deny `unsafe_code` and allow it only there (a crate that forbids it can't allow it in one module); the rest of the workspace keeps `unsafe_code = "forbid"`, and a test fails if `unsafe` is allowed anywhere else. Mateus accepted this (18 Oct 2026): it is the price of native plugins, not a workaround around another library.
- **The same plugin can be built in.** A plugin crate builds both as a `.oxp` and as an ordinary Rust library. The mobile apps link their plugins in (iOS forbids loading downloaded code), and our own tests use plugins as libraries without going through the interface.

### How the host loads and chooses

As built in `oxiguitar-host` (`Plugins`):

- **At start**, every `.oxp` file directly in the plugin folders is examined, in folder order and then by name, and nothing is loaded. A file is usable when its manifest reads and is valid, it is whole (every segment its headers describe lies inside the file, so a truncated download is refused before it is loaded), it is a library for this system and processor (one universal Mach-O file holds both of the Mac's), its interface is 1.x with a minor version this host has, its conditions (system, processor, display system) hold, and settings.json doesn't list its id under `disabled`. The footer names the plugins found, and every file that can't be used and why.
- **One file per plugin id**: the highest version is used; on a tie, the file in the earlier folder, then the earlier name. The others are reported as replaced.
- **Loaded on first use, never unloaded.** The host loads a plugin when a contribution of it is first used, checks that its table carries the same manifest as its file's section and was built for an interface the host has, and starts it once. A plugin that fails to start is withdrawn (below). A plugin asking for its own contributions while it starts gets an error instead of waiting forever.
- **Formats**: a file's format is found from the signatures in the manifests, without loading anything; the first usable plugin (in folder order) with a matching signature reads it. The formats of the usable plugins are all the core can open: it detects nothing itself and names no plugin it doesn't have (Mateus, 18 Oct 2026). With no plugin installed it opens nothing.
- **A file no plugin recognises** is refused, and the footer says so. When a usable plugin declares the file's extension among its formats' extensions (a `.gp5` file the Guitar Pro 5 plugin doesn't recognise), the message asks for a bug report at the issue tracker; otherwise it says no installed plugin opens this kind of file and asks for a feature request there.
- **Instruments**: the player offers a track the instruments whose classes include the track's (`InstrumentClass::of(program, percussion)`).
- **Services**: for an interface, the candidates are the services of usable plugins whose own conditions hold. settings.json may pin one, as `"services": {"INTERFACE": "PLUGIN-ID/SERVICE-ID"}`; otherwise the highest `priority` comes first, then plugin order. Each candidate's probe is asked in turn whether it works here (loading its plugin), and the first that says yes is used for the rest of the session. A pin that can't run here is skipped, and said so if nothing else works.
- **Host services**: a plugin's footer messages carry its name; its settings are the object under its id in settings.json, saved at once through the one copy of settings.json the app also uses; it may call only the service interfaces its manifest lists under `requires`.
- **Errors and panics**: a plugin's error crosses with its kind, offset and context. **A plugin that fails is withdrawn, as if it weren't installed** (Mateus, 19 Oct 2026): one that can't be loaded or started, crashes (a panic, caught in the plugin), or breaks the interface's contract (an answer that can't be read or wasn't asked for, tab contents the core can't show) is not used again this session, and everything else goes on as if it had never been installed: its formats, tabs, services, instruments and events are gone, a file it would have read goes to the next plugin that recognises it, and a service is chosen again among the others. The footer says so once, as an error, and the plugin manager shows why. An error a plugin answers with (a damaged file, a widget it refuses) is an answer, not a failure. (A plugin built with `panic = "abort"` takes the program down instead.) As built: `Found::failure`, `Plugins::open` (recognise and read, falling back to the next plugin), `call_service` choosing again, `Core::withdrawn` (the window takes the plugin's tabs away), and the contract tests with `plugins/test-second`, a second test plugin reading the test plugin's format.
- **Threads**: any call may come from any thread. A program has one `Plugins`: the system loads a library file once per process, so a second host over the same file would find the plugin already started.

### Tabs and events (interface 1.1)

As built in slice 3 (`oxiguitar_plugin_api::tab`, `display` and `event`; the host's `Plugins::tabs`, `tab_content`, `tab_event`, `push`):

- **A tab** is declared in the manifest (`"tabs": [{"id": "tuner", "title": "Tuner", "area": "bottom"}]`; areas `top`, `bottom`, `left`, `right`), so the window shows its name without loading the plugin. When the tab is first shown, the host asks the plugin for its contents (`Request::Tab`), which loads it.
- **Contents** are rows from top to bottom, each holding widgets from left to right: a label (plain or muted), a button, a toggle, a slider, a one-line field, a choice (drop-down), a list with one selected line, and a canvas. The core draws them in the window's own style, so every plugin looks like the rest of OxiGuitar and follows Dark Interface by itself. A shape this simple shows the same way in every area and, later, on mobile.
- **The user using a widget** is sent to the plugin (`Request::TabEvent`: a click, a tick, a slider let go, Enter in a field, an option or line chosen, the pointer on a canvas), and the plugin answers with the tab's new contents. A plugin whose tab changes on its own (after an event, say) asks the host to redraw it (`HostRequest::Redraw`); the window asks for the contents again when the tab is on screen.
- **The canvas** is drawn with the display list's wire form, version 1 (`oxiguitar_plugin_api::display`): lines, rectangles, paths (filled and stroked) and text, in the canvas's own units with y pointing down, coloured by role (ink, page, accent, muted) so the window's theme resolves them. Text crosses as text and the core places it with its own fonts (the text font, or the music font with SMuFL code points), so a plugin needs no fonts. The core scales the canvas to the room it has, keeping its proportions, and reports pointer positions (down, drag, up; which button) in the canvas's units; drags the plugin hasn't answered yet are merged. A canvas is at most 100,000 units a side and 100,000 items, and everything on it stays within its reach: every point within the canvas extended by its own size on every side, no size or line width over three times its longer side (the `plugin_tab` fuzz target found that a line 10²⁰ units long made the renderer fail); text running past that reach is cut there. `oxiguitar-display`'s `wire` feature turns it into the core's own display list, drawn by the same renderer as the score.
- **Contents the core can't show** (an id missing or used twice, a slider whose range is backwards, a choice of an option it doesn't have, a canvas that is empty, too large, or holds a number that isn't finite) are refused with a sentence that names the first problem, and the tab says so; the plugin's other tabs carry on. The SDK's `check`, run by `cargo test`, catches them before a plugin ships.
- **Events**: a plugin lists what it wants under `"events"`: `score` (a score was opened, changed or closed; each carries a revision number, never the score itself), `selection` (a run of beats in a track, or nothing) and `playback` (playing or not, the beat the cursor is on, and the time in seconds). The host pushes each event to every usable plugin that subscribes, loading it the first time one fires. They are delivered in order, on one thread of the host's that sleeps until an event arrives; nothing polls. Events that say where something is now (the selection, the playback position, the score's revision) replace one of the same kind still waiting at the end of the queue, so a slow plugin gets the latest rather than a backlog. A plugin that answers an event with an error is named once in the footer; one that crashes on it is withdrawn.
- **The open score**: a plugin that needs it after a score event asks for it (`HostRequest::Score`), so the score is converted and sent only to plugins that use it, when they use it.
- Today the app sends score events. Selection events come with the editor (M3) and playback events with the player (M4). The interface already carries both, and the contract suite tests them through the host.

### Writing a plugin

The plugin developer guide, [`docs/plugins/`](../plugins/src/README.md) (mdBook, published beside the user manual at `/plugin-guide/`), shows how, step by step, from the template plugin in `docs/plugins/template/` (M2's slice 7). The template is a workspace of its own that authors copy: a format (read and write), a tab with widgets and a canvas, score events, settings and a service. It takes the SDK and the contract suite from this repository at the release's tag (git dependencies; nothing is published on crates.io: Mateus, 19 Oct 2026), and its `tests/contract.rs` builds the `.oxp` file and runs the contract suite under `cargo test`. The guide's code examples are included from the template's sources (mdBook anchors), so they are the code CI builds: `scripts/check-plugin-template.sh` copies the template out of the repository, points it at this checkout's SDK, and runs fmt, clippy, its tests, `oxiguitar-plugin-test --build` and `oxiguitar-cli` on its sample, on Linux, Windows and macOS (CI's "Plugin template" jobs, part of `ci-success`).

`plugins/test` remains the example of every part of the interface, and our Guitar Pro plugins (`plugins/gp3` … `plugins/gp`) of a format plugin as we ship it.

### The options considered

| | Native library (chosen) | WebAssembly component (wasmtime) | Both kinds | Separate processes | Scripts (Lua, Rhai) |
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
| **Instrument** (output) | Turns the player's timed events into sound for the tracks given to it: MIDI synthesiser, NAM guitar, RSE, drums. Each track chooses its instrument, with one default for the rest. The manifest says which **instrument classes** it plays (below); without the field it plays everything. | Play is pressed |
| **Service** | One implementation of an interface the core or another plugin defines, so the right code is chosen per platform or environment: an audio device (PipeWire, ALSA, CoreAudio, WASAPI), a MIDI device, Wayland drops. Each declares its conditions; settings can pin a choice; otherwise the first available wins, by priority. | First use |
| **Tool** | A Tools menu entry (interface 1.2): the manifest's `tools` gives its id, name and description; picked, the core asks the plugin to run it, and it changes the score through `Apply` (one undoable command) and answers with a report for the footer. The core's own tools (`oxiguitar-tools`) come first in the menu. | Picked from the menu |
| **Exporter** | File → Export entries: PNG, SVG, WAV (PDF is the core's). Added to the interface when the first exporter plugin needs it. | Picked |
| **Tab** | A tab in the top, bottom, left or right area, with a title (interface 1.1; "Tabs and events" below). Its contents are a tree of standard widgets (buttons, toggles, sliders, lists, text, fields) that the core draws in its own style, plus, if needed, a canvas the plugin draws with OxiGuitar's display list (a fretboard, a meter) and receives pointer events on. Chosen over Slint markup run by Slint's interpreter: no compiler in the core, one look, and it works the same on mobile (Mateus: whatever is fast, easy and elegant, 18 Oct 2026). | The tab is first shown |
| **Toolbar section** | A titled group of icon buttons, toggles and drop-downs in the toolbar. Icons are vector paths in the manifest, tinted by the core. | An item is used |
| **Settings** | A plugin keeps its settings under its id in settings.json and shows them, if it has any, in its own tab. The core draws no settings forms (Mateus, 18 Oct 2026). | |
| **Events** | What the plugin wants to hear about: score opened or changed, selection changed, playback position. Events are pushed to it as they happen; nothing polls. | When the event first fires |

The **player** works like this: the core turns the score into a timeline of events (notes with every effect, tempo, the metronome's clicks and count-in) and runs the transport. Each instrument renders its tracks' events into sample buffers on the audio thread, the core mixes them (volume, pan, mute, solo), and the audio-device service sends the mix to the system. MIDI output to hardware is an instrument that sends MIDI instead of samples. With no instrument installed, the transport is greyed out. The rules of real-time audio (no allocation, no locks, no waiting on the audio thread) bind instrument plugins too, and the SDK's test suite checks them.

### Instrument classes

An instrument plugin declares what it can play, so the player offers it only for tracks it suits; a plugin that says nothing plays everything (Mateus, 18 Oct 2026). The manifest has two optional lists, `plays` and `does_not_play`, of classes from this fixed set, which the core derives from each track's instrument (General MIDI program, percussion flag and strings):

| Class | Tracks |
| --- | --- |
| `fretted` | Guitars and basses with frets, banjo, sitar (GM 24–34, 36, 37, 104, 105; the same set the engraver treats as fretted) |
| `fretless` | Fretless bass and stringed instruments without frets: shamisen, koto (GM 35, 106, 107) |
| `bowed` | Violin, viola, cello, contrabass, string ensembles, fiddle (GM 40–45, 48–51, 110) |
| `keyboard` | Pianos, organs, chromatic percussion such as vibraphone (GM 0–23) |
| `voice` | Choirs and voices (GM 52–54) |
| `winds` | Brass, reeds and pipes, bagpipe, shanai (GM 56–79, 109, 111) |
| `synth` | Synth basses, leads, pads and effects (GM 38, 39, 80–103) |
| `drums` | Percussion tracks (the drum-kit flag, MIDI channel 10) |
| `other` | Harp, timpani, orchestra hit, kalimba, percussive and sound effects (GM 46, 47, 55, 108, 112–127) |

Every one of the 128 programs falls in exactly one class (`oxiguitar_plugin_api::class`, tested).

The `instruments` key is in interface 1.0, but the instrument interface itself comes with the player (M4), in a later minor version. **The player ignores instruments declared by a plugin built for an interface before that one** (Mateus, 19 Oct 2026: the key stays, and the rule avoids rework), so until then no plugin counts as an output.

`plays` lists what the plugin covers (absent = everything); `does_not_play` removes classes from that. A realistic-drums plugin says `plays: ["drums"]`; the MIDI synthesiser says nothing. A track whose chosen instrument no longer exists falls back to the default, with a footer message.

**Plugins can depend on plugins.** A plugin can define a service interface for others: the NAM plugin, for example, needs a clean guitar signal, which a "guitar DI" service provides (a plucked-string model today, sampled DI notes later). The manifest says what a plugin `provides` and `requires`.

## Folders and settings

On launch the app makes sure its user folder, `settings.json` (empty, `{}`) and the user plugin folder exist, and creates any that are missing. It never creates or writes system folders; packages install those.

| System | User folder (settings.json) | User plugins | System plugins |
| --- | --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/oxiguitar/` (default `~/.config/oxiguitar/`) | `$XDG_DATA_HOME/oxiguitar/plugins/` (default `~/.local/share/oxiguitar/plugins/`) | `/usr/lib/oxiguitar/plugins/` (found next to the program: `<bin>/../lib/oxiguitar/plugins/`, which also covers `/usr/local`, the AppImage and a build from source) |
| macOS | `~/Library/Application Support/OxiGuitar/` | `…/OxiGuitar/plugins/` | `OxiGuitar.app/Contents/PlugIns/`, then `/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `%APPDATA%\OxiGuitar\` | `%APPDATA%\OxiGuitar\plugins\` | `plugins\` beside `oxiguitar.exe` (the portable zip) |

- On Linux the folders follow the freedesktop [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/) convention, like the rest of the desktop (Mateus, 18 Oct 2026): settings under the config home, plugins and their data under the data home. Relative or empty `XDG_*` values are ignored, as the specification says.
- The environment variable `OXIGUITAR_HOME` puts everything in one folder instead (`settings.json`, `plugins/`, `data/`): for tests and for a portable copy on a USB stick.
- `settings.json` key `plugin_folders` (a list of paths) replaces the default list. Folders are searched in order; when two folders hold the same plugin id, the higher version wins, and the earlier folder on a tie.
- Other keys, written by the core (the plugin manager writes `disabled`): `disabled` (plugin ids), `services` (a pinned implementation per interface, `{"audio-device": "PLUGIN-ID/SERVICE-ID"}`), `instruments` (default and per track), and one object per plugin id for its own settings. Unknown keys are kept as they are. The app and every plugin write through one shared copy, so none overwrites another's keys; a settings.json that couldn't be read at start is never written.
- A plugin that keeps data (a downloaded SoundFont) gets its own folder: `$XDG_DATA_HOME/oxiguitar/data/<plugin id>/` on Linux, `data/<plugin id>/` in the user folder elsewhere.

## The plugin manager

Part of the core, not a plugin (Mateus, 18 Oct 2026; ADR 0008, "Amendments"), so it can't be disabled from its own list. It opens only from **Tools → Plugins**, as a separate, non-modal window, and its only task is to list the plugins and enable or disable them:

- It reads what is in the plugin folders, the user's and the system's (or those `plugin_folders` lists), and nothing else. It never copies, downloads, installs or removes a file. Installing a plugin means putting its `.oxp` file in a plugin folder, by hand or with a package; removing it means deleting the file.
- One row per plugin file found: its name, its version, a tickbox to enable or disable it, its folder, and its state: loaded, not loaded yet (plugins load on first use), replaced by another file of the same plugin, or why it can't be used (damaged, another system's, a newer interface, its conditions don't hold).
- Ticking or unticking writes `disabled` in settings.json and takes effect at the next start, since plugins are never unloaded; the window says so. A file that can't be used for another reason has no tickbox.
- It makes no distinction between our plugins and anyone else's, and neither does the rest of the app (Mateus, 18 Oct 2026): we make promises only about what we distribute, in the README and the manual (NET-4).

Two more things in the core help when a plugin misbehaves: `oxiguitar --safe-mode` starts with no plugins at all (every manifest is still read, so the manager lists every file and can disable one; nothing is loaded, nothing opens, no tab or event), and Help → About lists the plugin folders in search order.

As built in slice 3: `crates/oxiguitar-app/src/manager.rs` (the rows and the `disabled` list, unit-tested without a window) and `ui/plugin_manager.slint`; the host's `Plugins::discover_in_safe_mode` and `start_in_safe_mode`. A tick is refused, saying why, when settings.json couldn't be read at start, since it is then never written.

## Distribution

- **Nothing goes to crates.io** (Mateus, 19 Oct 2026: we write an application, and our code stays on GitHub). Plugin authors take the SDK and the contract suite from this repository at a release's tag, as git dependencies.
- **Our plugins live in this repository**, under `plugins/`, one crate each, so a change to the interface and the plugins that use it land in one pull request while the interface is young. They may move to their own repository once the interface reaches 1.0.
- **One zip of plugins per system** (Mateus, 21 Oct 2026, from the release after 0.5.0): each release attaches the core packages and, for each system, `OxiGuitar-plugins-basic-VERSION-SYSTEM-ARCH.zip` (and the Linux .deb and .rpm), with SHA256SUMS; no plugin is attached on its own (0.2.0 and 0.5.0 attached all 25, and users couldn't tell which to take). The release notes open with a table of the two files each system needs (`scripts/package/release-notes.md`). Plugins don't sit in subfolders, inside the zip or in a plugin folder: the host reads only `.oxp` files directly in a folder.
- **Two packages** (Mateus, 18 Oct 2026): `oxiguitar`, the core alone, and `oxiguitar-plugins-basic`, the plugins we write and recommend, installed into `/usr/lib/oxiguitar/plugins/`. `oxiguitar` lists `oxiguitar-plugins-basic` under **Suggests** (apt doesn't install suggested packages by default, so the core really ships alone). Both must meet Debian's criteria for its archive. The basic set: the Guitar Pro format plugins (3, 4, 5, 6, 7–8), the MIDI instrument and the audio-device plugins for Linux. The .rpm splits the same way (`Suggests:`); the AppImage, the Windows zip and the .dmg come in two downloads each, the core and a zip of the basic plugins (Mateus, 18 Oct 2026: no plugins inside the AppImage), the plugins unpacked into the user's plugin folder by hand.
- With the core alone, the footer says that nothing can be opened or played yet and where to get plugins.
- Built in M2's slice 5. Each release carries, for every system, `OxiGuitar-plugins-basic-VERSION-SYSTEM-ARCH.zip`: the basic plugins flat, with a README naming the plugin folders. On Linux the plugins also come as `oxiguitar-plugins-basic` (.deb and .rpm, `Enhances: oxiguitar`); the X11 core takes the same package. No core package holds a plugin: the AppImage, the Windows zip and the .app have no plugin folder of their own. The release checks that each core package alone opens nothing and that it opens a Guitar Pro file once the plugins are added. Until M4 the basic set is the five Guitar Pro plugins; M4 adds the player's instrument and the system's audio-device plugin to the same zip and packages.
- Anyone can host plugins anywhere. The core names none: its footer points to our releases page when nothing can be opened or played.

## Testing

- **Each plugin tests itself** as an ordinary Rust library, and **through its file**: tested together in one workspace, Cargo gives every plugin the features of all, so what holds only for a plugin built alone (that it carries and reads only its own format) is checked on its `.oxp` file. The readers' fixture, corpus and fuzz tests stay with the reader code in `oxiguitar-formats`; `oxiguitar-plugin-test`'s `ours` test reads every fixture, reference and corpus file through the five Guitar Pro plugins' files and compares the result with the in-process reader, and the `plugin_dispatch` fuzz target covers the plugins' side of the interface.
- **The interface has a contract suite** (`oxiguitar-plugin-test`) that builds a plugin as an `.oxp` and checks it through the host: the manifest reads without loading, it starts, a contribution it doesn't have is "not found", damaged input (nothing, zeros, its signature alone) and an empty score never crash it, every service answers its probe, every tab's contents can be shown before and after a plausible use of each widget (a click, a tick, the slider's lowest value, an empty field, the first option, a press, drag and release on a canvas), and every kind of event it subscribes to is taken without an error. The instrument checks (nothing allocated, locked or waited for on the audio thread) come with the instrument interface (M4). The test plugin (`plugins/test`) runs every part of the interface end to end: reading and writing with their logs, errors with offsets, a caught panic, the choice of services (conditions, priority, probe, a pin), instruments by class, settings, footer messages, a plugin calling a service through the host, duplicates, disabled and damaged files, and (1.1) two tabs in two areas with every kind of widget and a canvas, pointer events in canvas units, contents the core refuses, events in order and merged, a redraw asked for after an event, and the open score asked for. The `plugin_tab` fuzz target covers a plugin's answer with a tab's contents as the core takes it, canvases drawn.
- **The core is tested with no plugins** (it opens nothing and says why), with test plugins, and with ours.
- The existing gates stay: the speed budget now includes reading the manifests and loading the format plugin (target: the 300-bar file still opens in under a second, and start-up with 30 plugins installed costs under 10 ms of manifest reading); visual references must not change when the readers move into plugins; NET-5 runs with our plugins installed.
- CI path filters build and test only the plugins a pull request touches, plus the contract suite.

## Privacy and trust

NET-1 to NET-5 cover the core and our plugins. A native plugin can do anything the user can, so the README and the manual say plainly that the promises cover only the app and the plugins we publish (NET-4). The app itself makes no distinction between our plugins and others' (Mateus, 18 Oct 2026). Our plugins download only when the user asks, through the core's consent dialog.

## Debian

Goal (Mateus, 18 Oct 2026): `oxiguitar` and `oxiguitar-plugins-basic` hosted in Debian's own archive, in **main**, as MuseScore and TuxGuitar are (TuxGuitar is in unstable as `1.6.6+dfsg1-3`). Main is the right area: everything we use is DFSG-free. Debian's other areas don't help with what is hard here: **non-free** is for software whose *licence* isn't free and **contrib** for free software that needs non-free software, while our obstacle is packaging work, not licences.

What Debian's archive needs, and where we stand:

| Need | Where we stand | Work |
| --- | --- | --- |
| DFSG-free licences for every file | Yes: GPL-3.0, MIT, Apache-2.0, OFL-1.1 and the like, enforced by cargo-deny | Keep it; a `debian/copyright` listing every crate's licence, generated like THIRD-PARTY-LICENSES.html |
| Build from source with Debian's own tools, offline | Builds need no network. Our minimum Rust (1.99) comes from rustup since 24 Oct 2026 (Mateus) and is ahead of Debian unstable's rustc (1.97.1; 1.98 in experimental): the archive build waits for unstable to reach it, so it is raised only when needed | A `debian/` directory (debhelper and `dh-cargo`) building both packages; a CI job building it in a Debian sid container with the network off |
| **Every Rust crate we use packaged in Debian** (`librust-*-dev`), at versions Debian has, or a justified copy in our source | The hard part. Slint is on its way (`rust-i-slint-common` 1.17.1 in unstable, other Slint crates in the NEW queue) | An audit of every crate in `Cargo.lock` against Debian; package the missing ones with the Debian Rust team (its `debcargo` tooling), or narrow our dependencies |
| No copies of things Debian already ships | We bundle Bravura and Noto Sans | Use `fonts-noto-core` instead of our Noto copy in the Debian build; Bravura has no Debian package we found, so we package it (with its sources) or ship it with its OFL licence |
| Debian Policy and a lintian-clean package | Paths follow the FHS (`/usr/bin`, `/usr/lib/oxiguitar/plugins`) | Manual pages for `oxiguitar` and `oxiguitar-cli`; lintian in CI |
| A Maintainer with a working email, and a Debian Developer to sponsor uploads | Our own .deb names the GitHub issue tracker (by decision); Debian requires an email address of a person or a team there | Find a sponsor (mentors.debian.net, a request for sponsorship, or a team such as Debian Multimedia); the Maintainer becomes that team's list or a project address, not Mateus's own email |

Our GitHub release .deb can keep shipping in the meantime; the Debian archive's packages would be built from the same source by Debian.
