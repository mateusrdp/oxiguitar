# Flatpak (#163)

ROADMAP.md, "The way to v1.0", item 6: OxiGuitar as a Flatpak app and each plugin as an extension, built in CI, signed with our key and served as a static OSTree repository on GitHub Pages like the apt and dnf ones, with a `.flatpakrepo` file to add it; no third party until v1.0 is out and mature, so the runtime (org.freedesktop.Platform) is mirrored too, in a repository and Pages site of its own (oxiguitar-flatpak-runtime), with the source offer its GPL binaries need, checked against the runtime's own signatures before it is signed again with ours. The runtime's size against Pages' 1 GB site limit is checked first. Flathub is for after v1.0.

This document is the design as of session 40 (9 Nov 2026), with the spike that checks it (`spikes/flatpak/`, `.github/workflows/flatpak.yml`) and the decisions left to Mateus.

## What exists

- **Packages.** The core's `release.yml` builds the `.deb`, `.rpm` and AppImage (both builds, x86_64 and ARM64); each plugin's own repository calls oxiguitar-packages' `plugin-release.yml` and `plugin-publish.yml` (`@main`). oxiguitar-packages' Site workflow builds the apt and dnf repositories from **published** releases, signs them with our key and serves them on its Pages. Releases are made with the Release button.
- **Plugin folders** (`oxiguitar-host/src/folders.rs`). The system plugin folder is derived from the program's path: `PREFIX/bin/oxiguitar` gives `PREFIX/lib/oxiguitar/plugins`. Inside a Flatpak, `/app/bin/oxiguitar` gives `/app/lib/oxiguitar/plugins`. `inventory::plugin_files` lists the `.oxp` files directly in a folder with `Path::is_file`, which follows symbolic links. Of several files with one id the plugin choice takes the highest version, then more `cpu` features (the NAM plugin's x86-64-v3 build), then the later folder.
- **Files.** The file dialogs use the XDG desktop portal (`rfd` with `xdg-portal`): inside a sandbox they open and save any file the user picks, with no filesystem permission.
- **Drawing.** Slint 1.17.1 with the femtovg renderer (OpenGL) and the software one; the Wayland build holds no X11 code (ADR 0009), the X11 build is deprecated.
- **Sound.** The core plays nothing itself: the audio-device plugins (ALSA, PipeWire) and the instruments are plugins. The MIDI plugin plays out of the box with the system's own sound set (`/usr/share/sounds/sf2`, `sf3`, `/usr/share/soundfonts`) and the sets in the user's `soundfonts` folder, where the sound-set installer (oxiguitar-bettersounds-soundfonts) puts them.

## The app

- Id `io.github.mateusrdp.OxiGuitar` (the desktop file, metainfo and icons already use it). Runtime `org.freedesktop.Platform`, its newest `YY.08` branch; the spike finds it on Flathub.
- **Built from source in the Freedesktop SDK** with Rust 1.99 from rustup, `cargo build --release --locked`. The build reaches the network (`--share=network`) for rustup and the crates: allowed in our own repository, not on Flathub, which would want the crates as listed sources. Built in the SDK, the program links against the runtime's own libraries (fontconfig, Wayland, libxkbcommon, Mesa), not Ubuntu 22.04's.
- **The Wayland build only** (question 3).
- Installs `oxiguitar`, `oxiguitar-cli`, the desktop file, metainfo, MIME type, icons and LICENSE, as the `.deb` does.

## Plugins as extensions, with no change to the core

The app declares one extension point:

```yaml
add-extensions:
  io.github.mateusrdp.OxiGuitar.Plugin:
    directory: lib/oxiguitar
    subdirectories: true
    merge-dirs: plugins
    no-autodownload: true
    autodelete: true
```

