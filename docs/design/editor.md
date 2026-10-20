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

Built in slice 5 (each waits for .gp5 in slice 7 to be ticked): adding, duplicating, removing and moving tracks; the track dialog; restringing keeping the pitches or the frets; transposition, all three ways.

- [ ] Adding, duplicating, removing (after a question) and reordering tracks
- [ ] Track properties: name, instrument, capo; strings (3 to 10) and tuning, as a key and a pattern (Standard, Drop, Open with the root low, Open with the fifth low, Other) (EDIT-4)
- [ ] Changing strings or tuning on a track with notes, keeping the pitches or the frets
- [ ] Transposition, chromatic and diatonic, keeping the fingering or finding new frets (EDIT-8), three ways: a track (right-click on its row), a selection (right-click on it in the page), every track (Tools → Transpose…)
- [ ] Score information: title, subtitle, artist, album, words, music, copyright, transcriber, instructions, notices
- [ ] Lyrics of a track
- [ ] Track selection by a click on its row (slice 1); mute and solo follow with the player (M4)

### Effects (slice 6; TAB-1)

Built in slice 6 so far (each waits for .gp5 in slice 7 to be ticked): every effect below, from its key and the palette, on the caret's note or beat or on a selection; the bend and whammy-bar editor; dynamics from the palette; chord diagrams with Guitar Pro 8's builder, and text. The values (trill fret and speed, tremolo speed, brush speed, harmonic kind and fret, fingering) start at a default and the Inspector changes them. Lyrics of a track (above, under Tracks and the score) too, typed on the page.

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
- **Bars** (slice 4, `oxiguitar_edit`'s `bars.rs`): a bar command works on the caret's bar or the selection's bars. Time and key signatures, feel and clefs change from the bar until the next bar that had another (Guitar Pro's "until the next change"); a repeat opens at the first bar and closes at the last; tempo is the song's tempo in the first bar, a change at the bar's start after it. Tempo changes inside a bar keep their fraction of the bar when its time signature changes (Guitar Pro keeps them so), on its 960-tick grid. Deleting bars never deletes the last one, moves the last tempo change in them to the bar after them, and re-ties tied notes after them to the note they now continue. A clef keeps its staff's octave (Guitar Pro keeps the octave per track); drum tracks keep the drums' clef. The Bar dialog is one change, recorded as the master bars and the caret's staff from the bar on. Keys as Guitar Pro 8's: Insert, Ctrl + Delete, `[`, `]`, `|` / Alt + Shift + L, `/`; Ctrl + K, Ctrl + /, D and Shift + Insert open the Bar dialog, which holds what Guitar Pro spreads over five dialogs. Guitar Pro's Ctrl + T (time signature) types lyrics in OxiGuitar, and the time signature has no key of its own (Mateus, 20 and 21 Oct 2026).
- **The clipboard** (slice 4, `oxiguitar_edit`'s `clip.rs`), as Guitar Pro 8's: a run of beats copies as beats (the caret's beat with no selection) and pastes into any voice of any track; a selection across tracks copies whole bars with their master bars and pastes as bars, from the caret's track down, other tracks getting empty bars. Paste inserts at the caret, or at the selection's start; Special Paste sets how many times, and whether to replace instead. Replacing beats removes as much music as they last, and the part of the last removed beat they don't cover stays as rests; replacing bars overwrites from the caret's bar, adding bars at the end. Between drum and pitched tracks only rests carry over (Guitar Pro: "only the rhythm represented by rests is retained"); notes on strings the track lacks are dropped, and so are notes placed by pitch pasted into a track with strings or by string into one without (slice 6: a .gp file would read them back the other way); chord diagrams travel with their beats (found or added in the target track). Cut is a delete named "Cut". Pasted beats can overflow their bar, which is then marked; Guitar Pro pushes the overflow into new bars instead. That may follow if Mateus wants it.
- **Tracks** (slice 5, `oxiguitar_edit`'s `tracks.rs`): Add Track puts a steel-string guitar after the caret's track and moves the caret to it; Duplicate names the copy "X (copy)"; Delete never deletes the last track. The track dialog is one change. Changing the strings or tuning keeps each note's pitch on its own string when it fits, else on a free string of its beat at the fret nearest where it was; or keeps the frets, letting the pitches change. Notes no string can play are left out after a question naming how many. Identical chord diagrams left after restringing are merged. The window follows each track where it went (`was` in the worker): shown tracks stay shown after tracks are added, deleted or moved, a new track is shown with the caret on it, and a caret left on a hidden track goes to the nearest shown one. An edit that changes the track list waits for the window's new list of shown tracks before engraving.
- **Transposition** (slice 5, `transpose.rs`): by semitones, or by steps of the key (the major scale of the key's tonic, so the music stays in its key). Keeping the fingering keeps each note on its string; otherwise each note goes to the fret nearest the one it had, so a run stays in position. Notes past a string's range or MIDI's are left out after a question. Every track (Tools → Transpose…) moves the key signatures too, choosing six flats over six sharps; a track or a selection leaves them. Drum tracks are never transposed. A right-click on the page moves the caret there unless it falls inside the selection; Transpose… there acts on the selection, or the caret's beat.
- **Voices** (Mateus, 20 Oct 2026): only the caret's voice is drawn in full ink (black, or white on a dark page); the other voices are greyed. A PDF draws every voice, and everything in it, in pure black (`Theme::PRINT`).
- **Drum tracks** take no frets: their notes are drum sounds, entered with the drum tools later.
- **Effects** (slice 6, `oxiguitar_edit`'s `effects.rs`; Mateus, 20 Oct 2026): an effect is switched on or off. A note effect acts on the note on the caret's string and does nothing when that string has no note in the beat; on a staff without strings, on every note of the beat. A beat effect acts on the caret's beat. With a selection, every note (or beat) selected gets it, unless all of them have it already: then it comes off all of them. Rests take no effects, but dynamics. Effects that need strings (slides, bends, harmonics, vibrato, palm mute, let ring, trills, hammer-ons, tapping, slapping and popping, strokes, the whammy bar) apply only to notes on strings, never on drum tracks. A note slides in one way and out one way, and has one harmonic: another of the same side or kind replaces it. An effect with a value starts at a default, which the Inspector changes: a trill with the next fret up in sixteenths, tremolo picking in thirty-seconds (as Guitar Pro 8's key), a brush over a sixteenth, an artificial or tapped harmonic 12 up. A grace note becomes a thirty-second, as Guitar Pro writes them (83 of the corpus's 83 grace notes in .gp files are 32nds or 64ths). Tremolo picking is kept on the beat, as .gp files hold it. A dynamic is set, never switched off: it goes on the beat (notes and rests) and its notes play at it; there is always one in force.
- **Effect keys** (Mateus, 20 Oct 2026): Guitar Pro 8's keys, and for the effects it gives none, the effect's first letter with at most one modifier, the commonest guitar effects first; without a free one, no key. T is tapping (Guitar Pro's `)` too), Shift + T Text and Ctrl + T lyrics (21 Oct 2026); Alt + S shift slide and Shift + S slide out downwards (S stays Guitar Pro's legato slide; the corpus's .gp files have 178 shift slides, 77 legato, 21 out downwards, 15 in from below, 1 in from above); Alt + P pinch harmonic (P and Shift + P stay palm mute); Alt + T tapped harmonic. No key: slides in from below and above and out upwards, the semi-harmonic, popping, and the dynamics.
- **The bend editor** (slice 6, `bend_form.rs` in the app): as Guitar Pro 8's Bend window, a type, a pitch from ¼ tone to 3 tones, and a curve dragged on a grid of twelfths of the note by quarter tones. The curve is always an origin, a middle held at one pitch between two places, and a destination: the shape .gp files hold, so what the editor shows is what is saved. A curve read from a file that has another shape opens as its first, furthest and last points, as the .gp writer keeps it. The whammy bar's editor is the same, down to −3 tones and up to +3.
- **The Inspector's note and beat** (slice 6, `effect_form.rs` in the app; Mateus, 20 Oct 2026: the Inspector edits them, and effects with a value start at a default it changes): under the caret's bar, the note on the caret's string and its beat, as rows of tickboxes, drop-downs, numbers and buttons. A tickbox sends the effect's own command, so it acts as its key does (on the selection too); a drop-down or a number sets a value (`oxiguitar_edit::NoteChange`, `BeatChange`): accent, slide in and out, harmonic and its interval, trill speed and fret, fingering, dynamic, tremolo speed, grace note, brush direction and speed, pick stroke, technique. Bend and Whammy bar open the editor. Trills faster than sixteenths are allowed; a .gp file names them as a loss (GPIF has no trill speed), and .gp5 holds them.
- **Chords** (slice 6; `oxiguitar_edit::harmony`, the app's `chord_form.rs`; Mateus, 20 Oct 2026: with Guitar Pro 8's builder): a chord is a root (17 spellings, C to B with sharps and flats), a type (major, minor, augmented, diminished, sus2, sus4, power chord), a sixth, seventh or extension (6, 6/9, 7, maj7, 9, maj9, 11, 13, add9; 7 on a diminished triad is dim7), alterations (♭5, ♯5, ♭9, ♯9, ♯11, ♭13) and a bass; its name is written in ASCII as tabs write it (`C7(b9)/E`, `Am(maj7)`, `Bb(add9)`), which the .gp writer spells for Guitar Pro. A name the builder doesn't make is kept as typed. The ways to play it: every shape within four frets that sounds every note (the fifth, and the ninth and eleventh of an eleventh or thirteenth, may be left out), its bass lowest, at least three strings (two for a power chord), at most four fingers with a barre that no open string lies under; ranked by fingers, strings skipped between played ones, stretch, height and strings not played. A diagram's notes are named by the simplest chord that is exactly them (its lowest note the root if it can be, else the bass). The track keeps each diagram once: one it has is used again (as the .gp reader keeps them). `Command::Chord` puts a diagram on the caret's beat, or takes it off (the diagram stays in the track's list); on a rest, or the place after the voice's last beat, a diagram with frets puts its notes in the beat, as Guitar Pro 8 does on an empty beat. A diagram's frets are absolute (0 open, `None` not played), never below its first fret.
- **Text** (`Command::Text`): the text above the caret's beat (a note or a rest), trimmed; empty for none. Shift + T (Mateus, 21 Oct 2026; T is tapping).
- **Lyrics are typed on the page, as in MuseScore** (Mateus, 21 Oct 2026; `Command::Syllable`, `oxiguitar_model::lyrics`, the app's `Lyric` in `editing.rs`): Ctrl + T (Guitar Pro's time signature key) opens a field under the caret's beat on the page, on line 1 at the first beat the track's first voice sings from the caret's moment; Space and hyphen set the syllable and go to the next sung beat (a hyphen means the word goes on), Backspace in an empty field goes back, Up and Down change line (up to five, a new one under the last once the current holds something), Enter, Escape or a click elsewhere end. No lyrics dialog (Mateus, 21 Oct 2026). The model keeps Guitar Pro's lines: a text from a bar, a syllable per sung beat (first staff, first voice, with notes, not a grace note, not all tied) in order. Guitar Pro's syntax is followed exactly (its guide, "Lyrics Editing"): a space or a hyphen ends a syllable, each further one leaves a beat empty, as does one before the first syllable; a run of line breaks is one space; a plus joins words on one beat; square brackets hold a comment, and space right after one is no empty beat (Mateus, 21 Oct 2026: follow Guitar Pro, so files with several spaces show gaps, as in Guitar Pro). Typing a syllable changes only its own place in the text, keeping comments and line breaks; a beat past the end is reached with spaces, at the start with hyphens (a reader trims spaces at the ends). A new line starts at the beat's bar; typing before a line's start moves its start there. A line left with no syllable goes (a file can't keep an empty line in its place), and those after it move up. The caret follows the field, and every edit leaves it where typing goes on.
- **Dynamics** are hidden on the page by default (View → Dynamics; Mateus, 20 Oct 2026: they clutter the page).
