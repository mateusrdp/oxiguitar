# Private test corpus

The private corpus is a collection of free community transcriptions (from CifraClub and Ultimate Guitar) and other files of unclear ownership. We test our readers against it, but **the files are never committed to this repository**, not even temporarily.

This folder holds only `MANIFEST.toml`: each file's name, real format (detected from its contents, not its extension), size, SHA-256 checksum and source. The checksums let anyone who has the same files confirm they are testing the same thing.

To run corpus tests locally, put the files in `corpus/files/` (ignored by git) and run the commands in the manifest's header.
