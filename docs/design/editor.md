# The editor (M3)

M3 makes OxiGuitar an editor: everything TuxGuitar edits, saved as .gp (the default) or .gp5. Its release, v0.5, is the first that Linux users can write tabs with, where until now they had TuxGuitar or Wine (Mateus, 19 Oct 2026). The plan and its slices are in [ROADMAP.md](../../ROADMAP.md) ("M3's slices, in order"); this page holds the parity checklist that decides when M3 is done, and the editing rules as they are decided.

## The TuxGuitar parity checklist

What TuxGuitar 1.6 lets a user edit, read from its menus and dialogs (helge17/tuxguitar, `desktop/TuxGuitar/src/app/tuxguitar/app/view/menu/impl/`, checked 19 Oct 2026), next to the brief's requirements (EDIT-1 to EDIT-10, TAB-1). An item is ticked when it can be entered from the palette or a menu and by its shortcut, changed, deleted, undone and redone, is drawn, and survives saving as .gp and .gp5 (or is listed as a .gp5 loss) and reading back. Mateus approved the list on 19 Oct 2026 ("looks good for now"); it can grow.

Where OxiGuitar goes past TuxGuitar (four voices to its two, any tuplet to its list), the brief's requirement is the item.

### Notes and rhythm (slice 3; tuplets in slice 4)

- [ ] Notes entered on the tablature: a fret number on a string, two-digit frets (EDIT-2)
- [ ] Notes entered on the standard staff: a pitch, its string chosen for the lowest playable fret near the caret's string (EDIT-2)
- [ ] Chords: several notes in one beat
- [ ] Rests; inserting a rest before the caret; deleting a note or a rest
- [ ] Note values from whole to 64th; dotted and double-dotted
- [ ] Tied notes
- [ ] Tuplets: the Triplet tool and its shortcut; Custom N-tuplet for any n in the time of m (EDIT-10; TuxGuitar offers 3:2, 5:4, 6:4, 7:4, 9:8 to 13:8)
- [ ] Four voices per staff (EDIT-3; TuxGuitar has two), moving a beat to another voice
- [ ] Semitone up and down; the other spelling of a note (enharmonic); a note moved to the next string up or down at the same pitch
- [ ] Moving beats left and right
- [ ] Clearing a beat and a bar; removing a voice nobody uses
- [ ] Bars that don't add up: marked, listed one by one (TuxGuitar's "measure errors" dialog does this, with a fix button; OxiGuitar marks them in red and walks through them, slice 1)

Built in slice 3 so far (each waits for .gp5 in slice 7 to be ticked): frets typed on the tablature with two-digit frets, chords, rests, inserting and deleting beats and notes, note values whole to 64th from the keys and the palette, dots, ties (note and beat), voices started by typing in them, semitones, the same note on the next string; then pitches entered on the standard staff (Enter at the caret's line or space, the string with the lowest fret nearest the caret's), File → New, a bar added by Right at the end, moving beats left and right and to another voice, clearing beats and bars, removing unused voices.

### Bars and structure (slice 4)

Built in slice 4 so far (each waits for .gp5 in slice 7 to be ticked): inserting and deleting bars; time and key signatures, clefs, tempo, triplet feel, repeats with a count, alternate endings 1 to 8, directions, sections, double bar lines and free time, from the keys, the palette and the Bar dialog; the Triplet tool and Custom N-tuplet; copy, cut and paste with Special Paste, and Edit → Go to Section.

- [ ] Inserting and deleting bars
- [ ] Time signature, key signature, clef and tempo changes from any bar (EDIT-6)
- [ ] Triplet feel
- [ ] Repeat start and end with a count; alternate endings (EDIT-7)
- [ ] Segno, coda, D.S., D.C., fine and the "al" forms (EDIT-7; not in TuxGuitar)
- [ ] Sections (markers), with a list to jump between them
- [ ] Double bar lines; free time
- [ ] Copy, cut and paste of beats and bars; special paste: how many times, insert or replace, across voices and tracks (EDIT-6)
- [ ] Select all; extending the selection by beat, bar and to the start and end

