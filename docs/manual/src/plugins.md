# Settings and plugins

OxiGuitar is a small program that does more with plugins: one plugin for each file format, plugins that play the score, plugins that export it in other formats than PDF. Plugins are single files ending in `.oxp`. The plugin system is being built now; this version finds plugin files but cannot load them yet, and it still reads Guitar Pro files by itself.

## Where OxiGuitar keeps things

When it starts, OxiGuitar makes sure its settings folder, a `settings.json` file and your `plugins` folder exist (on Linux in the usual freedesktop places), and creates any that are missing. It never changes anything else.

| System | Settings (`settings.json`) | Your plugins | Plugins installed with the program |
| --- | --- | --- | --- |
| Linux | `~/.config/oxiguitar/` | `~/.local/share/oxiguitar/plugins/` | `/usr/lib/oxiguitar/plugins/` |
| macOS | `~/Library/Application Support/OxiGuitar/` | its `plugins` folder | inside the app, `Contents/PlugIns/` |
| Windows | `%APPDATA%\OxiGuitar\` | its `plugins` folder | a `plugins` folder beside `oxiguitar.exe` |

To keep everything somewhere else, for example on a USB stick, start OxiGuitar with the environment variable `OXIGUITAR_HOME` set to that folder: settings and plugins then both live there.

OxiGuitar also keeps the View menu's choices in `settings.json`, under `view`.

## settings.json

A new `settings.json` is empty: `{}`. To look for plugins in folders of your own, list them, in the order to search:

```json
{
  "plugin_folders": ["/home/me/oxiguitar-plugins", "/usr/lib/oxiguitar/plugins"]
}
```

This list replaces the usual folders. If the file isn't valid JSON, OxiGuitar says so at the bottom of the window, leaves the file as it is and uses the usual settings.

## Messages about plugins

At start, the line at the bottom of the window says what is missing:

- **No output plugin is installed, so the player is off.** The player's buttons stay greyed out until a plugin that makes sound is installed.
- **No read/write plugins are installed.** OxiGuitar can open and save files only through format plugins, so nothing can be opened. The packages install our Guitar Pro plugins in the program's own plugin folder; this message means they are missing there, or the folder isn't searched (see `plugin_folders` above).

Both messages say where to download plugins: [the releases page](https://github.com/mateusrdp/oxiguitar/releases). Put the files in your plugins folder.

Plugins made by the OxiGuitar project keep the program's privacy promises: nothing is uploaded, and nothing is downloaded unless you ask. Plugins from anyone else can do anything a program can; install only plugins you trust.
