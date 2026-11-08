# Fitting bars to their time signature (#222, v0.16)

The user writes a song in one time signature and later decides it belongs in another: 4/4 full of triplets that wants 12/8 or 3/4, 6/8 that reads better as 3/4, 3/4 that should have been 4/4. Today they set the new time signature (Bar tab, from the bar until the next change), every bar turns red ("doesn't add up") and they rewrite every bar by hand. #222 asks for a tool of the core editor, like Transpose, that fits the bars into the time signature they now have, finding where tuplets are needed.

## What exists

- **Time.** Durations are exact `Ratio`s of a whole note; `Duration` is a value, dots and a stack of `Tuplet`s (outermost first), and the engraver brackets runs of beats that share their first tuplet. A bar's length is its `TimeSignature::bar_length`.
- **Bars.** `MasterBar` (shared by every track: time and key signature, repeats, endings, marker, double bar, feel, tempo changes at positions in the bar, directions, free time) and per staff a `Bar` of one to four `Voice`s of `Beat`s. A grace beat takes no time and belongs to the beat after it. `BeatKind::Empty` beats are Guitar Pro's placeholders for unused voices.
- **The bar check.** `check::adds_up` and `BarCheck`: a voice that holds a note or rest must last exactly its bar; free-time bars always add up. Tools → Next Bar That Doesn't Add Up (F4) walks through the red bars.
- **How bars sound.** `oxiguitar_play::time::bar_lengths`: a bar lasts its time signature, or its longest voice when that is longer. A red bar plays every note it has.
- **Changing the time signature** (`Command::TimeSignature`) changes master bars only, and rescales the tempo changes inside them.
- **Ties.** A note with `tie` continues the note before it on its string in its voice; `entry::retie` gives tied notes their fret again after bars move. Lyrics (`lyrics::is_sung`) skip a beat whose notes are all tied, so splitting a note into tied pieces keeps every syllable on its note.
- **Scopes.** Transpose and Find Fingering share `Scope` (selection or beat, track, every track), `transpose::spans` and `transpose::regions`; their menus are the page's right-click menu, the track row's right-click menu, Edit and Tools (`main.rs`, `app.slint`). The Edit menu is already taller than an 800 px screen.

## Two ways to fit a bar

1. **Re-bar** (the music flows across the bar lines). Every note keeps its length and its moment; only the bar lines move. 4/4 to 3/4 turns four bars into five and a third. A note that now crosses a bar line is split into notes tied across it; a rest is split without a tie. Bars are added or removed at the end of the passage, in every track. No tuplet is ever needed. This is what 6/8 ↔ 3/4 and most 4/4 ↔ 3/4 changes want, and the song sounds exactly as before.
2. **Stretch** (each bar keeps its notes and is refitted on its own). The bars stay where they are; every beat of a bar is scaled by the bar's new length over its old one, and written as the plain value that length is, or with a tuplet where no plain value is (3/4 to 4/4: three quarters become a half-note triplet; 4/4 triplet eighths to 3/4: plain sixteenths, the triplets gone). The bar count never changes, so repeats, endings, markers and lyrics stay where they are. This is #222's own example ("3/4 to 4/4 needs tuplets").

Both work only on the passages that need them: a passage whose bars all add up is left untouched, so running the tool on a whole song changes only the red bars.

### Re-bar, in detail

- **The old time line.** Each old bar lasts as long as its longest voice that holds music (an empty bar, as long as its time signature). Not as the player plays a red bar: the player pads a short bar to its time signature, but a bar is short here because its time signature is new (3/4 music marked 4/4), and the padding would put silence in the music. Every beat gets its moment on that line; gaps a short voice leaves become rests.
- **Passages.** The time line is cut where the structure needs a bar line: a repeat sign, an alternate ending's start or end, a section (marker), a direction (segno, coda, fine and the jumps), a double bar, a free-time bar, and a change of time or key signature. Each passage is re-barred on its own, in its time signature. Passages whose bars all add up are left as they are.
- **The last bar of a passage** that its music doesn't fill is left short, and the bar check marks it (Mateus, 8 Nov 2026): the user sees where the phrase didn't fit.
- **What moves with the time line**: the triplet feel goes to the bar its moment falls in; repeat starts, sections and segno or coda signs go on a passage's first new bar, repeat ends, double bars and the other directions on its last; tempo changes keep their moment (bar and position); chord diagrams, texts and beat effects stay on their beat (on the first piece of a split one); lyrics keep their syllables (tied pieces aren't sung) and a line's first bar moves with the bar it started on.
- **A split note**: the first piece keeps what is struck (accent, dead and ghost, harmonics, bends, slide into, pick stroke, chord, text); the last keeps what leads on (hammer-on or pull-off, slide out or to the next note, staccato); both keep what lasts (palm mute, let ring, vibrato, tremolo picking). Each piece is written as the fewest tied plain values (one or two dots allowed), inside the tuplet its note was in.
- **A pickup**: a first bar of the song shorter than its time signature and than the bar after it is a pickup, and stays as it is (in both ways). The second condition tells a pickup from a first bar of 3/4 music now marked 4/4.
- **Grace notes** stay with the beat they lead into.

### Stretch, in detail

- Only red bars that aren't in free time and aren't a pickup. The bar's old length is its longest voice holding music; the factor is the new length over that. Every voice of every staff in the bar is scaled by the same factor, so voices stay together.
- A scaled beat is written, in order of preference: as a plain value with its own tuplets kept or dropped (a triplet eighth times 3/4 is a sixteenth), as its tuplets with one more inside them, or with one tuplet (3:2, 5:4, 7:4, 9:8 and so on, by the odd part of its length); a length no single note can show becomes tied notes.
- Tempo changes inside the bar keep their place in it (scaled). The tempo itself stays (Mateus, 8 Nov 2026): a stretched bar lasts its new time signature.

## The tool

- **Fit Bars to Time Signature…**, a small dialog: Re-bar or Stretch, and what it applies to.
- Since re-barring adds and removes bars in every track, the tool always works on every track: on the selected bars, or on the whole song. It is one change for Undo.
- Menus: Tools (the whole song) and the page's right-click menu (the selected bars, or the caret's bar). Not the Edit menu: it already runs off an 800 px screen. Nothing to fit, nothing changes (and nothing goes on the Undo list).
- The time signatures themselves are the user's: the tool never picks one. Tuplets appear only when stretching; re-barring never needs them.

## Where it lives

- `oxiguitar-edit::fit`: `Command::FitBars(Fit { how, whole_song })`, recorded as `Region::Whole`. `spell` writes a length as a value (two dots at most), a tuplet stack, or tied values; `piece` splits a beat's effects between the first and last pieces.
- The app: `FitDialog` in `bar_dialog.slint`, `open_fit` and `fit_dialog_done` in `main.rs`.
- Tests: unit tests in `fit.rs`; `tests/fit_corpus.rs` changes every corpus file's time signatures and fits them both ways, checking that re-barring strikes every note at the same moment and stretching strikes the same notes in the same order; a worker test.

## Decided by Mateus (8 Nov 2026)

- Both ways, picked in a small dialog.
- Started from Tools (the whole song) and the page's right-click menu (the selected bars, or the caret's bar); always every track.
- Re-bar: a passage that doesn't fill its last bar leaves it short and red (not padded with rests, not flowed through repeats).
- Stretch: the tempo stays as it is.
