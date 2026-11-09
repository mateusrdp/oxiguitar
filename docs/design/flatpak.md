# Flatpak (#163)

OxiGuitar and each of its plugins as Flatpak packages, built in CI and served from a Flatpak repository of our own on GitHub Pages, as the apt and dnf repositories are: a user who has the command or the address adds the repository once and installs the app and the plugins they want from it.

This document is the design as of session 40 (9 Nov 2026), with the spike that checks it (`spikes/flatpak/`, `.github/workflows/flatpak.yml`).

## Decided by Mateus (9 Nov 2026)

- **The app and each plugin are packaged individually** as Flatpaks, from a repository of our own, like the `.deb` packages.
- **No mirror of the runtime.** Our repository holds our packages only; the runtime (org.freedesktop.Platform) comes from Flathub's own repository, as for any Flatpak app. This replaces the roadmap's earlier plan of mirroring it in a repository of its own.
- **English only:** no translations, so the runtime's translations extension isn't needed either.
- **The Flatpak installs the PNG icons** (16 to 512 px); the SVG stays everywhere else (the `.deb`, `.rpm` and AppImage).
- **The default Flatpak sandbox**: no permission beyond what the window and sound need. The user must still have a way to add plugins, sound sets and amp models (see "Adding plugins, sound sets and amp models").
- **Both builds, as in the AppImage**: the Wayland build and the X11 build, with a launcher choosing.

## What exists

- **Packages.** The core's `release.yml` builds the `.deb`, `.rpm` and AppImage (both builds, x86_64 and ARM64); each plugin's own repository calls oxiguitar-packages' `plugin-release.yml` and `plugin-publish.yml` (`@main`). oxiguitar-packages' Site workflow builds the apt and dnf repositories from **published** releases, signs them with our key and serves them on its Pages. Releases are made with the Release button.
- **Plugin folders** (`oxiguitar-host/src/folders.rs`). The system plugin folder is derived from the program's path: `PREFIX/bin/oxiguitar` gives `PREFIX/lib/oxiguitar/plugins`. Inside a Flatpak, `/app/bin/oxiguitar` gives `/app/lib/oxiguitar/plugins`. `inventory::plugin_files` lists the `.oxp` files directly in a folder with `Path::is_file`, which follows symbolic links.
- **Files.** The file dialogs use the XDG desktop portal (`rfd` with `xdg-portal`): inside a sandbox they open and save any file the user picks, with no filesystem permission.
- **Drawing.** Slint 1.17.1 with the femtovg renderer (OpenGL) and the software one; the Wayland build holds no X11 code (ADR 0009), the X11 build is deprecated.
- **Sound.** The core plays nothing itself: the audio-device plugins (ALSA, PipeWire) and the instruments are plugins. The MIDI plugin plays out of the box with the system's own sound set (`/usr/share/sounds/sf2`, `sf3`, `/usr/share/soundfonts`) and the sets in the user's `soundfonts` folder, where the sound-set installer (oxiguitar-bettersounds-soundfonts) puts them.

## The app

- Id `io.github.mateusrdp.OxiGuitar` (the desktop file, metainfo and icons already use it). Runtime `org.freedesktop.Platform` 26.08, from Flathub.
- **Built from source in the Freedesktop SDK** with Rust 1.99 from rustup, `cargo build --release --locked`. The build reaches the network (`--share=network`) for rustup and the crates: allowed in our own repository (Flathub would want the crates as listed sources). Built in the SDK, the program links against the runtime's own libraries (fontconfig, Wayland, libxkbcommon, Mesa).
- **Both builds, as in the AppImage** (`scripts/package/AppRun`): `oxiguitar` (Wayland), `oxiguitar-x11`, and the launcher `oxiguitar-start` as the Flatpak's command. With `--socket=wayland` and `--socket=fallback-x11` Flatpak gives the sandbox Wayland in a Wayland session and X11 only where there is none, and the launcher starts the build for what it was given. The Flatpak's desktop file says `Exec=oxiguitar-start %f` (Flatpak runs the command an `Exec` line names, so `oxiguitar` there would always start the Wayland build).
- Installs `oxiguitar`, `oxiguitar-x11`, `oxiguitar-start`, `oxiguitar-cli`, the desktop file, metainfo, MIME type, the PNG icons and LICENSE.

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
| `--socket=fallback-x11`, `--share=ipc` | The window on X11 desktops (X11 build); X11 is given only where there is no Wayland. |
| `--device=dri` | OpenGL for the femtovg renderer. |
| `--socket=pulseaudio` | The ALSA plugin: the runtime's ALSA library plays through the sound server. |
| `--filesystem=xdg-run/pipewire-0:ro` | The PipeWire plugin's socket. |

