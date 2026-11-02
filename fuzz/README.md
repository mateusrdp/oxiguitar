# Fuzzing

The part of the core that takes what a plugin sends is fuzzed with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer): a malformed answer may be refused, but must never crash, hang or run out of memory. The file readers and the writer are fuzzed in their own repository, oxiguitar-guitarpro (#165).

This folder is its own Cargo workspace, outside the main one, and needs nightly Rust.

## Targets

| Target | What it reads | Seeds |
| --- | --- | --- |
| `plugin_tab` | a plugin's answer with a tab's contents (interface 1.1), as the core takes it: decoded, checked, and each canvas turned into a display list and drawn | `fixtures/plugin/*.answer`: the test plugin's tab, and the canvas line this target found |

Seeds come from the public fixtures only (`seeds.sh`), never from the private corpus.

## Running

```sh
cargo install cargo-fuzz
fuzz/seeds.sh                        # fills fuzz/corpus/<target>/
fuzz/run.sh plugin_tab 600                # ten minutes on every core
# or by hand:
cd fuzz && cargo +nightly fuzz run plugin_tab corpus/plugin_tab
```

`run.sh` uses libFuzzer's fork mode on every core, stops at the first crash, an input that takes over 30 s, or one that needs over 2 GB, and then shrinks the corpus (`cargo fuzz cmin`). An input that failed is in `fuzz/artifacts/<target>/`; `cargo fuzz run <target> <file>` replays it. Fix the reader and add the input to that reader's tests, so it stays fixed.

## In CI

`.github/workflows/fuzz.yml`:

- **Pull requests** that touch `oxiguitar-model`, `oxiguitar-display`, `oxiguitar-render`, the plugin interface or SDK, `fixtures/` or this folder: four minutes of every target.
- **By hand** (Actions → Fuzz → Run workflow, "minutes" per reader, 360 by default): the target `plugin_tab`, in two jobs of half the time, the second continuing from the first one's corpus (a job on a standard runner may take at most six hours). At most four fuzz jobs run at a time. The final corpora are kept as artifacts for 30 days.

A crash fails the job, its message becomes an error on the run, and the input is kept as the artifact "fuzz-crash \<target\>".
