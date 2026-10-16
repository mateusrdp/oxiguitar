// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The visual-regression references: fifty small scores, each about one
//! part of engraving, and how each is shown.
//!
//! The scores are saved in `fixtures/reference/` as Guitar Pro 7 files, so
//! the test reads them back through the real reader, and next to them the
//! pictures they engrave to. Run `OXIGUITAR_BLESS=1 cargo test -p
//! oxiguitar-testgen --test visual` after an intended change to engraving
//! and look at every picture that changed before committing them.

use std::collections::BTreeMap;

use oxiguitar_layout::{Layout, Notation, Options, Paper};

use crate::song::{Diagram, Kind, Song, Track};

/// One reference: a song and how to show it.
#[derive(Clone, Debug)]
pub struct Reference {
    /// File name without extension, in `fixtures/reference/`.
    pub name: &'static str,
    /// What it checks, for the index.
    pub about: &'static str,
    /// The music.
    pub song: Song,
    /// How it is engraved.
    pub options: Options,
    /// Drawn on the dark page.
    pub dark: bool,
}

fn reference(name: &'static str, about: &'static str, song: Song) -> Reference {
    Reference {
        name,
        about,
        song,
        options: Options::default(),
        dark: false,
    }
}

impl Reference {
    fn with(mut self, change: impl FnOnce(&mut Options)) -> Reference {
        change(&mut self.options);
        self
    }
}

/// A song with one track playing `voice` (and a second voice if given).
fn solo(title: &str, kind: Kind, voices: &[&str]) -> Song {
    let bars = crate::song::parse(voices[0]).map_or(1, |b| b.len());
    let mut song = Song::new(title, bars);
    song.artist = "OxiGuitar references".to_owned();
    let name = match &kind {
        Kind::Drums => "Drums",
        Kind::Fretted {
            bass_clef: true, ..
        } => "Bass",
        Kind::Fretted { .. } => "Guitar",
        Kind::Pitched { .. } => "Instrument",
    };
    let t = song.track(Track::new(name, "", kind));
    for (i, v) in voices.iter().enumerate() {
        t.voice(i, v);
    }
    song
}

fn guitar(title: &str, voices: &[&str]) -> Song {
    solo(title, Kind::guitar(), voices)
}

/// A typical four-bar riff, for references about the page rather than the
/// notes.
const RIFF: &str = "e 6:0 6:0 6:3 6:0 5:2 5:0 6:3 6:0 | q 6:0+5:2+4:2 e 4:2 4:0 q 5:2 5:0 | \
                    e 3:2 3:2 3:0 4:2 4:0 4:2 5:0 5:2 | h 6:0+5:2+4:2+3:1+2:0+1:0 h r";

fn riff(times: usize) -> String {
    vec![RIFF; times].join(" | ")
}

