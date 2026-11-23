# Testing

Every test of this repository runs with one command, on your own machine, and anyone can run it. When something fails, it tells you how to report it.

## Run everything

```sh
python3 scripts/test-all.py --corpus /path/to/your/files
```

On Windows, type `python` instead of `python3` if that's what you have.

You need:

- Rust, from [rustup](https://rustup.rs) (the version in `Cargo.toml`'s `rust-version` or newer);
- git, and Python 3.8 or newer (nothing to install with pip);
- on Linux, the libraries a build needs: `libfontconfig1-dev`, `libxkbcommon-dev` and `libwayland-dev` on Debian and Ubuntu (`fontconfig-devel`, `libxkbcommon-devel` and `wayland-devel` on Fedora);
- for the plugin template stage: `bash`.
- for the no network stage: `bash`, `strace`, `weston`, `Xvfb`, `curl`.

It runs:

1. every Rust test of the repository (`cargo test` over the whole workspace), with a progress bar;
1. the plugin contract tests that use OxiGuitar's released plugins (`cargo test -p oxiguitar-plugin-test --features released`);
1. a plugin built from the template as the plugin guide says, and checked (`scripts/check-plugin-template.sh`);
1. on Linux, a whole session of the app and the command-line program under strace, failing at any attempt to open an internet connection (`scripts/test-no-network.sh`; needs strace, weston and Xvfb);
1. the speed budgets (`cargo test --release -p oxiguitar-app speed`): advisory, since they depend on the machine;
1. a check of the [Features](features.md) page: every feature names a test, and every test it names exists.

It tests the system and processor it runs on, and only those (as Rust sees them: `rustc -vV`'s host). Tests for other systems are left out and the summary says so; they are not failures.

## Your corpus

The corpus is any folder of files you have (searched recursively, hidden files left out): give it with `--corpus DIR`, or set `OXIGUITAR_CORPUS=DIR`. Each corpus test takes the files it understands and skips the rest, and only checks what must hold for every file (it opens, nothing panics, a round trip is stable), so your corpus doesn't have to be ours to pass. Files are never uploaded, and never belong in the repository.

`cargo test` on its own uses `OXIGUITAR_CORPUS` too, or the repository's ignored `corpus/files/` folder, and quietly checks nothing when neither is there (that's how CI runs). `--no-corpus` runs the script without one.

## The plugins the tests use

OxiGuitar reads no file format by itself, so the tests that open files use OxiGuitar's released Guitar Pro plugins, and a few use its other released plugins. They fetch them from GitHub's releases once, checked against each release's SHA256SUMS. To test with plugins you built yourself instead, point `OXIGUITAR_PLUGINS_DIR` at the folder their `.oxp` files are in. (OxiGuitar itself never downloads anything: only the tests do.)

## What you get

A line per stage while it runs, then the result. In `target/test-all/<date>-<time>/`:

- `summary.md`: the result, your system, what failed and why, short enough to paste into a GitHub issue;
- `test-all.log`: everything that happened, every command and its output, and your corpus's file names, sizes and SHA-256 sums;
- `corpus-*.json`: what each corpus test made of each file.

The script exits with 0 when everything passed, 1 when something failed, and 2 when it couldn't start.

## When something fails

1. Update to the newest `develop` (`git checkout develop && git pull`) and run it again: it may be fixed already.
2. Look for an issue about the same test at [github.com/mateusrdp/oxiguitar/issues](https://github.com/mateusrdp/oxiguitar/issues).
3. Otherwise [open one](https://github.com/mateusrdp/oxiguitar/issues/new): call it `test-all: <the first failing test>`, paste the whole `summary.md`, and attach `test-all.log` (drag it onto the text box; zip it first if it's over 25 MB). Read the log first: it has your folder names and your corpus's file names, so change anything you'd rather keep private.
4. Don't attach corpus files unless they're yours to share. The summary lists each failing file's SHA-256, which is often enough for us to find the same file. If a failing file is yours, attaching it helps a lot.
5. You can also give `summary.md` and `test-all.log` to a language model along with this repository and ask it to find the cause.

## Options

- `--release`: an optimised test build: slower to build, faster corpus tests.
- `--ignore-rust-version`: for a Rust older than `rust-version` (unsupported, but handy).
- `--out DIR`: where the results go.

## Adding a feature

A new feature comes with its tests and a row on the [Features](features.md) page, in the same pull request; a new corpus test uses `tests/support/corpus.rs`. The script fails until the row names tests that exist.

## For every OxiGuitar repository

Each OxiGuitar repository, plugins included, has the same `scripts/test-all.py` (change it in all of them at once) and `tests/support/corpus.rs`, and describes itself in `tests/suite.json`. `scripts/new-repo.sh` starts a repository with all three and an empty Features page.

### The Features page

`docs/manual/src/features.md` holds tables; in each row, the Tests column (the last one, or the one headed Tests) names tests between backticks:

- `undo_restores_the_score`: a test of that name in any test binary; `oxiguitar-edit/entry::undo_restores_the_score` or `lib::tests::undo_restores_the_score` narrow it down (a test's full name is `package/target::path`, `lib` for the library's own tests, `doc` for doc tests);
- `fingering::tests::*`: every test starting so (at least one must exist);
- `stage:NAME`: a stage of `tests/suite.json`;
- `linux:...`, `windows,macos:...`, `aarch64:...`: tests that only exist on those systems or processors, not looked for elsewhere.

A row that names no test, a test that doesn't exist on this system, or a page with no row fails the run, and CI's Features page step.

### tests/suite.json

- `repository`: `owner/name`, for the links in the summary.
- `os`, `arch`: the systems (`linux`, `windows`, `macos`) and processors (`x86_64`, `aarch64`) the repository is for; elsewhere the script says there is nothing to test and passes. The same two keys work on every stage, setup step and plugin.
- `cargo`: `enabled` (default true when there is a Cargo.toml), `args` (default `["--workspace"]`), `exclude`, `features`, `locked` (default true), `test_args` (after `--`).
- `corpus`: `uses` (the corpus tests read `OXIGUITAR_CORPUS`), and for a file format plugin `through_core`: `extensions` it reads, `write_as` (extensions to convert each file to, three times over, the second and third results compared), `readers` (released plugin repositories, such as `guitarpro`, that read the other files to write them as these).
- `plugins`: each plugin file to build and check, with `name`, `package`, `library`, `manifest`, `features` (default `oxp`) and `with` (environment variables naming the `.oxp` files of plugins it needs, for `oxiguitar-plugin-test --with`). Their presence makes the script fetch and build the newest OxiGuitar release.
- `setup`: commands run before the build, each with `name` and `run`; `env_from_stdout` puts the last line printed into that environment variable (a step whose variable is already set is skipped, so you can give your own; `--features-only` runs none).
- `stages`: commands run after the tests, each with `name`, `run` (a list), `doc` (a line for the Testing page), and as needed `needs` (programs that must be installed, or the stage is skipped), `ready` (`{"run": [...], "why": "..."}`: a quick command that must succeed, such as `docker info`, or the stage is skipped with `why`), `when: "corpus"`, `env`, and `advisory` (a failure is reported but doesn't fail the run, for a speed budget).

Each repository's Testing page lists what its own run does.
