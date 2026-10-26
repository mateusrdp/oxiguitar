# An OxiGuitar plugin

A template for [OxiGuitar](https://github.com/mateusrdp/oxiguitar) plugins. Copy this folder, rename a few things, and you have a plugin that builds, passes OxiGuitar's contract suite, and opens in the app. The [plugin developer guide](https://mateusrdp.github.io/oxiguitar/plugin-guide/) explains every part.

As it stands, the plugin adds three things, each in its own file:

| File | What it adds | Manifest entry |
| --- | --- | --- |
| `src/format.rs` | Opens and saves "Example tab" files (`.extab`), a small text format: one guitar track of eighth notes. Try `samples/open-strings.extab`. | `formats` |
| `src/statistics.rs` | A "Statistics" tab in the right area: the notes on each string of a track of the open score, as a small chart. Click a bar for its count. | `tabs`, `events` |
| `src/service.rs` | The service `note-names`, which other plugins can call to name a pitch. | `services` |

`src/lib.rs` ties them together and exports the plugin; `src/notes.rs` holds note names and standard tuning.

## Make it yours

1. Copy this folder (with its hidden `.github/` folder) somewhere of your own, and make it a git repository if you like.
2. In `Cargo.toml`, rename the package (`example-plugin`). In `manifest.json`, change the `id` (`yourname.example`: lowercase letters and digits, starting with your own name), `name`, `description`, `authors`, `license` and `homepage`. In `build-oxp.sh`, change `NAME` (the file's name) and `LIBRARY` (the package's name with `_` for `-`), and `NAME` in `.github/workflows/build.yml`. In `tests/contract.rs`, the file's name.
3. Delete the parts you don't need: the module, its line in `src/lib.rs`'s `Plugin` impl, and its entry in `manifest.json`. `cargo test` tells you if the two disagree.
4. Keep the version in `manifest.json` equal to the one in `Cargo.toml`.

The template is MIT OR Apache-2.0, like OxiGuitar's plugin SDK; your plugin may use any GPL-compatible licence.

## Build and test

You need Rust 1.99 or later for the SDK of v0.9.0, which the template uses, from [rustup](https://rustup.rs).

```sh
cargo test
```

That runs the unit tests, a check that `manifest.json` matches the code, and `tests/contract.rs`, which builds `target/oxp/example.oxp` and checks it through OxiGuitar's own plugin host.

For the release file for your system:

```sh
./build-oxp.sh
```

It writes `dist/example-VERSION-SYSTEM-ARCH.oxp` (on macOS, one universal file for both processors; on Windows, run it from Git Bash). The workflow in `.github/workflows/build.yml` runs it for all six targets on GitHub and attaches the files to a release when you push a tag `v*`.

## Try it in OxiGuitar

Put the `.oxp` file (from `dist/`, or `target/oxp/example.oxp` after `cargo test`) in your plugin folder and start OxiGuitar:

| System | Plugin folder |
| --- | --- |
| Linux | `~/.local/share/oxiguitar/plugins/` |
| macOS | `~/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `%APPDATA%\OxiGuitar\plugins\` |

Tools → Plugins lists it. Open `samples/open-strings.extab`, and look at the Statistics tab in the right area.

## The Example tab format

```text
EXAMPLE TAB
title: Open Strings
artist: Your Name
tempo: 96
# One bar per line: eight eighth notes, each a string and a fret
# (string 1 is the highest), several joined by + for a chord, or - for
# a rest. A bar with fewer than eight is filled with rests.
6:0 5:0 4:0 3:0 2:0 1:0 - -
```

The first line must be `EXAMPLE TAB`: that is how OxiGuitar recognises the file without loading the plugin.
