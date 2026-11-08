# The rest of the palette (v0.17)

v0.17 closes the palette: every tool that still says "comes in M5" works, the open M1 engraving items are drawn, and the two editing modes at the top of Guitar Pro 8's palette, Multivoice and Design Mode, arrive. ROADMAP.md, "The way to v1.0", step 5; the brief's NOT-3 (hairpins, ornaments, ottava lines, fermatas) and the part of TAB-2 the palette holds.

## What the palette lacked (8 Nov 2026)

Forty-nine of the palette's 124 tools did nothing:

| Group | Tools |
| --- | --- |
| Voices and modes | Multivoice, Design Mode |
| Bar | Single and Double Repeat Sign (simile), 8va, 8vb, 15ma, 15mb |
| Note | Nested N-tuplet, Fermata, Double Flat, Flat, Natural, Sharp, Double Sharp, Change Accidental, Crescendo, Decrescendo |
| Effects | Legato (slur), Sustain Pedal, Fade-out, Volume Swell, Upper and Lower Mordent, Turn, Inverted Turn, Wide Left-Hand Vibrato, Slight Bridge Vibrato, Closed and Open Wah-Wah, Downward and Upward Arpeggio, Rasgueado, Left- and Right-Hand Fingering, String Number, Finger and Thumb Golpe, Upward and Downward Pick Scrape, Dead Slap |
| Notation | Slash, Extended Barre, Automatic / Force / Break / Break Secondary Beaming, Force Groups |

Also open from M1 (engraving.md, "Not yet"): whammy-bar curves on the tab (drawn as "w/bar" only), fingering numbers, courtesy signatures before a change, and a second overlap pass. Left- and right-hand fingers were already in the model, read from files and set from the Inspector, but never drawn.

None of the rest was in the score model. The Guitar Pro readers warned and dropped them ("hairpins are not supported yet"). In the corpus they are rare: one file has 248 beats under 8va, one has 8 decrescendo beats, eight have slight vibratos; Guitar Pro also stores a spelling for every note, which the readers ignored.

## Decided by Mateus (8 Nov 2026)

- **The Guitar Pro plugin follows in the same session**: it reads and writes every new mark, released after the core (a plugin built on the 1.6 SDK needs v0.17).
- **Sound: play what we can.** Rasgueado plays as fast strokes by its pattern; golpe and dead slap as muted percussive hits; wah and pick scrape are shown only (no wah filter or scrape sample in our sets; Guitar Pro plays neither pick scrape nor dead slap).
- **Spellings: keep the file's.** A note keeps the spelling its file gives where it differs from the one OxiGuitar would work out, so scores look as their authors wrote them.
- **Multivoice and Design Mode are both in v0.17.**

## The model

Like Guitar Pro, marks that span several beats are stored **on each beat they cover**, never as objects with a start and an end: an 8va line is the run of consecutive beats of a voice that carry `Ottava::Alta8`, a slur the run of beats each slurred to the next. Edits (insert, delete, copy, transpose, fit) then need no bookkeeping, as tuplets already need none: the engraver groups runs, the player reads each beat.

`oxiguitar-model` gains:

