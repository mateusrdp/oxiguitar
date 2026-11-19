# Exporting audio

**File → Export Audio…** saves the song's sound as an audio file. OxiGuitar plays the song into the file as fast as your computer can, without a sound card, so a three-minute song takes seconds. The formats come from plugins: the entry shows once a plugin that exports audio is installed ([Official plugins](official-plugins.md)).

## What goes in

A dialog asks before the file dialog:

- **Format**: every format the installed plugins export to, starting with the one you picked last. Under it, the format's own choices, from its plugin (a bit depth, a rate, a bitrate). **Export…** keeps them for the next time; **Cancel** forgets what you changed.
- **What**: the whole song, or the selected bars (when bars or beats are selected).
- **Tempo**: as written, or at the practice speed set in the Player area (Speed), when it isn't 100 %. The pitch stays the same, as when playing.
- **The metronome's clicks in the sound**: off unless you tick it.

Every track sounds as it does when you press Play: its sound set, its amp model and cabinet ([Amp models](amp-models.md)), and the mixer as it is now: mute, solo, volume, pan and the master level. Two seconds follow the last bar, so the last notes ring out.

The file is written under a temporary name and only takes the name you chose once it is whole: a cancelled or failed export leaves nothing, and a file you are replacing stays as it was until then. A card shows how far the export has got; **Cancel** (or Escape) stops it. The bottom of the window says how long the file is and at what rate, and names any track that played silent.

## The file's format

Each format has its own choices, shown under the **Format** list when it is picked. **Export…** keeps them for the next time:

| Format | Choices | Default |
| --- | --- | --- |
| **WAV** | Bit depth: **16-bit**, as CDs, the smallest files; **24-bit**, with room for quiet passages; **32-bit float**, for mixing further, where nothing is ever clipped. Rate: **44.1 kHz**, as CDs, or **48 kHz**, as video and most sound sets | 24-bit, 48 kHz |
| **FLAC** | The same sound as WAV in about half the space, with nothing lost. Bit depth: **16-bit** or **24-bit**. Rate: 44.1 or 48 kHz | 24-bit, 48 kHz |
| **MP3** | Lossy, and played everywhere. Quality: a variable bitrate (**V0**, about 245 kbps; **V2**, about 190; **V4**, about 165) or a constant one (**128**, **192**, **256** or **320 kbps**). Rate: 44.1 or 48 kHz | V2, 44.1 kHz |
| **Opus** (`.opus`) | Lossy, and smaller than MP3 at the same quality. Bitrate: **96** to **256 kbps**. Always 48 kHz. Current browsers and most players open it | 160 kbps |

The MP3 and Opus files start and end exactly where the song does, so a song loops without a gap in a player that can. A minute of sound takes 10 MB in WAV at 16 bits and 44.1 kHz, 16 MB at 24 bits and 48 kHz, 22 MB in 32-bit float at 48 kHz; in FLAC about half of that; 1.4 MB in MP3 at V2 and 1.2 MB in Opus at 160 kbps. A WAV file holds at most 4 GB, over three hours of sound.

## From the command line

`oxiguitar-cli export song.gp song.wav` does the same: the whole song as written, with the file's own mixer settings and the format's choices as the dialog keeps them. The extension picks the format: `.wav`, `.flac`, `.mp3` or `.opus`. `--bars 5-8` exports some bars, `--speed 75` plays at 75 %, `--metronome` adds the clicks.
