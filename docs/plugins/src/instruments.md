# Instruments

An instrument turns the player's timed events into sound for the tracks given to it: a MIDI synthesiser, a modelled guitar amplifier, sampled drums. The core plays the score: it turns it into a timeline of notes with every effect, tempo, the metronome and count-in, runs the transport, and mixes the instruments' sound (volume, pan, mute, solo) before an audio-device service sends it to the system.

**The instrument interface comes with the player (OxiGuitar 1.0).** Interface 1.0 already lets a manifest declare instruments, so the window knows them, but there is no `Instrument` trait yet, and nothing calls one. The player will ignore instruments declared by a plugin built for an interface older than the instrument interface, so declaring one now does nothing: declare instruments when you implement them, with the SDK that brings the trait.

What is settled, so you can plan:

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
- **Audio crosses as raw sample buffers**, rendered on the audio thread: no allocation, no locks, no waiting there. The contract suite will check that.
- **Large data** (SoundFonts, amplifier models) goes in the plugin's data folder, downloaded only when the user asks, through a consent dialog of the core's that comes with the first plugin needing it.
