# Editing

OxiGuitar is becoming an editor. This page grows with it: today you can move around the score, select, undo a tool, and find the bars that don't add up. Entering notes comes next.

## The caret

A box over the page shows where the next edit goes: a beat of one voice, and on the tablature the string, framed. Click anywhere in the music to put it there, or move it with the keys, as in Guitar Pro:

| Keys | Moves the caret to |
| --- | --- |
| Left / Right | The previous / next beat, across bar lines |
| Up / Down | The string above / below |
| Home / End | The first / last beat of the bar |
| Ctrl + Home / Ctrl + End | The first / last bar |
| Tab / Shift + Tab | The next / previous staff |
| Ctrl + Up / Ctrl + Down | The previous / next track shown |
| Ctrl + 1 to Ctrl + 4 | Voice 1 to 4, at the same moment of the bar |

The page scrolls to keep the caret in view. A click on a track's name in the Tracks tab selects that track and moves the caret to it; the selected track's name is highlighted.

## Selecting

Hold **Shift** with the arrows, Home or End, or Shift + click, to select from where the caret was. **Ctrl + Shift + Left / Right** selects bar by bar. Within one voice of one staff a selection is a run of beats; once it reaches another track it takes whole bars of every track in between. **Ctrl + A** (Edit → Select All) selects everything, and **Escape** drops the selection.

## Undo and redo

**Edit → Undo** (Ctrl + Z) and **Edit → Redo** (Ctrl + Y), or the arrows in the toolbar, undo and redo every change, as many times as you like. The menu names the change: "Undo Infer Key Signatures".

## Bars that don't add up

A bar whose beats are longer or shorter than its time signature is laid in translucent red on the page. Such bars are allowed (many files have a few), and the PDF export shows the music without the red. When a file has any, the line at the bottom of the window says how many and where the first is. **Tools → Next Bar That Doesn't Add Up** (F4, as Guitar Pro's "Check bar duration") moves the caret to the next one in the tracks shown, going round to the start. A voice that holds nothing (Guitar Pro fills unused voices with empty beats) doesn't count.
