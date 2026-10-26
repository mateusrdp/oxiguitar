# Credits

OxiGuitar stands on the work of many people. This file lists every project we port code from, learn from or depend on, with its authors and license. The app's About window shows the same list.

Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. Contributors keep the copyright on what they contribute.

## Special thanks

To everyone who has contributed code, tests, testing, translations of ideas into issues, or files for our test corpus. Your name goes here with your first merged contribution.

- Mateus Rocha de Paula ([@mateusrdp](https://github.com/mateusrdp)) — creator and maintainer

## Projects we port from or study

Each file ported or adapted from one of these starts with an SPDX header, the original copyright line, and the note "Ported from <project> <path> at <commit>". Nothing has been ported yet; this list records where we will look.

| Project | Authors | License | What we take |
| --- | --- | --- | --- |
| [MuseScore Studio](https://github.com/musescore/MuseScore) 4.7 | Muse Group and contributors | GPL-3.0-only | Engraving and layout algorithms, .mscz logic, guitar features |
| [TuxGuitar](https://github.com/helge17/tuxguitar) 2.0 | Julian Casadesus, helge17 and contributors | LGPL-2.1-only | Data model, Guitar Pro importers, effect-to-MIDI mapping |
| [alphaTab](https://github.com/CoderLine/alphaTab) 1.x | Danielku15 / CoderLine and contributors | MPL-2.0 | Guitar Pro 3–8 readers and writer, GPIF knowledge, effect rendering |
| [PyGuitarPro](https://github.com/Perlence/PyGuitarPro) 0.11 | Sviatoslav Abakumov (Perlence) and contributors | LGPL-3.0-only | Byte-level documentation of the GP3–5 formats |
| [guitarpro / scorelib](https://codeberg.org/slundi/scorelib) 0.4 | slundi | MIT | Rust readers for GP3–8; used as a cross-check while testing our readers (see ADR 0003) |
| [Verovio](https://github.com/rism-digital/verovio) | RISM Digital Center and contributors | LGPL-3.0 | Engraving techniques; reference only |

## Rust crates we depend on

The authoritative list, with exact versions and licenses, is generated from `Cargo.lock` by [cargo-about](https://github.com/EmbarkStudios/cargo-about) (MIT OR Apache-2.0; `scripts/package/licenses.sh`) and shipped with every release package as THIRD-PARTY-LICENSES.html. CI runs [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and fails on any license outside the allow-list in `deny.toml`.

Direct dependencies so far, all Rust (the desktop app also links the system's font, keyboard and window libraries):

| Crate | Authors | License | Use |
| --- | --- | --- | --- |
| [krilla](https://github.com/LaurenzV/krilla) 0.8 | Laurenz Stampfl | MIT OR Apache-2.0 | PDF output. Its NOTICE.md credits code adapted from resvg (MPL-2.0) and from typst, svg2pdf and vello (Apache-2.0). |
| [tiny-skia](https://github.com/linebender/tiny-skia) 0.12 | Yevhenii Reizner and contributors; a port of parts of Skia (Google) | BSD-3-Clause | Pixel output: screen, PNG |
| [skrifa](https://github.com/googlefonts/fontations) 0.42 | Google (fontations) | MIT OR Apache-2.0 | Font outlines and metrics |
| [serde](https://serde.rs), [serde_json](https://github.com/serde-rs/json) | David Tolnay, Erick Tryzelaar | MIT OR Apache-2.0 | Reading the SMuFL metadata file |
| [zip](https://github.com/zip-rs/zip2) 9 | Mathijs van de Nes, Marli Frost, Ryan Levick, Chris Hennick and contributors | MIT | Guitar Pro 7/8 archives (later .mscz). No default features (they compile the zstd C library); deflate through flate2 with the pure-Rust [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) (Zlib) |
| [postcard](https://github.com/jamesmunns/postcard) 1.1 | James Munns and contributors | MIT OR Apache-2.0 | Encoding the plugin interface's messages |
| [rustysynth](https://github.com/sinshu/rustysynth) 1.3.8 | Nobuaki Tanaka (sinshu), a port of his MeltySynth | MIT | SoundFont synthesis: the SoundFont plugin plays every track through it, as published |
| [cpal](https://github.com/RustAudio/cpal) 0.18 | The RustAudio contributors | Apache-2.0 | Sound output: the audio-device plugins for PipeWire, ALSA, WASAPI and Core Audio; compatible with GPL-3.0, not GPL-2.0 |
| [object](https://github.com/gimli-rs/object) 0.37 | The gimli-rs developers | Apache-2.0 OR MIT | Reading a plugin's manifest section without loading the plugin |
| [libloading](https://github.com/nagisa/rust_libloading) 0.8 | Simonas Kazlauskas | ISC | Loading plugins |
| [quick-xml](https://github.com/tafia/quick-xml) 0.42 | Johann Tuffe, Mingun and contributors | MIT | GPIF (Guitar Pro 6–8); later MusicXML and .mscx |
| [Slint](https://slint.dev) 1.17 | SixtyFPS GmbH and contributors | GPL-3.0-only (one of its three licences; the one we use) | The desktop window (ADR 0004). Because Slint is GPL-3.0-only, the distributed app is under GPL-3.0. Its dependencies include winit, femtovg and AccessKit; we build it without its system-tray support |
| [rfd](https://github.com/PolyMeilex/rfd) 0.17 | Bartłomiej Maryńczak (PolyMeilex) and contributors | MIT | The system's Open and Save dialogs (the XDG desktop portal on Linux, no GTK) |

Tools used only to test OxiGuitar, never part of what it ships:

| Tool | Authors | License | Use |
| --- | --- | --- | --- |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libfuzzer-sys](https://github.com/rust-fuzz/libfuzzer) 0.4 | The rust-fuzz project; libFuzzer by the LLVM project | MIT OR Apache-2.0; libFuzzer under the LLVM licence (NCSA) | Fuzzing the file readers (`fuzz/`) |

Checked for later milestones (licences confirmed 15 October 2026, see ROADMAP.md):

| Crate | Version checked | License | Note |
| --- | --- | --- | --- |
| [midir](https://github.com/Boddlnagg/midir) | 0.11.1 | MIT | MIDI ports (M8) |
| [musicxml](https://github.com/hedgetechllc/musicxml) | 1.1.2 | MIT | MusicXML reader/writer or reference (M1); last release November 2024 |
| [abc-parser](https://gitlab.com/Askaholic/rust-abc-2) | 0.4.0 | MIT | ABC 2.1 parser, incomplete (M1) |

## Fonts and assets

| Asset | Authors | License | Use |
| --- | --- | --- | --- |
| Placeholder icon (green pick, "OxG") | OxiGuitar contributors | GPL-3.0-or-later | App icon until real artwork exists |
| [Bravura](https://github.com/steinbergmedia/bravura) 1.482 and its SMuFL metadata | Steinberg Media Technologies GmbH | OFL-1.1 | Default music font |
| [Noto Sans](https://github.com/notofonts/latin-greek-cyrillic) 2.015 | The Noto Project Authors | OFL-1.1 | Text and tab numbers |

[assets/fonts/](assets/fonts/) holds the font files with their copyright notices and licence texts. [Leland](https://github.com/MuseScoreFonts/Leland) (Martin Keary, Simon Smith, OFL-1.1) is planned as an alternative music font. Fonts travel with the program under their own license; documents made with them are not affected.

## Trademarks

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. TuxGuitar, alphaTab and other names belong to their owners. OxiGuitar mirrors Guitar Pro's layout and shortcuts only: no code, icons, fonts, sounds or sample songs are copied from it.
