# Guitar Pro 3, 4 and 5 (.gp3, .gp4, .gp5)

Binary, little-endian, written sequentially with no index. Reader: `crates/oxiguitar-formats/src/gp/`.

Sources: PyGuitarPro (LGPL-3.0) `guitarpro/gp3.py`, `gp4.py`, `gp5.py`; alphaTab (MPL-2.0) `Gp3To5Importer.ts`; TuxGuitar (LGPL-2.1) `GP3InputStream`, `GP4InputStream`, `GP5InputStream`; guitarpro crate 0.4.3 (MIT). Checked against our corpus of 47 GP3, 15 GP4 and 27 GP5 files on 15 October 2026. No code was copied.

## Primitives

| Name | Bytes | Notes |
| --- | --- | --- |
| byte / signed byte | 1 | |
| short | 2 | |
| int | 4 | |
| double | 8 | GP5 note duration percentage only |
| *byte-size string (n)* | 1 + n | length byte, then a fixed field of n bytes |
| *int-size string* | 4 + len | lyrics only |
| *int-byte-size string* | 4 + 1 + (size − 1) | int = field size (length + 1), length byte, text; most strings |
| colour | 4 | r, g, b, unused |

Text is in the writer's Windows code page, in practice Windows-1252. We take valid UTF-8 as UTF-8 and everything else as Windows-1252.

## File layout

1. **Version:** byte-size string (30), e.g. `FICHIER GUITAR PRO v5.10`. Versions seen: 3.00, 4.00, 4.06, 5.00, 5.10. Detection reads only this (FMT-6); four corpus files named `.gp3` are 5.x.
2. **Score information:** title, subtitle, artist, album, then one author field (GP3/4) or words and music (GP5), copyright, tab author, instructions; an int count of notice lines and the lines.
3. GP3/4: **triplet feel** (bool) for the whole song. GP5 stores it per bar.
4. GP4/5: **lyrics:** int track number (1-based), then 5 × (int start bar, int-size string).
5. GP5.10 only: **RSE master effect:** int volume, int unknown, 11 equaliser bytes.
6. GP5: **page setup:** 7 ints (page size, margins, score size %), short header/footer flags, 10 int-byte-size strings (templates).
7. **Tempo, key:** GP3: int tempo, int key. GP4: int tempo, int key, signed-byte octave. GP5: int-byte-size tempo name, int tempo, bool tempo-hidden (5.10 only), signed-byte key, int octave.
8. **MIDI channels:** 64 × 12 bytes (int program, volume, balance, chorus, reverb, phaser, tremolo, 2 unused). Volume and balance run 0 to 16 (balance 8 is the middle): we read them as sixteenths of the full range, and the writer keeps sixteen steps. A track's flags short has 0x10 solo and 0x20 mute (GP5).
9. GP5: **directions:** 19 shorts, each a 1-based bar number or −1, in this order: coda, double coda, segno, segno segno, fine, da capo, d.c. al coda, d.c. al double coda, d.c. al fine, dal segno, d.s. al coda, d.s. al double coda, d.s. al fine, d.s.s., d.s.s. al coda, d.s.s. al double coda, d.s.s. al fine, to coda, to double coda. Then int master reverb.
10. int **bar count**, int **track count**.
11. **Bar headers** × bar count.
12. **Tracks** × track count. GP5 then skips 2 bytes (5.00) or 1 (5.10).
13. **Bars:** for each bar, for each track, one bar of beats.
14. GP3/4 files usually end with 4 zero bytes. Three corpus files carry extra data after the score; we log and ignore it.

## Bar header

GP5 starts every header after the first with one blank byte. Then a flag byte:

| Bit | Data (in this order) |
| --- | --- |
| 0x01 | numerator (byte) |
| 0x02 | denominator (byte) |
| 0x04 | repeat start (no data) |
| 0x08 | repeat end (signed byte): GP5 stores how many times the section plays; GP3/4 store how many times it repeats (one less) |
| 0x10 | GP3/4: alternate ending (byte) here — a count, see below |
| 0x20 | marker: int-byte-size string, colour |
| 0x40 | key: signed byte fifths, signed byte minor |
| 0x80 | double bar (no data) |

