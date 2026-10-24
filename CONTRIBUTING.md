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
- **Crates Debian has.** OxiGuitar aims for Debian's main archive, which builds only from crates Debian packages ([docs/debian.md](docs/debian.md)). Given a choice, take a crate and a version Debian unstable has; the Debian workflow's weekly audit shows which.
- **Formatting and lint:** `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
- **No panics on user input.** Readers return errors that name the file and the byte position or element where things went wrong (FMT-1). Reader crates deny `unwrap`, `expect`, `panic!` and unchecked indexing outside tests.
- **No `unsafe`.** The workspace forbids it. The one exception is the plugin interface (ADR 0008): the host's loader (`crates/oxiguitar-host/src/loader.rs`) and the SDK's export glue (`crates/oxiguitar-plugin/src/glue.rs`, with the two attributes `export_plugin!` generates). Their crates deny `unsafe_code` and allow it in that one module; every `unsafe` block carries a `// SAFETY:` comment saying why it is sound, and changes to either file are reviewed line by line. A test (`crates/oxiguitar-plugin-test/tests/unsafe_audit.rs`) fails if `unsafe` is allowed anywhere else. Plugin crates deny it too (not forbid, so `export_plugin!` can allow its attributes). If something else truly needs `unsafe`, open a discussion first.
- **Document public items.** `missing_docs` is on.
- **Minimum Rust version:** 1.99 (`rust-version` in `Cargo.toml`), built with Rust from [rustup](https://rustup.rs) rather than a distribution's package (Mateus, 24 Oct 2026). CI tests that version and the latest stable. Raise it only when something needs it, in a pull request of its own: Debian's archive builds with the rustc Debian unstable has (1.97 in October 2026), so before OxiGuitar can be uploaded there, unstable must have caught up with ours ([docs/debian.md](docs/debian.md)).
- **Exact time.** Durations are exact fractions; never use floating point for musical time.
- **Edits are commands.** Every change to a score goes through a reversible command, so undo and redo always work.
- **Privacy.** No telemetry, analytics, crash uploads or background requests, ever. Network code may run only after the user asks for a download. CI enforces this (NET-5).

## Known platform gaps

Features that don't work on one system because of a library we build on, with the ADR that explains why and what has to change. Reviewers: check this list whenever winit or Slint is upgraded, and when a pull request touches one of these features.

| Feature | Missing on | Why | Enable it when | ADR |
| --- | --- | --- | --- | --- |
| Drop a file on the window to open it | Wayland | winit 0.30 has no drag and drop on Wayland, and Slint 1.18 passes no OS drops on. A workaround would need `unsafe` code and polling, which we rejected | winit's Wayland backend emits `DroppedFile` (no code change needed), or Slint forwards OS drops. Test with `scripts/wayland-dnd-test/` | [0006](docs/adr/0006-wayland-file-drops.md) |
| Double-click a file in Finder (or Open With, or drop on the Dock icon) to open it | macOS | macOS sends these as Apple Events to the application delegate; winit 0.30 and Slint 1.18 report none, and a delegate of our own would need `unsafe` Objective-C glue, which we rejected | winit reports opened files on macOS and Slint passes the event on, or Slint offers an "open file" callback | [0007](docs/adr/0007-macos-open-document-events.md) |

## Design principles

These come from Mateus's decisions and apply to every pull request.

- **No polling, ever** (Mateus, 17 and 18 Oct 2026). Nothing may wake up on a timer to check whether something happened: not a file, a queue, a socket or another library's state. Code reacts to events the operating system or a library delivers, or it waits on a channel, a condition or a file descriptor that wakes it. A one-shot timer for something that really is about time (a message that fades after ten seconds, a double-click window) is not polling. If the only way to learn about an event is to poll, the feature waits for upstream and the gap is documented, as ADRs 0006 and 0007 show.
- **Wayland first** (Mateus, 19 Oct 2026; ADR 0009). When the Wayland and X11 builds pull different ways, the Wayland build gets the clean, native solution; fallbacks and workarounds belong in the X11 build, which will be dropped. The Wayland build holds no X11 code, and CI checks it (`scripts/check-display-crates.sh`). Test on Wayland first (headless Weston), then X11.
- **No hacks.** A workaround that races another library's event loop, or needs `unsafe` to borrow its internals, is not acceptable. Document the gap, say what upstream has to change, and list it under "Known platform gaps" above.
- **As fast as it can be.** Start-up, opening a file and drawing are measured (`docs/testing.md`, "Speed budget"). Do work when it is needed, not up front, and off the UI thread.
- **Elegant over clever.** Code is read far more often than written: prefer the plain, obvious shape that a new contributor can follow and maintain.
- **Don't be too smart** (Mateus, 16 Oct 2026). Show what the file says; anything that changes it is an explicit action the user picks.

## File headers

Every source file we write starts with:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
```

The plugin interface and the SDK (`crates/oxiguitar-plugin-api`, `crates/oxiguitar-plugin`) are the exception: they are MIT OR Apache-2.0, so that plugins built on them may use any GPL-compatible licence, and their files start with `// SPDX-License-Identifier: MIT OR Apache-2.0`. Code from the GPL crates must never be copied into them.

A file ported or adapted from another project keeps that project's license and copyright, and says where it came from:

```rust
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
// Copyright (C) MuseScore Limited and others
// Ported from MuseScore src/engraving/layout/v0/beamlayout.cpp at <commit>
```

Add the project to [CREDITS.md](CREDITS.md) in the same pull request. Nothing may be copied from Guitar Pro: no code, icons, fonts, sounds or sample songs.

## Test files

[docs/testing.md](docs/testing.md) describes the kinds of tests and the quality gates (speed budget, visual regression, fuzzing).

- **Public fixtures** (`fixtures/`) are files we made ourselves or that carry a license allowing redistribution. Each one is listed in `fixtures/README.md` with its origin.
- **The private corpus** — community transcriptions of unclear ownership — is never committed. `corpus/MANIFEST.toml` lists names and checksums only. Never add such a file to the repository, even temporarily.

### Checking what OxiGuitar writes, in Guitar Pro and elsewhere

`fixtures/guitar-pro-check/` holds the .gp files OxiGuitar's writer makes of every reference song and of the test song in every format, with what each conversion lost (`conversions.txt`). They are there so anyone can open them in the program and version they have, on the system they have, and report what looks wrong. That covers Guitar Pro 7 or 8, older and newer releases, Windows, macOS, Wine, and other programs that read .gp (MuseScore, TuxGuitar, alphaTab, Songsterr). Each file says what it tests in its score information (File → Score Information in Guitar Pro, under Instructions), and `fixtures/reference/README.md` lists them all.

When you check them:

- Say which program, version and system you used, and which file and bar looks wrong. A screenshot helps, and so does the file as your program saves it again.
- A file that tests how OxiGuitar shows a score (its layouts, paper size, dark page, the tracks shown) looks ordinary elsewhere. Its instructions say so.
- Known differences that are other programs' bugs, not ours, are listed in `fixtures/guitar-pro-check/README.md`.

After changing the writer or the reference songs, run `scripts/guitar-pro-check.sh` to write the files again; a test (`cargo test -p oxiguitar-testgen --test guitar_pro_check`) fails until they match the writer. Every test build also attaches them as "guitar-pro-check".

## Adding a file format

Every format is its own plugin; the core recognises no format itself (docs/design/plugins.md). The Guitar Pro plugins in `plugins/gp3` … `plugins/gp` show the shape.

1. Write down what you learn about the format in `docs/formats/<format>.md`.
2. Add a reader to `oxiguitar-formats` behind a Cargo feature of its own, so its plugin carries only its code. It recognises the format from its contents (FMT-6), never from the extension, fills the score model, and records anything it cannot represent in the import log (FMT-2).
3. Add the plugin: `plugins/NAME/`, a crate `oxiguitar-NAME-plugin` with the feature `oxp = []`, depending on `oxiguitar-formats` with only that reader's feature and `plugin`, and a `manifest.json` whose signatures recognise the format without loading the plugin (the host picks a plugin by them; the extensions only decide which report a refused file asks for). Add it to `OURS` in `crates/oxiguitar-plugin-test/src/lib.rs`; `scripts/build-plugins.sh` finds it by its folder.
4. Add small public fixtures and tests; `cargo test -p oxiguitar-plugin-test --test ours` checks the plugin against the contract and the in-process reader. Run the private corpus locally if you have it.
5. Writers come with round-trip tests (FMT-4) and loss warnings for lossy formats (FMT-3).
6. A new fuzz target for the reader, and the plugin in `fuzz/fuzz_targets/plugin_dispatch.rs`.
7. If the plugin joins the basic set we publish, it goes into each system's plugins zip (`OxiGuitar-plugins-basic-VERSION-SYSTEM-ARCH.zip`) and the `oxiguitar-plugins-basic` .deb and .rpm by itself; raise the plugin counts the release workflow checks (`.github/workflows/release.yml`) and add it to the list in `scripts/package/plugins-README.txt`. Releases attach no plugin on its own (docs/design/plugins.md, "Distribution").

## Adding an effect

1. Add it to the model, attached to the beat or note it acts on.
2. Teach each reader and writer about it, or record in the import log that they can't.
3. Later milestones add its drawing (layout) and its sound (playback).

## Decisions

A significant decision — a dependency, a format strategy, a design rule — gets a short record in `docs/adr/`, numbered in order. Copy `docs/adr/0000-template.md`.