/// Every reference, in a fixed order.
pub fn references() -> Vec<Reference> {
    let mut out = vec![
        // Rhythm and notes.
        reference(
            "notes-and-rests",
            "Whole to sixty-fourth notes and rests on standard notation and tablature.",
            guitar(
                "Notes and Rests",
                &["w 3:2 | h 3:2 r | q 3:2 r 3:4 r | e 3:2 r 3:4 r 2:3 r 2:5 r | \
                   s 3:2 r 3:4 r 2:3 r 2:5 r 1:3 r 1:5 r 1:7 r 1:8 r | \
                   t 1:5 r 1:7 r 1:8 r 1:7 r 1:5 r 1:7 r 1:8 r 1:7 r q r e r s r x 1:5 1:7 r r"],
            ),
        ),
        reference(
            "beams-and-flags",
            "Beams over groups of eighths and sixteenths, flags on single notes, mixed groups.",
            guitar(
                "Beams and Flags",
                &["e 3:2 q 3:4 e 2:3 e 2:5 s 2:3 2:5 e 1:3 e r | \
                   s 3:2 3:4 2:3 2:5 e. 1:3 s 1:5 e 1:3 s 1:5 1:3 q 2:5 | \
                   e 6:0 s 6:3 6:5 e 5:2 s 5:3 5:5 q 4:2 e r 4:5 | \
                   t 1:5 1:7 1:8 1:7 1:5 1:7 1:8 1:10 q 1:12 h 1:8"],
            ),
        ),
        reference(
            "dots-and-ties",
            "Dotted and double-dotted notes, ties inside a bar and across the bar line.",
            guitar(
                "Dots and Ties",
                &["q. 3:2 e 3:4 q.. 2:3 s 2:5 | h 3:2 q ~3:2 q 2:3 | h. 1:3 q 1:5 | \
                   w ~1:5 | q 4:2+3:2 ~4:2+~3:2 h ~4:2+~3:2"],
            ),
        ),
        reference(
            "triplets",
            "Eighth, quarter and sixteenth triplets, and a triplet with a rest in it.",
            guitar(
                "Triplets",
                &["e/3 3:2 3:4 2:3 e/3 2:5 r 2:3 h 3:2 | q/3 1:3 1:5 1:3 h 2:5 | \
                   s/3 3:2 3:4 2:3 2:5 2:3 3:4 s/3 3:2 3:4 2:3 2:5 2:3 3:4 h r"],
            ),
        ),
        reference(
            "unusual-tuplets",
            "Quintuplets, sextuplets, septuplets, a duplet and a 5:3 ratio (marked n:m).",
            guitar(
                "Unusual Tuplets",
                &["s/5 1:5 1:7 1:8 1:7 1:5 s/6 2:5 2:6 2:8 2:6 2:5 2:6 h r | \
                   s/7 3:5 3:7 2:5 2:6 2:8 1:5 1:7 h r q r | \
                   e/2:3 3:2 3:4 q. r q r | e/5:3 1:3 1:5 1:7 1:5 1:3 q r q. r"],
            ),
        ),
        reference(
            "chords-and-accidentals",
            "Chords, sharps, flats and naturals in C, accidentals carried through the bar.",
            guitar(
                "Chords and Accidentals",
                &["q 6:0+5:2+4:2+3:1+2:0+1:0 5:0+4:2+3:2+2:2+1:0 5:3+4:2+3:0+2:1+1:0 \
                   6:3+5:2+4:0+3:0+2:0+1:3 | q 3:3 3:4 3:3 3:2 | q 2:1 2:2 2:1 2:0 | \
                   e 4:1 4:2 4:1 4:2 q 4:3+3:3 4:4+3:4"],
            ),
        ),
        {
            let mut song = guitar(
                "Key Signatures",
                &["q 3:0 3:2 2:0 2:1 | q 3:0 3:2 2:0 2:1 | q 3:0 3:2 2:0 2:1 | \
                   q 3:0 3:2 2:0 2:1 | q 3:0 3:2 2:0 2:1 | q 3:0 3:2 2:0 2:1"],
            );
            song.key(1, 2, false).key(2, 4, false).key(3, -3, false).key(4, -6, false);
            song.key(5, 0, true);
            reference(
                "key-signatures",
                "Key changes through sharps and flats, with naturals cancelling the old key.",
                song,
            )
        },
        {
            let mut song = guitar(
                "Time Signatures",
                &["q 3:0 3:2 2:0 2:1 | q 3:0 3:2 2:0 | e 3:0 3:2 2:0 2:1 2:3 1:0 | \
                   q 3:0 3:2 2:0 2:1 2:3 | e 1:0 1:1 1:3 1:5 1:3 1:1 1:0 | h 1:0 2:1 | \
                   e 3:0 3:2 2:0 2:1 2:3 1:0 1:1 1:3 1:5 1:3 1:1 1:0"],
            );
            song.time(1, 3, 4).time(2, 6, 8).time(3, 5, 4).time(4, 7, 8);
            song.time(5, 2, 2).time(6, 12, 8);
            reference(
                "time-signatures",
                "4/4, 3/4, 6/8, 5/4, 7/8, 2/2 and 12/8 in a row.",
                song,
            )
        },
        {
            let mut song = guitar("Repeats and Endings", &[&riff(2)]);
            song.bars[0].repeat_start = true;
            song.bars[2].repeat_end = Some(3);
            song.bars[2].endings = vec![1, 2];
            song.bars[3].endings = vec![3];
            song.bars[4].repeat_start = true;
            song.bars[6].endings = vec![1];
            song.bars[6].repeat_end = Some(2);
            song.bars[7].endings = vec![2];
            song.bars[7].double_bar = true;
            reference(
                "repeats-and-endings",
                "Repeat signs, a repeat played three times, first and second endings, a double bar.",
                song,
            )
        },
        {
            let mut song = guitar("Sections and Directions", &[&riff(2)]);
            song.bars[0].section = Some(("A".to_owned(), "Intro".to_owned()));
            song.bars[1].directions = vec!["Segno"];
            song.bars[2].directions = vec!["DaCoda"];
            song.bars[3].section = Some(("B".to_owned(), "Verse".to_owned()));
            song.bars[4].directions = vec!["Fine"];
            song.bars[5].directions = vec!["DaSegnoAlCoda"];
            song.bars[6].directions = vec!["Coda"];
            song.bars[6].section = Some(("C".to_owned(), "Outro".to_owned()));
            reference(
                "sections-and-directions",
                "Rehearsal letters with section names; segno and coda signs at the start of their bars, To Coda, Fine and D.S. al Coda at the end of theirs.",
                song,
            )
        },
        {
            let mut song = guitar("Tempo Changes", &[&riff(2)]);
            song.tempo = 90;
            song.tempo_label = "Moderato".to_owned();
            song.tempo_changes = vec![(2, 0.0, 120), (4, 0.5, 140), (6, 0.0, 70)];
            reference(
                "tempo-changes",
                "A labelled starting tempo, then changes at a bar and in the middle of one.",
                song,
            )
        },
        reference(
            "two-voices",
            "Two voices on one staff: voice 1 stems up, voice 2 stems down, rests moved aside.",
            guitar(
                "Two Voices",
                &[
                    "e 1:0 1:3 1:5 1:3 1:0 1:3 1:5 1:7 | q 2:5 2:3 h 2:1 | \
                     e 1:5 1:3 1:0 2:3 q 1:0 r | w 1:0",
                    "h 6:0 5:2 | q 6:0 r 5:0 r | h 5:0 r | w 6:0",
                ],
            ),
        ),
        reference(
            "four-voices",
            "Four voices: 1 and 3 up, 2 and 4 down, voices that rest for a whole bar hidden.",
            guitar(
                "Four Voices",
                &[
                    "q 1:0 1:3 1:5 1:3 | q 1:0 1:3 h 1:5 | w 1:3",
                    "w 6:0 | h 6:3 6:5 | w 6:0",
                    "h 2:1 2:3 | w r | h 2:1 2:3",
                    "w 5:2 | h 5:3 5:2 | w r",
                ],
            ),
        ),
        {
            let mut song = guitar(
                "Rhythm on Tab",
                &[
                    "e 1:0 1:3 1:5 1:3 q 1:0 r | s 2:1 2:3 2:5 2:3 e 2:1 r q. 2:3 e r | w 1:0",
                    "h 6:0 5:2 | q 6:0 r 5:0 r | w 6:0",
                ],
            );
            song.tracks[0].notation(2);
            reference(
                "rhythm-on-tab",
                "A tab-only track: rhythm stems above for voice 1, below for voice 2, rests outside the staff.",
                song,
            )
        },
        {
            let mut song = guitar("Standard Only", &[&riff(1)]);
            song.tracks[0].notation(1);
            reference(
                "standard-only",
                "A guitar saved with standard notation only: no tablature drawn.",
                song,
            )
        },
        {
            let mut song = guitar(
                "Chord Names",
                &["@c0 h 5:0+4:2+3:2+2:1+1:0 @c1 h 6:3+5:2+4:0+3:0+2:0+1:3 | \
                   @c2 q 5:3+4:2+3:0+2:1+1:0 @c3 q 4:0+3:2+2:3+1:2 @c4 q 6:1+5:3+4:3+3:2+2:1+1:1 \
                   @c5 q 6:5+5:7+4:7+3:6+2:5+1:5 | @c0 e 5:0 4:2 3:2 2:1 @c1 e 6:3 4:0 3:0 1:3 | \
                   @c2 w 5:3+4:2+3:0+2:1+1:0"],
            );
            let t = &mut song.tracks[0];
            for (name, frets) in [
                ("Am", "x02210"),
                ("G", "320003"),
                ("C", "x32010"),
                ("D", "xx0232"),
                ("F", "133211"),
                ("A", "577655"),
            ] {
                t.chord(Diagram::new(name, frets));
            }
            reference(
                "chord-names",
                "Chord names over their beats, never pushed sideways, and the diagrams in the header.",
                song,
            )
        },
        reference(
            "beat-text",
            "Text above beats, next to chord names and tempo.",
            guitar(
                "Beat Text",
                &["@tlet_ring q 5:0+4:2 5:0 @tpalm_mute e 6:0 6:0 6:0 6:0 | \
                   @tharmonics h 1:12 @tslower h 1:7 | w 6:0"],
            ),
        ),
        {
            let mut song = solo(
                "Lyrics",
                Kind::Pitched {
                    program: 54,
                    bass_clef: false,
                },
                &["q C5 D5 E5 C5 | h. G5 q E5 | e F5 F5 E5 E5 q D5 C5 | w C5"],
            );
            song.tracks[0].name = "Voice".to_owned();
            song.tracks[0].lyrics(0, "Twin-kle lit-tle star, how I won-der what you are now");
            reference(
                "lyrics",
                "A voice with lyrics under the staff, syllables split by hyphens.",
                song,
            )
        },
        reference(
            "dynamics",
            "Dynamics from ppp to fff, marked only where they change.",
            guitar(
                "Dynamics",
                &["@ppp q 3:2 @pp 3:4 @p 2:3 @mp 2:5 | @mf q 3:2 3:4 @f 2:3 2:5 | \
                   @ff h 1:3 @fff 1:5 | @p w 3:2"],
            ),
        ),
        reference(
            "dynamics-hidden",
            "The same dynamics with View → Dynamics off: none drawn.",
            guitar(
                "Dynamics Hidden",
                &["@ppp q 3:2 @pp 3:4 @p 2:3 @mp 2:5 | @mf q 3:2 3:4 @f 2:3 2:5 | \
                   @ff h 1:3 @fff 1:5 | @p w 3:2"],
            ),
        )
        .with(|o| o.dynamics = false),
        reference(
            "grace-notes",
            "Grace notes before beats, alone and before chords, on both staves.",
            guitar(
                "Grace Notes",
                &["@grace s 3:4 q 3:5 @grace s 2:3 q 2:5 h r | @grace s 1:3 h 1:5 @grace s 4:2 \
                   h 4:2+3:2 | @grace s 2:6 e 2:8 2:6 @grace s 2:6 q 2:8 h r"],
            ),
        ),
        // Effects (TAB-1).
        reference(
            "hammer-on-pull-off",
            "Hammer-ons and pull-offs, single and in chains, with the arcs and H and P.",
            guitar(
                "Hammer-on and Pull-off",
                &["e 3:5h 3:7 3:7h 3:5 2:5h 2:7h 2:8 r | s 1:5h 1:8h 1:5h 1:8 1:5h 1:8h 1:5 r \
                   e 2:8h 2:5 q r | q 3:2h 3:4 3:4h 3:2"],
            ),
        ),
        reference(
            "bends",
            "Full, half and released bends, with their arrows and labels.",
            guitar(
                "Bends",
                &["q 2:8b 2:8B h 2:8R | q 3:7b 1:5B h 2:10b | e 2:8B 2:8 q 3:7R h 1:12b"],
            ),
        ),
        reference(
            "slides",
            "Shift and legato slides between notes, slides into and out of a note.",
            guitar(
                "Slides",
                &["q 3:5/ 3:7 3:7S 3:9 | q 2:5i 2:8d h 2:7u | q 1:5I 1:7 3:2/ 3:4 | w 4:5d"],
            ),
        ),
        reference(
            "vibrato-and-trill",
            "Vibrato on single notes and long notes, and trills with their other fret.",
            guitar(
                "Vibrato and Trill",
                &["q 2:8v 2:10v h 1:12v | h 3:7T9 2:5T7 | w 1:15v"],
            ),
        ),
        reference(
            "harmonics",
            "Natural, artificial and pinch harmonics.",
            guitar(
                "Harmonics",
                &["q 3:12n 2:12n 1:12n 6:5n | q 3:5a 2:7a h 3:9p | h 3:7p 2:5p"],
            ),
        ),
        reference(
            "dead-ghost-palm-mute",
            "Dead notes (x), ghost notes in parentheses and palm mutes (P.M.).",
            guitar(
                "Dead, Ghost and Palm Mute",
                &["e 6:0x 6:0x 6:0m 6:0m 6:0m 6:0m 5:2g 5:2g | q 3:2g 3:4 6:0x+5:0x+4:0x 3:2 | \
                   s 6:0m 6:0m 6:0m 6:0m 6:3m 6:3m 6:0m 6:0m e 6:5 6:3 6:0x 6:0"],
            ),
        ),
        reference(
            "let-ring",
            "Let ring over arpeggios.",
            guitar(
                "Let Ring",
                &["e 5:0l 4:2l 3:2l 2:1l 1:0l 2:1l 3:2l 4:2l | e 6:3l 5:2l 4:0l 3:0l 2:0l 1:3 q r"],
            ),
        ),
        reference(
            "articulations",
            "Accents, heavy accents and staccato on both staves.",
            guitar(
                "Articulations",
                &["q 3:2> 3:4^ 2:3. 2:5>. | e 1:3. 1:5. 1:3. 1:5. q 1:7^ 1:8> | w 1:5>"],
            ),
        ),
        reference(
            "tremolo-picking",
            "Tremolo picking at eighth, sixteenth and thirty-second speeds.",
            guitar(
                "Tremolo Picking",
                &["@trem8 h 1:12 @trem16 h 1:10 | @trem32 w 1:8 | @trem16 q 2:8 @trem16 2:10 h r"],
            ),
        ),
        reference(
            "strums-and-techniques",
            "Strums up and down, slap, pop and tapping.",
            guitar(
                "Strums and Techniques",
                &["@down q 6:0+5:2+4:2+3:1+2:0+1:0 @up 6:0+5:2+4:2+3:1+2:0+1:0 \
                   @down h 5:0+4:2+3:2+2:1+1:0 | @slap q 6:0 @pop 4:2 @slap 6:0 @pop 4:4 | \
                   @tap e 1:12 1:5 1:8 @tap 1:12 1:5 1:8 q r"],
            ),
        ),
        reference(
            "whammy-and-fade-in",
            "A whammy-bar dive and a fade-in.",
            guitar(
                "Whammy and Fade In",
                &["@dive w 3:7 | @fade h 2:5+1:5 h r | @dive h 6:0 @fade h 4:2"],
            ),
        ),
        reference(
            "effects-together",
            "A dense lick with several effects on neighbouring notes: lanes above the tab.",
            guitar(
                "Effects Together",
                &["s 1:5h 1:8 2:5h 2:8m 3:5m 3:7m 2:8b 2:8 e 1:5v @trem16 1:5 q 3:7/ | \
                   e 3:9n 2:10a 1:12l 1:12l s 2:8x 2:8x 2:8g 2:8g q 1:8R"],
            ),
        ),
        // Instruments.
        reference(
            "bass",
            "A four-string bass: bass clef an octave down over a four-line tab.",
            solo(
                "Bass",
                Kind::bass(),
                &["e 4:0 4:0 4:3 4:0 3:2 3:0 4:3 4:0 | q 3:0 3:2 2:0 2:2 | \
                   e 4:5 4:5 3:7 4:5 q 3:5 r | w 4:0"],
            ),
        ),
        reference(
            "seven-string",
            "A seven-string guitar: seven tab lines and its tuning in the header.",
            solo(
                "Seven String",
                Kind::Fretted {
                    program: 30,
                    tuning: vec![35, 40, 45, 50, 55, 59, 64],
                    capo: 0,
                    bass_clef: false,
                },
                &["e 7:0m 7:0m 7:3 7:0m 7:0m 7:5 7:0m 6:2 | q 7:0+6:2+5:2 r h 7:0+6:2+5:2"],
            ),
        ),
        reference(
            "drop-d-and-capo",
            "Drop D tuning with a capo: tuning and capo in the header.",
            solo(
                "Drop D and Capo",
                Kind::Fretted {
                    program: 25,
                    tuning: vec![38, 45, 50, 55, 59, 64],
                    capo: 2,
                    bass_clef: false,
                },
                &["q 6:0+5:0+4:0 6:2+5:2+4:2 6:3+5:3+4:3 6:0+5:0+4:0 | e 1:0 2:3 3:2 4:0 h 6:0"],
            ),
        ),
        {
            let mut song = solo(
                "Drums",
                Kind::Drums,
                &["e d36+d42 d42 d38+d42 d42 d36+d42 d36+d42 d38+d42 d42 | \
                   e d49+d36 d42 d38+d42 d42 s d38 d38 d47 d45 e d43 d41 | \
                   q d36+d51 d38+d51 d36+d51 d38+d51 | w d49+d36"],
            );
            song.tracks[0].short_name = "drm.".to_owned();
            reference(
                "drums",
                "A drum kit on the percussion clef: kick, snare, hi-hat, toms and cymbals.",
                song,
            )
        },
        reference(
            "piano-grand-staff",
            "A piano with one staff in the file, split over a grand staff by hand range.",
            solo(
                "Piano",
                Kind::piano(),
                &[
                    "q C5+E5+G5 C5+E5+G5 B4+D5+G5 B4+D5+G5 | h A4+C5+E5 F4+A4+C5 | \
                     e G5 F5 E5 D5 C5 B4 A4 G4 | w C5+E5+G5",
                    "h C3 G2 | h A2 F2 | q G2 G3 G2 B2 | w C3",
                ],
            ),
        ),
        {
            let mut song = Song::new("Violin and Cello", 4);
            song.artist = "OxiGuitar references".to_owned();
            song.track(Track::new(
                "Violin",
                "Vln.",
                Kind::Pitched {
                    program: 40,
                    bass_clef: false,
                },
            ))
            .voice(0, "q E5 F#5 G5 A5 | h B5 A5 | q G5 F#5 E5 D5 | w E5");
            song.track(Track::new(
                "Cello",
                "Vc.",
                Kind::Pitched {
                    program: 42,
                    bass_clef: true,
                },
            ))
            .voice(0, "h E3 B2 | h G2 D3 | h C3 A2 | w E2");
            reference(
                "strings-treble-and-bass",
                "Two instruments without strings: treble clef and bass clef staves together.",
                song,
            )
            .with(|o| o.tracks = vec![0, 1])
        },
        // Several tracks.
        reference(
            "notation-choice",
            "The user's notation per track: the lead as tab only, the bass as standard only.",
            band(4),
        )
        .with(|o| {
            o.tracks = vec![0, 2];
            o.notation = BTreeMap::from([
                (
                    0,
                    Notation {
                        standard: false,
                        tablature: true,
                    },
                ),
                (
                    2,
                    Notation {
                        standard: true,
                        tablature: false,
                    },
                ),
            ]);
        }),
        {
            let mut song = guitar("Header and Credits", &[&riff(1)]);
            song.subtitle = "A Subtitle Long Enough to Matter".to_owned();
            song.artist = "The OxiGuitar Contributors".to_owned();
            song.album = "Reference Scores".to_owned();
            song.words = "Someone Who Writes Words".to_owned();
            song.music = "Someone Who Writes Music".to_owned();
            song.copyright = "Public domain".to_owned();
            song.tabber = "oxiguitar-testgen".to_owned();
            reference(
                "header-and-credits",
                "A full header: title, subtitle, artist, album, words, music, copyright, tabber, tuning.",
                song,
            )
        },
        // Layouts.
        reference(
            "page-breaks",
            "Thirty-two bars over pages, with page numbers.",
            guitar("Page Breaks", &[&riff(8)]),
        ),
        reference(
            "layout-vertical",
            "The Vertical layout: lines as wide as a narrow window, no pages.",
            guitar("Vertical", &[&riff(3)]),
        )
        .with(|o| {
            o.layout = Layout::Vertical;
            o.screen_width = 420.0;
        }),
        reference(
            "layout-horizontal",
            "The Horizontal layout: every bar on one line, no header.",
            guitar("Horizontal", &[&riff(2)]),
        )
        .with(|o| o.layout = Layout::Horizontal),
        reference(
            "layout-parchment",
            "The Parchment layout: page-wide lines on one sheet, no page breaks.",
            guitar("Parchment", &[&riff(7)]),
        )
        .with(|o| o.layout = Layout::Parchment),
        reference(
            "paper-letter",
            "US Letter paper instead of A4.",
            guitar("Letter Paper", &[&riff(3)]),
        )
        .with(|o| *o = o.clone().with_paper(Paper::Letter)),
        {
            let mut r = reference(
                "dark-page",
                "The dark page (View → Dark Page) with four tracks together, names at each system.",
                band(2),
            )
            .with(|o| o.tracks = vec![0, 1, 2, 3]);
            r.dark = true;
            r
        },
        {
            // Twelve tracks: the eight of the band and four more guitars.
            let mut song = band8(band(2));
            for i in 0..4 {
                let mut t = song.tracks[i % 2].clone();
                t.name = format!("Guitar {}", i + 3);
                t.short_name = format!("Gt. {}", i + 3);
                song.tracks.push(t);
            }
            reference(
                "tall-system-shrinks",
                "Twelve tracks in one system, taller than the page at full size: the staff shrinks to fit.",
                song,
            )
            .with(|o| o.tracks = (0..12).collect())
        },
        reference(
            "eight-tracks",
            "Eight tracks together: guitars, bass, drums, piano on a grand staff, strings, voice, acoustic tab.",
            band8(band(2)),
        )
        .with(|o| o.tracks = (0..8).collect()),
        reference(
            "empty-bars",
            "Bars with only rests: a whole rest, a dotted half rest filling 3/4, rests after a note.",
            {
                let mut s = guitar("Empty Bars", &["w r | h. r | q 3:2 h. r | w r"]);
                s.time(1, 3, 4).time(2, 4, 4);
                s
            },
        ),
    ];
    for r in &mut out {
        if r.song.artist.is_empty() {
            r.song.artist = "OxiGuitar references".to_owned();
        }
    }
    out
}

