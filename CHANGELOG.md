# Changelog

All notable changes to OxiGuitar. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

Milestone M0 (foundations) is complete and merged into `main` (16 October 2026). No version is tagged for it: the first release, v0.1, comes with the viewer at the end of M1. Work on M1 (the viewer) has started on `develop`.

### Added

- `oxiguitar-formats` reads Guitar Pro 6 (.gpx) and Guitar Pro 7 and 8 (.gp) files: the BCFZ/BCFS and ZIP containers and the GPIF score inside, with an import log of anything not carried over yet. Which notation each track shows (standard, tablature) comes from the file. All 15 GPX and GP files in the test corpus open; alphaTab reads the same notes from every one of them, bar for bar.
- Guitar Pro 6 tied notes with a stale fret take the fret of the note they continue; flat "bends" that Guitar Pro 7 writes on tied notes are dropped.
- Public test songs in Guitar Pro 6 and 7 format (`fixtures/gp/oxiguitar-test-song.gpx` and `.gp`), checked against the Guitar Pro 5 reader on the same music.
- Corpus tests (`cargo test --release -p oxiguitar-formats --test corpus`) that read every GPX and GP file and compare six songs saved in two formats; they run only where `corpus/files/` holds the private corpus.
- `docs/formats/guitar-pro-6-8.md`: what we learned about the containers and GPIF.
- Section markers keep their rehearsal letter (Guitar Pro 7 and 8); `oxiguitar-cli info` shows it as `[A] "Intro"`.

### Changed

- Drum notes from Guitar Pro 3–5 files are kept as pitches (the General MIDI drum) rather than as string and fret, as the score model intends for instruments without strings.

- Cargo workspace, licence, contributor guide, credits, roadmap and architecture notes.
- CI on Windows, macOS and Linux (x86_64 and ARM64): formatting, lint, tests, minimum Rust version, licence and advisory checks, sign-off check and a no-network test.
- Test builds of `oxiguitar-cli` for every target on each push to `develop`.
- Placeholder app icon.
- Score model, first version (`oxiguitar-model`): score, master bars, tracks, staves, voices, beats, notes and the core effects, with exact fractional durations.
- `oxiguitar-formats`: recognises Guitar Pro 1–8, MuseScore, MusicXML, ABC and MIDI files from their contents, never their extension (FMT-6), and reads Guitar Pro 3, 4 and 5 files into the score model with an import log of anything it cannot carry over (FMT-2). Damaged files give an error with the byte position, never a crash (FMT-1).
- `oxiguitar-cli info`, `detect` and `manifest` commands.
- Public test songs in Guitar Pro 3, 4 and 5 format (`fixtures/gp/`) and a manifest of the private test corpus (`corpus/MANIFEST.toml`).
- Decision record 0003: OxiGuitar writes its own file readers.
- `oxiguitar-display`: the display list that layout produces and every renderer draws, with colour roles for light and dark pages, the bundled Bravura and Noto Sans fonts, and a reader for SMuFL metadata.
- `oxiguitar-render`: draws a display list into pixels (screen, PNG) and into PDF with embedded font subsets.
- `oxiguitar-cli specimen`: a test page of notation and tablature as PNG and PDF.
- Decision record 0005: the renderer draws one display list on the CPU for the screen and with krilla for PDF.
- GUI-toolkit spike in Slint, iced and egui (`spikes/gui/`), built on every target by an advisory CI workflow. Decision record 0004 chooses Slint.
- Licences of the crates planned for later milestones checked and recorded in CREDITS.md and ROADMAP.md.
- Interface design rules (`docs/design/ui.md`) from the first review of the GUI spike. The Slint spike follows them: menus only on top, collapsible palette groups, palette icons centred on their ink, interface zoom with Ctrl + wheel and Ctrl + + / − / 0, a score-zoom slider and a dark page. View → Layout lists the four staff layouts to come in M1: Page, Vertical, Horizontal and Parchment.
