# Architecture

OxiGuitar is a small core and a set of plugins (decided 18 Oct 2026; [ADR 0008](docs/adr/0008-plugin-architecture.md), [docs/design/plugins.md](docs/design/plugins.md)). The core holds the score, shows it (with PDF export) and edits it (with Infer Key Signatures), and drives the player's transport; file formats, sound, other exports and tools, and anything platform-specific are plugins, loaded only when used. The core recognises no file format itself: what it can open is what the installed plugins recognise (ADR 0008, "Amendments"). Inside the core, one score model sits at the centre of a Cargo workspace; layout, editing, the plugin host and the GUI are separate crates around it, so the GUI can change without touching the music code, and the command-line tool can render and test files in CI without a window.

```text
   THE CORE                                             PLUGINS (.oxp files, M2 on)
                                                        loaded on first use
   oxiguitar-app (window shell, plugin  oxiguitar-cli
                 manager, Slint)
        |      |       |      \            |           formats: gp3 gp4 gp5 gpx gp
        |      |       |       \           |                    (later musicxml abc
        v      v       v        v           v                     mscz midi)
   render  oxiguitar-edit  oxiguitar-host --------------> exporters: png svg wav
   (screen, (commands,     (folders, settings,            tools (later)
    PDF)     undo/redo)     manifests, loading,           instruments: midi nam rse drums
     v         |            services)  ^                  services: audio and midi devices,
   display <- layout       oxiguitar-play                           wayland drops ...
   (display    |           (events, transport,            tabs
    list,      |            mixer)                          ^
    fonts)     |              |        |                    | oxiguitar-plugin (SDK)
               \______________|________|____________________| oxiguitar-plugin-api
                              v                               (messages)
                       oxiguitar-model
           Score > Track > Staff > Bar > Voice > Beat > Note
```

Arrows show which crate calls which. Every crate reads or writes the shared model, and plugins see it only through the plugin interface's messages, so the model can change without breaking them.

## Crates

