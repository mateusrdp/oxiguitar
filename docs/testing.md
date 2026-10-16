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

## Visual regression

M1 asks for visual regression tests over 50 reference scores, generated and kept in the repository. `crates/oxiguitar-testgen/tests/visual.rs` holds it:

- **The references** are defined in `crates/oxiguitar-testgen/src/references.rs`: fifty small scores, each about one part of engraving (rhythm, tuplets, keys and times, repeats, chord names, lyrics, dynamics, every effect, bass, seven strings, drums, piano, several tracks, the four layouts, Letter paper, the dark page, a system that must shrink to fit), with the options to show each. `fixtures/reference/README.md` lists them.
- **Kept in the repository** (`fixtures/reference/`): each as a Guitar Pro 7 file and its pictures at 1.5 pixels per point, `NAME-1.png` and on for each page. 1.4 MB in all, grey PNGs where the page is grey.
- **The check** first makes sure the stored file is what the generator makes, then reads it through the real reader (no reader warnings allowed), engraves it, draws it and compares the pictures. A pixel counts as changed when a channel moved by more than 40 of 255, and a picture fails when more than 4 pixels changed or its size changed: anti-aliasing noise passes, a missing staccato dot does not. A second test makes sure a picture shifted by one pixel, or with a dot painted out, fails.
- **On failure** the test writes what it drew and a picture of the differences (changes in red, the rest faded) to `target/tmp/visual-diff/`. In CI that folder is uploaded as an artifact named "visual-diff" and the platform.

It runs in every `cargo test`, on all six CI targets.

After an intended change to engraving (or to the references), bless the new pictures and look at every one that changed before committing:

```sh
OXIGUITAR_BLESS=1 cargo test -p oxiguitar-testgen --test visual -- --nocapture
git status fixtures/reference
```

Blessing writes every picture that differs at all, the stored scores, the index, and removes files that are no longer references. To add a reference, add it to `references()` (the test there keeps the count at fifty and every bar full) and bless.

The references show what the engraver draws today, including what it does not draw yet: the segno, coda and D.S. directions in `sections-and-directions` are in the file but not on the page, and `dynamics` shows ppp and pp crowding each other (see `docs/design/engraving.md`). When those are fixed, the pictures change and are blessed again.

## Fuzzing

M1 asks for six hours of fuzzing per reader without a crash (lowered from 24 hours by Mateus on 16 October 2026). `fuzz/` holds the cargo-fuzz targets, one per reader (Guitar Pro 3–5, Guitar Pro 6, Guitar Pro 7/8, and the GPIF XML on its own) plus one for any file through format detection; `fuzz/README.md` says how to run them. Seeds come from the public fixtures only.

CI's "Fuzz" workflow runs every target for four minutes on each pull request that touches a reader, and the long run when started by hand: each reader for six hours (by default), in two jobs of three hours on standard runners, at most four at a time. Fuzzing needs nightly Rust, so it lives outside the main workspace and its stable-only CI.

The first six-hour run (16 October 2026, [run 38891529443](https://github.com/mateusrdp/oxiguitar/actions/runs/38891529443)) found nothing in any reader:

| Reader (target) | Inputs tried | Coverage (edges) | Corpus kept |
| --- | --- | --- | --- |
| Guitar Pro 3–5 (`guitar_pro_3_5`) | 1,255 million | 2,539 | 1,534 |
| Guitar Pro 6 (`guitar_pro_6`) | 335 million | 3,806 | 2,258 |
| Guitar Pro 7/8 (`guitar_pro_7`) | 272 million | 4,300 | 1,320 |
| GPIF XML (`gpif`) | 37 million | 4,482 | 2,506 |

Each over two three-hour jobs on four cores. The final corpora are the run's artifacts ("fuzz-corpus … (final)", kept 30 days); a later run can start from them instead of the fixtures alone.
