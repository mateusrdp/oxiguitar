# Editing

OxiGuitar is becoming an editor. This page grows with it: today you can move around the score, select, enter and change notes, undo, and find the bars that don't add up. Bars, copy and paste, tracks and effects come next.

## The caret

A box over the page shows where the next edit goes: a beat of one voice, and on the tablature the string, framed. Click anywhere in the music to put it there, or move it with the keys, as in Guitar Pro:

| Keys | Moves the caret to |
| --- | --- |
| Left / Right | The previous / next beat, across bar lines |
| Up / Down | The string above / below |
| Home / End | The first / last beat of the bar |
| Ctrl + Home / Ctrl + End | The first / last bar |
| Tab / Shift + Tab | The next / previous staff |
| Ctrl + Up / Ctrl + Down | The previous / next track shown; with a single track on the page, the previous / next track of the song takes its place |
| Ctrl + 1 to Ctrl + 4 | Voice 1 to 4, at the same moment of the bar (or the voice buttons at the top of the palette) |

When a voice is shorter than its bar, the caret has one more place after its last beat: typing there adds a beat. Click past a short voice's last note to go there.

The page scrolls to keep the caret in view. The palette's voice buttons, 1 to 4 at its top, show which voice the caret is in. A click on a track's name in the Tracks tab selects that track and moves the caret to it; the selected track's name is highlighted.

## Selecting

Hold **Shift** with the arrows, Home or End, or Shift + click, to select from where the caret was. **Ctrl + Shift + Left / Right** selects bar by bar. Within one voice of one staff a selection is a run of beats; once it reaches another track it takes whole bars of every track in between. **Ctrl + A** (Edit → Select All) selects everything, and **Escape** drops the selection.

## Entering notes

Type at the caret, with Guitar Pro 8's keys:

| Keys | Does |
| --- | --- |
| 0 to 9 | Puts that fret on the caret's string. Two digits in a row, within a second, make one fret (1 then 2 is fret 12); a fret past the instrument's last is not taken |
| R | Makes the beat a rest |
| − / + | Makes the beat longer / shorter, from a whole note to a sixty-fourth |
| Shift + . / Ctrl + . | Dots the beat once / twice; again takes the dots off |
| L | Ties the note on the caret's string to the note before it on that string (or unties it) |
| Shift + L | Ties every note of the beat before into the caret's beat (or unties them) |
| Backspace (or Delete) | Deletes the note on the caret's string; a beat left without notes becomes a rest |
| Shift + Delete | Deletes the beat, or the selected beats |
| Ctrl + Insert | Inserts a rest as long as the caret's beat, before it |
| Alt + Shift + Up / Down | Moves the note a semitone up / down on its string |
| Alt + Up / Down | Moves the note to the string above / below, at the same pitch |
| Enter | On the standard staff, puts a note at the caret's line or space (see below) |
| Ctrl + Shift + Delete | Clears the beat, or the selected beats: rests of the same length, without effects |
| Alt + 1 to Alt + 4 | Moves the beat to voice 1 to 4, at the same moment of the bar |
| Right, at the very end | Adds a bar after the last, in its time and key |

**On the standard staff**, click a line or a space: the caret marks it, and Up / Down move it a step at a time. **Enter** puts a note there, in the bar's key (on a line or space the key signature sharpens, the note is sharp); on a guitar it goes on the string where its fret is lowest, nearest the caret's string, and the caret moves to that string. A semitone up or down with Alt + Shift + Up / Down gives the notes the key leaves out. Clicking the tablature, or Tab, takes the caret back to the strings.

The **Edit** menu has the same commands and a few more: Insert Beat, Delete Beats, Clear Beats, Clear Bar, Move Beat Left / Right (it swaps places with its neighbour in the bar), Move Beat to Voice (into a voice that has room at that moment: a rest of its length takes its place), and Remove Unused Voices (voices after the first that hold no notes, in every bar of the track).

**File → New** (Ctrl + N) starts a new score: one bar of 4/4 at 120 and a steel-string guitar in standard tuning. Pressing Right at its end adds a bar. Save asks where to put it the first time.

A beat added after a voice's last beat is as long as the beat before it. Typing into a voice that the bar doesn't have yet starts that voice. Note lengths, dots, semitones and deleting beats act on every selected beat when a run of beats is selected.

The palette's **Note** group works too: the whole to sixty-fourth note buttons set the beat's length, and Rest, Dotted Note, Double-Dotted Note and Tie do what their keys do. They light up to show the caret's beat: its length, its dots, whether it is a rest, whether its note is tied.

A tied note always plays the fret of the note it continues: change that note and its ties follow. Notes of a drum track are drum sounds, not frets; entering them comes later.

Every change can be undone. After typing, the bars that don't add up are checked again (see below).

## Undo and redo

**Edit → Undo** (Ctrl + Z) and **Edit → Redo** (Ctrl + Y), or the arrows in the toolbar, undo and redo every change, as many times as you like. The menu names the change: "Undo Infer Key Signatures".

## Bars that don't add up

A bar whose beats are longer or shorter than its time signature is laid in translucent red on the page. Such bars are allowed (many files have a few), and the PDF export shows the music without the red. When a file has any, the line at the bottom of the window says how many and where the first is. **Tools → Next Bar That Doesn't Add Up** (F4, as Guitar Pro's "Check bar duration") moves the caret to the next one in the tracks shown, going round to the start. A voice that holds nothing (Guitar Pro fills unused voices with empty beats) doesn't count.
