# 0008 — A small core; formats, sound, exports and platform code as plugins

- Status: accepted (18 Oct 2026, with Mateus's answers below); amended twice on 18 Oct 2026 (see "Amendments")
- Date: 2026-10-18
- Milestone: M2 (moved forward from M5)

## Context

After v0.1 (the viewer), Mateus asked for the plugin system to come first, before the editor, so every later decision is made with it in mind and nothing has to be reworked (18 Oct 2026):

- Only reading, editing, writing and the player's controls belong in the core. Everything else is a plugin.
- Anything that depends on a platform or environment (X11 or Wayland, an audio API) is a plugin. The app must not carry or load code it cannot use.
- Each file format is its own plugin (.gp3, .gp4, .gp5, .gpx, .gp, …); the core's reading and writing uses whatever is installed.
- One file per plugin, fast to load: no unzipping at start; disk space matters less than load time.
- Plugins are found in a configurable list of folders, by default the user's `~/.oxiguitar/plugins/` and the system's `/usr/lib/oxiguitar/plugins/` (and the equivalents elsewhere). The core ships alone; our own plugins are hosted on GitHub; anyone may host others.
- Plugins can add tabs to four areas of the window (top, bottom, left, right), sections to a toolbar, and implementations of exposed methods, so the right code can be chosen per platform and environment. They have access to the score.
- The core must pass Debian's acceptance criteria.

Until now the plan had the plugin system at M5, after the editor, the player and Guitar Pro 8 parity, and the core read and wrote Guitar Pro itself.

## Decision

The design in [docs/design/plugins.md](../design/plugins.md):

1. **Plugins are native libraries**, one `.oxp` file per plugin and target, with a versioned C interface that passes messages as bytes (audio as raw buffers), and a manifest in a named section that the host reads without loading the code. Code loads on first use and is never unloaded during a session.
2. **The core** keeps the model, editing with undo, engraving and the four layouts on screen, the window shell, the player's transport and event timeline, the mixer, settings, the plugin host and a catalogue of our own plugins. Formats, exports, tools, instruments, audio and MIDI devices, the plugin manager and platform-gap fillers are plugins. (Amended: PDF export and Infer Key Signatures stay in the core, and the core has no catalogue; see "Amendments".)
3. **The plugin system becomes M2**, before the editor. The readers of M1 move into five format plugins with their tests, and the M1 quality gates must still pass through them.

## Options for the plugin format

Native libraries, WebAssembly components, both kinds, separate processes, and scripts; compared in plugins.md ("The options considered"). Native libraries alone can do the platform work Mateus wants in plugins, load fastest and run DSP fastest, and add the least to the core. WebAssembly would sandbox plugins and run one file everywhere, but it cannot reach audio devices or the windowing system, so platform plugins would need their code back in the core, and it puts a compiler (Cranelift) in the core.

## Mateus's answers (18 Oct 2026)

1. **Native plugins, with the contained `unsafe`**: yes. The loader and the SDK's export glue are the only `unsafe` code we write; the rest of the workspace keeps `forbid`.
2. **Engraving in the core**: yes. PDF export, Infer Key Signatures and the other tools become plugins. (Amended at the end of the day: PDF export and Infer Key Signatures stay in the core; see "Amendments".)
3. **Mixer and speed in the core**: yes.
4. **Output per track** with a default and a separate audio-device service: yes. Output plugins also say which **classes of instrument** they play or don't; by default, everything (plugins.md, "Instrument classes").
5. **Debian**: whatever it takes for the packages to be hosted in Debian's own archive, in main, as MuseScore and TuxGuitar are; non-free if main had a hard blocker. Main is achievable: every licence is free, so the blockers are packaging work (plugins.md, "Debian"). Non-free wouldn't remove any of them.
6. **Folders**: the freedesktop XDG convention on Linux (`~/.config/oxiguitar/`, `~/.local/share/oxiguitar/plugins/`), to keep aligned with the community.
7. **Packages**: the core alone (`oxiguitar`) and `oxiguitar-plugins-basic`, the plugins we write and recommend, also fit for Debian's archive; the core lists it under **Suggests**.
8. **Plugin tabs**: whatever is fast, easy and elegant: standard widgets drawn by the core, plus a display-list canvas.
9. **Two Linux builds**: Wayland and X11, the X11 one deprecated from the start and dropped later.
10. **v1.0** = viewer, editor, player and plugins, with at least the Guitar Pro 7+ format plugin reading and writing, and a basic MIDI player.

(He also accepted the wheel zooming the page on touchpads too.)

## Consequences

- The milestones are renumbered (ROADMAP.md): M2 plugin system and window shell, M3 editor, M4 player = v1.0 (viewer, editor, player and plugins; at least the Guitar Pro 7+ format plugin and a basic MIDI player), M5 Guitar Pro 8 parity, M6 format plugins, M7 NAM, M8 RSE, M9 drums and hardware, M10 mobile.
- `oxiguitar-formats` becomes shared reader code used by the format plugins; `oxiguitar-cli` loads plugins like the app. (`oxiguitar-tools` and PDF export stay in the core: "Amendments".)
- Linux gets two builds of the core (Wayland, and a deprecated X11 one) and two packages (`oxiguitar`, `oxiguitar-plugins-basic`), each meant for Debian's archive.
- The brief's FMT-5 ("the core reads and writes Guitar Pro formats only"), PLAY-1 ("a built-in SoundFont synthesiser") and UI-7 ("no toolbar row") change accordingly.
- A plugin can crash the app; the plugin manager and the manual say which plugins are ours.
- Revisit if a third-party plugin ecosystem needs sandboxing: WebAssembly can be added as a second kind behind the same SDK traits.

## Amendments

### PDF export and Infer Key Signatures stay in the core (Mateus, 18 Oct 2026)

At the end of the day, after the plugin interface and the host were built (PRs #48 and #49), Mateus changed part of answer 2:

- **Infer Key Signatures stays in the core**, as a feature of the core editor. There is no plugin for it; `oxiguitar-tools` stays a core crate.
- **PDF export stays in the core**, as a feature of the main viewer. The core engraves already, so krilla and the fonts stay where they are.
- Other tools, and the other exports (PNG, SVG, WAV), are still plugins.
- **AppImage**: a core AppImage plus a zip file holding the core plugins we develop and recommend (the basic set), like the Windows zip and the .dmg. No plugins inside the AppImage. This answers the question ROADMAP.md left open for the packaging slice.

Consequences: M2's slice 2 ("PDF export and Infer Key Signatures as plugins") is dropped. The interface gains exporter and tool contributions only when a real exporter or tool plugin needs them, not in M2.

### No catalogue of formats in the core (Mateus, 18 Oct 2026)

Before slice 1 was built, Mateus decided how the core recognises a file:

- **With no plugin installed, the core refuses to open anything.** It has no detection of its own and no built-in catalogue of our plugins.
- **Each plugin identifies its own format reliably** (today: the signatures in its manifest), and the formats the installed plugins recognise make up the list of what the app can open and identify.
- **A file no installed plugin recognises is refused**, and the message says so. If an installed plugin declares the file's extension (a `.gp5` file the Guitar Pro 5 plugin doesn't recognise), it asks the user to open a bug report; otherwise, a feature request. Both go to the GitHub issue tracker.

This replaces "a catalogue of our own plugins" in item 2 of the decision, and the M1 rule that MuseScore, MusicXML, ABC and MIDI files are recognised and refused with a message naming the plugin they will need (16 Oct 2026). Guitar Pro 1–2 files, which no plugin reads, are refused like any other file no plugin recognises. Text formats (MusicXML, ABC, MuseScore's .mscx), which today's byte and ZIP-member signatures can't recognise reliably, will need a richer way for their plugins to identify them when those plugins come (M6).
