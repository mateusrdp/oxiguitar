// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Converting between the model and the score as it crosses the plugin
//! interface: version 1 (`oxiguitar_plugin_api::score`) and version 2
//! (`score2`, interface 1.2), which is version 1 with lists of extensions.
//!
//! Version 2 has room for the whole model, so nothing is lost either way
//! (`tests/wire.rs` in oxiguitar-guitarpro checks this on every fixture);
//! version 1 lacks what the model gained since (free time).
//! Every model struct is taken apart field by field (and every wire struct
//! built field by field), so a field added to either doesn't compile here until someone decides how it crosses the
//! interface: in a new version of the wire score, never by changing
//! version 1.

use oxiguitar_plugin_api::score as w;
use oxiguitar_plugin_api::score2 as w2;

use crate::*;

/// The wire form of `score`, for a plugin. Indices that don't fit in 32
/// bits (no real score has 4 billion chord diagrams) become `u32::MAX`,
/// which points at nothing.
pub fn to_wire(score: &Score) -> w::Score {
    let Score {
        info,
        initial_tempo,
        tempo_label,
        master_bars,
        tracks,
        // Version 2's extension (to_wire2).
        layout: _,
        track_bar_colors: _,
    } = score;
    w::Score {
        info: info_to(info),
        initial_tempo: *initial_tempo,
        tempo_label: tempo_label.clone(),
        master_bars: master_bars.iter().map(master_bar_to).collect(),
        tracks: tracks.iter().map(track_to).collect(),
    }
}

/// The model form of a score a plugin sent. Fails only on a fraction with
/// a zero denominator, or one too large to reduce; the caller should then
/// check the score's structure ([`Score::check_structure`]) too.
pub fn from_wire(score: w::Score) -> Result<Score, String> {
    let w::Score {
        info,
        initial_tempo,
        tempo_label,
        master_bars,
        tracks,
    } = score;
    Ok(Score {
        info: info_from(info),
        initial_tempo,
        tempo_label,
        master_bars: master_bars
            .into_iter()
            .enumerate()
            .map(|(i, b)| master_bar_from(b).map_err(|e| format!("bar {}: {e}", i + 1)))
            .collect::<Result<_, _>>()?,
        tracks: tracks
            .into_iter()
            .enumerate()
            .map(|(i, t)| track_from(t).map_err(|e| format!("track {}: {e}", i + 1)))
            .collect::<Result<_, _>>()?,
        layout: Vec::new(),
        track_bar_colors: std::collections::BTreeMap::new(),
    })
}

/// The wire form of `score`, version 2 (interface 1.2): version 1, with
/// what the model holds beyond it as extensions.
pub fn to_wire2(score: &Score) -> w2::Score2 {
    let mut out = w2::Score2::from_v1(to_wire(score));
    if !score.layout.is_empty() {
        out.ext.push(w2::ScoreExt::Layout(score.layout.clone()));
    }
    for (m, w) in score.master_bars.iter().zip(&mut out.master_bars) {
        if m.free_time {
            w.ext.push(w2::MasterBarExt::FreeTime);
        }
        for f in &m.fermatas {
            w.ext.push(w2::MasterBarExt::Fermata(fermata_to(f)));
        }
    }
    for (t, wt) in score.tracks.iter().zip(&mut out.tracks) {
        if !t.layout.is_empty() {
            wt.ext.push(w2::TrackExt::Layout(t.layout.clone()));
        }
        if t.chords_from != usize::MAX {
            wt.ext.push(w2::TrackExt::ChordsFrom(index_to(t.chords_from)));
        }
        for (s, ws) in t.staves.iter().zip(&mut wt.staves) {
            for (b, wb) in s.bars.iter().zip(&mut ws.bars) {
                if let Some(simile) = b.simile {
                    wb.ext.push(w2::BarExt::Simile(simile_to(simile)));
                }
                for (v, wv) in b.voices.iter().zip(&mut wb.voices) {
                    for (beat, wbeat) in v.beats.iter().zip(&mut wv.beats) {
                        wbeat.ext = beat_ext_to(beat);
                        for (n, wn) in beat.notes.iter().zip(&mut wbeat.notes) {
                            wn.ext = note_ext_to(n);
                        }
                    }
                }
            }
        }
    }
    out
}

