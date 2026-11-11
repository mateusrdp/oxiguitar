# Better sounds

OxiGuitar plays your scores with **sound sets**: files of recorded instruments. Out of the box it uses the small General MIDI set your system has, which sounds plain. Free recorded sets sound far better, and the best for guitars, basses and drums are **SFZ** sets. OxiGuitar itself never downloads anything: the sound-set installer below fetches the sets we recommend after asking you, or you download sets yourself. The installer's sets play by themselves; sets you download yourself you put in a folder and pick once.

## The sound-set installer

The easiest way: **oxiguitar-bettersounds-soundfonts** installs one good, free recorded instrument for each job:

| Set | Sound | Size | Licence |
| --- | --- | --- | --- |
| Clean Guitar | Clean electric guitar (FreePats) | 128 MB | CC0 |
| Overdriven Guitar | Overdriven electric guitar (FreePats) | 136 MB | CC0 |
| Distorted Guitar | Distorted electric guitar (FreePats) | 313 MB | CC0 |
| Nylon Guitar | Nylon-string guitar (FreePats) | 5 MB | CC0 |
| Steel String Guitar | Steel-string guitar (FreePats) | 13 MB | GPL-3.0 or later, with an exception for music made with it |
| Ukulele | Ukulele (Mateusz Dąbrowski, for FreePats) | 3 MB | CC0 |
| Bass | Black and Blue Basses, plucked (Karoryfer Samples) | 471 MB | CC0 |
| Drum Kit | MuldjordKit (Lars Muldjord, DrumGizmo) | 139 MB | CC BY 4.0 |

About 1.2 GB in all; you can take any of them. Before it downloads anything it shows each set's size, licence, author and the hosts its files come from, and waits for your yes; every file is checked against the checksum it was pinned with.

