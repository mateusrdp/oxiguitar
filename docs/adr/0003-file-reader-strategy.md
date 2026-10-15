# 0003 — File readers: write our own, cross-check with the guitarpro crate

- Status: accepted
- Date: 2026-10-15
- Milestone: M0

## Context

The brief asked M0 to evaluate the `guitarpro` crate (slundi, MIT, now developed as "scorelib" on Codeberg) on our corpus and decide whether to adopt it, fork it or write our own readers.

What we measured on 15 October 2026, with guitarpro 0.4.3 and the private corpus (`corpus/MANIFEST.toml`):

- **guitarpro read 107 of the 109 Guitar Pro files** (GP3 to GP8) without error and without a panic. It failed on two: `dream-theater-hollow-years.gp5` ("unexpected EOF ... needed 100663296 bytes") and `scorpions-wind-of-change-11370381.gp3`.
- **Our new GP3–5 reader (this ADR's pull request) read all 89 GP3–5 files**, including those two. On 86 files it stops exactly at the end of the file, or at the four zero bytes GP3 and GP4 files usually end with; three files carry 500–2,500 bytes of extra data after the score, which we log rather than fail on. On 71 files every voice of every bar matches its time signature; the rest have a few bars too short or long, typical of hand-made transcriptions.
- **Where the two readers disagree on note counts, ours agrees with Guitar Pro 7.** The corpus holds Metallica's "Seek and Destroy" as both .gp5 and .gp. Our GP5 reader finds 9,339 notes; guitarpro's GP7 reader finds 9,340 in the .gp file; guitarpro's GP5 reader finds 8,946. On several GP3 files guitarpro's per-track counts differ from ours by a factor of two or more while both readers reach the end of the file, so the difference is in what it keeps, not in what it parses.
- guitarpro converts into its own PyGuitarPro-style model (`Song`, `MeasureHeader`, `Measure`), not ours. It keeps no record of what it could not represent, which FMT-2's import log needs. It has one main author, a 0.x API, and moved repository in 2026.

## Options

1. **Adopt guitarpro as a dependency** and convert its `Song` into our model. The adapter would be about as large as a reader, would inherit its gaps silently, and ties us to its release cycle.
2. **Fork it.** We would own 39,000 lines of someone else's design and still need the adapter and the import log.
3. **Write our own readers** straight into our model, using PyGuitarPro, alphaTab and TuxGuitar as documentation and guitarpro as a second opinion.

## Decision

Option 3. `oxiguitar-formats` has its own readers, starting with Guitar Pro 3, 4 and 5. guitarpro is not a dependency. We keep it as a **cross-check** when running the private corpus locally: where its results and ours differ, we look at the file and find out who is right.

Our readers are written for untrusted input from the start: bounds-checked reads, counts checked against the bytes left, errors that give the byte offset and the bar and track being read, and clippy lints that forbid `unwrap`, `expect`, `panic!` and unchecked indexing in the crate.

## Consequences

- We write the GPX (BCFZ/BCFS) and GP7/8 (ZIP + GPIF XML) readers next, in M1. They will need the `zip` and `quick-xml` crates, which are pure Rust and permissively licensed.
- MuseScore, MusicXML and ABC readers follow once more sample files arrive, as planned. guitarpro's MuseScore support can serve as a cross-check there too.
- No code is copied from guitarpro or the other readers; if we ever port code, the file gets the SPDX header and "Ported from" note our rules require.
- Revisit if keeping our readers current with new Guitar Pro releases becomes a burden that a shared library would carry better.
