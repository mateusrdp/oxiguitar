# Installing on Linux

OxiGuitar for Linux comes as an AppImage, a .deb, an .rpm and a Flatpak, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases) and our package repositories. The [Installing](installing.md) page lists the files and their requirements. This page is about where plugins go, and how they are kept up to date.

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

`oxiguitar` recommends `oxiguitar-guitarpro`, `oxiguitar-midi` and the audio outputs `oxiguitar-alsa` and `oxiguitar-pipewire`, so apt installs them with it (leave out recommended packages and you get the program alone), and suggests `oxiguitar-nam` for amp models, `oxiguitar-eq` for an equalizer, and `oxiguitar-musicxml`, `oxiguitar-midi-file` and `oxiguitar-tuxguitar` to open and save MusicXML, MIDI and TuxGuitar files; add any plugin by name. `sudo apt upgrade` then keeps OxiGuitar and its plugins current. The repository's key is signed into the `.gpg` file; its fingerprint is on the [repository's page](https://github.com/mateusrdp/oxiguitar-packages). A downloaded `.deb` works too, but then you install the plugins' `.deb` files yourself.

## Our dnf repository (Fedora and relatives)

The same repository serves dnf, signed the same way:

```sh
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.repo -o /etc/yum.repos.d/oxiguitar.repo
sudo dnf install oxiguitar
```

dnf installs the recommended plugins with it, and `sudo dnf upgrade` keeps them current.

## The recorded sound sets

`oxiguitar-midi` suggests `oxiguitar-bettersounds-soundfonts`, the installer of the free recorded sound sets we recommend (about 1.2 GB; [Better sounds](better-sounds.md#the-sound-set-installer)). Installing it downloads the sets into `/usr/share/oxiguitar/soundfonts`, where OxiGuitar looks and uses them by itself: with apt it asks first (yes is the default, so an unattended install downloads them), with dnf it just does. If you say no, or a download fails, `sudo oxiguitar-install-sounds --system` chooses and downloads them any time later. OxiGuitar itself never downloads anything.

## Flatpak (any Linux)

OxiGuitar and each plugin are also Flatpak packages, in a signed Flatpak repository of our own beside the apt and dnf ones. Add it once, then install OxiGuitar and the plugins you want:

```sh
flatpak remote-add --if-not-exists oxiguitar https://mateusrdp.github.io/oxiguitar-packages/flatpak/oxiguitar.flatpakrepo
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar.Plugin.guitarpro io.github.mateusrdp.OxiGuitar.Plugin.midi io.github.mateusrdp.OxiGuitar.Plugin.pipewire
```

Opening [io.github.mateusrdp.OxiGuitar.flatpakref](https://mateusrdp.github.io/oxiguitar-packages/flatpak/io.github.mateusrdp.OxiGuitar.flatpakref) in a software centre does the first two steps in one. Flatpak takes the runtime OxiGuitar runs on (org.freedesktop.Platform) from Flathub, which it adds if you don't have it yet; everything else comes from our repository. Add `--user` to each command to install for yourself only, without an administrator's password.

Each plugin is a package of its own, named `io.github.mateusrdp.OxiGuitar.Plugin.` and the plugin's short name: `guitarpro`, `midi`, `alsa`, `pipewire`, `nam`, `eq`, `wav`, `musicxml`, `midi-file`, `tuxguitar` ([The official plugins](official-plugins.md)). Nothing comes with OxiGuitar on its own: take at least `guitarpro`, `midi` and an audio output (`pipewire` on most desktops, `alsa` otherwise). GNOME Software and KDE Discover list them as OxiGuitar's add-ons. `flatpak update` keeps OxiGuitar and its plugins current, and `flatpak remove` takes a plugin away.

The Flatpak runs in Flatpak's own sandbox: it opens and saves the files you choose in its file dialogs, and sees nothing else of your computer. So it has folders of its own, which you can always copy files into:

| Folder | What goes in it |
| --- | --- |
| `~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/plugins/` | Plugin files (`.oxp`) that are not Flatpak packages: someone else's, or a test build |
| `~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/soundfonts/` | Sound sets |
| `~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/nam/` | Amp models and cabinets ([Amp models (NAM)](amp-models.md)) |

Create a folder if it isn't there yet. **Help → About** lists the folders OxiGuitar searches.

The sandbox doesn't see your system's sound sets either, so the Flatpak plays nothing until it has one: run the sound-set installer's `install.sh` ([Better sounds](better-sounds.md#the-sound-set-installer)), which fills the Flatpak's `soundfonts` folder when OxiGuitar's Flatpak is installed, or copy a set into that folder yourself. The MIDI plugin says so, with a link to that page, when it finds no set.

## AppImage

The AppImage runs anywhere without installing. It holds the program only. Its plugins are files you add yourself:

1. Download the plugin's Linux zip from its releases page (the [official plugins](official-plugins.md) page lists them; each repository's release has one zip for your processor, x86_64 or ARM64).
2. Unzip it and put the `.oxp` file in `~/.local/share/oxiguitar/plugins/`.
3. Start OxiGuitar again. **Tools → Plugins…** lists it.

You need the essential ones: a plugin that reads Guitar Pro files, the MIDI plugin, and the ALSA plugin, which is the output the AppImage uses (the PipeWire plugin can't be built for the AppImage's oldest supported system). Plugins are updated by hand: download the newer zip and replace the file. (The AppImage will update itself in place once it has an update channel.)

If a plugin is missing, OxiGuitar says so once when it starts and links here.
