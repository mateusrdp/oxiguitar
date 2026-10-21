// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What Guitar Pro 7 needs to give a track a sound: an instrument set and a
//! sound from its own library for every General MIDI program, and its
//! standard drum kit. Read from files Mateus made in Guitar Pro 7.6.0 on
//! 19 Oct 2026 (`docs/formats/guitar-pro-sounds.md`); generated, one row per
//! line, so `rustfmt` leaves the tables alone.

/// One instrument's set and sound.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sound {
    /// The instrument set's name (`<InstrumentSet><Name>`).
    pub(super) set_name: &'static str,
    /// The instrument set's type (`<InstrumentSet><Type>`).
    pub(super) set_type: &'static str,
    /// The sound's path in Guitar Pro's library (`<Sound><Path>`).
    pub(super) path: &'static str,
    /// The sound engine's patch (`<RSE><SoundbankPatch>`).
    pub(super) patch: &'static str,
}

impl Sound {
    const fn new(
        set_name: &'static str,
        set_type: &'static str,
        path: &'static str,
        patch: &'static str,
    ) -> Sound {
        Sound {
            set_name,
            set_type,
            path,
            patch,
        }
    }
}

/// Guitar Pro 7's own drum kit, for drum tracks.
pub(super) const DRUMS: Sound =
    Sound::new("Drums", "drumKit", "Drums/Drums/Drumkit", "Drumkit-Master");

/// One drum of the kit.
#[derive(Debug)]
pub(super) struct DrumElement {
    pub(super) name: &'static str,
    pub(super) kind: &'static str,
    pub(super) soundbank: &'static str,
    pub(super) articulations: &'static [Articulation],
}

/// One way to play a drum: where it sits on the staff, its note heads, the
/// General MIDI note it is entered as and the one it plays.
#[derive(Debug)]
pub(super) struct Articulation {
    pub(super) name: &'static str,
    pub(super) staff_line: i8,
    pub(super) noteheads: &'static str,
    pub(super) placement: &'static str,
    pub(super) symbol: &'static str,
    pub(super) input: u8,
    pub(super) rse: &'static str,
    pub(super) output: u8,
}

/// The place in the kit (counted through every drum's articulations) of
/// the articulation entered as General MIDI note `midi`, if the kit has one.
pub(super) fn drum_articulation(midi: u8) -> Option<usize> {
    DRUM_KIT
        .iter()
        .flat_map(|e| e.articulations)
        .position(|a| a.input == midi)
}

#[rustfmt::skip]
mod generated {
    use super::{Articulation, DrumElement, Sound};

