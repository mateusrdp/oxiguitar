# Guitar Pro 6, 7 and 8 (.gpx, .gp)

Both formats hold the same XML score, **GPIF** (`score.gpif`), in different containers. Readers: `crates/oxiguitar-formats/src/container.rs` (containers), `src/gpif/` (the score), `src/xml.rs` (the XML tree).

Sources: alphaTab (MPL-2.0) `GpxFileSystem.ts`, `GpifParser.ts`, `PartConfiguration.ts`; our corpus of 3 GPX and 12 GP files (versions 7, 7.6.0 and one 8.1.4), studied on 16 October 2026. No code was copied. alphaTab 1.8.4 reads every corpus file into the same notes as our reader, bar for bar (11,739 bars; checked 16 October 2026 with a throwaway script, see "Checking" below).

## Containers

### .gp (Guitar Pro 7 and 8)

A ZIP archive. Entries seen in the corpus:

| Entry | Use |
| --- | --- |
| `VERSION` | `7.0`, even for files saved by Guitar Pro 8 |
| `Content/score.gpif` | The score (XML) |
| `Content/PartConfiguration` | Which notation each track shows (below) |
| `Content/LayoutConfiguration` | Layout options; not read yet |
| `Content/BinaryStylesheet` | Engraving style; not read yet |
| `Content/Preferences.json` | Practice settings (speed trainer, view); ignored |
| `Content/ScoreViews/*.gpsv` | Guitar Pro 8 only: saved views; ignored |

All entries are deflated. We read them with the `zip` crate (no default features; pure-Rust zlib-rs inflate), refuse any entry that inflates beyond 128 MiB, and never look at entry names other than these.

### .gpx (Guitar Pro 6)

**BCFZ:** `BCFZ`, the uncompressed length as a little-endian int, then a bit stream. Bits are read from the most significant bit of each byte down. Repeat until the output reaches the promised length:

- Flag bit **1**: a 4-bit word size *n* (bits read high to low), then an *n*-bit offset and an *n*-bit length, both read **low to high**. Copy `min(offset, length)` bytes starting `offset` bytes back in the output.
- Flag bit **0**: a 2-bit count (low to high), then that many literal bytes (each 8 bits, high to low).

One corpus file (ghost-year-zero.gpx) ends one byte short of its promised length; the reader stops at the end of the input and keeps what it has. A back-reference before the start of the output is an error.

**BCFS:** the decompressed data (or an uncompressed file) is `BCFS` followed by 4 KiB sectors. A sector whose first int is **2** describes a file: name (NUL-terminated, at offset 4, up to 127 bytes), size (int at 0x8C), then a list of sector numbers (ints from 0x94) ending with 0. The file's contents are those sectors joined and cut to the size. Files seen: `score.gpif`, `misc.xml`, `BinaryStylesheet`, `PartConfiguration`, `LayoutConfiguration`, and an empty one with a one-letter name.

### PartConfiguration

Big-endian. An int count of *views*; per view a byte (multi-rest), an int count of tracks and one flag byte per track: **1** standard notation, **2** tablature, **4** slash, **8** numbered. The file ends with an int (the selected view). The first view is the full score with one flag per track, in master-track order; the others are the single-track parts. We use the first view; a flag of 0 means standard notation. Slash and numbered views are shown as standard notation until M4.

## GPIF

The root `<GPIF>` holds flat sections that refer to each other by id:

```text
MasterBar.Bars ──► Bar (one per staff, track by track in master-track order)
Bar.Voices     ──► Voice (4 ids, -1 for none)
Voice.Beats    ──► Beat (ids may repeat: two voices can share a rest)
Beat.Notes     ──► Note
Beat.Rhythm    ──► Rhythm
```

The reader builds a fresh model object at every reference. A reference to a missing id is logged and left out, never an error.

### Header and master track

