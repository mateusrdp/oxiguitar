# Track sections

A plugin can add rows of its own to the **Track tab** in the window's right-hand area (interface 1.3, where it was the track dialog), under a heading, for the tracks it cares about: the NAM plugin adds its amp model and impulse response for guitar tracks. What the user sets there belongs to that track of that score.

Declare each section in the manifest:

```json
"track_sections": [
  { "id": "amp", "title": "Amp", "plays": ["fretted", "fretless"] }
]
```

`plays` and `does_not_play` pick the tracks by [instrument class](instruments.md), as instruments do; without them a section shows for every track. Then return it from `Plugin::track_section(id)`, as a value implementing `TrackSection`:

- `content(track, settings)` returns the rows to show: the widgets of a [tab](tabs.md), without a canvas. `track` is a `TrackInfo` (its index, name and class); `settings` is the track's settings for this plugin, a JSON object, empty the first time.
- `on(track, settings, widget, event)` changes `settings` to match what the user did; the section is shown again from the new settings.

The plugin keeps nothing itself: OxiGuitar holds each track's settings and sends them with every call. They change with every use of a widget, and OxiGuitar then writes them into the track as one change the user can undo, so the player prepares the track with them at once and a user can adjust a tone while the song plays. Merely showing a section changes nothing. An error from `on` is said in the footer, and the settings stay as they were.

No score format holds a plugin's settings, so OxiGuitar saves them beside the score: `song.gp3` keeps them in `song.gp3.oxg`, a small JSON file, one entry per track. Saving the score writes it, opening reads it, and `oxiguitar-cli convert` carries it to the new file. A track whose place changed (because another program moved tracks) finds its entry by name. A plugin gets a track's settings again when the player prepares that track (with the instrument interface).

The SDK re-exports serde_json as `oxiguitar_plugin::serde_json`, so a plugin needs no dependency of its own for the settings.
