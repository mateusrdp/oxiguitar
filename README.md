# OxiGuitar

<p align="center"><img src="assets/icon/oxiguitar-placeholder.svg" alt="OxiGuitar placeholder icon: a green guitar pick with OxG on it" width="96"></p>

**OxiGuitar** is a Free, Open-Source fully modular guitar tablature and score viewer, editor and player for Windows, macOS and Linux.

It is written entirely in Rust. Why Rust you'd ask? Because what do you get when you *Oxi*dise METAL \\m/? RUST! And because it’s very fast, of course. We don’t just *want* - we *need* great performance. First because we respect your hardware and your time (especially in this gawd damned economy), and second because we need a lot of grunt to give you a good experience with all the cool features we want in.

It opens the industry’s usual files (Guitar Pro 3 to 8 today; MuseScore, MusicXML, ABC and MIDI files are planned), because no one wants to have to rewrite their entire tab collection they’ve been diligently maintaining since the 90’s just to be able to use Mat's fancy f\*cking app. It needs to be useful from the get-go. It shows them as standard notation and tablature and allows you to play, edit and save them. Every input/output format is a plugin, so you can write your own if we still don’t support what you need.

We support a wide variety of typesetting tools, and try to stay aligned with the music industry standard. Because what's the point of opening your fancy tabs and not being able to display all the work you put in it, right?

Playback is also big. We all like to play along with our favorite songs, don’t always have a band, and can’t always find a good backing track. So this is KEY. And we want quality here, because no one wants to be playing along to a cheap CASIO keyboard from the 90's operated by the neighbor’s toddler who just learnt a few chords and where that dumb arse automatic drumkit is! This is NOT a glorified metronome! Like file formats, every output method is a plugin. The most basic is a MIDI synthesizer that plays whatever sound fonts your computer already has (Windows and macOS come with one; on Linux it’s a package away). But we REALLY recommend that you get yourself some better ones – it makes a massive difference. We do have a few suggestions of nice free ones, with instructions on how to download, install and use them with OxiGuitar. And v1.0 brings some BIG tones too, using the famed *Neural Amp Modeler (NAM)* for some juicy realistic guitar tones. Compose your song, get yourself a sweet profile online (or capture your own rig with their software!) and you’ll have a pretty decent backing track to rock with real quick!

## Privacy

OxiGuitar values your privacy first. It never uploads anything. No telemetry, no analytics, no automatic crash reports, no accounts, no background requests — not a single bit of data. It downloads nothing either: better sounds and plugins are files you get yourself and put in a folder, and the manual says where. The only network use ever planned is an update check you start yourself. If it crashes, it writes a log to your disk and tells you where it is; sending it to us is up to you. A CI test even runs OxiGuitar under a system-call tracer and fails if it ever tries to open a network connection to keep us honest. We don’t collect your data EVER. Because friends do **not** look at their friends’ browser histories.

**THAT SAID**, these promises cover the OxiGuitar core and the plugins we publish, and nothing else: we make no promises about plugins you get from anywhere else. Because again, OxiGuitar is fully modular, open-source and encourages the community to collaborate! **Anyone** can write plugins for it! **Be careful where you get your plugins from!** It’s **your** responsibility to check, and so you should! Ours is open-source, so you can peek inside anytime to see that there’s nothing funky in there and can even compile your own builds if you’re extra-paranoid and don’t trust GitHub with compiling even after reading the build scripts!

## Status

**v0.5.0, the editor (milestone M3), published as a pre-release on 21 October 2026.**

