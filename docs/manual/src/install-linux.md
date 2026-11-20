# Installing on Linux

OxiGuitar for Linux comes as an AppImage, a .deb, an .rpm and a Flatpak, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases) and from the package repositories. The [Installing](installing.md) page lists the files and their requirements. This page is about where plugins go.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `~/.local/share/oxiguitar/plugins/`, and in the program's folder, `/usr/lib/oxiguitar/plugins/`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found.

## apt, dnf and Flatpak

This work depends on the [packages manual](https://mateusrdp.github.io/oxiguitar-packages/manual/use-cases.html). It has the commands that add the apt, dnf and Flatpak repositories and install OxiGuitar with the plugins you want, and it says how they are kept up to date. Each plugin's manual says what it does.

The Flatpak runs in a sandbox. It opens and saves the files you choose in its file dialogs and sees nothing else, so it has plugin folders of its own, listed in the packages manual.

## AppImage

The AppImage runs anywhere without installing. It holds the program only. Its plugins are files you add yourself:

1. Download the plugin's Linux zip from its releases page (each repository's release has one zip for your processor, x86_64 or ARM64). The [packages manual](https://mateusrdp.github.io/oxiguitar-packages/manual/plugins.html) links to every plugin.
2. Unzip it and put the `.oxp` file in `~/.local/share/oxiguitar/plugins/`.
3. Start OxiGuitar again. **Tools → Plugins…** lists it.

You need a plugin that reads Guitar Pro files, one that plays a score and the ALSA plugin, which is the output the AppImage uses (the PipeWire plugin can't be built for the AppImage's oldest supported system). Plugins are updated by hand: download the newer zip and replace the file. (The AppImage will update itself in place once it has an update channel.)

If a plugin is missing, OxiGuitar says so once when it starts and links here.