No network, no home folder, nothing of the system's: score files come through the portal, and everything else the app reads is in its own folders (below). The app downloads and uploads nothing, as everywhere.

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
- **Both builds through the launcher** (run 42598928786): with only an X server (Xvfb, no Wayland) the sandbox gets `DISPLAY` and no `WAYLAND_DISPLAY`, `oxiguitar-start` starts `/app/bin/oxiguitar-x11`, and it is still running 15 seconds later; the exported menu entry runs `flatpak run --command=oxiguitar-start ... %f`. Both builds' libraries are all in the runtime.
- **appstreamcli compose refused the scalable SVG icon** ("Unrecognized image file format", also with a one-rectangle SVG, so not our drawing); hence the PNG icons.
- **The runtime from Flathub** is 282.7 MB to download for x86_64 (270.5 MB for ARM64) plus 150.5 MB (139.1 MB) for its GL drivers, once per machine and shared with every other Flatpak app using 26.08.

## Adding plugins, sound sets and amp models

A Flatpak keeps its own folders: `$XDG_DATA_HOME` inside the sandbox is `~/.var/app/io.github.mateusrdp.OxiGuitar/data`, so OxiGuitar's data folder there is

```
~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/
    plugins/      the user's own plugin files
    soundfonts/   sound sets
    nam/          amp models and cabinets
```

The app reads it and nothing else of the user's; **the user, a file manager or a script on the computer can always write into it**, with no permission and without the app running. That is the default Flatpak way, and it is how all three get in.

- **Our plugins** are Flatpak packages in our repository: `flatpak install oxiguitar io.github.mateusrdp.OxiGuitar.Plugin.guitarpro`, updated with `flatpak update`. GNOME Software and KDE Discover list them under the app's add-ons, installed with a click. Nothing changes in OxiGuitar.
- **Any other plugin file** (someone else's, a test build): copied into `plugins/` above. That works today; **Help → About** already lists the plugin folders, so it shows this path.
- **Sound sets:** the sandbox has no network, and the app downloads nothing anyway, so the download can't happen inside it. The sound-set installer runs on the computer, as it does now, and writes into the Flatpak's `soundfonts/` folder. Proposed change to oxiguitar-bettersounds-soundfonts' `install.sh`: when the OxiGuitar Flatpak is installed (`flatpak info io.github.mateusrdp.OxiGuitar` succeeds, or `~/.var/app/io.github.mateusrdp.OxiGuitar` exists), install there too, and say so; the same one command for every user. A sound set the user already has is copied into the same folder by hand. Packaging the sets as Flatpaks instead would mean hosting several GB of them ourselves, where the installer deliberately fetches them from their own authors' sites.
- **Amp models (NAM) and cabinets:** copied into `nam/` above, as into `~/.local/share/oxiguitar/nam/` without Flatpak. A plugin's track section can't open a file dialog (its widgets are Label, Button, Toggle, Slider, Field, Choice, Link), so there is no picking them from inside the app today.
- **The system's own sound sets** (`/usr/share/sounds/sf2` and the rest) are not visible in the default sandbox, so the Flatpak does not play out of the box as the `.deb` does: it needs a sound set from the installer or copied in. The MIDI plugin's "no sound set" message should then name the Flatpak's folder and the installer. (The one permission that would change it, read-only access to the system's files, `--filesystem=host-os:ro`, is outside the default way and was left out.)
- **The manual** gets a "Flatpak" section in install-linux.md with these folders and commands.

Two conveniences could come later, if wanted: a button in the plugin manager that copies a chosen `.oxp` into the user's plugin folder through the file dialog (today the manager "never downloads, copies, installs or removes anything"), and the same for amp models (a host-provided file picker for plugin sections, a plugin-interface addition).

## Questions for Mateus

1. **The sound-set installer:** should `install.sh` fill the Flatpak's `soundfonts` folder too when the OxiGuitar Flatpak is installed? (Proposed: yes, automatically, the same command for everyone.)
2. **The MIDI plugin's message** when it finds no sound set: should it say, inside the Flatpak, where to put one and which command fills it? (Proposed: yes; it already tells other users where to get one.)
