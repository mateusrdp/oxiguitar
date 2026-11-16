# Drum entry (v1.0 slice, parity gap 1)

Status: **design, for Mateus's decision** (17 Nov 2026). Nothing below is built yet.

[docs/parity.md](../parity.md) ranks drum entry first among the gaps, and Mateus put it in v1.0 on 17 Nov 2026. Today drum tracks open, play, show on a percussion staff and copy as rhythm, but every note command is refused on them (`oxiguitar-edit/src/entry.rs`, `plan`: Fret, Pitch, Tie, StringUp/Down, SemitoneUp/Down return `None` on a percussion track), and the manual says "entering them comes later".

## What OxiGuitar already has

- **The model**: a drum note is `NotePlace::Pitched(Pitch)` holding its General MIDI percussion key (35 bass drum to 81 open triangle) on a track whose `instrument.percussion` is set, on a staff with no strings. Every format plugin reads and writes drums this way; the player plays them on channel 10. Nothing in the model or the plugin interface needs to change.
- **The kit map, hard-coded in the engraver**: `oxiguitar-layout/src/pitch.rs`, `drum(p) -> (step, cross)`, puts each key on a line or space of the five-line staff (kick in the bottom space, snare in the third space, toms between, cymbals and hi-hats above with cross noteheads) and draws every other key on the middle line.
- **Standard-staff entry**: on a staff without tablature the caret already has a height (`Editing::height`, a staff step moved with Up and Down) and Enter puts a pitch there (`Command::Pitch`). A drum staff can use the same caret.

## How the two editors do it

- **Guitar Pro 8** (user guide, "Drums and Percussion"): notes are entered on the drum staff as on any staff; several kit elements share a line, and the numeric keypad's 1, 2, 3 pick among them (its example: a closed, half-open or open hi-hat). A Drum View (View > Show Drum View) shows the kit to click elements into the beat. Drum tablature shows each element's MIDI number, and the mapping can be set there.
- **TuxGuitar 2.1**: drums are typed as numbers on its tablature, and its **matrix editor** shows a bar as a grid: one row per kit element, one column per step of a chosen duration, a click toggles a hit.

## The options

**A. The drum staff with a kit map, as in Guitar Pro 8 (proposed for v1.0).** The caret moves over the staff's lines and spaces as it does on a standard staff. Each line or space has the kit elements the map puts there, in an order:

| Staff step (0 = bottom line) | Elements, in order: Enter gives the first (GM key) |
| --- | --- |
| 11, the space over the first ledger line | Splash (55), China (52) |
| 10, the first ledger line above | Crash 1 (49), Crash 2 (57) |
| 9, the space above the staff | Closed hi-hat (42), Open hi-hat (46) |
| 8, the top line | Ride (51), Ride bell (53), Ride 2 (59), High tom (50) |
| 7, the top space | High-mid tom (48), Tambourine (54), Cowbell (56) |
| 6, the fourth line | Low-mid tom (47), Low tom (45) |
| 5, the third space | Snare (38), Side stick (37), Electric snare (40), Hand clap (39) |
| 4, the middle line | every other key |
| 3, the second space | High floor tom (43) |
| 2, the second line | Low floor tom (41) |
| 1, the bottom space | Bass drum 1 (36), Bass drum 2 (35) |
| -1, the space below the staff | Pedal hi-hat (44) |

- **Enter** puts the line's first element on the caret's beat; **1** to **9** put its first to ninth (on a drum staff digits are no frets, so they are free, as in Guitar Pro 8). The beat keeps one element per line: typing another on the same line replaces it (Snare, then 2, gives a side stick), and other lines add to the beat (kick and hi-hat together).
- **Backspace** and **Delete** take away the element on the caret's line; rests, durations, dots, tuplets, ties of a beat, voices, copy and paste, and the note effects that mean something on drums (accent, ghost, dead, grace notes for flams, tremolo picking for rolls, let ring, dynamics) work as on any track.
- **The Note tab** shows the caret's element by name ("Snare (38)") with a drop-down of every General MIDI key, for elements that share the middle line or to change one without the keys.
- **The map moves out of the engraver** into one table both the engraver and the editor read (`oxiguitar-model` or `oxiguitar-layout`, with the names), so what is drawn on a line is what is entered there. It stays the General MIDI map for every drum track.
- Size: one editor slice in about three pull requests (the shared map and the entry commands; the window's keys, the caret on the drum staff and the Note tab's drop-down; manual, README, a screenshot).

**B. A matrix editor, as in TuxGuitar.** A grid for the caret's bar, rows the kit's elements and columns sixteenth notes (or the bar's smallest note), each cell a hit; clicks write the bar's beats. Good for drawing grooves with the mouse; it needs a canvas of its own and rules for rewriting a bar's beats from the grid (durations, voices, tuplets). Since interface 1.8 it could be a plugin (a tab or a dialog with a canvas, reading the selection and handing back the bar), so it can come after v1.0 without touching the core.

**C. Both**: A in v1.0, B later as a plugin.

## Questions for Mateus

1. Which way for v1.0? Proposed: **A**, the drum staff with the kit map (keyboard entry as in Guitar Pro 8); B after v1.0, as a plugin, if wanted.
2. The kit map: is the fixed General MIDI map above enough for v1.0? Proposed: **yes**, one map for every drum track; a map the user edits per track (Guitar Pro 8 sets it in the drum tablature) after v1.0.
3. Drum tablature (each element's MIDI number on lines, as Guitar Pro 8 and TuxGuitar show): proposed **not in v1.0**; the Note tab names the element instead.
