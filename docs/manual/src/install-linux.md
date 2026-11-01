# Installing on Linux

OxiGuitar for Linux comes as an AppImage, a .deb and an .rpm, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). The [Installing](installing.md) page lists the files and their requirements. This page is about where plugins go, and how they are kept up to date.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/.local/share/oxiguitar/plugins/`, and in the program's folder, `/usr/lib/oxiguitar/plugins/`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Today

**Every download carries the basic plugins**: the .deb and .rpm depend on the package `oxiguitar-plugins-basic`, which holds them, and the AppImage holds them inside. Install the two packages together, as [Installing](installing.md) shows, and everything works. Nothing else is needed to open and play Guitar Pro files.

## Coming with OxiGuitar 1.0

OxiGuitar 1.0 splits the plugins from the program. Nothing below is published yet; the commands will be here when it is.

- **Debian, Ubuntu and relatives: our apt repository.** `apt install oxiguitar` brings the essential plugins with it (Guitar Pro, MIDI and an audio output) as recommended packages, and `apt upgrade` keeps them all current. Each plugin is a package of its own (`oxiguitar-guitarpro`, `oxiguitar-midi`, `oxiguitar-nam`, and so on), so you can add the optional ones by name. The repository is signed; you add its key once.
- **Fedora, openSUSE and relatives: our dnf repository**, the same packages as .rpm, signed the same way.
- **Any Linux, through Flathub:** OxiGuitar as a Flatpak, with each plugin an extension you add in GNOME Software or Discover; the essential ones come with it.
- **The sound sets**: a package `oxiguitar-bettersounds-soundfonts` that downloads the recommended free SFZ sets into `/usr/share/oxiguitar/soundfonts`, after asking you; OxiGuitar looks there too ([Better sounds](better-sounds.md)).

## AppImage

The AppImage runs anywhere without installing. **Today** it holds the basic plugins inside itself. **From 1.0 it holds the program only**, and updates itself in place (AppImageUpdate and Gear Lever fetch just what changed). Its plugins are then files you add yourself:

1. Download the plugin's Linux zip from its releases page (the [official plugins](official-plugins.md) page lists them; each repository's release has one zip for your processor, x86_64 or ARM64).
2. Unzip it and put the `.oxp` file in `~/.local/share/oxiguitar/plugins/`.
3. Start OxiGuitar again. **Tools → Plugins…** lists it.

You need the essential ones: a plugin that reads Guitar Pro files, the MIDI plugin, and the ALSA plugin, which is the output the AppImage uses (the PipeWire plugin can't be built for the AppImage's oldest supported system). Plugins are updated by hand: download the newer zip and replace the file.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