    /// What Guitar Pro 7.6 chose for a track set to each General MIDI program:
    /// the instrument set's name and type, the sound's path and its sound-engine
    /// patch (`docs/formats/guitar-pro-sounds.md`).
    pub(in super::super) const GM_SOUNDS: [Sound; 128] = [
        Sound::new("Acoustic Piano", "acousticPiano", "Orchestra/Keyboard/Acoustic Piano", "German-APiano"), // 0
        Sound::new("Acoustic Piano", "acousticPiano", "Orchestra/Keyboard/Acoustic Piano", "German-APiano"), // 1
        Sound::new("Electric Piano", "electricPiano", "Orchestra/Keyboard/Acoustic Piano", "German-APiano"), // 2
        Sound::new("Acoustic Piano", "acousticPiano", "Orchestra/Keyboard/Acoustic Piano", "German-APiano"), // 3
        Sound::new("Electric Piano", "electricPiano", "Orchestra/Keyboard/Electric Piano", "MarkI-EPiano"), // 4
        Sound::new("Electric Piano", "electricPiano", "Orchestra/Keyboard/Electric Piano", "200A-EPiano"), // 5
        Sound::new("Harpsichord", "harpsichord", "Orchestra/Keyboard/Clavinet", "Harpsichord-Key"), // 6
        Sound::new("Harpsichord", "harpsichord", "Orchestra/Keyboard/Clavinet", "Harpsichord-Key"), // 7
        Sound::new("Celesta", "celesta", "Orchestra/Other/Celesta", "Celesta-Tuned"), // 8
        Sound::new("Vibraphone", "vibraphone", "Orchestra/Other/Celesta", "Celesta-Tuned"), // 9
        Sound::new("Vibraphone", "vibraphone", "Orchestra/Other/Vibraphone", "Vibraphone-Tuned"), // 10
        Sound::new("Vibraphone", "vibraphone", "Orchestra/Other/Vibraphone", "Vibraphone-Tuned"), // 11
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 12
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 13
        Sound::new("Vibraphone", "vibraphone", "Orchestra/Other/Vibraphone", "Vibraphone-Tuned"), // 14
        Sound::new("Banjo", "banjo", "Stringed/Other Stringed Instruments/Banjo", "5-Banjo"), // 15
        Sound::new("Electric Organ", "electricOrgan", "Orchestra/Keyboard/Organ", "Jazz-Organ"), // 16
        Sound::new("Electric Organ", "electricOrgan", "Orchestra/Keyboard/Organ", "Jazz-Organ"), // 17
        Sound::new("Electric Organ", "electricOrgan", "Orchestra/Keyboard/Organ", "Jazz-Organ"), // 18
        Sound::new("Electric Organ", "electricOrgan", "Orchestra/Keyboard/Organ", "Ballad-Organ"), // 19
        Sound::new("Electric Organ", "electricOrgan", "Orchestra/Winds/Other Winds", "Oboe-Solo"), // 20
        Sound::new("Electric Organ", "electricOrgan", "Stringed/Acoustic Guitars/Steel Guitar", "Accordion"), // 21
        Sound::new("Recorder", "recorder", "Orchestra/Winds/Harmonica", "Harmonica"), // 22
        Sound::new("Electric Organ", "electricOrgan", "Stringed/Acoustic Guitars/Steel Guitar", "Bandoneon"), // 23
        Sound::new("Nylon Guitar", "nylonGuitar", "Stringed/Acoustic Guitars/Nylon Guitar", "Concerto-Nylon"), // 24
        Sound::new("Steel Guitar", "steelGuitar", "Stringed/Acoustic Guitars/Steel Guitar", "D-Steel"), // 25
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Jazz Guitar", "ES-Guitar"), // 26
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Clean Guitar", "Strat-Guitar"), // 27
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Clean Guitar", "Strat-Guitar"), // 28
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Overdrive Guitar", "Strat-Guitar"), // 29
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Distortion Guitar", "Classic-Guitar"), // 30
        Sound::new("Electric Guitar", "electricGuitar", "Stringed/Electric Guitars/Distortion Guitar", "Classic-Guitar"), // 31
        Sound::new("Acoustic Bass", "acousticBass", "Stringed/Basses/Acoustic Bass", "Acoustic-Bass"), // 32
        Sound::new("Electric Bass", "electricBass", "Stringed/Basses/Clean Bass", "Pre-Bass"), // 33
        Sound::new("Electric Bass", "electricBass", "Stringed/Basses/Clean Bass", "Pre-Bass"), // 34
        Sound::new("Acoustic Bass", "acousticBass", "Stringed/Basses/Fretless Bass", "Fretless-Bass"), // 35
        Sound::new("Electric Bass", "electricBass", "Stringed/Basses/Slap Bass", "Modern-Bass"), // 36
        Sound::new("Electric Bass", "electricBass", "Stringed/Basses/Slap Bass", "Pre-Bass"), // 37
        Sound::new("Synth Bass", "synthBass", "Stringed/Basses/Synth Bass", "FMSyn-Bass"), // 38
        Sound::new("Synth Bass", "synthBass", "Orchestra/Synth/Bass", "Dep-Bass"), // 39
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin-Solo"), // 40
        Sound::new("Viola", "viola", "Orchestra/Strings/Viola", "Viola-Solo"), // 41
        Sound::new("Cello", "cello", "Orchestra/Strings/Cello", "Cello-Solo"), // 42
        Sound::new("Contrabass", "contrabass", "Orchestra/Strings/Contrabass", "Contrabass-Solo"), // 43
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin1-Ensemble"), // 44
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin1-Ensemble-Pizz"), // 45
        Sound::new("Harp", "harp", "Orchestra/Strings/Harp", "Harp-Solo"), // 46
        Sound::new("Timpani", "timpani", "Orchestra/Other/Timpani", "TimpaniHi-Tuned"), // 47
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin1-Ensemble"), // 48
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin1-Ensemble"), // 49
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin1-Ensemble"), // 50
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Lucid-Pad"), // 51
        Sound::new("Voice", "voice", "Orchestra/Other/Singer", "Vocal-Pad"), // 52
        Sound::new("Voice", "voice", "Orchestra/Other/Singer", "MetaChoir-Pad"), // 53
        Sound::new("Voice", "voice", "Orchestra/Other/Singer", "MetaChoir-Pad"), // 54
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "OBStrings-Pad"), // 55
        Sound::new("Trumpet", "trumpet", "Orchestra/Winds/Trumpet", "Trumpet-Solo"), // 56
        Sound::new("Trombone", "trombone", "Orchestra/Winds/Trombone", "Trombone-Solo"), // 57
        Sound::new("Tuba", "tuba", "Orchestra/Winds/Tuba", "BassTuba-Solo"), // 58
        Sound::new("Trumpet", "trumpet", "Orchestra/Winds/Trumpet", "Trumpet-Sordino-Solo"), // 59
        Sound::new("French Horn", "frenchHorn", "Orchestra/Winds/Other Winds", "FrenchHorn-Solo"), // 60
        Sound::new("Trumpet", "trumpet", "Orchestra/Winds/Other Winds", "Trumpet-Ensemble"), // 61
        Sound::new("Trumpet", "trumpet", "Orchestra/Synth/Brass", "Simple-Brass"), // 62
        Sound::new("Trumpet", "trumpet", "Orchestra/Synth/Brass", "Matrix-Brass"), // 63
        Sound::new("Saxophone", "saxophone", "Orchestra/Winds/Saxophone", "Soprano-Sax"), // 64
        Sound::new("Saxophone", "saxophone", "Orchestra/Winds/Saxophone", "Alto-Sax"), // 65
        Sound::new("Saxophone", "saxophone", "Orchestra/Winds/Saxophone", "Tenor-Sax"), // 66
        Sound::new("Saxophone", "saxophone", "Orchestra/Winds/Saxophone", "Baritone-Sax"), // 67
        Sound::new("Oboe", "oboe", "Orchestra/Winds/Other Winds", "Oboe-Solo"), // 68
        Sound::new("English Horn", "englishHorn", "Orchestra/Winds/Other Winds", "EnglishHorn-Solo"), // 69
        Sound::new("Bassoon", "bassoon", "Orchestra/Winds/Bassoon", "Bassoon-Solo"), // 70
        Sound::new("Clarinet", "clarinet", "Orchestra/Winds/Clarinet", "Clarinet-Solo"), // 71
        Sound::new("Piccolo", "piccolo", "Orchestra/Winds/Flute", "Piccolo-Solo"), // 72
        Sound::new("Flute", "flute", "Orchestra/Winds/Flute", "Flute-Solo"), // 73
        Sound::new("Recorder", "recorder", "Orchestra/Winds/Flute", "Recorder-Solo"), // 74
        Sound::new("Flute", "flute", "Orchestra/Winds/Flute", "Flute-Solo"), // 75
        Sound::new("Recorder", "recorder", "Orchestra/Winds/Flute", "Recorder-Solo"), // 76
        Sound::new("Flute", "flute", "Orchestra/Winds/Flute", "Flute-Solo"), // 77
        Sound::new("Recorder", "recorder", "Orchestra/Winds/Flute", "Recorder-Solo"), // 78
        Sound::new("Flute", "flute", "Orchestra/Winds/Flute", "Flute-Solo"), // 79
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Classic-Lead"), // 80
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Classic-Lead"), // 81
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Bottle-Pad"), // 82
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Brassy-Lead"), // 83
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Classic-Lead"), // 84
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Vox-Pad"), // 85
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Classic-Lead"), // 86
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Classic-Lead"), // 87
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Intimacy-Pad"), // 88
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Vox-Pad"), // 89
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Brassy-Lead"), // 90
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Vocal-Pad"), // 91
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Lush-Pad"), // 92
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Lucid-Pad"), // 93
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Vocal-Pad"), // 94
        Sound::new("Pad Synthesizer", "padSynthesizer", "Orchestra/Synth/Pad", "Rosin-Pad"), // 95
        Sound::new("Seq Synthesizer", "seqSynthesizer", "Orchestra/Synth/Sequencer", "Harpy-Seq"), // 96
        Sound::new("Seq Synthesizer", "seqSynthesizer", "Orchestra/Other/Celesta", "Mini-Pad"), // 97
        Sound::new("Seq Synthesizer", "seqSynthesizer", "Orchestra/Synth/Sequencer", "Bottle-Pad"), // 98
        Sound::new("Seq Synthesizer", "seqSynthesizer", "Orchestra/Synth/Sequencer", "Rubber-Lead"), // 99
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Celesta-Tuned"), // 100
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Recorder-Solo"), // 101
        Sound::new("Lead Synthesizer", "leadSynthesizer", "Orchestra/Synth/Lead", "Recorder-Solo"), // 102
        Sound::new("Trumpet", "trumpet", "Orchestra/Synth/Brass", "Simple-Brass"), // 103
        Sound::new("Banjo", "banjo", "Stringed/Electric Guitars/Clean Guitar", "Electric-Sitar"), // 104
        Sound::new("Banjo", "banjo", "Stringed/Other Stringed Instruments/Banjo", "5-Banjo"), // 105
        Sound::new("Ukulele", "ukulele", "Stringed/Other Stringed Instruments/Ukulele", "CC-Ukulele"), // 106
        Sound::new("Banjo", "banjo", "Stringed/Other Stringed Instruments/Banjo", "5-Banjo"), // 107
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 108
        Sound::new("Bassoon", "bassoon", "Orchestra/Keyboard/Acoustic Piano", "Bagpipe"), // 109
        Sound::new("Violin", "violin", "Orchestra/Strings/Violin", "Violin-Solo"), // 110
        Sound::new("Flute", "flute", "Orchestra/Keyboard/Acoustic Piano", "Oboe-Solo"), // 111
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Celesta", "Celesta-Tuned"), // 112
        Sound::new("Celesta", "celesta", "Orchestra/Other/Celesta", "Xylophone-Tuned"), // 113
        Sound::new("Vibraphone", "vibraphone", "Orchestra/Other/Vibraphone", "Vibraphone-Tuned"), // 114
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 115
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 116
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 117
        Sound::new("Xylophone", "xylophone", "Orchestra/Other/Xylophone", "Xylophone-Tuned"), // 118
        Sound::new("Celesta", "celesta", "Orchestra/Other/Celesta", "Celesta-Tuned"), // 119
        Sound::new("Steel Guitar", "steelGuitar", "Stringed/Acoustic Guitars/Steel Guitar", "D-Steel"), // 120
        Sound::new("Recorder", "recorder", "Orchestra/Keyboard/Acoustic Piano", "Recorder-Solo"), // 121
        Sound::new("Recorder", "recorder", "Orchestra/Keyboard/Acoustic Piano", "Recorder-Solo"), // 122
        Sound::new("Recorder", "recorder", "Orchestra/Keyboard/Acoustic Piano", "Recorder-Solo"), // 123
        Sound::new("Recorder", "recorder", "Drums/Idiophone Percussion/Maracas", "Celesta-Tuned"), // 124
        Sound::new("Recorder", "recorder", "Orchestra/Keyboard/Acoustic Piano", "Recorder-Solo"), // 125
        Sound::new("Recorder", "recorder", "Drums/Idiophone Percussion/Hand Clap", "GroupHandClap-Percu"), // 126
        Sound::new("Timpani", "timpani", "Drums/Idiophone Percussion/Maracas", "TimpaniHi-Tuned"), // 127
    ];

    /// Guitar Pro 7's standard drum kit, as Guitar Pro 7.6 writes it for a new
    /// drum track: each element (a drum) and its articulations (ways to play it).
    /// A drum note names its articulation by its place in this list, counted
    /// through every element.
    pub(in super::super) const DRUM_KIT: &[DrumElement] = &[
        DrumElement { name: "Snare", kind: "snare", soundbank: "Master-Snare", articulations: &[
            Articulation { name: "Snare (hit)", staff_line: 3, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 38, rse: "stick.hit.hit", output: 38 },
            Articulation { name: "Snare (side stick)", staff_line: 3, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 37, rse: "stick.hit.sidestick", output: 37 },
            Articulation { name: "Snare (rim shot)", staff_line: 3, noteheads: "noteheadDiamondWhite noteheadDiamondWhite noteheadDiamondWhite", placement: "outside", symbol: "", input: 91, rse: "stick.hit.rimshot", output: 38 },
        ] },
        DrumElement { name: "Charley", kind: "hiHat", soundbank: "Master-Hihat", articulations: &[
            Articulation { name: "Hi-Hat (closed)", staff_line: -1, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 42, rse: "stick.hit.closed", output: 42 },
            Articulation { name: "Hi-Hat (half)", staff_line: -1, noteheads: "noteheadCircleSlash noteheadCircleSlash noteheadCircleSlash", placement: "outside", symbol: "", input: 92, rse: "stick.hit.half", output: 46 },
            Articulation { name: "Hi-Hat (open)", staff_line: -1, noteheads: "noteheadCircleX noteheadCircleX noteheadCircleX", placement: "outside", symbol: "", input: 46, rse: "stick.hit.open", output: 46 },
            Articulation { name: "Pedal Hi-Hat (hit)", staff_line: 9, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 44, rse: "pedal.hit.pedal", output: 44 },
        ] },
        DrumElement { name: "Acoustic Kick Drum", kind: "kickDrum", soundbank: "AcousticKick-Percu", articulations: &[
            Articulation { name: "Kick (hit)", staff_line: 8, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 35, rse: "pedal.hit.hit", output: 35 },
        ] },
        DrumElement { name: "Kick Drum", kind: "kickDrum", soundbank: "Master-Kick", articulations: &[
            Articulation { name: "Kick (hit)", staff_line: 7, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 36, rse: "pedal.hit.hit", output: 36 },
        ] },
        DrumElement { name: "Tom Very High", kind: "tom", soundbank: "Master-Tom05", articulations: &[
            Articulation { name: "High Floor Tom (hit)", staff_line: 1, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 50, rse: "stick.hit.hit", output: 50 },
        ] },
        DrumElement { name: "Tom High", kind: "tom", soundbank: "Master-Tom04", articulations: &[
            Articulation { name: "High Tom (hit)", staff_line: 2, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 48, rse: "stick.hit.hit", output: 48 },
        ] },
        DrumElement { name: "Tom Medium", kind: "tom", soundbank: "Master-Tom03", articulations: &[
            Articulation { name: "Mid Tom (hit)", staff_line: 4, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 47, rse: "stick.hit.hit", output: 47 },
        ] },
        DrumElement { name: "Tom Low", kind: "tom", soundbank: "Master-Tom02", articulations: &[
            Articulation { name: "Low Tom (hit)", staff_line: 5, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 45, rse: "stick.hit.hit", output: 45 },
        ] },
        DrumElement { name: "Tom Very Low", kind: "tom", soundbank: "Master-Tom01", articulations: &[
            Articulation { name: "Very Low Tom (hit)", staff_line: 6, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 43, rse: "stick.hit.hit", output: 43 },
        ] },
        DrumElement { name: "Ride", kind: "ride", soundbank: "Master-Ride", articulations: &[
            Articulation { name: "Ride (edge)", staff_line: 0, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "above", symbol: "pictEdgeOfCymbal", input: 93, rse: "stick.hit.edge", output: 51 },
            Articulation { name: "Ride (middle)", staff_line: 0, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 51, rse: "stick.hit.mid", output: 51 },
            Articulation { name: "Ride (bell)", staff_line: 0, noteheads: "noteheadDiamondWhite noteheadDiamondWhite noteheadDiamondWhite", placement: "outside", symbol: "", input: 53, rse: "stick.hit.bell", output: 53 },
            Articulation { name: "Ride (choke)", staff_line: 0, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "articStaccatoAbove", input: 94, rse: "stick.hit.choke", output: 51 },
        ] },
        DrumElement { name: "Splash", kind: "splash", soundbank: "Master-Splash", articulations: &[
            Articulation { name: "Splash (hit)", staff_line: -2, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 55, rse: "stick.hit.hit", output: 55 },
            Articulation { name: "Splash (choke)", staff_line: -2, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "articStaccatoAbove", input: 95, rse: "stick.hit.choke", output: 55 },
        ] },
        DrumElement { name: "China", kind: "china", soundbank: "Master-China", articulations: &[
            Articulation { name: "China (hit)", staff_line: -3, noteheads: "noteheadHeavyXHat noteheadHeavyXHat noteheadHeavyXHat", placement: "outside", symbol: "", input: 52, rse: "stick.hit.hit", output: 52 },
            Articulation { name: "China (choke)", staff_line: -3, noteheads: "noteheadHeavyXHat noteheadHeavyXHat noteheadHeavyXHat", placement: "outside", symbol: "", input: 96, rse: "stick.hit.choke", output: 52 },
        ] },
        DrumElement { name: "Crash High", kind: "crash", soundbank: "Master-Crash02", articulations: &[
            Articulation { name: "Crash high (hit)", staff_line: -2, noteheads: "noteheadHeavyX noteheadHeavyX noteheadHeavyX", placement: "outside", symbol: "", input: 49, rse: "stick.hit.hit", output: 49 },
            Articulation { name: "Crash high (choke)", staff_line: -2, noteheads: "noteheadHeavyX noteheadHeavyX noteheadHeavyX", placement: "outside", symbol: "articStaccatoAbove", input: 97, rse: "stick.hit.choke", output: 49 },
        ] },
        DrumElement { name: "Crash Medium", kind: "crash", soundbank: "Master-Crash01", articulations: &[
            Articulation { name: "Crash medium (hit)", staff_line: -1, noteheads: "noteheadHeavyX noteheadHeavyX noteheadHeavyX", placement: "outside", symbol: "", input: 57, rse: "stick.hit.hit", output: 57 },
            Articulation { name: "Crash medium (choke)", staff_line: -1, noteheads: "noteheadHeavyX noteheadHeavyX noteheadHeavyX", placement: "outside", symbol: "articStaccatoAbove", input: 98, rse: "stick.hit.choke", output: 57 },
        ] },
        DrumElement { name: "Cowbell Low", kind: "cowbell", soundbank: "CowbellBig-Percu", articulations: &[
            Articulation { name: "Cowbell low (hit)", staff_line: 1, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpHalf noteheadTriangleUpWhole", placement: "outside", symbol: "", input: 99, rse: "stick.hit.hit", output: 56 },
            Articulation { name: "Cowbell low (tip)", staff_line: 1, noteheads: "noteheadXBlack noteheadXHalf noteheadXWhole", placement: "outside", symbol: "", input: 100, rse: "stick.hit.tip", output: 56 },
        ] },
        DrumElement { name: "Cowbell Medium", kind: "cowbell", soundbank: "CowbellMid-Percu", articulations: &[
            Articulation { name: "Cowbell medium (hit)", staff_line: 0, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpHalf noteheadTriangleUpWhole", placement: "outside", symbol: "", input: 56, rse: "stick.hit.hit", output: 56 },
            Articulation { name: "Cowbell medium (tip)", staff_line: 0, noteheads: "noteheadXBlack noteheadXHalf noteheadXWhole", placement: "outside", symbol: "", input: 101, rse: "stick.hit.tip", output: 56 },
        ] },
        DrumElement { name: "Cowbell High", kind: "cowbell", soundbank: "CowbellSmall-Percu", articulations: &[
            Articulation { name: "Cowbell high (hit)", staff_line: -1, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpHalf noteheadTriangleUpWhole", placement: "outside", symbol: "", input: 102, rse: "stick.hit.hit", output: 56 },
            Articulation { name: "Cowbell high (tip)", staff_line: -1, noteheads: "noteheadXBlack noteheadXHalf noteheadXWhole", placement: "outside", symbol: "", input: 103, rse: "stick.hit.tip", output: 56 },
        ] },
        DrumElement { name: "Woodblock Low", kind: "woodblock", soundbank: "WoodblockLow-Percu", articulations: &[
            Articulation { name: "Woodblock low (hit)", staff_line: -9, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpBlack noteheadTriangleUpBlack", placement: "outside", symbol: "", input: 77, rse: "stick.hit.hit", output: 77 },
        ] },
        DrumElement { name: "Woodblock High", kind: "woodblock", soundbank: "WoodblockHigh-Percu", articulations: &[
            Articulation { name: "Woodblock high (hit)", staff_line: -10, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpBlack noteheadTriangleUpBlack", placement: "outside", symbol: "", input: 76, rse: "stick.hit.hit", output: 76 },
        ] },
        DrumElement { name: "Bongo High", kind: "bongo", soundbank: "BongoHigh-Percu", articulations: &[
            Articulation { name: "Bongo High (hit)", staff_line: -4, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 60, rse: "hand.hit.hit", output: 60 },
            Articulation { name: "Bongo High (mute)", staff_line: -5, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "inside", symbol: "noteheadParenthesis", input: 104, rse: "hand.hit.mute", output: 60 },
            Articulation { name: "Bongo High (slap)", staff_line: -6, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 105, rse: "hand.hit.slap", output: 60 },
        ] },
        DrumElement { name: "Bongo Low", kind: "bongo", soundbank: "BongoLow-Percu", articulations: &[
            Articulation { name: "Bongo Low (hit)", staff_line: -7, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 61, rse: "hand.hit.hit", output: 61 },
            Articulation { name: "Bongo Low (mute)", staff_line: -8, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "inside", symbol: "noteheadParenthesis", input: 106, rse: "hand.hit.mute", output: 61 },
            Articulation { name: "Bongo Low (slap)", staff_line: -16, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 107, rse: "hand.hit.slap", output: 61 },
        ] },
        DrumElement { name: "Timbale Low", kind: "timbale", soundbank: "TimbaleLow-Percu", articulations: &[
            Articulation { name: "Timbale low (hit)", staff_line: 10, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 66, rse: "stick.hit.hit", output: 66 },
        ] },
        DrumElement { name: "Timbale High", kind: "timbale", soundbank: "TimbaleHigh-Percu", articulations: &[
            Articulation { name: "Timbale high (hit)", staff_line: 9, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 65, rse: "stick.hit.hit", output: 65 },
        ] },
        DrumElement { name: "Agogo Low", kind: "agogo", soundbank: "AgogoLow-Percu", articulations: &[
            Articulation { name: "Agogo low (hit)", staff_line: 12, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 68, rse: "stick.hit.hit", output: 68 },
        ] },
        DrumElement { name: "Agogo High", kind: "agogo", soundbank: "AgogoHigh-Percu", articulations: &[
            Articulation { name: "Agogo high (hit)", staff_line: 11, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 67, rse: "stick.hit.hit", output: 67 },
        ] },
        DrumElement { name: "Conga Low", kind: "conga", soundbank: "CongaLow-Percu", articulations: &[
            Articulation { name: "Conga low (hit)", staff_line: 17, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 64, rse: "hand.hit.hit", output: 64 },
            Articulation { name: "Conga low (slap)", staff_line: 16, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 108, rse: "hand.hit.slap", output: 64 },
            Articulation { name: "Conga low (mute)", staff_line: 15, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "inside", symbol: "noteheadParenthesis", input: 109, rse: "hand.hit.mute", output: 64 },
        ] },
        DrumElement { name: "Conga High", kind: "conga", soundbank: "CongaHigh-Percu", articulations: &[
            Articulation { name: "Conga high (hit)", staff_line: 14, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 63, rse: "hand.hit.hit", output: 63 },
            Articulation { name: "Conga high (slap)", staff_line: 13, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 110, rse: "hand.hit.slap", output: 63 },
            Articulation { name: "Conga high (mute)", staff_line: 19, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "inside", symbol: "noteheadParenthesis", input: 62, rse: "hand.hit.mute", output: 62 },
        ] },
        DrumElement { name: "Whistle Low", kind: "whistle", soundbank: "WhistleLow-Percu", articulations: &[
            Articulation { name: "Whistle low (hit)", staff_line: -11, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 72, rse: "blow.hit.hit", output: 72 },
        ] },
        DrumElement { name: "Whistle High", kind: "whistle", soundbank: "WhistleHigh-Percu", articulations: &[
            Articulation { name: "Whistle high (hit)", staff_line: -17, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 71, rse: "blow.hit.hit", output: 71 },
        ] },
        DrumElement { name: "Guiro", kind: "guiro", soundbank: "Guiro-Percu", articulations: &[
            Articulation { name: "Guiro (hit)", staff_line: 38, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 73, rse: "stick.hit.hit", output: 73 },
            Articulation { name: "Guiro (scrap-return)", staff_line: 37, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 74, rse: "stick.scrape.return", output: 74 },
        ] },
        DrumElement { name: "Surdo", kind: "surdo", soundbank: "Surdo-Percu", articulations: &[
            Articulation { name: "Surdo (hit)", staff_line: 36, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 86, rse: "brush.hit.hit", output: 86 },
            Articulation { name: "Surdo (mute)", staff_line: 35, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "inside", symbol: "noteheadParenthesis", input: 87, rse: "brush.hit.mute", output: 87 },
        ] },
        DrumElement { name: "Tambourine", kind: "tambourine", soundbank: "Tambourine-Percu", articulations: &[
            Articulation { name: "Tambourine (hit)", staff_line: 3, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpBlack noteheadTriangleUpBlack", placement: "outside", symbol: "", input: 54, rse: "hand.hit.hit", output: 54 },
            Articulation { name: "Tambourine (return)", staff_line: 2, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpBlack noteheadTriangleUpBlack", placement: "above", symbol: "stringsUpBow", input: 111, rse: "hand.hit.return", output: 54 },
            Articulation { name: "Tambourine (roll)", staff_line: 1, noteheads: "noteheadTriangleUpBlack noteheadTriangleUpBlack noteheadTriangleUpBlack", placement: "above", symbol: "stringsDownBow", input: 112, rse: "hand.hit.roll", output: 54 },
            Articulation { name: "Tambourine (hand)", staff_line: -7, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 113, rse: "hand.hit.handhit", output: 54 },
        ] },
        DrumElement { name: "Cuica", kind: "cuica", soundbank: "Cuica-Percu", articulations: &[
            Articulation { name: "Cuica (open)", staff_line: 30, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 79, rse: "hand.hit.hit", output: 79 },
            Articulation { name: "Cuica (mute)", staff_line: 29, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 78, rse: "hand.hit.mute", output: 78 },
        ] },
        DrumElement { name: "Vibraslap", kind: "vibraslap", soundbank: "Vibraslap-Percu", articulations: &[
            Articulation { name: "Vibraslap (hit)", staff_line: 28, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 58, rse: "hand.hit.hit", output: 58 },
        ] },
        DrumElement { name: "Triangle", kind: "triangle", soundbank: "Triangle-Percu", articulations: &[
            Articulation { name: "Triangle (hit)", staff_line: 27, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 81, rse: "stick.hit.hit", output: 81 },
            Articulation { name: "Triangle (mute)", staff_line: 26, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "inside", symbol: "noteheadParenthesis", input: 80, rse: "stick.hit.mute", output: 80 },
        ] },
        DrumElement { name: "Grancassa", kind: "grancassa", soundbank: "Grancassa-Percu", articulations: &[
            Articulation { name: "Grancassa (hit)", staff_line: 25, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 114, rse: "mallet.hit.hit", output: 43 },
        ] },
        DrumElement { name: "Piatti", kind: "piatti", soundbank: "Piatti-Percu", articulations: &[
            Articulation { name: "Piatti (hit)", staff_line: 18, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 115, rse: "hand.hit.hit", output: 49 },
            Articulation { name: "Piatti (hand)", staff_line: 24, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 116, rse: "hand.hit.hit", output: 49 },
        ] },
        DrumElement { name: "Cabasa", kind: "cabasa", soundbank: "Cabasa-Percu", articulations: &[
            Articulation { name: "Cabasa (hit)", staff_line: 23, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 69, rse: "hand.hit.hit", output: 69 },
            Articulation { name: "Cabasa (return)", staff_line: 22, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "stringsUpBow", input: 117, rse: "hand.hit.return", output: 69 },
        ] },
        DrumElement { name: "Castanets", kind: "castanets", soundbank: "Castanets-Percu", articulations: &[
            Articulation { name: "Castanets (hit)", staff_line: 21, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 85, rse: "hand.hit.hit", output: 85 },
        ] },
        DrumElement { name: "Claves", kind: "claves", soundbank: "Claves-Percu", articulations: &[
            Articulation { name: "Claves (hit)", staff_line: 20, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 75, rse: "stick.hit.hit", output: 75 },
        ] },
        DrumElement { name: "Left Maraca", kind: "maraca", soundbank: "Maracas-Percu", articulations: &[
            Articulation { name: "Left Maraca (hit)", staff_line: -12, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 70, rse: "hand.hit.hit", output: 70 },
            Articulation { name: "Left Maraca (return)", staff_line: -13, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "stringsUpBow", input: 118, rse: "hand.hit.return", output: 70 },
        ] },
        DrumElement { name: "Right Maraca", kind: "maraca", soundbank: "Maracas-Percu", articulations: &[
            Articulation { name: "Right Maraca (hit)", staff_line: -14, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 119, rse: "hand.hit.hit", output: 70 },
            Articulation { name: "Right Maraca (return)", staff_line: -15, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "stringsUpBow", input: 120, rse: "hand.hit.return", output: 70 },
        ] },
        DrumElement { name: "Shaker", kind: "shaker", soundbank: "ShakerStudio-Percu", articulations: &[
            Articulation { name: "Shaker (hit)", staff_line: -23, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 82, rse: "hand.hit.hit", output: 54 },
            Articulation { name: "Shaker (return)", staff_line: -24, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "stringsUpBow", input: 122, rse: "hand.hit.return", output: 54 },
        ] },
        DrumElement { name: "Bell Tree", kind: "bellTree", soundbank: "BellTree-Percu", articulations: &[
            Articulation { name: "Bell Tree (hit)", staff_line: -18, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 84, rse: "stick.hit.hit", output: 53 },
            Articulation { name: "Bell Tree (return)", staff_line: -19, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "stringsUpBow", input: 123, rse: "stick.hit.return", output: 53 },
        ] },
        DrumElement { name: "Jingle Bell", kind: "jingleBell", soundbank: "JingleBell-Percu", articulations: &[
            Articulation { name: "Jingle Bell (hit)", staff_line: -20, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 83, rse: "stick.hit.hit", output: 53 },
        ] },
        DrumElement { name: "Tinkle Bell", kind: "jingleBell", soundbank: "JingleBell-Percu", articulations: &[
            Articulation { name: "Tinkle Bell (hit)", staff_line: -20, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 83, rse: "stick.hit.hit", output: 53 },
        ] },
        DrumElement { name: "Golpe", kind: "unpitched", soundbank: "Golpe-Percu", articulations: &[
            Articulation { name: "Golpe (thumb)", staff_line: -21, noteheads: "noteheadNull noteheadNull noteheadNull", placement: "below", symbol: "guitarGolpe", input: 124, rse: "thumb.hit.body", output: 62 },
            Articulation { name: "Golpe (finger)", staff_line: -22, noteheads: "noteheadNull noteheadNull noteheadNull", placement: "above", symbol: "guitarGolpe", input: 125, rse: "finger4.hit.body", output: 62 },
        ] },
        DrumElement { name: "Hand Clap", kind: "handClap", soundbank: "GroupHandClap-Percu", articulations: &[
            Articulation { name: "Hand Clap (hit)", staff_line: 3, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 39, rse: "hand.hit.hit", output: 39 },
        ] },
        DrumElement { name: "Electric Snare", kind: "snare", soundbank: "ElectricSnare-Percu", articulations: &[
            Articulation { name: "Electric Snare (hit)", staff_line: 3, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 40, rse: "stick.hit.hit", output: 40 },
        ] },
        DrumElement { name: "Sticks", kind: "snare", soundbank: "Stick-Percu", articulations: &[
            Articulation { name: "Snare (side stick)", staff_line: 3, noteheads: "noteheadSlashedBlack2 noteheadSlashedBlack2 noteheadSlashedBlack2", placement: "outside", symbol: "", input: 31, rse: "stick.hit.sidestick", output: 40 },
        ] },
        DrumElement { name: "Very Low Floor Tom", kind: "tom", soundbank: "LowFloorTom-Percu", articulations: &[
            Articulation { name: "Low Floor Tom (hit)", staff_line: 5, noteheads: "noteheadBlack noteheadHalf noteheadWhole", placement: "outside", symbol: "", input: 41, rse: "stick.hit.hit", output: 41 },
        ] },
        DrumElement { name: "Ride Cymbal 2", kind: "ride", soundbank: "Ride-Percu", articulations: &[
            Articulation { name: "Ride (edge)", staff_line: 2, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "above", symbol: "pictEdgeOfCymbal", input: 59, rse: "stick.hit.edge", output: 59 },
            Articulation { name: "Ride (middle)", staff_line: 2, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 126, rse: "stick.hit.mid", output: 59 },
            Articulation { name: "Ride (bell)", staff_line: 2, noteheads: "noteheadDiamondWhite noteheadDiamondWhite noteheadDiamondWhite", placement: "outside", symbol: "", input: 127, rse: "stick.hit.bell", output: 59 },
            Articulation { name: "Ride (choke)", staff_line: 2, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "articStaccatoAbove", input: 29, rse: "stick.hit.choke", output: 59 },
        ] },
        DrumElement { name: "Reverse Cymbal", kind: "crash", soundbank: "Reverse-Cymbal", articulations: &[
            Articulation { name: "Reverse Cymbal (hit)", staff_line: -3, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 30, rse: "stick.hit.hit", output: 49 },
        ] },
        DrumElement { name: "Metronome", kind: "snare", soundbank: "Metronome-Percu", articulations: &[
            Articulation { name: "Metronome (hit)", staff_line: 3, noteheads: "noteheadXBlack noteheadXBlack noteheadXBlack", placement: "outside", symbol: "", input: 33, rse: "stick.hit.sidestick", output: 37 },
            Articulation { name: "Metronome (bell)", staff_line: 3, noteheads: "noteheadBlack noteheadBlack noteheadBlack", placement: "outside", symbol: "", input: 34, rse: "stick.hit.hit", output: 38 },
        ] },
    ];
}

pub(super) use generated::{DRUM_KIT, GM_SOUNDS};
