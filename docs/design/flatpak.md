# Flatpak (#163)

OxiGuitar and each of its plugins as Flatpak packages, built in CI and served from a Flatpak repository of our own on GitHub Pages, as the apt and dnf repositories are: a user who has the command or the address adds the repository once and installs the app and the plugins they want from it.

This document is the design as of session 40 (9 Nov 2026), with the spike that checks it (`spikes/flatpak/`, `.github/workflows/flatpak.yml`).

## Decided by Mateus (9 Nov 2026)

- **The app and each plugin are packaged individually** as Flatpaks, from a repository of our own, like the `.deb` packages.
- **No mirror of the runtime.** Our repository holds our packages only; the runtime (org.freedesktop.Platform) comes from Flathub's own repository, as for any Flatpak app. This replaces the roadmap's earlier plan of mirroring it in a repository of its own.
- **English only:** no translations, so the runtime's translations extension isn't needed either.
- **The Flatpak installs the PNG icons** (16 to 512 px); the SVG stays everywhere else (the `.deb`, `.rpm` and AppImage).

## What exists

- **Packages.** The core's `release.yml` builds the `.deb`, `.rpm` and AppImage (both builds, x86_64 and ARM64); each plugin's own repository calls oxiguitar-packages' `plugin-release.yml` and `plugin-publish.yml` (`@main`). oxiguitar-packages' Site workflow builds the apt and dnf repositories from **published** releases, signs them with our key and serves them on its Pages. Releases are made with the Release button.
- **Plugin folders** (`oxiguitar-host/src/folders.rs`). The system plugin folder is derived from the program's path: `PREFIX/bin/oxiguitar` gives `PREFIX/lib/oxiguitar/plugins`. Inside a Flatpak, `/app/bin/oxiguitar` gives `/app/lib/oxiguitar/plugins`. `inventory::plugin_files` lists the `.oxp` files directly in a folder with `Path::is_file`, which follows symbolic links.
- **Files.** The file dialogs use the XDG desktop portal (`rfd` with `xdg-portal`): inside a sandbox they open and save any file the user picks, with no filesystem permission.
- **Drawing.** Slint 1.17.1 with the femtovg renderer (OpenGL) and the software one; the Wayland build holds no X11 code (ADR 0009), the X11 build is deprecated.
- **Sound.** The core plays nothing itself: the audio-device plugins (ALSA, PipeWire) and the instruments are plugins. The MIDI plugin plays out of the box with the system's own sound set (`/usr/share/sounds/sf2`, `sf3`, `/usr/share/soundfonts`) and the sets in the user's `soundfonts` folder, where the sound-set installer (oxiguitar-bettersounds-soundfonts) puts them.

## The app

- Id `io.github.mateusrdp.OxiGuitar` (the desktop file, metainfo and icons already use it). Runtime `org.freedesktop.Platform` 26.08, from Flathub.
- **Built from source in the Freedesktop SDK** with Rust 1.99 from rustup, `cargo build --release --locked`. The build reaches the network (`--share=network`) for rustup and the crates: allowed in our own repository (Flathub would want the crates as listed sources). Built in the SDK, the program links against the runtime's own libraries (fontconfig, Wayland, libxkbcommon, Mesa).
- **The Wayland build only** (question 2).
- Installs `oxiguitar`, `oxiguitar-cli`, the desktop file, metainfo, MIME type, the PNG icons and LICENSE.

## Each plugin its own package, with no change to the core

Flatpak packages a plugin of an app as an *extension* of it: a package of its own, installed and removed on its own, that the app sees when it is there. The app declares one extension point:

```yaml
add-extensions:
  io.github.mateusrdp.OxiGuitar.Plugin:
    directory: lib/oxiguitar
    subdirectories: true
    merge-dirs: plugins
    no-autodownload: true
    autodelete: true
```

