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
| [PyGuitarPro](https://github.com/Perlence/PyGuitarPro) | Perlence and contributors | LGPL-3.0 | Byte-level documentation of the GP3–5 formats |
| [guitarpro / scorelib](https://codeberg.org/slundi/scorelib) 0.4 | slundi | MIT | Rust readers for GP3–8; used as a cross-check while testing our readers (see ADR 0003) |
| [Verovio](https://github.com/rism-digital/verovio) | RISM Digital Center and contributors | LGPL-3.0 | Engraving techniques; reference only |

## Rust crates we depend on

The authoritative list, with exact versions and licenses, is generated from `Cargo.lock` and shipped with every release as a third-party license file. CI runs [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and fails on any license outside the allow-list in `deny.toml`.

So far OxiGuitar uses only the Rust standard library.

## Fonts and assets

| Asset | Authors | License | Use |
| --- | --- | --- | --- |
| Placeholder icon (green pick, "OxG") | OxiGuitar contributors | GPL-3.0-or-later | App icon until real artwork exists |

Planned: [Bravura](https://github.com/steinbergmedia/bravura) (Steinberg, OFL-1.1) as the default music font and [Leland](https://github.com/MuseScoreFonts/Leland) (Martin Keary, Simon Smith, OFL-1.1) as an alternative. Fonts ship beside the program under their own license.

## Trademarks

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. TuxGuitar, alphaTab and other names belong to their owners. OxiGuitar mirrors Guitar Pro's layout and shortcuts only: no code, icons, fonts, sounds or sample songs are copied from it.
