# Interface versions

The plugin interface has a major and a minor version, `MAJOR.MINOR`. A plugin's manifest says which it was built for (`"interface": "1.1"`), and the SDK it was built with knows that version.

| Version | Since | Adds |
| --- | --- | --- |
| 1.0 | OxiGuitar 0.2 | Formats (read and write), services, instruments (declared), footer messages, settings |
| 1.1 | OxiGuitar 0.2 | Tabs (widgets and a canvas, version 1 of the display list), pushed events, the open score |
| 1.2 | OxiGuitar 0.5 | The score version 2, which grows by extensions; the editor's selection (`Event::Selection2`); tools in the Tools menu and changes to the score, each one step the user can undo ([Tools and changes](tools.md)) |

**Interface 1 is frozen from OxiGuitar 0.2 on.** What exists stays as it is:

- **A newer OxiGuitar loads older plugins.** A plugin built for 1.0 or 1.1 loads in every OxiGuitar with interface 1.x; the host sends it only what its minor version knows (a 1.0 plugin is never asked for a tab).
- **Minor versions only add**: new messages, new variants at the end of an enum, new fields at the end of a C table, new manifest keys. Nothing that exists changes meaning or encoding; tests in the interface crate pin every encoding.
- **An older OxiGuitar refuses newer plugins** that ask for a minor version it doesn't have, and says so in Tools → Plugins. Ask for the lowest version that has what you use: `1.0` for a format or service plugin, `1.1` for tabs or events, `1.2` for tools, changes to the score, the editor's selection or the score version 2.
- **A new major version** (`2.0`) would have its own entry point, `oxiguitar_plugin_v2`; OxiGuitar would refuse plugins built for another major version, naming the version they need.

The score crosses the interface as its own types, which don't follow the core's model as it grows. Version 1 (`oxiguitar_plugin::score`) is frozen. Version 2 (`oxiguitar_plugin::score2`, interface 1.2) has the same tree, and each of its parts ends in a list of extensions (`ext`), an enum that later minor versions extend at its end: free time is the first (`MasterBarExt::FreeTime`). OxiGuitar leaves out the extensions a plugin's version doesn't know before sending it a score, and a plugin built for 1.0 or 1.1 gets version 1, with what version 1 can't hold named as losses when it writes a file. So there is never a version 3: what M5's notation adds arrives as extensions.

What comes next, each when a plugin needs it: instruments (with the player), exporters, toolbar sections, the plugin's data folder and its settings window. Each will be a minor version. If your plugin needs something the interface doesn't have, [open an issue](https://github.com/mateusrdp/oxiguitar/issues/new) saying what.
