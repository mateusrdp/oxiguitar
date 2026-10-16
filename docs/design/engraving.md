# Engraving

How `oxiguitar-layout` turns a score into pages, and what it does not draw yet. Requirement IDs refer to the project brief.

## Pipeline

0. **Keys** (`key.rs`). Most tabs set no key signature: every bar stays in C major, Guitar Pro's default, whatever the music. When a file sets no key other than C anywhere, the engraver infers one per bar (Mateus, 16 Oct 2026):
   - For each bar, the music in each pitch class is counted across every pitched track, written pitch, weighted by duration (a note counts for at most a half note; grace notes a sixteenth; tied and dead notes and drums not at all).
   - For each key from six flats to five sharps, a bar costs the share of its music outside that key's scale. G flat major stands for F sharp major, as songs in E flat tuning are usually written.
   - A best path through the song (Viterbi) picks one key per bar. Changing key costs as much as two and a half bars wholly out of key, one and a half where a section starts (a marker, a repeat, after a double bar), plus a fifth of a bar per step round the circle of fifths. So a chromatic run stays in the key, while a modulation that lasts changes it where it happens. Ties go to fewer accidentals.
   - The inferred keys are used only if they spare at least 2.5 % of the music its accidentals compared with C; chromatic riffs stay in C.

   On the private corpus, the inference agrees with every file that sets its own single key (Wish You Were Here, Little Wing, Shine On, The Loner, Reelin' in the Years) and finds the modulations of The Bigger Picture and Sardinha Desvairada bar for bar. Where it differs, it usually picks a relative or neighbouring key (Surfing with the Alien's modal passages, for one). `Options::infer_keys` (`--no-key-inference`) turns it off.
