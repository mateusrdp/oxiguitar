# Tools and changes to the score

Since interface 1.2, a plugin can add entries to the Tools menu and change the open score. Every change goes through the core, as one step the user can undo and redo: a plugin never changes the score behind the editor's back.

## Tools

List each tool under `tools` in the manifest, with an `id`, the menu entry's `name` in title case, and a one-sentence `description` (`oxiguitar-cli tools` shows it):

```json
"tools": [
  { "id": "octave-up", "name": "Up an Octave", "description": "Moves every note of the score up an octave." }
]
```

Your `Plugin` hands out each one from `tool(id)`, as something implementing `Tool`. When the user picks the entry, the core calls its `run`, on a thread of the core's: return a sentence for the footer about what it did ("Moved 312 notes up an octave."), or an empty one to say nothing, or an error saying why it couldn't run. A tool runs with no score open too (the contract suite tries it), so check.

## On the palette and in other menus

Since interface 1.8 a tool says where else it shows, and a contributor's tool needs no change to OxiGuitar to be on the palette:

```json
"tools": [
  { "id": "open", "name": "Open Wah-Wah", "description": "Puts an open wah-wah mark on the caret's beat.",
    "palette": { "section": "Effects", "symbol": "U+E5E7" } },
  { "id": "count", "name": "Count the Notes", "description": "Says how many notes the selection holds.",
    "menu": "page" }
]
```

| Key | What it is |
| --- | --- |
| `palette` | A button on the palette: `section` is one of the palette's own sections (`"Effects"`) or a new one of yours, at most 24 characters, which comes after the core's; the button shows `symbol`, a glyph of the music font by its SMuFL code point (`"U+E5E7"`), or `text`, at most 5 characters. One of the two. The tool's `name` is the button's tooltip, and the palette's search finds it |
| `menu` | The menu its entry goes in: `file`, `edit`, `view`, `tools` or `help`; `page`, the right-click menu of the page; `track`, the right-click menu of a track's row; or any other name, at most 24 characters, for a menu of your own on the menu bar, between Tools and Help. Plugins that name the same menu share it |

| `toolbar` | A button in the toolbar's **Plugins** section, the one section all plugins' buttons share, after OxiGuitar's own: `symbol` or `text`, as for the palette. The button is small (an 18-pixel icon), so a symbol or one or two letters read best. For what users click often |

A palette tool is a button, not a mode: a click runs it, as a menu entry does. The same goes for a toolbar button. A tool with `palette` or `toolbar` and no `menu` is a button only; give it a `menu` too to have both. A tool with none of the three is in Tools. A manifest whose palette section or menu name is too long, or whose button has neither or both of `symbol` and `text`, is refused.

## Changing the score

A tool, a tab's button or anything else in your plugin changes the score the same way:

1. Ask for it: `host.score2()` gives the score, version 2, and its **revision**, a number that changes with every change to the score (score events carry it too). `None` when no score is open.
2. Change your copy.
3. Hand it back: `host.apply(revision, "Up an Octave", score)`. The name is what the Edit menu shows ("Undo Up an Octave"), a few words.

The core takes the whole score and records the difference as one change; the page shows it a moment later, and plugins that listen hear `Event::ScoreChanged`. If the score changed since your revision (the user typed a note while your tool worked), the core refuses the change with an error: ask for the score again and redo it, or tell the user. A score the core can't hold (a staff missing bars, a fraction with a zero denominator) is refused the same way.

## The selection

Built for 1.2, a plugin that lists `selection` under `events` hears `Event::Selection2`, the selection as the editor has it, instead of 1.1's `Event::Selection`:

- `Caret { track, staff, voice, bar, beat, string }`: nothing selected, the caret on a beat of a voice (beats count in their own voice) and on a string of the tablature (from 1, the highest; 0 on a staff without strings);
- `Beats { track, staff, voice, from, to }`: a run of beats of one voice;
- `Bars { first_track, last_track, first_bar, last_bar }`: whole bars across tracks;

and `None` when no score is open. Everything counts from 0 but the string.

A tool that only needs the selection when it runs asks for it instead (interface 1.8): `host.selection()` answers with the same `Selection2` as it is now, and the plugin needs no `selection` event. The events are for what follows the caret as it moves.

## Your own data and marks on beats

Since interface 1.8 a beat's extensions hold what a plugin keeps there:

- `BeatExt::Plugin { plugin, data }`: your data on that beat, a JSON object as text, under your plugin's id. One a plugin and beat.
- `BeatExt::Mark { plugin, text }`: a few characters (at most 16) that the engraver sets over the beat, with the playing techniques, in the text font. One a plugin and beat.

Put them on your copy of the score and hand it back with `host.apply`; take them off the same way. They travel with the beat when the user copies, cuts or moves it, and undo takes them back. No score format holds them, so OxiGuitar saves them in the file it keeps beside the score (`song.gp.oxg`) and never sends them to a format plugin, nor names them as a loss. Leave other plugins' entries as they are. An effect of yours that follows the score gets each beat's with the beat ([Instruments](instruments.md#following-the-score)).

## Reading the engraved page

`host.engraving()` (interface 1.8) gives the open score as the window shows it now, in the layout and with the tracks the user chose; `None` when no score is open or in a program without a window:

- `revision`: the score's revision it was engraved from;
- `pages`: each page's `width` and `height` in points, and its drawing as the canvas's own items ([Tabs](tabs.md#the-canvas)): text in the text or the music font, lines, rectangles and paths, in the four colour roles;
- `beats`: where each beat is: its `track`, `staff`, `bar`, `voice` and `beat`, the `page` it is on, and its box on that page (`x`, `y`, `width`, `height`, in points; the box spans its staff's height).

Read it to place something of your own in a canvas, to measure the page, or to find what is drawn where. To put something on the page itself, give a beat a mark (above) and hand the score back: the core engraves it, so it prints, exports and scrolls as the rest does. A plugin never draws on the page directly.

## The score, version 2

`host.score2()`, `Host::apply`, and the readers and writers of `reader2` and `writer2` ([Formats](formats.md)) use `oxiguitar_plugin::score2`: version 1's tree, each part ending in a list `ext` of extensions. Read the ones you know (`MasterBar2::free_time()`), keep the others as they are when you hand a score back, and add only what your format holds.
