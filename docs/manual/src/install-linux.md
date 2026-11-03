# Installing on Linux

OxiGuitar for Linux comes as an AppImage, a .deb and an .rpm, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). The [Installing](installing.md) page lists the files and their requirements. This page is about where plugins go, and how they are kept up to date.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/.local/share/oxiguitar/plugins/`, and in the program's folder, `/usr/lib/oxiguitar/plugins/`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Our apt repository (Debian, Ubuntu and relatives)

The plugins are packages of their own (`oxiguitar-guitarpro`, `oxiguitar-midi`, `oxiguitar-nam`, and so on), kept in a signed repository on GitHub Pages that you add once, like a PPA:

```sh
sudo install -d -m 0755 /etc/apt/keyrings
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.gpg -o /etc/apt/keyrings/oxiguitar.gpg
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.sources -o /etc/apt/sources.list.d/oxiguitar.sources
sudo apt update
sudo apt install oxiguitar
```

`oxiguitar` recommends `oxiguitar-guitarpro`, `oxiguitar-midi` and the audio outputs `oxiguitar-alsa` and `oxiguitar-pipewire`, so apt installs them with it (leave out recommended packages and you get the program alone), and suggests `oxiguitar-nam` for amp models; add any plugin by name. `sudo apt upgrade` then keeps OxiGuitar and its plugins current. The repository's key is signed into the `.gpg` file; its fingerprint is on the [repository's page](https://github.com/mateusrdp/oxiguitar-packages). A downloaded `.deb` works too, but then you install the plugins' `.deb` files yourself.

## Our dnf repository (Fedora and relatives)

The same repository serves dnf, signed the same way:

```sh
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.repo -o /etc/yum.repos.d/oxiguitar.repo
sudo dnf install oxiguitar
```

dnf installs the recommended plugins with it, and `sudo dnf upgrade` keeps them current.

## Coming later

- **Any Linux, through Flathub:** OxiGuitar as a Flatpak, with each plugin an extension you add in GNOME Software or Discover.
- **The sound sets**: a package `oxiguitar-bettersounds-soundfonts` that downloads the recommended free SFZ sets into `/usr/share/oxiguitar/soundfonts`, after asking you; OxiGuitar looks there too ([Better sounds](better-sounds.md)).

## AppImage

The AppImage runs anywhere without installing. It holds the program only. Its plugins are files you add yourself:

1. Download the plugin's Linux zip from its releases page (the [official plugins](official-plugins.md) page lists them; each repository's release has one zip for your processor, x86_64 or ARM64).
2. Unzip it and put the `.oxp` file in `~/.local/share/oxiguitar/plugins/`.
3. Start OxiGuitar again. **Tools → Plugins…** lists it.

You need the essential ones: a plugin that reads Guitar Pro files, the MIDI plugin, and the ALSA plugin, which is the output the AppImage uses (the PipeWire plugin can't be built for the AppImage's oldest supported system). Plugins are updated by hand: download the newer zip and replace the file. (The AppImage will update itself in place once it has an update channel.)

If a plugin is missing, OxiGuitar says so once when it starts and links here.