| Element | Model |
| --- | --- |
| `Score/Title`, `SubTitle`, `Artist`, `Album`, `Words`, `Music`, `Copyright`, `Tabber`, `Instructions` | `ScoreInfo` (Tabber → transcriber) |
| `Score/WordsAndMusic` | Fills whichever of words and music is empty |
| `Score/Notices` | One notice per line |
| `MasterTrack/Tracks` | Track order |
| `MasterTrack/Automations/Automation` (Type `Tempo`) | `Value` is "number unit": unit 1 eighth, 2 quarter, 3 dotted quarter, 4 half, 5 dotted half; we convert to quarter notes. `Bar` (0-based) and `Position` (fraction of the bar) place it; the one at bar 0, position 0 is the initial tempo, with `Text` as its label. `Linear` = gradual. |

### Tracks

| Element | Model |
| --- | --- |
| `Name`, `ShortName`, `Color` ("r g b") | Track |
| GP7/8 `InstrumentSet/Type` = `drumKit`; GP6 `Instrument ref="drmkt…"` | Percussion |
| GP7/8 `Sounds/Sound/MIDI/Program`; GP6 `GeneralMidi/Program` | MIDI program (first sound) |
| `RSE/ChannelStrip/Parameters` | 16 numbers 0–1; number 11 is pan (0.5 centre), 12 volume (read in double precision, so every volume and pan the writer writes reads back exactly) |
| `PlaybackState` | `Mute`, `Solo` |
| GP7/8 `Transpose/Chromatic`, `Octave`; GP6 `PartSounding/TranspositionPitch` | Sounding = written + this. An octave down (guitar, bass) becomes the 8vb clef; the rest becomes `Staff::transposition` with the sign turned round. |
| GP7/8 `Staves/Staff/Properties`; GP6 `Track/Properties` | Staff properties (below) |
| `Lyrics/Line/Text`, `Offset` | One lyric line per non-empty line; offset = first bar (0-based) |
| `Automations/Automation` (Type `Sound`) after bar 0 | Sound changes: logged, not supported yet |

Staff properties (`<Property name="…">`):

| Property | Model |
| --- | --- |
| `Tuning/Pitches` | MIDI pitches, **lowest string first**; drum kits store zeros, which we treat as no strings |
| `CapoFret/Fret` | Capo |
| `FretCount/Number` | Fret count |
| `PartialCapoFret` | Logged, not supported yet |
| `DiagramCollection/Items/Item` | Chord diagrams: `Diagram` has `stringCount` and `baseFret`; each `Fret` has `string` (0 = lowest) and `fret` relative to the base fret, except that `fret="0"` is an open string wherever the diagram starts (an E chord at the seventh fret: base 6, the low E at fret 0). The first fret shown is `baseFret + 1`. A diagram with `<Property name="ShowDiagram" value="false">` is a chord name only. Beside each `Diagram`, Guitar Pro 7 keeps a `Chord`: `KeyNote` and `BassNote` (`step`, `accidental`) and a `Degree` per interval (`interval` Second … Thirteenth, `alteration` Major, Minor, Perfect, Diminished, Augmented, `omitted`); the reader takes the name and leaves the spelling. |

### Master bars

