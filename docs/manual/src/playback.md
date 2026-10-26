# Playing a score

OxiGuitar plays the open score through your sound card, with the [sound sets](better-sounds.md) it finds. Its controls are the **Player** tab at the top of the window and each track's row at the bottom.

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

**The score can't be edited while it plays.** Pause it (Space), edit, and play on. If you type while it plays, the bottom of the window reminds you.

## Practising

- **Loop** repeats the selection over and over, or the whole song when nothing is selected. Select the bars or beats first, then turn Loop on and press Play.
- **Count-In** plays a bar of clicks before the music starts.
- **Metronome** clicks every beat, higher on the bar's first.
- **Visual Metronome** (the button with rays, next to it) flashes on every beat, brighter on the bar's first, whether or not the metronome is heard.
- **Speed** plays from 25 % to 200 % of the written tempo, in steps of 5. The pitch doesn't change.

## The mixer

Each track's row at the bottom has a **mute** button (the loudspeaker crossed out), a **solo** button (headphones: when any track is soloed, only soloed tracks are heard), a **volume** slider and a **pan** knob (drag it, or use the arrow keys; a double click centres it). They start as the file sets them and change the sound as you move them. **Level**, in the Player tab, is the volume of everything together. None of this is saved into the file; it lasts until you close OxiGuitar.

However loud a song is, OxiGuitar never lets the sound go past just under full scale, so it doesn't distort when many tracks play at once.

## Output

**Output** chooses where the sound goes: the system's default, or one of the outputs your system lists. OxiGuitar remembers the choice. It opens the output when you press Play and lets go of it a couple of seconds after the sound stops, so other programs can use it in between.

## When it can't play

The controls are greyed out when nothing can play the score, and the line at the bottom of the window says why and what to get:

- **No sound set**: put one in your `soundfonts` folder, or install one with your package manager ([Better sounds](better-sounds.md)).
- **No instrument or no audio output**: the basic plugins hold both (the SoundFont plugin, and an audio output for your system). See [Settings and plugins](plugins.md).

Sounds are made ready when a file opens, so Play starts at once. A SoundFont 3 set is decoded the first time it is used, which can take a few seconds.

From a terminal, `oxiguitar-cli play song.gp` plays a score too; `oxiguitar-cli play song.gp --wav song.wav` writes it to a WAV file instead (see `man oxiguitar-cli`).
