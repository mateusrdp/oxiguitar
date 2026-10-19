# OxiGuitar

<p align="center"><img src="assets/icon/oxiguitar-placeholder.svg" alt="OxiGuitar placeholder icon: a green guitar pick with OxG on it" width="96"></p>

**OxiGuitar** is a free, open-source guitar tablature and score viewer, editor and player for Windows, macOS and Linux, written entirely in Rust.

It opens Guitar Pro files (.gp, .gpx, .gp5, and the older .gp3 and .gp4), shows them as standard notation and tablature, lets you edit them and plays them back. MuseScore, MusicXML, ABC and MIDI files are planned as plugins once the plugin system exists. Its layout follows Guitar Pro 8 closely, so it feels familiar; every icon and piece of artwork is our own.

> **Status: early development (milestone M1, the viewer).** OxiGuitar opens and shows Guitar Pro files and exports them to PDF; editing and playback come in the next milestones. See [ROADMAP.md](ROADMAP.md) for what is done and what comes next.

## Privacy

OxiGuitar never uploads anything. No telemetry, no analytics, no automatic crash reports, no accounts, no background requests — not a single bit. It uses the network only for downloads you start yourself (a better SoundFont, for example, or a manual update check). If it crashes, it writes a log to your disk and tells you where it is; sending it to us is up to you.

A CI test runs OxiGuitar under a system-call tracer and fails if it ever tries to open a network connection.

These promises cover OxiGuitar and the plugins we publish, and nothing else: we make no promises about plugins you get from anywhere else.

## Supported systems

| System | Versions | CPUs |
| --- | --- | --- |
| Windows | 10 and later | x86_64, ARM64 |
| Linux | current distributions with glibc 2.39 or newer: Debian trixie and sid, Ubuntu 24.04+, Fedora 40+ | x86_64, ARM64 |
| macOS | the current release | Apple Silicon and Intel (one universal app) |