- **Beat** (`Beat`): `slur` (slurred to the next beat of its voice), `hairpin: Option<Hairpin>` (crescendo, decrescendo), `ottava: Option<Ottava>` (8va, 8vb, 15ma, 15mb), `slash`, `beaming: Beaming` (automatic, force to the next beat, break before, break secondary before), `tuplet_group` (Force Groups: consecutive equal tuplets from here are bracketed as one), `barre: Option<Barre>` (fret, full or half).
- **Beat effects** (`BeatEffects`): `fade: Option<Fade>` (in, out, volume swell) in place of `fade_in`; `bridge_vibrato: Option<Vibrato>` (slight, wide) in place of `vibrato_wide`; `pedal: Option<Pedal>` (down, up); `wah: Option<Wah>` (open, closed); `arpeggio: Option<StrokeDirection>`; `rasgueado: Option<Rasgueado>` (Guitar Pro's eighteen patterns); `golpe: Option<Golpe>` (finger, thumb); `pick_scrape: Option<StrokeDirection>`; `dead_slap`.
- **Note effects** (`NoteEffects`): `vibrato: Option<Vibrato>` in place of the flag; `ornament: Option<Ornament>` (upper and lower mordent, turn, inverted turn); `string_number` (show the string's number).
- **Note**: `spelling: Option<Spelling>` (a step C to B and an alteration from double flat to double sharp). A spelling only counts while it spells the note's pitch: the engraver ignores one that no longer does, and writers drop it, so an edit that changes a pitch (a fret, a semitone, a transposition) never needs to clear it.
- **Master bar**: `fermatas: Vec<Fermata>` (position in the bar, short, medium or long, and Guitar Pro's length, kept for the round trip). On the master bar, as Guitar Pro stores them: a fermata holds every track.
- **Bar** (a staff's): `simile: Option<Simile>` (one bar, first or second of two). A simile bar keeps a copy of the bar or bars it repeats, made when the mark is set or the file is read, so time, the bar check, the player and every tool work on it as on any bar; the engraver draws the sign in place of its notes.
- **Score** and **Track**: `layout: Vec<u8>`, bars per system (Design Mode, below): the score's for several tracks shown together, a track's for that track shown alone. Empty means automatic.

## Interface 1.6

Each addition is a variant at the end of the matching extension list of the wire score (`score2.rs`), `since` 1.6: `MasterBarExt::Fermata`, `BarExt::Simile`, `BeatExt` (slur, hairpin, ottava, slash, beaming, tuplet group, barre, fade out and volume swell, bridge vibrato, pedal, wah, arpeggio, rasgueado, golpe, pick scrape, dead slap), `NoteExt` (wide vibrato, ornament, string number, spelling), `ScoreExt::Layout` and `TrackExt::Layout`. Version 1's own fields keep what they held (`fade_in`, `vibrato_wide`, a note's `vibrato`), so a plugin older than 1.6 still sees a fade-in and a vibrato, and a wide one as plain. The host already leaves out the variants a plugin doesn't know (`keep_known`); a write by an older plugin now names each kind it leaves out among the losses. The SDK crates go to 1.6.0. Nothing else changes: no message, no table.

## Editing

Every tool follows the effects' rules (M3 slice 6, Mateus 20 Oct 2026): on the caret's note or beat, or on every selected note or beat, on unless all have it, then off. A run-mark (slur, hairpin, ottava, barre, slash) set on a selection covers it; set at the caret it covers that beat, and the user extends it by selecting.

- **Note marks**: ornaments, wide left-hand vibrato (replacing a slight one, and back), string number, fingering (Left- and Right-Hand Fingering step through none, then each finger, as Guitar Pro's do; the Inspector picks one).
- **Accidentals**: Double Flat to Double Sharp spell the caret's note (or the selected notes) with that accidental where one spells its pitch, else leave it; Change Accidental steps through the spellings of the pitch. Raise and Lower by a Semitone keep using sharps up and flats down.
- **Beat marks**: fermata (at the beat's position in its master bar, medium; the Inspector changes the kind), sustain pedal (at the caret, off → down → up; on a selection, down on its first beat and up on its last), fade-out and volume swell (replacing a fade-in: one fade a beat), slight bridge vibrato, wah, arpeggio, rasgueado (ii at first; the Inspector picks the pattern), golpe, pick scrape, dead slap, extended barre (at the lowest fret the beat's fretted notes use, full; the Inspector changes fret and shape), slash, beaming, Force Groups.
- **Bar marks**: Single Repeat Sign copies the bar before into the caret's bar and marks it; Double Repeat Sign copies the two bars before into the caret's bar and the next. Editing a note in a simile bar removes its mark (the bar now holds its own music). 8va to 15mb are run-marks of the beat.
- **Nested N-tuplet** opens the tuplet dialog and adds the tuplet inside the one the selected beats already have.
- **Keys**: Guitar Pro 8's where it has them (Ctrl + M Multivoice); the other new tools have none, as in Guitar Pro, and are found with the palette's search.

The Inspector gets rows only for marks with a value: fermata kind, rasgueado pattern, barre fret and shape, and the existing fingering rows.

## Engraving

- **Slurs**: a curve over (or under, by stem direction) the noteheads of a run, broken at a system's end and continued on the next, as ties are; on the tab, over the fret numbers, as hammer-ons are drawn. A hammer-on arc and a slur over the same two notes draw once.
- **Hairpins**: wedges under the staff at the dynamics' line, from the first beat of the run to the end of the last, split at system breaks.
- **Ottavas**: "8va", "8vb", "15ma", "15mb" (Bravura) with a dashed line and a hook over (or under) the run; notes under it are drawn on the staff one or two octaves lower (higher for bassa) than they sound. The tab and the sound are unchanged.
- **Fermatas**: Bravura's short, medium and long fermata over the staff at their beat, and over the tab on tab-only tracks.
- **Ornaments**: their Bravura symbols over the note, with the articulations.
- **Fingering**: left-hand fingers (T, 1–4) beside the notehead on the staff, right-hand p i m a c above the staff; string numbers as circled digits beside the notehead.
- **Accidentals**: from the note's spelling when it spells its pitch, else as today.
- **Beat marks**: pedal "Ped." and its release star under the staff; wah "+" and "o"; arpeggio a wavy line with an arrow before the chord; rasgueado "rasg." with the pattern; golpe, pick scrape and dead slap their Bravura symbols over the tab; fade-out and volume swell wedges over the tab as the fade-in's; slight bridge vibrato a thin wavy line beside the wide one.
- **Barre**: "C" or "½C" and the fret in Roman numerals, with a dashed line over the run.
- **Slash**: slash noteheads on the middle line, the rhythm kept; the tab still shows the frets.
- **Beaming** overrides and Force Groups change the grouping the engraver already computes.
- **Nested tuplets**: a second bracket inside the first for each inner run.
- **Simile bars**: Bravura's one- or two-bar repeat sign centred in the bar (across the bar line for two), the notes not drawn.
- **Whammy curves**: the whammy bar's curve under the tab, as bends are drawn above it, with its depth in tones.
- **Courtesy signatures**: a key or time signature that changes at the start of a system is shown at the end of the system before it.
- **Overlap**: the new marks take their room as the articulations and dynamics already do; a second pass separates bend amounts that touch.

## Playback

- **Fermatas** hold every track: the beat at the fermata lasts half again (short), twice (medium) or three times (long) its length, everything sounding through it.
- **Hairpins** move the velocity from the dynamic in force at the run's start to the next dynamic after it, or one step up or down when there is none.
- **Ornaments**: a mordent plays the note, its neighbour a step up (upper) or down (lower) in the key, and the note again, the first two each a thirty-second; a turn plays upper, note, lower, note across the note's length (inverted: lower first).
- **Fade-out** and **volume swell** shape the volume as fade-in does: down to silence over the beat, or up from silence and back.
- **Vibratos**: wide left-hand vibrato twice the slight depth; slight bridge vibrato half the wide.
- **Sustain pedal**: notes under the pedal ring until it is released, as let ring.
- **Arpeggio**: the chord spread like a brush, over the beat's first half (at most a quarter note).
- **Rasgueado**: one stroke per letter of its pattern across the beat (triplet patterns in thirds, anapaest ones short-short-long), each a fast brush, down except an index finger's return.
- **Golpe** and **dead slap**: a dead (muted) note, golpe on the lowest string (thumb) or the highest (finger), dead slap on every string.
- **Ottavas**, slurs, slash, wah, pick scrape: as written, nothing added.

The timeline goldens change only for reference songs that use these marks; new reference songs (with their `guitar-pro-check` files) cover them.

## Multivoice (Ctrl + M)

Guitar Pro 8: "all notes active for changes". Here, with Multivoice on (the palette's button lit; Ctrl + M), every voice is drawn in full ink, the caret moves left and right through the moments any voice of the staff has a beat, taking the voice of the beat it lands on (the current voice first when several start together), and up and down onto the note on that string of whichever voice sounds there. Typing, effects and the Inspector act on the caret's voice as usual, and a selection stays in one voice. Ctrl + 1 to 4 still pick a voice and turn Multivoice off.

## Design Mode

Guitar Pro 8's Design Mode sets bars per system, bar widths and beat positions by dragging. Guitar Pro files store the first of these (bars per system, for the score and for each track); OxiGuitar v0.17 does that one, and leaves widths and beat positions to the automatic spacing.

- The palette's Design Mode (and View → Design Mode) toggles it. In the Page and Parchment layouts, each system then shows its bar count over its first bar, and the page's right-click menu offers **One Bar More on This System**, **One Bar Less on This System** and **Automatic Layout from Here**. Each is one step to undo.
- The page follows a stored layout in the Page layout (the score's for several tracks, the track's for one track alone); a system that can't hold its bars at their narrowest is broken automatically and drawn with its count in red in Design Mode. Other layouts ignore it.
- Files carry their layout: Guitar Pro writes one into every .gp file, so .gp files now open with Guitar Pro's systems.

## The Guitar Pro plugin

oxiguitar-guitarpro reads and writes every mark (alphaTab's GPIF reader is the reference for element names, MPL-2.0, read, never copied), keeps a note's spelling (the core drops the ones it would choose itself when the file opens), reads the systems layouts, copies a simile bar's source into it, and turns the sustain-pedal automations into beat marks. Pick scrape and Force Groups have no known Guitar Pro form: the .gp writer names them as losses. The .gp5 writer keeps what GP5 holds (slight and wide vibratos, fade-out and swell, rasgueado, string numbers, fingering) and names the rest. The plugin is built on the SDK pinned to the core commit, moved to the v0.17.0 tag once it exists, and released after the core.

## Pull requests

Stacked in the core, each with its tests: (1) the model, interface 1.6 and editing, with the palette and Inspector; (2) engraving; (3) playback; (4) Multivoice and Design Mode, with the manual. Then the plugin's PR.
