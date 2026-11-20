# Drawing on the page

Since interface 1.8 a plugin can draw anything on the score's page, where it keeps data: over a staff's bar, a beat, or a beat whose notes carry its data. The core engraves what it draws with the rest of the page, so it shows on screen, prints, exports to PDF, scrolls and zooms like the music, and clicking it finds its beat. Since interface 1.13 a drawing can also run on for as long as some seconds of the music last (a run, below), and drawing needs 1.13: the request grew in place then, since no released plugin drew.

## In the manifest

```json
"draws": true
```

## The drawer

Each time the score is engraved, OxiGuitar asks the plugin about the places that carry its data or marks, and only those: `Request::Draw { places, times }`. When the places and their times are the same as last time it doesn't ask again, and uses the last answer, so drawing costs nothing while the user edits elsewhere. Keep the answer a pure function of the places and times: the same request, the same drawings. Don't read files or wait on anything there; keep what you need in your data.

```rust
impl Plugin for MyPlugin {
    fn drawer(&self) -> Option<&dyn Drawer> {
        Some(&self.drawer)
    }
}

impl Drawer for MyDrawer {
    fn draw(&self, places: &[DrawPlace], times: &DrawTimes) -> Result<Vec<Drawn>, Error> {
        // One Drawn a place at most; none where you draw nothing.
    }
}
```

Each place is one of:

| Place | What it gives |
| --- | --- |
| `DrawPlace::Bar { at: [track, staff, bar], data }` | A staff's bar with your data: `data` is it |
| `DrawPlace::Beat { at: [track, staff, bar, voice, beat], beat }` | A beat with your data or mark, on the beat or on one of its notes: `beat` is the whole beat, its notes and every extension (yours and the core's marks) |

`times` says when they are played, in seconds of the score at 100 % speed from the song's start (the clock effects that follow the score get, [Instruments and effects](instruments.md)), from OxiGuitar's own time map, tempo changes, repeats and jumps included:

| Field | What it gives |
| --- | --- |
| `starts` | For each place, by its index, when it starts each time it is played, in playing order; empty for a place never played |
| `bars` | Every bar in playing order, each time it is played (`PlayedBar { bar, start, end }`): where the playing order jumps (a repeat, an ending, a D.C.), the next one isn't the bar after |

## What you draw

A `Drawn` is a box measured in **staff spaces** (the distance between two lines of a five-line staff), so it scales with the page as the notes do:

- `place`: the index of its place in `places`;
- `width` and `height`: the box, at most 40 staff spaces each;
- `items`: what is in it, the same items as a canvas's ([Tabs](tabs.md#the-canvas)), from the box's top left corner, y downwards, a text's `size` in staff spaces too; at most 2000.

OxiGuitar stands every box on the baseline of a lane of its own above the staff (above the tablature when the staff has one), the outermost of the lanes of marks, as tall as the tallest box in that line of music. A beat's boxes start at its column and go rightwards; a bar's are drawn at its first beat, before the beat's own. The column is made wide enough for them. Draw in the four colour roles; they follow the page's theme.

- `run` (interface 1.13): `None`, or a line from the box's right edge onwards, at its middle height, for `seconds` of the music from the first time its place is played (`DrawnRun { seconds, width, paint, dashed, hook }`; more than 0 and at most a day; at most 1 staff space wide). OxiGuitar works out where that time falls, with tempo changes, and draws the line there across beats, bars and systems, with a short hook down at its end if you ask. Where the playing order jumps, or the song ends, before then, the line stops at the end of that bar, without its hook: tell the user yourself, from `times.bars`, if that matters to what you draw.

Besides the four colour roles, a drawing can use `Paint::Warning` (interface 1.13), a translucent red, for a shape laid under what it marks: something wrong the user should see.

A drawing larger than the limits, or at a place the plugin wasn't given, leaves all of that answer out, and the footer says why. The contract suite asks every plugin that draws to draw at a bar and a beat that carry an empty object as its data, and checks the answer keeps the limits.

## Clicks

A click on what a plugin drew goes to the plugin, after the caret has moved there: `Request::Clicked`, with the place as the score has it now, the point in the drawing's box in staff spaces from its top left corner, and whether it was a double click (its first click came before, as a click). The drawer's `clicked` answers as a tool does: a sentence about what it did, which the footer shows with the plugin's name, or nothing, and it can change the score as a tool can. Doing nothing is the default, so a drawer that ignores clicks needs no code for them.

```rust
impl Drawer for MyDrawer {
    fn clicked(&self, place: &DrawPlace, at: Point, double: bool) -> Result<String, Error> {
        // Toggle what the drawing shows, open a dialog, edit the beat...
        Ok(String::new())
    }
}
```

A shift-click only selects, and a click while the song plays moves the playing position, as anywhere on the page.

## What it isn't

A drawing is the plugin's view of its data, made anew when the data changes; it isn't kept anywhere. To keep something, keep data ([Tools](tools.md#your-own-data-and-marks-on-beats)). A plugin draws nothing at places without its data: data on the score itself or on a bar of every track has no place on the page yet.
