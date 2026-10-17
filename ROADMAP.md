# Roadmap

The milestones from the project brief, with live status. Requirement IDs (FMT-1, VIEW-3, …) refer to the brief's product requirements; decision records live in [docs/adr/](docs/adr/).

v1.0, a full TuxGuitar replacement for reading, editing and playing, ships at the end of M4. From M5 on, the work closes the gap with Guitar Pro 8.

On 18 October 2026 Mateus moved the plugin system forward, from M5 to M2, so that every later milestone is built on it: the core reads, edits, writes, shows and drives playback, and everything else (every file format, every sound, every export, anything platform-specific) is a plugin. See [ADR 0008](docs/adr/0008-plugin-architecture.md) and [docs/design/plugins.md](docs/design/plugins.md). Milestones after M1 were renumbered.

| Milestone | Release | Status |
| --- | --- | --- |
| M0 Foundations | — | **Done** (16 Oct 2026) |
| M1 Viewer | v0.1 | **Done** (17 Oct 2026; PR #40 to main and the v0.1.0 tag wait for Mateus) |
| M2 Plugin system and window shell | v0.2 | **Next** (design proposed, ADR 0008) |
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

Turn the viewer into a small core with plugins, and give the window the shape the later milestones fill in. Design: [docs/design/plugins.md](docs/design/plugins.md); open questions: [ADR 0008](docs/adr/0008-plugin-architecture.md).

- [ ] The window shell (docs/design/ui.md): toolbar with sections, tabbed top, bottom, left and right areas that collapse and hide, the page on a dark canvas, a one-line footer that expands into the session's messages; player controls greyed out in the top area until an output plugin is installed; mouse wheel zooms the page, Ctrl + wheel the interface.
- [ ] User folder, settings.json and the plugin folders, created on launch; the footer says when no output or no read/write plugin is installed and where to get them.
- [ ] Mateus answers ADR 0008's questions; ADR 0008 accepted.
- [ ] `oxiguitar-plugin-api` (the interface's messages), `oxiguitar-plugin` (the SDK for plugin authors, no `unsafe` for them), the host in `oxiguitar-host` (manifests read without loading, lazy loading, contributions registry, host services, services chosen per platform and environment).
- [ ] Five format plugins from M1's readers: Guitar Pro 3, 4, 5, 6 (.gpx), 7–8 (.gp), each with its fixtures, corpus, fuzz and visual tests. The core holds no format code.
- [ ] Plugins for PDF export and for Infer Key Signatures; `oxiguitar-cli` loads plugins like the app.
- [ ] The plugin manager plugin: list, enable, disable, uninstall, install from a file, settings forms. `--safe-mode`.
- [ ] Plugins built for all six targets in CI, attached to releases with SHA256SUMS; packages as decided (ADR 0008, question 7).
- [ ] The plugin developer guide (`docs/plugins/`), with a template plugin, and the interface's contract test suite.
- [ ] Debian checks for the core: manual pages for both binaries, lintian in CI (ADR 0008, question 5).

Done when: the core with no plugins opens nothing and says which plugin a file needs; with our plugins, every M1 gate still passes (42 of 42 corpus files, the speed budget including plugin loading, the 50 visual references unchanged, six hours of fuzzing per reader); start-up reads 30 plugin manifests in under 10 ms; the plugin manager works on all three systems; the developer guide builds a working plugin from the template.

## M3 — Editor (v0.5)

- M3a: note entry on tab and staff, the full core palette (left area), the inspector (right area), 4 voices, tracks and tunings, undo and redo, copy and paste, bar and signature changes, repeats, transposition, tuplets of any grouping with a triplet shortcut and "Other…" (EDIT-1 to EDIT-10); the .gp writer, in the GP 7–8 format plugin (the default save format).
- M3b: the .gp5 writer in the GP5 plugin, with loss warnings (FMT-3).

Done when: the TuxGuitar parity checklist works; every edit undoes and redoes; round-trip tests pass for every writable format; files we write open correctly in Guitar Pro 8; transposition works chromatically and diatonically.

## M4 — Player (v1.0)

- In the core: score-to-event conversion with repeats, jumps, tempo changes and feel (PLAY-3, PLAY-4); the transport in the top area (play, play selection, stop, pause, rewind, count-in, loop, audible and visual metronome, PLAY-5); the follow cursor; the mixer (PLAY-6); the output choice.
- As plugins: the MIDI instrument (a SoundFont synthesiser, PLAY-1, with downloads of better SoundFonts at the user's request, PLAY-8), audio-device plugins per system, WAV export (PLAY-7).

Done when: playback matches the written effects by ear; playback starts in under 100 ms and stays in sync over 10 minutes; golden-file tests cover event output; the practice tools work on every reference song; with no instrument installed the transport is greyed out and the footer says where to get one.

## Later milestones

- **M5** — the full Guitar Pro 8 notation set (TAB-2), its editing tools, stylesheets and design mode, per-note playback settings.
- **M6** — format plugins, each reading and writing, one plugin per format: MusicXML (.musicxml, .xml, .mxl), ABC, MuseScore (.mscz, .mscx) and MIDI files, with tab derivation for staves without tablature (TAB-3, TAB-4). Their sample files are counted apart from the Guitar Pro corpus (decided 16 Oct 2026).
- **M7** — a NAM instrument plugin: a clean guitar signal (a plucked-string model, or sampled DI notes if freely licensed ones exist) through Neural Amp Modeler and cabinet impulse responses. How the guitar signal is made is still open.
- **M8** — research spike and go/no-go on reading users' own Guitar Pro RSE soundbanks, as an instrument plugin.
- **M9** — a realistic drums instrument plugin (with a research spike on Singular Sound BeatBuddy content, used only from the user's own copy), audio tracks, advanced practice tools, line-in, PNG/SVG export, external MIDI devices.
- **M10** — a simpler viewer and player for Android and iPad, on the same core, with its plugins built in.

Unscheduled, late nice-to-have: screen-reader support (UI-11). Keyboard access is required from M1.
