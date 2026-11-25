# Your own UI in Slint

From interface 1.15 (#346), a plugin writes its UI in [Slint](https://slint.dev), the same toolkit OxiGuitar uses. You write a `.slint` file; OxiGuitar compiles it with Slint's interpreter the first time one of your views is shown, and runs it with its own Slint, its own widgets (the Fluent style), its own fonts and its own light or dark colours. Your plugin carries no widget code and no copy of Slint, and OxiGuitar draws nothing of its own in your views.

Your Rust code never touches the UI directly. It **sets properties** and **hears callbacks**, as data that crosses the plugin interface, so your plugin keeps working when OxiGuitar upgrades Slint or its compiler.

## Requirements

- **Interface 1.15**, and the SDK of an OxiGuitar release with it; **1.16** to name your file in the manifest (below).
- **Slint's language at OxiGuitar's version**: OxiGuitar compiles your file with the Slint it is built with (Slint 1.19 for interface 1.15). A file that uses something newer is refused, with the compiler's message in the footer. Write for that version: [its language reference](https://slint.dev/releases/1.19.0/docs/slint/).
- **No compiler or Slint version to match.** Your plugin isn't built against Slint at all: only the `.slint` text crosses the interface.

## Declaring views

Each place your UI appears is a **view**: an exported component of your file, and where it goes.

```json
"views": [
  { "id": "manager", "component": "ManagerWindow", "place": "window" },
  { "id": "chat", "component": "Chat", "place": "right", "title": "Chat" },
  { "id": "amp", "component": "AmpSection", "place": "track", "title": "Amp" },
  { "id": "ogg-options", "component": "OggOptions", "place": "export" }
]
```

| Key | What it is |
| --- | --- |
| `id` | The view's id in the plugin |
| `component` | The exported component of your `.slint` file it shows |
| `place` | `window`, `top`, `bottom`, `left`, `right`, `track` or `export` (below) |
| `title` | The tab's name for a panel, the heading of a track section; a window takes its title from its component |

| Place | What it is | Its component |
| --- | --- | --- |
| `window` | A window of its own, which your plugin opens and closes, and the user can close | inherits `Window`, or `Dialog` for a dialog with Slint's standard buttons |
| `top`, `bottom`, `left`, `right` | A panel: a tab in that area of OxiGuitar's window | an ordinary component, not a `Window` |
| `track` | A section of the Track tab, in place of a track section's widgets; the manifest's `track_sections` needs an entry of the same id, which says which tracks it shows for | an ordinary component |
| `export` | An exporter's options in File → Export Audio…; the exporter's `options` names the view | an ordinary component |

Panels, track sections and export options sit inside OxiGuitar's window, in Slint's `ComponentContainer`, which Slint 1.19 marks as experimental. OxiGuitar follows Slint if it changes; your file doesn't depend on it.

## The file and the callbacks

Put your `.slint` file in your crate's `ui` folder, which goes into your plugin file ([The plugin file](manifest.md)), and name it in the manifest (interface 1.16):

```json
"ui": "ui/my-plugin.slint"
```

OxiGuitar compiles it from where your plugin file is unpacked, so it can import your other `.slint` files (`import { Row } from "row.slint";`) and use your pictures (`@image-url("../assets/logo.png")`), as in any Slint project. A plugin that asks for interface 1.15 has no `ui` key: its `Ui` hands over the file's text instead, with `fn source(&self) -> String { include_str!("../ui/my-plugin.slint").into() }`, and imports only Slint's own widgets.

Give the plugin a `Ui` (from `oxiguitar_plugin`), and say what each callback does:

```rust,ignore
struct MyUi { host: Host }

impl oxiguitar_plugin::Ui for MyUi {
    fn shown(&self, view: &str, track: Option<&TrackView>) -> Result<(), Error> {
        // The view was made: set what depends on what it shows.
        Ok(())
    }

    fn callback(&self, view: &str, callback: &str, args: Vec<Value>) -> Result<(), Error> {
        match (view, callback) {
            ("manager", "remove") => { /* … then set properties again */ }
            _ => {}
        }
        Ok(())
    }

    fn closed(&self, view: &str, accepted: bool) -> Result<(), Error> {
        Ok(())
    }
}

impl Plugin for MyPlugin {
    // …
    fn ui(&self) -> Option<&dyn oxiguitar_plugin::Ui> {
        Some(&self.ui)
    }
}
```

All of a plugin's views are components of **one file**, so they share their structs and helper components. Every callback of a view's component reaches `callback`, by its name in the file, with its arguments; a callback returns nothing to the UI. Callbacks come on a thread of OxiGuitar's, never its UI thread, so a slow answer never freezes the window: do the work, then set properties.

## Properties

```rust,ignore
host.set_view("manager", vec![
    ("lines".into(), Value::List(lines)),
    ("status".into(), Value::String("Added.".into())),
])?;
```

`Value` is `Bool`, `Number` (Slint's `int`, `float`, lengths and durations), `String`, `List` (a model to the UI: an array of structs for a `ListView`), `Struct` (fields by name) and `Image` (premultiplied RGBA, at most 4096 pixels a side). A callback's enumeration values and colours come as text.

OxiGuitar keeps the values you set for each view. A view made again (a window reopened, a panel shown again, the Track tab on another track) starts with them, so set a property when it changes rather than when a view is made. A property your component doesn't have, or a value of the wrong type, is named in the footer.

## Opening, closing, and where the user opens it

- `host.show_view("manager")` opens a window, or brings it forward if it is open; for a panel it brings its tab to the front. Open a window from a tool (a menu entry) or a callback.
- `host.close_view("manager")` closes a window. When the user closes it, `closed(view, false)` tells you.
- **Track sections** are made when the Track tab shows a track they show for: `shown` gets the track and its settings for your plugin. When your section changes them, `host.set_track_settings(view, &settings)` puts them into the track as one change the user can undo, as a section's widgets' settings go.
- **Export options** are made when the export dialog shows your exporter. `closed(view, true)` comes when the user presses OK, before your exporter's `start`: keep the options in your settings there and read them when the export starts, as before.

## Testing

The contract suite (`oxiguitar_plugin_test::check`) compiles your file as OxiGuitar does and checks that every view's component is in it, then tells each view it was made and closed. `oxiguitar_plugin_test::compile_ui(source)` compiles it in your own tests, so a mistake in the file fails `cargo test`, not your users' window.

## Before interface 1.15

Tabs, dialogs, track sections and export options were rows of widgets that OxiGuitar drew ([Tabs](tabs.md)). They still work, for plugins built before 1.15, until OxiGuitar 1.0 retires them; new plugins should use views.
