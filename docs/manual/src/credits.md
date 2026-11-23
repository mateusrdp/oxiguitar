# Credits

OxiGuitar stands on the work of many people. Thank you to every one of them: the authors of the libraries below, the people who documented formats and engraving rules in the open, the font designers, and everyone who has tested OxiGuitar, reported a bug or shared a file for our tests.

This page covers the program itself. Each plugin credits its own libraries, sounds and sources in its own manual ([Plugin manuals](official-plugins.md)). The complete list, with every library's exact version and licence, ships with each release as `THIRD-PARTY-LICENSES.html`, and [CREDITS.md](https://github.com/mateusrdp/oxiguitar/blob/develop/CREDITS.md) in the repository is the record this page comes from.

## Libraries the program uses

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
| [cpal](https://github.com/RustAudio/cpal) | The RustAudio contributors | Apache-2.0 | Sound output: `oxiguitar-audio-cpal`, the layer the audio-output plugins for PipeWire, ALSA, WASAPI and Core Audio share; compatible with GPL-3.0, not GPL-2.0 |
| [object](https://github.com/gimli-rs/object) | The gimli-rs developers | Apache-2.0 OR MIT | Reading a plugin's manifest section without loading the plugin |
| [libloading](https://github.com/nagisa/rust_libloading) | Simonas Kazlauskas | ISC | Loading plugins |
| [libc](https://github.com/rust-lang/libc) | The Rust Project Developers | MIT OR Apache-2.0 | Catching a crash in a plugin to say which plugin it was (Unix) |
| [quick-xml](https://github.com/tafia/quick-xml) | Johann Tuffe, Mingun and contributors | MIT | GPIF (Guitar Pro 6 to 8) in the test songs (`oxiguitar-testgen`) |
| [Slint](https://slint.dev) | SixtyFPS GmbH and contributors | GPL-3.0-only (one of its three licences; the one we use) | The desktop window (ADR 0004). Because Slint is GPL-3.0-only, the distributed app is under GPL-3.0. Its dependencies include winit, femtovg and AccessKit; we build it without its system-tray support |
| [rfd](https://github.com/PolyMeilex/rfd) | Bartłomiej Maryńczak (PolyMeilex) and contributors | MIT | The system's Open and Save dialogs (the XDG desktop portal on Linux, no GTK) |

## Projects we learn from

| Project | Authors | License | What we take |
| --- | --- | --- | --- |
| [MuseScore Studio](https://github.com/musescore/MuseScore) | Muse Group and contributors | GPL-3.0-only | Engraving and layout algorithms, .mscz logic, guitar features |
| [TuxGuitar](https://github.com/helge17/tuxguitar) | Julian Casadesus, helge17 and contributors | LGPL-2.1-only | Data model, Guitar Pro importers, effect-to-MIDI mapping |
| [alphaTab](https://github.com/CoderLine/alphaTab) | Danielku15 / CoderLine and contributors | MPL-2.0 | Guitar Pro 3 to 8 readers and writer, GPIF knowledge, effect rendering |
| [PyGuitarPro](https://github.com/Perlence/PyGuitarPro) | Sviatoslav Abakumov (Perlence) and contributors | LGPL-3.0-only | Byte-level documentation of the GP3 to 5 formats |
| [guitarpro / scorelib](https://codeberg.org/slundi/scorelib) | slundi | MIT | Rust readers for GP3 to 8; used as a cross-check while testing our readers (see ADR 0003) |
| [Verovio](https://github.com/rism-digital/verovio) | RISM Digital Center and contributors | LGPL-3.0 | Engraving techniques; reference only |
| [sfizz](https://github.com/sfztools/sfizz) | The sfizz contributors | BSD-2-Clause | How SFZ players read and play SFZ files (its parser's rules, opcodes' defaults, curves); reference only, for our own SFZ sampler |

## Fonts and assets

| Asset | Authors | License | Use |
| --- | --- | --- | --- |
| Placeholder icon (green pick, "OxG") | OxiGuitar contributors | GPL-3.0-or-later | App icon until real artwork exists |
| [Bravura](https://github.com/steinbergmedia/bravura) and its SMuFL metadata | Steinberg Media Technologies GmbH | OFL-1.1 | Default music font |
| [Noto Sans](https://github.com/notofonts/latin-greek-cyrillic) | The Noto Project Authors | OFL-1.1 | Text and tab numbers |

## Tools

| Tool | Authors | License | Use |
| --- | --- | --- | --- |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libfuzzer-sys](https://github.com/rust-fuzz/libfuzzer) | The rust-fuzz project; libFuzzer by the LLVM project | MIT OR Apache-2.0; libFuzzer under the LLVM licence (NCSA) | Fuzzing the file readers (`fuzz/`) |
| [cargo-about](https://github.com/EmbarkStudios/cargo-about) | Embark Studios and contributors | MIT OR Apache-2.0 | The third-party licence list in every release |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Embark Studios and contributors | MIT OR Apache-2.0 | Checks every dependency's licence on each change |
| [mdBook](https://github.com/rust-lang/mdBook) | The Rust Project | MPL-2.0 | Builds this manual |

## Thanks

To Mateus Rocha de Paula, who started OxiGuitar, and to everyone who has contributed code, tests, testing, ideas or files. Your name goes in [CREDITS.md](https://github.com/mateusrdp/oxiguitar/blob/develop/CREDITS.md) with your first merged contribution.

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. TuxGuitar, alphaTab and other names belong to their owners. OxiGuitar mirrors Guitar Pro's layout and shortcuts only: no code, icons, fonts, sounds or sample songs are copied from it.
