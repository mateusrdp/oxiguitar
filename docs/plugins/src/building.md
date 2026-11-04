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

The Linux files are built on Ubuntu 22.04, the oldest Ubuntu LTS still in standard support, so they need glibc 2.35 or newer, like OxiGuitar's AppImage, and load with every OxiGuitar package: a library built on a newer system needs that system's glibc and doesn't load on older ones.

## Packages and installers (the official plugins' workflow)

The official plugins live in repositories of their own and are released with two reusable workflows from the [`oxiguitar-packages`](https://github.com/mateusrdp/oxiguitar-packages) repository, which any plugin repository may call. They live there, apart from OxiGuitar, so a change to the packaging never waits for an OxiGuitar release: call them `@main`. `plugin-release.yml` runs the same five builds as above (it expects the template's `build-oxp.sh`, which writes `dist/NAME-VERSION-SYSTEM-ARCH.oxp`) and then makes, from those files:

- a zip per target, and `SHA256SUMS`;
- a `.deb` and an `.rpm` per Linux processor, named `oxiguitar-NAME`, that install the file in `/usr/lib/oxiguitar/plugins/`;
- a Windows installer per processor (Inno Setup), which installs for the user alone, without administrator rights, in `%APPDATA%\OxiGuitar\plugins`, and registers an uninstaller;
- a macOS `.pkg` (`pkgbuild` and `productbuild` with `enable_currentUserHome`), which installs into the user's own `~/Library/Application Support/OxiGuitar/plugins` without a password.

A repository may hold several plugins, as oxiguitar-guitarpro holds five: its `build-oxp.sh` writes one `dist/FILE-VERSION-SYSTEM-ARCH.oxp` for each, `name` is then the package's name (`guitarpro`, for `oxiguitar-guitarpro`), and the zip, the packages and the installers hold every file, each installed as `FILE.oxp`. Give `package` the crate whose version is the release's when the workspace has several crates.

Each package is installed and removed in its own kind of system (Debian, Fedora, Windows, macOS) before the release is made. For a pushed `v*` tag the workflow makes a draft release holding all of it; the maintainer publishes it. On a branch or a pull request nothing is released, and the files stay under the run for 14 days, with a `~dev` version.

```yaml
jobs:
  release:
    uses: mateusrdp/oxiguitar-packages/.github/workflows/plugin-release.yml@main
    with:
      name: example
      title: Example
      description: "What it does, in one line."
      # Optional: depends-deb/-rpm and suggests-deb/-rpm, such as
      # suggests-deb: oxiguitar-bettersounds-soundfonts
    permissions:
      contents: write
```

`plugin-publish.yml` is called on `release: published`. It checks that the release holds every file and sum, then sends a `plugin-published` event to the package repositories (when the secret `PACKAGES_TOKEN` is set) and, when you give it a `winget-id`, opens the pull request on winget-pkgs (`WINGET_TOKEN`). The signing of the package repositories lives in its own repository. Nothing is signed with an Apple or Microsoft certificate: macOS asks for "Open Anyway" once for the app, and a plugin installed by the `.pkg` is not quarantined.

You can use the scripts without the workflows: oxiguitar-packages' `scripts/plugin-package/` holds `deb.sh`, `rpm.sh`, `windows.sh` (with `windows.iss`) and `macos-pkg.sh`, each documented at its top, and `specs.sh`, which turns a folder of built files into their arguments.

## Before you release

- Raise the version in both `Cargo.toml` and `manifest.json` (the official plugins' Release button, in oxiguitar-packages, does it for them).
- Run the contract suite on each system's file, not only on yours (the workflow does).
- Try the file in OxiGuitar on at least one system ([Testing](testing.md)).
- Say in your README what the plugin does, which OxiGuitar it needs (0.2 or later for interface 1.1), what it downloads or sends anywhere (ideally nothing), and where to report its bugs.
