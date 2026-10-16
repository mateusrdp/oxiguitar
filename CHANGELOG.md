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
- `oxiguitar-layout`, the engraver: turns a score into one display list per page in the Page layout (VIEW-3). Standard notation and tablature share columns bar by bar (VIEW-1), and several tracks line up in one score (VIEW-4). It draws clefs, key and time signatures with key changes and cancelling naturals, notes, chords, rests, accidentals that follow the key within a bar, ledger lines, stems and beams by voice, flags, dots, ties, tuplets, repeats, volta brackets, double and final bar lines, sections, tempo, chord names, beat text, bar numbers, and the score header with credits and tuning (VIEW-6). Tablature-only tracks show their rhythm around the tab (VIEW-2), and drum tracks use a drum staff with the General MIDI kit. Spacing follows duration and widens where symbols would touch; systems are stretched to the line, an overfull bar is squeezed, and a system taller than the page is engraved with a smaller staff (NOT-1, NOT-2, NOT-4).
- `oxiguitar-cli render <file> --png PREFIX --pdf FILE`: engraves a score into PNG pages and a PDF, light or dark (`--dark`), with a choice of tracks (`--tracks 1,3` or `all`).
- Engraving tests: public fixtures, damaged scores, a 300-bar 8-track score (engraved in well under half a second), and every Guitar Pro file of the private corpus with all its tracks, checking that everything lands on the page and every glyph exists in the fonts (`cargo test --release -p oxiguitar-layout --test engrave`).
- `docs/design/engraving.md`: how the engraver works, and what it does not draw yet.
- CI shows failing tests as an annotation on the pull request.
- Effects and marks in the engraving (TAB-1, NOT-3):
  - **Between notes:** hammer-ons and pull-offs (arcs with H/P), slides between notes and into or out of them, bends and releases with their amounts.
  - **Along the music:** vibrato, trills, palm mute, let ring, harmonic kinds, tap/slap/pop, w/bar, fade in, pick strokes.
  - **On notes:** natural harmonics (diamond heads, `<12>` on tab), ghost notes in parentheses, accents and staccato, tremolo strokes, strums.
  - **Grace notes** before their main note.
  - **Dynamics** where the marking changes.
  - **Lyrics** under the staff, with room made for each syllable.

  Marks that run along the music get lanes of their own above the tablature, so they never collide.
- Chord diagrams of the chords used, in the score header (VIEW-6).
- Characters the text font lacks, such as ♯ and ♭ in chord names, come from the music font.
- Track names at the start of each system when several tracks are shown: full names on the first system, short names after, as in Guitar Pro's multitrack view.
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

### Changed

- Guitar Pro 6–8 beats keep their dynamic as a marking (`BeatEffects::dynamic`) as well as giving it to their notes as velocity, so the engraver can tell a marking from a velocity.
- Beat text at the end of a system no longer runs off the page; the page test now checks where each text ends, not only where it starts.
- Plan (Mateus, 16 Oct 2026): the core reads and writes Guitar Pro formats only. MuseScore, MusicXML, ABC and MIDI files, and tab derivation for staves without tablature, become plugins after the plugin system (M5). M1's "98 % of the corpus opens" counts Guitar Pro files only. Visual regression uses 50 generated reference scores kept in the repository. ROADMAP and README follow. Opening one of those files now says that a format plugin will open it, instead of "not supported yet".
- Voice 1's stems point up even when it is the only voice in the bar (Mateus, 16 Oct 2026). Stems on the staff now always follow the voice number.
- Stems follow the voice number, as Mateus asked after the first review: voices 1 and 3 up, voices 2 and 4 down, whenever a voice other than the first has notes in the bar (before, the first voice with notes went up and all others down, so voice 3 pointed down and a lone voice 2 pointed up). Drums always follow it; voice 1 alone still points its stems by where its notes sit. Rests of several voices move up or down by the same rule.
- Rhythm on tablature (tab-only tracks) shows every voice with the same rule: voices 1 and 3 with stems up above the tab, voices 2 and 4 with stems down below it. Before, only voice 1 was drawn, with stems down. Rests on tab move out of the staff into each voice's rhythm area, so they no longer cover fret numbers.
- Chord names stay over their beat. Instead of pushing a name sideways when it would touch the next one, the music opens up to make room. On tab-only tracks the chord names sit above the upward stems.
- Drum notes from Guitar Pro 3–5 files are kept as pitches (the General MIDI drum) rather than as string and fret, as the score model intends for instruments without strings.
