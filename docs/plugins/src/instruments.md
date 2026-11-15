# Instruments

An instrument turns the player's timed events into sound for the tracks given to it: a MIDI synthesiser, a modelled guitar amplifier, sampled drums. The core plays the score: it turns it into a timeline of notes with every effect, tempo, the metronome and count-in, runs the transport, and mixes the instruments' sound (volume, pan, mute, solo) before an audio-device service sends it to the system.

**The instrument interface is interface 1.3** (OxiGuitar 0.9 and later). The player ignores instruments declared by a plugin built for an older interface.

## How an instrument works

1. **Prepared, off the audio thread.** When a song is opened or its sounds change, the core asks the instrument to get ready for the tracks it plays (`Instrument::prepare`, given a `Prepare`: the sample rate, the largest piece it will be asked for, and each track's program, tuning, capo and its [track section's](track-sections.md) settings). Load the sounds now, and touch their memory, so nothing waits later. It hands back a `Renderer`.
2. **Rendered, on the audio thread.** Each device callback is split at every event, and each piece goes to `Renderer::render` as a `Piece`:
   - `events()` all happen on the piece's first frame: a note starts (`RtEventKind::NOTE_ON`), ends (`NOTE_OFF`, the same note, by `id`), or every note stops at once (`ALL_OFF`: stop, seek, loop). A piece may have no frames, only events.
   - A note (`RtNote`) carries what it means, not MIDI: its track's place, key, velocity, string, sound (normal, dead, palm mute, harmonic of a kind), attack (picked, legato, tapped, slapped, popped), flags (ghost, let ring, fade in or out, grace, vibrato), and its start and end in the score's seconds. Its pitch curve (bends, the whammy bar, slides) is `piece.pitch_of(&note)`, and `piece.cents_at(&note, frame)` follows it frame by frame, whatever the speed. The curves are runs of one array, `piece.pitch()`, which stays put while anything sounds, so a renderer that follows a note across calls keeps the note (or its `pitch_start` and `pitch_len`) and finds its run again in the next call's array (in the SDK of 0.9.0 and later).
   - `piece.track(slot)` is a track's left and right buffers, zeroed: add into them.
   - `time()` and `step()` give the score's time at the first frame and its seconds per frame, which change with the speed. The pitch never does: playing slower re-times the notes.
3. **Released**, when the core is done with it, off the audio thread: the renderer is dropped.

Two numbers describe it: `latency()`, the frames its sound comes late by (the cursor moves back by it), and `resolution()`, the most frames late a note may start (a synthesiser rendering in blocks of 8 starts notes on the next block: 7). Both default to 0.

**The audio thread's rules**: in `render`, never allocate, lock, wait or make a system call. A device callback lasts a few milliseconds, and a late one is a click the listener hears. The contract suite checks each instrument: it prepares, a note starts within its resolution and not before, nothing is allocated while it renders (the SDK installs a counting allocator in every `.oxp`, so don't set a `#[global_allocator]` of your own), and on Linux no system call is made while it renders. A lock no other thread holds makes no system call, so the suite can't see that one: keep locks off the audio thread altogether.

## Effects

An effect works on one track's sound after its instrument, in the order the track lists them: NAM is the first. Declare it under `effects` (the same shape as an instrument, by class), and implement `Effect::prepare`, which hands back a `Processor`: `process(left, right)` changes the track's buffers in place, under the same rules.

### Following settings while the song plays

Since interface 1.5 (OxiGuitar 0.14), an effect can follow its track's settings as the user changes them in its [track section](track-sections.md) while the song plays: an equalizer's slider, an amp's level. Without it, the song keeps the settings it started with until it next starts. It takes two methods, one on each side of the audio thread:

- `Effect::update(prepare, previous)`, off the audio thread, gets what the processor was prepared with, but with the track's new settings in `prepare.track.settings`, and the settings it follows now in `previous`, to see what changed. Do the slow work here (read the JSON, compute coefficients, allocate) and hand back `Some(Update::new(value))`, whatever value the processor needs. `Ok(None)`, the default, says the processor can't follow these settings while it runs (a new model file to load, say): OxiGuitar then prepares the effect again when the song next starts.
- `Processor::apply(update)`, on the audio thread, between two calls of `process`, gets that update (`update.get::<T>()`). The audio thread's rules hold: take the values out, or swap your old ones into it, and allocate nothing; the update is freed off the audio thread. Move towards the new values over a few milliseconds instead of jumping, or the change clicks: ramp a gain, glide a filter's coefficients.

```rust
impl Effect for Level {
    fn prepare(&self, p: &PrepareEffect) -> Result<Box<dyn Processor>, Error> {
        let gain = gain_of(&p.track.settings);
        Ok(Box::new(Scale { gain, target: gain }))
    }

    fn update(&self, p: &PrepareEffect, _previous: &str) -> Result<Option<Update>, Error> {
        Ok(Some(Update::new(gain_of(&p.track.settings))))
    }
}

impl Processor for Scale {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right) {
            self.gain += (self.target - self.gain) * 0.002; // about 10 ms
            *l *= self.gain;
            *r *= self.gain;
        }
    }

    fn apply(&mut self, update: &mut Update) {
        if let Some(&target) = update.get::<f32>() {
            self.target = target;
        }
    }
}
```

The update reaches the processor before its next `process` call; if the user moves a slider faster than the audio thread runs, only the newest update is taken. The SDK's glue keeps the two threads apart with no lock. The contract suite hands each effect an update and checks that taking it allocates nothing and makes no system call. Ask for interface 1.5 in the manifest (`"interface": "1.5"`).

## Tracks and classes

- **One instrument per track.** Each track chooses its instrument, with one default for the rest.
- **Instrument classes.** The manifest says which tracks an instrument suits, from a fixed set of classes the core derives from each track's General MIDI program and drum flag:

  | Class | Tracks |
  | --- | --- |
  | `fretted` | Guitars and basses with frets, banjo, sitar |
  | `fretless` | Fretless bass, shamisen, koto |
  | `bowed` | Violin, viola, cello, contrabass, string ensembles, fiddle |
  | `keyboard` | Pianos, organs, chromatic percussion |
  | `voice` | Choirs and voices |
  | `winds` | Brass, reeds, pipes, bagpipe, shanai |
  | `synth` | Synth basses, leads, pads and effects |
  | `drums` | Drum-kit tracks |
  | `other` | Harp, timpani, orchestra hit, kalimba, percussive and sound effects |

  ```json
  "instruments": [
    { "id": "kit", "name": "Realistic drums", "plays": ["drums"] },
    { "id": "synth", "name": "Everything but drums", "does_not_play": ["drums"] }
  ]
  ```

  `plays` lists what it covers (absent: everything); `does_not_play` takes classes out. The player offers an instrument only for the tracks it plays.
- **Large data** (sound sets, amplifier models) is files the user puts in a folder, such as the `soundfonts` folder beside `plugins`; OxiGuitar and our plugins download nothing (Mateus, 21 Oct 2026). What a plugin makes from them (a cache) goes in its data folder. The player's design: [docs/design/player.md](https://github.com/mateusrdp/oxiguitar/blob/develop/docs/design/player.md).
