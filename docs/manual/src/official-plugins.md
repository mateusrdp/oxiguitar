# The official plugins

OxiGuitar opens, plays and exports files only through plugins ([Settings and plugins](plugins.md)). These are the ones the OxiGuitar project makes. Every one is free software, licensed GPL-3.0-or-later like OxiGuitar, and sends nothing anywhere.

OxiGuitar's own download holds the program only. Each plugin has a repository and releases of its own, on GitHub at `github.com/mateusrdp/` followed by the repository's name below, and is updated on its own. Our apt and dnf repositories install OxiGuitar with the essential plugins and keep them all updated; elsewhere each plugin's releases page has a zip and an installer. The install pages say how: [Linux](install-linux.md), [Windows](install-windows.md), [macOS](install-macos.md).

## Essential

These two, with an audio output, are what OxiGuitar needs to open and play a score. If one is missing, OxiGuitar tells you once, with a link here, and you can silence the message ([Advice dialogs](plugins.md#advice-dialogs)).

| Plugin | What it does | Repository |
| --- | --- | --- |
| **Guitar Pro** | Opens and saves Guitar Pro files: .gp3, .gp4, .gp5, .gpx and .gp (Guitar Pro 3 to 8). One plugin for each format, installed together | `oxiguitar-guitarpro` |
| **MIDI (SoundFont, SFZ)** | Plays the score through a sound set: SoundFont (.sf2, .sf3), DLS or SFZ. Plays at once with your system's own set, and better with the free recorded ones the sound-set installer fetches ([Better sounds](better-sounds.md)). Called **SoundFont** in OxiGuitar 0.10 and earlier | `oxiguitar-midi` |

## Audio outputs

One sends the sound to your speakers. OxiGuitar uses the one that fits your system; install the one for yours.

| Plugin | System | Repository |
| --- | --- | --- |
| **PipeWire** | Linux with PipeWire (most desktops today) | `oxiguitar-pipewire` |
| **ALSA** | Any Linux; the AppImage uses it, and PipeWire desktops pass it on to PipeWire | `oxiguitar-alsa` |
| **WASAPI** | Windows | `oxiguitar-wasapi` |
| **Core Audio** | macOS | `oxiguitar-coreaudio` |

## More

| Plugin | What it does | Repository |
| --- | --- | --- |
| **Audio export** | File → Export Audio…: the whole song or some bars as a WAV, FLAC, MP3 or Opus file ([Exporting audio](audio-export.md)). It was the WAV plugin (`oxiguitar-wav`) until its version 0.1.2: remove that one by hand | `oxiguitar-export` |
| **NAM** | Amp models (Neural Amp Modeler captures) and cabinets for guitar and bass tracks ([Amp models](amp-models.md)). Needs the MIDI plugin, which makes the sound it shapes | `oxiguitar-nam` |
| **Equalizer** | A three-band or ten-band equalizer on any track, with or without an amp model ([Playing a score](playback.md#each-tracks-sound)). Suggested by OxiGuitar's packages, as NAM is | `oxiguitar-eq` |
| **Wah-wah** | Wah-wah marks, written and played: the palette's Closed and Open Wah-Wah, an editor for each mark's curve and Q, and a wah pedal on any track that the marks rock, a Cry Baby by default ([Wah-wah](editing.md#wah-wah), [How the marks sound](playback.md#how-the-marks-sound)). Comes before the amp unless the Track tab moves it. Suggested by OxiGuitar's packages, as NAM is | `oxiguitar-wah` |
| **MusicXML** | Opens and saves MusicXML (.musicxml, .xml) and compressed MusicXML (.mxl), the format MuseScore, Finale, Sibelius, Dorico and Guitar Pro exchange scores in. Suggested by OxiGuitar's packages. Guitars and basses open as tablature, with the file's own frets when it has a tablature staff and fingered by OxiGuitar when it has only notes; other parts open as notation, their tablature one click away in the Track tab. Needs OxiGuitar 0.18 or later | `oxiguitar-musicxml` |
| **MIDI files** | Opens MIDI files (.mid, .midi, karaoke .kar, .rmi) and saves .mid files. A MIDI file holds played notes, not note values, so OxiGuitar puts each beat's notes on the grid that fits them best (straight or triplets) as it opens; guitars and basses open as tablature, fingered by OxiGuitar, other tracks as notation, channel 10 as drums. Saving writes the song as OxiGuitar plays it, repeats played out and bends as pitch bend. Not the MIDI plugin above, which plays sound sets. Suggested by OxiGuitar's packages. Needs OxiGuitar 0.18 or later | `oxiguitar-midi-file` |
| **TuxGuitar** | Opens and saves TuxGuitar 2's files (.tg): tracks, both voices, chord diagrams, lyrics and every note effect TuxGuitar has. Files from TuxGuitar 1.5 or older open once TuxGuitar 2 has saved them again; TuxGuitar 1.x can't open what OxiGuitar saves. Suggested by OxiGuitar's packages. Needs OxiGuitar 0.18 or later | `oxiguitar-tuxguitar` |
| **Temperament** | Fretted tracks play as a real guitar would, with its string stretching and its small sharpness near the nut, or as a True Temperament neck would (Thidell Formula 1), chosen per track in the track dialog. It changes only the pitches played: the score, the tablature and the saved file stay as written. A fun extra, not suggested by OxiGuitar's packages. Needs OxiGuitar 0.21.3 or later | `oxiguitar-temperament` |
| **Audio clips** | Plays an audio file along with the score. The palette's Clip tool, in its Plugins section, puts a WAV, FLAC, MP3 or Opus file on a rest; the page draws a line from that beat across bars and lines of music to the exact point where the audio ends, and the clip plays with its track (volume, pan, mute and solo apply) and in exported audio. At a practice speed below or above 100 % the pitch is kept. The file's path is kept relative to the score, so a folder with the score and its clips can be shared, or absolute, so the score can move; you choose in the Clip dialog. A clip that runs into the next one on its track, or across a repeat, is drawn in red. A fun extra, not suggested by OxiGuitar's packages. Needs OxiGuitar 0.21.4 or later | `oxiguitar-clips` |
| **AI assistant** | A chat tab in the inspector with a language model that can answer questions about your score, write and change notes in it, and run the program's tools (Transpose, Find Fingering and the plugins' tools) and move the selection, all by the OpenAI chat protocol. It works with a server on your computer (llama.cpp, Ollama, LM Studio) or a hosted one (OpenAI, Anthropic, Gemini): the settings dialog has presets, and the scheme, host, port, path and your key are separate fields saved in `settings.json`. A local server is used at once; a remote one only after you allow it, and nothing else leaves your computer. Each answer is one change you can undo; the chat is kept in memory only and files are never touched. In Flatpak, allow the network first: `flatpak override --user --share=network io.github.mateusrdp.OxiGuitar`. A fun extra, not suggested by OxiGuitar's packages. Needs OxiGuitar 0.21.5 or later | `oxiguitar-ai` |
| **Better sounds** | Not a plugin but an installer: downloads the free recorded sound sets we recommend (clean, overdriven and distorted electric guitars, nylon and steel-string guitars, ukulele, bass, drums; about 1.2 GB), into your `soundfonts` folder (by hand, after you say yes) or for every user (the Linux packages do it as they install) ([Better sounds](better-sounds.md#the-sound-set-installer)). Suggested by the MIDI plugin's packages | `oxiguitar-bettersounds-soundfonts` |

## Plugins from elsewhere

Anyone can write a plugin ([the plugin developer guide](plugin-guide/index.html)). A plugin is a file ending in `.oxp`; put it in your plugins folder and it is found at the next start. We make no promises about plugins from anywhere else, so install only plugins you trust.
