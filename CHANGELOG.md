# Changelog

All notable changes to OxiGuitar. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace, licence, contributor guide, credits, roadmap and architecture notes.
- CI on Windows, macOS and Linux (x86_64 and ARM64): formatting, lint, tests, minimum Rust version, licence and advisory checks, sign-off check and a no-network test.
- Test builds of `oxiguitar-cli` for every target on each push to `develop`.
- Placeholder app icon.
- Score model, first version (`oxiguitar-model`): score, master bars, tracks, staves, voices, beats, notes and the core effects, with exact fractional durations.
- `oxiguitar-formats`: recognises Guitar Pro 1–8, MuseScore, MusicXML, ABC and MIDI files from their contents, never their extension (FMT-6), and reads Guitar Pro 3, 4 and 5 files into the score model with an import log of anything it cannot carry over (FMT-2). Damaged files give an error with the byte position, never a crash (FMT-1).
- `oxiguitar-cli info`, `detect` and `manifest` commands.
- Public test songs in Guitar Pro 3, 4 and 5 format (`fixtures/gp/`) and a manifest of the private test corpus (`corpus/MANIFEST.toml`).
- Decision record 0003: OxiGuitar writes its own file readers.
