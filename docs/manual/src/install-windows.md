# Installing on Windows

OxiGuitar for Windows is a zip you unzip anywhere, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). [Installing](installing.md) lists the files and the first-launch warning. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `%APPDATA%\OxiGuitar\plugins`, and in the `plugins` folder beside `oxiguitar.exe`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Today

**The zip carries the basic plugins** in its `plugins` folder: unzip it and run `oxiguitar.exe`; Guitar Pro files open and play at once. Keep the folder together.

## Coming with OxiGuitar 1.0

OxiGuitar 1.0 splits the plugins from the program. Nothing below is published yet; the exact commands will be here when it is.

- **winget** installs and updates OxiGuitar (the portable zip) and each plugin by name; `winget upgrade --all` keeps them all current. winget's package repository reviews each new package by hand, so the first ones take a while to appear.
- **An installer for each plugin.** Each plugin comes as a small setup program that installs for you alone: no administrator password. It puts the plugin in `%APPDATA%\OxiGuitar\plugins`, which you rarely find by yourself, and Windows then lists it under Installed apps, where you can remove it.
- The portable zip then holds the program only. You need at least the Guitar Pro plugin, the MIDI plugin and the WASAPI plugin (the one that sends sound to your speakers).
- **The sound sets**: a setup program that lists what it will download and from where, and asks before it fetches anything. OxiGuitar itself never downloads. ([Better sounds](better-sounds.md)).

If a plugin is missing, OxiGuitar says so once when it starts and links here.
