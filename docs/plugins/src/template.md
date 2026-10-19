# Start from the template

The template plugin is a complete plugin you copy and change. It lives in OxiGuitar's repository as [`docs/plugins/template/`](https://github.com/mateusrdp/oxiguitar/tree/main/docs/plugins/template), and adds one of each common thing:

- a **format**, "Example tab" (`.extab`): a tiny text format of eighth notes on a guitar, read and written;
- a **tab**, "Fretboard", in the bottom area: a fretboard tuned like the open score's first track, where a click on a string names the note, with a setting remembered in settings.json;
- a **service**, `note-names`, which other plugins can call to name a pitch.

Each is one file in `src/`; you delete what you don't need.

## 1. Copy it

Copy the folder, hidden `.github/` included, to wherever you keep your projects. With git:

```sh
git clone --depth 1 --branch v0.2.0 https://github.com/mateusrdp/oxiguitar
cp -R oxiguitar/docs/plugins/template my-plugin
rm -rf oxiguitar
cd my-plugin
```

The template's `Cargo.toml` takes the SDK from the same tag, `v0.2.0`. To build for a later OxiGuitar, change both tags in `Cargo.toml`.

## 2. Build and test it

```sh
cargo test
```

The first run downloads and builds the SDK and the contract suite, which takes a minute. Then it runs:

- the template's own unit tests (`src/*.rs`);
- `the_manifest_matches_the_plugin`, a test `export_plugin!` adds: the manifest is valid, its version is the crate's, and every format, service and tab it declares is implemented, with contents the core can show;
- `tests/contract.rs`, which builds the plugin file `target/oxp/example.oxp` and checks it through OxiGuitar's own plugin host ([Testing](testing.md)).

## 3. Try it in OxiGuitar

Copy the file into your plugin folder (OxiGuitar creates it the first time it starts):

```sh
# Linux
cp target/oxp/example.oxp ~/.local/share/oxiguitar/plugins/
# macOS
cp target/oxp/example.oxp ~/Library/Application\ Support/OxiGuitar/plugins/
```

On Windows, copy `target\oxp\example.oxp` to `%APPDATA%\OxiGuitar\plugins\`.

Start OxiGuitar. Tools → Plugins lists "Example", not loaded yet. Open `samples/open-strings.extab` (File → Open): the plugin loads, reads it, and the footer says what its import log holds. The bottom area now has a **Fretboard** tab; click a string on it.

## 4. Make it yours

1. **Rename it.** In `Cargo.toml`, the package's `name`. In `manifest.json`: `id` (lowercase letters and digits joined by `.` or `-`, starting with your own name: `jane.chord-finder`), `name`, `description`, `authors`, `license` and `homepage`. In `build-oxp.sh`, `NAME` (your file's name) and `LIBRARY` (the package's name with `_` for `-`); in `.github/workflows/build.yml`, `NAME` again. In `tests/contract.rs`, the file's name.
2. **Delete what you don't need.** Each contribution is a module, a line in `src/lib.rs`'s `impl Plugin`, and an entry in `manifest.json`. Remove all three; `cargo test` says if the manifest and the code disagree.
3. **Write your own.** The next pages explain each part: [the manifest](manifest.md), [the SDK](sdk.md), [formats](formats.md), [tabs](tabs.md), [events](events.md), [services](services.md), [settings](settings.md).
4. **Keep the versions equal.** The version in `manifest.json` must be the crate's, and you raise both with each release: when two files of the same plugin are installed, OxiGuitar uses the higher version.

## What's in the folder

| File | What it is |
| --- | --- |
| `Cargo.toml` | The crate: the `oxp` feature, the SDK, the contract suite for tests, lints, a release profile for small files |
| `manifest.json` | What the plugin adds, read by OxiGuitar without loading the plugin |
| `src/lib.rs` | The plugin type, which hands out each contribution, and `export_plugin!` |
| `src/format.rs` | The Example tab format |
| `src/fretboard.rs` | The Fretboard tab |
| `src/service.rs` | The `note-names` service |
| `src/notes.rs` | Note names and standard tuning, shared by the last two |
| `tests/contract.rs` | Builds the `.oxp` file and runs the contract suite on it |
| `samples/open-strings.extab` | A file to open |
| `build-oxp.sh` | Builds the release file for this system, `dist/NAME-VERSION-SYSTEM-ARCH.oxp` ([Building for every system](building.md)) |
| `.github/workflows/build.yml` | Builds and tests the plugin for every system on GitHub, and attaches the files to a release |

Here is `src/lib.rs`, the whole plugin in one place:

```rust,ignore
{{#include ../template/src/lib.rs:plugin}}
```