GP5 continues with: 4 beam-grouping bytes if 0x01 or 0x02 is set; **one alternate-endings byte, always present** (a bit set, meaningful only if 0x10 is set); one triplet-feel byte (0 none, 1 eighth, 2 sixteenth).

Time and key signatures carry over from the previous bar when absent.

GP3/4 alternate endings are a number *n*: the bar belongs to every ending from 1 to *n* not already used since the last repeat start (PyGuitarPro's rule).

**Sources disagree on GP5's alternate-endings byte.** alphaTab reads it after the beam bytes, as above. PyGuitarPro and the guitarpro crate read it before the marker, as in GP3/4, and skip a byte later only when 0x10 is clear. The two agree whenever a bar has an alternate ending but no marker or key change. All six GP5 corpus files with alternate endings read to their exact end with alphaTab's order.

## Track

GP5 starts the first track (and every track in 5.00) with a blank byte. Then: flags byte (0x01 drums); name, byte-size string (40); int string count (1–7); 7 ints tuning, highest string first, as MIDI numbers; int port; int channel; int effect channel; int fret count; int capo; colour.

GP5 adds: short display flags (0x01 tablature, 0x02 standard notation); byte auto-accentuation; byte MIDI bank; RSE block (byte humanise, 24 unknown bytes, RSE instrument: 3 ints plus a short and a blank byte in 5.00 or an int in 5.10; in 5.10 also 4 equaliser bytes and two int-byte-size strings).

The track's sound comes from MIDI channel `(port − 1) × 16 + (channel − 1)` of the table; channel 10 means drums.

## Bar of one track

GP3/4: one voice. GP5: two voices, then a line-break byte (missing at the very end of some files). A voice is an int beat count and the beats.

## Beat

Flag byte:

| Bit | Data |
| --- | --- |
| 0x40 | status byte first: 0x00 empty, 0x02 rest |
| — | duration (signed byte): −2 whole, −1 half, 0 quarter … 4 sixty-fourth |
| 0x01 | dotted (no data) |
| 0x20 | tuplet (int): 3 → 3:2; 5, 6, 7 → n:4; 9–13 → n:8 |
| 0x02 | chord diagram |
| 0x04 | text (int-byte-size string) |
| 0x08 | beat effects |
| 0x10 | mix-table change |
| — | string byte: bit (7 − s) set for each string s (1 = highest) with a note, then the notes from string 1 down |

GP5 ends each beat with a short of flags; if 0x0800 is set, one more byte (secondary beam break).

### Beat effects

GP3: one flag byte. 0x01 vibrato and 0x04/0x08 natural/artificial harmonic apply to every note of the beat; 0x02 wide vibrato; 0x10 fade in; 0x20 tap/slap/pop byte, then an int (whammy dip depth when the byte is 0); 0x40 stroke.

GP4/5: two flag bytes. 0x02 wide vibrato, 0x10 fade in, 0x20 tap/slap/pop byte, 0x40 stroke (two signed bytes, down then up in GP4, up then down in GP5; speed 1–6: 1 or less is a 128th, 2 a 64th, 3 a 32nd, 4 a 16th, 5 an 8th, 6 a quarter. alphaTab reads each one step shorter), second byte 0x04 whammy curve (bend format), 0x02 pick stroke (byte: 1 up, 2 down), 0x01 rasgueado (no data).

### Mix-table change

Signed bytes instrument (GP5: then an RSE instrument, and a blank byte in 5.00), volume, balance, chorus, reverb, phaser, tremolo; GP5: tempo name string; int tempo. Then a transition byte for each of the six values that is ≥ 0, and for the tempo if ≥ 0 (plus, in 5.10, a tempo-hidden bool). GP4/5: a flags byte. GP5: a wah byte; 5.10: two RSE strings. −1 means "unchanged". We keep tempo changes and log the others.

## Note

Flag byte: 0x20 note type byte (1 normal, 2 tie, 3 dead) **first**; GP3/4 0x01 two bytes of independent duration; 0x10 dynamic (signed byte, 1 ppp … 8 fff); 0x20 fret (signed byte); 0x80 left- and right-hand finger (signed bytes); GP5 0x01 duration percentage (double); GP5 a second flag byte (accidental spelling); 0x08 note effects. Bits 0x04 ghost and 0x40 accent carry no data; GP5 0x02 heavy accent.

A tied note's fret byte is meaningless: the fret comes from the previous note on the same string of the same track.

### Note effects

GP3: one flag byte: 0x01 bend, 0x02 hammer-on/pull-off, 0x04 slide, 0x08 let ring, 0x10 grace note.

GP4/5: two flag bytes, data in this order: bend (first byte 0x01), grace (0x10), tremolo picking (second byte 0x04: 1 eighth, 2 sixteenth, 3 thirty-second), slide (0x08: GP4 a signed type −2…4; GP5 a bit set), harmonic (0x10: GP4 a type byte; GP5 a type byte plus data for artificial (3 bytes) and tapped (1 byte)), trill (0x20: fret, period). No-data bits: first byte 0x02 hammer, 0x08 let ring; second byte 0x01 staccato, 0x02 palm mute, 0x40 vibrato.

**Bend:** signed byte type, int height, int point count, then per point int position (0–60 across the note), int height (50 per semitone), bool vibrato. We store offsets as exact fractions and heights in cents.

**Grace note:** GP3/4: fret, dynamic, duration (1 → 1/64, 2 → 1/32, 3 → 1/16), transition (0 none, 1 slide, 2 bend, 3 hammer). GP5: fret, dynamic, transition, duration, flags (0x01 dead, 0x02 on the beat). We turn grace notes into a grace beat placed before the main beat.

## Chord diagram

A bool says whether the "new" layout follows.

- **Old layout (GP3/4):** int-byte-size name, int first fret, and if the first fret is not 0, 6 ints of frets.
- **GP3 new layout:** bool sharp, 3 blank, ints root, type, extension, bass, tonality, bool add, byte-size string (22) name, 3 ints alterations, int first fret, 6 ints frets, int barre count, 2 × 3 ints barres, 7 bools omissions, 1 blank. (The guitarpro crate reads root, type and extension as bytes here; PyGuitarPro and our corpus say ints.)
- **GP4/5 new layout:** bool sharp, 3 blank, bytes root, type, extension, ints bass, tonality, bool add, byte-size string (22) name, 3 bytes alterations, int first fret, 7 ints frets, byte barre count, 3 × 5 barre bytes, 7 bools omissions, 1 blank, 7 signed bytes fingering, bool show. GP5 always uses this layout.

Frets: −1 not played, 0 open.

## Writing GP5 (M3 slice 7)

The writer (`gp/write.rs`) makes version 5.10 and holds what the reader reads. What it cannot hold is named as a loss (a warning, FMT-3); what it holds in another shape is named as information. A file read and written again is exactly the same score, and what it writes is read back the same by the reader, so the second trip is exact (`tests/gp5_round_trip.rs`).

- **Layout.** As above, in the order the reader takes: the title block, lyrics, RSE master, page setup, tempo, key, 64 channels, directions and their counts, the master bars (time signature, repeats, marker, key, beams, alternate ending, triplet feel), the tracks, then every bar's two voices. A voice with no beats is one empty quarter beat.
- **Strings and drums.** The model keeps no string for a drum, so the writer gives each drum its own string on the track and a tied drum the string where the same drum was last struck; the file ties a note to the one before it on its string. A tie that cannot be written that way is written as a new note and named.
- **Tracks without strings** (pianos and the like) are written as a guitar's tablature with notes placed by pitch, and named.
- **Tempo changes** are mix-table changes on the first track's beat at that position; a change not at the start of a beat is left out and named.
- **Text** is Windows-1252; other characters are written as ? and named. Text beyond 255 characters is cut and named.
- **Information, not loss:** dynamic markings are written as the notes' dynamics; tremolo picking and let ring on a beat are written on each note; short track names are left out; volume and pan are kept to sixteen steps.
