# Installing and sharing

Installing a plugin means putting its `.oxp` file in a plugin folder; removing it means deleting the file. Nothing is unpacked or registered, and OxiGuitar itself never downloads, copies or removes plugins.

## Where users put the file

Each user has a plugin folder, which OxiGuitar creates the first time it starts:

| System | The user's plugin folder |
| --- | --- |
| Linux | `~/.local/share/oxiguitar/plugins/` (`$XDG_DATA_HOME/oxiguitar/plugins/` when that is set) |
| macOS | `~/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `%APPDATA%\OxiGuitar\plugins\` |

OxiGuitar also searches the program's own plugin folder, where packages install plugins for every user:

| System | The program's plugin folder |
| --- | --- |
| Linux | `/usr/lib/oxiguitar/plugins/` (beside the program: `<bin>/../lib/oxiguitar/plugins/`) |
| macOS | `OxiGuitar.app/Contents/PlugIns/`, then `/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `plugins\` beside `oxiguitar.exe` |

The user's folder comes first. Help → About lists every folder in the order searched. A `plugin_folders` list in settings.json replaces the defaults, and `OXIGUITAR_HOME` puts settings.json and `plugins/` in one folder (for tests and portable copies).

Only `.oxp` files directly in a folder count, not in subfolders. The file's name doesn't matter, but keep it recognisable: `example.oxp`, or the download's own name, `example-0.1.0-linux-x86_64.oxp`.

## What OxiGuitar does with it

At the next start, OxiGuitar reads the file's manifest and lists it in **Tools → Plugins**, with a tickbox to enable or disable it from the next start (it writes the plugin's id under `disabled` in settings.json). The plugin is loaded the first time one of its contributions is used.

When two files carry the same plugin `id`, the higher version is used and the other is listed as replaced; on a tie, the file in the earlier folder, then the earlier name. So a user who installs your new version beside an old one gets the new one, and a user's own copy wins over a package's of the same version.

A file that can't be used is listed with the reason: another system's or processor's file, a damaged file, a manifest problem, an interface newer than this OxiGuitar has, or conditions that don't hold here.

## macOS: the download mark

macOS marks downloaded files, and refuses to load an unsigned library that carries the mark ("cannot be opened because the developer cannot be verified"). OxiGuitar's own plugins aren't signed either; the user manual tells users to clear the mark:

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/example.oxp
```

Say the same in your README, or sign and notarise your file with an Apple Developer ID.

## What to tell your users

- What the plugin adds, and which OxiGuitar it needs: 0.2 or later.
- Which file is for which system, and where to put it (the table above).
- **What it does beyond OxiGuitar.** A plugin is native code with the user's rights; OxiGuitar's promises (no telemetry, never upload, never download) cover the app and our own plugins, not yours. If your plugin never touches the network, say so; if it does, say when and why.
- Its licence. The SDK is MIT OR Apache-2.0, so any GPL-compatible licence works.
- Where to report its bugs. When OxiGuitar refuses a file your plugin's extension names, it suggests reporting a bug at OxiGuitar's tracker; we will point people to you, but a line in your README saves them the trip.

## Sharing it

Host it wherever you like: a GitHub release (the template's workflow makes one), your own site, a package for your Linux distribution installing into `/usr/lib/oxiguitar/plugins/`. OxiGuitar names no plugin source of its own, ours included, beyond its footer's pointer to our releases page when nothing can be opened. If you'd like your plugin mentioned in OxiGuitar's documentation, open an issue.
