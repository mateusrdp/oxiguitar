# Editing

OxiGuitar is becoming an editor. This page grows with it: today you can move around the score, select, enter and change notes, change bars, copy and paste, add and change tracks ([Tracks and notation](tracks.md)), transpose, add effects, undo, and find the bars that don't add up. Chord diagrams, lyrics, text and the score's information come next.

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

The page scrolls to keep the caret in view. Only the caret's voice is drawn in full ink; the other voices are greyed, so you see which notes you are editing (a PDF prints them all in black). The palette's voice buttons, 1 to 4 at its top, show which voice the caret is in. A click on a track's name in the Tracks tab selects that track and moves the caret to it; the selected track's name is highlighted.

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

### Tuplets

**/** (or the palette's **Triplet**) makes the beat a triplet, three in the time of two, or takes it out of one; with a run of beats selected, all of them. **Custom N-tuplet** (in the palette, or **Edit → Custom N-tuplet…**) asks for any grouping, *n* notes in the time of *m*, with the usual *m* filled in as you change *n*: 5 to 7 in the time of 4, 9 to 15 in the time of 8, and 2, 4 and 8 in the time of 3, 3 and 6. A beat added after a tuplet's last beat is in the same tuplet, so typing on fills it.

## Bars

The bar commands work on the caret's bar or, with a selection, on its bars.

| Keys | What they do |
| --- | --- |
| Insert | Inserts an empty bar before the caret's, in its time and key |
| Ctrl + Delete | Deletes the caret's bar, or the selected bars, from every track (never the last bar left). A tempo change in them moves to the bar after them, so the music after them keeps its tempo |
| [ and ] | A repeat opens at the bar, or closes at it (played twice); press again to take it away |
| \| or Alt + Shift + L | Free time: the bar has no time signature to add up to |
| Ctrl + K, Ctrl + /, D, Shift + Insert | Opens the **Bar** dialog |
| Double-click on a time signature | Opens the **Bar** dialog for that bar |

The **Bar** dialog (also **Edit → Bar…** and the palette's Clef, Key Signature, Time Signature, Rhythmic Interpretation, Alternate Endings and Directions) shows what the bar carries and changes any of it at once, as one change to undo:

- **Time signature**, **key** and **feel** change from this bar until the next bar that had another, as Guitar Pro changes them. A tempo change inside a bar stays at the same point of the bar when its time signature changes.
- **Clef** changes the caret's track from this bar until its next clef. A guitar's staff sounds an octave below the written notes, and it keeps doing so in any clef (its bass clef has the small 8 too). Drum tracks keep the drums' clef.
- **Tempo** is the tempo from the start of the bar: in the first bar, the song's tempo.
- **Repeat**: whether a repeat opens here, and whether one closes here and how many times it is played.
- **Alternate endings** 1 to 8, **Section** (a letter and a name; empty for none), **Double bar line**, **Free time**, and the **Directions**: segno, coda, fine and the jumps (D.C., D.S., "to coda" and the rest).

The palette's **Repeat Start**, **Repeat Close**, **Free Time** and **Double Bar Line** switch those at once, and light up when the caret's bar has them; **Triplet** lights up on a triplet beat.

## Copy, cut and paste

OxiGuitar keeps its own clipboard: copying and pasting work inside OxiGuitar, not with other programs.

- **Copy** (Ctrl + C) copies the selected beats or, with nothing selected, the beat at the caret. A selection across tracks copies whole bars of each selected track, with what the bars carry (time and key signatures, repeats, sections, tempo changes).
- **Cut** (Ctrl + X) copies, then takes the beats out of their voice, or the bars out of every track.
- **Paste** (Ctrl + V) puts what was copied in at the caret, or at the start of the selection. Beats go into the caret's voice before its beat; a bar that then holds more than its time signature is marked red. Bars go in before the caret's bar, in every track: the tracks that were copied, from the caret's track down, get their bars, and the others get empty bars.
- **Special Paste** (Ctrl + Shift + V) pastes several times, or replaces what is there instead of inserting. Beats then take the place of as much music as they last, and what they don't cover of the last beat they replace stays as rests. Bars then take the place of the bars from the caret's on, and bars are added at the end when needed.

Beats and bars paste into any track. Between a drum track and a pitched one only the rhythm carries over, as rests. Notes on strings the track hasn't got are left out. Chord names go with their diagrams.

**Edit → Go to Section** lists the score's sections and moves the caret to the one you choose.

## Transposing

**Transpose…** moves notes up or down, by semitones or by **steps of the key**, which keeps the music in its key (C, D, E up a step in C major become D, E, F). There are three ways in:

- **Right-click on the page → Transpose…**, or **Edit → Transpose Selection…**: the selection, or the caret's beat when nothing is selected. A right-click outside the selection first moves the caret there.
- **Right-click on a track's name → Transpose…**: the whole track.
- **Tools → Transpose…**: every track, with the key signatures.

Up is a positive number, down a negative one. **Keep the fingering** keeps each note on its string and moves its fret; otherwise each note goes to the fret nearest the one it had, so a run stays where your hand is. Notes that no string can play any more are left out, after a question saying how many. Drum tracks are never transposed. Undo brings everything back.

## Effects

Every effect of Guitar Pro's tablature has a key and a tool in the palette's **Effects** group. Pressed once, an effect goes on; pressed again, it comes off. A note effect goes on the note on the caret's string, and does nothing if that string has no note in the beat; on a staff without strings it goes on every note of the beat. A beat effect goes on the caret's beat. With a selection, both go on every note (or beat) selected, unless all of them have it already: then it comes off all of them. Rests take no effects. The tools light up when the caret's note or beat has their effect.

Guitar Pro 8's keys, and a few of our own for effects it gives none (the effect's first letter, with at most one modifier):

| Keys | Effect | On |
| --- | --- | --- |
| X | Dead note | Note |
| O | Ghost note | Note |
| ; / : | Accent / heavy accent | Note |
| ! | Staccato | Note |
| H | Hammer-on or pull-off to the next note | Note |
| ( | Left-hand tapping | Note |
| S | Legato slide | Note |
| Alt + S | Shift slide | Note |
| Shift + S | Slide out downwards | Note |
| V | Vibrato | Note |
| P / Shift + P | Palm mute on the note / on every note of the beat | Note |
| I | Let ring | Note |
| Y | Natural harmonic | Note |
| Ctrl + Alt + Y | Artificial harmonic (an octave up) | Note |
| Alt + P | Pinch harmonic | Note |
| Alt + T | Tapped harmonic (12 frets up) | Note |
| N | Trill with the next fret up, in sixteenths | Note |
| B | The bend editor | Note |
| " | Tremolo picking, in thirty-seconds | Beat |
| T or ) | Tapping | Beat |
| $ | Slap | Beat |
| < | Fade in | Beat |
| G / Ctrl + Alt + G | Grace note before the beat / on the beat | Beat |
| Ctrl + D / Ctrl + U | Brush down / up, over a sixteenth | Beat |
| Shift + D / Shift + U | Pick stroke down / up | Beat |
| Ctrl + Alt + W | Wide vibrato with the whammy bar | Beat |
| Ctrl + Alt + V | The whammy bar's editor | Beat |

Slides in from below and from above and out upwards, popping and the semi-harmonic have tools in the palette but no keys. A note slides in one way and out one way: another slide on the same side takes the place of the first. A note has one harmonic: another kind takes its place. A grace note is a thirty-second, as Guitar Pro writes them, and takes no time in the bar.

The palette's **dynamics**, ppp to fff, set the dynamic of the caret's beat, or of every selected beat (rests too), and its notes play at it. They have no keys. View → Dynamics shows them on the page.

### Bends and the whammy bar

**B** (or the palette's **Bend**) opens the **bend editor** for the note on the caret's string; **Ctrl + Alt + V** (or **Whammy Bar**) opens it for the beat's whammy bar. Pick a type (Bend, Bend and release, Release, Prebend, Prebend and bend, Prebend and release; for the whammy bar Dive, Dip, Return, Predive, Predive and dive) and a pitch from ¼ tone to 3 tones, or drag the curve's points on the grid: time across the note in twelfths, pitch up the side in quarter tones. A press on an empty grid starts a bend at that pitch. The curve has an origin, a middle held at one pitch between two places, and a destination, which is the shape a .gp file holds. **Enter** applies it, **Escape** cancels, and "No bend" takes the bend away. With a selection, the curve goes on every selected note (or beat).

### The Inspector's note and beat

Under the caret's bar, the **Inspector** shows the note on the caret's string and its beat, and changes them. The tickboxes switch effects as their keys do (on a selection, on every selected note or beat). The drop-downs and numbers set what the keys only start at a default: the accent, how the note slides in and out, the harmonic and how far up an artificial or tapped harmonic sounds, the trill's speed (sixteenths, thirty-seconds or sixty-fourths) and the fret it trills with, the fingers of the left and right hands, the dynamic, the speed of tremolo picking, grace notes, the brush and how long it takes, the pick stroke, and tapping, slapping or popping. **Edit…** beside the bend and the whammy bar opens their editor. On a rest only its dynamic shows. Each change is one step to undo.

A .gp file keeps trills in sixteenths only: saving a faster trill as .gp names it among what the file can't hold.

## Undo and redo

**Edit → Undo** (Ctrl + Z) and **Edit → Redo** (Ctrl + Y), or the arrows in the toolbar, undo and redo every change, as many times as you like. The menu names the change: "Undo Infer Key Signatures".

## Bars that don't add up

The **Inspector**, on the right, shows the caret's bar: its time signature, and for each voice how many beats it holds against it, counted in the time signature's own unit (quarter notes in 4/4, eighth notes in 6/8): "3½ of 4", then *Full*, *½ beat short* or *1 beat too long*, and how many beats (notes, chords and rests) were entered. The caret's voice is highlighted, and the last line says on which beat the caret's beat starts. It follows every move and every change, so it is the quickest way to see what a bar still needs while composing.

A bar whose beats are longer or shorter than its time signature is laid in translucent red on the page. Such bars are allowed (many files have a few), and the PDF export shows the music without the red. When a file has any, the line at the bottom of the window says how many and where the first is. **Tools → Next Bar That Doesn't Add Up** (F4, as Guitar Pro's "Check bar duration") moves the caret to the next one in the tracks shown, going round to the start. A voice that holds nothing (Guitar Pro fills unused voices with empty beats) doesn't count.