### Tracks and the score (slices 1, 5 and 6)

- [ ] Adding, duplicating, removing (after a question) and reordering tracks
- [ ] Track properties: name, instrument, capo; strings (3 to 10) and tuning, as a key and a pattern (Standard, Drop, Open with the root low, Open with the fifth low, Other) (EDIT-4)
- [ ] Changing strings or tuning on a track with notes, keeping the pitches or the frets
- [ ] Transposition, chromatic and diatonic, keeping the fingering or finding new frets (EDIT-8), three ways: a track (right-click on its row), a selection (right-click on it in the page), every track (Tools → Transpose…)
- [ ] Score information: title, subtitle, artist, album, words, music, copyright, transcriber, instructions, notices
- [ ] Lyrics of a track
- [ ] Track selection by a click on its row (slice 1); mute and solo follow with the player (M4)

### Effects (slice 6; TAB-1)

- [ ] Dead note, ghost note, accent, heavy accent, staccato
- [ ] Hammer-on and pull-off
- [ ] Slides: shift, legato, in from below and above, out down and up
- [ ] Bends, with a curve editor and presets; whammy bar, with its curve
- [ ] Vibrato (note), wide vibrato (beat)
- [ ] Harmonics: natural, artificial, tapped, pinch, semi
- [ ] Palm mute, let ring
- [ ] Trill (the other fret, the speed), tremolo picking (the speed)
- [ ] Grace notes, before and on the beat
- [ ] Tapping, slapping, popping
- [ ] Fade in
- [ ] Brush strokes up and down (with their speed); pick strokes up and down
- [ ] Dynamics, ppp to fff
- [ ] Chord names with their diagrams (a chord editor)
- [ ] Free text on a beat

### Files (slices 2 and 7)

- [ ] Save and Save As: .gp (the default) and .gp5; a file is saved in its own format unless that format would lose something, and then as .gp (Mateus, 19 Oct 2026). .gp done in slice 2; .gp5 in slice 7
- [x] Losses named before writing (FMT-3), and what the opened file had that OxiGuitar couldn't keep (FMT-2) (slice 2)
- [x] A question before closing or opening over unsaved changes (slice 2)
- [ ] File → New
- Not planned: TuxGuitar also writes Guitar Pro 3 and 4 and its own .tg; OxiGuitar writes .gp and .gp5 only (the brief: Guitar Pro 3–4 are read only).

## Editing rules

Decided by Mateus on 19 Oct 2026 unless said otherwise.