1. **Views** (`system.rs`). Each shown track contributes one view per staff: standard notation, tablature or both, as the file says (VIEW-1). Tracks with no strings get only standard notation; drum tracks get a drum staff.
2. **Plans** (`plan.rs`, `pitch.rs`, `rhythm.rs`). Every bar of every view is planned once:
   - beats are grouped into columns by start time;
   - notes are spelt in the key (flats in flat keys, except the relative minor's leading tone), placed on staff steps and given the accidentals the bar needs;
   - stems follow the voice number, always: voices 1 and 3 up, voices 2 and 4 down, on the staff, on drums and in rhythm on tab (Mateus's rule, 16 Oct 2026);
   - beams group eighths and shorter within a beat (a dotted quarter in compound time);
   - the room each column needs on each side (noteheads, seconds, accidentals, dots, flags, fret numbers, chord names) is measured.

   Voices after the first that hold only rests in a bar are not drawn, as Guitar Pro does.
3. **Grid** (`grid.rs`). The columns of all views in a bar merge into one grid, so every staff of every track puts the same moment at the same x (VIEW-1, VIEW-4). The distance between columns grows with duration, less than in proportion (1.6 + 2·q^0.6 staff spaces for q quarter notes), and widens where symbols would touch.
4. **Systems** (`system.rs`). Bars fill a line at natural spacing. The system is then stretched to the full width, sharing the extra room by duration. The last system is stretched only if it is more than two thirds full. A single bar wider than the line is squeezed.
5. **Drawing** (`engrave.rs`). Each system is drawn at the origin:
   - Marks above the staff go on lines of their own, bottom to top: volta brackets, chord names, beat text, tempo, sections. A line exists only when the system uses it, so marks cannot collide with each other or with the notes and stems.
   - Chord names stay exactly over their beat and are never pushed sideways (Mateus, 16 Oct 2026). The grid opens up instead, so each name ends before the next one starts and before the bar line.
   - Tablature always sits under its standard staff. When standard notation is hidden (the file says which notation each track shows), the tab shows the rhythm of every voice (VIEW-2). Stems follow the same voice rule as the staff: voices 1 and 3 above the tab pointing up, voices 2 and 4 below pointing down. Each voice's beams, flags, dots, tuplet numbers and rests (at 80 % size) sit in its own rhythm area, clear of the fret numbers.
   - Effects and marks (`effects.rs`) are drawn once every bar of the view is placed. Marks that run along the music get **lanes** of their own above the tablature (between the staff and the tab when both are shown; above the staff when there is no tab), nearest the tab first:
     - H/P letters, lifted clear of the arcs over the top string;
     - bends;
     - vibrato;
     - trill;
     - P.M.;
     - let ring;
     - harmonic kinds;
     - techniques: tap, slap, pop, w/bar, fade in, pick strokes.

     A lane exists only when the system uses it, so marks cannot collide.
   - **Hammer-ons, pull-offs and slides** connect a note to the next note on the same string, even in the next bar. On the tab that is an arc or a slanted line; on the staff, a slur or a line between noteheads.
   - **Bends** draw arrows from the fret number, with the amount ("1/4", "1/2", "full", "1 1/2") over each peak, release arrows down, and dashes while the bend is held. Pre-bends are drawn too.
   - **Notehead marks:**
     - natural harmonics: diamond heads and `<12>` on the tab;
     - ghost notes: in parentheses;
     - dead notes: × heads;
     - accents and staccato: on the notehead side away from the stem;
     - tremolo strokes: on the stem;
     - strums: an arrow left of the chord.
   - **Grace notes:** small notes before their main note, stem up, slashed when played before the beat, slurred to the main note; small fret numbers on the tab. Spacing makes room for them.
   - **Under the staff:**
     - Dynamics are shown where the first voice's dynamic *marking* changes from Guitar Pro's default *mf*, unless `Options::dynamics` is off (`--no-dynamics`). Only Guitar Pro 6–8 store markings (a beat property); Guitar Pro 3–5 only store a velocity per note, which is not a marking and is never shown.
     - Lyrics go one line per verse, one syllable per sung beat (tied notes continue the syllable). Spacing keeps syllables apart.
   - With several tracks, each track's name stands left of its staves: the full name on the first system, the short name after, shortened with "…" past a fifth of the line.
6. **Pages** (`page.rs`). The header (title, subtitle, artist, album, credits on the right, tuning and capo on the left, copyright at the foot) goes on page one (VIEW-6). Systems stack down the page, and a new page starts when the next system does not fit. If a system is taller than a page, the whole score is engraved again with a smaller staff.

Every symbol is a SMuFL glyph placed by the font's metadata: stem anchors, bounding boxes, engraving defaults (VIEW-7). The output is a display list per page, so the screen, PNG and PDF show exactly the same thing.

## Sizes

- **Staff space:** 4.5 pt by default, a 6.4 mm staff. A4 with 15 mm margins.
- **Tablature:** lines 1.5 staff spaces apart. Fret numbers are 1.2 times that gap, with the line knocked out behind them.
- **Rhythm on tab:** stems end 3 staff spaces beyond the tab staff (above it for voices 1 and 3, below for 2 and 4).
- **Grace notes:** 65 % of a normal note.
- **Text:** characters Noto Sans lacks (♯ ♭ ♮) come from Bravura, raised to sit with the letters.
- **Chord diagrams in the header:** 4.2 pt between strings, 5.2 pt between frets, four frets shown.

## Checking

- **Tests:** `cargo test --release -p oxiguitar-layout --test engrave` engraves the public fixtures, damaged scores and a 300-bar 8-track score. Where `corpus/files/` exists, it also engraves every Guitar Pro file in the corpus with all its tracks. Each run checks that every item lands on the page, every glyph exists in its font and output is deterministic.
- **By eye:** `oxiguitar-cli render FILE --png out/page --pdf out/score.pdf [--tracks all] [--dark] [--no-dynamics] [--no-key-inference]`.

## Not drawn yet

These are the M1 engraving items still to do (see ROADMAP.md):

- **Notation marks not in the score model yet:** slurs between different notes (other than hammer-ons, pull-offs and legato slides), hairpins, ornaments, fermatas and ottava lines (NOT-3). The readers log them as not carried over.
- **Effects:** whammy-bar curves (shown as "w/bar" only), fingering numbers, and articulations on tab-only tracks.
- **Other layouts:** Vertical, Horizontal and Parchment (VIEW-3).
- **Overlap pass, second round:** bend amounts of quick successive bends can touch, and dense chords can crowd ledger lines (VIEW-7). Tuplet numbers and articulations now reserve their room.
- **Courtesy signatures:** key and time signatures at the end of a system before a change.