The plugin `NAME` is the package `io.github.mateusrdp.OxiGuitar.Plugin.NAME` (the `NAME` of `NAME.oxp` and of the `.deb`'s `oxiguitar-NAME`), mounted at `/app/lib/oxiguitar/NAME`, its files in `plugins/`. `merge-dirs` makes Flatpak link every installed plugin's files into `/app/lib/oxiguitar/plugins`, the system plugin folder OxiGuitar already derives, and `plugin_files` follows the links. So the core needs no Flatpak code; the spike checks it (the test plugin's package, found by `oxiguitar-cli detect` in the sandbox).

A plugin's package holds its released `.oxp` files for its architecture, unchanged (`gp3`, `gp4`, `gp5`, `gpx` and `gp` together in the guitarpro package; both NAM builds in the nam package, the plugin choice picking by CPU as it does elsewhere). Released `.oxp` files are built against an older glibc than the runtime's (plugin-release.yml's `glibc` input checks the highest version a plugin needs), so they run in it. Each carries a small metainfo file, so software centres list it as an add-on of OxiGuitar.

`no-autodownload` means no plugin comes with the app on its own: the user installs the ones they want, as with the `.deb` Suggests (the Recommends of the `.deb` have no Flatpak equivalent; the install instructions name them).

## Permissions

| Permission | Why |
| --- | --- |
| `--socket=wayland` | The window (Wayland build). |
| `--device=dri` | OpenGL for the femtovg renderer. |
| `--socket=pulseaudio` | The ALSA plugin: the runtime's ALSA library plays through the sound server. |
| `--filesystem=xdg-run/pipewire-0:ro` | The PipeWire plugin's socket. |
| `--filesystem=host-os:ro` | The system's own sound sets, read-only at `/run/host/usr/share/...` (question 1). |

No network, no home folder: files come through the portal. The app downloads and uploads nothing, as everywhere.

## Releases and the repository

Following the apt and dnf repositories (Site builds from published releases; nothing in a release depends on a script):

1. **The core's `release.yml`** builds the app in the SDK for x86_64 and ARM64 and attaches `OxiGuitar-VERSION-ARCH.flatpak` bundles (single-file Flatpaks, which `flatpak install` can also install directly) to the draft release.
2. **oxiguitar-packages' `plugin-release.yml`** builds each plugin's package from the `.oxp` files it just built and attaches `oxiguitar-NAME-VERSION-ARCH.flatpak` bundles.
3. **oxiguitar-packages' Site workflow** imports the bundles of every published release into one OSTree repository (`flatpak build-import-bundle`, which keeps each bundle's commit, so a rebuild from the same releases gives the same commits and clients see no false update), signs it with the key that signs apt and dnf (`flatpak build-sign`, `build-update-repo --gpg-sign`), and publishes it under `flatpak/` beside `apt/` and `rpm/`, with `oxiguitar.flatpakrepo` and `io.github.mateusrdp.OxiGuitar.flatpakref`, whose `RuntimeRepo` names Flathub's `.flatpakrepo` so the runtime comes from there.

The user adds our repository once:

```sh
flatpak remote-add --if-not-exists oxiguitar https://mateusrdp.github.io/oxiguitar-packages/flatpak/oxiguitar.flatpakrepo
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar.Plugin.guitarpro   # and the others
```

or opens the `.flatpakref`, which adds our repository (and Flathub, for the runtime, if it isn't there yet).

## Measured

The spike's run of 9 Nov 2026 (run 42587616431, both architectures green):

- **The runtime is org.freedesktop.Platform 26.08** (Flathub also has 25.08).
- **The app builds in the 26.08 SDK** with Rust 1.99 from rustup in about 7 minutes (x86_64; 5.5 on ARM64), and every library it needs is in the runtime. Its bundle is 13 MB (x86_64) and 11 MB (ARM64); the app and the test plugin's package take 19 MB and 18 MB in an OSTree repository, so our Pages site holds the app and every plugin easily.
- **Plugins as separate packages work with no change to the core.** In the sandbox `/app/lib/oxiguitar/plugins/test.oxp` is Flatpak's link to `/app/lib/oxiguitar/test/plugins/test.oxp`, and `oxiguitar-cli detect` says "read by the Test plugin plugin".
- **`host-os:ro` shows the host's `/usr/share/sounds` at `/run/host/usr/share/sounds`** (empty on a CI runner, which has no sound set).
- **appstreamcli compose refused the scalable SVG icon** ("Unrecognized image file format", also with a one-rectangle SVG, so not our drawing); hence the PNG icons.
- **The runtime from Flathub** is 282.7 MB to download for x86_64 (270.5 MB for ARM64) plus 150.5 MB (139.1 MB) for its GL drivers, once per machine and shared with every other Flatpak app using 26.08.

## Questions for Mateus

1. **Should the Flatpak play with sound sets already on the computer?** The sandbox doesn't see the system's `/usr/share/sounds` unless it is given read access to it (`host-os:ro`), and the MIDI plugin would then look there too (a small change in oxiguitar-midi). Nor does it see the sets the sound-set installer put in `~/.local/share/oxiguitar/soundfonts`: a Flatpak keeps its own folders under `~/.var/app/io.github.mateusrdp.OxiGuitar/`. Proposed: yes to both, read-only (the system's sets through `host-os:ro`, the installer's folder through `--filesystem=xdg-data/oxiguitar/soundfonts:ro`, which the MIDI plugin would also look in). Without it, a Flatpak user hears nothing until they copy a sound set into the Flatpak's own `soundfonts` folder.
2. **Is the Wayland build enough for the Flatpak?** Users on X11 desktops would use the `.deb`, `.rpm` or AppImage instead. The other way is both builds in the Flatpak with a small launcher choosing, as the AppImage does.
