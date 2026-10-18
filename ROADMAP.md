# Roadmap

The milestones from the project brief, with live status. Requirement IDs (FMT-1, VIEW-3, …) refer to the brief's product requirements; decision records live in [docs/adr/](docs/adr/).

v1.0 ships at the end of M4: the viewer, the editor, the player and the plugin system, with at least the Guitar Pro 7+ format plugin (reading and writing) and a basic MIDI player (Mateus, 18 Oct 2026). From M5 on, the work closes the gap with Guitar Pro 8.

On 18 October 2026 Mateus moved the plugin system forward, from M5 to M2, so that every later milestone is built on it: the core reads, edits, writes, shows (with PDF export) and drives playback, and everything else (every file format, every sound, every other export and tool, anything platform-specific) is a plugin. See [ADR 0008](docs/adr/0008-plugin-architecture.md) and [docs/design/plugins.md](docs/design/plugins.md). Milestones after M1 were renumbered.

| Milestone | Release | Status |
| --- | --- | --- |
| M0 Foundations | — | **Done** (16 Oct 2026) |
| M1 Viewer | v0.1 | **Done** (17 Oct 2026; v0.1.0 published as a pre-release, 18 Oct 2026) |
| M2 Plugin system and window shell | v0.2 | **In progress** (design accepted, ADR 0008, amended 18 Oct 2026) |
| M3 Editor | v0.5 | Not started |
| M4 Player | **v1.0** | Not started |
| M5 Guitar Pro 8 notation and editing parity | | Not started |
| M6 Format plugins: MusicXML, ABC, MuseScore, MIDI files | | Not started |
| M7 Realistic guitar through NAM (plugin) | | Not started |
| M8 Guitar Pro RSE compatibility (plugin) | | Not started |
| M9 Realistic drums, hardware plugins, remaining Guitar Pro 8 features | | Not started |
| M10 Android and iPad companion | | Not started |

## M0 — Foundations

Set up everything the later milestones stand on, and settle the open technical choices with small experiments.

- [x] Cargo workspace, formatting and lint rules, LICENSE, CREDITS.md, CONTRIBUTING.md with DCO sign-off
- [x] CI on Windows, macOS and Linux, x86_64 and ARM64; cargo-deny licence check; no-network test (NET-5); `ci-success` gate
- [x] Test builds of every push to `develop` for all targets
- [x] Score model, first version (ADR 0002)
- [x] Evaluate the guitarpro crate on the corpus: adopt, fork or write our own (ADR 0003: write our own)
- [x] Format detection from file contents (FMT-6) and a GP3/GP4/GP5 reader with import log
- [x] `oxiguitar-cli info` prints the structure of any GP5 file
- [x] Corpus manifest: names, formats, sources and checksums (per-file source URLs still to collect)
- [x] Public fixture set of self-made and freely licensed files (GP3/4/5 test songs; each new reader adds its own in M1)
- [x] Placeholder app icon: a green guitar pick with "OxG"
- [x] Licences marked "confirm" in the brief checked before any code is copied (see below)
- [x] GUI spike: the same small screen (palette, track list, score canvas) in Slint, iced and egui (ADR 0004: Slint)
- [x] Render spike: SMuFL glyphs and tab numbers on screen and in a PDF from one display list (ADR 0005)

Done when:

- [x] Decision records exist for GUI toolkit (0004), renderer (0005), model (0002) and file-reader strategy (0003)
- [x] `oxiguitar-cli` prints the structure of any GP5 file (all 27 GP5 files in the corpus)
- [x] CI is green on all three systems (Windows, macOS, Linux; x86_64 and ARM64)

Licences confirmed on 15 October 2026, from each package's own metadata and licence files:

| Project | Version | Licence | Note |
| --- | --- | --- | --- |
| PyGuitarPro | 0.11 | LGPL-3.0-only | Reference only; ported code would keep LGPL-3.0 notices |
| musicxml | 1.1.2 | MIT | Last release November 2024 |
| abc-parser | 0.4.0 | MIT | |
| krilla | 0.8.2 | MIT OR Apache-2.0 | Its NOTICE credits MPL-2.0 and Apache-2.0 snippets; all GPL-3.0-compatible |
| cpal | 0.18.2 | Apache-2.0 | |
| midir | 0.11.1 | MIT | |
| zip | 9.0.3 | MIT | Use without default features: they build the zstd C library |
| quick-xml | 0.42.0 | MIT | |

## M1 — Viewer (v0.1)

Open every Guitar Pro file and show it beautifully.

