# Settings and plugins

OxiGuitar is a small program that does more with plugins: one plugin for each file format, plugins that play the score, plugins that export it in other formats than PDF, and plugins that add tabs to the window's panels. Plugins are single files ending in `.oxp`. OxiGuitar opens files only through its format plugins; ours, in repositories of their own, open Guitar Pro files ([Installing](installing.md)). A plugin is loaded only when it is first needed: the Guitar Pro 5 plugin when you open a .gp5 file, a plugin with a tab when you open its tab.

## Where OxiGuitar keeps things

When it starts, OxiGuitar makes sure its settings folder, a `settings.json` file and your `plugins` folder exist (on Linux in the usual freedesktop places), and creates any that are missing, with the `soundfonts` folder beside `plugins` for sound sets ([Better sounds](better-sounds.md)). It never changes anything else.

| System | Settings (`settings.json`) | Your plugins | The program's plugin folder |
| --- | --- | --- | --- |
| Linux | `~/.config/oxiguitar/` | `~/.local/share/oxiguitar/plugins/` | `/usr/lib/oxiguitar/plugins/` (inside the AppImage, its own `usr/lib/oxiguitar/plugins/`) |
| macOS | `~/Library/Application Support/OxiGuitar/` | its `plugins` folder | inside the app, `Contents/PlugIns/` |
| Windows | `%APPDATA%\OxiGuitar\` | its `plugins` folder | the `plugins` folder beside `oxiguitar.exe` |

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
- **No read/write plugins are installed.** OxiGuitar can open and save files only through format plugins, so nothing can be opened. Install the Guitar Pro plugin ([Installing](installing.md)). If you did, it isn't in a folder OxiGuitar searches: a `plugin_folders` list in `settings.json` that leaves out the folder it is in (see above), or, on Debian, Ubuntu and Fedora, the package `oxiguitar-guitarpro` removed. **Help → About** lists the folders searched.

Both messages link to [the install page](installing.md), which says where each plugin comes from.

The same gaps also open a dialog, once, when the window is up ([Advice dialogs](#advice-dialogs)): it says what is missing and links to [the official plugins](official-plugins.md) and to the install page for your system ([Linux](install-linux.md), [Windows](install-windows.md), [macOS](install-macos.md)). There are three, each silenced on its own:

| Dialog | When | Its line in `settings.json` |
| --- | --- | --- |
| OxiGuitar can't open any file | No plugin reads files | `no-format-plugin` |
| OxiGuitar can't play | No plugin plays a score | `no-player-plugin` |
| OxiGuitar has nowhere to send the sound | A plugin plays scores, but none sends sound to your speakers | `no-output-plugin` |

## Advice dialogs

Sometimes a plugin or OxiGuitar itself needs something from you that it can't do alone: a sound set to install, an amp model to put in a folder. It then shows a small dialog once, when the window is up, saying what to do, with links to the page that explains it. A link opens in your browser only when you click it. Tick **Don't show this again** and press OK, and that dialog never returns.

OxiGuitar keeps what you silenced in `settings.json`, under `advice`, one line for each dialog, under the name of the plugin that asked (`oxiguitar` for OxiGuitar's own):

```json
{
  "advice": {
    "oxiguitar": { "no-player-plugin": "hidden" }
  }
}
```

The official plugins use it for:

| Dialog | Asked by | When | Its line under the plugin's name |
| --- | --- | --- | --- |
| Only basic sounds are installed | `oxiguitar.midi` | A score plays and only your system's own sound set is found ([Better sounds](better-sounds.md)) | `no-sound-sets` |
| No amp model to play | `oxiguitar.nam` | A track's amp is on and the `nam` folder holds no model | `no-model` |
| No cabinet of your own | `oxiguitar.nam` | A track's amp is on, its model doesn't include a cabinet and the `nam` folder holds no impulse response | `no-cabinet` |

The last two link to [Amp models](amp-models.md).

Delete a line to get its dialog back. No dialog appears in safe mode.

## The plugin manager

**Tools → Plugins…** opens the plugin manager in a window of its own, which you can leave open beside the score. It lists every plugin file in the plugin folders, yours and the program's, with its name, its version, the folder and file it is in, and what became of it:

- **Loaded**: it is in use.
- **Not loaded yet**: it loads when it is first needed.
- **Disabled**: you switched it off.
- **Not used**: another file holds the same plugin at a higher or equal version, and that one is used.
- **Can't be used**, and why: the file is damaged, it is for another system, it needs a newer OxiGuitar, or its own conditions don't hold here (a plugin only for Wayland, say).
- **Stopped working**, and why: it failed while OxiGuitar was running (it crashed, say). OxiGuitar stops using it until you restart and carries on as if it weren't installed: its tabs close, and a file it would have opened goes to another plugin that opens it, if you have one. The line at the bottom of the window says so when it happens.

The tickbox beside a plugin enables or disables it. The change takes effect the next time OxiGuitar starts: a plugin can't be unloaded while the program runs, so one you untick while it is in use stays in use until you restart. The plugin's line says what will change, and the window reminds you to restart OxiGuitar while a change is waiting. OxiGuitar keeps the list in `settings.json`, under `disabled`.

The plugin manager only lists plugins and switches them on and off. It never downloads, copies, installs or removes anything. To install a plugin, put its `.oxp` file in your plugins folder (see above), or install a package that puts it in the program's folder. To remove one, delete its file.

## Safe mode

If a plugin misbehaves so badly that OxiGuitar won't start properly, start it in **safe mode**:

```sh
oxiguitar --safe-mode
```

In safe mode no plugin is used at all, so nothing can be opened or played, but **Tools → Plugins…** still lists every plugin file: untick the one that causes trouble, then start OxiGuitar again normally. **Help → About** lists the plugin folders, in the order they are searched, so you can also find the file and delete it.

## Privacy

Plugins made by the OxiGuitar project keep the program's privacy promises: nothing is uploaded, and nothing is downloaded. We make no promises about plugins from anywhere else: a plugin can do anything a program can, so install only plugins you trust.

## Writing your own

Plugins are written in Rust. The [plugin developer guide](plugin-guide/index.html) shows how, from a template plugin that builds, passes our checks and opens in OxiGuitar as it is.
