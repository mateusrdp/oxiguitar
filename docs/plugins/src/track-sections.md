# Track sections

A plugin can add rows of its own to the **Track tab** in the window's right-hand area (interface 1.3, where it was the track dialog), under a heading, for the tracks it cares about: the NAM plugin adds its amp model and impulse response for guitar tracks. What the user sets there belongs to that track of that score.

Declare each section in the manifest:

```json
"track_sections": [
  { "id": "amp", "title": "Amp", "plays": ["fretted", "fretless"] }
]
```

`plays` and `does_not_play` pick the tracks by [instrument class](instruments.md), as instruments do; without them a section shows for every track. The section itself is one of your [views](ui.md), whose place is `track`, with the same id:

```json
"views": [
  { "id": "amp", "component": "AmpSection", "place": "track", "title": "Amp" }
]
```

When the Track tab shows a track the section is for, OxiGuitar makes the view and calls `Ui::shown(view, Some(track))`: `track.track` is a `TrackInfo` (its index, name and class), `track.settings` the track's settings for this plugin, a JSON object, `{}` the first time. Set the view's properties from them. When the user changes something, change the settings and hand them back with `host.set_track_settings(view, &settings)`; OxiGuitar writes them into the track as one change the user can undo. The contract suite refuses a track section with no view of its own (since interface 2.0, when OxiGuitar stopped drawing sections' widgets itself).

A section works while the song plays too (OxiGuitar 0.14): its settings are the one edit allowed then. An effect built for interface 1.5 or later follows them at once, as it plays ([Following settings while the song plays](instruments.md#following-settings-while-the-song-plays)); anything else (an instrument's settings, an effect that can't follow) takes them when the song next starts. Moving a section up or down, which changes the order of the track's effects, waits for the song to stop. Merely showing a section changes nothing. An error from a callback is said in the footer, and the settings stay as they were.

No score format holds a plugin's settings, so OxiGuitar saves them beside the score: `song.gp3` keeps them in `song.gp3.oxg`, a small JSON file, one entry per track. Saving the score writes it, opening reads it, and `oxiguitar-cli convert` carries it to the new file. A track whose place changed (because another program moved tracks) finds its entry by name. A plugin gets a track's settings again when the player prepares that track (with the instrument interface).

The SDK re-exports serde_json as `oxiguitar_plugin::serde_json`, so a plugin needs no dependency of its own for the settings.
