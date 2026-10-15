# 0002 — Score model, first version

- Status: accepted
- Date: 2026-10-15
- Milestone: M0

## Context

Every reader, writer, the engraver, the editor and the player share one score model (ARCHITECTURE.md). It has to hold everything in the TAB-1 core set now, grow to the Guitar Pro 8 set (TAB-2) in M4, and represent MuseScore and MusicXML files that carry standard notation only.

We compared the models of TuxGuitar (`TGSong > TGMeasureHeader / TGTrack > TGMeasure > TGBeat > TGVoice > TGNote`), alphaTab (`Score > MasterBar / Track > Staff > Bar > Voice > Beat > Note`) and Guitar Pro's own GPIF XML (`MasterBars`, `Tracks`, `Bars`, `Voices`, `Beats`, `Notes`, each referenced by id). MuseScore's `.mscx` is organised by staff and measure with chords and rests, which maps onto the same tree.

## Options

1. **Copy TuxGuitar's model.** Proven for the formats we read first, but it has one staff per track (no grand staff), puts voices inside beats, and has no room for nested tuplets.
2. **Copy GPIF's flat, id-referenced tables.** Closest to the .gp format we save by default, but awkward to edit and to walk in order.
3. **A tree like alphaTab's, written for Rust.** The same split between shared master bars and per-track bars as Guitar Pro and TuxGuitar, multiple staves per track, voices per bar, and exact time.

## Decision

Option 3. `oxiguitar-model` defines:

- `Score` with `ScoreInfo`, `MasterBar`s (time and key signature, repeats, alternate endings, markers, tempo changes, triplet feel, navigation signs) and `Track`s.
- `Track` with instrument, mixer, chord diagrams, lyrics and one or more `Staff`s. Tuning and capo live on the staff, as in GPIF, so a track can have a tab staff and a separate standard staff later.
- `Staff > Bar > Voice > Beat > Note`, with up to four voices per bar.
- **Exact time:** a `Duration` is a note value, dots and a list of tuplets (outermost first, so nested tuplets work); its length is a `Ratio` of a whole note, computed with integer arithmetic. Checked arithmetic is available for numbers that come straight from files.
- **Grace notes are beats** (`Beat::grace`) that take no time in the bar, as in GPIF and alphaTab; the GP3–5 readers turn note-level grace notes into such beats.
- **Notes are fretted or pitched** (`NotePlace`). Tablature files give string and fret; standard-notation files give a pitch until tab derivation (TAB-3) assigns a string. Frets count from the capo, as in Guitar Pro.
- **Bends** are curves of points with an exact offset (fraction of the note) and a height in cents, which covers Guitar Pro's quarter-tone steps and MuseScore's free curves.
- Effects sit on the note (`NoteEffects`) or the beat (`BeatEffects`) they act on.

`Score::check_structure` tests the main invariant: every staff has one bar per master bar.

## Consequences

- Readers for GP3–5, GPX and .gp map onto this almost one to one, and the alphaTab and TuxGuitar readers stay easy to follow while porting.
- Things not yet in the model (TAB-2 notation, stylesheets, per-note playback settings, MuseScore layout overrides) are added as optional fields when their milestone comes; until then readers record them in the import log (FMT-2).
- The model is plain data with public fields. Edits will go through command objects in `oxiguitar-edit` (M2), which is where invariants are enforced.
- Revisit if the M1 engraver or M2 editor needs a different shape, for example ids for cross-references such as ties and slurs spanning bars.
