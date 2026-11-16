# Feature parity: Guitar Pro 8, TuxGuitar and alphaTab

The parity check ROADMAP.md's "The way to v1.0" calls for before the beta ("Then we stop"). Made on 16 Nov 2026, against OxiGuitar 0.20.0 with its official plugins. Each program is read from its own documentation or code:

- **Guitar Pro 8** (Arobas Music, proprietary): its user guide's table of contents and overview at guitar-pro.com/docs/gp8, and its palette as `docs/design/ui.md` and `docs/design/palette.md` list it.
- **TuxGuitar 2.1.1** (LGPL-2.1, Java; the latest tag of github.com/helge17/tuxguitar): its modules, dialogs and song model in the source (`common/`, `desktop/`, `TGNoteEffect`, `TGBeat`, `TGMeasureHeader`), and its Flathub description.
- **alphaTab 1.8** (MPL-2.0, TypeScript, C# and Kotlin): its introduction and release notes at alphatab.net. alphaTab is a library that renders and plays scores inside other programs. It has no editor of its own (its authoring route is alphaTex, a text format), so its editing rows say "n/a".
- **OxiGuitar**: the manual (`docs/manual/src/`), the design documents and the code.

**Yes**: there and working. **Partial**: there, with the limit given. **No**: missing (for the other programs: not in their guide, model or code). **?**: not established from those sources. **Out**: missing by Mateus's decision. **Plugin**: OxiGuitar's, in an official plugin.

## In short

- **Notation and the palette match Guitar Pro 8's tablature notation**: every one of its 124 palette tools works (v0.17), the wah through its plugin. TuxGuitar's model has far fewer marks (no ornaments, fermatas, ottavas, hairpins, pedal, rasgueado, golpe, barre, slurs apart from hammer-ons, fingering; two voices; one staff a track). alphaTab draws nearly everything Guitar Pro writes.
- **OxiGuitar is ahead** in amp modelling with users' own NAM captures, SFZ instruments, Harmonize, key inference, the `.oxg` sidecar (nothing lost in any format), reading and writing MusicXML, MIDI files and TuxGuitar files, plugins anyone can write, and being free and offline.
- **The largest gaps** are in editing and practice:
  - drum entry (both editors have it; OxiGuitar's manual says "entering them comes later");
  - the mixer not saved into the file;
  - volume, pan and instrument automations;
  - instrument views (fretboard, keyboard);
  - image export from the window;
  - a speed trainer.
- **Formats**: Guitar Pro 8 and TuxGuitar also import PowerTab and TablEdit, and write ASCII tab; TuxGuitar writes LilyPond and Guitar Pro 3 and 4 files; alphaTab reads CapXML and alphaTex. MuseScore and ABC are out (9 Nov 2026), and the formats #223 adds (PowerTab, Capella, alphaTex) wait until after v1.0 (16 Nov 2026).
- **Not before v1.0**: audio tracks and line-in (M9 in ROADMAP.md; #240), Guitar Pro's song store. **Out by decision**: the automation and stem tools of Guitar Pro's palette (19 Oct 2026).

"Gaps worth deciding before the beta", at the end, ranks the gaps for Mateus.

## Notation and engraving

| | Guitar Pro 8 | TuxGuitar 2.1 | alphaTab 1.8 | OxiGuitar 0.20 |
| --- | --- | --- | --- | --- |
| Standard notation and tablature, together or apart | Yes | Yes | Yes | Yes |
| Rhythm drawn around tablature-only staves | Yes | Yes | Yes | Yes |
| Drum staff | Yes | Yes | Yes (and drum tabs) | Yes (General MIDI kit) |
| Slash notation | Yes (staff and beats) | No | Yes (staff and beats) | Partial: slash beats only, no slash staff |
| Numbered notation (jianpu) | Yes | No | Yes | No |
| Several staves a track (grand staff) | Yes | No | Yes | Yes (keyboards get a grand staff) |
| Voices a bar | 4 | 2 | Several | 4 |
| Tuplets, nested tuplets | Yes, yes | Yes, no | Yes, ? | Yes, yes |
| Clefs, key and time signatures, changes mid-score | Yes | Yes | Yes | Yes, with courtesy signatures |
| Repeats, alternate endings | Yes | Yes | Yes | Yes |
| Directions (segno, coda, D.S., fine) | Yes | No (markers only) | Yes | Yes |
| Sections | Yes | Markers | Yes | Yes |
| Free time bars | Yes | Partial (its free edition mode allows bars that don't add up) | Yes | Yes |
| Fermatas, hairpins, ottavas | Yes | No | Yes | Yes |
| Ornaments (mordents, turns) | Yes | No | Yes | Yes |
| Slurs (legato over several beats) | Yes | No | Yes | Yes |
| Guitar effects (bends, slides, harmonics, vibrato, palm mute, let ring, dead and ghost notes, tapping, slap and pop, trills, tremolo picking, grace notes, strokes) | Yes | Yes (one kind of slide) | Yes | Yes |
| Whammy bar | Yes | Yes | Yes | Yes |
| Rasgueado, golpe, pick scrape, dead slap | Yes | No | Partial (golpe drawn only; pick scrape ?) | Yes |
| Sustain pedal, wah | Yes | No | Drawn only | Yes (wah: Plugin) |
| Left- and right-hand fingering, string numbers | Yes | No | Yes | Yes |
| Extended barre | Yes | No | Yes | Yes |
| Simile (repeat-bar) signs | Yes | No | ? | Yes |
| Accidentals as written (enharmonic spelling) | Yes | Yes | ? | Yes (a file's own kept) |
| Beaming overrides | Yes | No | Yes | Yes |
| Stem direction overrides | Yes | No | ? | Out (stems go by voice, 19 Oct 2026) |
| Tenuto | ? | No | Yes | No |
| Chord names and diagrams | Yes | Yes | Yes | Yes |
| Lyrics, verses | Yes (5) | Yes (1) | Yes | Yes (5) |
| Dynamics | Yes | Yes | Yes | Yes (hidden by default) |
| Multi-bar rests | Yes | No | Yes | No |
| Bars per system (design mode) | Yes, with bar widths and beat positions | Line breaks | Yes (from the file) | Partial: bars per system only |
| Stylesheet (fonts, spacing, header and footer, page format) | Yes | Partial (style settings) | Partial (formatting templates) | No: paper size only |
| Colours for single notation elements | ? | No | Yes | No |
| Engraving rules (Behind Bars), errors flagged | Yes | Partial (bars that don't add up shown) | Partial (traditional engraving rules) | Yes (bars that don't add up are red) |

## Editing

| | Guitar Pro 8 | TuxGuitar 2.1 | alphaTab 1.8 | OxiGuitar 0.20 |
| --- | --- | --- | --- | --- |
| Note entry from the keyboard on the tablature | Yes | Yes | n/a | Yes, Guitar Pro 8's keys |
| Note entry on the standard staff | Yes | Yes | n/a | Yes |
| **Drum entry** | Yes | Yes (matrix editor and tab) | n/a | **No** ("entering them comes later") |
| Entry from a MIDI instrument | Yes (MIDI capture) | ? | n/a | No |
| Bend and whammy editors | Yes | Yes | n/a | Yes |
| Copy, cut, paste; special paste | Yes | Yes | n/a | Yes |
| Undo, redo | Yes | Yes | n/a | Yes |
| Tracks: add, duplicate, move, delete | Yes | Yes | n/a | Yes |
| Tunings, capo, 3 to 10 strings | Yes | Yes | n/a | Yes |
| Transpose (semitones, keeping the key) | Yes | Yes | n/a | Yes |
| Automatic fingering | Yes ("Automatic Finger Positioning") | ? | n/a | Yes (Find Fingering) |
| Re-barring to the time signature | Yes ("Bar Arranger") | ? | n/a | Yes (Fit Bars: re-bar or stretch) |
| Bar check (bars that don't add up) | Yes | Partial (shown) | n/a | Yes (F4) |
| Chord builder with voicings | Yes | Yes | n/a | Yes, with chord styles |
| Harmonize (chords found for a melody) | No | No | n/a | Yes |
| Key signatures inferred from the notes | No | No | n/a | Yes |
| Multivoice editing | Yes | ? | n/a | Yes |
| Command palette, search | Yes (every command) | No | n/a | Partial: the palette's tools only |
| Customisable shortcuts | ? | Yes | n/a | No |
| Song information | Yes | Yes | n/a | Yes |
| Scales tool and scale diagrams | Yes | Yes (scale finder) | n/a | No |
| Polyphonic tuner | Yes | Yes (plugin) | n/a | No (needs audio input: M9) |
| Plugins that add tools, tabs, drawings, sounds | No | Yes (Java) | n/a | Yes (interface 1.8, any language with a C ABI) |

## Playback and sound

| | Guitar Pro 8 | TuxGuitar 2.1 | alphaTab 1.8 | OxiGuitar 0.20 |
| --- | --- | --- | --- | --- |
| Playback with a cursor that follows | Yes | Yes | Yes | Yes |
| Every written effect heard | Yes | Yes, of its own marks | Mostly (sustain pedal, golpe and wah drawn only) | Yes (wah: Plugin; pick scrape shown only, as in Guitar Pro) |
| Sound engine | RSE: over 200 sound banks, modelled amps and pedals | MIDI: Gervill, FluidSynth, JACK, LV2 and Audio Unit hosts | SoundFont 2 (TinySoundFont) | SoundFont 2 and 3, DLS, SFZ sampler (Plugin) |
| Amp modelling | Yes (its own amps and pedalboard) | No | No | Yes: users' NAM captures and cabinet responses (Plugin) |
| Equalizer a track | Yes | No | No | Yes (Plugin) |
| Plays out of the box with the system's sounds | Yes (its own) | Yes (bundled set) | Needs a SoundFont | Yes (the system's set; none bundled, by decision) |
| Mixer: mute, solo, volume, pan | Yes | Yes | Yes | Yes |
| **Mixer saved in the file** | Yes | Yes | n/a | **No** (lasts until OxiGuitar closes) |
| **Volume, pan and tempo automations** | Yes | Tempo only | ? | **Partial**: tempo changes only (Guitar Pro's automation tools left out, 19 Oct 2026) |
| **Instrument changes mid-song** | Yes | No | ? | **No** |
| Loop, count-in, metronome | Yes | Yes | Yes (API) | Yes |
| Visual metronome | Yes | ? | ? | Yes |
| Speed without changing pitch | Yes | Yes | Yes | Yes (25 % to 200 %) |
| **Speed trainer** (speed raised a step each loop) | Yes | ? | No | **No** |
| Play the chord diagrams | ? | No | No | Yes |
| Audio track to play along | Yes | No | Yes (backing tracks, external media) | No (M9; #240) |
| Line-in through amps and pedals | Yes | No | No | No (M9) |
| MIDI output to external devices | Yes | Yes | No | No (M9) |

## Files

| | Guitar Pro 8 | TuxGuitar 2.1 | alphaTab 1.8 | OxiGuitar 0.20 |
| --- | --- | --- | --- | --- |
| Guitar Pro 3, 4 | Read | Read, write | Read | Read (Plugin) |
| Guitar Pro 5 | Read | Read, write | Read | Read, write (Plugin) |
| Guitar Pro 6 (.gpx) | Read | Read | Read | Read (Plugin) |
| Guitar Pro 7, 8 (.gp) | Read, write | Read | Read, write | Read, write (Plugin) |
| MusicXML | Read, write | Write | Read | Read, write (Plugin) |
| MIDI files | Read, write | Read, write | Write | Read, write (Plugin) |
| TuxGuitar (.tg) | No | Read, write | No | Read, write: 2.x (Plugin) |
| PowerTab | Read | Read | No | No (#223, after v1.0) |
| TablEdit | Read | Read | No | No |
| ASCII tab | Read, write | Write | No | No |
| CapXML (Capella) | No | No | Read | No (#223, after v1.0) |
| alphaTex | No | No | Read, write | No (#223, after v1.0) |
| LilyPond | No | Write | No | No |
| MuseScore, ABC | No | No | No | Out (9 Nov 2026) |
| PDF | Write | Write | No | Write |
| **PNG, SVG** | Write | Write | Yes (headless rendering) | **Partial**: PNG from `oxiguitar-cli render` only |
| Audio | Write | Write | Yes (audio export API) | Write: WAV (Plugin) |
| Printing | Yes | Yes | n/a | Through the PDF |
| Batch conversion | Yes | Yes (converter) | n/a | `oxiguitar-cli convert` |
| What a format can't hold kept beside it | No | No | n/a | Yes (`.oxg`) |

## Display and the window

| | Guitar Pro 8 | TuxGuitar 2.1 | alphaTab 1.8 | OxiGuitar 0.20 |
| --- | --- | --- | --- | --- |
| Page and screen layouts, vertical and horizontal | Yes | Yes | Yes | Yes (Page, Vertical, Horizontal, Parchment) |
| Zoom | Yes | Yes | Yes | Yes (score and interface) |
| Tracks shown or hidden, several in one score | Yes | Yes | Yes | Yes |
| **Instrument views** (fretboard, keyboard, drums) | Yes | Yes (fretboard, piano, matrix) | No | **No** |
| Panels, palette, inspector | Yes | Yes | n/a | Yes |
| Platforms | Windows, macOS | Windows, macOS, Linux, FreeBSD, Android | Web, .NET, Android | Linux (Wayland, X11), Windows, macOS; mobile is M10 |
| Packages | Installers | Installers, Flathub | Libraries | AppImage, apt, dnf, Flatpak (our own repository), installers |
| Free and open source | No | Yes | Yes | Yes |

## Gaps worth deciding before the beta

Ranked by how often a user switching from Guitar Pro 8 or TuxGuitar would miss each one. Each was a question for Mateus: before the beta, after v1.0, or never.

**Decided (Mateus, 17 Nov 2026): gaps 1 and 2 go into v1.0,, the mixer built first; gaps 3 to 11 stay out of v1.0.** Notes on the ones he gave a reason or a hint for: volume, pan and instrument automations (3) may come back later, perhaps as a plugin; the stylesheet (7) is out; multi-bar rests (8) are rarely needed, as ties and the present rests do. The list below is kept as it was ranked.

1. **Drum entry.** In v1.0 (17 Nov 2026). Both editors have it, and drum tracks are in most files people open: today they can be played, transposed around and copied as rhythm, but not written. Guitar Pro 8's way is a drum staff with a kit map, and TuxGuitar's is a matrix editor. Size: an editor slice (the model already holds drum notes as General MIDI pitches).
2. **The mixer saved into the file.** In v1.0 (17 Nov 2026). Guitar Pro and TuxGuitar keep each track's volume and pan. OxiGuitar reads them from the file, then forgets the user's changes at closing. Size: small (the model holds them; the formats that carry them, Guitar Pro's among them, would write the user's).
3. **Volume, pan and instrument automations.** Out of v1.0 (17 Nov 2026; maybe a plugin later). Guitar Pro files carry them, and OxiGuitar's model holds only tempo changes, so the rest is lost when such a file opens and the song plays differently from Guitar Pro. Size: the model, the timeline, the Guitar Pro plugin. The palette's automation tools were left out on 19 Oct 2026, so this asks to reopen that.
4. **Image export from the window** (PNG, SVG). Out of v1.0 (17 Nov 2026). Both editors have it, and the renderer already writes PNG for the CLI. Size: small, as an exporter plugin or in File → Export.
5. **A speed trainer** (out of v1.0, 17 Nov 2026): loop and raise the speed a step each time. Guitar Pro 8 has it, and OxiGuitar has the loop and speed already. Size: small.
6. **Instrument views** (a fretboard and a keyboard showing what plays). Out of v1.0 (17 Nov 2026). Both editors have them, and they matter for learning. Size: a panel tab, which a plugin could add since interface 1.8.
7. **Stylesheet** (out of v1.0, 17 Nov 2026): fonts, spacing, header and footer, page format. Guitar Pro 8 has it for publishing. Size: medium.
8. **Multi-bar rests**, for parts that rest for long. Out of v1.0 (17 Nov 2026): rarely needed, ties and the present rests do. Guitar Pro and alphaTab draw them. Size: engraving only.
9. **Numbered notation**, slash staves, tenuto: rare among guitarists. Out of v1.0 (17 Nov 2026).
10. **Formats**: PowerTab, TablEdit and ASCII tab reading (both editors), ASCII and LilyPond writing, Guitar Pro 3 and 4 writing. After v1.0 (#223), as Mateus said on 16 Nov 2026, and out of v1.0 (17 Nov 2026).
11. **Scales tool, tuner, MIDI devices, customisable shortcuts.** The tuner and MIDI devices are M9 (they need audio and MIDI input). The others are small. Out of v1.0 (17 Nov 2026).

Not counted as gaps: audio tracks and line-in (M9, #240), Guitar Pro's song store, MuseScore and ABC (out), and the Flatpak repository, which is live since 16 Nov 2026 and fills as each plugin's release with a Flatpak extension is published (#163).
