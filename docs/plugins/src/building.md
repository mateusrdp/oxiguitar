# Building for every system

A plugin file holds native code for one system and processor, so you build one file for each system you support. OxiGuitar runs on six targets, which make five files (one macOS file serves both Mac processors):

| File | Rust target | Library cargo builds |
| --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `libLIBRARY.so` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `libLIBRARY.so` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `LIBRARY.dll` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `LIBRARY.dll` |
| macOS, universal | `aarch64-apple-darwin` and `x86_64-apple-darwin`, joined | `libLIBRARY.dylib` |

`LIBRARY` is your package's name with `_` for `-` (`example_plugin`).

## One build

The template's `build-oxp.sh` builds the release file for the system it runs on (on Windows, from Git Bash):

```sh
./build-oxp.sh
# dist/example-0.1.0-linux-x86_64.oxp
```

Change its `NAME` (your file's name) and `LIBRARY` (above) when you rename the plugin. What it does is one cargo command and a copy:

```sh
cargo rustc --release --features oxp --crate-type cdylib
cp target/release/libexample_plugin.so dist/example-0.1.0-linux-x86_64.oxp
```

with `libexample_plugin.dylib` on macOS and `example_plugin.dll` on Windows. On macOS it builds both processors and joins them with `lipo` into one universal file:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # once
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo rustc --release --features oxp --crate-type cdylib --target "$target"
done
lipo -create -output dist/example-0.1.0-macos-universal.oxp \
    target/aarch64-apple-darwin/release/libexample_plugin.dylib \
    target/x86_64-apple-darwin/release/libexample_plugin.dylib
```

The template's release profile (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`) keeps files small: our Guitar Pro plugins are 475 to 796 KB. Stripping keeps the entry point, which is exported. Check each file before you publish it: `oxiguitar-plugin-test dist/*.oxp` ([Testing](testing.md)).

## Every system, on GitHub

The template's `.github/workflows/build.yml` does all of this on GitHub's runners, each system on its own kind of machine, so nothing is cross-compiled but the Intel half of the macOS file. For each of the five files it:

1. runs `cargo test` on that system, the contract suite included;
2. runs `build-oxp.sh`, which names the file `NAME-VERSION-SYSTEM-ARCH.oxp` (`example-0.1.0-linux-x86_64.oxp`, `example-0.1.0-macos-universal.oxp`), the way OxiGuitar's own plugins are named;
3. keeps it as a workflow artifact.

When you push a tag starting with `v` (`git tag v0.1.0 && git push --tags`), a last job creates a GitHub release with the five files and a `SHA256SUMS` file. Users drop the file as it is into a plugin folder; the name doesn't matter to OxiGuitar.

The Linux files are built on Ubuntu 22.04, the oldest Ubuntu LTS still in standard support, so they need glibc 2.35 or newer, like OxiGuitar's own packages: a library built on a newer system needs that system's glibc and doesn't load on older ones.

## Before you release

- Raise the version in both `Cargo.toml` and `manifest.json`.
- Run the contract suite on each system's file, not only on yours (the workflow does).
- Try the file in OxiGuitar on at least one system ([Testing](testing.md)).
- Say in your README what the plugin does, which OxiGuitar it needs (0.2 or later for interface 1.1), what it downloads or sends anywhere (ideally nothing), and where to report its bugs.
