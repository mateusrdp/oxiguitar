# Tracks and notation

The **Tracks** tab of the bottom panel shows every track of the song: its name, its mixer, its notation buttons and an overview of the track across the song. **View → Panels → Bottom** (F8) hides or shows the panel, a click on the open tab's name folds it away, and dragging its top edge makes it taller or shorter.

- **Click a name** to select that track: the caret moves to it ([Editing](editing.md)). A track is shown while its clef or TAB button is on.
- **Click a hidden track's notation button** to show it with the others, lined up bar by bar in one score. There's no separate "show all" switch: showing each track you want is enough.
- With several tracks, each system starts with the tracks' names.
- **Right-click a name** for the track's menu: Properties…, Transpose…, Add Track, Duplicate, Move Up, Move Down and Delete…. Delete asks first; Undo brings a deleted track back.

## Adding, moving and deleting tracks

**Add Track** (also Edit → Add Track) adds a steel-string guitar in standard tuning after the track, with empty bars, and moves the caret to it. **Duplicate** copies the track with its notes, named "… (copy)". **Move Up** and **Move Down** change its place in the song. The tracks you had on the page stay there wherever they move, and when the caret's track is deleted the caret goes to the nearest track still shown.

## Track properties

**Properties…** (also Edit → Track Properties…, or right-click on the page → Track Properties…) opens the track dialog:

- **Name**, and a short name for the systems after the first.
- **Instrument**: the General MIDI sound, or Drum Kit.
- **Strings**: none for an instrument without tablature, or 3 to 10. **Capo** and the number of **frets**.
- **Tuning**: a key and a pattern. The patterns are Standard, Drop (the lowest string a tone down, as Drop D), Open with the root on the lowest string (Open D, Open E), Open with the fifth on the lowest string (Open G, Open A), and Other, where you type the notes, lowest first (`D2 A2 D3 G3 A3 D4`; a note without an octave goes above the one before it). Tunings are named the same way at the top of the page: "Standard tuning", "Tuning: Drop D", or the notes.
- **If the strings change**: keep the notes' pitches, finding new frets near the old ones, or keep the frets and let the notes change.

The dialog is one change you can undo. If the new strings or capo can't play some notes, a question says how many before they are left out.

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

After its name, each track has its mixer: two buttons, **mute** (a loudspeaker crossed out) and **solo** (headphones), a **volume** slider and a **pan** knob (left to right). They start as the file sets them and change the sound as you move them, while it plays ([Playing a score](playback.md#the-mixer)); they are greyed out while nothing can play the score. Nothing you change there is written to the file.

## Overview

The rest of each row shows the track across the whole song, from start to end, like a waveform: it is thick where the track plays a lot and a thin line where it is silent, so you can see at a glance where each instrument comes in. The mark across it is the playback position: it follows the sound while the score plays, stays where you paused, and otherwise shows where Play will start (the caret). Click an overview to move the playback there.
