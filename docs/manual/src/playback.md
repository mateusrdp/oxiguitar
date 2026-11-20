# Playing a score

OxiGuitar plays the open score through your sound card, using the plugins that make and send the sound. Its controls are the **Player** tab at the top of the window and each track's row at the bottom.

![The Player tab](images/player.png)

## Play, pause and stop

| Control | Key | Does |
| --- | --- | --- |
| Play | Space | Plays from the caret's beat (click a note to put the caret there) |
| Pause | Space | Stops the sound where it is; the caret stays on the beat you heard, so Play goes on from there |
| Stop | | Stops the sound and puts the caret back where playing began |
| Play Selection | | Plays what is selected, then stops. Select with Shift and the arrows or Shift + click |
| Rewind | | Back to the start of the song (or of the loop) |
| | Ctrl + Left / Ctrl + Right | A bar back or on: the sound while it plays, the caret otherwise |

A blue line moves through the music with the sound, and the beat you are hearing is lightly boxed in blue (the caret is green). The page turns by itself to keep it in sight. While a song plays, each track's overview at the bottom shows where the sound is; click an overview to jump there.

**The score can't be edited while it plays.** Pause it (Space), edit, and play on. If you type while it plays, the bottom of the window reminds you. The plugins' sections of the **Track** tab are the exception: many of their settings work while the song plays ([Each track's sound](#each-tracks-sound)).

## Practising

- **Loop** repeats the selection over and over, or the whole song when nothing is selected. Select the bars or beats first, then turn Loop on and press Play.
- **Count-In** plays a bar of clicks before the music starts.
- **Metronome** clicks every beat, higher on the bar's first.
- **Visual Metronome** (the button with rays, next to it) flashes on every beat, brighter on the bar's first, whether or not the metronome is heard.
- **Speed** plays from 25 % to 200 % of the written tempo, in steps of 5. The pitch doesn't change.

## How the marks sound

Every effect of the score is heard as written, the notation of Guitar Pro 8's palette included:

- A **fermata** holds its beat, and everything sounding through it: half again (short), twice (medium) or three times (long) its length. Every track waits for it.
- A **crescendo** or **decrescendo** moves the notes under it towards the next dynamic, or a dynamic's step louder or softer when there is none.
- **Mordents** play the note, the note a step above (upper) or below (lower) in the key, and the note again, the first two quickly; a **turn** plays the step above, the note, the step below and the note across the note's length (an inverted turn starts below).
- **Fade out** dies away over the beat; a **volume swell** rises and dies away.
- **Wide vibrato** waves twice as far as slight; the whammy bar's slight vibrato half as far as its wide one.
- Under the **sustain pedal**, notes ring until it comes up (or their string is struck again).
- An **arpeggio** rolls the chord over the first half of its beat (at most a quarter note).
- A **rasgueado** strikes the chord once for each finger of its pattern across the beat, short-short-long for an anapaest pattern; an index finger's second stroke (i i) comes back up.
- A **golpe** is a muted knock (the lowest string for the thumb, the highest for a finger), a **dead slap** every string muted.
- Ottava lines, slurs and slash notation change how the music is written, not how it sounds; pick scrapes are shown only, as in Guitar Pro.
- A **wah-wah** mark is played by the Wah-wah plugin, and is shown only without it ([Plugin manuals](official-plugins.md)).
- A bar with a **repeat sign** plays the bar (or two bars) it repeats.

## The mixer

Each track's row at the bottom has a **mute** button (the loudspeaker crossed out), a **solo** button (headphones: when any track is soloed, only soloed tracks are heard), a **volume** slider and a **pan** knob (drag it, or use the arrow keys; a double click centres it). They start as the file sets them and change the sound as you move them, even while the song plays. **Level**, in the Player tab, is the volume of everything together.

The mixer is part of the song: **saving writes it into the file**, and **Edit → Undo** takes a move back (a whole drag of a slider or the knob is one step; Undo waits until the song is paused, as all editing does). Mute and solo are saved as they are, as Guitar Pro and TuxGuitar save them, so a track muted when you save opens muted. Each format keeps as much as it holds: Guitar Pro 7-8 (`.gp`) and TuxGuitar files keep all four; a Guitar Pro 5 file keeps volume and pan to sixteen steps and keeps mute and solo; a MusicXML or MIDI file keeps volume and pan, but neither has a place for mute or solo. **Level** is OxiGuitar's own and isn't saved.

However loud a song is, OxiGuitar never lets the sound go past just under full scale, so it doesn't distort when many tracks play at once.

## Playing the chords

A track with chord diagrams over its music (Chord…, or A) can play them too: tick **Play its chords** in its **Track** tab. Each chord sounds from the beat that shows it until the next chord, or the end of the song, as a quick downstroke of the strings its diagram shows, softly under the track's own notes. It plays with the track's instrument, sound and effects, but as a part of its own, as if on a second guitar: a distorted guitar's chords go through their own copy of its amp, so the two never pile into one muddy distortion. The track's volume, pan, mute and solo hold for its chords too. The choice is kept beside the score, in `FILE.oxg`. Only tracks with strings play chords, since a diagram is frets on strings.

## Each track's sound

Select a track (click its name) and open the **Track** tab on the right: under the track's name, instrument and strings, plugins add sections that change how that one track sounds, such as a sound set, an amp or an equalizer. Each plugin's manual describes its own ([Plugin manuals](official-plugins.md)). They are kept beside the score, in `FILE.oxg`, so the song sounds the same the next time; the score itself doesn't change.

Sections that change the sound after the instrument (an amp, an equalizer) come last, in the order they work: the sound goes through the first, then the next. The up and down arrows by such a section's heading move it, and so change that order for the track. Each change, like any edit, is one step for **Undo**.

**While the song plays**, these sections still work, so you can set a tone by ear: a setting that can change on the fly does so as you move it, and one that has to load something takes effect the next time you press Play. Moving a section up or down waits for the song to be paused or stopped, and so does **Undo**.

## Output

**Output** chooses where the sound goes: the system's default, or one of the outputs your system lists. OxiGuitar remembers the choice. It opens the output when you press Play and lets go of it a couple of seconds after the sound stops, so other programs can use it in between.

## When it can't play

The controls are greyed out when nothing can play the score, and the line at the bottom of the window says why and what to get:

- **No sound set, no instrument or no audio output**: these come from plugins. See [Plugin manuals](official-plugins.md) and [Settings and plugins](plugins.md).

Sounds are made ready when a file opens, so Play starts at once. A SoundFont 3 set is decoded the first time it is used, which can take a few seconds.

From a terminal, `oxiguitar-cli play song.gp` plays a score too; `oxiguitar-cli play song.gp --wav song.wav` writes it to a WAV file instead (see `man oxiguitar-cli`).