- Readers for Guitar Pro 5 onwards (.gp5, .gpx, .gp), with import logs; GP3 and GP4 if cheap. The core reads and writes Guitar Pro formats only. MuseScore, MusicXML, ABC and MIDI files are recognised (FMT-6) and refused with a clear message naming the plugin they will need (decided 16 Oct 2026; now M6).
- Engraving for standard notation, tablature and rhythm-on-tab, with the core effects drawn (VIEW-1, VIEW-2, VIEW-7, NOT-1 to NOT-4).
- Page, vertical, horizontal and parchment layouts, multi-track display, zoom, score header (VIEW-3 to VIEW-6).
- The desktop window (`oxiguitar-app`, Slint) following [docs/design/ui.md](docs/design/ui.md): menus only on top, collapsible palette groups, interface zoom by keys and wheel, score-zoom slider, dark page (UI-7 to UI-10).
- PDF export (EXP-1). User manual on GitHub Pages. Release packages (release.yml).

Done when: 98% of the Guitar Pro 5-and-later files in the corpus open without error, the rest with clear messages (MuseScore, MusicXML and ABC files are counted separately and do not count towards this); 6 hours of fuzzing per reader without a crash (lowered from 24 hours by Mateus, 16 Oct 2026); a 300-bar, 8-track file opens in under a second; visual regression tests cover 50 reference scores, generated and kept in the repository (the private corpus is checked locally only); Mateus has reviewed PDFs of 10 songs.

Progress:

- [x] Guitar Pro 6 (.gpx) and 7/8 (.gp) readers, with import logs and public fixtures (16 Oct 2026). Every GPX and GP file in the corpus opens; alphaTab reads the same notes from each, bar for bar; six songs saved in two formats agree (`--test corpus`).
- [ ] Engraving into a display list (`oxiguitar-layout`): standard notation, tablature, rhythm on tab, effects
  - [x] First engraving (16 Oct 2026): notes, rests, chords, accidentals, beams, flags, dots, ties, tuplets, signatures and key changes, repeats, voltas, sections, tempo, chord names, text; tab sharing columns with the staff; rhythm on tab-only tracks; drum staff. `oxiguitar-cli render` writes PNG and PDF. Every Guitar Pro file in the corpus engraves with all its tracks.
  - [x] Effects (TAB-1), grace notes, lyrics, dynamics, articulations, chord diagrams in the header, text font fallback, overlap room for tuplets and articulations (16 Oct 2026)
  - [x] Dynamics can be hidden (16 Oct 2026)
  - [ ] Slurs, hairpins, ornaments, fermatas and ottavas once the score model holds them (NOT-3); whammy curves; fingering; second overlap pass (VIEW-7); courtesy signatures
- [x] Page, vertical, horizontal and parchment layouts; multi-track; score header
  - [x] Page layout, multi-track alignment, score header (16 Oct 2026)
  - [x] Track names at the start of systems (16 Oct 2026)
  - [x] Vertical, horizontal and parchment layouts, checked against the Guitar Pro 8 user guide (16 Oct 2026)