OxiGuitar opens, shows, edits and saves Guitar Pro files and exports them to PDF. It supports pretty much every tool from TuxGuitar at least (more to come!). The window does not play anything yet: the player (milestone M4, NAM included) is what we are building now, and v1.0 follows it. The command-line tool already does: `oxiguitar-cli play` plays a score with the best sound set it finds: yours in the `soundfonts` folder, else the one your system has ([Better sounds](https://mateusrdp.github.io/oxiguitar/better-sounds.html)). See [ROADMAP.md](ROADMAP.md) for what is done and what comes next, and [CHANGELOG.md](CHANGELOG.md) for what changed.

## How to get involved

I’m a pretty good computer scientist and software engineer myself but this is all vibe-coded at the moment, just to get a headstart. Get the grunt work out of the way, and get something working quick. Cause, you know, I have a life, a full time job, and not a lot of time or money to spare (f\*ck, I even like to play the guitar when I can!). But I promise I have been VERY strict with Claude – if the machines turn on us, it *might* be because I have been so mean to them, just saying. Once we get something good out, we REALLY wanna have this all code reviewed by humans. De-slopify this sh\*t! Meanwhile we REALLY wanna have this all at least thoroughly tested by humans. Because Claude and I are doing all we can with what we have.

That’s where **you** come in. Yes, you.

How can you help, you ask?

1. **USE IT!** Let us know what you think in the discussion pages! Report bugs when you find them!
2. **TELL YOUR FRIENDS!** The more people using it and getting involved the better!
3. **GET YOUR HANDS DIRTY!** This is open-source! So, are you a good dev too? SWEET, how about you help us squash a few bugs? Had a wild idea? Go and write a plugin for it or something! Implement a new feature to make the core better! I don’t know! Follow your dreams or whatever! You have the source code, don't you?
4. **BUY ME A DRINK** to say thanks and/or **CHIP IN TO BUY MORE USAGE TOKENS**. Cause yeah, this takes time and resources. And Dario Amodei’s butt lift isn’t paying for itself.

[![Donate with PayPal](assets/paypal-donate-button.png)](https://www.paypal.com/donate/?business=4QYDZY4RP8V8L&no_recurring=0&item_name=Thanks+heaps!+You+rock!&currency_code=AUD&source=qr)
<img src="assets/DonationQRCode.png" alt="OxiGuitar Donation QR Code (PayPal)">

# Nerd Section

## Supported systems

| System | Versions | CPUs |
| --- | --- | --- |
| Windows | 10 and later | x86\_64, ARM64 |
| Linux | current distributions with glibc 2.39 or newer: Debian trixie and sid, Ubuntu 24.04+, Fedora 40+ | x86\_64, ARM64 |
| macOS | the current release | Apple Silicon and Intel (one universal app) |

Each [release](https://github.com/mateusrdp/oxiguitar/releases) has two kinds of download, and you need one of each:

1. **The program**, for your system: a portable .exe for Windows (in a zip); an AppImage, a .deb and an .rpm for Linux; one .dmg for macOS.
2. **The plugins** it opens files with: `OxiGuitar-plugins-basic-VERSION-SYSTEM-ARCH.zip` for your system, unzipped into your plugin folder ([how](https://mateusrdp.github.io/oxiguitar/plugins.html)). On Debian, Ubuntu and Fedora, install the `oxiguitar-plugins-basic` package instead. Without them, OxiGuitar opens nothing. (Releases up to 0.5.0 also list every plugin as a file of its own; you don't need those, as the zip holds them all.)

Every release carries a SHA256SUMS file. The .deb, .rpm and .dmg register the Guitar Pro file types (.gp, .gpx, .gp5, .gp4, .gp3), so double-clicking a file opens OxiGuitar; the portable .exe and the AppImage register nothing, and the [user manual](https://mateusrdp.github.io/oxiguitar/installing.html) shows how to do it by hand. The [manual](https://mateusrdp.github.io/oxiguitar/) explains the whole program in a few short pages.

- **Debian, Ubuntu:** `sudo apt install ./oxiguitar_VERSION_amd64.deb ./oxiguitar-plugins-basic_VERSION_amd64.deb` (or `_arm64.deb`).
- **Fedora:** `sudo dnf install ./oxiguitar-VERSION-1.x86_64.rpm ./oxiguitar-plugins-basic-VERSION-1.x86_64.rpm` (or `.aarch64.rpm`).
- **Linux on X11:** the packages above are the Wayland build, for today's desktops. A desktop without Wayland needs the X11 build: `oxiguitar-x11_….deb`, `oxiguitar-x11-….rpm` or `OxiGuitar-x11-….AppImage`. It is deprecated and will be dropped. Each build carries only its own display system's code, and installing one package replaces the other.

None of the packages is code-signed yet, so the first launch shows a warning:

- **Windows:** SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS:** right-click (or Control-click) the app, choose **Open**, then **Open** again. On recent macOS versions you may instead need **System Settings → Privacy \& Security → Open Anyway**.
- **Linux AppImage:** make it executable (`chmod +x OxiGuitar-*.AppImage`), then run it.

## Building from source

You need Rust 1.99 or newer, from [rustup](https://rustup.rs): distributions' own Rust packages are usually older. On Debian and Ubuntu:

```
sudo apt install git build-essential pkg-config rustup \
  libfontconfig1-dev libxkbcommon-dev libwayland-dev \
  libasound2-dev libpipewire-0.3-dev libclang-dev
rustup default stable
git clone https://github.com/mateusrdp/oxiguitar.git
cd oxiguitar
cargo build --release
scripts/build-plugins.sh
cargo test
```

Elsewhere, install rustup from your system's packages or from [rustup.rs](https://rustup.rs). The `lib…-dev` packages are the system libraries OxiGuitar needs: fontconfig, xkbcommon and Wayland for fonts, keyboard and windows, and ALSA and PipeWire (with libclang, which builds PipeWire's bindings) for the audio-device plugins.

The program opens files only through format plugins (one per format, `.oxp` files). `scripts/build-plugins.sh` builds ours (Guitar Pro 3, 4, 5, 6 and 7–8, the SoundFont player and the audio outputs) and puts them where a build from source looks for them (`target/lib/oxiguitar/plugins/` on Linux); run it again after changing a plugin or a reader. The release packages hold the program alone; ours come as `oxiguitar-plugins-basic` (.deb, .rpm) and as a zip for each system.

Run the desktop app, optionally with a file to open:

```
cargo run --release -p oxiguitar-app -- path/to/your/song.gp
```

That is the Wayland build, the default. On a desktop with only an X server, build the X11 one instead: `cargo run --release -p oxiguitar-app --no-default-features --features x11 -- song.gp`. Each build carries only its own display system's code, and says which build you need if it can't find its session.

File → New starts a score; the [manual's Editing page](https://mateusrdp.github.io/oxiguitar/editing.html) and [Keyboard and mouse](https://mateusrdp.github.io/oxiguitar/keys.html) list the keys. On Linux, the Open and Export dialogs come from the desktop portal (`xdg-desktop-portal` with your desktop's backend, installed by default with GNOME and KDE). The binary is `target/release/oxiguitar`; it finds the plugins in `target/lib/oxiguitar/plugins/`, and in your own plugin folder ([Settings and plugins](https://mateusrdp.github.io/oxiguitar/plugins.html)). On Wayland, desktops take an app's icon from its desktop entry rather than from the window; to give a build from source (or an AppImage) its icon, a menu entry and the Guitar Pro file types, run `scripts/install-desktop-entry.sh [path to the program]` (it writes only to `~/.local/share`; `--uninstall` removes it).

Try the command-line tool, which reads files through the same plugins (Guitar Pro 3 to 8: .gp3, .gp4, .gp5, .gpx, .gp) and engraves them into PNG pages and a PDF:

```
cargo run --release -p oxiguitar-cli -- info fixtures/gp/oxiguitar-test-song.gp
cargo run --release -p oxiguitar-cli -- info --bars path/to/your/song.gp5
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --png page --pdf song.pdf
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --tracks all --dark --pdf song-dark.pdf
cargo run --release -p oxiguitar-cli -- render path/to/your/song.gp --tool infer-key-signatures --no-dynamics --pdf keys.pdf
cargo run --release -p oxiguitar-cli -- convert path/to/your/song.gp3 song.gp
cargo run --release -p oxiguitar-cli -- play path/to/your/song.gp --wav song.wav
```

## Writing plugins

Every file format, tool and tab in OxiGuitar is a plugin: one `.oxp` file, a native library with a small manifest, built from Rust with our SDK. The [plugin developer guide](https://mateusrdp.github.io/oxiguitar/plugin-guide/) walks through it from its template (`docs/plugins/template/`), and plugins may use any GPL-compatible license, since the interface and the SDK are MIT OR Apache-2.0.

## Testers wanted

The maintainer tests by hand on x86\_64 Debian sid only. Everything else — **Windows, macOS, other Linux distributions, and any ARM64 machine** — is built and tested automatically, but nobody has clicked through it yet. If you can try test builds on one of those systems, please say hello in [Discussions](https://github.com/mateusrdp/oxiguitar/discussions) or open an issue with what you find.

Test builds of every change on `develop` (the program for each system, its plugins, and the songs we write as .gp and .gp5 for checking in Guitar Pro) are under the [Actions](https://github.com/mateusrdp/oxiguitar/actions/workflows/builds.yml) tab, kept for 14 days (you need to be signed in to GitHub to download them). Packages of the latest state are made by hand under [Release packages](https://github.com/mateusrdp/oxiguitar/actions/workflows/release.yml), kept just as long.

## Contributing

Pull requests are welcome from day one. Read [CONTRIBUTING.md](CONTRIBUTING.md) first: it covers code standards, the sign-off rule and how to add a file format or an effect. [ARCHITECTURE.md](ARCHITECTURE.md) explains how the code fits together, and [docs/adr/](docs/adr/) records why things are the way they are.

## Donations

See [How to get involved](#how-to-get-involved), point 4. The same PayPal link and QR code are in the app, under **Help → About**.

## License

OxiGuitar is free software under the [GNU General Public License, version 3](LICENSE). Our own code is GPL-3.0-or-later; the program as distributed is GPL-3.0, because code ported from MuseScore keeps its GPL-3.0-only license. The plugin interface and the SDK for plugin authors (`oxiguitar-plugin-api`, `oxiguitar-plugin`) are MIT OR Apache-2.0, so plugins may use any GPL-compatible license.

Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. [CREDITS.md](CREDITS.md) lists every project we build on and thanks everyone who has contributed.

Guitar Pro is a trademark of Arobas Music. MuseScore is a trademark of Muse Group. OxiGuitar is not affiliated with either.
