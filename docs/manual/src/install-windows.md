# Installing on Windows

OxiGuitar for Windows is a zip you unzip anywhere, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). [Installing](installing.md) lists the files and the first-launch warning. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `%APPDATA%\OxiGuitar\plugins`, and in the `plugins` folder beside `oxiguitar.exe`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found.

## Plugins

The zip holds the program only, portable: unzip it anywhere and run `oxiguitar.exe`. Plugins need to be in a specific place, which is why they alone have installers.

- **An installer for each plugin.** Each plugin's releases page has a small setup program (`oxiguitar-NAME-VERSION-windows-x86_64-setup.exe`, or `-arm64`) that installs for you alone, with no administrator password. It puts the plugin in `%APPDATA%\OxiGuitar\plugins`, and Windows lists it under Installed apps, where you can remove it. Or unzip the plugin's own zip and put the `.oxp` file there yourself.
- **You need at least** a plugin that reads Guitar Pro files, one that plays a score and the WASAPI plugin, which sends the sound to your speakers.
- Updates are by hand: install the newer setup program over the old one.

This work depends on the [packages manual](https://mateusrdp.github.io/oxiguitar-packages/manual/plugins.html), which links to the manual and releases page of every plugin.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