- [x] `oxiguitar-app` window (Slint) following docs/design/ui.md, with a View toggle for dynamics and the Tools menu (16 Oct 2026): File → Open, tiled canvas on a worker thread, the four layouts, Dark Page, score and interface zoom, track list, status line
- [x] `oxiguitar-tools`: the Tools menu's tools, one shape for all so plugins can add more (M2); first tool: Infer Key Signatures, with key changes (16 Oct 2026)
- [x] PDF export of the shown tracks, A4 or Letter (16 Oct 2026)
- [ ] Quality gates (see [docs/testing.md](docs/testing.md))
  - [x] Speed budget (16 Oct 2026): a generated 300-bar, 8-track score as long as a real Guitar Pro 7 file (16 MB of XML) opens to its first tiles in 0.18 s, or 0.23 s with every track shown; CI's "Speed budget" job holds release builds to one second. Reading GPIF is about twice as fast as before.
  - [x] Visual regression (16 Oct 2026): 50 generated reference scores in `fixtures/reference/`, each a Guitar Pro 7 file and its pictures, compared with a tolerance on every CI target
  - [x] Fuzzing (16 Oct 2026): six hours per reader without a crash, hang or out-of-memory input ([run 38891529443](https://github.com/mateusrdp/oxiguitar/actions/runs/38891529443)): Guitar Pro 3–5 1.26 billion inputs, Guitar Pro 6 335 million, Guitar Pro 7/8 272 million, GPIF XML 37 million. Short runs on every pull request that touches a reader
- [x] Release packages (16 Oct 2026): `release.yml` builds the .exe zip, AppImage, .deb, .rpm (x86_64 and ARM64) and universal .dmg, unsigned, on v* tags and by hand; the .deb, .rpm and .dmg register the Guitar Pro file types (UI-4)
- [x] User manual (16 Oct 2026): `docs/manual/` (mdBook), published to GitHub Pages from main by the "Manual" workflow

Corpus status (16 Oct 2026): Guitar Pro 5 and later, 42 of 42 files open (27 GP5, 3 GPX, 12 GP). Counted separately, for the format plugins (M6): 5 MuseScore and 1 MusicXML file; no ABC yet.

## M2 — Plugin system and window shell (v0.2)

Turn the viewer into a small core with plugins, and give the window the shape the later milestones fill in. Design: [docs/design/plugins.md](docs/design/plugins.md); decision: [ADR 0008](docs/adr/0008-plugin-architecture.md) (accepted 18 Oct 2026; amended the same day: PDF export and Infer Key Signatures stay in the core, and the core recognises no format itself).

- [x] The window shell (docs/design/ui.md, PR #43): toolbar with sections, tabbed top, bottom, left and right areas that collapse and hide, the page on a dark canvas, a one-line footer that expands into the session's messages; player controls greyed out in the top area until an output plugin is installed; mouse wheel zooms the page, Ctrl + wheel the interface.
- [x] User folder, settings.json and the plugin folders (XDG on Linux), created on launch (PR #43); the footer says when no output or no read/write plugin is installed and where to get them.
- [x] Mateus answers ADR 0008's questions; ADR 0008 accepted (18 Oct 2026).
- [x] `oxiguitar-plugin-api` (the interface's messages), `oxiguitar-plugin` (the SDK for plugin authors, no `unsafe` for them), the host in `oxiguitar-host` (manifests read without loading, lazy loading, contributions registry, host services, services chosen per platform and environment). PRs #48 and #49.
- [x] A test plugin built as an `.oxp` and the contract suite (`oxiguitar-plugin-test`); 30 manifests read in about 1 ms.
- [x] Five format plugins from M1's readers: Guitar Pro 3, 4, 5, 6 (.gpx), 7–8 (.gp), each with its fixtures, corpus, fuzz and visual tests. The core holds no format code and recognises no format itself; `oxiguitar-cli` loads plugins like the app (PR #51, 18 Oct 2026). Every M1 gate passes through the `.oxp` files: 42 of 42 corpus files; all 159 Guitar Pro files read byte for byte as develop's in-process reader read them; the 50 visual references unchanged; the speed budget with the plugin loaded (0.26 s and 0.32 s for the first runs); the contract on all five; NET-5 with the plugins installed; a fuzz target for the plugins' dispatch.
- [x] ~~Plugins for PDF export and for Infer Key Signatures~~: dropped. Both stay in the core, PDF export as a feature of the viewer and Infer Key Signatures as a feature of the editor (Mateus, 18 Oct 2026).
- [ ] Plugin tabs (standard widgets and a display-list canvas) and events pushed to plugins; the plugin manager in the core (Tools → Plugins: list, enable, disable); `--safe-mode`; the plugin folders in Help → About. Built in session 11 (interface 1.1 in #53, the window's tabs and score events, the manager); left to tick: Mateus's check on sid.
- [ ] Plugins built for all six targets in CI, attached to releases with SHA256SUMS; two packages, `oxiguitar` (the core) and `oxiguitar-plugins-basic` (Suggested by the core); the AppImage, the Windows zip and the .dmg each beside a zip of the basic plugins.
- [x] Two Linux builds of the core: Wayland (default) and X11 (deprecated), as separate packages and AppImages (session 11): each holds only its own display system's code (checked in CI), with no exception (Slint pinned to 1.17.1 for a Wayland-only clipboard, ADR 0009); both are packaged, installed and started in CI.
- [ ] The plugin developer guide (`docs/plugins/`), with a template plugin, and the interface's contract test suite.
- [ ] Debian: manual pages for both binaries, lintian in CI, a `debian/` directory building both packages offline in a sid container, and an audit of every crate against Debian's archive (plugins.md, "Debian"). The goal is Debian main.

Done when: the core with no plugins opens nothing and says so, and a file no installed plugin recognises is refused with a request for a feature (or for a bug report, when an installed plugin declares its extension); with our plugins, every M1 gate still passes (42 of 42 corpus files, the speed budget including plugin loading, the 50 visual references unchanged, six hours of fuzzing per reader); start-up reads 30 plugin manifests in under 10 ms; the plugin manager works on all three systems; the developer guide builds a working plugin from the template.

### M2's remaining slices, in order (planned 18 Oct 2026)

Each slice is one or more pull requests into develop, with the same gates as M1. Mateus set the order.

1. **The five Guitar Pro readers as format plugins.** Done (PR #51); the six-hour fuzzing run per reader by hand is the one gate left.
   - `plugins/gp3`, `gp4`, `gp5`, `gpx` and `gp` (ids `oxiguitar.gp3` …, files `gp3.oxp` …). Each is a thin crate over `oxiguitar-formats`, which keeps the shared reader code and gains one Cargo feature per reader, so each plugin carries only its own. Manifests carry today's detection as signatures: `FICHIER GUITAR PRO v3/v4/v5` at offset 1, `BCFZ`/`BCFS` at 0, the ZIP member `Content/score.gpif`.
   - No catalogue and no detection in the core (Mateus, 18 Oct 2026): the formats the installed plugins recognise are what the app opens. With no plugin installed it opens nothing. A file no plugin recognises is refused: a bug report is asked for when an installed plugin declares the file's extension, a feature request otherwise (both at the issue tracker). Guitar Pro 1–2 files are refused that way too.
   - The app and `oxiguitar-cli` open files through `Plugins` (wire score → model); `BUILT_IN_READERS` goes away.
   - Development builds find the plugins in `target/lib/oxiguitar/plugins/`, the program's system plugin folder, via `scripts/build-plugins.sh`.
   - Until slice 5 split the packages, every package installed the five plugins in its system plugin folder.
   - Gates, all through the `.oxp` files:
     - the corpus (42 of 42), and the reader-change `cmp` check: `{:#?}` of score and log from develop's in-process reader against the plugin path, byte for byte, for every GP file;
     - the 50 visual references unchanged;
     - the speed budget including loading the plugin (300 bars under 1 s; 30 manifests under 10 ms);
     - fuzzing: today's targets keep fuzzing the reader code, a new one fuzzes the plugin's `dispatch` (wire conversion included), and the 6-hour run per reader by hand;
     - NET-5 with the plugins installed;
     - the contract suite on all five.
   - CI stops using `--all-features`, since two plugin crates linked into one test binary would both export the entry symbol; the features it needs are named instead.
2. ~~PDF export and Infer Key Signatures as plugins.~~ **Dropped** (Mateus, 18 Oct 2026): PDF export stays in the core as a feature of the viewer, and Infer Key Signatures as a feature of the editor. Other tools and other exports (PNG, SVG, WAV) are still plugins; the interface gains exporter and tool contributions when the first such plugin needs them. The versioned wire form of the display list comes with slice 3, for plugin tabs' canvases (ADR 0008, answer 8).
3. **Plugin tabs and the plugin manager.**
   - First the interface for tabs: standard widgets the core draws in its own style, plus a canvas the plugin draws with a versioned wire form of the display list and receives pointer events on. Then events pushed to plugins: score opened or changed, selection, playback position. No polling.
   - Then the plugin manager, **in the core** (Mateus, 18 Oct 2026; ADR 0008, "Amendments"): Tools → Plugins opens a separate, non-modal window listing every plugin file in the plugin folders (name, version, a tickbox to enable or disable it from the next start, its folder, loaded or not and why not). It copies, downloads, installs and removes nothing, draws no settings forms, and makes no distinction between our plugins and others'.
   - In the core: `--safe-mode` (no plugins at all) and the plugin folders in Help → About.
   - Interface additions only at the ends of enums and tables, as minor versions.
   - Gates: the contract suite covers tabs, widgets, canvas and events; the manager works on all three systems (headless on Linux; contract tests everywhere; Mateus on sid); every existing gate stays green; no new `unsafe`; no polling.
4. **Two Linux builds.** Built in session 11 (18 Oct 2026). Slint's `unstable-winit-030` turns on both display systems, so the app takes `i-slint-backend-winit` directly; Wayland first (Mateus, 19 Oct 2026): Slint is pinned to 1.17.1, whose Wayland clipboard uses the window's own connection, so the Wayland build holds no X11 crate (ADR 0009).
   - `oxiguitar-app` features `wayland` (default) and `x11`, each enabling only Slint's `backend-winit-wayland` or `backend-winit-x11`; `display_system()` then follows the build.
   - Packages `oxiguitar` and `oxiguitar-x11` (which Provides, Conflicts with and Replaces `oxiguitar`), and two AppImages; the manual says the X11 build will be dropped.
   - CI checks that the Wayland build holds no X11 crate and the reverse, and runs the Wayland build's headless tests in the nested compositor (`scripts/wayland-dnd-test/`).
5. **Packages and Debian.**
   - Done: releases attach every plugin for all six targets (`oxiguitar-gp5-0.2.0-linux-x86_64.oxp` …) with SHA256SUMS.
   - Done: `oxiguitar-plugins-basic` (.deb and .rpm, into `/usr/lib/oxiguitar/plugins/`), which the core Suggests; the AppImage, the Windows zip and the .dmg each come as two downloads, the core and a zip of the basic plugins (Mateus, 18 Oct 2026: no plugins inside the AppImage).
   - Done: the audit of every crate against Debian's archive, weekly (`docs/debian.md`; 19 Oct 2026: 276 of 424 at our version, 28 missing, chiefly Slint 1.17.1 itself and krilla).
   - Done: manual pages for `oxiguitar` and `oxiguitar-cli` (`assets/linux/man/`), in the .deb and .rpm; lintian from sid on every .deb in the release workflow, errors and warnings failing it (only the two tags about the issue-tracker contact suppressed).
   - The rest of the Debian work towards main: a `debian/` directory (dh-cargo) building both packages offline in a sid container, and `fonts-noto-core` instead of our Noto copy.

The plugin developer guide (`docs/plugins/`) and its template plugin come after slice 3, when the interface has every M2 contribution, and before v0.2 freezes interface 1.0.

## M3 — Editor (v0.5)

- M3a: note entry on tab and staff, the full core palette (left area), the inspector (right area), 4 voices, tracks and tunings, undo and redo, copy and paste, bar and signature changes, repeats, transposition, tuplets of any grouping with a triplet shortcut and "Other…" (EDIT-1 to EDIT-10); the .gp writer, in the GP 7–8 format plugin (the default save format).
- M3b: the .gp5 writer in the GP5 plugin, with loss warnings (FMT-3).

Done when: the TuxGuitar parity checklist works; every edit undoes and redoes; round-trip tests pass for every writable format; files we write open correctly in Guitar Pro 8; transposition works chromatically and diatonically.

## M4 — Player (v1.0)

- In the core: score-to-event conversion with repeats, jumps, tempo changes and feel (PLAY-3, PLAY-4); the transport in the top area (play, play selection, stop, pause, rewind, count-in, loop, audible and visual metronome, PLAY-5); the follow cursor; the mixer (PLAY-6); the output choice.
- As plugins: the MIDI instrument (a SoundFont synthesiser, PLAY-1, with downloads of better SoundFonts at the user's request, PLAY-8), audio-device plugins per system, WAV export (PLAY-7). Instruments declare the classes of instrument they play.
- v1.0's minimum: the Guitar Pro 7+ format plugin reading and writing, and the basic MIDI player, in `oxiguitar-plugins-basic`.

Done when: playback matches the written effects by ear; playback starts in under 100 ms and stays in sync over 10 minutes; golden-file tests cover event output; the practice tools work on every reference song; with no instrument installed the transport is greyed out and the footer says where to get one.

## Later milestones

- **M5** — the full Guitar Pro 8 notation set (TAB-2), its editing tools, stylesheets and design mode, per-note playback settings.
- **M6** — format plugins, each reading and writing, one plugin per format: MusicXML (.musicxml, .xml, .mxl), ABC, MuseScore (.mscz, .mscx) and MIDI files, with tab derivation for staves without tablature (TAB-3, TAB-4). Their sample files are counted apart from the Guitar Pro corpus (decided 16 Oct 2026).
- **M7** — a NAM instrument plugin: a clean guitar signal (a plucked-string model, or sampled DI notes if freely licensed ones exist) through Neural Amp Modeler and cabinet impulse responses. How the guitar signal is made is still open.
- **M8** — research spike and go/no-go on reading users' own Guitar Pro RSE soundbanks, as an instrument plugin.
- **M9** — a realistic drums instrument plugin (with a research spike on Singular Sound BeatBuddy content, used only from the user's own copy), audio tracks, advanced practice tools, line-in, PNG/SVG export, external MIDI devices.
- **M10** — a simpler viewer and player for Android and iPad, on the same core, with its plugins built in.

Unscheduled, late nice-to-have: screen-reader support (UI-11). Keyboard access is required from M1.
