# The official plugins

OxiGuitar opens, plays and exports files only through plugins ([Settings and plugins](plugins.md)). These are the ones the OxiGuitar project makes. Every one is free software, licensed GPL-3.0-or-later like OxiGuitar, and sends nothing anywhere.

**Today** every OxiGuitar download carries all the plugins below that run on its system, so one file opens and plays Guitar Pro files ([Installing](installing.md)). **Coming with OxiGuitar 1.0**, each plugin gets a repository and a release of its own, and OxiGuitar itself comes without plugins; your system's package manager then installs it with the essential ones and keeps them all updated. Each entry says which it is, and the install pages say how: [Linux](install-linux.md), [Windows](install-windows.md), [macOS](install-macos.md).

## Essential

These two, with an audio output, are what OxiGuitar needs to open and play a score. If one is missing, OxiGuitar tells you once, with a link here, and you can silence the message ([Advice dialogs](plugins.md#advice-dialogs)).

| Plugin | What it does | Repository (coming with 1.0) |
| --- | --- | --- |
| **Guitar Pro** | Opens and saves Guitar Pro files: .gp3, .gp4, .gp5, .gpx and .gp (Guitar Pro 3 to 8). One plugin for each format, installed together | `oxiguitar-guitarpro` |
| **MIDI (SoundFont, SFZ)** | Plays the score through a sound set: SoundFont (.sf2, .sf3), DLS or SFZ. Plays at once with your system's own set, and better with the free ones in [Better sounds](better-sounds.md). Called **SoundFont** before OxiGuitar 1.0 | `oxiguitar-midi` |

## Audio outputs

One sends the sound to your speakers. OxiGuitar uses the one that fits your system; install the one for yours.

| Plugin | System | Repository (coming with 1.0) |
| --- | --- | --- |
| **PipeWire** | Linux with PipeWire (most desktops today) | `oxiguitar-pipewire` |
| **ALSA** | Any Linux; the AppImage uses it, and PipeWire desktops pass it on to PipeWire | `oxiguitar-alsa` |
| **WASAPI** | Windows | `oxiguitar-wasapi` |
| **Core Audio** | macOS | `oxiguitar-coreaudio` |

## More

| Plugin | What it does | Repository (coming with 1.0) |
| --- | --- | --- |
| **WAV** | File → Export WAV Audio…: the whole song or some bars as a WAV file ([Exporting audio](audio-export.md)) | `oxiguitar-wav` |
| **NAM** | Amp models (Neural Amp Modeler captures) and cabinets for guitar and bass tracks ([Amp models](amp-models.md)). Needs the MIDI plugin, which makes the sound it shapes | `oxiguitar-nam` |
| **Equalizer** | A three-band or ten-band equalizer on any track, with or without an amp model ([Playing a score](playback.md#each-tracks-sound)). Suggested by OxiGuitar's packages, as NAM is | `oxiguitar-eq` |
| **Better sounds** | Not a plugin but an installer: downloads the free recorded sound sets we recommend, after you say yes, into your `soundfonts` folder ([Better sounds](better-sounds.md)). Coming with 1.0 | `oxiguitar-bettersounds-soundfonts` |

On GitHub each repository is `github.com/mateusrdp/` followed by its name. They appear when the plugin's first release does; until then the plugin is inside OxiGuitar's own download.

## Planned

Plugins for MusicXML, ABC and MuseScore files, and for TuxGuitar's format, are planned ([ROADMAP.md](https://github.com/mateusrdp/oxiguitar/blob/main/ROADMAP.md)). They will join this page when they exist.

## Plugins from elsewhere

Anyone can write a plugin ([the plugin developer guide](plugin-guide/index.html)). A plugin is a file ending in `.oxp`; put it in your plugins folder and it is found at the next start. We make no promises about plugins from anywhere else, so install only plugins you trust.
