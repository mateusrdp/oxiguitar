# Settings

A plugin keeps its settings as a JSON object under its id in OxiGuitar's settings.json:

```json
{
  "view": { "dark_interface": true },
  "yourname.example": { "flats": true }
}
```

It reads and writes only its own object, through the host:

```rust,ignore
{{#include ../template/src/statistics.rs:settings_read}}
```

```rust,ignore
{{#include ../template/src/statistics.rs:settings_write}}
```

- `settings()` returns the object (empty when there is none). It is a `serde_json::Map`, so add `serde_json = "1"` to your dependencies.
- `set_settings(map)` replaces the whole object and saves settings.json at once. Read, change, write: keys you leave out are gone.
- The app and every plugin write through one shared copy of settings.json, so none overwrites another's keys. A settings.json that couldn't be read when OxiGuitar started is never written, so settings then last for the session only. `set_settings` returns an error when the file can't be written (a full disk, a folder the user can't write to): handle it (the template tells the footer and carries on).
- **Show them in a dialog of your own** (below).
- Users can also edit settings.json by hand while OxiGuitar is closed, so read it defensively: a value of the wrong type falls back to your default, as the template's `and_then(|v| v.as_bool())` does.

Settings are for small values, and they are global. What belongs to one track of one score goes in a [section of the Track tab](track-sections.md) instead.

## A settings dialog

OxiGuitar draws nothing of a plugin's settings: the plugin shows them itself, in a [dialog](tabs.md#dialogs) of its own, opened by a [tool](tools.md) that doesn't edit the score (interface 1.9):

```json
"tabs": [{ "id": "settings", "title": "Sound Sets", "area": "dialog" }],
"tools": [{ "id": "settings", "name": "MIDI Sound Sets…",
            "description": "Picks the sound set for each kind of instrument.",
            "edits": false }]
```

- The tool's `run` calls `host.open_dialog("settings")`. With `"edits": false` its menu entry works with no score open and while the song plays. Without `menu`, it is in the **Plugins** menu; name it after your plugin so users find it there.
- The dialog shows what is saved, and its `on` changes a **draft** the plugin keeps, not settings.json. In `Tab::closed(ok)`, OK (`true`) writes the draft with `set_settings`; Cancel (`false`) forgets it. Clear the draft when the tool opens the dialog, so each opening starts from what is saved. The test plugin's `SettingsDialog` does exactly this.
- When `set_settings` changes what a plugin that plays sound had saved, OxiGuitar stops the song and prepares its instruments and effects again, so the next Play uses the new settings. Writing only on OK means that happens once, when the user has decided.

OxiGuitar 0.9 to 0.20 drew a settings window for a manifest with `"settings_window": true`, opened from the plugin manager. Interface 1.9 retired it (Mateus, 17 Nov 2026): the core owns nothing of a plugin. The key is now unknown, and the SDK's checks say so; OxiGuitar ignores it in older plugins, which then show no settings until they move to a dialog.

## Folders (interface 1.3)

`host.folders()` hands the plugin its own two folders, as paths:

| Folder | Where | For |
| --- | --- | --- |
| `data` | `$XDG_DATA_HOME/oxiguitar/data/<plugin id>/` on Linux, `data/<plugin id>/` in the user folder elsewhere | What the plugin keeps until the user deletes it |
| `cache` | `$XDG_CACHE_HOME/oxiguitar/<plugin id>/` on Linux, `~/Library/Caches/OxiGuitar/<plugin id>/` on macOS, `%LOCALAPPDATA%\OxiGuitar\cache\<plugin id>\` on Windows | What it can make again (a sound set decoded once); the system or the user may empty it at any time |

The host makes the plugin's own two folders the first time it asks, and the shared folders a manifest lists (below) when OxiGuitar starts, so the user finds them before any plugin runs. Nothing is ever downloaded into them: OxiGuitar and its plugins download nothing.

## Shared folders

A plugin names its shared folders, which the user fills (sound sets, amp models): any name in the manifest's `folders`, and OxiGuitar knows none of them. Asking where one is by name needs interface 1.8; until interface 1.10, `folders()` also answered two names OxiGuitar used to know, `soundfonts` and `nam`, which it no longer does (ADR 0012).

```json
"folders": ["impulses"]
```

Such a folder sits beside `plugins`, in each of OxiGuitar's folders:

- **the user's** (`~/.local/share/oxiguitar/impulses` on Linux, the user folder elsewhere, the Flatpak's own data folder in the Flatpak): OxiGuitar creates it at start, the user puts files there, and the plugin may write there;
- **beside a system plugin folder** (`/usr/lib/oxiguitar/impulses`, `PREFIX/lib/oxiguitar/impulses`), where a package can ship files;
- **beside the program** in a portable copy (`impulses\` next to `oxiguitar.exe`).

`host.folder("impulses")` answers with `FolderPlaces`: `user`, and `others`, the system's and the program's that exist, in the order plugins are looked for; `all()` walks them, the user's first. Read from every place; write only to the user's. A name is 1 to 24 lowercase letters, digits and single hyphens, and none of `plugins`, `data` and `cache`. Two plugins that list the same name share the folder, as the MIDI plugin and a sampler might share `soundfonts`. `host.folder` refuses a name the manifest doesn't list.

## The score's file (interface 1.13)

A plugin that keeps paths to files the user picked can keep them relative to the open score, so a folder holding the score, its `.oxg` file and those files can be moved or shared whole: `host.score_file()` answers with the score's file as an absolute path, or `None` when no score is open or it was never saved. Resolve a relative path against its folder each time you need the file, since the score may have been saved elsewhere since. OxiGuitar opens no files for a plugin: a plugin that lets the user pick one shows its own file dialog.

## The plugin files (interface 1.15)

A plugin that manages the others (lists them, adds or removes their files) asks for what OxiGuitar found: `host.plugin_files()` answers with `PluginFiles`, holding `user_folder`, the user's plugin folder, where a plugin the user adds goes, and `files`, every `.oxp` file in the plugin folders, in the order OxiGuitar examined them (folder by folder in search order, then by name). Each `PluginFile` has:

- `path`, the file;
- `folder`, its folder's kind: `User`, `Program` (the folder that comes with the program: a portable copy's, the AppImage's, the macOS app's), `System` (where packages install) or `Listed` (another folder settings.json's `plugin_folders` names);
- `id`, `name`, `version` and `homepage`, from its manifest (`id` is `None` and the rest empty when the manifest can't be read);
- `state`: `Used`, `Disabled`, `Replaced(path)` (another copy of the same plugin is used), `Unusable(why)`, `Unmet(why)` (a plugin it needs is missing) or `Withdrawn(why)` (it failed this session);
- `loaded`, whether its code is loaded now.

Copies another file replaces are listed too, so a plugin can show every copy of a plugin and the one that is used. What changes in the folders takes effect at the next start: OxiGuitar reads the folders once, and never unloads a plugin.
