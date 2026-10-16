# Roadmap

The milestones from the project brief, with live status. Requirement IDs (FMT-1, VIEW-3, …) refer to the brief's product requirements; decision records live in [docs/adr/](docs/adr/).

v1.0, a full TuxGuitar replacement for reading, editing and playing, ships at the end of M3. From M4 on, the work closes the gap with Guitar Pro 8.

| Milestone | Release | Status |
| --- | --- | --- |
| M0 Foundations | — | **Done** (16 Oct 2026) |
| M1 Viewer | v0.1 | **In progress** |
| M2 Editor | v0.5 | Not started |
| M3 MIDI player | **v1.0** | Not started |
| M4 Guitar Pro 8 notation and editing parity | | Not started |
| M5 Plugin system | | Not started |
| M6 Realistic guitar through NAM (plugin) | | Not started |
| M7 Guitar Pro RSE compatibility (plugin) | | Not started |
| M8 Remaining Guitar Pro 8 features, hardware plugins | | Not started |
| M9 Android and iPad companion | | Not started |

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

Open every supported format and show it beautifully.

- Readers for GP5 onwards and the other formats in the brief, with import logs; GP3 and GP4 if cheap. Guitar Pro readers first; MuseScore, MusicXML and ABC once more sample files arrive.
- Engraving for standard notation, tablature and rhythm-on-tab, with the core effects drawn (VIEW-1, VIEW-2, VIEW-7, NOT-1 to NOT-4).
- Tab derivation for files with only standard notation (TAB-3, TAB-4).
- Page, vertical, horizontal and parchment layouts, multi-track display, zoom, score header (VIEW-3 to VIEW-6).
- The desktop window (`oxiguitar-app`, Slint) following [docs/design/ui.md](docs/design/ui.md): menus only on top, collapsible palette groups, interface zoom by keys and wheel, score-zoom slider, dark page (UI-7 to UI-10).
- PDF export (EXP-1). User manual on GitHub Pages. Release packages (release.yml).

Done when: 98% of the GP5-and-later corpus opens without error, the rest with clear messages; 24 hours of fuzzing per reader without a crash; a 300-bar, 8-track file opens in under a second; visual regression tests cover 50 reference scores; Mateus has reviewed PDFs of 10 songs.

Progress:

- [x] Guitar Pro 6 (.gpx) and 7/8 (.gp) readers, with import logs and public fixtures (16 Oct 2026). Every GPX and GP file in the corpus opens; alphaTab reads the same notes from each, bar for bar; six songs saved in two formats agree (`--test corpus`).
- [ ] Engraving into a display list (`oxiguitar-layout`): standard notation, tablature, rhythm on tab, effects
- [ ] Page, vertical, horizontal and parchment layouts; multi-track; score header
- [ ] `oxiguitar-app` window (Slint) following docs/design/ui.md
- [ ] PDF export
- [ ] MuseScore, MusicXML and ABC readers; tab derivation
- [ ] Fuzzing, speed budget, visual regression tests
- [ ] User manual, release packages

Corpus status (GP5 and later, 16 Oct 2026): 27 GP5, 3 GPX and 12 GP files open (42 of 48); the 6 MuseScore and MusicXML files wait for their readers.

## M2 — Editor (v0.5)

- M2a: note entry on tab and staff, the full core palette, 4 voices, tracks and tunings, undo and redo, copy and paste, bar and signature changes, repeats, transposition (EDIT-1 to EDIT-9); writers for .gp and MusicXML.
- M2b: writers for .gp5, .mscz and ABC, with loss warnings (FMT-3).

Done when: the TuxGuitar parity checklist works; every edit undoes and redoes; round-trip tests pass for every writable format; files we write open correctly in Guitar Pro 8 and MuseScore 4.7; transposition works chromatically and diatonically.

## M3 — MIDI player (v1.0)

- Score-to-event conversion with repeats, jumps, tempo changes and feel (PLAY-3, PLAY-4); built-in SoundFont synthesiser (PLAY-1).
- Transport, speed, follow cursor, mixer; looping, count-in, audible and visual metronome (PLAY-5, PLAY-6).
- MIDI import with quantisation and tab derivation; MIDI and WAV export (PLAY-7).

Done when: playback matches the written effects by ear; playback starts in under 100 ms and stays in sync over 10 minutes; golden-file tests cover event output; the practice tools work on every reference song.

## Later milestones

- **M4** — the full Guitar Pro 8 notation set (TAB-2), its editing tools, stylesheets and design mode, per-note playback settings.
- **M5** — plugin system (WebAssembly and native), plugin manager; the metronome becomes the reference plugin.
- **M6** — a pure-Rust plucked-string model through Neural Amp Modeler and cabinet impulse responses, as a plugin.
- **M7** — research spike and go/no-go on reading users' own Guitar Pro RSE soundbanks, as a plugin.
- **M8** — audio tracks, advanced practice tools, line-in, PNG/SVG export, external MIDI devices.
- **M9** — a simpler viewer and player for Android and iPad.

Unscheduled, late nice-to-have: screen-reader support (UI-11). Keyboard access is required from M1.
