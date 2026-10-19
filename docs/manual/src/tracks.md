# Tracks and notation

The **Tracks** tab of the bottom panel shows every track of the song: its name, its mixer, its notation buttons and an overview of the track across the song. **View → Panels → Bottom** (F8) hides or shows the panel, a click on the open tab's name folds it away, and dragging its top edge makes it taller or shorter.

- **Click a name** to select that track: the caret moves to it ([Editing](editing.md)). A track is shown while its clef or TAB button is on.
- **Click a hidden track's notation button** to show it with the others, lined up bar by bar in one score. There's no separate "show all" switch: showing each track you want is enough.
- With several tracks, each system starts with the tracks' names.

## Notation buttons

Each track has two buttons at the end of its row: **TAB** for tablature and a **treble clef** for standard notation. A lit button is on. Tracks without strings, such as drums, have only the clef.

| TAB | Clef | The track shows |
| --- | --- | --- |
| on | on | standard notation above tablature |
| off | on | standard notation |
| on | off | tablature, with the rhythm drawn around it |
| off | off | nothing: the track is hidden |

Clicking either button of a hidden track shows it again, with that notation. The last track shown cannot be hidden.

When a file opens, each track shows the notation it was saved with. Tablature is on only for fretted instruments (guitars, basses, banjo and the like), and keyboard tracks get a grand staff, split between the hands.

## Mixer

After its name, each track has its mixer: two buttons, **mute** (a loudspeaker crossed out) and **solo** (headphones), a **volume** slider and a **pan** knob (left to right). They show what the file sets for each track, and they stay greyed out until an output plugin is installed and the player arrives ([Settings and plugins](plugins.md)). Nothing you change there is written to the file.

## Overview

The rest of each row shows the track across the whole song, from start to end, like a waveform: it is thick where the track plays a lot and a thin line where it is silent, so you can see at a glance where each instrument comes in. The mark at its start is where the playback position will be shown, once the player arrives.
