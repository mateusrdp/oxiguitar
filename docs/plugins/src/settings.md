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
- **Show them in a settings window** (interface 1.3): with `"settings_window": true` in the manifest and `Plugin::settings_window` returning a `Tab`, the plugin manager (Tools → Plugins) gives the plugin a Settings… button, which opens a window of the plugin's widgets: the same widgets as a tab's, without a canvas. Its `on` reads the settings, changes them and writes them back. Before 1.3, put them in a tab of your own ([Tabs](tabs.md)).
- Users can also edit settings.json by hand while OxiGuitar is closed, so read it defensively: a value of the wrong type falls back to your default, as the template's `and_then(|v| v.as_bool())` does.

Settings are for small values, and they are global. What belongs to one track of one score goes in a [section of the Track tab](track-sections.md) instead.

## Folders (interface 1.3)

`host.folders()` hands the plugin its folders, as paths:

| Folder | Where | For |
| --- | --- | --- |
| `data` | `$XDG_DATA_HOME/oxiguitar/data/<plugin id>/` on Linux, `data/<plugin id>/` in the user folder elsewhere | What the plugin keeps until the user deletes it |
| `cache` | `$XDG_CACHE_HOME/oxiguitar/<plugin id>/` on Linux, `~/Library/Caches/OxiGuitar/<plugin id>/` on macOS, `%LOCALAPPDATA%\OxiGuitar\cache\<plugin id>\` on Windows | What it can make again (a sound set decoded once); the system or the user may empty it at any time |
| `shared` | Beside `plugins` in the user folder: `soundfonts`, `nam` | Files the user puts there for plugins to read, one entry per folder the manifest lists under `"folders"` |

The host makes the plugin's own two folders the first time it asks, and the shared folders a manifest lists when OxiGuitar starts, so the user finds them before any plugin runs. Nothing is ever downloaded into them: OxiGuitar and its plugins download nothing.

## Folders of your own

Since interface 1.8 a plugin names its own shared folders: any name in the manifest's `folders`, and OxiGuitar needs no change to know it.

```json
"folders": ["impulses"]
```

Such a folder sits where `soundfonts` and `nam` do, beside `plugins`, in each of OxiGuitar's folders:

- **the user's** (`~/.local/share/oxiguitar/impulses` on Linux, the user folder elsewhere, the Flatpak's own data folder in the Flatpak): OxiGuitar creates it at start, the user puts files there, and the plugin may write there;
- **beside a system plugin folder** (`/usr/lib/oxiguitar/impulses`, `PREFIX/lib/oxiguitar/impulses`), where a package can ship files;
- **beside the program** in a portable copy (`impulses\` next to `oxiguitar.exe`).

`host.folder("impulses")` answers with `FolderPlaces`: `user`, and `others`, the system's and the program's that exist, in the order plugins are looked for; `all()` walks them, the user's first. Read from every place; write only to the user's. A name is 1 to 24 lowercase letters, digits and single hyphens, and none of `plugins`, `data` and `cache`. Two plugins that list the same name share the folder, as the MIDI plugin and a sampler might share `soundfonts`. `host.folder` refuses a name the manifest doesn't list. `folders()` still answers `soundfonts` and `nam` as before.
