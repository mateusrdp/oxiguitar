# Guitar Pro 7.6's sounds

Guitar Pro 7 and 8 give every track a sound from their own library, named by a path, and read nothing else to choose it: a track that names only a General MIDI program, as our test generator's files did, crashed Guitar Pro 7.6's sound engine when the program had no sound it could find (a voice, program 54; `fixtures/guitar-pro-7.6/README.md`). The .gp writer (M3 slice 2) therefore writes each track's instrument and sound as below.

Read on 19 Oct 2026 from files Mateus made in Guitar Pro 7.6.0: a track for every General MIDI program, and one for every instrument of Guitar Pro's instrument list. Each row is what Guitar Pro itself writes for that instrument: the instrument set's name and type (`<InstrumentSet>`), the sound's path (`<Sound><Path>`), the program (`<MIDI><Program>`) and the sound engine's patch (`<RSE><SoundbankPatch>`).

## By General MIDI program

What Guitar Pro 7.6 chose for a track set to each program. The writer uses this row for a track whose instrument is that program.

| Program | Instrument set | Type | Sound path | Patch |
| --- | --- | --- | --- | --- |
| 0 | Acoustic Piano | `acousticPiano` | Orchestra/Keyboard/Acoustic Piano | German-APiano |
| 1 | Acoustic Piano | `acousticPiano` | Orchestra/Keyboard/Acoustic Piano | German-APiano |
| 2 | Electric Piano | `electricPiano` | Orchestra/Keyboard/Acoustic Piano | German-APiano |
| 3 | Acoustic Piano | `acousticPiano` | Orchestra/Keyboard/Acoustic Piano | German-APiano |
| 4 | Electric Piano | `electricPiano` | Orchestra/Keyboard/Electric Piano | MarkI-EPiano |
| 5 | Electric Piano | `electricPiano` | Orchestra/Keyboard/Electric Piano | 200A-EPiano |
| 6 | Harpsichord | `harpsichord` | Orchestra/Keyboard/Clavinet | Harpsichord-Key |
| 7 | Harpsichord | `harpsichord` | Orchestra/Keyboard/Clavinet | Harpsichord-Key |
| 8 | Celesta | `celesta` | Orchestra/Other/Celesta | Celesta-Tuned |
| 9 | Vibraphone | `vibraphone` | Orchestra/Other/Celesta | Celesta-Tuned |
| 10 | Vibraphone | `vibraphone` | Orchestra/Other/Vibraphone | Vibraphone-Tuned |
| 11 | Vibraphone | `vibraphone` | Orchestra/Other/Vibraphone | Vibraphone-Tuned |
| 12 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 13 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 14 | Vibraphone | `vibraphone` | Orchestra/Other/Vibraphone | Vibraphone-Tuned |
| 15 | Banjo | `banjo` | Stringed/Other Stringed Instruments/Banjo | 5-Banjo |
| 16 | Electric Organ | `electricOrgan` | Orchestra/Keyboard/Organ | Jazz-Organ |
| 17 | Electric Organ | `electricOrgan` | Orchestra/Keyboard/Organ | Jazz-Organ |
| 18 | Electric Organ | `electricOrgan` | Orchestra/Keyboard/Organ | Jazz-Organ |
| 19 | Electric Organ | `electricOrgan` | Orchestra/Keyboard/Organ | Ballad-Organ |
| 20 | Electric Organ | `electricOrgan` | Orchestra/Winds/Other Winds | Oboe-Solo |
| 21 | Electric Organ | `electricOrgan` | Stringed/Acoustic Guitars/Steel Guitar | Accordion |
| 22 | Recorder | `recorder` | Orchestra/Winds/Harmonica | Harmonica |
| 23 | Electric Organ | `electricOrgan` | Stringed/Acoustic Guitars/Steel Guitar | Bandoneon |
| 24 | Nylon Guitar | `nylonGuitar` | Stringed/Acoustic Guitars/Nylon Guitar | Concerto-Nylon |
| 25 | Steel Guitar | `steelGuitar` | Stringed/Acoustic Guitars/Steel Guitar | D-Steel |
| 26 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Jazz Guitar | ES-Guitar |
| 27 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Clean Guitar | Strat-Guitar |
| 28 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Clean Guitar | Strat-Guitar |
| 29 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Overdrive Guitar | Strat-Guitar |
| 30 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Distortion Guitar | Classic-Guitar |
| 31 | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Distortion Guitar | Classic-Guitar |
| 32 | Acoustic Bass | `acousticBass` | Stringed/Basses/Acoustic Bass | Acoustic-Bass |
| 33 | Electric Bass | `electricBass` | Stringed/Basses/Clean Bass | Pre-Bass |
| 34 | Electric Bass | `electricBass` | Stringed/Basses/Clean Bass | Pre-Bass |
| 35 | Acoustic Bass | `acousticBass` | Stringed/Basses/Fretless Bass | Fretless-Bass |
| 36 | Electric Bass | `electricBass` | Stringed/Basses/Slap Bass | Modern-Bass |
| 37 | Electric Bass | `electricBass` | Stringed/Basses/Slap Bass | Pre-Bass |
| 38 | Synth Bass | `synthBass` | Stringed/Basses/Synth Bass | FMSyn-Bass |
| 39 | Synth Bass | `synthBass` | Orchestra/Synth/Bass | Dep-Bass |
| 40 | Violin | `violin` | Orchestra/Strings/Violin | Violin-Solo |
| 41 | Viola | `viola` | Orchestra/Strings/Viola | Viola-Solo |
| 42 | Cello | `cello` | Orchestra/Strings/Cello | Cello-Solo |
| 43 | Contrabass | `contrabass` | Orchestra/Strings/Contrabass | Contrabass-Solo |
| 44 | Violin | `violin` | Orchestra/Strings/Violin | Violin1-Ensemble |
| 45 | Violin | `violin` | Orchestra/Strings/Violin | Violin1-Ensemble-Pizz |
| 46 | Harp | `harp` | Orchestra/Strings/Harp | Harp-Solo |
| 47 | Timpani | `timpani` | Orchestra/Other/Timpani | TimpaniHi-Tuned |
| 48 | Violin | `violin` | Orchestra/Strings/Violin | Violin1-Ensemble |
| 49 | Violin | `violin` | Orchestra/Strings/Violin | Violin1-Ensemble |
| 50 | Violin | `violin` | Orchestra/Strings/Violin | Violin1-Ensemble |
| 51 | Violin | `violin` | Orchestra/Strings/Violin | Lucid-Pad |
| 52 | Voice | `voice` | Orchestra/Other/Singer | Vocal-Pad |
| 53 | Voice | `voice` | Orchestra/Other/Singer | MetaChoir-Pad |
| 54 | Voice | `voice` | Orchestra/Other/Singer | MetaChoir-Pad |
| 55 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | OBStrings-Pad |
| 56 | Trumpet | `trumpet` | Orchestra/Winds/Trumpet | Trumpet-Solo |
| 57 | Trombone | `trombone` | Orchestra/Winds/Trombone | Trombone-Solo |
| 58 | Tuba | `tuba` | Orchestra/Winds/Tuba | BassTuba-Solo |
| 59 | Trumpet | `trumpet` | Orchestra/Winds/Trumpet | Trumpet-Sordino-Solo |
| 60 | French Horn | `frenchHorn` | Orchestra/Winds/Other Winds | FrenchHorn-Solo |
| 61 | Trumpet | `trumpet` | Orchestra/Winds/Other Winds | Trumpet-Ensemble |
| 62 | Trumpet | `trumpet` | Orchestra/Synth/Brass | Simple-Brass |
| 63 | Trumpet | `trumpet` | Orchestra/Synth/Brass | Matrix-Brass |
| 64 | Saxophone | `saxophone` | Orchestra/Winds/Saxophone | Soprano-Sax |
| 65 | Saxophone | `saxophone` | Orchestra/Winds/Saxophone | Alto-Sax |
| 66 | Saxophone | `saxophone` | Orchestra/Winds/Saxophone | Tenor-Sax |
| 67 | Saxophone | `saxophone` | Orchestra/Winds/Saxophone | Baritone-Sax |
| 68 | Oboe | `oboe` | Orchestra/Winds/Other Winds | Oboe-Solo |
| 69 | English Horn | `englishHorn` | Orchestra/Winds/Other Winds | EnglishHorn-Solo |
| 70 | Bassoon | `bassoon` | Orchestra/Winds/Bassoon | Bassoon-Solo |
| 71 | Clarinet | `clarinet` | Orchestra/Winds/Clarinet | Clarinet-Solo |
| 72 | Piccolo | `piccolo` | Orchestra/Winds/Flute | Piccolo-Solo |
| 73 | Flute | `flute` | Orchestra/Winds/Flute | Flute-Solo |
| 74 | Recorder | `recorder` | Orchestra/Winds/Flute | Recorder-Solo |
| 75 | Flute | `flute` | Orchestra/Winds/Flute | Flute-Solo |
| 76 | Recorder | `recorder` | Orchestra/Winds/Flute | Recorder-Solo |
| 77 | Flute | `flute` | Orchestra/Winds/Flute | Flute-Solo |
| 78 | Recorder | `recorder` | Orchestra/Winds/Flute | Recorder-Solo |
| 79 | Flute | `flute` | Orchestra/Winds/Flute | Flute-Solo |
| 80 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Classic-Lead |
| 81 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Classic-Lead |
| 82 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Bottle-Pad |
| 83 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Brassy-Lead |
| 84 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Classic-Lead |
| 85 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Vox-Pad |
| 86 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Classic-Lead |
| 87 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Classic-Lead |
| 88 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Intimacy-Pad |
| 89 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Vox-Pad |
| 90 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Brassy-Lead |
| 91 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Vocal-Pad |
| 92 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Lush-Pad |
| 93 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Lucid-Pad |
| 94 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Vocal-Pad |
| 95 | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | Rosin-Pad |
| 96 | Seq Synthesizer | `seqSynthesizer` | Orchestra/Synth/Sequencer | Harpy-Seq |
| 97 | Seq Synthesizer | `seqSynthesizer` | Orchestra/Other/Celesta | Mini-Pad |
| 98 | Seq Synthesizer | `seqSynthesizer` | Orchestra/Synth/Sequencer | Bottle-Pad |
| 99 | Seq Synthesizer | `seqSynthesizer` | Orchestra/Synth/Sequencer | Rubber-Lead |
| 100 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Celesta-Tuned |
| 101 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Recorder-Solo |
| 102 | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | Recorder-Solo |
| 103 | Trumpet | `trumpet` | Orchestra/Synth/Brass | Simple-Brass |
| 104 | Banjo | `banjo` | Stringed/Electric Guitars/Clean Guitar | Electric-Sitar |
| 105 | Banjo | `banjo` | Stringed/Other Stringed Instruments/Banjo | 5-Banjo |
| 106 | Ukulele | `ukulele` | Stringed/Other Stringed Instruments/Ukulele | CC-Ukulele |
| 107 | Banjo | `banjo` | Stringed/Other Stringed Instruments/Banjo | 5-Banjo |
| 108 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 109 | Bassoon | `bassoon` | Orchestra/Keyboard/Acoustic Piano | Bagpipe |
| 110 | Violin | `violin` | Orchestra/Strings/Violin | Violin-Solo |
| 111 | Flute | `flute` | Orchestra/Keyboard/Acoustic Piano | Oboe-Solo |
| 112 | Xylophone | `xylophone` | Orchestra/Other/Celesta | Celesta-Tuned |
| 113 | Celesta | `celesta` | Orchestra/Other/Celesta | Xylophone-Tuned |
| 114 | Vibraphone | `vibraphone` | Orchestra/Other/Vibraphone | Vibraphone-Tuned |
| 115 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 116 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 117 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 118 | Xylophone | `xylophone` | Orchestra/Other/Xylophone | Xylophone-Tuned |
| 119 | Celesta | `celesta` | Orchestra/Other/Celesta | Celesta-Tuned |
| 120 | Steel Guitar | `steelGuitar` | Stringed/Acoustic Guitars/Steel Guitar | D-Steel |
| 121 | Recorder | `recorder` | Orchestra/Keyboard/Acoustic Piano | Recorder-Solo |
| 122 | Recorder | `recorder` | Orchestra/Keyboard/Acoustic Piano | Recorder-Solo |
| 123 | Recorder | `recorder` | Orchestra/Keyboard/Acoustic Piano | Recorder-Solo |
| 124 | Recorder | `recorder` | Drums/Idiophone Percussion/Maracas | Celesta-Tuned |
| 125 | Recorder | `recorder` | Orchestra/Keyboard/Acoustic Piano | Recorder-Solo |
| 126 | Recorder | `recorder` | Drums/Idiophone Percussion/Hand Clap | GroupHandClap-Percu |
| 127 | Timpani | `timpani` | Drums/Idiophone Percussion/Maracas | TimpaniHi-Tuned |

