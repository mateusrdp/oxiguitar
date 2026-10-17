# 0008 — A small core; formats, sound, exports and platform code as plugins

- Status: proposed (direction accepted by Mateus on 18 Oct 2026; the open questions below decide the details)
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

## Decision (proposed)

The design in [docs/design/plugins.md](../design/plugins.md):

1. **Plugins are native libraries**, one `.oxp` file per plugin and target, with a versioned C interface that passes messages as bytes (audio as raw buffers), and a manifest in a named section that the host reads without loading the code. Code loads on first use and is never unloaded during a session.
2. **The core** keeps the model, editing with undo, engraving and the four layouts on screen, the window shell, the player's transport and event timeline, the mixer, settings, the plugin host and a catalogue of our own plugins. Formats, exports, tools, instruments, audio and MIDI devices, the plugin manager and platform-gap fillers are plugins.
3. **The plugin system becomes M2**, before the editor. The readers of M1 move into five format plugins with their tests, and the M1 quality gates must still pass through them.

## Options for the plugin format

Native libraries, WebAssembly components, both kinds, separate processes, and scripts; compared in plugins.md ("The options considered"). Native libraries alone can do the platform work Mateus wants in plugins, load fastest and run DSP fastest, and add the least to the core. WebAssembly would sandbox plugins and run one file everywhere, but it cannot reach audio devices or the windowing system, so platform plugins would need their code back in the core, and it puts a compiler (Cranelift) in the core.

## Open questions for Mateus

1. **Plugin format.** Native libraries (proposed), WebAssembly, or both? Native plugins need `unsafe` code in exactly two small places (the host's loader and the SDK's export glue): loading a library and calling through function pointers is `unsafe` in Rust however it is done. This is not a workaround around another library, unlike ADRs 0006 and 0007, but it does break CONTRIBUTING.md's "No `unsafe`". Accept it, contained and reviewed, or choose WebAssembly and give up platform plugins?
2. **Engraving in the core?** Proposed: yes, the four layouts and the screen drawing, since editing needs them. PDF export, Infer Key Signatures and the other tools become plugins.
3. **Mixer and speed in the core?** Your list for the core player is play, play selection, stop, pause, rewind, count-in, loop and metronome. Proposed: also per-track volume, pan, mute and solo, and playback speed (25–200 %, PLAY-5), because instruments from different plugins have to be mixed somewhere.
4. **Output per track?** Proposed: a default instrument plus a choice per track (drums through the drum plugin, a guitar through NAM, the rest through MIDI), and a separate audio-device service, so several instruments share one sound card.
5. **Which Debian criteria?** (a) DFSG-free licences, Debian Policy and a lintian-clean .deb (we can meet this in M2: manual pages and a lintian job are what's missing), or (b) entering Debian's own archive, which also needs every crate packaged in Debian (Slint is only partly there) and fonts from Debian packages (we found no Bravura package), plus a Debian developer to sponsor it.
6. **Folders on Linux.** `~/.oxiguitar/` as you asked (what the app does now), or the freedesktop convention `~/.config/oxiguitar/` and `~/.local/share/oxiguitar/plugins/`? And the system folder: you wrote both `/usr/lib/oxiguitar/plugins/` and `/usr/lib/oxiguitar/lib/plugins/`; the first is assumed.
7. **Packages.** Ship the core alone, so a fresh install opens nothing until the user downloads plugins? Or split each package into `oxiguitar` (the core) and `oxiguitar-plugins` (ours, *Recommended* on Debian), so it works out of the box while the core stays the core?
8. **Plugin tabs.** Standard widgets plus a canvas drawn with our display list (proposed: works on mobile too, keeps one look), or Slint markup run by Slint's interpreter (more freedom, a compiler in the core, desktop only)?
9. **The windowing toolkit exception.** Slint and winit carry both X11 and Wayland in the Linux binary. Accept that, or ship two Linux builds of the core?
10. **v1.0.** Still the editor plus the MIDI player (now M4), or something else?

## Consequences

- The milestones are renumbered (ROADMAP.md): M2 plugin system and window shell, M3 editor, M4 player = v1.0, M5 Guitar Pro 8 parity, M6 format plugins, M7 NAM, M8 RSE, M9 drums and hardware, M10 mobile.
- `oxiguitar-formats` becomes shared reader code used by the format plugins; `oxiguitar-tools` and PDF export move into plugins; `oxiguitar-cli` loads plugins like the app.
- The brief's FMT-5 ("the core reads and writes Guitar Pro formats only"), PLAY-1 ("a built-in SoundFont synthesiser") and UI-7 ("no toolbar row") change accordingly.
- A plugin can crash the app; the plugin manager and the manual say which plugins are ours.
- Revisit if a third-party plugin ecosystem needs sandboxing: WebAssembly can be added as a second kind behind the same SDK traits.
