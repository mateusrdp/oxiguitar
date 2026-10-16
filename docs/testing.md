# Testing

How OxiGuitar is tested, and the quality gates M1 sets. `cargo test --workspace` runs everything that needs no private files; CI runs it on all six targets.

## Kinds of tests

- **Unit tests** beside the code, in every crate.
- **Public fixtures** (`fixtures/`, listed in `fixtures/README.md`): files we made ourselves, such as the test song in every Guitar Pro format, read by the format tests on every run.
- **Generated scores** (`crates/oxiguitar-testgen`): songs written in a short text notation and saved as Guitar Pro 7 files, so tests go through the real reader. The notation is described at the top of `crates/oxiguitar-testgen/src/song.rs`. Test code only: no OxiGuitar program depends on it.
- **The private corpus** (`corpus/`): community transcriptions that are never committed. The corpus tests (`--test corpus` in `oxiguitar-formats` and `oxiguitar-tools`, `--test engrave` in `oxiguitar-layout`) run only where `corpus/files/` holds it.

## Speed budget

M1 asks that a 300-bar, 8-track file opens in under a second. `crates/oxiguitar-app/src/speed.rs` holds it:

- **The score** is `oxiguitar_testgen::band_song(300)`: lead, rhythm and acoustic guitars, bass, piano, strings, voice with lyrics and drums, every track playing in every bar (sixteenths in the lead's solos). It is written as Guitar Pro 7 writes real files, with every note's pitch three ways and the usual beat properties: about 16 MB of score XML, as long as a real file of that size would be (Seek and Destroy, 258 bars and 5 tracks, is 10 MB).
- **What is timed** is what the window does: the worker reads the file, engraves it with the settings the window starts with, and draws the tiles the first view asks for (16 tiles of 512 pixels at a high-resolution screen's scale). The clock starts when the file is asked for and stops at the last tile. Once with the first track shown (how a file opens), once with all eight (View → Multitrack).
- **The check** takes the best of three runs, since a busy machine can only make a run slower, and holds only optimised builds to the budget. A debug build runs the same steps once and prints its times.

Run it with:

```sh
cargo test --release -p oxiguitar-app speed -- --nocapture
```

CI's "Speed budget" job runs it on Linux x86_64 on every pull request. Its times show as a notice on the run.

Times on the development machine (2 cores, 16 October 2026): 0.18 s with one track, 0.23 s with all eight; reading the file is most of it. Before this gate the XML tree made several allocations per element, and `oxiguitar-cli info` took 0.40 s on the same file; it now takes 0.18 s (Seek and Destroy: 0.26 s, now 0.11 s).
