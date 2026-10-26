# Better sounds

OxiGuitar plays your scores with a **sound set**: a file of recorded instruments, such as a SoundFont. It never downloads one. Out of the box it plays with the set your system already has, and you can give it better ones by putting their files in a folder.

## The set your system has

- **Windows** has one, `gm.dls`, and **macOS** has one, in Core Audio. OxiGuitar plays with these when it finds nothing better.
- **Linux** has one when a package installs it. On Debian and Ubuntu the `oxiguitar-plugins-basic` package recommends `fluid-soundfont-gm` (FluidR3, 148 MB) or `musescore-general-soundfont-small` (MuseScore General, 40 MB), and your package manager installs one with it unless you told it not to; on Fedora it recommends `fluid-soundfont-gm`. OxiGuitar looks in `/usr/share/sounds/sf2`, `/usr/share/sounds/sf3` and `/usr/share/soundfonts`.

These are General MIDI sets: one sound for each of the 128 standard instruments and a drum kit, small enough to come with a system. Windows' and macOS's are the smallest and plainest. The first time OxiGuitar gets a score ready to play with only your system's sets (when you open it), the bottom of the window says so, once, with a link to this page.

## Using a better set

Put the set's file in the `soundfonts` folder beside your plugins folder (OxiGuitar creates it when it starts):

| System | The `soundfonts` folder |
| --- | --- |
| Linux | `~/.local/share/oxiguitar/soundfonts/` |
| macOS | `~/Library/Application Support/OxiGuitar/soundfonts/` |
| Windows | `%APPDATA%\OxiGuitar\soundfonts\` |

With `OXIGUITAR_HOME` set, it is the `soundfonts` folder in that folder. OxiGuitar reads SoundFont 2 (`.sf2`), SoundFont 3 (`.sf3`) and Downloadable Sounds (`.dls`) files there, and **SFZ instruments**: a `.sfz` file with its samples (WAV, FLAC or Ogg Vorbis) in folders beside it, as their makers ship them, so put the whole unpacked folder in. It looks in folders inside it too, five levels deep, and never changes anything there.

A set in your folder plays before any of the system's, and of several, the largest plays. To choose yourself, open **Tools → Plugins…**, select **SoundFont** and press **Settings…**: pick a set for electric guitars, one for acoustic guitars, one for basses, one for drums and one for everything else. The choice holds for every file. "The best found" goes back to the automatic choice, which is never an SFZ instrument (below).

A SoundFont 3 file is decoded the first time it plays, which takes a few seconds for a large set; OxiGuitar keeps the decoded copy in its cache folder, so later it loads at once. That copy can be large (216 MB for the small MuseScore General, 490 MB for the full one); deleting it is harmless.

## Choosing a set for each instrument

A set made for one instrument usually sounds far better than a General MIDI set's version of it, so you can mix: recorded guitars from one set, the bass, the drums and the rest from another. Open **Tools → Plugins…**, select **SoundFont**, press **Settings…**, and pick a set for each of **Electric guitars**, **Acoustic guitars**, **Basses**, **Drums** and **Everything else**. Electric guitars are General MIDI's six electric sounds (jazz, clean, muted, overdriven, distortion, harmonics), acoustic guitars its nylon and steel strings, and basses its eight bass sounds; banjos and the other fretted instruments go with everything else, so they keep their own sounds. A set made for one instrument only has that instrument, so pick it only for its own row and leave a General MIDI set for the others.

**SFZ instruments** are listed in the same rows, as their set's folder and their own name ("… (SFZ)"). An SFZ file is one instrument, not a set of 128, so it plays only in a row you pick it for. Many SFZ sets come with several instruments (programs) for the same recording, with different microphones or articulations: try them. OxiGuitar loads an SFZ instrument's samples into memory, whole, when you open a score, so playing never waits for the disk; a big drum kit can take a few seconds and some gigabytes of memory. If an instrument can't load, or some of its samples are missing, the bottom of the window says so and a General MIDI set plays instead.

## Sets worth getting

All of these are free to use. Download them from their makers' pages, unpack them if they come packed (the FreePats files are `.7z`: 7-Zip on Windows, The Unarchiver on macOS, `p7zip` on Linux), then put the `.sf2` or `.sf3` file in your `soundfonts` folder.

### For everything

- **FluidR3_GM** (MIT licence, 148 MB): a complete General MIDI set, the best sounding of those we tried. On Linux it is the `fluid-soundfont-gm` package.
- **MuseScore General** (MIT licence; SoundFont 3, 86 MB, or SoundFont 2, 490 MB): MuseScore's set, close behind. On Debian and Ubuntu: `musescore-general-soundfont` (SoundFont 3) or `musescore-general-soundfont-lossless`.

### Guitars

FreePats' electric guitars (CC0: free for anything, no credit needed), each from its own page on GitHub (**Releases**, the file with `SF2` in its name):

| Set | Download | Size packed |
| --- | --- | --- |
| Clean | [electric-guitar-FSBS-clean](https://github.com/freepats/electric-guitar-FSBS-clean/releases) | 129 MB (or `bridge-small`, 6 MB) |
| Jazz | [electric-guitar-FSBS-jazz](https://github.com/freepats/electric-guitar-FSBS-jazz/releases) | 66 MB (or `bridge-small`, 5 MB) |
| Direct | [electric-guitar-FSBS-direct](https://github.com/freepats/electric-guitar-FSBS-direct/releases) | 60 MB |

Pick one for **Electric guitars** in the settings window. **Direct** sounds thin on its own: it is the raw signal an amp model starts from, for the amp modelling (NAM) that OxiGuitar 1.0 brings; get it for then.

### Basses and drums

For now, the General MIDI sets above play the basses and the drums. The best free recorded basses and drum kits (Karoryfer's basses and kits, SM Drums, Naked Drums) come in the SFZ format, which the SoundFont plugin now plays (above); this page will name the ones to get once we have listened to them with OxiGuitar.
