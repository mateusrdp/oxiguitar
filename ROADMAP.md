# Roadmap

The milestones from the project brief, with live status. Requirement IDs (FMT-1, VIEW-3, …) refer to the brief's product requirements; decision records live in [docs/adr/](docs/adr/).

v1.0 ships at the end of M4: the viewer, the editor, the player and the plugin system, with at least the Guitar Pro 7+ format plugin (reading and writing) and a basic MIDI player (Mateus, 18 Oct 2026), and the NAM plugin (Mateus, 25 Oct 2026). From M5 on, the work closes the gap with Guitar Pro 8.

On 18 October 2026 Mateus moved the plugin system forward, from M5 to M2, so that every later milestone is built on it: the core reads, edits, writes, shows (with PDF export) and drives playback, and everything else (every file format, every sound, every other export and tool, anything platform-specific) is a plugin. See [ADR 0008](docs/adr/0008-plugin-architecture.md) and [docs/design/plugins.md](docs/design/plugins.md). Milestones after M1 were renumbered.

| Milestone | Release | Status |
| --- | --- | --- |
| M0 Foundations | — | **Done** (16 Oct 2026) |
| M1 Viewer | v0.1 | **Done** (17 Oct 2026; v0.1.0 published as a pre-release, 18 Oct 2026) |
| M2 Plugin system and window shell | v0.2 | **Done** (19 Oct 2026; v0.2.0 published as a pre-release, 19 Oct 2026) |
| M3 Editor | v0.5 | **Done** (21 Oct 2026; v0.5.0 published as a pre-release, 21 Oct 2026) |
| M4 Player (with the NAM plugin) | **v1.0** (v0.9 for testers) | Slices 1–4 built (21–26 Oct 2026): it plays in the window; **v0.9.0 for testers published** (pre-release, 26 Oct 2026); slices 5–7 (SFZ, NAM, WAV export) next |
| M5 Guitar Pro 8 notation and editing parity | | Not started |
| M6 Format plugins: MusicXML, ABC, MuseScore, MIDI files | | Not started |
| M7 A generated guitar for NAM (plugin) | | Not started |
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

