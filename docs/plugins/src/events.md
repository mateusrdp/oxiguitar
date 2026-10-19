# Events and the open score

A plugin that follows what the user does subscribes to events: OxiGuitar pushes each one to it as it happens. Nothing polls, and a plugin that wants nothing hears nothing. Events need interface 1.1.

## Subscribing

```json
"events": ["score"]
```

| Kind | Events | Sent today |
| --- | --- | --- |
| `score` | `ScoreOpened { revision }`, `ScoreChanged { revision }`, `ScoreClosed` | Yes |
| `selection` | `Selection(Some(Selection { track, from, to }))`, or `Selection(None)` when nothing is selected; a plugin built for interface 1.2 gets `Selection2` instead, with voice, string and several tracks ([Tools and changes](tools.md#the-selection)) | From the editor (OxiGuitar 0.5) |
| `playback` | `Playback(Playback { playing, at, seconds })`: playback started or stopped, the cursor reached another beat, or it jumped | From the player (OxiGuitar 1.0) |

Positions are a bar and a beat of the bar's first voice, both from 0; tracks are numbered from 0. A score's `revision` is a number that changes with every score opened or changed in the session.

## Receiving

```rust,ignore
fn event(&self, event: &Event) { … }
```

`Plugin::event` is called on one thread of the host's, which delivers every plugin's events in order. **Keep it short**: note what happened, call `host.redraw` for a tab that should show it, and return. While one plugin's `event` runs, the next plugin's events wait.

Events that say where something is now (the selection, the playback position, a score's revision) replace one of the same kind still waiting for your plugin, so a slow plugin gets the latest rather than a backlog. A plugin that panics on an event is withdrawn, like any plugin that fails ([What a plugin is](what.md)).

The template's tab:

```rust,ignore
{{#include ../template/src/statistics.rs:event}}
```

## The open score

Score events carry no score. A plugin that needs it asks:

```rust,ignore
let score: Option<oxiguitar_plugin::score::Score> = host.score()?;
```

`None` means no score is open. The core converts the score into the interface's types for each request, which takes milliseconds for a short song and tens of milliseconds for a long one, so ask once per revision, when you need it, rather than in every `event` or every `content`. The template asks the next time its tab is drawn, and only if a score event came since:

```rust,ignore
{{#include ../template/src/statistics.rs:score}}
```

The score you get is a copy: changing it changes nothing in OxiGuitar. Edits will go through the core as undoable commands, from the editor's release.
