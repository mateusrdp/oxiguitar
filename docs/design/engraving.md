# Engraving

How `oxiguitar-layout` turns a score into pages, and what it does not draw yet. Requirement IDs refer to the project brief.

## Pipeline

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
   - With several tracks, each track's name stands left of its staves: the full name on the first system, the short name after, shortened with "…" past a fifth of the line.
6. **Pages** (`page.rs`). The header (title, subtitle, artist, album, credits on the right, tuning and capo on the left, copyright at the foot) goes on page one (VIEW-6). Systems stack down the page, and a new page starts when the next system does not fit. If a system is taller than a page, the whole score is engraved again with a smaller staff.

Every symbol is a SMuFL glyph placed by the font's metadata: stem anchors, bounding boxes, engraving defaults (VIEW-7). The output is a display list per page, so the screen, PNG and PDF show exactly the same thing.

## Sizes

- **Staff space:** 4.5 pt by default, a 6.4 mm staff. A4 with 15 mm margins.
- **Tablature:** lines 1.5 staff spaces apart. Fret numbers are 1.2 times that gap, with the line knocked out behind them.
- **Rhythm on tab:** stems end 3 staff spaces beyond the tab staff (above it for voices 1 and 3, below for 2 and 4).

## Checking

- **Tests:** `cargo test --release -p oxiguitar-layout --test engrave` engraves the public fixtures, damaged scores and a 300-bar 8-track score. Where `corpus/files/` exists, it also engraves every Guitar Pro file in the corpus with all its tracks. Each run checks that every item lands on the page, every glyph exists in its font and output is deterministic.
- **By eye:** `oxiguitar-cli render FILE --png out/page --pdf out/score.pdf [--tracks all] [--dark]`.

## Not drawn yet

These are the M1 engraving items still to do (see ROADMAP.md):

- **Effects:** TAB-1 effects: bends, slides, hammer-ons and pull-offs, palm mute, let ring, vibrato, harmonics, trills, tremolo picking, strokes. Also grace notes.
- **Notation marks:** slurs, dynamics, articulations, ornaments, fermatas, ottavas (NOT-3), and lyrics.
- **Score header:** chord diagrams in the header (VIEW-6).
- **Other layouts:** Vertical, Horizontal and Parchment (VIEW-3).
- **Overlap pass:** tuplet numbers under down-stem beams can touch the tab staff, and dense chords can crowd ledger lines (VIEW-7).
- **Text fallback:** characters missing from Noto Sans (♯ ♭ in chord names, non-Latin scripts) should fall back to another font.
- **Courtesy signatures:** key and time signatures at the end of a system before a change.
