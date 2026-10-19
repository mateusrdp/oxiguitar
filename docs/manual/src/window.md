# The window

From top to bottom:

- **Menus**: File, View, Tools, Help.
- **Toolbar**: buttons in groups. File: Open, Save, Export PDF. Edit: Undo, Redo. Save, Undo and Redo are greyed out until the editor arrives. **View → Toolbar** hides it. Plugins will add their own groups.
- **Top panel**: the **Player** tab, with play, play selection, pause, stop and rewind, count-in, loop and metronome, and the choice of output (how the score will sound). They stay greyed out until an output plugin is installed ([Settings and plugins](plugins.md)).
- **Left panel, the score, right panel.** The left panel holds the **palette**, every notation and effect tool of the editor as a button, in groups (voices and modes, bar, note, effects, notation, automation). They are greyed out until the editor arrives; hover over one to see its name. Click a group's heading to fold it away or bring it back. The right panel will hold the properties of what you select, with the editor; today it holds a note saying so and starts folded away. The score sits on a dark background whatever the page colour, with its zoom slider in the corner ([Zoom](zoom.md)).
- **Bottom panel**: the **Tracks** tab, each track with its mixer, its notation buttons and an overview of the track across the song ([Tracks and notation](tracks.md)).
- **Messages**: one line at the very bottom with the latest error or warning: what went wrong opening a file, what is missing, a plugin that stopped working. It shows nothing else, so it stays quiet while everything works. Click it to see every message since OxiGuitar started; the chevron in its corner folds the list back.

## Panels

Each panel has tabs. Click a tab's name to show it; click the name of the tab that is already showing, or the chevron at the end of the tab strip, to fold the panel away to its strip, and again to bring it back. A folded side panel is a thin strip: click it to open it. Drag the edge between a panel and the score to resize it.

**View → Panels** hides or shows each panel altogether (F7 for the left one, F8 for the bottom one). Plugins can add tabs to any panel. They appear after OxiGuitar's own tabs, with the plugin's name for them, and look like the rest of the window: the plugin decides what is in them (buttons, tickboxes, sliders, lists, drawings), and OxiGuitar draws it in its own style and colours. A plugin's tab may take a moment the first time it opens, while the plugin loads.