Releases come as a portable .exe for Windows (in a zip); an AppImage, a .deb and an .rpm for Linux; and one .dmg for macOS. Each [release](https://github.com/mateusrdp/oxiguitar/releases) carries a SHA256SUMS file. The .deb, .rpm and .dmg register the Guitar Pro file types (.gp, .gpx, .gp5, .gp4, .gp3), so double-clicking a file opens OxiGuitar; the portable .exe and the AppImage register nothing, and the [user manual](https://mateusrdp.github.io/oxiguitar/installing.html) shows how to do it by hand. The [manual](https://mateusrdp.github.io/oxiguitar/) explains the whole program in a few short pages.

- **Debian, Ubuntu:** `sudo apt install ./oxiguitar_VERSION_amd64.deb` (or `_arm64.deb`).
- **Fedora:** `sudo dnf install ./oxiguitar-VERSION-1.x86_64.rpm` (or `.aarch64.rpm`).
- **Linux on X11:** the packages above are the Wayland build, for today's desktops. A desktop without Wayland needs the X11 build: `oxiguitar-x11_….deb`, `oxiguitar-x11-….rpm` or `OxiGuitar-x11-….AppImage`. It is deprecated and will be dropped. Each build carries only its own display system's code, and installing one package replaces the other.

None of the packages is code-signed yet, so the first launch shows a warning:

- **Windows:** SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS:** right-click (or Control-click) the app, choose **Open**, then **Open** again. On recent macOS versions you may instead need **System Settings → Privacy & Security → Open Anyway**.
- **Linux AppImage:** make it executable (`chmod +x OxiGuitar-*.AppImage`), then run it.

## Building from source

You need Rust 1.97 or newer. On Debian sid everything comes from apt:

```sh
sudo apt install git build-essential pkg-config rustc cargo rustfmt rust-clippy \
  libfontconfig1-dev libxkbcommon-dev libwayland-dev
git clone https://github.com/mateusrdp/oxiguitar.git
cd oxiguitar
cargo build --release
scripts/build-plugins.sh
cargo test
```

Elsewhere, install Rust from your system's packages or from [rustup.rs](https://rustup.rs). The last three packages are the system libraries the desktop app needs for fonts, keyboard and windows (the same names on Ubuntu). Audio, from milestone M3, adds `libasound2-dev`.

The program opens files only through format plugins (one per format, `.oxp` files). `scripts/build-plugins.sh` builds ours (Guitar Pro 3, 4, 5, 6 and 7–8) and puts them where a build from source looks for them (`target/lib/oxiguitar/plugins/` on Linux); run it again after changing a plugin or a reader. The release packages hold the program alone; ours come as `oxiguitar-plugins-basic` (.deb, .rpm) and as a zip for each system. To write your own plugin, see the [plugin developer guide](https://mateusrdp.github.io/oxiguitar/plugin-guide/) and its template, `docs/plugins/template/`.

Run the desktop app, optionally with a file to open:

```sh
cargo run --release -p oxiguitar-app -- path/to/your/song.gp
```

That is the Wayland build, the default. On a desktop with only an X server, build the X11 one instead: `cargo run --release -p oxiguitar-app --no-default-features --features x11 -- song.gp`. Each build carries only its own display system's code, and says which build you need if it can't find its session.

It opens Guitar Pro 3 to 8 files and shows them in four layouts (View → Layout), with a dark page, the track list (F8), the Tools menu and PDF export. On Linux, the Open and Export dialogs come from the desktop portal (`xdg-desktop-portal` with your desktop's backend, installed by default with GNOME and KDE). The binary is `target/release/oxiguitar`; it finds the plugins in `target/lib/oxiguitar/plugins/`, and in your own plugin folder ([Settings and plugins](https://mateusrdp.github.io/oxiguitar/plugins.html)). On Wayland, desktops take an app's icon from its desktop entry rather than from the window; to give a build from source (or an AppImage) its icon, a menu entry and the Guitar Pro file types, run `scripts/install-desktop-entry.sh [path to the program]` (it writes only to `~/.local/share`; `--uninstall` removes it).

Try the command-line tool, which reads files through the same plugins (Guitar Pro 3 to 8: .gp3, .gp4, .gp5, .gpx, .gp) and engraves them into PNG pages and a PDF:

```sh
cargo run --release -p oxiguitar-cli -- info fixtures/gp/oxiguitar-test-song.gp
cargo run --release -p oxiguitar-cli -- info --bars path/to/your/song.gp5
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --png page --pdf song.pdf
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --tracks all --dark --pdf song-dark.pdf
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --tool infer-key-signatures --no-dynamics --pdf keys.pdf
```

## Testers wanted

The maintainer tests by hand on x86_64 Debian sid only. Everything else — **Windows, macOS, other Linux distributions, and any ARM64 machine** — is built and tested automatically, but nobody has clicked through it yet. If you can try test builds on one of those systems, please say hello in [Discussions](https://github.com/mateusrdp/oxiguitar/discussions) or open an issue with what you find.

Test builds of every change on `develop` are under the [Actions](https://github.com/mateusrdp/oxiguitar/actions/workflows/builds.yml) tab, kept for 14 days (you need to be signed in to GitHub to download them). Packages of the latest state are made by hand under [Release packages](https://github.com/mateusrdp/oxiguitar/actions/workflows/release.yml), kept just as long.

## Contributing

Pull requests are welcome from day one. Read [CONTRIBUTING.md](CONTRIBUTING.md) first: it covers code standards, the sign-off rule and how to add a file format or an effect. [ARCHITECTURE.md](ARCHITECTURE.md) explains how the code fits together, and [docs/adr/](docs/adr/) records why things are the way they are.

## Donations

Donations are welcome; details will follow here soon.

## License

OxiGuitar is free software under the [GNU General Public License, version 3](LICENSE). Our own code is GPL-3.0-or-later; the program as distributed is GPL-3.0, because code ported from MuseScore keeps its GPL-3.0-only license. The plugin interface and the SDK for plugin authors (`oxiguitar-plugin-api`, `oxiguitar-plugin`) are MIT OR Apache-2.0, so plugins may use any GPL-compatible license.

Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. [CREDITS.md](CREDITS.md) lists every project we build on and thanks everyone who has contributed.

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. OxiGuitar is not affiliated with either.
