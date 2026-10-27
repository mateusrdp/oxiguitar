# Better sounds

OxiGuitar plays your scores with **sound sets**: files of recorded instruments. Out of the box it uses the small General MIDI set your system has, which sounds plain. Free recorded sets sound far better, and the best for guitars, basses and drums are **SFZ** sets. OxiGuitar never downloads anything: you download the sets, put them in a folder, and pick them once.

## Get the three recommended sets

These are the sets we recommend. We listened to every reference song with each, and OxiGuitar knows their layouts, so they play the score's hammer-ons, palm mutes, dead and ghost notes with their own recorded articulations. Hear them on the [Hear it](hear-it.md) page.

| For | Set | Download this file | Size (packed, unpacked) | Licence |
| --- | --- | --- | --- | --- |
| Electric guitars | **Black and Green Guitars** by Karoryfer Lecolds (a Gretsch Anniversary and a Hofner Club, recorded by Brian Wood) | `Karoryfer_Black_And_Green_Guitars_1000.zip` from its [Releases](https://github.com/sfzinstruments/karoryfer.black-and-green-guitars/releases) | 460 MB, 560 MB | CC0 |
| Basses | **Black and Blue Basses** by Karoryfer Lecolds | `Black_And_Blue_Basses_1002.zip` from its [Releases](https://github.com/sfzinstruments/karoryfer.black-and-blue-basses/releases) | 962 MB, 1.1 GB | CC0 |
| Drums | **MuldjordKit** by Lars Muldjord and the DrumGizmo team, SFZ by kinwie (a Tama Superstar rock and metal kit) | its [page](https://github.com/sfzinstruments/DrumGizmo.MuldjordKit): **Code → Download ZIP** | 340 MB unpacked | CC BY 4.0 |

All three together take about 2 GB of disk. OxiGuitar holds the samples of the sets a score uses in memory while the score is open, about as much again.

1. **Download** the three files. For the two Karoryfer sets take the file named above from **Releases**: their **Code → Download ZIP** and `git clone` copies have corrupted samples. The MuldjordKit has no releases; its **Download ZIP** is fine.
2. **Unpack** each zip into your `soundfonts` folder, as a whole folder. OxiGuitar creates the `soundfonts` folder the first time it starts:

   | System | The `soundfonts` folder |
   | --- | --- |
   | Linux | `~/.local/share/oxiguitar/soundfonts/` |
   | macOS | `~/Library/Application Support/OxiGuitar/soundfonts/` |
   | Windows | `%APPDATA%\OxiGuitar\soundfonts\` |

   With `OXIGUITAR_HOME` set, it is the `soundfonts` folder in that folder.
3. **Pick them**: **Tools → Plugins…**, select **SoundFont**, press **Settings…**, and choose:
   - **Electric guitars**: `01-green_keyswitch` (the Gretsch) or `02-black_keyswitch` (the Hofner);
   - **Basses**: `01-darkblack_keysw`;
   - **Drums**: `DrumGizmo MuldjordKit` (in its `Stereo` folder).

   Leave **Acoustic guitars** and **Everything else** on a General MIDI set (below). The choice holds for every file.
4. **Open your score again.** It loads the new sets, which takes a few seconds for a big kit, and plays with them.

The MuldjordKit is under CC BY 4.0: if you publish music made with it, credit Lars Muldjord and the DrumGizmo team. Karoryfer's sets are CC0 and need no credit.

## What plays without them

- **Windows** has a General MIDI set, `gm.dls`, and **macOS** one in Core Audio. These are the smallest and plainest.
- **Linux** has one when a package installs it: on Debian and Ubuntu the `oxiguitar-plugins-basic` package recommends `fluid-soundfont-gm` (FluidR3, 148 MB) or `musescore-general-soundfont-small` (MuseScore General, 40 MB), which your package manager installs with it unless told not to; on Fedora it recommends `fluid-soundfont-gm`. OxiGuitar looks in `/usr/share/sounds/sf2`, `/usr/share/sounds/sf3` and `/usr/share/soundfonts`.

A General MIDI set has one sound for each of the 128 standard instruments and a drum kit. While only your system's sets play, the SoundFont plugin's settings window says so, with a link to this page, and the first time it happens the bottom of the window says it too.

## A better set for everything else

For the instruments the SFZ sets don't cover (pianos, strings, organs, acoustic guitars), a better General MIDI set helps. Put its file in the `soundfonts` folder; it then plays before your system's set, and of several, the largest plays.

- **FluidR3_GM** (MIT licence, 148 MB): the best sounding General MIDI set we tried. On Linux it is the `fluid-soundfont-gm` package.
- **MuseScore General** (MIT licence; SoundFont 3, 86 MB, or SoundFont 2, 490 MB): close behind. On Debian and Ubuntu: `musescore-general-soundfont` (SoundFont 3) or `musescore-general-soundfont-lossless`.

A SoundFont 3 file is decoded the first time it plays, which takes a few seconds for a large set. OxiGuitar keeps the decoded copy in its cache folder (216 MB for the small MuseScore General, 490 MB for the full one), so later it loads at once; deleting it is harmless.

## Electric guitars as SoundFonts

If you'd rather keep disk use small, FreePats' electric guitars (CC0) are SoundFonts, each from its own page on GitHub (**Releases**, the file with `SF2` in its name). They come packed as `.7z`: unpack with 7-Zip on Windows, The Unarchiver on macOS, `p7zip` on Linux, and put the `.sf2` file in your `soundfonts` folder.

| Set | Download | Size packed |
| --- | --- | --- |
| Clean | [electric-guitar-FSBS-clean](https://github.com/freepats/electric-guitar-FSBS-clean/releases) | 129 MB (or `bridge-small`, 6 MB) |
| Jazz | [electric-guitar-FSBS-jazz](https://github.com/freepats/electric-guitar-FSBS-jazz/releases) | 66 MB (or `bridge-small`, 5 MB) |
| Direct | [electric-guitar-FSBS-direct](https://github.com/freepats/electric-guitar-FSBS-direct/releases) | 60 MB |

Pick one for **Electric guitars**. **Direct** sounds thin on its own: it is the raw signal an amp model starts from, for the amp modelling (NAM) that OxiGuitar 1.0 brings.

## How the settings window chooses

The SoundFont plugin's settings window (**Tools → Plugins…**, **SoundFont**, **Settings…**) has a row for each of **Electric guitars** (General MIDI's six electric sounds), **Acoustic guitars** (nylon and steel strings), **Basses** (its eight bass sounds), **Drums** and **Everything else** (banjos and the other fretted instruments among them). Each row lists every set found, with its folder.

- **The best found** plays a General MIDI set: one in your `soundfonts` folder before your system's, the larger of several. It is never an SFZ instrument.
- **An SFZ instrument** is listed as its set's folder and its own name ("… (SFZ)"). It is one instrument, not a set of 128, so it plays only in a row you pick it for. Many SFZ sets have several instruments (programs) for the same recording, with different microphones or articulations. For the recommended sets, the keyswitched programs (`…_keyswitch`, `…_keysw`) are the ones with every articulation.
- OxiGuitar reads SoundFont 2 (`.sf2`), SoundFont 3 (`.sf3`) and Downloadable Sounds (`.dls`) files, and SFZ instruments with WAV, FLAC or Ogg Vorbis samples, in folders up to five levels deep. It never changes anything there.
- If an instrument can't load, or some of its samples are missing, the bottom of the window says so and a General MIDI set plays instead.

## Other SFZ sets

Any SFZ instrument plays. One OxiGuitar has no layout for plays its notes as they are: palm mutes and dead notes ended early, hammer-ons carrying on the ringing string. OxiGuitar also knows the layouts of SM Drums, Naked Drums (its own programs) and Karoryfer's Big Rusty, Swirly and Unruly Drums, so their drums play at the right keys; of all the kits, we found the MuldjordKit the most natural.
