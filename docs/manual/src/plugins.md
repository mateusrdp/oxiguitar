# Settings and plugins

OxiGuitar is a small program that does more with plugins: one plugin for each file format, plugins that play the score, plugins that export it in other formats than PDF, and plugins that add tabs to the window's panels. Plugins are single files ending in `.oxp`. OxiGuitar opens files only through its format plugins; the packages install ours for Guitar Pro files. A plugin is loaded only when it is first needed: the Guitar Pro 5 plugin when you open a .gp5 file, a plugin with a tab when you open its tab.

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

## The plugin manager

**Tools → Plugins…** opens the plugin manager in a window of its own, which you can leave open beside the score. It lists every plugin file in the plugin folders, yours and the program's, with its name, its version, the folder and file it is in, and what became of it:

- **Loaded**: it is in use.
- **Not loaded yet**: it loads when it is first needed.
- **Disabled**: you switched it off.
- **Not used**: another file holds the same plugin at a higher or equal version, and that one is used.
- **Can't be used**, and why: the file is damaged, it is for another system, it needs a newer OxiGuitar, or its own conditions don't hold here (a plugin only for Wayland, say).

The tickbox beside a plugin enables or disables it. The change takes effect the next time OxiGuitar starts, since a plugin that is loaded stays loaded until the program ends; the plugin's line says what will change. OxiGuitar keeps the list in `settings.json`, under `disabled`.

The plugin manager only lists plugins and switches them on and off. It never downloads, copies, installs or removes anything. To install a plugin, put its `.oxp` file in your plugins folder (see above), or install a package that puts it in the program's folder. To remove one, delete its file.

## Safe mode

If a plugin misbehaves so badly that OxiGuitar won't start properly, start it in **safe mode**:

```sh
oxiguitar --safe-mode
```

In safe mode no plugin is used at all, so nothing can be opened or played, but **Tools → Plugins…** still lists every plugin file: untick the one that causes trouble, then start OxiGuitar again normally. **Help → About** lists the plugin folders, in the order they are searched, so you can also find the file and delete it.

## Privacy

Plugins made by the OxiGuitar project keep the program's privacy promises: nothing is uploaded, and nothing is downloaded unless you ask. We make no promises about plugins from anywhere else: a plugin can do anything a program can, so install only plugins you trust.