| Crate | Status | Purpose |
| --- | --- | --- |
| `oxiguitar-model` | M0 | The score model: data types only, no I/O. Its `wire` feature converts to and from the score as it crosses the plugin interface. |
| `oxiguitar-formats` | M0 | Format detection from contents (FMT-6), readers and writers, import logs (FMT-2). The .gp writer (M3, `src/gpif/write/`, feature `gp`) names what a file can't hold (FMT-3). Since M2 the shared reader code of the format plugins: one Cargo feature per reader (`gp3`, `gp4`, `gp5`, `gpx`, `gp`) so each plugin carries only its own, and `plugin` for what they share. The core doesn't depend on it. |
| `oxiguitar-cli` | M0 | `info`, `detect`, `render`, `tools`, later `convert`. Used by CI. Opens files through the format plugins, like the app, with the app's folders and settings read only. |
| `oxiguitar-host` | M2 | The plugin host: the user folder, settings.json (one copy shared by the app and the plugins), the plugin folders, reading manifests without loading code (`oxp.rs`, with `object`), loading plugins on first use and never unloading them (`loader.rs`, the host's one `unsafe` module, with `libloading`), and `Plugins`: formats by signature, instruments by class, services by conditions, priority, probe and settings. |
| `oxiguitar-plugin-api` | M2 | The plugin interface: the C tables, the messages (postcard), the manifest (JSON in the `oxg_manifest` section), the score as plugins see it (version 1, frozen, and version 2, which grows by extensions; interface 1.2), format signatures, instrument classes. MIT OR Apache-2.0. |
| `oxiguitar-plugin` | M2 | The SDK plugin authors use: traits and `export_plugin!`, safe Rust only (the export glue in `src/glue.rs` is the SDK's one `unsafe` module). MIT OR Apache-2.0. |
| `oxiguitar-plugin-test` | M2 | The plugin contract suite: builds a plugin crate as an `.oxp` file and checks it against the interface through the host; also a program for plugin authors. Test-only. |
| `plugins/test` (`oxiguitar-test-plugin`) | M2 | A plugin that exercises every part of the interface, built as `test.oxp` by the contract tests. Never shipped. |
| `plugins/gp3`, `gp4`, `gp5`, `gpx`, `gp` | M2 | Our Guitar Pro format plugins (`gp3.oxp` … `gp.oxp`, ids `oxiguitar.gp3` …): thin crates over `oxiguitar-formats`, each with one reader; `gp` also writes .gp files (M3). `scripts/build-plugins.sh` builds them. |
| `oxiguitar-edit` | M3 | The editor without a window: the caret and how the keys move it (as Guitar Pro 8's cursor), selections, edit commands with unlimited undo and redo that copy only the parts of the score they change, and the check for bars that don't add up (every bar on open, then only the bars an edit touches). See [docs/design/editor.md](docs/design/editor.md). |
| `oxiguitar-display` | M0 | The display list (positioned glyph runs, lines, rectangles, paths, colour roles), the bundled fonts and SMuFL metadata (ADR 0005). |
| `oxiguitar-layout` | M1 | Engraving: turns the model into a display list per page, or one long sheet, in four layouts, with a map of where every staff, bar, beat and string is drawn (`Hits`) for the editor's clicks and caret. See [docs/design/engraving.md](docs/design/engraving.md). |
| `oxiguitar-tools` | M1 | The Tools menu's core tools: actions the user starts that change a score, such as inferring key signatures. Model only, one shape per tool. Stays in the core: Infer Key Signatures is a feature of the editor (18 Oct 2026). See [docs/design/tools.md](docs/design/tools.md). |
| `oxiguitar-render` | M0 | Draws a display list into pixels (screen, PNG) with tiny-skia and into PDF with krilla (ADR 0005); SVG later. PDF export stays in the core, a feature of the viewer (18 Oct 2026). |
| `oxiguitar-play` | M4 | The player's core. Built (slice 1): the **timeline**, the score as it plays: the playing order (`order.rs`: repeats, endings, D.C., D.S., codas, Fine), time (`time.rs`: bar lengths, tempo with exact gradual changes, swing and triplet feel, and the way back from a time to the bar and position being heard, for the cursor), every note as an event with its exact start and end in seconds and what it means for the sound (`events.rs`: every TAB-1 effect), the metronome and the count-in (`click.rs`), and the text the golden files hold (`text.rs`). To come: the transport, the scheduler and the mixer. The synthesiser and the audio device are plugins. |
| `oxiguitar-app` | M1 | The desktop window, binary `oxiguitar`, in Slint (ADR 0004), following [docs/design/ui.md](docs/design/ui.md). A worker thread reads, engraves and draws the score as tiles; the window only shows them, so it never waits on the music code. |

| `oxiguitar-testgen` | M1 | Test code only: scores written in a short text notation and saved as Guitar Pro 7 files, for the speed budget and visual regression tests. See [docs/testing.md](docs/testing.md). |

Crates marked with a later milestone do not exist yet. Throwaway experiments live in [spikes/](spikes/), outside the workspace.

## Design rules

- **One model.** Every format is read into, and written from, the same score model: Score, Track, Staff, Bar, Voice, Beat, Note, with effects attached to the beat or note they act on. Format-specific data we cannot represent goes into the import log, never silently away.
- **Exact time.** Durations and positions are exact fractions, so tuplets, nested tuplets and grace notes never drift through rounding.
- **Edits are commands.** Every change is a reversible command object, which gives unlimited undo and redo.
- **Layout produces a display list.** The engraving engine outputs positioned glyphs and lines. Screen, PDF, SVG and PNG all draw the same list, so what you see is what you export.
- **Real-time audio never waits.** The audio thread allocates no memory and takes no locks; it receives events through lock-free queues.
- **Small core, everything else a plugin** (from M2). The core never carries or loads code it cannot use on the machine it runs on; it compiles per target, and a plugin's code loads only when one of its contributions is used.
- **Thin GUI.** All logic lives in toolkit-free crates, because Rust GUI toolkits change fast.
- **Untrusted input.** Files come from anywhere. Readers never panic, never allocate without bounds checks, and report errors with the file and position.
- **No network** and no downloads; the only use planned is an update check the user starts (NET-1, NET-2, as changed on 21 Oct 2026). CI enforces this.
- **No polling.** Code reacts to events or waits on something that wakes it; see CONTRIBUTING.md, "Design principles".

## Where decisions are recorded

[docs/adr/](docs/adr/) holds one short record per significant decision. [docs/formats/](docs/formats/) holds what we have learned about each file format.
