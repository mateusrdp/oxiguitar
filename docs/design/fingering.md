# Automatic fingering (#157, v0.15)

Picks a string and fret for each note of a passage so a human hand can play it, and plays it with as little effort as it can. The brief calls it tab derivation (TAB-3, TAB-4): the format readers that hold pitches without strings (MusicXML, MuseScore, MIDI files, ABC; #223) depend on it, and the editor offers it as a tool for tabs whose fingering is poor.

## What exists

- A note is `NotePlace::Fretted { string, fret }` (string 1 the highest, fret counted from the capo) or `NotePlace::Pitched(Pitch)` (no string yet). `Staff::pitch_of` gives a fretted note's sounding pitch, capo included.
- Three places already pick a string for a pitch, each greedily, one beat at a time: typing a note on the standard staff (`Command::Pitch`: the lowest fret, nearest the caret's string), transposing with new frets (`transpose::moved`: the fret nearest the note's old one) and changing a track's strings while keeping pitches (`tracks::replace`). None looks at the notes around the beat or at the hand.
- Readers of formats without tablature are plugins. They return the wire score, where `Pitched` already exists.

## The algorithm

A cost-minimising search over the whole passage, as the brief asks (dynamic programming, Viterbi style; the classic approach for string fingering, Sayegh 1989, Radicioni 2004, and the one most tab programs use).

1. **Slices.** A staff's notes are grouped by onset across its voices: every note struck at one moment is one slice, since one hand plays all voices of a staff. A note still ringing from an earlier slice keeps its string busy, and its fret counts in the hand's span (the held bass note).
2. **Candidates.** For each slice, every way to put its notes on distinct strings within the frets. Each candidate gets a hand: its fretted notes assigned to fingers 1 to 4 in fret order, without crossing (a finger never behind a lower-numbered one), the index allowed to barre one fret across strings. A candidate needs no more than four fingers (a thumb on the lowest string, at most two frets behind the index, counts as a fifth in chords only, at a price); the stretch from index to little finger is at most four frets (five from the seventh fret up, at a price). Candidates that no hand can play are dropped, unless none is left: then the cheapest is kept and marked hard.
3. **Cost of a candidate** (what makes one chord shape or note better than another): the stretch (nothing up to three frets, a four-fret box; more for four, from the fifth fret up less; five only from the seventh fret up), barres with other fingers than the index, the thumb, and, on a beat with a chord diagram, any note off the diagram's strings and frets. No fret or open string is preferred for its own sake (Mateus, 8 Nov 2026); exact ties go to the note's own place, then the lower fret.
4. **Cost of moving** from one candidate to the next: the hand's shift along the neck (cheap within two or three frets, the overlapping blocks the issue asks for; steeper beyond, and steeper still the less time there is), string skipping between consecutive single notes, and leaving the string of a note tied, slid, hammered or pulled to the next (a tie is never broken: its note keeps its string).
5. **The path.** Viterbi picks the cheapest sequence of candidates through the passage. Each slice keeps at most 48 candidates (the cheapest by their own cost). The notes around the passage (the beat before and after a selection) are the path's fixed ends, so a fingered bar joins its neighbours.

Repeated motifs get the same fingering because the costs are the same each time; the bass note is reachable because the held note stays in the hand's span; the next note is reachable because moving is what the path minimises.

Hard spots, where the cheapest hand still breaks a limit (more than four fingers, a stretch past five frets), are reported, not hidden.

## Where it lives

- `oxiguitar-edit::fingering`: the search, on the score model only, tested without a window. `Command::Fingering(Scope)` runs it as one undoable change, with the same scopes as Transpose (the selection or beat, a track, every track); `finger_pitched` places the notes that have only a pitch, and nothing else.
- The app and `oxiguitar-cli` call `finger_pitched` on every score a format plugin reads. The readers stay as they are: a reader without tablature returns `Pitched` notes on a staff with a tuning, and the core fingers them. No change to the plugin interface.
- The three greedy places above keep their rules: they place one note as the user types it.

## Decided by Mateus (8 Nov 2026)

- **Where it starts:** like Transpose, without a dialog. Right-click on the page and Edit → Find Fingering for Selection (the selection, or the caret's beat), right-click on a track's name (the track), Tools → Find Fingering (every track).
- **What it rewrites:** every note in scope, whatever its string, finger number or effects. Harmonics stay harmonics of the same note and kind: a natural harmonic may go to any string and node that sounds the same pitch; artificial, pinch, semi and feedback harmonics keep their fretted pitch; a tapped harmonic's touched fret moves with its note. A trill's other fret moves with its note, and a place where it would leave the neck isn't offered.
- **Files without tab:** fingered as they open, so the tab shows at once (not an Undo step, no texts).
- **Scoring:** the whole neck is equally good; only the hand's comfort and the transitions count. Beats that stay hard get a text in the score: "Hard: stretch", "Hard: fingers" or "Hard: shift" (seven frets or more within an eighth note). A beat that has a text of its own keeps it; a later run takes away its own texts where the beat is no longer hard.

## What the corpus taught (8 Nov 2026)

`cargo test --release -p oxiguitar-edit --test fingering_corpus -- --nocapture` takes each Guitar Pro file's tab away, fingers the pitches again, and runs Find Fingering on every track; every note must sound as it did, ties included (a tied note sounds as the note before it on its string, as playback has it). On the files the gp5 and gp plugins read, about 177,000 notes, 66 % land exactly where the transcriber put them, and no file takes more than 0.06 s. What it found:

- **A tie continues the note before it on its string**, whatever fret the file wrote on the tied note (some files have stale numbers there). For a note with only a pitch, the last note of that pitch in its voice.
- **A tie that continues nothing** (nothing ringing up to it on its string) is struck, and its tie mark goes: playback would otherwise stretch whatever note the string last had.
- **One note struck by two voices** is played once; it rings as long as the longer of the two, so a tie in either voice keeps its string.
- **The left hand's effects** (bend, vibrato, slide, trill) need a fretted note: no open string is offered for them, unless the note is open already.
- **A hammer, pull or slide** goes to the next note on its string in its voice. When that note is the very next beat, the search keeps the two on one string (a cost); otherwise the note keeps its place, and so does the note it reaches, since moving either would make it reach another note.
- **Frets past the neck** (some files write fret 25 on a 24-fret staff): a note may always keep the place it has; otherwise only the neck's frets are offered.
- **No hand shape at all** (more notes than strings, or ringing notes where struck ones must go): the struck notes get distinct strings first and the ringing ones are cut short; a note no string is left for keeps its place (or stays a pitch). A ringing note moved costs a lot but not infinitely, so the path stays sound around it.

## Limits

- Time is measured in whole notes, not seconds: a shift costs the same at any tempo for the same written values.
- A motif is fingered alike because its costs repeat, not because the search looks for repeats.
- The right hand isn't modelled (picking direction, string crossing for the pick), beyond string skipping between single notes.
- Notes in the same moment on two voices with one pitch are played once and share a place.
