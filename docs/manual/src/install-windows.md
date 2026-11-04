# Installing on Windows

OxiGuitar for Windows is a zip you unzip anywhere, from the [releases page](https://github.com/mateusrdp/oxiguitar/releases). [Installing](installing.md) lists the files and the first-launch warning. This page is about plugins.

Plugins are files ending in `.oxp`. OxiGuitar looks for them in your own folder, `%APPDATA%\OxiGuitar\plugins`, and in the `plugins` folder beside `oxiguitar.exe`. **Help → About** lists every folder it searches, and **Tools → Plugins…** every plugin it found. [The official plugins](official-plugins.md) says what each does.

## Plugins

The zip holds the program only, portable: unzip it anywhere and run `oxiguitar.exe`. Plugins need to be in a specific place, which is why they alone have installers.

- **An installer for each plugin.** Each plugin's releases page has a small setup program (`oxiguitar-NAME-VERSION-windows-x86_64-setup.exe`, or `-arm64`) that installs for you alone: no administrator password. It puts the plugin in `%APPDATA%\OxiGuitar\plugins`, which you rarely find by yourself, and Windows then lists it under Installed apps, where you can remove it. Or unzip the plugin's own zip and put the `.oxp` file there yourself.
- **You need at least** the Guitar Pro plugin, the MIDI plugin and the WASAPI plugin (the one that sends sound to your speakers). The releases pages are linked from [the official plugins](official-plugins.md).
- Updates are by hand: install the newer setup program over the old one.

## The recorded sound sets

The sound-set installer's releases page has `install.ps1`, a PowerShell script that lists what it would download, from where and under which licence, and asks before it fetches anything, into your `soundfonts` folder ([Better sounds](better-sounds.md#the-sound-set-installer)). OxiGuitar itself never downloads anything.

## Coming later

- **winget**, to install and update OxiGuitar and each plugin by name. winget's package repository reviews each new package by hand, so the first ones take a while to appear; not published yet.

If a plugin is missing, OxiGuitar says so once when it starts and links here.
