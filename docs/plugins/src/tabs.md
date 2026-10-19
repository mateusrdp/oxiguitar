# Tabs: widgets and canvases

A tab sits in one of the window's four areas, beside the core's own (Player, Tracks, Palette). Its contents are rows of standard widgets that the core draws in the window's own style, so it looks like the rest of OxiGuitar and follows the dark or light interface by itself, and, if it needs one, a canvas the plugin draws. Tabs need interface 1.1.

## Declaring a tab

```json
"tabs": [{ "id": "statistics", "title": "Statistics", "area": "right" }]
```

| Key | What it is |
| --- | --- |
| `id` | The tab's id in the plugin |
| `title` | Its name on the area's tab strip: a word or two |
| `area` | `top` (across the window, above the page), `bottom` (across, below), `left` or `right` (beside the page, narrower) |

The window shows the tab's title from the manifest alone. The plugin is loaded when the user first opens the tab.

## Contents

```rust,ignore
pub trait Tab: Send + Sync {
    fn content(&self) -> TabContent;
    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> { Ok(()) }
}
```

`content` says what the tab shows now: `TabContent { rows }`, rows from top to bottom, each a `Row { widgets }` from left to right. The core asks for it when the tab is shown, after every `on`, and after the plugin asks for a redraw. It runs on a thread of the core's, never the window's, but keep it quick: the tab shows its old contents until it answers.

| Widget | Shows | The user's event (`TabEvent`) |
| --- | --- | --- |
| `Label { text, muted }` | Text; dimmer when `muted` | None |
| `Button { id, text, enabled }` | A push button | `Clicked` |
| `Toggle { id, text, on, enabled }` | A tickbox with its text | `Toggled(bool)` |
| `Slider { id, value, min, max, enabled }` | A slider, `min` < `max` | `Slid(f32)`, when it is let go |
| `Field { id, text, placeholder, enabled }` | One line of text to type | `Edited(String)`, when Enter is pressed |
| `Choice { id, options, selected, enabled }` | A drop-down | `Chosen(u32)`, the option's index |
| `List { id, items, selected }` | Lines, one of which may be selected | `Chosen(u32)`, the line's index |
| `Canvas(Canvas { id, width, height, items })` | A drawing of the plugin's own | `Pointer(Pointer)` |

Every widget the user can use has an `id`, unique in the tab (a valid id: lowercase letters and digits joined by `.` or `-`). When the user uses one, the core calls `on(id, event)`, then `content` again. The widgets hold no state of their own: a toggle shows `on` as your last `content` said, so `on` must remember what changed.

An error returned from `on` takes the tab's place, and the footer says it, until the plugin calls `host.redraw` for the tab. Return one only when the tab can't carry on (an event for a widget it doesn't have, say); for something the user should just know, call `host.notify` and return `Ok(())`, as the template does when it can't save a setting.

The template's tab:

```rust,ignore
{{#include ../template/src/statistics.rs:content}}
```

and what it does with the user's events:

```rust,ignore
{{#include ../template/src/statistics.rs:on}}
```

### Contents the core refuses

Contents with an id missing or used twice, a slider whose range is backwards, a choice or list selecting what it doesn't have, or a canvas that is empty, too large or holds something it can't draw are refused: the tab shows a sentence naming the first problem, and the plugin's other tabs carry on. `TabContent::problems()` gives the same sentences, and `cargo test` runs it on every tab's first contents; the contract suite also uses each widget once (a click, a tick, the slider's lowest value, an empty field, the first option, a press, drag and release in the middle of a canvas) and checks the contents after each.

## Redrawing

A tab whose contents change on their own (after an event, a timer, a finished download) calls `host.redraw("statistics")`. The core asks for `content` when the tab is next on screen; a tab no one is looking at costs nothing. Nothing polls: don't ask for redraws on a timer to look for changes.

## The canvas

A canvas is a drawing in its own units: `width` × `height`, with x to the right and y **down** from the top-left corner. The core scales it to the room the tab has, keeping its proportions, and reports the pointer in the same units, so you never deal with pixels or screen scales.

Its `items` are drawn in order, later ones over earlier ones (`oxiguitar_plugin::display`):

| Item | Draws |
| --- | --- |
| `Line { from, to, width, paint }` | A straight line with square ends |
| `Rect { at, width, height, paint }` | A filled rectangle, `at` its top-left corner |
| `Path { cmds, fill, stroke }` | A shape of `MoveTo`, `LineTo`, `CubicTo` and `Close`, filled (non-zero winding), stroked (`(width, paint)`), or both |
| `Text { text, at, size, font, anchor, paint }` | One line of text on its baseline at `at`, starting, centred or ending there (`Anchor::Start`, `Middle`, `End`); `size` is the font's em in canvas units |

Colours are **roles**, resolved by the window's theme, so a canvas follows the dark interface by itself:

| `Paint` | For |
| --- | --- |
| `Ink` | Lines, shapes and text |
| `Page` | The canvas's background colour: for knocking shapes out |
| `Accent` | Highlights: the selection, the playing note |
| `Muted` | Secondary marks |

Text crosses as text, and the core draws it with its own fonts: `Font::Sans`, the window's text font, or `Font::Music`, the music font (Bravura), where the text is [SMuFL](https://w3c.github.io/smufl/latest/) code points and `size` is a five-line staff's height, as in engraving (`"\u{E050}"` is a G clef). A plugin needs no fonts of its own.

The template draws a bar chart, one bar per string:

```rust,ignore
{{#include ../template/src/statistics.rs:draw}}
```

### Limits

A canvas is at most 100,000 units a side and holds at most 100,000 items, and every number in it is finite. Everything stays within reach of the canvas: every point lies within the canvas extended by its own size on every side, and no size, line width or text size is more than three times its longer side. Text running further is cut there. These keep a mistaken plugin from costing the window time; a real drawing never comes near them.

### The pointer

```rust,ignore
pub struct Pointer {
    pub kind: PointerKind,      // Down, Drag (moved with a button held) or Up
    pub at: Point,              // in the canvas's units
    pub button: PointerButton,  // Primary, Secondary or Middle
}
```

A press gives `Down`, moving with the button held gives `Drag`s, letting go gives `Up` (wherever the pointer is). Drags the plugin hasn't answered yet are merged, so a slow plugin gets the latest position rather than a backlog. There is no hover: the pointer is reported only while a button is held. The template finds the bar under a press:

```rust,ignore
{{#include ../template/src/statistics.rs:pointer}}
```

## Tips

- One tab, one purpose. A left or right area is narrow: a column of widgets fits better there than a wide canvas. The bottom area is as tall as the track list (up to five tracks) until the user drags its splitter, so a tab there should still make sense when short. A canvas shrinks to fit, keeping its proportions, but a tab doesn't scroll: rows that don't fit are cut off, so put buttons and toggles above a canvas, as the template does.
- Lay a canvas out in round units of your own (the template's chart is 120 × 60) and let the core scale it.
- Keep `content` cheap: build it from state you already have. Do slow work (reading the score, a download) when an event or a click asks for it, and call `redraw` when it's done.
- A plugin's settings belong in its own tab; the core draws no settings forms ([Settings](settings.md)).
