# Playing the notes your own way

Since interface 1.8 a plugin can change how the notes sound: given each track's notes as the score gives them, it gives them back as they should be played, changed, added or left out. A strumming pattern, a humanizer, a plugin that plays its own marks (a swell, a tremolo, an ornament the core doesn't know) all work this way, and the core knows nothing of any of them.

## In the manifest

```json
"performs": true
```

## The performer

When the song is got ready to play or export, OxiGuitar asks the plugin about each track: `Request::Perform(Perform)`. When a track's notes and beats are the same as last time it doesn't ask again and uses the last answer, so editing one track asks about that track only. Keep the answer a pure function of what you're given. It runs off the audio thread, but the user waits for it before the song starts: keep it quick.

```rust
impl Plugin for MyPlugin {
    fn performer(&self) -> Option<&dyn Performer> {
        Some(&self.performer)
    }
}

impl Performer for MyPerformer {
    fn perform(&self, track: &Perform) -> Result<Vec<PlayedNote>, Error> {
        // The notes as they should sound.
    }
}
```

`Perform` gives:

| Field | What it is |
| --- | --- |
| `track` | The track, from 0 |
| `class` | Its class of instrument |
| `visits` | The bar each visit plays, in playing order, repeats unrolled; a note's `visit` indexes it |
| `notes` | Its notes by start time, as the score gives them, or as the performing plugins before yours gave them back |
| `beats` | Its beats that carry marks or plugins' data, each time they are played (`PlayedBeat`, as effects that follow the score get them): where you find your own data |

A `PlayedNote` is a note as instruments get it ([Instruments](instruments.md)): its staff, voice, visit and beat, string (0 for none), key, start and end in seconds of the score at 100 % speed, velocity, sound, harmonic, attack, flags and pitch curve as `(seconds, cents)` points. Change any of them, add notes (give them the place of the beat they belong to) or leave some out.

The answer must keep the interface's limits: keys 0 to 127, velocities 1 to 127, times that are numbers, never negative, a note not ending before it starts, a visit that exists, and at most 1,000,000 notes. An answer outside them, or an error, leaves the track as written, and the footer says why. The contract suite asks every plugin that performs to play a track of two notes, one on a beat with an empty object as its data, and checks the answer keeps the limits.

## Taking turns

Performing plugins take turns in plugin order, each given what the one before gave back. The notes the player plays are the last answer: the cursor, the practice tools and audio export follow them too. Chord diagrams that are played (a track's chord voice) aren't given.
