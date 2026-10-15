# Bundled fonts

Both fonts are licensed under the SIL Open Font License 1.1 (OFL-1.1), which allows bundling them with GPL software. Each folder keeps the font's copyright notice and licence text, as the OFL requires; the release's third-party licence file and the app's About window carry them too. Documents OxiGuitar produces with these fonts, such as PDFs, are not affected by the OFL.

| Folder | Font | Version | Source | Use |
| --- | --- | --- | --- | --- |
| `bravura/` | Bravura (Steinberg Media Technologies GmbH), reserved font name "Bravura" | 1.482 | [steinbergmedia/bravura](https://github.com/steinbergmedia/bravura) `redist/`, commit 37b19437 (24 Aug 2026) | Music symbols (SMuFL reference font) |
| `bravura/Bravura.json` | Bravura's SMuFL metadata | 1.482 | same | Engraving defaults, glyph boxes and anchors |
| `noto-sans/` | Noto Sans Regular (The Noto Project Authors) | 2.015 | [notofonts.github.io](https://github.com/notofonts/notofonts.github.io) `fonts/NotoSans/hinted/ttf/` | Text, titles and tab numbers |

The files are unmodified. To update one, replace the files, update this table and the version in CREDITS.md, and run the tests: `oxiguitar-display` checks that every SMuFL glyph it names exists in both the font and the metadata.
