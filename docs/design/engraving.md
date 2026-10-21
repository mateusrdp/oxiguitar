# Engraving

How `oxiguitar-layout` turns a score into pages or sheets, and what it does not draw yet. Requirement IDs refer to the project brief.

The engraver shows what the file says and never second-guesses it: a tab that leaves every bar in C major is engraved in C major. Changes such as inferring key signatures are tools the user picks ([tools.md](tools.md)).

## Pipeline

1. **Views** (`system.rs`). Each shown track contributes one view per staff: standard notation, tablature or both (VIEW-1; see "Clefs, tablature and keyboards" below for which). Tracks with no strings get only standard notation; drum tracks get a drum staff; a keyboard track becomes a grand staff.
2. **Plans** (`plan.rs`, `pitch.rs`, `rhythm.rs`). Every bar of every view is planned once:
   - beats are grouped into columns by start time;
   - notes are spelt in the key (flats in flat keys, except the relative minor's leading tone), placed on staff steps and given the accidentals the bar needs;
   - stems follow the voice number, always: voices 1 and 3 up, voices 2 and 4 down, on the staff, on drums and in rhythm on tab (Mateus's rule, 16 Oct 2026);
   - beams group eighths and shorter within a beat (a dotted quarter in compound time);
   - tuplets are marked with their number of notes ("3", "6") when they take the usual time (the power of two just below: 3:2, 5:4, 6:4, 7:4, 9:8; or 2:3 and 4:3), and with the full ratio ("5:3") otherwise, so any grouping reads unambiguously. Successive tuplets are marked one by one, as the file stores them: two triplets show two 3s, a sextuplet one 6 (Cliffs of Dover stores its runs as triplets);
   - the room each column needs on each side (noteheads, seconds, accidentals, dots, flags, fret numbers, chord names) is measured.

   Voices after the first that hold only rests in a bar are not drawn, as Guitar Pro does.
3. **Grid** (`grid.rs`). The columns of all views in a bar merge into one grid, so every staff of every track puts the same moment at the same x (VIEW-1, VIEW-4). The distance between columns grows with duration, less than in proportion (1.6 + 2·q^0.6 staff spaces for q quarter notes), and widens where symbols would touch.
4. **Systems** (`system.rs`). Bars fill a line at natural spacing (in the Horizontal layout the line has no end, so every bar goes on it, unstretched). The system is then stretched to the full width, sharing the extra room by duration. The last system is stretched only if it is more than two thirds full. A single bar wider than the line is squeezed.
5. **Drawing** (`engrave.rs`). Each system is drawn at the origin:
   - Marks above the staff go on lines of their own, bottom to top: volta brackets, chord names, beat text, directions, tempo, sections. A line exists only when the system uses it, so marks cannot collide with each other or with the notes and stems. Directions put segno and coda signs (scaled to 2.4 staff spaces) at the start of their bar, and Fine, the D.C./D.S. jumps and "To Coda" ending at its end, 0.7 staff spaces clear of the bar lines so a jump and the next bar's sign stay apart.
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
     - Lyrics go one line per verse, one syllable per sung beat (tied notes continue the syllable). Spacing keeps syllables apart. As in Guitar Pro, a space or a hyphen starts the next syllable and each further one leaves a beat empty (a run of line breaks counts as one space), a plus joins two words on one beat, and text in square brackets is a comment that isn't drawn (M3 slice 6; `oxiguitar_model::lyrics`).
   - With several tracks, each track's name stands left of its staves: the full name on the first system, the short name after, shortened with "…" past a fifth of the line.
6. **Pages and sheets** (`page.rs`), by layout (below). The header (title, subtitle, artist, album, credits on the right, tuning and capo on the left) goes at the top (VIEW-6), the copyright at the foot of page one or of the sheet.

Every symbol is a SMuFL glyph placed by the font's metadata: stem anchors, bounding boxes, engraving defaults (VIEW-7). The output is a display list per page or sheet, so the screen, PNG and PDF show exactly the same thing.

## Clefs, tablature and keyboards

Decided with Mateus on 16 Oct 2026.

- **Clefs.** Guitar Pro 6–8 files store the clef of every bar, and it is used as stored. Guitar Pro 3–5 files store none, so the reader picks it from the instrument as Guitar Pro does: basses, and any track tuned below G1, in the bass clef an octave down; guitars and other fretted instruments in the treble clef an octave down; everything else (keyboards, voices, winds, strings) in the treble clef at sounding pitch. This gives exactly the clefs Guitar Pro 7 wrote for the five corpus songs saved in both formats (checked by `oxiguitar-formats --test corpus`).
- **Tablature** is shown by default only for fretted string instruments (`Instrument::is_fretted`: guitars, fretted basses, sitar, banjo), and only where the file shows it. Keyboards, voices, winds, bowed strings, synth basses and fretless bass get standard notation. The user chooses each track's notation: standard, tablature or both (`Options::notation`; the track list's notation buttons; `render --notation 2=tab,3=both`). A track with neither is hidden (left out of `Options::tracks`). A keyboard shown as tablature only gets no grand staff.
- **Keyboards** (`Instrument::is_keyboard`: pianos, electric pianos, harpsichord, clavinet, organs, accordions) with a single staff in standard notation are shown on a **grand staff**: the right hand on a treble staff, the left on a bass staff, joined by a brace, with bar lines through both. If their tablature is asked for, it goes under the grand staff with every note. `hands.rs` splits the notes by hand range, not at a fixed note: a chord one hand can span (an octave and a little more) goes to one hand, the one that played the voice's last chord unless it lies clearly on the other side of the split point; a wider chord is cut at the gap that best separates two hands. The split point starts at middle C, follows those gaps and drifts back, staying between F3 and G4. Rests left by the other hand merge into the fewest rests on their own beats; a hand with nothing in a bar gets one whole-bar rest. Chord names, beat text, dynamics and lyrics go on the top staff only.

## Layouts (VIEW-3)

`Options::layout` picks one of four staff layouts; the app offers them under View → Layout.

| Layout | Line width | Sheets | Header | Guitar Pro 8 name |
| --- | --- | --- | --- | --- |
| **Page** | the paper's, less margins | one per page, page numbers, a new page when the next system does not fit | yes | Page – Vertical |
| **Vertical** | the window's (`Options::screen_width`), less margins | one long sheet | yes | Screen – Vertical |
| **Horizontal** | none: every bar on one line at natural spacing | one sheet as wide as the music | no | Screen – Horizontal |
| **Parchment** | the paper's, less margins | one long sheet, no page breaks | yes | Page – Scroll |

- Only the Page layout shrinks the staff when a system is taller than a page; the others have no page to fit.
- In the Vertical layout the app passes the window's width divided by the score zoom, so zooming in puts fewer bars on a line, as in Guitar Pro's screen modes.
- PDF export always uses the Page layout, on A4 or Letter (`Paper`, EXP-1).

**Checked against Guitar Pro (16 Oct 2026).** The four names come from Guitar Pro 6 and 7 and from Mateus's description. Guitar Pro 8's user guide ("Page and Screen Display Modes") lists six modes: Page – Vertical (pages stacked, as printed), Page – Horizontal (pages side by side), Page – Grid (as many pages across as the window holds), Page – Scroll (page breaks removed, a continuous view), Screen – Vertical (bars arranged in lines that use the window's width) and Screen – Horizontal (one line). Guitar Pro 6's guide describes Parchment as scrolling through the file without turning pages. So Parchment is Guitar Pro 8's Page – Scroll: paper-width lines with no page breaks, which is what sets it apart from Vertical (window-width lines). Page – Horizontal and Page – Grid only arrange the same pages differently on screen; they are not in the brief and are left for later, if wanted. Guitar Pro 8's guide does not say whether screen modes show the score header; we show it in Vertical and leave it out of the single line of Horizontal.

## Sizes

- **Staff space:** 4.5 pt by default, a 6.4 mm staff. A4 with 15 mm margins.
- **Tablature:** lines 1.5 staff spaces apart. Fret numbers are 1.2 times that gap, with the line knocked out behind them.
- **Rhythm on tab:** stems end 3 staff spaces beyond the tab staff (above it for voices 1 and 3, below for 2 and 4).
- **Grace notes:** 65 % of a normal note.
- **Text:** characters Noto Sans lacks (♯ ♭ ♮) come from Bravura, raised to sit with the letters.
- **Chord diagrams in the header:** 4.2 pt between strings, 5.2 pt between frets, four frets shown.

## Checking

- **Tests:** `cargo test --release -p oxiguitar-layout --test engrave` engraves the public fixtures, damaged scores, a 300-bar 8-track score and the same bars in all four layouts. Where `corpus/files/` exists, it also engraves every Guitar Pro file in the corpus with all its tracks, in every layout. Each run checks that every item lands on the page or sheet, every glyph exists in its font and output is deterministic.
- **By eye:** `oxiguitar-cli render FILE --png out/page --pdf out/score.pdf [--layout page|vertical|horizontal|parchment] [--width PT] [--paper a4|letter] [--tracks all] [--dark] [--no-dynamics] [--tool ID]`. Long sheets are cut into PNGs of at most 8192 pixels.

## Not drawn yet

These are the M1 engraving items still to do (see ROADMAP.md):

- **Notation marks not in the score model yet:** slurs between different notes (other than hammer-ons, pull-offs and legato slides), hairpins, ornaments, fermatas and ottava lines (NOT-3). The readers log them as not carried over.
- **Effects:** whammy-bar curves (shown as "w/bar" only), fingering numbers, and articulations on tab-only tracks.
- **Overlap pass, second round:** bend amounts of quick successive bends can touch, and dense chords can crowd ledger lines (VIEW-7). Tuplet numbers, articulations and dynamics now reserve their room (dynamics like chord names: they stay under their beat and the music opens up).
- **Nested tuplets:** only the outermost tuplet of a note is marked (TAB-2, M4).
- **Courtesy signatures:** key and time signatures at the end of a system before a change.
- **Grand staff details:** voices are split one by one, so a part written as one voice with both hands gets each hand's stems from that voice's rule; cross-staff beams and clef changes within a staff are not drawn.