The plugin `NAME` is the extension `io.github.mateusrdp.OxiGuitar.Plugin.NAME` (the `NAME` of `NAME.oxp` and `oxiguitar-NAME`), mounted at `/app/lib/oxiguitar/NAME`, its files in `plugins/`. `merge-dirs` makes Flatpak link every installed extension's `plugins/` files into `/app/lib/oxiguitar/plugins` (OBS Studio's plugins and the LinuxAudio base extension work this way), which is the system plugin folder OxiGuitar already derives, and `plugin_files` follows the links. So the core needs no Flatpak code at all; the spike checks it (the test plugin's extension, found by `oxiguitar-cli detect` in the sandbox).

An extension holds the plugin's released `.oxp` files for its architecture, unchanged (`gp3`, `gp4`, `gp5`, `gpx` and `gp` together in the guitarpro extension; both NAM builds in the nam extension, the plugin choice picking by CPU as it does elsewhere). Released `.oxp` files are built against an older glibc than any current runtime's (plugin-release.yml's `glibc` input checks the highest version a plugin needs), so they run in it. Each extension carries a small metainfo file so software centres list it as an add-on of OxiGuitar.

Building an extension needs the app installed (its runtime is the app), so a plugin's extension is built against the app's latest published release.

## Permissions

| Permission | Why |
| --- | --- |
| `--socket=wayland` | The window (Wayland build). |
| `--device=dri` | OpenGL for the femtovg renderer. |
| `--socket=pulseaudio` | The ALSA plugin: the runtime's ALSA library plays through the sound server. |
| `--filesystem=xdg-run/pipewire-0:ro` | The PipeWire plugin's socket. |
| `--filesystem=host-os:ro` | The system's own sound sets, read-only at `/run/host/usr/share/...` (question 2). |

No network, no home folder: files come through the portal. The app downloads and uploads nothing, as everywhere.

## Releases and the repositories

Proposed, following the apt and dnf repositories (Site builds from published releases; nothing in a release depends on a script):

1. **The core's `release.yml`** builds the app in the SDK for x86_64 and ARM64 and attaches `OxiGuitar-VERSION-ARCH.flatpak` bundles (single-file Flatpaks, which `flatpak install` can also install directly) to the draft release.
2. **oxiguitar-packages' `plugin-release.yml`** builds each plugin's extension from the `.oxp` files it just built and attaches `oxiguitar-NAME-VERSION-ARCH.flatpak` bundles.
3. **oxiguitar-packages' Site workflow** imports the bundles of every published release into one OSTree repository (`flatpak build-import-bundle`, which keeps each bundle's commit, so a rebuild from the same releases gives the same commits and clients see no false update), signs it with our key (`flatpak build-sign`, `build-update-repo --gpg-sign`), and publishes it under `flatpak/` beside `apt/` and `rpm/`, with `oxiguitar.flatpakrepo` and `io.github.mateusrdp.OxiGuitar.flatpakref` (whose `RuntimeRepo` names the runtime mirror's `.flatpakrepo`).
4. **oxiguitar-flatpak-runtime** (a new repository, made by `scripts/new-repo.sh`) mirrors the runtime: its workflow pulls `org.freedesktop.Platform` and `org.freedesktop.Platform.GL.default` at the branch the app uses from Flathub, **verifying Flathub's signature**, keeps the latest commit, signs it with our key, writes a `SOURCES.md` naming the freedesktop-sdk tag of each mirrored commit (the source offer: gitlab.com/freedesktop-sdk/freedesktop-sdk at that tag, and its source mirror), and publishes the static repository on its own Pages.

The user adds our repository once:

```sh
flatpak remote-add --if-not-exists oxiguitar https://mateusrdp.github.io/oxiguitar-packages/flatpak/oxiguitar.flatpakrepo
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar.Plugin.guitarpro   # and the others
```

or opens the `.flatpakref`, which adds both our repository and the runtime mirror.

## Measured

Filled in from the spike's run (job summaries of `Flatpak (spike)`): the runtime branch, each runtime ref's size on Flathub and mirrored, and our own app and extension sizes, per architecture.

## Open questions for Mateus

1. **The runtime mirror against the 1 GB limit.** Platform and GL for x86_64 and ARM64 together, with each update kept only as its latest commit (see "Measured"). If it doesn't fit: a site per architecture (two runtime repositories), or leaving GL to the host (not possible: Flatpak apps can't use the host's Mesa), or the software renderer only (no GL extension; slower drawing). The Platform's translations (`.Locale`) are optional and need not be mirrored.
2. **Sound sets.** With `host-os:ro` the sandbox sees the system's sound sets at `/run/host/usr/share/sounds/sf2` and so on; the MIDI plugin would look there too when it runs in a Flatpak (a small change in oxiguitar-midi, keeping "plays out of the box with the system's own sound set"). Without it, a Flatpak user hears nothing until they put a set in the `soundfonts` folder. And the sound-set installer writes to `~/.local/share/oxiguitar/soundfonts`, while the Flatpak's own folders are under `~/.var/app/io.github.mateusrdp.OxiGuitar/` (settings in `config/oxiguitar`, plugins and sets in `data/oxiguitar`). Proposed: the Flatpak keeps its own folders (the Flatpak way: no plugin built for the host mixed into the sandbox), and the installer gains a Flatpak target for that folder.
3. **X11.** The Wayland build only (X11 desktops use the `.deb`, `.rpm` or AppImage), or both builds with `--socket=fallback-x11` and a launcher choosing, as the AppImage does.
4. **Which plugins come with the app.** Flatpak has no "Recommends": an extension point either installs every extension the repository has (`no-autodownload: false`) or none. Proposed: none, and the install instructions and the `.flatpakref` page name the recommended ones (guitarpro, midi, pipewire; alsa too, though inside the sandbox it plays through the same sound server). The other way, every plugin installed with the app, costs the user about the size of all plugins together.
5. **The key.** Proposed: the GPG key that signs apt and dnf signs the Flatpak repository and the runtime mirror (one key for users to trust; the Site workflow already holds it).
6. **Runtime updates.** The runtime gets security updates between branches. Proposed: the mirror's workflow runs from a button like the Release one (no schedule), and a new runtime branch (once a year) is a change to the app's manifest, released like any other.
