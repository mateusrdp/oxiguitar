# Fuzzing

Every file reader is fuzzed with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer): a malformed file may fail to open, but must never crash, hang or run out of memory (FMT-1). M1's gate is six hours per reader without a crash.

This folder is its own Cargo workspace, outside the main one, and needs nightly Rust.

## Targets

| Target | What it reads | Seeds |
| --- | --- | --- |
| `guitar_pro_3_5` | Guitar Pro 3, 4 and 5 (.gp3, .gp4, .gp5) | the three test songs in `fixtures/gp/` |
| `guitar_pro_6` | Guitar Pro 6 (.gpx): BCFZ, BCFS and the GPIF inside | the .gpx test song |
| `guitar_pro_7` | Guitar Pro 7 and 8 (.gp): the ZIP archive and the GPIF inside | every .gp in `fixtures/` |
| `gpif` | the GPIF XML itself (and the XML tree under it), packed into a .gp archive by the target, with a dictionary of GPIF names (`gpif.dict`) | the score.gpif of every .gp in `fixtures/` |
| `any_file` | format detection (FMT-6), then whichever reader it picks | every file in `fixtures/` |
| `plugin_dispatch` | the file as each of our five format plugins answers the host's request to read it: the SDK's `dispatch`, the answer encoded and decoded as it crosses the plugin interface, and the score converted back into the model | every file in `fixtures/` |

The first three skip inputs that are not recognised as their format, so their time goes to their reader. Seeds come from the public fixtures only (`seeds.sh`), never from the private corpus.

## Running

```sh
cargo install cargo-fuzz
fuzz/seeds.sh                        # fills fuzz/corpus/<target>/
fuzz/run.sh gpif 600                 # ten minutes on every core
# or by hand:
cd fuzz && cargo +nightly fuzz run guitar_pro_7 corpus/guitar_pro_7
```

`run.sh` uses libFuzzer's fork mode on every core, stops at the first crash, an input that takes over 30 s, or one that needs over 2 GB, and then shrinks the corpus (`cargo fuzz cmin`). An input that failed is in `fuzz/artifacts/<target>/`; `cargo fuzz run <target> <file>` replays it. Fix the reader and add the input to that reader's tests, so it stays fixed.

`gpif.dict` is made from the names the reader looks for (the quoted names in `crates/oxiguitar-formats/src/gpif/`); add new ones when the reader learns them.

## In CI

`.github/workflows/fuzz.yml`:

- **Pull requests** that touch `oxiguitar-formats`, `oxiguitar-model`, the plugin interface or SDK, `plugins/`, `fixtures/` or this folder: four minutes of every target.
- **By hand** (Actions → Fuzz → Run workflow, "minutes" per reader, 360 by default): the four readers and `plugin_dispatch`, each in two jobs of half the time, the second continuing from the first one's corpus (a job on a standard runner may take at most six hours). At most four fuzz jobs run at a time. The final corpora are kept as artifacts for 30 days.

A crash fails the job, its message becomes an error on the run, and the input is kept as the artifact "fuzz-crash \<target\>".