- **Bars that don't add up** are allowed, shown and played (M4 adjusts what playback needs), only marked: on screen in red, never in a PDF. Every bar is checked when a file opens; after that only the bars an edit touches are checked again, nothing more ("I want this app to be FAST"). A voice holding only empty placeholder beats (Guitar Pro 3–5 fill an unused voice with one) doesn't count. The footer says how many bars and where the first is; Tools → Next Bar That Doesn't Add Up walks the caret through them.
- **Stems** go by voice only: odd voices up, even voices down. The palette has no stem-direction tools.
- **Transposition** has three ways in (Mateus, 19 Oct 2026), so no task needs a selection made first: right-click on a track's row → Transpose… (that track), right-click on a selection → Transpose… (the selection, on its track), Tools → Transpose… (every track).
- **Tracks:** a click on a row selects the track; a track is shown when its TAB or standard-notation button is on. Right-click on a row: Properties (also Edit → Track Properties…), Transpose…, Delete (which asks first).
- **Tunings** are named compactly wherever they show (E Standard, Drop D, Open G, or the notes, low to high: `D A D G A D`). In the track dialog the tuning is a key and a pattern: Standard, Drop, Open with the root on the lowest string (Open D, Open E, Open C), Open with the fifth on the lowest string (Open G, Open A), or Other. Other keeps the key and takes any note for each string, checked only as a note and against the number of strings, never against the key.
- **The caret's voice** shows on the palette's voice buttons, highlighted (Mateus, 19 Oct 2026).
- **Ctrl + Up / Ctrl + Down** move among the shown tracks; with a single track shown, the previous or next track of the song takes its place, as in Guitar Pro's single-track view (Mateus, 19 Oct 2026).
- **The clipboard** is OxiGuitar's own: copying and pasting work inside OxiGuitar, not with other programs.
- **Undo** is unlimited (EDIT-5) and keeps copies of only what each command changed.
- **Note entry** (slice 3, `oxiguitar_edit::Command`) uses Guitar Pro 8's keys. A voice shorter than its bar gives the caret one more place after its last beat, where typing adds a beat as long as the one before it. A tied note always has the fret of the note it continues (the last note played on its string in the voice, as Guitar Pro reads a tie), so a change to a note carries through its ties. A typed note takes its beat's dynamic, or the last one before it (Guitar Pro keeps one per beat). A beat made a rest drops tapping, which belongs to notes. Voices keep their places: an unused voice before a used one is an empty placeholder, written as no voice. Every edit is held to two gates over every fixture (and the corpus where it is): random runs undone to the original and redone to the end, and the edited score saved as .gp reading back equal (`oxiguitar-edit --test entry`).
- **The standard staff** (slice 3): a click on it gives the caret a line or space, which Up / Down move and Enter fills, in the bar's key. Guitar Pro 8 offers its fretboard and keyboard views for this; OxiGuitar's caret is the MuseScore-like way in until those views come as plugins.
- **File → New** starts as Guitar Pro does: one bar, 4/4, 120, a steel-string guitar in standard tuning; Right at the end of the score adds a bar, as TuxGuitar does.
- **Bars** (slice 4, `oxiguitar_edit`'s `bars.rs`): a bar command works on the caret's bar or the selection's bars. Time and key signatures, feel and clefs change from the bar until the next bar that had another (Guitar Pro's "until the next change"); a repeat opens at the first bar and closes at the last; tempo is the song's tempo in the first bar, a change at the bar's start after it. Tempo changes inside a bar keep their fraction of the bar when its time signature changes (Guitar Pro keeps them so), on its 960-tick grid. Deleting bars never deletes the last one, moves the last tempo change in them to the bar after them, and re-ties tied notes after them to the note they now continue. A clef keeps its staff's octave (Guitar Pro keeps the octave per track); drum tracks keep the drums' clef. The Bar dialog is one change, recorded as the master bars and the caret's staff from the bar on. Keys as Guitar Pro 8's: Insert, Ctrl + Delete, `[`, `]`, `|` / Alt + Shift + L, `/`; Ctrl + T, Ctrl + K, Ctrl + /, D and Shift + Insert open the Bar dialog, which holds what Guitar Pro spreads over five dialogs.
- **The clipboard** (slice 4, `oxiguitar_edit`'s `clip.rs`), as Guitar Pro 8's: a run of beats copies as beats (the caret's beat with no selection) and pastes into any voice of any track; a selection across tracks copies whole bars with their master bars and pastes as bars, from the caret's track down, other tracks getting empty bars. Paste inserts at the caret, or at the selection's start; Special Paste sets how many times, and whether to replace instead. Replacing beats removes as much music as they last, and the part of the last removed beat they don't cover stays as rests; replacing bars overwrites from the caret's bar, adding bars at the end. Between drum and pitched tracks only rests carry over (Guitar Pro: "only the rhythm represented by rests is retained"); notes on strings the track lacks are dropped; chord diagrams travel with their beats (found or added in the target track). Cut is a delete named "Cut". Pasted beats can overflow their bar, which is then marked; Guitar Pro pushes the overflow into new bars instead. That may follow if Mateus wants it.
- **Drum tracks** take no frets: their notes are drum sounds, entered with the drum tools later.
