# Changelog

All notable changes to OxiGuitar. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace, licence, contributor guide, credits, roadmap and architecture notes.
- CI on Windows, macOS and Linux (x86_64 and ARM64): formatting, lint, tests, minimum Rust version, licence and advisory checks, sign-off check and a no-network test.
- Test builds of `oxiguitar-cli` for every target on each push to `develop`.
- Placeholder app icon.
- Score model, first version (`oxiguitar-model`): score, master bars, tracks, staves, voices, beats, notes and the core effects, with exact fractional durations.