/// A four-track band (lead, rhythm, bass, drums) over `times` × 2 bars.
fn band(times: usize) -> Song {
    let lead = vec!["e 1:5 1:8 2:5 2:8 3:5 3:7 2:8b 1:5 | q 1:5v 2:8 h 3:7"; times].join(" | ");
    let rhythm = vec![
        "q 6:0+5:2+4:2 e 6:0m 6:0m q 6:0+5:2+4:2 e 6:3m 6:3m | h 5:0+4:2+3:2 h 6:3+5:2+4:0";
        times
    ]
    .join(" | ");
    let bass = vec!["e 4:0 4:0 4:0 4:0 4:3 4:3 4:3 4:3 | e 3:0 3:0 3:0 3:0 4:3 4:3 4:3 4:3"; times]
        .join(" | ");
    let drums =
        vec!["e d36+d42 d42 d38+d42 d42 d36+d42 d36+d42 d38+d42 d42 | e d36+d49 d42 d38+d42 d42 d36+d42 d42 d38+d42 d38"; times]
            .join(" | ");
    let mut song = Song::new("Band", 2 * times);
    song.artist = "OxiGuitar references".to_owned();
    song.tempo = 128;
    song.track(Track::new("Lead Guitar", "Ld.", Kind::guitar()))
        .voice(0, &lead);
    song.track(Track::new("Rhythm Guitar", "Rhy.", Kind::guitar()))
        .voice(0, &rhythm);
    song.track(Track::new("Bass", "Bs.", Kind::bass()))
        .voice(0, &bass);
    song.track(Track::new("Drums", "Dr.", Kind::Drums))
        .voice(0, &drums);
    song
}

