# Private test corpus

The private corpus is a collection of free community transcriptions (from CifraClub and Ultimate Guitar) and other files of unclear ownership. We test our readers against it, but **the files are never committed to this repository**, not even temporarily.

This folder holds only `MANIFEST.toml`: each file's name, real format (detected from its contents, not its extension), size, SHA-256 checksum and source. The checksums let anyone who has the same files confirm they are testing the same thing.

Any corpus works, ours or yours: a folder of files, searched recursively. Run every test on it with `python3 scripts/test-all.py --corpus DIR` (the manual's Testing page says more), or give it to `cargo test` as `OXIGUITAR_CORPUS=DIR`. Putting the files in `corpus/files/` (ignored by git) works too. Each corpus test takes the files it understands, skips the rest, and checks only what must hold for any file, so a corpus other than ours passes as well; when one doesn't, the script's summary says how to report it.
