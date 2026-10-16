# Contributing to OxiGuitar

Thank you for helping. Pull requests are welcome from anyone; the maintainer, Mateus Rocha de Paula ([@mateusrdp](https://github.com/mateusrdp)), reviews them.

Before a large change, open an issue or a discussion so we can agree the approach. [ROADMAP.md](ROADMAP.md) shows what is planned, [ARCHITECTURE.md](ARCHITECTURE.md) how the code fits together, and [docs/adr/](docs/adr/) why.

## Branches and pull requests

- `develop` is where work lands. Branch from it (`feat/gp5-reader`, `fix/tie-crash`) and open your pull request against it.
- `main` holds reviewed milestones. Only the maintainer merges into it and tags releases.
- Feature branches are squash-merged into `develop`; `develop` is merged into `main` with a merge commit.
- CI must be green. The `ci-success` check sums up every other job.
- One pull request carries the code, its tests, its documentation and any credit updates together.

## Sign-off (Developer Certificate of Origin)

We use the [Developer Certificate of Origin](https://developercertificate.org/) instead of a contributor license agreement. By signing off a commit you state that you wrote it, or otherwise have the right to submit it under the project's license. Sign off with `-s`:

```sh
git commit -s -m "Read GP5 bends"
```

This adds a line such as `Signed-off-by: Your Name <you@example.com>`, which must match the commit's author email. To fix a branch afterwards: `git rebase --signoff develop`, then force-push your branch.

Commits made by the Claude GitHub app, which writes much of the code with the maintainer, carry a `Co-Authored-By: Claude` trailer instead. The DCO is a statement by a person, so the check skips them, and also skips GitHub's squash merges of those pull requests into `develop`, which GitHub attributes to the account that merged them. The maintainer's review and merge of the pull request into `main` is the human sign-off for that work.

You keep the copyright on what you contribute. Your name goes in [CREDITS.md](CREDITS.md) with your first merged contribution.

## Code standards

- **Rust only.** Every line we write is Rust and every dependency is a Rust crate. The only exceptions are the operating system's own libraries for windows, graphics and audio. C and C++ projects are references to read and port from, never libraries to link.
- **Formatting and lint:** `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
- **No panics on user input.** Readers return errors that name the file and the byte position or element where things went wrong (FMT-1). Reader crates deny `unwrap`, `expect`, `panic!` and unchecked indexing outside tests.
- **No `unsafe`.** The workspace forbids it. If something truly needs it, open a discussion first.
- **Document public items.** `missing_docs` is on.
- **Minimum Rust version:** the rustc in Debian unstable, currently 1.97. Don't use newer language or library features; CI tests that version and the latest stable.
- **Exact time.** Durations are exact fractions; never use floating point for musical time.
- **Edits are commands.** Every change to a score goes through a reversible command, so undo and redo always work.
- **Privacy.** No telemetry, analytics, crash uploads or background requests, ever. Network code may run only after the user asks for a download. CI enforces this (NET-5).

## File headers

Every source file we write starts with:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
```

A file ported or adapted from another project keeps that project's license and copyright, and says where it came from:

```rust
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
// Copyright (C) MuseScore Limited and others
// Ported from MuseScore src/engraving/layout/v0/beamlayout.cpp at <commit>
```

Add the project to [CREDITS.md](CREDITS.md) in the same pull request. Nothing may be copied from Guitar Pro: no code, icons, fonts, sounds or sample songs.

## Test files

- **Public fixtures** (`fixtures/`) are files we made ourselves or that carry a license allowing redistribution. Each one is listed in `fixtures/README.md` with its origin.
- **The private corpus** — community transcriptions of unclear ownership — is never committed. `corpus/MANIFEST.toml` lists names and checksums only. Never add such a file to the repository, even temporarily.

## Adding a file format

1. Write down what you learn about the format in `docs/formats/<format>.md`.
2. Teach `oxiguitar-formats` to recognise it from its contents (FMT-6) — never from the extension.
3. Add a reader that fills the score model and records anything it cannot represent in the import log (FMT-2).
4. Add small public fixtures and tests; run the private corpus locally if you have it.
5. Writers come with round-trip tests (FMT-4) and loss warnings for lossy formats (FMT-3).

## Adding an effect

1. Add it to the model, attached to the beat or note it acts on.
2. Teach each reader and writer about it, or record in the import log that they can't.
3. Later milestones add its drawing (layout) and its sound (playback).

## Decisions

A significant decision — a dependency, a format strategy, a design rule — gets a short record in `docs/adr/`, numbered in order. Copy `docs/adr/0000-template.md`.