/// The band with four more tracks: piano, strings, voice and acoustic.
fn band8(mut song: Song) -> Song {
    let bars = song.bars.len();
    let times = bars / 2;
    let piano =
        vec!["q E4+A4+C5 E4+A4+C5 E4+A4+C5 E4+A4+C5 | h E4+A4+C5 D4+G4+B4"; times].join(" | ");
    let strings = vec!["w A4+C5+E5 | w G4+B4+D5"; times].join(" | ");
    let voice = vec!["q A4 C5 E5 C5 | h D5 r"; times].join(" | ");
    let acoustic =
        vec!["e 5:0 4:2 3:2 2:1 1:0 2:1 3:2 4:2 | e 6:3 5:2 4:0 3:0 2:0 3:0 4:0 5:2"; times]
            .join(" | ");
    song.track(Track::new("Piano", "Pno.", Kind::piano()))
        .voice(0, &piano);
    song.track(Track::new(
        "Strings",
        "Str.",
        Kind::Pitched {
            program: 48,
            bass_clef: false,
        },
    ))
    .voice(0, &strings);
    song.track(Track::new(
        "Voice",
        "Vo.",
        Kind::Pitched {
            program: 54,
            bass_clef: false,
        },
    ))
    .voice(0, &voice);
    song.track(Track::new("Acoustic Guitar", "Ac.", Kind::acoustic()))
        .voice(0, &acoustic);
    song
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Dur, Song};

    /// Every voice's bars hold exactly what the time signature says (a
    /// voice other than the first may also leave a bar empty).
    fn check_bars(song: &Song, label: &str, problems: &mut Vec<String>) {
        for t in &song.tracks {
            for (vi, v) in t.voices.iter().enumerate() {
                if v.len() != song.bars.len() {
                    problems.push(format!(
                        "{label}: {} voice {vi}: {} bars, the song has {}",
                        t.name,
                        v.len(),
                        song.bars.len()
                    ));
                }
                for (bi, (beats, mb)) in v.iter().zip(&song.bars).enumerate() {
                    let want = 3840 * u32::from(mb.time.0) / u32::from(mb.time.1);
                    let want = f64::from(want);
                    // Exact lengths: a septuplet sixteenth is not a whole tick.
                    let got: f64 = beats
                        .iter()
                        .filter(|b| !b.grace)
                        .map(|b| {
                            let (n, m) = b.dur.tuplet.unwrap_or((1, 1));
                            let plain = Dur {
                                tuplet: None,
                                ..b.dur
                            };
                            f64::from(plain.ticks()) * f64::from(m) / f64::from(n)
                        })
                        .sum();
                    if !((got - want).abs() < 0.01 || (got == 0.0 && vi > 0)) {
                        problems.push(format!(
                            "{label}: {} voice {vi} bar {}: {got} ticks, the bar holds {want}",
                            t.name,
                            bi + 1
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn fifty_references_with_full_bars() {
        let refs = references();
        assert_eq!(refs.len(), 50);
        let mut names: Vec<&str> = refs.iter().map(|r| r.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 50, "names are unique");
        let mut problems = Vec::new();
        for r in &refs {
            check_bars(&r.song, r.name, &mut problems);
        }
        check_bars(&crate::band_song(40), "band song", &mut problems);
        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }
}
