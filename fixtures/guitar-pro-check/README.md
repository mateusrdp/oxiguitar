# Files for checking the .gp and .gp5 writers

What OxiGuitar's .gp and .gp5 writers (the Guitar Pro 7–8 and Guitar Pro 5 plugins) make of every reference song (`../reference/`, listed with what each tests in `../reference/README.md`) and of the test song in every format (`../gp/`, as `test-song-from-EXT.gp`), each as a .gp and as a .gp5. `conversions.txt` lists what each conversion lost: nothing, so far. Each file also says what it tests in its score information (Instructions).

They are kept so anyone can open them in Guitar Pro, or any program that reads .gp or .gp5, in whatever version and on whatever system they have, and tell us what looks wrong. See "Checking what OxiGuitar writes" in `CONTRIBUTING.md`. `scripts/guitar-pro-check.sh` writes them again; the test `guitar_pro_check` (in `oxiguitar-testgen`) fails while they are out of date. Made by us; GPL-3.0-or-later.

## Checks so far

| Date | Program | System | Found |
| --- | --- | --- | --- |
| 20 Oct 2026 | Guitar Pro 7.6.0 (Mateus) | Wine 11, Debian sid | Every file opens. Hammer-ons and pull-offs had two arcs (the writer didn't mark the note they go to; fixed). Pianos, strings and voices showed a tuning of six Cs and frets of −2147483648 on the fretboard (fixed: Guitar Pro's own default tunings, and a string and fret on every note, from his Guitar Pro file). The staff was drawn an octave below the tab (fixed: Guitar Pro numbers octaves from MIDI 0, one above scientific pitch). `lyrics.gp` no longer crashes Guitar Pro. |

## Known differences that aren't ours

- **MuseScore 4 crashes on a slide from a voice's last note to no note.** Players slide off a note to nothing, so OxiGuitar keeps such a slide (Mateus, 20 Oct 2026); MuseScore's Guitar Pro reader has the bug. The interop check (`scripts/interop/musescore.py`) leaves such files out and says why.
