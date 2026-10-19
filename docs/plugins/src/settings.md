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
- **Show settings in your own tab.** The core draws no settings forms. A plugin with a few settings can put them in the tab it already has, or add one ([Tabs](tabs.md)).
- Users can also edit settings.json by hand while OxiGuitar is closed, so read it defensively: a value of the wrong type falls back to your default, as the template's `and_then(|v| v.as_bool())` does.

Settings are for small values. A plugin that keeps data (downloaded models, a cache) gets a folder of its own, `$XDG_DATA_HOME/oxiguitar/data/<plugin id>/` on Linux and `data/<plugin id>/` in the user folder elsewhere; the host doesn't hand its path to plugins yet.
