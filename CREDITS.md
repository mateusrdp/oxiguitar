# Credits

OxiGuitar stands on the work of many people. This file lists every project we port code from, learn from or depend on, with its authors and license. The app's **Help → About** links here.

Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. Contributors keep the copyright on what they contribute.

## Special thanks

To everyone who has contributed code, tests, testing, translations of ideas into issues, or files for our test corpus. Your name goes here with your first merged contribution.

- Mateus Rocha de Paula ([@mateusrdp](https://github.com/mateusrdp)) — creator and maintainer

## Projects we port from or study

Each file ported or adapted from one of these starts with an SPDX header, the original copyright line, and the note "Ported from <project> <path> at <commit>". Nothing has been ported yet; this list records where we will look.

| Project | Authors | License | What we take |
| --- | --- | --- | --- |
| [MuseScore Studio](https://github.com/musescore/MuseScore) | Muse Group and contributors | GPL-3.0-only | Engraving and layout algorithms, .mscz logic, guitar features |
| [TuxGuitar](https://github.com/helge17/tuxguitar) | Julian Casadesus, helge17 and contributors | LGPL-2.1-only | Data model, Guitar Pro importers, effect-to-MIDI mapping |
| [alphaTab](https://github.com/CoderLine/alphaTab) | Danielku15 / CoderLine and contributors | MPL-2.0 | Guitar Pro 3–8 readers and writer, GPIF knowledge, effect rendering |
| [PyGuitarPro](https://github.com/Perlence/PyGuitarPro) | Sviatoslav Abakumov (Perlence) and contributors | LGPL-3.0-only | Byte-level documentation of the GP3–5 formats |
| [guitarpro / scorelib](https://codeberg.org/slundi/scorelib) | slundi | MIT | Rust readers for GP3–8; used as a cross-check while testing our readers (see ADR 0003) |
| [Verovio](https://github.com/rism-digital/verovio) | RISM Digital Center and contributors | LGPL-3.0 | Engraving techniques; reference only |
| [sfizz](https://github.com/sfztools/sfizz) | The sfizz contributors | BSD-2-Clause | How SFZ players read and play SFZ files (its parser's rules, opcodes' defaults, curves); reference only, for our own SFZ sampler |

## Rust crates we depend on

The authoritative list, with exact versions and licenses, is generated from `Cargo.lock` by [cargo-about](https://github.com/EmbarkStudios/cargo-about) (MIT OR Apache-2.0; `scripts/package/licenses.sh`) and shipped with every release package as THIRD-PARTY-LICENSES.html. CI runs [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and fails on any license outside the allow-list in `deny.toml`.

The core's direct dependencies, all Rust (the desktop app also links the system's font, keyboard and window libraries). The official plugins live in repositories of their own since OxiGuitar 0.11, and each credits its own libraries, sounds and sources in its README and in its manual's Credits page: the SoundFont and SFZ libraries in the MIDI plugin's, the Neural Amp Modeler engines and cabinets in the NAM plugin's, and so on.

| Crate | Authors | License | Use |
| --- | --- | --- | --- |
| [krilla](https://github.com/LaurenzV/krilla) | Laurenz Stampfl | MIT OR Apache-2.0 | PDF output. Its NOTICE.md credits code adapted from resvg (MPL-2.0) and from typst, svg2pdf and vello (Apache-2.0). |
| [tiny-skia](https://github.com/linebender/tiny-skia) | Yevhenii Reizner and contributors; a port of parts of Skia (Google) | BSD-3-Clause | Pixel output: screen, PNG |
| [skrifa](https://github.com/googlefonts/fontations) | Google (fontations) | MIT OR Apache-2.0 | Font outlines and metrics |
| [serde](https://serde.rs), [serde_json](https://github.com/serde-rs/json) | David Tolnay, Erick Tryzelaar | MIT OR Apache-2.0 | Reading the SMuFL metadata file |
| [zip](https://github.com/zip-rs/zip2) | Mathijs van de Nes, Marli Frost, Ryan Levick, Chris Hennick and contributors | MIT | Writing the Guitar Pro test songs (`oxiguitar-testgen`). No default features (they compile the zstd C library); deflate through flate2 with the pure-Rust [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) (Zlib) |
| [postcard](https://github.com/jamesmunns/postcard) | James Munns and contributors | MIT OR Apache-2.0 | Encoding the plugin interface's messages |
| [miniz_oxide](https://github.com/Frommi/miniz_oxide) | Frommi, oyvindln and contributors | MIT OR Zlib OR Apache-2.0 | Looking inside ZIP files to recognise their format (format signatures) |
| [chordparser](https://github.com/PauSala/chordparser) | Pau Sala | MIT | Chord names read in any convention and a diagram's notes named (the chord dialog, chord notation styles). It uses [enum-bitset](https://github.com/glueball/enum-bitset) (MIT) |
| [rtrb](https://github.com/mgeier/rtrb) | Matthias Geier and contributors | MIT OR Apache-2.0 | The player's lock-free queues to and from the audio thread |
| [object](https://github.com/gimli-rs/object) | The gimli-rs developers | Apache-2.0 OR MIT | Reading a plugin's manifest section without loading the plugin |
| [libloading](https://github.com/nagisa/rust_libloading) | Simonas Kazlauskas | ISC | Loading plugins |
| [libc](https://github.com/rust-lang/libc) | The Rust Project Developers | MIT OR Apache-2.0 | Catching a crash in a plugin to say which plugin it was (Unix) |
| [quick-xml](https://github.com/tafia/quick-xml) | Johann Tuffe, Mingun and contributors | MIT | GPIF (Guitar Pro 6–8) in the test songs (`oxiguitar-testgen`) |
| [Slint](https://slint.dev) | SixtyFPS GmbH and contributors | GPL-3.0-only (one of its three licences; the one we use) | The desktop window (ADR 0004). Because Slint is GPL-3.0-only, the distributed app is under GPL-3.0. Its dependencies include winit, femtovg and AccessKit; we build it without its system-tray support |
| [rfd](https://github.com/PolyMeilex/rfd) | Bartłomiej Maryńczak (PolyMeilex) and contributors | MIT | The system's Open and Save dialogs (the XDG desktop portal on Linux, no GTK) |

Tools used only to test OxiGuitar, never part of what it ships:

| Tool | Authors | License | Use |
| --- | --- | --- | --- |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libfuzzer-sys](https://github.com/rust-fuzz/libfuzzer) | The rust-fuzz project; libFuzzer by the LLVM project | MIT OR Apache-2.0; libFuzzer under the LLVM licence (NCSA) | Fuzzing the file readers (`fuzz/`) |

Checked for later milestones (licences confirmed 15 October 2026, see ROADMAP.md):

| Crate | License | Note |
| --- | --- | --- |
| [midir](https://github.com/Boddlnagg/midir) | MIT | MIDI ports (M8) |

## Fonts and assets

| Asset | Authors | License | Use |
| --- | --- | --- | --- |
| Placeholder icon (green pick, "OxG") | OxiGuitar contributors | GPL-3.0-or-later | App icon until real artwork exists |
| [Bravura](https://github.com/steinbergmedia/bravura) and its SMuFL metadata | Steinberg Media Technologies GmbH | OFL-1.1 | Default music font |
| [Noto Sans](https://github.com/notofonts/latin-greek-cyrillic) | The Noto Project Authors | OFL-1.1 | Text and tab numbers |

[assets/fonts/](assets/fonts/) holds the font files with their copyright notices and licence texts. [Leland](https://github.com/MuseScoreFonts/Leland) (Martin Keary, Simon Smith, OFL-1.1) is planned as an alternative music font. Fonts travel with the program under their own license; documents made with them are not affected.

## Trademarks

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. TuxGuitar, alphaTab and other names belong to their owners. OxiGuitar mirrors Guitar Pro's layout and shortcuts only: no code, icons, fonts, sounds or sample songs are copied from it.