## Guitar Pro's instrument list

Every instrument Guitar Pro 7.6 offers by name (stringed, drums and percussion, orchestra), with its default tuning where it has strings (low to high, MIDI numbers) and the octave its notation is written in (`-1`: an octave above the sound, as for guitars). Drums and percussion all use MIDI channel 10 and the type `drumKit`.

| Instrument | Instrument set | Type | Sound path | Program | Patch | Tuning | Octave |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Steel Guitar | Steel Guitar | `steelGuitar` | Stringed/Acoustic Guitars/Steel Guitar | 25 | D-Steel | 40 45 50 55 59 64 | -1 |
| 12 St. Steel Guitar | Steel Guitar | `steelGuitar` | Stringed/Acoustic Guitars/12 String Steel | 25 | 12-Steel | 40 45 50 55 59 64 | -1 |
| Nylon Guitar | Nylon Guitar | `nylonGuitar` | Stringed/Acoustic Guitars/Nylon Guitar | 24 | Concerto-Nylon | 40 45 50 55 59 64 | -1 |
| Resonator | Steel Guitar | `steelGuitar` | Stringed/Acoustic Guitars/Resonator | 25 | Reso-Nylon | 40 45 50 55 59 64 | -1 |
| Overdriven Guitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Overdrive Guitar | 29 | Strat-Guitar | 40 45 50 55 59 64 | -1 |
| Distortion Guitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Distortion Guitar | 30 | Classic-Guitar | 40 45 50 55 59 64 | -1 |
| Clean Guitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Clean Guitar | 27 | Strat-Guitar | 40 45 50 55 59 64 | -1 |
| Jazz Guitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Jazz Guitar | 26 | ES-Guitar | 40 45 50 55 59 64 | -1 |
| 12 St. Electric Guitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/12 Strings Electric Guitar | 27 | Rick12-Guitar | 40 45 50 55 59 64 | -1 |
| E.Sitar | Electric Guitar | `electricGuitar` | Stringed/Electric Guitars/Electric Sitar | 104 | Electric-Sitar | 40 45 50 55 59 64 | -1 |
| Acoustic Bass | Acoustic Bass | `acousticBass` | Stringed/Basses/Acoustic Bass | 32 | Acoustic-Bass | 28 33 38 43 | -1 |
| Electric Bass | Electric Bass | `electricBass` | Stringed/Basses/Clean Bass | 33 | Pre-Bass | 28 33 38 43 | -1 |
| Fretless Bass | Electric Bass | `electricBass` | Stringed/Basses/Fretless Bass | 35 | Fretless-Bass | 28 33 38 43 | -1 |
| Synthesizer Bass | Synth Bass | `synthBass` | Stringed/Basses/Synth Bass | 38 | FMSyn-Bass | 28 33 38 43 | -1 |
| Upright Bass | Acoustic Bass | `acousticBass` | Stringed/Basses/Upright Bass | 32 | Upright-Bass | 28 33 38 43 | -1 |
| Ukulele | Ukulele | `ukulele` | Stringed/Other Stringed Instruments/Ukulele | 24 | CC-Ukulele | 67 60 64 69 | 0 |
| Banjo | Banjo | `banjo` | Stringed/Other Stringed Instruments/Banjo | 105 | 5-Banjo | 67 50 55 59 62 | -1 |
| Mandolin | Mandolin | `mandolin` | Stringed/Other Stringed Instruments/Mandolin | 106 | American-Mandolin | 55 62 69 76 | 0 |
| Drumkit | Drums | `drumKit` | Drums/Drums/Drumkit | 0 | Drumkit-Master |  | 0 |
| Bongos N Congas | Drums | `drumKit` | Drums/Drums/Bongos N Congas | 0 | Kit-Percu |  | 0 |
| Kit-Samba | Drums | `drumKit` | Drums/Drums/Kit Samba | 0 | Kit-Samba |  | 0 |
| Kit-Africa | Drums | `drumKit` | Drums/Drums/Kit Africa | 0 | Kit-Africa |  | 0 |
| Kit-Latino | Drums | `drumKit` | Drums/Drums/Kit Latino | 0 | Kit-Latino |  | 0 |
| 2 Hands Maracas | Drums | `drumKit` | Drums/Idiophone Percussion/2 Hands Maracas | 70 | Maracas-Percu |  | 0 |
| Agogo | Drums | `drumKit` | Drums/Idiophone Percussion/Agogo | 0 | AgogoHigh-Percu |  | 0 |
| Bell Tree | Drums | `drumKit` | Drums/Idiophone Percussion/Bell Tree | 84 | BellTree-Percu |  | 0 |
| Cabasa | Drums | `drumKit` | Drums/Idiophone Percussion/Cabasa | 69 | Cabasa-Percu |  | 0 |
| Castanets | Drums | `drumKit` | Drums/Idiophone Percussion/Castanets | 85 | Castanets-Percu |  | 0 |
| Claves | Drums | `drumKit` | Drums/Idiophone Percussion/Claves | 75 | Claves-Percu |  | 0 |
| Cowbell | Drums | `drumKit` | Drums/Idiophone Percussion/Cowbell | 56 | CowbellSmall-Percu |  | 0 |
| Guiro | Drums | `drumKit` | Drums/Idiophone Percussion/Guiro | 73 | Guiro-Percu |  | 0 |
| Hand Clap | Drums | `drumKit` | Drums/Idiophone Percussion/Hand Clap | 39 | GroupHandClap-Percu |  | 0 |
| Jingle Bell | Drums | `drumKit` | Drums/Idiophone Percussion/Jingle Bell | 83 | JingleBell-Percu |  | 0 |
| Maracas | Drums | `drumKit` | Drums/Idiophone Percussion/Maracas | 70 | Maracas-Percu |  | 0 |
| Reverse Cymbal | Drums | `drumKit` | Drums/Idiophone Percussion/Reverse Cymbal | 119 | Reverse-Cymbal |  | 0 |
| Shaker | Drums | `drumKit` | Drums/Idiophone Percussion/Shaker | 82 | ShakerWooden-Percu |  | 0 |
| Vibraslap | Drums | `drumKit` | Drums/Idiophone Percussion/Vibraslap | 58 | Vibraslap-Percu |  | 0 |
| Whistle | Drums | `drumKit` | Drums/Idiophone Percussion/Whistle | 0 | Kit-Whistle |  | 0 |
| Woodblock | Drums | `drumKit` | Drums/Idiophone Percussion/Woodblock | 0 | Kit-Woodblock |  | 0 |
| Triangle | Drums | `drumKit` | Drums/Idiophone Percussion/Triangle | 70 | Triangle-Percu |  | 0 |
| Piatti | Drums | `drumKit` | Drums/Drums/Drumkit | 119 | Piatti-Percu |  | 0 |
| Tinkle Bell | Drums | `drumKit` | Drums/Idiophone Percussion/Jingle Bell | 83 | JingleBell-Percu |  | 0 |
| Bongos | Drums | `drumKit` | Drums/Membranophone Percussion/Bongos | 0 | Kit-Bongo |  | 0 |
| Congas | Drums | `drumKit` | Drums/Membranophone Percussion/Conga | 0 | Kit-Conga |  | 0 |
| Cuica | Drums | `drumKit` | Drums/Membranophone Percussion/Cuica | 0 | Kit-Cuica |  | 0 |
| Melodic Tom | Drums | `drumKit` | Drums/Membranophone Percussion/Melodic Tom | 117 | TimbaleHigh-Percu |  | 0 |
| Surdo | Drums | `drumKit` | Drums/Membranophone Percussion/Surdo | 87 | Surdo-Percu |  | 0 |
| Synth Drum | Drums | `drumKit` | Drums/Membranophone Percussion/Synth Drum | 118 | TimbaleHigh-Percu |  | 0 |
| Taiko Drum | Drums | `drumKit` | Drums/Membranophone Percussion/Taiko | 116 | TimbaleLow-Percu |  | 0 |
| Tambourine | Drums | `drumKit` | Drums/Membranophone Percussion/Tambourine | 54 | Latino Tambourine-Percu |  | 0 |
| Timbale | Drums | `drumKit` | Drums/Membranophone Percussion/Timbale | 0 | Kit-Timbale |  | 0 |
| Grancassa | Drums | `drumKit` | Drums/Membranophone Percussion/Bongos | 116 | Grancassa-Percu |  | 0 |
| Snare | Drums | `drumKit` | Drums/Membranophone Percussion/Bongos | 117 | SnareDrum-Percu |  | 0 |
| Acoustic Piano | Acoustic Piano | `acousticPiano` | Orchestra/Keyboard/Acoustic Piano | 0 | German-APiano | 40 45 50 55 59 64 | 0 |
| Electric Piano | Electric Piano | `electricPiano` | Orchestra/Keyboard/Electric Piano | 4 | MarkI-EPiano | 40 45 50 55 59 64 | 0 |
| Organ | Electric Organ | `electricOrgan` | Orchestra/Keyboard/Organ | 16 | Jazz-Organ | 40 45 50 55 59 64 | 0 |
| Harpsichord | Harpsichord | `harpsichord` | Orchestra/Keyboard/Clavinet | 6 | Harpsichord-Key | 40 45 50 55 59 64 | 0 |
| Accordion | Acoustic Piano | `acousticPiano` | Orchestra/Winds/Trombone | 21 | Accordion | 40 45 50 55 59 64 | 0 |
| Brass Synthesizer | Brass Synthesizer | `brassSynthesizer` | Orchestra/Synth/Brass | 62 | Simple-Brass | 40 45 50 55 59 64 | 0 |
| Key Synthesizer | Key Synthesizer | `keySynthesizer` | Orchestra/Synth/Keyboard | 98 | Harpy-Seq | 40 45 50 55 59 64 | 0 |
| Lead Synthesizer | Lead Synthesizer | `leadSynthesizer` | Orchestra/Synth/Lead | 86 | Classic-Lead | 40 45 50 55 59 64 | 0 |
| Bass Synthesizer | Bass Synthesizer | `bassSynthesizer` | Orchestra/Synth/Bass | 38 | FMSyn-Bass | 40 45 50 55 59 64 | 0 |
| Sequencer Synthesizer | Seq Synthesizer | `seqSynthesizer` | Orchestra/Synth/Sequencer | 96 | Harpy-Seq | 40 45 50 55 59 64 | 0 |
| Pad Synthesizer | Pad Synthesizer | `padSynthesizer` | Orchestra/Synth/Pad | 89 | Huge-Pad | 40 45 50 55 59 64 | 0 |
| Violin | Violin | `violin` | Orchestra/Strings/Violin | 40 | Violin-Solo | 55 62 69 76 | 0 |
| Viola | Viola | `viola` | Orchestra/Strings/Viola | 41 | Viola-Solo | 48 55 62 69 | 0 |
| Cello | Cello | `cello` | Orchestra/Strings/Cello | 42 | Cello-Solo | 36 43 50 57 | 0 |
| Contrabass | Contrabass | `contrabass` | Orchestra/Strings/Contrabass | 43 | Contrabass-Solo | 28 33 38 43 | -1 |
| Harp | Harp | `harp` | Orchestra/Strings/Harp | 46 | Harp-Solo | 40 45 50 55 59 64 | 0 |
| Harmonica | Harmonica | `harmonica` | Orchestra/Winds/Harmonica | 22 | Harmonica | 40 45 50 55 59 64 | 1 |
| Trumpet | Trumpet | `trumpet` | Orchestra/Winds/Trumpet | 56 | Trumpet-Solo | 40 45 50 55 59 64 | 0 |
| Flugelhorn | Trumpet | `trumpet` | Orchestra/Wind/Trumpet | 56 | Trumpet-Solo | 40 45 50 55 59 64 | 0 |
| Trombone | Trombone | `trombone` | Orchestra/Winds/Trombone | 57 | Trombone-Solo | 40 45 50 55 59 64 | 0 |
| Tuba | Tuba | `tuba` | Orchestra/Winds/Tuba | 58 | BassTuba-Solo | 23 28 33 38 43 48 | 0 |
| Saxophone  Alto | Saxophone | `saxophone` | Orchestra/Winds/Saxophone | 65 | Alto-Sax | 40 45 50 55 59 64 | 0 |
| Clarinet | Clarinet | `clarinet` | Orchestra/Winds/Clarinet | 71 | Clarinet-Solo | 40 45 50 55 59 64 | 0 |
| Bassoon | Bassoon | `bassoon` | Orchestra/Winds/Bassoon | 70 | Bassoon-Solo | 40 45 50 55 59 64 | 0 |
| Flute | Flute | `flute` | Orchestra/Winds/Flute | 73 | Flute-Solo | 40 45 50 55 59 64 | 0 |
| Bass Clarinet | Clarinet | `clarinet` | Orchestra/Winds/Other Winds | 71 | BassClarinet-Solo | 40 45 50 55 59 64 | 0 |
| Contrabassoon | Bassoon | `bassoon` | Orchestra/Winds/Other Winds | 70 | Contrabassoon-Solo | 40 45 50 55 59 64 | -1 |
| English Horn | English Horn | `englishHorn` | Orchestra/Winds/Other Winds | 69 | EnglishHorn-Solo | 40 45 50 55 59 64 | 0 |
| French Horn | French Horn | `frenchHorn` | Orchestra/Winds/Other Winds | 60 | FrenchHorn-Solo | 40 45 50 55 59 64 | 0 |
| Oboe | Oboe | `oboe` | Orchestra/Winds/Other Winds | 68 | Oboe-Solo | 40 45 50 55 59 64 | 0 |
| Recorder | Recorder | `recorder` | Orchestra/Winds/Other Winds | 74 | Recorder-Solo | 40 45 50 55 59 64 | 1 |
| Piccolo | Flute | `flute` | Orchestra/Wind/Flute | 73 | Piccolo-Solo | 40 45 50 55 59 64 | 0 |
| Bagpipe | Recorder | `recorder` | Orchestra/Wind/Flute | 109 | Bagpipe | 40 45 50 55 59 64 | 1 |
| Celesta | Celesta | `celesta` | Orchestra/Other/Celesta | 8 | Celesta-Tuned | 40 45 50 55 59 64 | 1 |
| Vibraphone | Vibraphone | `vibraphone` | Orchestra/Other/Vibraphone | 11 | Vibraphone-Tuned | 40 45 50 55 59 64 | 0 |
| Xylophone | Xylophone | `xylophone` | Orchestra/Other/Xylophone | 13 | Xylophone-Tuned | 40 45 50 55 59 64 | 1 |
| Timpani | Timpani | `timpani` | Orchestra/Other/Timpani | 47 | TimpaniHi-Tuned | 40 45 50 55 59 64 | 1 |
| Singer | Voice | `voice` | Orchestra/Other/Singer | 53 | Vocal-Pad | 40 45 50 55 59 64 | 0 |
| Choir | Choir | `choir` | Orchestra/Other/Singer | 52 | MetaChoir-Pad | 40 45 50 55 59 64 | 0 |