/// The model form of a score, version 2, a plugin sent: as [`from_wire`],
/// with the extensions.
pub fn from_wire2(score: w2::Score2) -> Result<Score, String> {
    let mut out = from_wire(score.to_v1())?;
    for e in &score.ext {
        match e {
            w2::ScoreExt::Layout(l) => out.layout.clone_from(l),
        }
    }
    for (m, w) in out.master_bars.iter_mut().zip(&score.master_bars) {
        for e in &w.ext {
            match e {
                w2::MasterBarExt::FreeTime => m.free_time = true,
                w2::MasterBarExt::Fermata(f) => m.fermatas.push(fermata_from(f)?),
            }
        }
    }
    for (t, wt) in out.tracks.iter_mut().zip(&score.tracks) {
        for e in &wt.ext {
            match e {
                w2::TrackExt::Layout(l) => t.layout.clone_from(l),
                w2::TrackExt::ChordsFrom(i) => t.chords_from = index_from(*i),
            }
        }
        for (s, ws) in t.staves.iter_mut().zip(&wt.staves) {
            for (b, wb) in s.bars.iter_mut().zip(&ws.bars) {
                for e in &wb.ext {
                    match e {
                        w2::BarExt::Simile(simile) => b.simile = Some(simile_from(*simile)),
                    }
                }
                for (v, wv) in b.voices.iter_mut().zip(&wb.voices) {
                    for (beat, wbeat) in v.beats.iter_mut().zip(&wv.beats) {
                        beat_ext_from(beat, &wbeat.ext);
                        for (n, wn) in beat.notes.iter_mut().zip(&wbeat.notes) {
                            note_ext_from(n, &wn.ext);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn fermata_to(f: &Fermata) -> w2::Fermata {
    // Guitar Pro's length, to a thousandth.
    let thousandths = (f64::from(f.length) * 1000.0).round();
    w2::Fermata {
        position: ratio_to(f.position),
        kind: match f.kind {
            FermataKind::Short => w2::FermataKind::Short,
            FermataKind::Medium => w2::FermataKind::Medium,
            FermataKind::Long => w2::FermataKind::Long,
        },
        length: w::Ratio {
            num: thousandths as i64,
            den: 1000,
        },
    }
}

fn fermata_from(f: &w2::Fermata) -> Result<Fermata, String> {
    let length = ratio_from(f.length)?;
    Ok(Fermata {
        position: ratio_from(f.position)?,
        kind: match f.kind {
            w2::FermataKind::Short => FermataKind::Short,
            w2::FermataKind::Medium => FermataKind::Medium,
            w2::FermataKind::Long => FermataKind::Long,
        },
        length: length.to_f64() as f32,
    })
}

fn simile_to(s: Simile) -> w2::Simile {
    match s {
        Simile::Simple => w2::Simile::Simple,
        Simile::FirstOfDouble => w2::Simile::FirstOfDouble,
        Simile::SecondOfDouble => w2::Simile::SecondOfDouble,
    }
}

fn simile_from(s: w2::Simile) -> Simile {
    match s {
        w2::Simile::Simple => Simile::Simple,
        w2::Simile::FirstOfDouble => Simile::FirstOfDouble,
        w2::Simile::SecondOfDouble => Simile::SecondOfDouble,
    }
}

/// The two pairs of Rasgueado, model and wire, in the same order.
const RASGUEADOS: [(Rasgueado, w2::Rasgueado); 18] = [
    (Rasgueado::Ii, w2::Rasgueado::Ii),
    (Rasgueado::Mi, w2::Rasgueado::Mi),
    (Rasgueado::MiiTriplet, w2::Rasgueado::MiiTriplet),
    (Rasgueado::MiiAnapaest, w2::Rasgueado::MiiAnapaest),
    (Rasgueado::PmpTriplet, w2::Rasgueado::PmpTriplet),
    (Rasgueado::PmpAnapaest, w2::Rasgueado::PmpAnapaest),
    (Rasgueado::PeiTriplet, w2::Rasgueado::PeiTriplet),
    (Rasgueado::PeiAnapaest, w2::Rasgueado::PeiAnapaest),
    (Rasgueado::PaiTriplet, w2::Rasgueado::PaiTriplet),
    (Rasgueado::PaiAnapaest, w2::Rasgueado::PaiAnapaest),
    (Rasgueado::AmiTriplet, w2::Rasgueado::AmiTriplet),
    (Rasgueado::AmiAnapaest, w2::Rasgueado::AmiAnapaest),
    (Rasgueado::Ppp, w2::Rasgueado::Ppp),
    (Rasgueado::Amii, w2::Rasgueado::Amii),
    (Rasgueado::Amip, w2::Rasgueado::Amip),
    (Rasgueado::Eami, w2::Rasgueado::Eami),
    (Rasgueado::Eamii, w2::Rasgueado::Eamii),
    (Rasgueado::Peami, w2::Rasgueado::Peami),
];

/// What a beat holds beyond version 1, as extensions.
fn beat_ext_to(b: &Beat) -> Vec<w2::BeatExt> {
    use w2::BeatExt as E;
    let mut out = Vec::new();
    if b.slur {
        out.push(E::Slur);
    }
    if let Some(h) = b.hairpin {
        out.push(E::Hairpin(match h {
            Hairpin::Crescendo => w2::Hairpin::Crescendo,
            Hairpin::Decrescendo => w2::Hairpin::Decrescendo,
        }));
    }
    if let Some(o) = b.ottava {
        out.push(E::Ottava(match o {
            Ottava::Alta8 => w2::Ottava::Alta8,
            Ottava::Bassa8 => w2::Ottava::Bassa8,
            Ottava::Alta15 => w2::Ottava::Alta15,
            Ottava::Bassa15 => w2::Ottava::Bassa15,
        }));
    }
    if b.slash {
        out.push(E::Slash);
    }
    match b.beaming {
        Beaming::Automatic => {}
        Beaming::Force => out.push(E::Beaming(w2::Beaming::Force)),
        Beaming::Break => out.push(E::Beaming(w2::Beaming::Break)),
        Beaming::BreakSecondary => out.push(E::Beaming(w2::Beaming::BreakSecondary)),
    }
    if b.tuplet_group {
        out.push(E::TupletGroup);
    }
    if let Some(Barre { fret, half }) = b.barre {
        out.push(E::Barre { fret, half });
    }
    let fx = &b.effects;
    match fx.fade {
        Some(Fade::Out) => out.push(E::FadeOut),
        Some(Fade::Swell) => out.push(E::VolumeSwell),
        // A fade in is version 1's.
        Some(Fade::In) | None => {}
    }
    if fx.bridge_vibrato == Some(Vibrato::Slight) {
        out.push(E::SlightBridgeVibrato);
    }
    if let Some(p) = fx.pedal {
        out.push(E::Pedal(match p {
            Pedal::Down => w2::Pedal::Down,
            Pedal::Up => w2::Pedal::Up,
        }));
    }
    if let Some(wah) = fx.wah {
        out.push(E::Wah(match wah {
            Wah::Open => w2::Wah::Open,
            Wah::Closed => w2::Wah::Closed,
        }));
    }
    if let Some(d) = fx.arpeggio {
        out.push(E::Arpeggio(stroke_direction_to(d)));
    }
    if let Some(r) = fx.rasgueado
        && let Some((_, w)) = RASGUEADOS.iter().find(|(m, _)| *m == r)
    {
        out.push(E::Rasgueado(*w));
    }
    if let Some(g) = fx.golpe {
        out.push(E::Golpe(match g {
            Golpe::Finger => w2::Golpe::Finger,
            Golpe::Thumb => w2::Golpe::Thumb,
        }));
    }
    if let Some(d) = fx.pick_scrape {
        out.push(E::PickScrape(stroke_direction_to(d)));
    }
    if fx.dead_slap {
        out.push(E::DeadSlap);
    }
    out
}

/// Sets what a beat's extensions hold.
fn beat_ext_from(b: &mut Beat, ext: &[w2::BeatExt]) {
    use w2::BeatExt as E;
    for e in ext {
        match *e {
            E::Slur => b.slur = true,
            E::Hairpin(h) => {
                b.hairpin = Some(match h {
                    w2::Hairpin::Crescendo => Hairpin::Crescendo,
                    w2::Hairpin::Decrescendo => Hairpin::Decrescendo,
                });
            }
            E::Ottava(o) => {
                b.ottava = Some(match o {
                    w2::Ottava::Alta8 => Ottava::Alta8,
                    w2::Ottava::Bassa8 => Ottava::Bassa8,
                    w2::Ottava::Alta15 => Ottava::Alta15,
                    w2::Ottava::Bassa15 => Ottava::Bassa15,
                });
            }
            E::Slash => b.slash = true,
            E::Beaming(m) => {
                b.beaming = match m {
                    w2::Beaming::Force => Beaming::Force,
                    w2::Beaming::Break => Beaming::Break,
                    w2::Beaming::BreakSecondary => Beaming::BreakSecondary,
                };
            }
            E::TupletGroup => b.tuplet_group = true,
            E::Barre { fret, half } => b.barre = Some(Barre { fret, half }),
            E::FadeOut => b.effects.fade = Some(Fade::Out),
            E::VolumeSwell => b.effects.fade = Some(Fade::Swell),
            E::SlightBridgeVibrato => b.effects.bridge_vibrato = Some(Vibrato::Slight),
            E::Pedal(p) => {
                b.effects.pedal = Some(match p {
                    w2::Pedal::Down => Pedal::Down,
                    w2::Pedal::Up => Pedal::Up,
                });
            }
            E::Wah(wah) => {
                b.effects.wah = Some(match wah {
                    w2::Wah::Open => Wah::Open,
                    w2::Wah::Closed => Wah::Closed,
                });
            }
            E::Arpeggio(d) => b.effects.arpeggio = Some(stroke_direction_from(d)),
            E::Rasgueado(r) => {
                b.effects.rasgueado = RASGUEADOS.iter().find(|(_, w)| *w == r).map(|(m, _)| *m);
            }
            E::Golpe(g) => {
                b.effects.golpe = Some(match g {
                    w2::Golpe::Finger => Golpe::Finger,
                    w2::Golpe::Thumb => Golpe::Thumb,
                });
            }
            E::PickScrape(d) => b.effects.pick_scrape = Some(stroke_direction_from(d)),
            E::DeadSlap => b.effects.dead_slap = true,
        }
    }
}

/// What a note holds beyond version 1, as extensions.
fn note_ext_to(n: &Note) -> Vec<w2::NoteExt> {
    use w2::NoteExt as E;
    let mut out = Vec::new();
    if n.effects.vibrato == Some(Vibrato::Wide) {
        out.push(E::WideVibrato);
    }
    if let Some(o) = n.effects.ornament {
        out.push(E::Ornament(match o {
            Ornament::UpperMordent => w2::Ornament::UpperMordent,
            Ornament::LowerMordent => w2::Ornament::LowerMordent,
            Ornament::Turn => w2::Ornament::Turn,
            Ornament::InvertedTurn => w2::Ornament::InvertedTurn,
        }));
    }
    if n.effects.string_number {
        out.push(E::StringNumber);
    }
    if let Some(Spelling { step, alter }) = n.spelling {
        out.push(E::Spelling { step, alter });
    }
    out
}

/// Sets what a note's extensions hold.
fn note_ext_from(n: &mut Note, ext: &[w2::NoteExt]) {
    use w2::NoteExt as E;
    for e in ext {
        match *e {
            E::WideVibrato => n.effects.vibrato = Some(Vibrato::Wide),
            E::Ornament(o) => {
                n.effects.ornament = Some(match o {
                    w2::Ornament::UpperMordent => Ornament::UpperMordent,
                    w2::Ornament::LowerMordent => Ornament::LowerMordent,
                    w2::Ornament::Turn => Ornament::Turn,
                    w2::Ornament::InvertedTurn => Ornament::InvertedTurn,
                });
            }
            E::StringNumber => n.effects.string_number = true,
            E::Spelling { step, alter } => {
                let s = Spelling { step, alter };
                n.spelling = (step < 7 && (-2..=2).contains(&alter)).then_some(s);
            }
        }
    }
}

fn index_to(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn index_from(i: u32) -> usize {
    usize::try_from(i).unwrap_or(usize::MAX)
}

fn ratio_to(r: Ratio) -> w::Ratio {
    w::Ratio {
        num: r.numer(),
        den: r.denom(),
    }
}

fn ratio_from(r: w::Ratio) -> Result<Ratio, String> {
    Ratio::new(r.num, r.den).ok_or_else(|| format!("{}/{} is not a valid fraction", r.num, r.den))
}

fn info_to(i: &ScoreInfo) -> w::ScoreInfo {
    let ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    } = i;
    w::ScoreInfo {
        title: title.clone(),
        subtitle: subtitle.clone(),
        artist: artist.clone(),
        album: album.clone(),
        words: words.clone(),
        music: music.clone(),
        copyright: copyright.clone(),
        transcriber: transcriber.clone(),
        instructions: instructions.clone(),
        notices: notices.clone(),
    }
}

fn info_from(i: w::ScoreInfo) -> ScoreInfo {
    let w::ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    } = i;
    ScoreInfo {
        title,
        subtitle,
        artist,
        album,
        words,
        music,
        copyright,
        transcriber,
        instructions,
        notices,
    }
}

fn master_bar_to(b: &MasterBar) -> w::MasterBar {
    let MasterBar {
        time_signature,
        key_signature,
        repeat_start,
        repeat_end,
        alternate_endings,
        marker,
        double_bar,
        triplet_feel,
        tempo_changes,
        directions,
        // Not in version 1: extensions of version 2 (to_wire2).
        free_time: _,
        fermatas: _,
        // Not yet in the wire format.
        bar_color: _,
        track_bar_color: _,
    } = b;
    w::MasterBar {
        time_signature: w::TimeSignature {
            numerator: time_signature.numerator,
            denominator: time_signature.denominator,
        },
        key_signature: w::KeySignature {
            fifths: key_signature.fifths,
            mode: match key_signature.mode {
                KeyMode::Major => w::KeyMode::Major,
                KeyMode::Minor => w::KeyMode::Minor,
            },
        },
        repeat_start: *repeat_start,
        repeat_end: *repeat_end,
        alternate_endings: *alternate_endings,
        marker: marker.as_ref().map(
            |Marker {
                 letter,
                 text,
                 color,
             }| w::Marker {
                letter: letter.clone(),
                text: text.clone(),
                color: color_to(*color),
            },
        ),
        double_bar: *double_bar,
        triplet_feel: match triplet_feel {
            TripletFeel::None => w::TripletFeel::None,
            TripletFeel::Triplet8th => w::TripletFeel::Triplet8th,
            TripletFeel::Triplet16th => w::TripletFeel::Triplet16th,
            TripletFeel::Dotted8th => w::TripletFeel::Dotted8th,
            TripletFeel::Dotted16th => w::TripletFeel::Dotted16th,
            TripletFeel::Scottish8th => w::TripletFeel::Scottish8th,
            TripletFeel::Scottish16th => w::TripletFeel::Scottish16th,
        },
        tempo_changes: tempo_changes
            .iter()
            .map(
                |TempoChange {
                     position,
                     bpm,
                     label,
                     linear,
                 }| w::TempoChange {
                    position: ratio_to(*position),
                    bpm: *bpm,
                    label: label.clone(),
                    linear: *linear,
                },
            )
            .collect(),
        directions: directions.iter().map(|&d| direction_to(d)).collect(),
    }
}

fn master_bar_from(b: w::MasterBar) -> Result<MasterBar, String> {
    let w::MasterBar {
        time_signature,
        key_signature,
        repeat_start,
        repeat_end,
        alternate_endings,
        marker,
        double_bar,
        triplet_feel,
        tempo_changes,
        directions,
    } = b;
    Ok(MasterBar {
        time_signature: TimeSignature {
            numerator: time_signature.numerator,
            denominator: time_signature.denominator,
        },
        key_signature: KeySignature {
            fifths: key_signature.fifths,
            mode: match key_signature.mode {
                w::KeyMode::Major => KeyMode::Major,
                w::KeyMode::Minor => KeyMode::Minor,
            },
        },
        repeat_start,
        repeat_end,
        alternate_endings,
        marker: marker.map(|m| Marker {
            letter: m.letter,
            text: m.text,
            color: color_from(m.color),
        }),
        double_bar,
        triplet_feel: match triplet_feel {
            w::TripletFeel::None => TripletFeel::None,
            w::TripletFeel::Triplet8th => TripletFeel::Triplet8th,
            w::TripletFeel::Triplet16th => TripletFeel::Triplet16th,
            w::TripletFeel::Dotted8th => TripletFeel::Dotted8th,
            w::TripletFeel::Dotted16th => TripletFeel::Dotted16th,
            w::TripletFeel::Scottish8th => TripletFeel::Scottish8th,
            w::TripletFeel::Scottish16th => TripletFeel::Scottish16th,
        },
        tempo_changes: tempo_changes
            .into_iter()
            .map(|t| {
                Ok(TempoChange {
                    position: ratio_from(t.position)?,
                    bpm: t.bpm,
                    label: t.label,
                    linear: t.linear,
                })
            })
            .collect::<Result<_, String>>()?,
        directions: directions.into_iter().map(direction_from).collect(),
        // Version 2's extensions (from_wire2).
        free_time: false,
        fermatas: Vec::new(),
        bar_color: None,
        track_bar_color: None,
    })
}

fn color_to(c: Color) -> w::Color {
    w::Color {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

fn color_from(c: w::Color) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

/// Pairs each model direction with its wire twin; one table for both ways.
macro_rules! directions {
    ($($name:ident),* $(,)?) => {
        fn direction_to(d: Direction) -> w::Direction {
            match d {
                $(Direction::$name => w::Direction::$name,)*
            }
        }

        fn direction_from(d: w::Direction) -> Direction {
            match d {
                $(w::Direction::$name => Direction::$name,)*
            }
        }
    };
}

directions!(
    Segno,
    SegnoSegno,
    Coda,
    DoubleCoda,
    Fine,
    ToCoda,
    ToDoubleCoda,
    DaCapo,
    DaCapoAlCoda,
    DaCapoAlDoubleCoda,
    DaCapoAlFine,
    DalSegno,
    DalSegnoAlCoda,
    DalSegnoAlDoubleCoda,
    DalSegnoAlFine,
    DalSegnoSegno,
    DalSegnoSegnoAlCoda,
    DalSegnoSegnoAlDoubleCoda,
    DalSegnoSegnoAlFine,
);

fn track_to(t: &Track) -> w::Track {
    let Track {
        name,
        short_name,
        color,
        instrument,
        mix,
        staves,
        chords,
        lyrics,
        // Plugins' settings for the track stay in the core: saved beside
        // the score (FILE.oxg), handed to a plugin only with its own
        // requests (interface 1.3); the order of the track's effects too.
        plugins: _,
        effects: _,
        play_chords: _,
        // Version 2's extension (to_wire2).
        layout: _,
        // Cross-referenced chords from another track.
        chords_from: _,
    } = t;
    let Instrument {
        midi_program,
        percussion,
    } = *instrument;
    let Mix {
        volume,
        pan,
        mute,
        solo,
    } = *mix;
    w::Track {
        name: name.clone(),
        short_name: short_name.clone(),
        color: color_to(*color),
        instrument: w::Instrument {
            midi_program,
            percussion,
        },
        mix: w::Mix {
            volume,
            pan,
            mute,
            solo,
        },
        staves: staves.iter().map(staff_to).collect(),
        chords: chords
            .iter()
            .map(
                |ChordDiagram {
                     name,
                     first_fret,
                     frets,
                 }| w::ChordDiagram {
                    name: name.clone(),
                    first_fret: *first_fret,
                    frets: frets.clone(),
                },
            )
            .collect(),
        lyrics: lyrics
            .iter()
            .map(|LyricLine { start_bar, text }| w::LyricLine {
                start_bar: index_to(*start_bar),
                text: text.clone(),
            })
            .collect(),
    }
}

fn track_from(t: w::Track) -> Result<Track, String> {
    let w::Track {
        name,
        short_name,
        color,
        instrument,
        mix,
        staves,
        chords,
        lyrics,
    } = t;
    Ok(Track {
        name,
        short_name,
        color: color_from(color),
        instrument: Instrument {
            midi_program: instrument.midi_program,
            percussion: instrument.percussion,
        },
        mix: Mix {
            volume: mix.volume,
            pan: mix.pan,
            mute: mix.mute,
            solo: mix.solo,
        },
        staves: staves
            .into_iter()
            .map(staff_from)
            .collect::<Result<_, _>>()?,
        chords: chords
            .into_iter()
            .map(|c| ChordDiagram {
                name: c.name,
                first_fret: c.first_fret,
                frets: c.frets,
            })
            .collect(),
        lyrics: lyrics
            .into_iter()
            .map(|l| LyricLine {
                start_bar: index_from(l.start_bar),
                text: l.text,
            })
            .collect(),
        plugins: Default::default(),
        effects: Vec::new(),
        play_chords: false,
        layout: Vec::new(),
        chords_from: usize::MAX,
    })
}

fn staff_to(s: &Staff) -> w::Staff {
    let Staff {
        tuning,
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars,
    } = s;
    w::Staff {
        tuning: tuning.iter().map(|p| w::Pitch(p.0)).collect(),
        capo: *capo,
        fret_count: *fret_count,
        transposition: *transposition,
        show_standard: *show_standard,
        show_tablature: *show_tablature,
        bars: bars
            .iter()
            .map(|Bar { clef, voices, .. }| w::Bar {
                clef: clef_to(*clef),
                voices: voices
                    .iter()
                    .map(|Voice { beats }| w::Voice {
                        beats: beats.iter().map(beat_to).collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn staff_from(s: w::Staff) -> Result<Staff, String> {
    let w::Staff {
        tuning,
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars,
    } = s;
    Ok(Staff {
        tuning: tuning.into_iter().map(|p| Pitch(p.0)).collect(),
        capo,
        fret_count,
        transposition,
        show_standard,
        show_tablature,
        bars: bars
            .into_iter()
            .enumerate()
            .map(|(i, b)| {
                Ok(Bar {
                    simile: None,
                    clef: clef_from(b.clef),
                    voices: b
                        .voices
                        .into_iter()
                        .map(|v| {
                            Ok(Voice {
                                beats: v
                                    .beats
                                    .into_iter()
                                    .map(beat_from)
                                    .collect::<Result<_, String>>()?,
                            })
                        })
                        .collect::<Result<_, String>>()
                        .map_err(|e| format!("bar {}: {e}", i + 1))?,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn clef_to(c: Clef) -> w::Clef {
    match c {
        Clef::Treble => w::Clef::Treble,
        Clef::Treble8vb => w::Clef::Treble8vb,
        Clef::Bass => w::Clef::Bass,
        Clef::Bass8vb => w::Clef::Bass8vb,
        Clef::Alto => w::Clef::Alto,
        Clef::Tenor => w::Clef::Tenor,
        Clef::Neutral => w::Clef::Neutral,
    }
}

fn clef_from(c: w::Clef) -> Clef {
    match c {
        w::Clef::Treble => Clef::Treble,
        w::Clef::Treble8vb => Clef::Treble8vb,
        w::Clef::Bass => Clef::Bass,
        w::Clef::Bass8vb => Clef::Bass8vb,
        w::Clef::Alto => Clef::Alto,
        w::Clef::Tenor => Clef::Tenor,
        w::Clef::Neutral => Clef::Neutral,
    }
}

fn beat_to(b: &Beat) -> w::Beat {
    let Beat {
        kind,
        duration,
        notes,
        grace,
        effects,
        chord,
        text,
        // Version 2's extensions (to_wire2).
        slur: _,
        hairpin: _,
        ottava: _,
        slash: _,
        beaming: _,
        tuplet_group: _,
        barre: _,
    } = b;
    let BeatEffects {
        stroke,
        pick_stroke,
        whammy,
        technique,
        fade,
        bridge_vibrato,
        tremolo_picking,
        dynamic,
        let_ring,
        pedal: _,
        wah: _,
        arpeggio: _,
        rasgueado: _,
        golpe: _,
        pick_scrape: _,
        dead_slap: _,
    } = effects;
    w::Beat {
        kind: match kind {
            BeatKind::Notes => w::BeatKind::Notes,
            BeatKind::Rest => w::BeatKind::Rest,
            BeatKind::Empty => w::BeatKind::Empty,
        },
        duration: duration_to(duration),
        notes: notes.iter().map(note_to).collect(),
        grace: grace.map(|g| match g {
            GraceKind::BeforeBeat => w::GraceKind::BeforeBeat,
            GraceKind::OnBeat => w::GraceKind::OnBeat,
        }),
        effects: w::BeatEffects {
            stroke: stroke.map(|Stroke { direction, speed }| w::Stroke {
                direction: stroke_direction_to(direction),
                speed: value_to(speed),
            }),
            pick_stroke: pick_stroke.map(stroke_direction_to),
            whammy: whammy.as_ref().map(bend_to),
            technique: technique.map(|t| match t {
                BeatTechnique::Tap => w::BeatTechnique::Tap,
                BeatTechnique::Slap => w::BeatTechnique::Slap,
                BeatTechnique::Pop => w::BeatTechnique::Pop,
            }),
            fade_in: *fade == Some(Fade::In),
            vibrato_wide: *bridge_vibrato == Some(Vibrato::Wide),
            tremolo_picking: tremolo_picking.map(value_to),
            dynamic: dynamic.map(dynamic_to),
            let_ring: *let_ring,
        },
        chord: chord.map(index_to),
        text: text.clone(),
    }
}

fn beat_from(b: w::Beat) -> Result<Beat, String> {
    let w::Beat {
        kind,
        duration,
        notes,
        grace,
        effects,
        chord,
        text,
    } = b;
    let w::BeatEffects {
        stroke,
        pick_stroke,
        whammy,
        technique,
        fade_in,
        vibrato_wide,
        tremolo_picking,
        dynamic,
        let_ring,
    } = effects;
    Ok(Beat {
        kind: match kind {
            w::BeatKind::Notes => BeatKind::Notes,
            w::BeatKind::Rest => BeatKind::Rest,
            w::BeatKind::Empty => BeatKind::Empty,
        },
        duration: duration_from(duration),
        notes: notes.into_iter().map(note_from).collect::<Result<_, _>>()?,
        grace: grace.map(|g| match g {
            w::GraceKind::BeforeBeat => GraceKind::BeforeBeat,
            w::GraceKind::OnBeat => GraceKind::OnBeat,
        }),
        effects: BeatEffects {
            stroke: stroke.map(|s| Stroke {
                direction: stroke_direction_from(s.direction),
                speed: value_from(s.speed),
            }),
            pick_stroke: pick_stroke.map(stroke_direction_from),
            whammy: whammy.map(bend_from).transpose()?,
            technique: technique.map(|t| match t {
                w::BeatTechnique::Tap => BeatTechnique::Tap,
                w::BeatTechnique::Slap => BeatTechnique::Slap,
                w::BeatTechnique::Pop => BeatTechnique::Pop,
            }),
            fade: fade_in.then_some(Fade::In),
            bridge_vibrato: vibrato_wide.then_some(Vibrato::Wide),
            tremolo_picking: tremolo_picking.map(value_from),
            dynamic: dynamic.map(dynamic_from),
            let_ring,
            ..BeatEffects::default()
        },
        chord: chord.map(index_from),
        text,
        ..Beat::default()
    })
}

fn stroke_direction_to(d: StrokeDirection) -> w::StrokeDirection {
    match d {
        StrokeDirection::Down => w::StrokeDirection::Down,
        StrokeDirection::Up => w::StrokeDirection::Up,
    }
}

fn stroke_direction_from(d: w::StrokeDirection) -> StrokeDirection {
    match d {
        w::StrokeDirection::Down => StrokeDirection::Down,
        w::StrokeDirection::Up => StrokeDirection::Up,
    }
}

fn value_to(v: NoteValue) -> w::NoteValue {
    match v {
        NoteValue::DoubleWhole => w::NoteValue::DoubleWhole,
        NoteValue::Whole => w::NoteValue::Whole,
        NoteValue::Half => w::NoteValue::Half,
        NoteValue::Quarter => w::NoteValue::Quarter,
        NoteValue::Eighth => w::NoteValue::Eighth,
        NoteValue::Sixteenth => w::NoteValue::Sixteenth,
        NoteValue::ThirtySecond => w::NoteValue::ThirtySecond,
        NoteValue::SixtyFourth => w::NoteValue::SixtyFourth,
        NoteValue::OneTwentyEighth => w::NoteValue::OneTwentyEighth,
        NoteValue::TwoFiftySixth => w::NoteValue::TwoFiftySixth,
    }
}

fn value_from(v: w::NoteValue) -> NoteValue {
    match v {
        w::NoteValue::DoubleWhole => NoteValue::DoubleWhole,
        w::NoteValue::Whole => NoteValue::Whole,
        w::NoteValue::Half => NoteValue::Half,
        w::NoteValue::Quarter => NoteValue::Quarter,
        w::NoteValue::Eighth => NoteValue::Eighth,
        w::NoteValue::Sixteenth => NoteValue::Sixteenth,
        w::NoteValue::ThirtySecond => NoteValue::ThirtySecond,
        w::NoteValue::SixtyFourth => NoteValue::SixtyFourth,
        w::NoteValue::OneTwentyEighth => NoteValue::OneTwentyEighth,
        w::NoteValue::TwoFiftySixth => NoteValue::TwoFiftySixth,
    }
}

fn duration_to(d: &Duration) -> w::Duration {
    let Duration {
        value,
        dots,
        tuplets,
    } = d;
    w::Duration {
        value: value_to(*value),
        dots: *dots,
        tuplets: tuplets
            .iter()
            .map(|&Tuplet { notes, in_time_of }| w::Tuplet { notes, in_time_of })
            .collect(),
    }
}

fn duration_from(d: w::Duration) -> Duration {
    Duration {
        value: value_from(d.value),
        dots: d.dots,
        tuplets: d
            .tuplets
            .into_iter()
            .map(|t| Tuplet {
                notes: t.notes,
                in_time_of: t.in_time_of,
            })
            .collect(),
    }
}

fn dynamic_to(d: Dynamic) -> w::Dynamic {
    match d {
        Dynamic::PPP => w::Dynamic::PPP,
        Dynamic::PP => w::Dynamic::PP,
        Dynamic::P => w::Dynamic::P,
        Dynamic::MP => w::Dynamic::MP,
        Dynamic::MF => w::Dynamic::MF,
        Dynamic::F => w::Dynamic::F,
        Dynamic::FF => w::Dynamic::FF,
        Dynamic::FFF => w::Dynamic::FFF,
    }
}

fn dynamic_from(d: w::Dynamic) -> Dynamic {
    match d {
        w::Dynamic::PPP => Dynamic::PPP,
        w::Dynamic::PP => Dynamic::PP,
        w::Dynamic::P => Dynamic::P,
        w::Dynamic::MP => Dynamic::MP,
        w::Dynamic::MF => Dynamic::MF,
        w::Dynamic::F => Dynamic::F,
        w::Dynamic::FF => Dynamic::FF,
        w::Dynamic::FFF => Dynamic::FFF,
    }
}

fn bend_to(Bend { points }: &Bend) -> w::Bend {
    w::Bend {
        points: points
            .iter()
            .map(|&BendPoint { offset, cents }| w::BendPoint {
                offset: ratio_to(offset),
                cents,
            })
            .collect(),
    }
}

fn bend_from(b: w::Bend) -> Result<Bend, String> {
    Ok(Bend {
        points: b
            .points
            .into_iter()
            .map(|p| {
                Ok(BendPoint {
                    offset: ratio_from(p.offset)?,
                    cents: p.cents,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn note_to(n: &Note) -> w::Note {
    let Note {
        place,
        tie,
        dynamic,
        effects,
        // Version 2's extension (to_wire2).
        spelling: _,
    } = n;
    let NoteEffects {
        hammer_pull,
        slides,
        bend,
        harmonic,
        vibrato,
        palm_mute,
        let_ring,
        dead,
        ghost,
        accent,
        staccato,
        trill,
        tremolo_picking,
        left_finger,
        right_finger,
        left_hand_tap,
        // Version 2's extensions (to_wire2).
        ornament: _,
        string_number: _,
    } = effects;
    w::Note {
        place: match *place {
            NotePlace::Fretted { string, fret } => w::NotePlace::Fretted { string, fret },
            NotePlace::Pitched(p) => w::NotePlace::Pitched(w::Pitch(p.0)),
        },
        tie: *tie,
        dynamic: dynamic_to(*dynamic),
        effects: w::NoteEffects {
            hammer_pull: *hammer_pull,
            slides: slides
                .iter()
                .map(|s| match s {
                    Slide::Shift => w::Slide::Shift,
                    Slide::Legato => w::Slide::Legato,
                    Slide::InFromBelow => w::Slide::InFromBelow,
                    Slide::InFromAbove => w::Slide::InFromAbove,
                    Slide::OutDown => w::Slide::OutDown,
                    Slide::OutUp => w::Slide::OutUp,
                })
                .collect(),
            bend: bend.as_ref().map(bend_to),
            harmonic: harmonic.map(|h| match h {
                Harmonic::Natural => w::Harmonic::Natural,
                Harmonic::Artificial { semitones } => w::Harmonic::Artificial { semitones },
                Harmonic::Tapped { fret } => w::Harmonic::Tapped { fret },
                Harmonic::Pinch => w::Harmonic::Pinch,
                Harmonic::Semi => w::Harmonic::Semi,
                Harmonic::Feedback => w::Harmonic::Feedback,
            }),
            vibrato: vibrato.is_some(),
            palm_mute: *palm_mute,
            let_ring: *let_ring,
            dead: *dead,
            ghost: *ghost,
            accent: accent.map(|a| match a {
                Accent::Normal => w::Accent::Normal,
                Accent::Heavy => w::Accent::Heavy,
            }),
            staccato: *staccato,
            trill: trill.map(|Trill { fret, speed }| w::Trill {
                fret,
                speed: value_to(speed),
            }),
            tremolo_picking: tremolo_picking.map(value_to),
            left_finger: *left_finger,
            right_finger: *right_finger,
            left_hand_tap: *left_hand_tap,
        },
    }
}

fn note_from(n: w::Note) -> Result<Note, String> {
    let w::Note {
        place,
        tie,
        dynamic,
        effects,
    } = n;
    let w::NoteEffects {
        hammer_pull,
        slides,
        bend,
        harmonic,
        vibrato,
        palm_mute,
        let_ring,
        dead,
        ghost,
        accent,
        staccato,
        trill,
        tremolo_picking,
        left_finger,
        right_finger,
        left_hand_tap,
    } = effects;
    Ok(Note {
        place: match place {
            w::NotePlace::Fretted { string, fret } => NotePlace::Fretted { string, fret },
            w::NotePlace::Pitched(p) => NotePlace::Pitched(Pitch(p.0)),
        },
        tie,
        dynamic: dynamic_from(dynamic),
        effects: NoteEffects {
            hammer_pull,
            slides: slides
                .into_iter()
                .map(|s| match s {
                    w::Slide::Shift => Slide::Shift,
                    w::Slide::Legato => Slide::Legato,
                    w::Slide::InFromBelow => Slide::InFromBelow,
                    w::Slide::InFromAbove => Slide::InFromAbove,
                    w::Slide::OutDown => Slide::OutDown,
                    w::Slide::OutUp => Slide::OutUp,
                })
                .collect(),
            bend: bend.map(bend_from).transpose()?,
            harmonic: harmonic.map(|h| match h {
                w::Harmonic::Natural => Harmonic::Natural,
                w::Harmonic::Artificial { semitones } => Harmonic::Artificial { semitones },
                w::Harmonic::Tapped { fret } => Harmonic::Tapped { fret },
                w::Harmonic::Pinch => Harmonic::Pinch,
                w::Harmonic::Semi => Harmonic::Semi,
                w::Harmonic::Feedback => Harmonic::Feedback,
            }),
            vibrato: vibrato.then_some(Vibrato::Slight),
            palm_mute,
            let_ring,
            dead,
            ghost,
            accent: accent.map(|a| match a {
                w::Accent::Normal => Accent::Normal,
                w::Accent::Heavy => Accent::Heavy,
            }),
            staccato,
            trill: trill.map(|t| Trill {
                fret: t.fret,
                speed: value_from(t.speed),
            }),
            tremolo_picking: tremolo_picking.map(value_from),
            left_finger,
            right_finger,
            left_hand_tap,
            ..NoteEffects::default()
        },
        spelling: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_denominator_is_refused_and_fractions_are_reduced() {
        let mut score = Score::new();
        let mut bar = MasterBar::default();
        bar.tempo_changes.push(TempoChange {
            position: Ratio::new(1, 2).unwrap(),
            bpm: 90.0,
            label: String::new(),
            linear: false,
        });
        score.master_bars.push(bar);
        let mut wire = to_wire(&score);
        assert_eq!(from_wire(wire.clone()).unwrap(), score);
        wire.master_bars[0].tempo_changes[0].position = w::Ratio { num: 2, den: 4 };
        assert_eq!(
            from_wire(wire.clone()).unwrap(),
            score,
            "2/4 reduces to 1/2"
        );
        wire.master_bars[0].tempo_changes[0].position = w::Ratio { num: 1, den: 0 };
        assert_eq!(
            from_wire(wire).unwrap_err(),
            "bar 1: 1/0 is not a valid fraction"
        );
    }

    /// A score with every mark of interface 1.6 on it.
    fn marked() -> Score {
        let mut score = Score::new();
        score.layout = vec![4, 3];
        let mut bar = MasterBar::default();
        bar.fermatas.push(Fermata {
            position: Ratio::new(1, 4).unwrap(),
            kind: FermataKind::Long,
            length: 0.5,
        });
        score.master_bars.push(bar);
        let mut staff = Staff::default();
        let mut note = Note::fretted(1, 3);
        note.effects.vibrato = Some(Vibrato::Wide);
        note.effects.ornament = Some(Ornament::InvertedTurn);
        note.effects.string_number = true;
        note.spelling = Some(Spelling { step: 1, alter: -2 });
        let mut beat = Beat {
            notes: vec![note],
            slur: true,
            hairpin: Some(Hairpin::Decrescendo),
            ottava: Some(Ottava::Bassa15),
            slash: true,
            beaming: Beaming::BreakSecondary,
            tuplet_group: true,
            barre: Some(Barre {
                fret: 5,
                half: true,
            }),
            ..Beat::default()
        };
        let fx = &mut beat.effects;
        fx.fade = Some(Fade::Swell);
        fx.bridge_vibrato = Some(Vibrato::Slight);
        fx.pedal = Some(Pedal::Up);
        fx.wah = Some(Wah::Closed);
        fx.arpeggio = Some(StrokeDirection::Up);
        fx.rasgueado = Some(Rasgueado::PeiAnapaest);
        fx.golpe = Some(Golpe::Thumb);
        fx.pick_scrape = Some(StrokeDirection::Down);
        fx.dead_slap = true;
        staff.bars.push(Bar {
            voices: vec![Voice { beats: vec![beat] }],
            simile: Some(Simile::SecondOfDouble),
            ..Bar::default()
        });
        score.tracks.push(Track {
            staves: vec![staff],
            layout: vec![2],
            ..Track::default()
        });
        score
    }

    #[test]
    fn every_mark_of_interface_1_6_goes_through_version_2() {
        let score = marked();
        let wire = to_wire2(&score);
        assert_eq!(from_wire2(wire.clone()).unwrap(), score);
        // Through the wire encoding too.
        let bytes = oxiguitar_plugin_api::encode(&wire).unwrap();
        let back: w2::Score2 = oxiguitar_plugin_api::decode(&bytes).unwrap();
        assert_eq!(from_wire2(back).unwrap(), score);
    }

    #[test]
    fn a_plugin_for_1_5_sees_version_1_s_part_of_each_mark_and_its_losses_are_named() {
        let mut score = marked();
        // A fade in and a wide bridge vibrato are version 1's own fields.
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .effects
            .fade = Some(Fade::In);
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .effects
            .bridge_vibrato = Some(Vibrato::Wide);
        let mut wire = to_wire2(&score);
        let lost = wire.extensions_newer_than(5);
        let names: Vec<&str> = lost.iter().map(|l| l.0).collect();
        assert!(names.contains(&"fermatas"));
        assert!(names.contains(&"hairpins"));
        assert!(names.contains(&"chosen spellings"));
        assert!(!names.contains(&"fade-outs"));
        assert!(wire.extensions_newer_than(6).is_empty());
        wire.keep_known(5);
        let old = from_wire2(wire).unwrap();
        let beat = &old.tracks[0].staves[0].bars[0].voices[0].beats[0];
        assert_eq!(beat.effects.fade, Some(Fade::In));
        assert_eq!(beat.effects.bridge_vibrato, Some(Vibrato::Wide));
        assert!(!beat.slur);
        // A wide left-hand vibrato is a plain one to an older plugin.
        assert_eq!(beat.notes[0].effects.vibrato, Some(Vibrato::Slight));
        assert_eq!(beat.notes[0].spelling, None);
        assert!(old.master_bars[0].fermatas.is_empty());
    }
}
