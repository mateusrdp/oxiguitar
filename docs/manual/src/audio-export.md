# Exporting audio

**File → Export WAV Audio…** saves the song's sound as a WAV file, which every audio program and player opens. OxiGuitar plays the song into the file as fast as your computer can, without a sound card, so a three-minute song takes seconds.

## What goes in

A dialog asks before the file dialog:

- **What**: the whole song, or the selected bars (when bars or beats are selected).
- **Tempo**: as written, or at the practice speed set in the Player area (Speed), when it isn't 100 %. The pitch stays the same, as when playing.
- **The metronome's clicks in the sound**: off unless you tick it.

Every track sounds as it does when you press Play: its sound set, its amp model and cabinet ([Amp models](amp-models.md)), and the mixer as it is now: mute, solo, volume, pan and the master level. Two seconds follow the last bar, so the last notes ring out.

The file is written under a temporary name and only takes the name you chose once it is whole: a cancelled or failed export leaves nothing, and a file you are replacing stays as it was until then. A card shows how far the export has got; **Cancel** (or Escape) stops it. The bottom of the window says how long the file is and at what rate, and names any track that played silent.

## The file's format

Pick it in the WAV plugin's settings: **Tools → Plugins…**, **WAV export**, **Settings…**:

| Setting | Choices |
| --- | --- |
| Bit depth | **16-bit**, as CDs, the smallest files; **24-bit** (the default), with room for quiet passages; **32-bit float**, for mixing further, where nothing is ever clipped |
| Rate | **44.1 kHz**, as CDs; **48 kHz** (the default), as video and most sound sets |

A minute of sound takes 10 MB at 16 bits and 44.1 kHz, 16 MB at 24 bits and 48 kHz, 22 MB in 32-bit float at 48 kHz. A WAV file holds at most 4 GB, over three hours of sound.

## From the command line

`oxiguitar-cli export song.gp song.wav` does the same: the whole song as written, with the file's own mixer settings and the WAV plugin's format. `--bars 5-8` exports some bars, `--speed 75` plays at 75 %, `--metronome` adds the clicks.