`Time` ("n/d"), `Key/AccidentalCount` and `Mode`, `Repeat` (`start`, `end`, `count` = times played), `AlternateEndings` ("1 2"), `Section/Letter` and `Text` (marker; the letter is new in the model), `DoubleBar`, `TripletFeel`, `Directions/Target` and `Jump` (names in the reader's `direction()`; note `DaCoda` means "to coda"). `FreeTime` (an empty element): the bar is in free time, so it need not add up to its time signature (score v2, interface 1.2). `Fermatas` are logged.

### Bars, voices, beats, notes

**Bar:** `Clef` is `G2`, `F4`, `C3`, `C4` or `Neutral`. With the track an octave down, `G2` is our treble 8vb and `F4` our bass 8vb. `Ottavia` on a bar is logged.

**Rhythm:** `NoteValue` (`DoubleWhole`, `Whole`, `Half`, `Quarter`, `Eighth`, `16th` … `256th`; `Long` is read as a double whole), `AugmentationDot count`, `PrimaryTuplet num/den` and `SecondaryTuplet` (nested).

**Beat:** `GraceNotes` (`BeforeBeat`, `OnBeat`), `Dynamic` (PPP … FFF, copied to each note), `FreeText`, `Chord` (diagram id), `Fadding` (`FadeIn`), `Tremolo` (`1/2`, `1/4`, `1/8` = eighths, sixteenths, thirty-seconds), `Whammy` (GP7/8, attributes) or `WhammyBar…` properties (GP6), `Brush` (strum; its length in ticks is XProperty 687935489, default 120), `PickStroke`, `Slapped`, `Popped`. Logged as not yet supported: arpeggio, hairpin, ottava, wah, golpe, rasgueado, barre, slurs between beats, ornaments. A beat with no notes is a rest.

**Note:** properties `String` (**0 = lowest**) and `Fret`; `Midi`; GP6 pitched instruments use `Octave` and `Tone` (MIDI = octave × 12 + tone); GP6 drums use `Element` and `Variation` (mapped to General MIDI, below). Effects: `HopoOrigin` (hammer-on or pull-off to the next note), `Slide/Flags` (1 shift, 2 legato, 4 out down, 8 out up, 16 in from below, 32 in from above, 64/128 pick scrape), `Bended` with `Bend{Origin,Middle,Destination}Value` and offsets (values: 100 = a whole tone; offsets in percent), `HarmonicType` and `HarmonicFret`, `PalmMuted`, `Muted` (dead note), `Tapped` (tapping on the beat), `LeftHandTapped`. Elements: `Tie destination="true"`, `Vibrato`, `LetRing`, `AntiAccent` (ghost), `Accent` flags (1 staccato, 4 heavy, 8 accent; 2 and 16 logged), `Trill` (the other note's MIDI pitch), `LeftFingering`/`RightFingering` (P I M A C).

Things the files do that we correct:

- **Ties:** Guitar Pro 6 sometimes stores a stale fret on the second note of a tie (Ghost - Faith bar 18: fret 4 tied to "fret 6"). A tie cannot change pitch, so the tied note takes the fret of the note before it on the same string and voice; logged as a note.
- **Flat bends:** Guitar Pro 7 marks some tied notes as bent with a curve that stays at zero. That is no bend; dropped.
- **Over-full bars:** Guitar Pro allows a voice longer than its time signature (ghost-cirice.gp bars 19–25). We keep it as written; `oxiguitar-cli info` reports it (EDIT-9).

**Drums:** Guitar Pro 7/8 give each drum note a MIDI number; we keep drum notes as pitches (the GP3–5 reader now does the same with their fret). Guitar Pro 6 names drum sounds by element and variation; our General MIDI match is in `GP6_DRUMS` in `src/gpif/effects.rs`. Rim shots, chokes and the half-open hi-hat have no General MIDI note and use the nearest one.

## Writing .gp files

`src/gpif/write/` writes a score as a .gp file (Guitar Pro 7 and 8): the reader in reverse, in the shape Guitar Pro 7.6 saves (element order, the empty sections it always writes). The archive holds `VERSION` ("7.0"), `Content/score.gpif` and `Content/PartConfiguration` (one score view, one notation byte per track); Guitar Pro supplies its stylesheet and view settings itself. The header says `<GPVersion>7</GPVersion>`: no particular version, the first that reads it (Mateus, 19 Oct 2026).

What Guitar Pro needs beyond what the reader reads, learnt from Guitar Pro 7.6 (`fixtures/guitar-pro-7.6/README.md`):

- **A sound from Guitar Pro's library for every track:** the instrument set's name and type, the sound's path, role and patch, by General MIDI program, from [guitar-pro-sounds.md](guitar-pro-sounds.md) (`src/gpif/write/tables.rs`). Without one, Guitar Pro 7.6 crashed on a voice track. The sound's effect chain is left empty for Guitar Pro to fill.
- **Drum tracks carry Guitar Pro's drum kit:** its 54 drums and 95 articulations as Guitar Pro 7.6 writes them for a new drum track; each drum note names its articulation (`InstrumentArticulation`, counted through the kit) besides its `Midi` number. A General MIDI drum the kit lacks plays as a snare in Guitar Pro; the writer notes it.
- **What other readers need:** a note of a track without strings carries its pitch as an octave and a tone too (as Guitar Pro 6 wrote it), besides `Midi`, because alphaTab reads only those there. A bar whose voices hold no beats is written with no voice (`-1 -1 -1 -1`), as Guitar Pro 6 wrote it, because MuseScore 4 fills an empty voice with notes from another bar. MuseScore 4.7 crashes on a slide to the next note from a voice's last note; there is nothing to slide to, so the writer leaves such a slide out and names the loss.
- **A spelled chord beside every diagram** (`src/gpif/write/chord.rs`): the key note, bass note and intervals read from the chord's name (`C7M9`, `F#Ø`, `D/F#`, `Gsus4`, `Bb7(b9)`…), each interval marked omitted when the diagram doesn't sound it, as Guitar Pro does. A name it can't read is spelled as Guitar Pro spells a chord it knows nothing about: C, no intervals. Without a spelling Guitar Pro 7.6 drops the diagram.

What a .gp file can't hold, or holds one way only, the writer names as losses (FMT-3), each with the first bar and track where it happens: per-note dynamics within a beat (GPIF has one per beat), section colours, trills other than in sixteenths, bend and whammy curves of more points than an origin, a middle and a destination (after leaving out the points that change nothing: repeats, points inside a hold, a hold at zero before or after the bend), a diagram's fret below its first fret, a slide from a voice's last note with no note to slide to, more than five lines of lyrics, more than four voices or staves, staves of one track with different transpositions or notation, octave clefs that disagree with the track's transposition, and a few values outside what the reader takes. What changes and nobody sees is not a loss: an unused voice's placeholder beats, spaces around texts, the order of a note's slides, a drum track's strings.

## Checking

- `cargo test -p oxiguitar-formats --test gpif`: the test song in both dialects (public fixtures `fixtures/gp/oxiguitar-test-song.gp` and `.gpx`), compared with the GP5 reader; damaged and cut files.
- `cargo test --release -p oxiguitar-formats --test corpus -- --nocapture`: every corpus GPX/GP file, and six songs that exist in two formats (see the test for what differs between editions and why).
- `cargo test --release -p oxiguitar-formats --test round_trip -- --nocapture`: every fixture and corpus file written as .gp and read back. A .gp file comes back exactly, with no losses; any other file comes back the same in its normal form unless the writer named a loss; a second round trip is always exact.
- `cargo test -p oxiguitar-testgen --test guitar_pro_7_6`: our writer against Guitar Pro 7.6's own files (`fixtures/guitar-pro-7.6/`): the same song, and every element Guitar Pro wrote for it.
- The `gp_writer` fuzz target (`fuzz/`): any file a reader takes, written and read back twice.
- Guitar Pro itself: the test builds carry every reference song written as .gp (the artifact "guitar-pro-check"), which Mateus opens in Guitar Pro 7.6.
- Other programs (`.github/workflows/interop.yml`, on pull requests that touch the formats): the formats crate's `write_gp` example writes every fixture as .gp, with the notes our reader finds in each file; alphaTab must read every file beat by beat as we do (`scripts/interop/alphatab.mjs`, alphaTab pinned in `scripts/interop/package.json`), and MuseScore 4 must open every file and find the same tracks, bars and notes, bar by bar (`scripts/interop/musescore.py`, which says where MuseScore reads Guitar Pro its own way). Locally, over the corpus as well:

  ```sh
  cargo run --release -p oxiguitar-formats --example write_gp -- corpus/files/* out
  (cd scripts/interop && npm ci && node alphatab.mjs ../../corpus/files/* ../../out)
  QT_QPA_PLATFORM=xcb xvfb-run -a python3 scripts/interop/musescore.py MUSESCORE/AppRun out corpus/files/*
  ```

  `MUSESCORE` is MuseScore's AppImage unpacked with `--appimage-extract`. On 20 Oct 2026, alphaTab read all 104 corpus files as we do, and MuseScore 4.7.5 97 of them; it can't open the other 7 originals either.