- **Debian, Ubuntu, Fedora:** the package `oxiguitar-bettersounds-soundfonts` from our repositories ([Installing on Linux](install-linux.md)), which the MIDI plugin's package suggests. Installing it downloads the sets into `/usr/share/oxiguitar/soundfonts`, for every user: apt asks first, with yes as the answer it takes when it isn't asked (an unattended install downloads them all), and dnf downloads them as it installs (`OXIGUITAR_SKIP_SOUNDS=1` in its environment skips that). Say no, or let a download fail, and `sudo oxiguitar-install-sounds --system` chooses and downloads them any time later. Removing the package removes the sets it installed.
- **Flatpak:** run `install.sh` (below). The Flatpak can't see the sets the `.deb` and `.rpm` put in `/usr/share/oxiguitar/soundfonts`, nor your own `soundfonts` folder: when OxiGuitar's Flatpak is installed, `install.sh` installs into its folder, `~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/soundfonts/`, too, and says so ([Flatpak](install-linux.md#flatpak-any-linux)).
- **Any system:** from the installer's [releases page](https://github.com/mateusrdp/oxiguitar-bettersounds-soundfonts/releases), `install.sh` (Linux, macOS: `bash install.sh` in a terminal) or `install.ps1` (Windows: in PowerShell, `powershell -ExecutionPolicy Bypass -File .\install.ps1`). They install into your own `soundfonts` folder (below). `--list` (`-List`) shows what is on offer, `--sets bass,drums` (`-Sets bass,drums`) takes only some, and `--uninstall` (`-Uninstall`) removes what it installed.

**There is nothing to pick.** Each set says which kind of track it is made for, so OxiGuitar plays clean guitars with Clean Guitar, overdriven ones with Overdriven Guitar, distorted ones with Distorted Guitar, basses with Bass and drums with the Drum Kit, from the next time a score plays. Nylon Guitar and Ukulele share **Nylon-string guitars**, where the larger, Nylon Guitar, plays (pick the Ukulele there, or give one track its own sound: [One track's own sound](#one-tracks-own-sound)); Steel String Guitar has **Steel-string guitars**. To use another set for a kind of track, pick it in the MIDI plugin's settings window (**Tools → Plugins…**, **MIDI (SoundFont, SFZ)**, **Settings…**): each set is listed by its name, with its licence and credit, and each row's first entry says which set plays when you pick nothing. A pick always wins. The installer writes a `CREDITS.txt` beside the sets; the drum kit's licence asks that music you publish with it credit "Drum samples provided by DrumGizmo.org".

## Sets to download yourself

These are sets we also recommend, larger and with more articulations. We listened to every reference song with each, and OxiGuitar knows their layouts, so they play the score's hammer-ons, palm mutes, dead and ghost notes with their own recorded articulations. Hear them on the [Hear it](hear-it.md) page.

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
   | Linux, the Flatpak | `~/.var/app/io.github.mateusrdp.OxiGuitar/data/oxiguitar/soundfonts/` |
   | macOS | `~/Library/Application Support/OxiGuitar/soundfonts/` |
   | Windows | `%APPDATA%\OxiGuitar\soundfonts\` |

   With `OXIGUITAR_HOME` set, it is the `soundfonts` folder in that folder.
3. **Pick them**: **Tools → Plugins…**, select **MIDI (SoundFont, SFZ)**, press **Settings…**, and choose:
   - **Clean guitars**: `01-green_keyswitch` (the Gretsch) or `02-black_keyswitch` (the Hofner);
   - **Basses**: `01-darkblack_keysw`;
   - **Drums**: `DrumGizmo MuldjordKit` (in its `Stereo` folder).

   These guitars are clean. Leave **Overdriven guitars** and **Distorted guitars** on a General MIDI set, whose own overdriven and distortion guitars then play there, or pick a distorted set of your own; or play the clean guitars through an amp model ([Amp models (NAM)](amp-models.md)). Leave **Nylon-string guitars**, **Steel-string guitars** and **Everything else** on a General MIDI set (below). The choice holds for every file; one track can play another row's set, or a set of its own ([One track's own sound](#one-tracks-own-sound)).
4. **Play.** A change in the settings window takes effect at once: if the song was playing it stops, the new sets load (a few seconds for a big kit) and the ones no longer picked are let go, and the next Play uses them.

If you publish music made with these sets, credit them: Karoryfer Lecolds for the guitars and basses, and, as the MuldjordKit's licence (CC BY 4.0) asks, Lars Muldjord and the DrumGizmo team for the drums.

## What plays without them

- **Windows** has a General MIDI set, `gm.dls`, and **macOS** one in Core Audio. These are the smallest and plainest.
- **Linux** has one when a package installs it: on Debian and Ubuntu the `oxiguitar-midi` package recommends `fluid-soundfont-gm` (FluidR3, 148 MB) or `musescore-general-soundfont-small` (MuseScore General, 40 MB), which your package manager installs with it unless told not to; on Fedora it recommends `fluid-soundfont-gm`. OxiGuitar looks in `/usr/share/sounds/sf2`, `/usr/share/sounds/sf3` and `/usr/share/soundfonts`, and, for the recorded sets the sound-set installer puts there, in `/usr/share/oxiguitar/soundfonts`.

A General MIDI set has one sound for each of the 128 standard instruments and a drum kit. While only your system's sets play, the MIDI plugin's settings window says so, with a link to this page, and the first time it happens a dialog says it too ([Advice dialogs](plugins.md#advice-dialogs); tick **Don't show this again** to silence it, `no-sound-sets` in `settings.json`).

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

Pick **Clean** or **Jazz** for **Clean guitars**. **Direct** sounds thin on its own: it is the raw signal an amp model starts from ([Amp models (NAM)](amp-models.md)); pick it for one track ([One track's own sound](#one-tracks-own-sound)) or for **Clean guitars**. FreePats' overdriven and distorted guitars (`electric-guitar-FSBS-dist2` and `-dist1`, on GitHub too) suit **Overdriven guitars** and **Distorted guitars**.

## How the settings window chooses

The MIDI plugin's settings window (**Tools → Plugins…**, **MIDI (SoundFont, SFZ)**, **Settings…**) has a row for each kind of instrument. A track goes to the row its General MIDI instrument names, which Guitar Pro files keep:

| Row | General MIDI instruments |
| --- | --- |
| **Clean guitars** | Jazz (27), Clean (28), Muted (29) and Harmonics (32) electric guitar |
| **Overdriven guitars** | Overdriven guitar (30) |
| **Distorted guitars** | Distortion guitar (31) |
| **Nylon-string guitars** | Nylon-string guitar (25) |
| **Steel-string guitars** | Steel-string guitar (26) |
| **Basses** | The eight basses (33–40) |
| **Drums** | The drum kit |
| **Everything else** | The rest, banjos and the other fretted instruments among them |

(The numbers are General MIDI's, counted from 1 as its list does.) A distorted bass has no General MIDI instrument of its own: give that track its own sound (below), or play a clean bass through an amp model. Each row lists every set found, with its folder. A set whose folder holds an `oxiguitar-set.json` file is listed by a short name, its source and its patch, such as `FreePats: Electric Guitar FSBS (clean)`; the installer writes one beside each set it installs, and you can write one for a set of your own:

   ```json
   { "name": "Black and Green Guitars", "role": "clean-guitars", "licence": "CC0", "credit": "Karoryfer Samples" }
   ```

   Every key is optional. The list shows `name`, then the product and variant taken from the parentheses at the end of `credit` (`"FreePats (Electric Guitar FSBS, clean)"` gives `Electric Guitar FSBS (clean)`). A set of several instruments (patches) may add `"recommended": "Programs/Bass.sfz"` and `"patches": { "Programs/Bass.sfz": "plucked", "Programs/01-dark.sfz": "dark" }` (paths from the set's folder), which give each patch its name in the list, and only the recommended one plays where nothing is picked. `role` (`clean-guitars`, `overdriven-guitars`, `distorted-guitars`, `nylon-guitars`, `steel-guitars`, `basses` or `drums`) says which row the set is made for; `electric-guitars`, from before the three electric rows, stands for all three, and `acoustic-guitars`, from before nylon and steel strings were split, for both acoustic rows. Where nothing is picked for a row, OxiGuitar plays the best set whose `role` names that row (yours before the installed ones, those before the system's, then the larger first), so a set with a `role` plays by itself; a set without one plays only where you pick it. The file is only read; nothing is sent anywhere. A settings file from before the three electric rows keeps working: its one **Electric guitars** pick holds for each of the three until that row has a pick of its own.

- **Automatic** is a row's first entry, and says what plays: the best set whose `oxiguitar-set.json` names the row's role, if there is one; else a General MIDI set, one in your `soundfonts` folder before your system's, the larger of several. A General MIDI set is the only fallback; an SFZ instrument with no `role` is never it.
- **One patch or all of them.** The clean guitar has 2 patches and the bass 11 (650 MB more). The installer asks, for each set that has others, whether to install them all or only the recommended one (`--patches all` or `--patches recommended` answers for every set; run it again to change your mind). With all of them installed, every patch is listed by its name in each row, and the recommended one still plays by itself.
- **An SFZ instrument** with no `oxiguitar-set.json` is listed as its set's folder and its own name ("… (SFZ)"). It is one instrument, not a set of 128, so it plays only in a row you pick it for. Many SFZ sets have several instruments (programs) for the same recording, with different microphones or articulations. For the recommended sets, the keyswitched programs (`…_keyswitch`, `…_keysw`) are the ones with every articulation.
- OxiGuitar reads SoundFont 2 (`.sf2`), SoundFont 3 (`.sf3`) and Downloadable Sounds (`.dls`) files, and SFZ instruments with WAV, FLAC or Ogg Vorbis samples, in folders up to five levels deep. It never changes anything there.
- If an instrument can't load, or some of its samples are missing, the bottom of the window says so and a General MIDI set plays instead.

## One track's own sound

The settings window's rows hold for every song. One track can sound otherwise: select it (click its name), open the **Track** tab on the right, and choose in its **Sound** section:

- **Automatic** (the default): the settings window's row its instrument names, as the table above says.
- **Like clean guitars**, **Like overdriven guitars**, **Like distorted guitars**, **Like basses** and the other rows: the track plays that row's set whatever its instrument says. Use **Like clean guitars** for a distorted guitar you play through an amp model, which wants a clean guitar ([Amp models (NAM)](amp-models.md)). The choice travels with the song: on another computer the track plays that row's set there.
- **A set**, by name: exactly that set. It is kept by where the set is on this computer; where it isn't (the song opened on another computer), the track plays as Automatic and the bottom of the window says so once.

The choice is kept per track beside the score, in `FILE.oxg`, like every plugin's track settings; the score itself doesn't change.

## Putting a set in by hand, and choosing from it

The sound-set installer does everything below for you. For a set you download yourself, this is how it works.

**Keep every file, and the folders as they are.** An SFZ instrument is a small text file (`.sfz`) that points to its samples by their place under it (`samples/Gtr/A3.flac`), and often to other text files (`#include "controls/common.sfz"`). Move or drop one, or flatten the folders, and the instrument loses its sounds: the bottom of the window says samples are missing and a General MIDI set plays instead. So a pack stays as a whole folder, with the folders inside it as it came.

**Unpack it into the `soundfonts` folder, and that is all.** Extract the zip so that its folder lands inside `soundfonts` (the folder for your system is in the table above). If the zip has no folder of its own, make one first, such as `soundfonts/My Guitar`, and extract into it. OxiGuitar looks up to five levels deep; open the settings window again and the pack is in the lists. There is nothing to register or edit.

**WAV or FLAC.** Both play, as does Ogg Vorbis. FLAC is lossless, so it sounds the same as WAV and takes about half the room: take the FLAC when a pack offers both. The `.sfz` files name their samples with the extension, so use a pack's `.sfz` files with the samples they name: take one download or the other whole, and do not mix a WAV pack's samples into a FLAC pack's folder. (The sets we install are as their authors publish them: the Karoryfer ones are WAV only.)

**Keeping only the patches you like.** Each `.sfz` file is one instrument (a patch), and each shows as one entry in the lists. A pack with a dozen of them (one for each guitar, microphone or set of articulations) shows a dozen entries. To see fewer, delete the `.sfz` files you will not use; the samples and the folders they share can stay, which is why this is safe. To give the room back too, delete the samples only those files used: open one of the deleted `.sfz` files before you delete it, its `sample=` lines say which. The `LICENSE` file stays, and so does any `.sfz` another one `#include`s (a file named `controls`, `common` or `modules` is usually that). With the installer, `--patches recommended` keeps one patch of each set.

**Choosing what the list says.** Without more, an entry reads `FOLDER: FILE NAME (SFZ)`, from the first folder under `soundfonts` and the file's name, so renaming the folder or the file renames the entry (`Black and Green Guitars: 06-green_hammer-on (SFZ)`). For a set you want to name properly, put a file called `oxiguitar-set.json` in its folder (the one under `soundfonts`):

   ```json
   { "name": "Karoryfer", "role": "clean-guitars",
     "credit": "Karoryfer Samples (Black and Green Guitars, green hammer-on)",
     "recommended": "Programs/06-green_hammer-on.sfz",
     "patches": { "Programs/06-green_hammer-on.sfz": "green hammer-on",
                  "Programs/02-black_keyswitch.sfz": "black" } }
   ```

   The entry then reads `name: product (patch)`: `Karoryfer: Black and Green Guitars (green hammer-on)`. The product is what comes before the comma in the last parentheses of `credit`, and the patch is the name `patches` gives that file (paths from the set's folder), or what comes after the comma. `recommended` is the patch that plays where nothing is picked, and `role` says which row the set is for (the keys are in the section above). Close the settings window and open it again after you edit the file.

## Other SFZ sets

Any SFZ instrument plays. One OxiGuitar has no layout for plays its notes as they are: palm mutes and dead notes ended early, hammer-ons carrying on the ringing string. OxiGuitar also knows the layouts of SM Drums, Naked Drums (its own programs) and Karoryfer's Big Rusty, Swirly and Unruly Drums, so their drums play at the right keys; of all the kits, we found the MuldjordKit the most natural.