- [x] The window shell (docs/design/ui.md, PR #43): toolbar with sections, tabbed top, bottom, left and right areas that collapse and hide, the page on a dark canvas, a one-line footer that expands into the session's messages; player controls greyed out in the top area until an output plugin is installed; the mouse wheel scrolls the page and Shift + wheel zooms it (#57).
- [x] User folder, settings.json and the plugin folders (XDG on Linux), created on launch (PR #43); the footer says when no output or no read/write plugin is installed and where to get them.
- [x] Mateus answers ADR 0008's questions; ADR 0008 accepted (18 Oct 2026).
- [x] `oxiguitar-plugin-api` (the interface's messages), `oxiguitar-plugin` (the SDK for plugin authors, no `unsafe` for them), the host in `oxiguitar-host` (manifests read without loading, lazy loading, contributions registry, host services, services chosen per platform and environment). PRs #48 and #49.
- [x] A test plugin built as an `.oxp` and the contract suite (`oxiguitar-plugin-test`); 30 manifests read in about 1 ms.
- [x] Five format plugins from M1's readers: Guitar Pro 3, 4, 5, 6 (.gpx), 7–8 (.gp), each with its fixtures, corpus, fuzz and visual tests. The core holds no format code and recognises no format itself; `oxiguitar-cli` loads plugins like the app (PR #51, 18 Oct 2026). Every M1 gate passes through the `.oxp` files: 42 of 42 corpus files; all 159 Guitar Pro files read byte for byte as develop's in-process reader read them; the 50 visual references unchanged; the speed budget with the plugin loaded (0.26 s and 0.32 s for the first runs); the contract on all five; NET-5 with the plugins installed; a fuzz target for the plugins' dispatch.
- [x] ~~Plugins for PDF export and for Infer Key Signatures~~: dropped. Both stay in the core, PDF export as a feature of the viewer and Infer Key Signatures as a feature of the editor (Mateus, 18 Oct 2026).
- [x] Plugin tabs (standard widgets and a display-list canvas) and events pushed to plugins; the plugin manager in the core (Tools → Plugins: list, enable, disable); `--safe-mode`; the plugin folders in Help → About. Built in session 11 (interface 1.1 in #53, the window's tabs and score events, the manager); left to tick: Mateus's check on sid.
- [x] Plugins built for all six targets in CI, attached to releases with SHA256SUMS; two packages, `oxiguitar` (the core) and `oxiguitar-plugins-basic` (Suggested by the core); the AppImage, the Windows zip and the .dmg each beside a zip of the basic plugins.
- [x] Two Linux builds of the core: Wayland (default) and X11 (deprecated), as separate packages and AppImages (session 11): each holds only its own display system's code (checked in CI), with no exception (Slint pinned to 1.17.1 for a Wayland-only clipboard, ADR 0009); both are packaged, installed and started in CI.
- [x] The window shows what's coming (Mateus, 19 Oct 2026; docs/design/ui.md): the left area holds the editor's palette, every tool as its icon in Guitar Pro 8's groups, greyed out until M3; the Mixer tab goes, and each Tracks row gains the mixer's controls (volume slider, pan knob, mute and solo icon toggles), greyed out until the player. For the people starting to follow the project, so the window shows where it is going. Built in session 12 (PR #67): 132 tools in GP8's six groups, every icon drawn by our renderer; checked by Mateus on 19 Oct 2026.
- [x] The plugin developer guide (`docs/plugins/`), with a template plugin, and the interface's contract test suite. Built in session 13 (19 Oct 2026, #68 and #69): the guide (an mdBook beside the manual, its examples included from the template's code), the template plugin (`docs/plugins/template/`: a format, a tab with a canvas, events, settings, a service), and CI building a plugin from it the guide's way on three systems, contract suite included; Mateus's answers on the interface points it raised, built.
- [x] Debian: manual pages for both binaries, lintian in CI, a `debian/` directory building both packages offline in a sid container, and an audit of every crate against Debian's archive (plugins.md, "Debian"). The goal is Debian main.

Done when: the core with no plugins opens nothing and says so, and a file no installed plugin recognises is refused with a request for a feature (or for a bug report, when an installed plugin declares its extension); with our plugins, every M1 gate still passes (42 of 42 corpus files, the speed budget including plugin loading, the 50 visual references unchanged, six hours of fuzzing per reader); start-up reads 30 plugin manifests in under 10 ms; the plugin manager works on all three systems; the developer guide builds a working plugin from the template.

### M2's remaining slices, in order (planned 18 Oct 2026)

Each slice is one or more pull requests into develop, with the same gates as M1. Mateus set the order.

1. **The five Guitar Pro readers as format plugins.** Done (PR #51), every gate met: the six-hour fuzzing run per reader found nothing (run 39105892287, `docs/testing.md`).
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
3. **Plugin tabs and the plugin manager.** Done (#52–#57, 18 Oct 2026); Mateus checked it on sid on 19 Oct 2026.
   - First the interface for tabs: standard widgets the core draws in its own style, plus a canvas the plugin draws with a versioned wire form of the display list and receives pointer events on. Then events pushed to plugins: score opened or changed, selection, playback position. No polling.
   - Then the plugin manager, **in the core** (Mateus, 18 Oct 2026; ADR 0008, "Amendments"): Tools → Plugins opens a separate, non-modal window listing every plugin file in the plugin folders (name, version, a tickbox to enable or disable it from the next start, its folder, loaded or not and why not). It copies, downloads, installs and removes nothing, draws no settings forms, and makes no distinction between our plugins and others'.
   - In the core: `--safe-mode` (no plugins at all) and the plugin folders in Help → About.
   - Interface additions only at the ends of enums and tables, as minor versions.
   - Gates: the contract suite covers tabs, widgets, canvas and events; the manager works on all three systems (headless on Linux; contract tests everywhere; Mateus on sid); every existing gate stays green; no new `unsafe`; no polling.
4. **Two Linux builds.** Built in session 11 (18 Oct 2026). Slint's `unstable-winit-030` turns on both display systems, so the app takes `i-slint-backend-winit` directly; Wayland first (Mateus, 19 Oct 2026): Slint is pinned to 1.17.1, whose Wayland clipboard uses the window's own connection, so the Wayland build holds no X11 crate (ADR 0009).
   - `oxiguitar-app` features `wayland` (default) and `x11`, each enabling only Slint's `backend-winit-wayland` or `backend-winit-x11`; `display_system()` then follows the build.
   - Packages `oxiguitar` and `oxiguitar-x11` (which Provides, Conflicts with and Replaces `oxiguitar`), and two AppImages; the manual says the X11 build will be dropped.
   - CI checks that the Wayland build holds no X11 crate and the reverse, and runs the Wayland build's headless tests in the nested compositor (`scripts/wayland-dnd-test/`).
5. **Packages and Debian.** Done (#59–#63, 19 Oct 2026).
   - Done: releases attach every plugin for all six targets (`oxiguitar-gp5-0.2.0-linux-x86_64.oxp` …) with SHA256SUMS.
   - Done: `oxiguitar-plugins-basic` (.deb and .rpm, into `/usr/lib/oxiguitar/plugins/`), which the core Suggests; the AppImage, the Windows zip and the .dmg each come as two downloads, the core and a zip of the basic plugins (Mateus, 18 Oct 2026: no plugins inside the AppImage).
   - Done: the audit of every crate against Debian's archive, weekly (`docs/debian.md`; 19 Oct 2026: 276 of 424 at our version, 28 missing, chiefly Slint 1.17.1 itself and krilla).
   - Done: manual pages for `oxiguitar` and `oxiguitar-cli` (`assets/linux/man/`), in the .deb and .rpm; lintian from sid on every .deb in the release workflow, errors and warnings failing it (only the two tags about the issue-tracker contact suppressed).
   - Done: `debian/` (dh-cargo) building both packages offline in a sid container, lintian-clean, with Noto Sans from `fonts-noto-core` (`docs/debian.md`). Its crates come from a vendor component until Debian packages them all: chiefly Slint 1.17.1 and krilla, which the weekly audit follows.
6. **The window shows what's coming** (Mateus, 19 Oct 2026). The editor's palette in the left area, greyed out, which then starts open; the Mixer tab removed and the mixer's controls in each Tracks row, greyed out. Gates: the visual and speed checks, headless screenshots of both themes, Mateus on sid. Done (PR #67); Mateus checked it on 19 Oct 2026.
   - The palette (`palette.rs`, `icons.rs`): Guitar Pro 8's groups and tools as its user guide lists them (voices and modes 8, bar 16, note 34, effects 56, notation 13, automation 5), each icon a Bravura symbol, a label or a small drawing, measured by its ink and centred, measured on a thread of its own at start; groups collapse and expand. Tests: every icon inside and centred at four sizes; every Bravura symbol checked against the metadata's box for its name.
   - Each Tracks row (Mateus, 19 Oct 2026): the name; the mixer (mute and solo toggles drawn by our renderer, volume slider, pan knob), starting at the file's values, greyed out until an output plugin is installed; TAB and standard notation; the track across the song like a waveform, with a placeholder for the playback position.
7. **The plugin developer guide** (`docs/plugins/`) and its template plugin, before v0.2 freezes interface 1.0. Done (#68, #69).
   - The guide: an mdBook, the user manual's sister book, published with it at `/plugin-guide/` (`.github/workflows/docs.yml`). Fourteen pages, from what a plugin is to building for every system and where users put the file; every code example is included from the template (mdBook anchors), so it is code CI builds.
   - The template (`docs/plugins/template/`): a workspace of its own that takes the SDK and the contract suite from this repository at the release's tag (git dependencies; nothing on crates.io, Mateus, 19 Oct 2026); a format read and written, a Statistics tab in the right area (widgets, a small chart canvas, pointer events, score events, the open score, a setting; it replaced a Fretboard tab Mateus found too big), a service; `cargo test` runs the contract suite on its `.oxp` file; a GitHub workflow for the five files of the six targets and a release.
   - Gate (M2's "the developer guide builds a working plugin from the template"): `scripts/check-plugin-template.sh` in CI on Linux, Windows and macOS, part of `ci-success`: the template copied out of the repository, fmt, clippy, its tests, the contract suite, and `oxiguitar-cli` opening its sample through the plugin file.
   - The interface points the guide raised, answered by Mateus on 19 Oct 2026: nothing is published on crates.io (plugin authors use git dependencies on the release tag); `instruments` stays in 1.0, ignored by the player for plugins older than the instrument interface (M4); a failing plugin must leave everything as if it had never been loaded (built: the plugin is withdrawn, its files go to the next plugin, its services are chosen again, its tabs leave the window); the footer shows only errors and important warnings, not import logs; bars that don't add up are the editor's (M3); conveniences go to the Wishlist below.

Then v0.2: released. Mateus merged the develop → main pull request (#72) and the guide's address fix (#74), tagged `v0.2.0`, and published v0.2.0 as a pre-release on 19 Oct 2026 (49 files and SHA256SUMS).

## M3 — Editor (v0.5)

Edit everything TuxGuitar edits, save it as .gp (the default) or .gp5, and release that as v0.5: the first release Linux users can write tabs with, where until now they had TuxGuitar or Wine (Mateus, 19 Oct 2026). The plan was proposed and approved on 19 Oct 2026; the parity checklist is [docs/design/editor.md](docs/design/editor.md).

- The editor's tools: note entry on tab and staff, the palette's TAB-1 and EDIT tools (left area), the inspector (right area), 4 voices, tracks and tunings, undo and redo, copy and paste, bar and signature changes, repeats, transposition, tuplets of any grouping with a triplet shortcut and "Other…" (EDIT-1 to EDIT-10). Guitar Pro 8-only tools stay greyed until M5. The Automation group, Timer and both stem-direction tools leave the palette: stems go by voice only, odd voices up and even voices down (Mateus, 19 Oct 2026).
- The .gp writer, in the GP 7–8 format plugin, the default save format; the .gp5 writer in the GP5 plugin, with loss warnings (FMT-3).
- Bars that don't add up (Mateus, 19 Oct 2026): checked against their time signature, every bar when a file opens and then only the bars an edit touches (speed first). Such bars are allowed, shown and played (with whatever adjustment playback needs, M4), only marked: drawn in red on screen, never in a PDF; the footer says how many and where the first is; Tools → Next Bar That Doesn't Add Up walks through them.
- Tracks (Mateus, 19 Oct 2026): a click on a row selects the track; a track is shown when its TAB or standard-notation button is on. Right-click on a row: Properties (also Edit → Track Properties…), Transpose… (the track), and Delete, which asks first. Transposition has two more ways in: a right-click on a selection (that selection) and Tools → Transpose… (every track). The track dialog: the name, instrument and capo and, for stringed instruments, the number of strings (3 to 10) and the tuning as a key and a pattern (Standard, Drop, Open with the root low as in Open D, Open with the fifth low as in Open G, Other); Other keeps the key and takes any note for each string, checked only as a note and against the number of strings. Tunings are named that compactly wherever they show (E Standard, Drop D, or the notes).
- Saving (Mateus, 19 Oct 2026): a file is saved in its own format unless that format would lose something, and then as .gp. Re-saving writes only what the model holds; the losses dialog names the rest, including what the opened file had that OxiGuitar couldn't keep. Our .gp claims no particular Guitar Pro version ("7", the oldest that reads it); the manual lists the versions tested.
- Plugin interface 1.2: the score v2 (free time first, with lists of extensions that only grow at their end), a selection event with voice, string and several tracks, and edits submitted by plugins as undoable commands, with plugin tools in the Tools menu. Frozen with v0.5.

Done when: the TuxGuitar parity checklist works; every edit undoes and redoes; round-trip tests pass for every writable format (FMT-4); files we write open correctly in Guitar Pro (Mateus, with Guitar Pro 7.6, at every slice that writes; the Guitar Pro 8 demo only if 8 itself is needed); transposition works chromatically and diatonically; a key typed in the 300-bar, 8-track file updates the page within 100 ms.

### M3's slices, in order (approved 19 Oct 2026)

Each slice is one or more pull requests into develop, with every gate green, a test build dispatched on its branch and direct artifact links for Mateus. From slice 2 on, every slice that writes files ends with Mateus's check in Guitar Pro 7.6.

1. **Caret, selection, undo machinery and the bar check.** Done (#79–#81); Mateus checked it on sid on 19 Oct 2026. A toolkit-free crate, `oxiguitar-edit`: the caret (track, staff, voice, bar, beat, string), selections, edit commands and an unlimited undo/redo history that keeps copies of only what each command touched. The engraver's map of where each beat and string sits. In the window: a click on the page or the arrow keys move a caret drawn above the tiles (no tile is redrawn); Shift extends a selection; the Edit menu (Undo, Redo); selection events reach plugins. A click on a track row selects it. Bars that don't add up, as above. The palette loses the Automation group, Timer and both stem-direction tools. The parity checklist (docs/design/editor.md), for Mateus's approval. Built in session 14 (#79, #80); after Mateus's check, #81 made the palette's voice buttons work (showing the caret's voice) and Ctrl + Up / Down switch a single shown track. The checklist is approved.
2. **Interface 1.2 and the .gp writer.** As above. Lessons from Guitar Pro 7.6 re-saving ten reference songs (`fixtures/guitar-pro-7.6/`, 19 Oct 2026): every track needs a sound from Guitar Pro's library (path, role, sound-engine settings; without one Guitar Pro 7.6 crashed on a voice track), chord diagrams need their chord spelled out (key note, bass, intervals, parsed from the name) or Guitar Pro drops them, and Guitar Pro 7.6 keeps no tempo text; the sound for every General MIDI program and every instrument of Guitar Pro's list is in `docs/formats/guitar-pro-sounds.md` (from files Mateus made in Guitar Pro 7.6); Save and Save As, the unsaved-changes question, the losses dialog (FMT-2 and FMT-3), Infer Key Signatures as one undoable command, `oxiguitar-cli convert`. Gates: round trips, MuseScore 4 and alphaTab opening what we write, a fuzz target for the writer, the contract suite on 1.2, and Guitar Pro 7.6's own shape: about ten small songs we make, which Mateus opens and saves in Guitar Pro 7.6; those re-saved files are committed as fixtures and our writer's output is compared with them (Mateus, 19 Oct 2026). Built so far (session 14, 20 Oct 2026): the .gp writer in the Guitar Pro 7–8 plugin, with its losses, `oxiguitar-cli convert`, the round trips over fixtures and corpus, the writer held to Guitar Pro 7.6's files, the writer's fuzz target, and the test builds' "guitar-pro-check" files for Mateus (#83); Save, Save As, the losses question and unsaved changes (#84); interface 1.2 with free time, the editor's selection, plugin tools and changes, the contract suite on 1.2. MuseScore 4 and alphaTab opening what we write, checked on every pull request (`interop.yml`). Left: Mateus's check in Guitar Pro 7.6.
3. **Note entry.** Frets on the tab, pitches on the staff, durations, rests, dots, ties, voices 1–4, insert and delete, File → New; Guitar Pro 8's shortcuts. Gates: the typing budget (100 ms), random command sequences undone to the original, every command saved and read back equal. Built (20 Oct 2026): frets, chords, rests, durations, dots, ties, voices, insert and delete, semitones and string moves from the keys and the palette, with all three gates (typing: 27 ms) (#87); pitches on the standard staff, File → New, a bar added at the end, moving beats, clearing, unused voices.
4. **Bars, structure, copy and paste, tuplets** (EDIT-6, EDIT-7, EDIT-10), inside OxiGuitar only. Built (20 Oct 2026): inserting and deleting bars, the Bar dialog (signatures, clef, tempo, feel, repeats, endings, directions, sections, double bar, free time), their keys and palette tools, triplets and Custom N-tuplet (#89); copy, cut and paste with Special Paste, Go to Section (#90). Mateus's check of slice 2 in Guitar Pro 7.6 (20 Oct 2026) fixed the .gp writer's octaves, stringless tracks and hammer-on arcs and kept slides to nothing (#91, #92); then the Inspector's beat counts and a double-click on a time signature for the Bar dialog (#93).
5. **Tracks, the track dialog and transposition** (EDIT-4, EDIT-8): a track from its right-click menu, a selection from its right-click menu, every track from Tools → Transpose…. Built (20 Oct 2026): adding, duplicating, deleting (after a question) and moving tracks; the track dialog (name, short name, instrument, strings 3 to 10, tuning as a key and a pattern or any notes, capo, frets) with a question when restringing would lose notes, keeping the pitches or the frets; transposition by semitones or steps of the key, keeping the fingering or finding new frets near where the hand was, for a track, the selection (or the caret's beat) and every track (which moves the key signatures); right-click menus on track rows and on the page; tunings named compactly in the page header. With them: notes of the caret's voice drawn in full ink and other voices greyed, and PDFs in pure black (Mateus, 20 Oct 2026).
6. **Effects, the inspector and the rest of the palette**: every TAB-1 effect, chord diagrams, lyrics, text; the palette's search field. Four pull requests: (a) the effects, from their keys and the palette, with the bend and whammy-bar editor; (b) the Inspector for the caret's note and beat, which changes the effects' values; (c) chord diagrams with Guitar Pro 8's chord builder, lyrics and text; (d) the score's information in a Song tab beside the Inspector, and the palette's search (Ctrl + F). Mateus's answers (20 Oct 2026): effects with a value start at a default that the Inspector changes; on a selection an effect goes on every note unless all have it, then off all; a note effect does nothing without a note on the caret's string; keys of our own are the effect's first letter with at most one modifier, commonest guitar effects first, and T is tapping, Ctrl + T Text (the time signature loses its key); dynamics get no key, and are hidden on the page by default. Built (20 Oct 2026): (a) every TAB-1 effect but chords and text, from the keys and the palette, the bend editor, dynamics from the palette; the palette's tools beyond TAB-1 say they come in M5 (#95); (b) the Inspector's note and beat, which change every value; (c) chord diagrams with the builder, text (Shift + T) and lyrics typed on the page as in MuseScore (Ctrl + T; Mateus, 21 Oct 2026) (#97); (d) the Song tab and the palette's search (Ctrl + F) (#98). Left: Mateus's check of slices 5 and 6.
7. **The .gp5 writer, then v0.5.** Built (21 Oct 2026): the Guitar Pro 5 plugin writes (interface 1.2, `writer2`) with every loss declared (FMT-3), and Save and Save As… write .gp5; the round-trip tests of FMT-4 (a .gp5 file read, written and read again is equal with no loss, in all 104 files of the corpus; any other file comes back equal unless the loss is named; the second trip is exact); the edit gate for .gp5; `fixtures/guitar-pro-check/` also holds every source as .gp5; the parity checklist ticked; the manual's saving chapter. The reader's mix scale, brush speeds and mute/solo were wrong and are fixed. Then the develop → main PR for v0.5, which Mateus merges.

## M4 — Player (v1.0)

The plan, proposed 21 Oct 2026 for Mateus's approval, with the audio spike's measurements: [docs/design/player.md](docs/design/player.md). Mateus's answers (21 Oct 2026): the app plays out of the box with whatever sound set the system has (Windows' gm.dls, macOS's CoreAudio set, a Linux distribution's SoundFont), bundles none and downloads none; better sets are files the user places in a `soundfonts` folder, and the footer links to the manual's page on getting them; the sound plugin's settings window (from the plugin manager) picks sets for guitars, basses, drums and the rest, globally; the sound must be exactly what the cursor shows, for exactly the written duration (people play along); the signal is built to feed NAM later (per-track NAM settings in the track dialog, added by the NAM plugin, which moved into v1.0 on 25 Oct 2026); SFZ before v1.0 if it isn't a big job, else v1.1; a release for testers as soon as playback is stable.

- In the core: score-to-event conversion with repeats, jumps, tempo changes and feel (PLAY-3, PLAY-4); the transport in the top area (play, play selection, stop, pause, rewind, count-in, loop, audible and visual metronome, PLAY-5); the follow cursor; the mixer (PLAY-6), in each Tracks row rather than a tab of its own; the output choice.
- As plugins: the sound-set instrument (SF2, SF3, DLS and SFZ; PLAY-1), reading the system's sets and the user's `soundfonts` folder, never downloading (PLAY-8 as changed on 21 Oct 2026), audio-device plugins per system, WAV export (PLAY-7). Instruments declare the classes of instrument they play.
- v1.0's minimum: the Guitar Pro 7+ format plugin reading and writing, the basic MIDI player and the NAM plugin, in `oxiguitar-plugins-basic`.

- A slide from a note to no note (a voice's last note, or before a rest) plays as a slide off the note: down toward the open string, up toward the last fret, fading out (Mateus, 20 Oct 2026: it is common on stringed instruments and must stay allowed; the player gives it a direction).
- Instruments declared by a plugin built for an interface before the instrument interface's are ignored by the player (Mateus, 19 Oct 2026: the `instruments` manifest key stays in 1.0, and this rule avoids rework). Until then no plugin counts as an output.

### M4's slices (approved 25 Oct 2026)

Mateus's answers (25 Oct 2026): rustysynth 1.3.8 for SF2, used exactly as published (no fork, no `[patch]`, nothing that waits on upstream) and as it is: its linear interpolation and the file modulators it ignores make a marginal difference, not worth working around; NAM through NeuralAmpModeler-rs on Linux x86_64 and nam-rs elsewhere; **the NAM plugin moves from M7 into v1.0**, since the spike showed the approach works.

1. **The timeline**: the score as events at their samples (playing order, tempo, feel, count-in, metronome, every TAB-1 effect), and the map back to beats. Golden files. Built (25 Oct 2026): `oxiguitar-play` and `oxiguitar-cli events`; golden timelines of the 55 reference and test-song files in `fixtures/events/`; all 104 corpus files give sound timelines (Wish You Were Here plays 5:32, the record 5:34); the 300-bar band song's 28,078 notes in 13 ms.
2. **Interface 1.3, audio devices and the engine**: instruments, effects, audio-device services (PipeWire, ALSA, WASAPI, CoreAudio through cpal), plugin sections in the track dialog, the scheduler, mixer and audio clock, plugin settings windows, footer links; the contract suite's real-time checks. In three pull requests (Mateus, 25 Oct 2026: cpal as it is; track sections saved in `FILE.oxg`; no editing while playing; `oxiguitar-cli play` kept). Built so far: the interface off the audio thread (settings windows from the plugin manager, track-dialog sections saved in `song.gp3.oxg`, footer messages with an https link, the plugins' data, cache and shared folders), through the test plugin and the contract suite; the engine (scheduler, mixer, clock, metronome and count-in; notes on their frames at every speed, no drift over ten minutes, nothing allocated while playing), with instruments and effects on the audio thread and the contract suite's real-time checks (timing, allocations, and system calls on Linux); the audio-device plugins (PipeWire, ALSA, WASAPI, Core Audio through cpal) and `oxiguitar-cli play`.
3. **The SoundFont plugin**: SF2, SF3 and DLS through rustysynth as it is (SF3 and DLS turned into in-memory SF2s); the `soundfonts` folders; its settings window; effects as MIDI. Out of the box on every system. In three pull requests: (1) the plugin with SoundFont 2 files, the sets found and the best one chosen, `oxiguitar-cli play --wav`, the packages' recommended set (built 25 Oct 2026); (2) SF3, DLS, the settings window and the footer's warning (built 25 Oct 2026); (3) the effects in MIDI terms (built 25 Oct 2026).
4. **The window**: transport, follow cursor, loop, count-in, metronome and visual metronome, speed, live mixer; the footer's link to better sounds; the manual's pages. Then a release for testers. In three pull requests (26 Oct 2026; the cursor a line moving with the sound plus the beat heard boxed, Mateus): (1) playing from the window: the transport, keys, speed, the master level with a limiter, the output, the live mixer, edits refused while playing (built 26 Oct 2026); (2) the cursor, the page following it, the overview's seek, the visual metronome (built 26 Oct 2026); (3) the manual's Playback page and the rest of Better sounds (built 26 Oct 2026; FreePats' guitars named once Mateus has heard them).
Mateus's order after v0.9 (26 Oct 2026): **SFZ before NAM**. SFZ brings recorded basses and drums, a big difference for everyone, while NAM mainly helps guitars (and is fastest on Linux x86_64); and it finishes the MIDI-driven sound before the amp modelling.

5. **The SFZ sampler** (in v1.0: Mateus, 26 Oct 2026), in the SoundFont plugin: round robins, velocity layers, release samples, key switches, choke groups; WAV, FLAC and Ogg samples, notes starting on their exact sample; SFZ instruments in the `soundfonts` folder and the settings window, with mapping files for drum kits that aren't General MIDI-mapped. With it, the manual's Better sounds page gets its basses and drums, and README and the Hear it page examples of recorded basses and drums, once Mateus has heard them. Mateus's answers (26 Oct 2026): an SFZ instrument plays only where picked; Guitars split into Electric and Acoustic guitars; samples whole in memory at their own bit depth. Five pull requests (player.md, "SFZ, as built"): reading SFZ instruments and their samples (built 26 Oct 2026); the sampler (built 26 Oct 2026); effects, mapping files and the settings window (built 26 Oct 2026); listening packs; the manual's and README's basses and drums.
6. **The NAM plugin**: amp models and cabinet IRs per track from the track dialog, on a DI sound set. With it, README and the manual's Hear it page get NAM sound examples and a screenshot of its track settings (Mateus, 26 Oct 2026).
7. **WAV export**, the milestone's gates, then v1.0. README.md shows the main features (Mateus, 26 Oct 2026): screenshots and example sounds, added with v0.9 (the score and the editor, playing with the cursor and the mixer; the manual's Hear it page with good sets only); recorded basses and drums join them with slice 5, NAM's track settings and NAM sound examples with slice 6, from an amp capture whose licence allows publishing.

Done when: playback matches the written effects by ear; playback starts in under 100 ms and stays in sync over 10 minutes (the cursor shows the sample being heard); golden-file tests cover event output; the practice tools work on every reference song; it plays out of the box on Windows, macOS and Linux with the system's own sound set; with no instrument or sound set the transport is greyed out and the footer says where to get one.

## Later milestones

- **M5** — the full Guitar Pro 8 notation set (TAB-2), its editing tools, stylesheets and design mode, per-note playback settings.
- **M6** — format plugins, each reading and writing, one plugin per format: MusicXML (.musicxml, .xml, .mxl), ABC, MuseScore (.mscz, .mscx) and MIDI files, with tab derivation for staves without tablature (TAB-3, TAB-4). Their sample files are counted apart from the Guitar Pro corpus (decided 16 Oct 2026).
- **M7** — a generated guitar signal for the NAM plugin (which ships in v1.0 on a DI sound set, Mateus, 25 Oct 2026): a pure-Rust plucked-string model (string, fret, pick position, pickup) that renders palm mute, harmonics, slides, bends and vibrato better than samples can, a few basic effects, per-track tone presets.
- **M8** — research spike and go/no-go on reading users' own Guitar Pro RSE soundbanks, as an instrument plugin.
- **M9** — a realistic drums instrument plugin (with a research spike on Singular Sound BeatBuddy content, used only from the user's own copy), audio tracks, advanced practice tools, line-in, PNG/SVG export, external MIDI devices.
- **M10** — a simpler viewer and player for Android and iPad, on the same core, with its plugins built in.

Unscheduled, late nice-to-have: screen-reader support (UI-11). Keyboard access is required from M1.

## Wishlist

Things that would make contributors' and plugin authors' lives easier, none needed by a milestone (Mateus, 19 Oct 2026: conveniences go here unless they speed up development). Each is an addition to the SDK, the API's Rust types or the tools, never to the wire, so any can come after the interface freezes.

- `Default` values or constructors for the wire score's types that have none (`MasterBar`, `Bar`, `Beat`, `Duration`, `Note`, `TimeSignature`, `KeySignature`), so a format reader doesn't spell out every field.
- Constructors for tab widgets (`Widget::button("id", "Text")` and the like).
- `oxiguitar-plugin-test --target TRIPLE`, and a universal macOS build, so the tool does what the template's `build-oxp.sh` does.
- Sample files in the contract suite (`oxiguitar-plugin-test FILE.oxp --sample song.extab`): read each one through the plugin and check the score's shape, since today the suite feeds readers only damaged input.

