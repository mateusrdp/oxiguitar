# Testing

How OxiGuitar is tested, and the quality gates M1 sets. `cargo test --workspace` runs everything that needs no private files; CI runs it on all six targets, with default features only (with `--all-features`, every plugin crate would export the plugin entry symbol, and a binary linking two would not link).

Since M2 the core opens files only through format plugins, so the gates run through our plugins built as the `.oxp` files we ship: tests that open files call `oxiguitar_plugin_test::build_ours`, which builds each plugin on its own (as `scripts/build-plugins.sh` does) into `target/oxp/ours-debug/` or `ours-release/`, and load them through the host once per test program.

## Kinds of tests

- **Unit tests** beside the code, in every crate.
- **Public fixtures** (`fixtures/`, listed in `fixtures/README.md`): files we made ourselves, such as the test song in every Guitar Pro format, read by the format tests on every run.
- **Generated scores** (`crates/oxiguitar-testgen`): songs written in a short text notation and saved as Guitar Pro 7 files, so tests go through the real reader. The notation is described at the top of `crates/oxiguitar-testgen/src/song.rs`. Test code only: no OxiGuitar program depends on it.
- **The private corpus** (`corpus/`): community transcriptions that are never committed. The corpus tests (`--test corpus` in `oxiguitar-formats`, `oxiguitar-tools` and `oxiguitar-edit`, `--test engrave` in `oxiguitar-layout`, the corpus part of `--test ours` in `oxiguitar-plugin-test`) run only where `corpus/files/` holds it.
- **Our plugins through their files** (`crates/oxiguitar-plugin-test/tests/ours.rs`): the five Guitar Pro plugins pass the contract; each recognises and reads its own format and refuses the others (it holds only its own reader); every fixture, reference and corpus file reads through them exactly as the reader reads it in-process, score and log (42 of 42 corpus files of Guitar Pro 5 and later open); a file no plugin recognises is refused with a feature request, or a bug report when a plugin declares its extension; with no plugin, nothing opens.
- **The plugin developer guide builds a working plugin** (`scripts/check-plugin-template.sh`, CI's "Plugin template" on Linux, Windows and macOS): the template plugin (`docs/plugins/template/`) copied out of the repository as an author would, against this checkout's SDK: fmt, clippy, its own tests, the manifest check, the contract suite through `cargo test` and through `oxiguitar-plugin-test --build`, then `oxiguitar-cli` opens its sample through the plugin file and engraves it. It also checks that every example the guide includes from the template exists.
- **The plugin interface end to end** (`crates/oxiguitar-plugin-test/tests/contract.rs`): the test plugin (`plugins/test`) built as a real `.oxp` and driven through the host on every system CI tests: manifests read without loading, reading and writing, errors and panics, services, settings, instruments by class, duplicates, disabled and damaged files, and (interface 1.1) tabs in two areas, every kind of widget and its event, a canvas with pointer events, contents the core refuses, events pushed in order (merged when a plugin is behind), a redraw asked for, and the open score asked for; and a plugin that fails (a crash, contents the core can't show) withdrawn as if it weren't installed, its file going to a second test plugin (`plugins/test-second`) that reads the same format, its services chosen again. `oxiguitar-plugin-test`'s `check` runs the same contract on any plugin file.

## Speed budget

M1 asks that a 300-bar, 8-track file opens in under a second. `crates/oxiguitar-app/src/speed.rs` holds it:

- **The score** is `oxiguitar_testgen::band_song(300)`: lead, rhythm and acoustic guitars, bass, piano, strings, voice with lyrics and drums, every track playing in every bar (sixteenths in the lead's solos). It is written as Guitar Pro 7 writes real files, with every note's pitch three ways and the usual beat properties: about 16 MB of score XML, as long as a real file of that size would be (Seek and Destroy, 258 bars and 5 tracks, is 10 MB).
- **What is timed** is what the window does: the worker reads the file through the Guitar Pro 7–8 plugin (`gp.oxp`, a release build), engraves it with the settings the window starts with, and draws the tiles the first view asks for (16 tiles of 512 pixels at a high-resolution screen's scale). The clock starts when the file is asked for and stops at the last tile. Once with the first track shown (how a file opens), once with all eight shown.
- **The check** takes the best of three runs, since a busy machine can only make a run slower, and holds only optimised builds to the budget. The first run of whichever test opens a file first also loads the plugin, so each test holds its first run to the budget too. A debug build runs the same steps once and prints its times.

Run it with:

```sh
cargo test --release -p oxiguitar-app speed -- --nocapture
```

CI's "Speed budget" job runs it on Linux x86_64 on every pull request. Its times show as a notice on the run.

Times on the development machine (2 cores, 16 October 2026): 0.18 s with one track, 0.23 s with all eight; reading the file is most of it. Through the plugin (18 October 2026): 0.21 s and 0.28 s, the first runs (loading the plugin) 0.26 s and 0.32 s; the score crossing the plugin interface (encoded, decoded, converted) is the difference. Before this gate the XML tree made several allocations per element, and `oxiguitar-cli info` took 0.40 s on the same file; it now takes 0.18 s (Seek and Destroy: 0.26 s, now 0.11 s).

## Visual regression

M1 asks for visual regression tests over 50 reference scores, generated and kept in the repository. `crates/oxiguitar-testgen/tests/visual.rs` holds it:

- **The references** are defined in `crates/oxiguitar-testgen/src/references.rs`: fifty small scores, each about one part of engraving (rhythm, tuplets, keys and times, repeats, chord names, lyrics, dynamics, every effect, bass, seven strings, drums, piano, several tracks, the four layouts, Letter paper, the dark page, a system that must shrink to fit), with the options to show each. `fixtures/reference/README.md` lists them.
- **Kept in the repository** (`fixtures/reference/`): each as a Guitar Pro 7 file and its pictures at 1.5 pixels per point, `NAME-1.png` and on for each page. 1.4 MB in all, grey PNGs where the page is grey.
- **As Guitar Pro 7.6 saves them** (`fixtures/guitar-pro-7.6/`, M3): ten of the references opened and saved by Mateus in Guitar Pro 7.6, their score XML only. `cargo test -p oxiguitar-testgen --test guitar_pro_7_6` reads each against our original: everything survives but for chord diagrams without a spelled-out chord, tempo text and the mixer, which Guitar Pro changes (the folder's README). The .gp writer is compared against them in slice 2.
- **The check** first makes sure the stored file is what the generator makes, then reads it as the app does, through the Guitar Pro 7–8 plugin built as `gp.oxp` (no reader warnings allowed), engraves it, draws it and compares the pictures. A pixel counts as changed when a channel moved by more than 40 of 255, and a picture fails when more than 4 pixels changed or its size changed: anti-aliasing noise passes, a missing staccato dot does not. A second test makes sure a picture shifted by one pixel, or with a dot painted out, fails.
- **On failure** the test writes what it drew and a picture of the differences (changes in red, the rest faded) to `target/tmp/visual-diff/`. In CI that folder is uploaded as an artifact named "visual-diff" and the platform.

It runs in every `cargo test`, on all six CI targets.

After an intended change to engraving (or to the references), bless the new pictures and look at every one that changed before committing:

```sh
OXIGUITAR_BLESS=1 cargo test -p oxiguitar-testgen --test visual -- --nocapture
git status fixtures/reference
```

Blessing writes every picture that differs at all, the stored scores, the index, and removes files that are no longer references. To add a reference, add it to `references()` (the test there keeps the count at fifty and every bar full) and bless.

The references show what the engraver draws today. When an engraving change is intended, the pictures it touches change and are blessed again; the ones that showed gaps (directions in `sections-and-directions`, ppp and pp crowding in `dynamics`) were blessed that way in session 6.

## Fuzzing

M1 asks for six hours of fuzzing per reader without a crash (lowered from 24 hours by Mateus on 16 October 2026). `fuzz/` holds the cargo-fuzz targets, one per reader (Guitar Pro 3–5, Guitar Pro 6, Guitar Pro 7/8, and the GPIF XML on its own), one for any file through format detection, one for our format plugins' `dispatch` (the request to read a file answered by each of the five, the answer encoded and decoded as it crosses the interface, the score converted back into the model), and one for a plugin's answer with a tab's contents as the core takes it (`plugin_tab`: decoded, checked, every canvas drawn; on pull requests only, like the any-file target); `fuzz/README.md` says how to run them. Seeds come from the public fixtures only.

CI's "Fuzz" workflow runs every target for four minutes on each pull request that touches a reader, and the long run when started by hand: each reader for six hours (by default), in two jobs of three hours on standard runners, at most four at a time. Fuzzing needs nightly Rust, so it lives outside the main workspace and its stable-only CI.

The first six-hour run (16 October 2026, [run 38891529443](https://github.com/mateusrdp/oxiguitar/actions/runs/38891529443)) found nothing in any reader:

| Reader (target) | Inputs tried | Coverage (edges) | Corpus kept |
| --- | --- | --- | --- |
| Guitar Pro 3–5 (`guitar_pro_3_5`) | 1,255 million | 2,539 | 1,534 |
| Guitar Pro 6 (`guitar_pro_6`) | 335 million | 3,806 | 2,258 |
| Guitar Pro 7/8 (`guitar_pro_7`) | 272 million | 4,300 | 1,320 |
| GPIF XML (`gpif`) | 37 million | 4,482 | 2,506 |

Each over two three-hour jobs on four cores. The final corpora are the run's artifacts ("fuzz-corpus … (final)", kept 30 days); a later run can start from them instead of the fixtures alone.

The second six-hour run (18 October 2026, [run 39105892287](https://github.com/mateusrdp/oxiguitar/actions/runs/39105892287), on develop f23cc11) fuzzed the readers once they had become the format plugins' code (M2 slice 1). It found no crash, hang or out-of-memory input:

| Reader (target) | Inputs tried | Coverage (edges) | Corpus kept |
| --- | --- | --- | --- |
| Guitar Pro 3–5 (`guitar_pro_3_5`) | 1,514 million | 2,547 | 1,529 |
| Guitar Pro 6 (`guitar_pro_6`) | 371 million | 3,803 | 2,229 |
| Guitar Pro 7/8 (`guitar_pro_7`) | 229 million | 4,265 | 1,324 |
| GPIF XML (`gpif`) | 58 million | 4,539 | 2,679 |
| The plugins' `dispatch` (`plugin_dispatch`) | 65 million | 9,025 | 3,221 |
