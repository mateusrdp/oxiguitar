# Interface versions

The plugin interface has a major and a minor version, `MAJOR.MINOR`. A plugin's manifest says which it was built for (`"interface": "1.1"`), and the SDK it was built with knows that version.

| Version | Since | Adds |
| --- | --- | --- |
| 1.0 | OxiGuitar 0.2 | Formats (read and write), services, instruments (declared), footer messages, settings |
| 1.1 | OxiGuitar 0.2 | Tabs (widgets and a canvas, version 1 of the display list), pushed events, the open score |

**Interface 1 is frozen from OxiGuitar 0.2 on.** What exists stays as it is:

- **A newer OxiGuitar loads older plugins.** A plugin built for 1.0 or 1.1 loads in every OxiGuitar with interface 1.x; the host sends it only what its minor version knows (a 1.0 plugin is never asked for a tab).
- **Minor versions only add**: new messages, new variants at the end of an enum, new fields at the end of a C table, new manifest keys. Nothing that exists changes meaning or encoding; tests in the interface crate pin every encoding.
- **An older OxiGuitar refuses newer plugins** that ask for a minor version it doesn't have, and says so in Tools → Plugins. Ask for the lowest version that has what you use: `1.0` for a format or service plugin, `1.1` for tabs or events.
- **A new major version** (`2.0`) would have its own entry point, `oxiguitar_plugin_v2`; OxiGuitar would refuse plugins built for another major version, naming the version they need.

The score crosses the interface as version 1 of its own types (`oxiguitar_plugin::score`), which don't follow the core's model as it grows: a change plugins must see would come as a second version beside the first, never as a change to it.

What comes next, each when a plugin needs it: instruments (with the player), tools for the Tools menu, exporters, toolbar sections, parts of the score and edits as undoable commands (with the editor), the plugin's data folder, and the download consent dialog. Each will be a minor version. If your plugin needs something the interface doesn't have, [open an issue](https://github.com/mateusrdp/oxiguitar/issues/new) saying what.
