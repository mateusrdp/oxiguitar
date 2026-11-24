# Tabs: widgets and canvases

From interface 1.15 a plugin writes its UI in Slint instead ([Your own UI in Slint](ui.md)); the widgets on this page stay for plugins built before it, until OxiGuitar 1.0 retires them.

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

A tab whose manifest entry names no `area` goes to the right one, the inspector, beside Song, Track, Bar and Note (interface 1.8).

`host.show_tab("statistics")` (interface 1.8) brings a tab of yours to the front of its area, opening the area if it is closed: for a tool whose result the tab shows.

## Dialogs

A tab whose `area` is `dialog` (interface 1.8) is a dialog: it is on no tab strip, and opens over the window when the plugin asks.

```json
"tabs": [{ "id": "editor", "title": "Wah-Wah", "area": "dialog" }]
```

`host.open_dialog("editor")` opens it, from a tool or a tab's button. Its title is the manifest's; its contents are the tab's `content`, widgets and canvas as in any tab, with `on` for their events; the core adds **OK** and **Cancel** under them. When the user closes it, the core calls `Tab::closed(ok)`: `true` for OK, `false` for Cancel. Change the score there, with `host.apply`, so the dialog is one step for Undo. One dialog is open at a time: `open_dialog` answers with an error while another is, and in a program without a window.

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
| `Field { id, text, placeholder, enabled }` | One line of text to type | `Edited(String)`, when Enter is pressed or the keyboard leaves it with the text changed |
| `Choice { id, options, selected, enabled }` | A drop-down | `Chosen(u32)`, the option's index |
| `List { id, items, selected }` | Lines, one of which may be selected | `Chosen(u32)`, the line's index |
| `Canvas(Canvas { id, width, height, items })` | A drawing of the plugin's own | `Pointer(Pointer)` |
| `Link { text, url }` (interface 1.4) | Text the user clicks to open `url`, an `https://` address, in the browser; the core opens it | None |
| `Surface(Surface { id, height, items, keys, cursor })` (interface 1.14) | A drawing of the plugin's own at the window's scale, which can take the keyboard ([Surfaces](#surfaces-interface-114)) | `Resized`, `Pointer`, `Wheel`, `Focus`, `Key`, `Typed` |

Every widget the user can use has an `id`, unique in the tab (a valid id: lowercase letters and digits joined by `.` or `-`). When the user uses one, the core calls `on(id, event)`, then `content` again. The widgets hold no state of their own: a toggle shows `on` as your last `content` said, so `on` must remember what changed.

An error returned from `on` is shown in the footer, as a warning, and the tab keeps showing its contents (the core asks for them again). Return one when the plugin refuses the event; for something the user should just know, call `host.notify` and return `Ok(())`, as the template does when it can't save a setting.

The template's tab:

```rust,ignore
{{#include ../template/src/statistics.rs:content}}
```

and what it does with the user's events:

```rust,ignore
{{#include ../template/src/statistics.rs:on}}
```

### Contents the core refuses

Contents with an id missing or used twice, a slider whose range is backwards, a choice or list selecting what it doesn't have, a link that isn't an `https://` address, or a canvas that is too large or holds something it can't draw break the interface's contract: the plugin is withdrawn for the rest of the session, its tabs with it, and the footer names the first problem. `TabContent::problems()` gives the same sentences, and `cargo test` runs it on every tab's first contents; the contract suite also uses each widget once (a click, a tick, the slider's lowest value, an empty field, the first option, a press, drag and release in the middle of a canvas) and checks the contents after each.

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
| `Image { at, width, height, columns, rows, pixels }` (interface 1.14) | A picture of `columns` × `rows` pixels (each side 1 to 4096), premultiplied RGBA a byte a channel, row by row from the top, scaled to fill the box. It keeps its own colours, so prefer the other items where they do: they follow the theme. A drawing on the page leaves pictures out |

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

A press gives `Down`, moving with the button held gives `Drag`s, letting go gives `Up` (wherever the pointer is). Drags the plugin hasn't answered yet are merged, so a slow plugin gets the latest position rather than a backlog. On a canvas there is no hover: the pointer is reported only while a button is held (a surface also hears moves). The template finds the bar under a press:

```rust,ignore
{{#include ../template/src/statistics.rs:pointer}}
```

## Surfaces (interface 1.14)

A **surface** is for a plugin that draws its own controls: a text box, a chat, a piano keyboard, anything the standard widgets don't have. OxiGuitar draws no such control for you; it gives the surface what it needs to be one:

```rust,ignore
Widget::Surface(Surface {
    id: "chat".into(),
    height: 0.0,            // 0: all the room the tab has left; or a height in pixels
    items,                  // in logical pixels from the top-left corner
    keys: true,             // takes the keyboard when clicked
    cursor: Cursor::Text,   // the pointer's shape over it
})
```

- **It isn't scaled.** One unit is one logical pixel of the window (the window's scale and zoom make it sharp on any screen), and its width is the room's. Its size comes as `TabEvent::Resized { width, height }` as soon as it is laid out and whenever it changes: lay yourself out for it in your next `content`. What you draw outside the room isn't shown, so to scroll, draw from an offset of your own.
- **The pointer**, as on a canvas, plus `PointerKind::Move` (no button held) and `Leave`, so buttons of your own can light up under it. Moves you haven't answered yet are merged. **The wheel**: `TabEvent::Wheel { at, dx, dy }` in pixels, `dy` positive to show what is further down.
- **The keyboard**, when `keys` is true: a click on the surface gives it the keyboard (`Focus(true)`; `Focus(false)` when it goes). Named keys (Enter, Escape, Backspace, Delete, Tab, the arrows, Home, End, Page Up and Down, Insert, F1 to F24) and characters pressed with Ctrl or Alt come as `Key(Key { name, shift, ctrl, alt })` (Ctrl is Command on macOS); everything else is text, `Typed(String)`: what the user types, what an input method makes, and what they paste with Ctrl + V or Shift + Insert. You never read the clipboard; to copy, `host.copy(text)` puts text on it.
- **Measuring text**: `host.measure_text(text, size, font)` answers where each character of `text` starts in the core's font at `size` and where it ends (`edges`, one more than characters), with the font's `ascent` and `descent`: what you need to wrap lines, place a caret and find the character under the pointer. Text is drawn exactly where it measures.

Every event is answered with your next `content`, as for any widget, so keep it quick and do slow work on a thread of your own, calling `host.redraw` when it is done. A surface is shown in tabs and dialogs; a track section or an export's options can't hold one. The test plugin's tab `side` (`plugins/test`) has a small one.

## Tips

- One tab, one purpose. A left or right area is narrow: a column of widgets fits better there than a wide canvas. The bottom area is as tall as the track list (up to five tracks) until the user drags its splitter, so a tab there should still make sense when short. A canvas shrinks to fit, keeping its proportions, but a tab doesn't scroll: rows that don't fit are cut off, so put buttons and toggles above a canvas, as the template does.
- Lay a canvas out in round units of your own (the template's chart is 120 × 60) and let the core scale it.
- Keep `content` cheap: build it from state you already have. Do slow work (reading the score, a download) when an event or a click asks for it, and call `redraw` when it's done.
- A plugin's settings belong in its own tab; the core draws no settings forms ([Settings](settings.md)).
