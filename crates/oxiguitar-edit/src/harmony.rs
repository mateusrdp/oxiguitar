// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Chords as Guitar Pro 8's chord builder makes them (M3 slice 6; Mateus,
//! 20 Oct 2026: the builder too): a root, a type, a seventh or extension,
//! alterations and a bass note, and from them the chord's name, its notes,
//! and the ways to play it on a track's strings. The other way round, a
//! diagram's frets give a name, as Guitar Pro suggests one once notes are
//! placed. Names are written as tabs write them, in ASCII (`F#m7`,
//! `Bb(add9)`, `C7(b9)/E`), which the .gp writer spells out for Guitar Pro.

/// Note names a root or a bass can take, in the order the builder lists
/// them.
pub const NOTE_NAMES: [&str; 17] = [
    "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb", "G", "G#", "Ab", "A", "A#", "Bb", "B",
];

/// The pitch class (0 = C) of each of [`NOTE_NAMES`].
const NOTE_CLASSES: [u8; 17] = [0, 1, 1, 2, 3, 3, 4, 5, 6, 6, 7, 8, 8, 9, 10, 10, 11];

/// The name a pitch class takes by default when a chord is recognised.
const DEFAULT_NAME: [usize; 12] = [0, 1, 3, 5, 6, 7, 8, 10, 12, 13, 15, 16];

/// The triad, or what stands for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Triad {
    /// Major.
    Major,
    /// Minor (m).
    Minor,
    /// Augmented (aug).
    Augmented,
    /// Diminished (dim).
    Diminished,
    /// The second for the third (sus2).
    Sus2,
    /// The fourth for the third (sus4).
    Sus4,
    /// Root and fifth only (5).
    Power,
}

impl Triad {
    /// Every triad, in the builder's order.
    pub const ALL: [Triad; 7] = [
        Triad::Major,
        Triad::Minor,
        Triad::Augmented,
        Triad::Diminished,
        Triad::Sus2,
        Triad::Sus4,
        Triad::Power,
    ];

    /// The builder's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Triad::Major => "Major",
            Triad::Minor => "Minor",
            Triad::Augmented => "Augmented",
            Triad::Diminished => "Diminished",
            Triad::Sus2 => "Suspended second",
            Triad::Sus4 => "Suspended fourth",
            Triad::Power => "Power chord",
        }
    }

    fn intervals(self) -> &'static [u8] {
        match self {
            Triad::Major => &[0, 4, 7],
            Triad::Minor => &[0, 3, 7],
            Triad::Augmented => &[0, 4, 8],
            Triad::Diminished => &[0, 3, 6],
            Triad::Sus2 => &[0, 2, 7],
            Triad::Sus4 => &[0, 5, 7],
            Triad::Power => &[0, 7],
        }
    }
}

/// The sixth, seventh or extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extension {
    /// None: the triad alone.
    None,
    /// A sixth (6).
    Sixth,
    /// A sixth and a ninth (6/9).
    SixNine,
    /// A seventh (7; dim7 on a diminished triad).
    Seventh,
    /// A major seventh (maj7).
    MajorSeventh,
    /// A ninth (9).
    Ninth,
    /// A major ninth (maj9).
    MajorNinth,
    /// An eleventh (11).
    Eleventh,
    /// A thirteenth (13).
    Thirteenth,
    /// A ninth added, with no seventh (add9).
    AddNine,
}

impl Extension {
    /// Every extension, in the builder's order.
    pub const ALL: [Extension; 10] = [
        Extension::None,
        Extension::Sixth,
        Extension::SixNine,
        Extension::Seventh,
        Extension::MajorSeventh,
        Extension::Ninth,
        Extension::MajorNinth,
        Extension::Eleventh,
        Extension::Thirteenth,
        Extension::AddNine,
    ];

    /// The builder's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Extension::None => "None",
            Extension::Sixth => "6",
            Extension::SixNine => "6/9",
            Extension::Seventh => "7",
            Extension::MajorSeventh => "maj7",
            Extension::Ninth => "9",
            Extension::MajorNinth => "maj9",
            Extension::Eleventh => "11",
            Extension::Thirteenth => "13",
            Extension::AddNine => "add9",
        }
    }

    /// How unusual it is, for choosing among names.
    fn weight(self) -> u32 {
        match self {
            Extension::None => 0,
            Extension::Seventh => 1,
            Extension::MajorSeventh | Extension::Sixth => 2,
            Extension::AddNine | Extension::Ninth => 3,
            Extension::MajorNinth | Extension::SixNine => 4,
            Extension::Eleventh | Extension::Thirteenth => 5,
        }
    }
}

/// The alterations, as bits.
pub mod alt {
    /// ♭5.
    pub const FLAT_FIVE: u8 = 1;
    /// ♯5.
    pub const SHARP_FIVE: u8 = 2;
    /// ♭9.
    pub const FLAT_NINE: u8 = 4;
    /// ♯9.
    pub const SHARP_NINE: u8 = 8;
    /// ♯11.
    pub const SHARP_ELEVEN: u8 = 16;
    /// ♭13.
    pub const FLAT_THIRTEEN: u8 = 32;
    /// Each bit and its name in a chord symbol.
    pub const NAMES: [(u8, &str); 6] = [
        (FLAT_FIVE, "b5"),
        (SHARP_FIVE, "#5"),
        (FLAT_NINE, "b9"),
        (SHARP_NINE, "#9"),
        (SHARP_ELEVEN, "#11"),
        (FLAT_THIRTEEN, "b13"),
    ];
}

/// A chord as the builder holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The root, an index into [`NOTE_NAMES`].
    pub root: usize,
    /// The triad.
    pub triad: Triad,
    /// The sixth, seventh or extension.
    pub extension: Extension,
    /// The alterations ([`alt`]).
    pub alterations: u8,
    /// A bass note other than the root, an index into [`NOTE_NAMES`].
    pub bass: Option<usize>,
}

impl Chord {
    /// A major chord on `root`.
    pub fn major(root: usize) -> Chord {
        Chord {
            root,
            triad: Triad::Major,
            extension: Extension::None,
            alterations: 0,
            bass: None,
        }
    }

    /// The root's pitch class.
    pub fn root_class(&self) -> u8 {
        NOTE_CLASSES.get(self.root).copied().unwrap_or(0)
    }

    /// The bass's pitch class: the bass note's, or the root's.
    pub fn bass_class(&self) -> u8 {
        self.bass
            .and_then(|b| NOTE_CLASSES.get(b).copied())
            .unwrap_or_else(|| self.root_class())
    }

    /// The chord's name, as tabs write it.
    pub fn name(&self) -> String {
        let root = NOTE_NAMES.get(self.root).copied().unwrap_or("C");
        let mut s = String::from(root);
        let ext = self.extension;
        let dim7 = self.triad == Triad::Diminished && ext == Extension::Seventh;
        match self.triad {
            Triad::Major | Triad::Sus2 | Triad::Sus4 => {}
            Triad::Minor => s.push('m'),
            Triad::Augmented => s.push_str("aug"),
            Triad::Diminished => s.push_str("dim"),
            Triad::Power => {
                s.push('5');
            }
        }
        if self.triad != Triad::Power {
            match ext {
                Extension::None => {}
                Extension::MajorSeventh | Extension::MajorNinth if self.triad == Triad::Minor => {
                    s.push_str(if ext == Extension::MajorSeventh {
                        "(maj7)"
                    } else {
                        "(maj9)"
                    });
                }
                Extension::Seventh if dim7 => s.push('7'),
                Extension::AddNine => s.push_str("(add9)"),
                e => s.push_str(e.label()),
            }
            match self.triad {
                Triad::Sus2 => s.push_str("sus2"),
                Triad::Sus4 => s.push_str("sus4"),
                _ => {}
            }
            let alts: Vec<&str> = alt::NAMES
                .iter()
                .filter(|(bit, _)| self.alterations & bit != 0)
                .map(|(_, n)| *n)
                .collect();
            if !alts.is_empty() {
                s.push('(');
                s.push_str(&alts.join(","));
                s.push(')');
            }
        }
        if let Some(b) = self.bass.and_then(|b| NOTE_NAMES.get(b)) {
            s.push('/');
            s.push_str(b);
        }
        s
    }

    /// The chord's pitch classes above its root (0 to 11), the bass's
    /// apart.
    fn intervals(&self) -> Vec<u8> {
        let mut v: Vec<u8> = self.triad.intervals().to_vec();
        let mut add = |i: u8| {
            if !v.contains(&i) {
                v.push(i);
            }
        };
        if self.triad != Triad::Power {
            let dim7 = self.triad == Triad::Diminished;
            match self.extension {
                Extension::None => {}
                Extension::Sixth => add(9),
                Extension::SixNine => {
                    add(9);
                    add(2);
                }
                Extension::Seventh => add(if dim7 { 9 } else { 10 }),
                Extension::MajorSeventh => add(11),
                Extension::Ninth => {
                    add(10);
                    add(2);
                }
                Extension::MajorNinth => {
                    add(11);
                    add(2);
                }
                Extension::Eleventh => {
                    add(10);
                    add(2);
                    add(5);
                }
                Extension::Thirteenth => {
                    add(10);
                    add(2);
                    add(9);
                }
                Extension::AddNine => add(2),
            }
            let a = self.alterations;
            let swap = |v: &mut Vec<u8>, from: u8, to: u8| {
                v.retain(|x| *x != from);
                if !v.contains(&to) {
                    v.push(to);
                }
            };
            if a & alt::FLAT_FIVE != 0 {
                swap(&mut v, 7, 6);
            }
            if a & alt::SHARP_FIVE != 0 {
                swap(&mut v, 7, 8);
            }
            if a & alt::FLAT_NINE != 0 {
                swap(&mut v, 2, 1);
            }
            if a & alt::SHARP_NINE != 0 {
                swap(&mut v, 2, 3);
            }
            if a & alt::SHARP_ELEVEN != 0 {
                swap(&mut v, 5, 6);
            }
            if a & alt::FLAT_THIRTEEN != 0 {
                swap(&mut v, 9, 8);
            }
        }
        v
    }

    /// The pitch classes the chord sounds, the root's first.
    pub fn classes(&self) -> Vec<u8> {
        let root = self.root_class();
        let mut v: Vec<u8> = self
            .intervals()
            .into_iter()
            .map(|i| (root + i) % 12)
            .collect();
        let bass = self.bass_class();
        if !v.contains(&bass) {
            v.push(bass);
        }
        v
    }

    /// The pitch classes a voicing may leave out: the fifth (unaltered),
    /// and in an eleventh or thirteenth the ninth and eleventh.
    fn optional(&self) -> Vec<u8> {
        let root = self.root_class();
        let mut v = Vec::new();
        let plain_fifth = self.alterations & (alt::FLAT_FIVE | alt::SHARP_FIVE) == 0
            && matches!(
                self.triad,
                Triad::Major | Triad::Minor | Triad::Sus2 | Triad::Sus4
            );
        if plain_fifth && self.intervals().len() > 3 {
            v.push((root + 7) % 12);
        }
        if matches!(self.extension, Extension::Eleventh | Extension::Thirteenth) {
            v.push((root + 2) % 12);
        }
        if self.extension == Extension::Thirteenth {
            v.push((root + 5) % 12);
        }
        v
    }

    /// How unusual the chord is, for choosing among names.
    fn weight(&self) -> u32 {
        let triad = match self.triad {
            Triad::Major | Triad::Minor => 0,
            Triad::Power => 1,
            Triad::Sus4 | Triad::Sus2 => 2,
            Triad::Diminished | Triad::Augmented => 3,
        };
        triad
            + self.extension.weight()
            + 3 * self.alterations.count_ones()
            + if self.bass.is_some() { 4 } else { 0 }
    }

    /// The chord named `name`, if the builder makes that name; `None` for
    /// one it doesn't (kept as typed).
    pub fn parse(name: &str) -> Option<Chord> {
        let name = name.trim();
        let (body, bass) = match name.rsplit_once('/') {
            Some((b, n)) if NOTE_NAMES.contains(&n) => (b, NOTE_NAMES.iter().position(|x| *x == n)),
            _ => (name, None),
        };
        // The longest root name that starts it.
        let root = (0..NOTE_NAMES.len())
            .filter(|i| NOTE_NAMES.get(*i).is_some_and(|n| body.starts_with(n)))
            .max_by_key(|i| NOTE_NAMES.get(*i).map_or(0, |n| n.len()))?;
        let mut alterations = Vec::new();
        for a in 0u8..64 {
            alterations.push(a);
        }
        for triad in Triad::ALL {
            for extension in Extension::ALL {
                for &alterations in &alterations {
                    let c = Chord {
                        root,
                        triad,
                        extension,
                        alterations,
                        bass: None,
                    };
                    if c.name() == body {
                        return Some(Chord { bass, ..c });
                    }
                }
            }
        }
        None
    }

    /// The chord a diagram's notes make (MIDI pitches), as Guitar Pro
    /// suggests once two notes are placed: the simplest name whose notes
    /// are exactly these, the lowest note its root if it can be, its bass
    /// otherwise. `None` for fewer than two pitch classes or none that
    /// fits.
    pub fn recognise(pitches: &[u8]) -> Option<Chord> {
        let lowest = *pitches.iter().min()? % 12;
        let mut classes: Vec<u8> = pitches.iter().map(|p| p % 12).collect();
        classes.sort_unstable();
        classes.dedup();
        if classes.len() < 2 {
            return None;
        }
        let mut best: Option<(u32, Chord)> = None;
        for &root in &classes {
            let root_name = *DEFAULT_NAME.get(usize::from(root))?;
            for triad in Triad::ALL {
                for extension in Extension::ALL {
                    if triad == Triad::Power && extension != Extension::None {
                        continue;
                    }
                    for a in [0u8, 1, 2, 4, 8, 16, 32] {
                        let mut c = Chord {
                            root: root_name,
                            triad,
                            extension,
                            alterations: a,
                            bass: None,
                        };
                        if lowest != root {
                            c.bass = DEFAULT_NAME.get(usize::from(lowest)).copied();
                        }
                        let mut need = c.classes();
                        need.sort_unstable();
                        need.dedup();
                        let optional = c.optional();
                        let fits = classes.iter().all(|x| need.contains(x))
                            && need
                                .iter()
                                .all(|x| classes.contains(x) || optional.contains(x));
                        if !fits {
                            continue;
                        }
                        // Leaving notes out costs a little.
                        let missing = need.iter().filter(|x| !classes.contains(x)).count();
                        let w = c.weight() * 2 + u32::try_from(missing).unwrap_or(9);
                        if best.as_ref().is_none_or(|(bw, _)| w < *bw) {
                            best = Some((w, c));
                        }
                    }
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// Ways to play the chord on strings tuned `tuning` (MIDI pitches,
    /// highest string first) within `frets` frets: frets per string
    /// (highest first; `None` not played), easiest first, at most
    /// `limit`. Each has every note the chord needs, its bass lowest, the
    /// fretted notes within four frets, and at least three strings played
    /// (two for a power chord).
    pub fn voicings(&self, tuning: &[u8], frets: u8, limit: usize) -> Vec<Vec<Option<u8>>> {
        let strings = tuning.len();
        if strings < 2 {
            return Vec::new();
        }
        let classes = self.classes();
        let optional = self.optional();
        let bass = self.bass_class();
        let least = if self.triad == Triad::Power { 2 } else { 3 }.min(strings);
        let mut found: Vec<(u32, Vec<Option<u8>>)> = Vec::new();
        let top = frets.min(15);
        for low in 0..=top.saturating_sub(1) {
            let high = low.saturating_add(3).min(frets);
            // Each string's choices: not played, open, or a chord note in
            // the window.
            let options: Vec<Vec<Option<u8>>> = tuning
                .iter()
                .map(|open| {
                    let mut o = vec![None];
                    for f in std::iter::once(0).chain(low.max(1)..=high) {
                        if classes.contains(&((open + f) % 12)) {
                            o.push(Some(f));
                        }
                    }
                    o
                })
                .collect();
            let mut pick = vec![0usize; strings];
            loop {
                let shape: Vec<Option<u8>> = pick
                    .iter()
                    .zip(&options)
                    .map(|(i, o)| o.get(*i).copied().flatten())
                    .collect();
                if let Some(score) = judge(&shape, tuning, &classes, &optional, bass, least)
                    && !found.iter().any(|(_, f)| *f == shape)
                {
                    found.push((score, shape));
                }
                // The next combination.
                let mut i = 0;
                while let Some(p) = pick.get_mut(i) {
                    *p += 1;
                    if *p < options.get(i).map_or(0, Vec::len) {
                        break;
                    }
                    *p = 0;
                    i += 1;
                }
                if i == strings {
                    break;
                }
            }
        }
        found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        found.into_iter().take(limit).map(|(_, f)| f).collect()
    }
}

/// How hard a shape is to play, or `None` if it isn't the chord: the
/// fingers it takes, strings left out between played ones, its stretch,
/// and how far up the neck it is.
fn judge(
    shape: &[Option<u8>],
    tuning: &[u8],
    classes: &[u8],
    optional: &[u8],
    bass: u8,
    least: usize,
) -> Option<u32> {
    let played: Vec<(usize, u8)> = shape
        .iter()
        .enumerate()
        .filter_map(|(i, f)| f.map(|f| (i, f)))
        .collect();
    if played.len() < least {
        return None;
    }
    let pitch = |(i, f): (usize, u8)| u16::from(*tuning.get(i).unwrap_or(&0)) + u16::from(f);
    let lowest = played.iter().copied().min_by_key(|p| pitch(*p))?;
    if u8::try_from(pitch(lowest) % 12).ok()? != bass {
        return None;
    }
    // Every note the chord needs.
    let sounding: Vec<u8> = played
        .iter()
        .filter_map(|p| u8::try_from(pitch(*p) % 12).ok())
        .collect();
    if classes
        .iter()
        .any(|c| !sounding.contains(c) && !optional.contains(c))
    {
        return None;
    }
    let fretted: Vec<u8> = played.iter().map(|(_, f)| *f).filter(|f| *f > 0).collect();
    let (lo, hi) = (
        fretted.iter().min().copied().unwrap_or(0),
        fretted.iter().max().copied().unwrap_or(0),
    );
    if hi.saturating_sub(lo) > 3 {
        return None;
    }
    // Fingers: one a fretted note, or with more than four, a barre: one
    // finger across the lowest fret, which no open string may lie under.
    let mut fingers = fretted.len();
    if fingers > 4 {
        let barred: Vec<usize> = played
            .iter()
            .filter(|(_, f)| *f == lo)
            .map(|(i, _)| *i)
            .collect();
        let (from, to) = (*barred.first()?, *barred.last()?);
        if (from..=to).any(|i| shape.get(i) == Some(&Some(0))) {
            return None;
        }
        fingers -= barred.len().saturating_sub(1);
        if fingers > 4 {
            return None;
        }
    }
    let first = played.first()?.0;
    let last = played.last()?.0;
    let gaps = (first..=last)
        .filter(|i| shape.get(*i).is_some_and(Option::is_none))
        .count();
    let missing = classes.iter().filter(|c| !sounding.contains(c)).count();
    let unplayed = shape.len() - played.len();
    Some(
        u32::try_from(fingers).unwrap_or(9) * 3
            + u32::try_from(gaps).unwrap_or(9) * 8
            + u32::from(hi.saturating_sub(lo)) * 2
            + u32::from(lo)
            + u32::try_from(missing).unwrap_or(9) * 4
            + u32::try_from(unplayed).unwrap_or(9) * 3,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    /// Standard tuning, highest string first.
    const E: [u8; 6] = [64, 59, 55, 50, 45, 40];

    fn named(name: &str) -> Chord {
        Chord::parse(name).unwrap_or_else(|| panic!("{name}"))
    }

    #[test]
    fn names_as_tabs_write_them() {
        let c = |root, triad, extension, alterations, bass| Chord {
            root,
            triad,
            extension,
            alterations,
            bass,
        };
        assert_eq!(
            c(13, Triad::Minor, Extension::Seventh, 0, None).name(),
            "Am7"
        );
        assert_eq!(
            c(0, Triad::Major, Extension::MajorSeventh, 0, None).name(),
            "Cmaj7"
        );
        assert_eq!(
            c(0, Triad::Major, Extension::Seventh, alt::FLAT_NINE, Some(6)).name(),
            "C7(b9)/E"
        );
        assert_eq!(
            c(3, Triad::Sus4, Extension::Seventh, 0, None).name(),
            "D7sus4"
        );
        assert_eq!(c(6, Triad::Power, Extension::None, 0, None).name(), "E5");
        assert_eq!(
            c(15, Triad::Major, Extension::AddNine, 0, None).name(),
            "Bb(add9)"
        );
        assert_eq!(
            c(1, Triad::Diminished, Extension::Seventh, 0, None).name(),
            "C#dim7"
        );
        assert_eq!(
            c(13, Triad::Minor, Extension::MajorSeventh, 0, None).name(),
            "Am(maj7)"
        );
        for name in [
            "Am7", "C7(b9)/E", "F#m7(b5)", "Bb(add9)", "Gsus2", "E5", "Ebaug", "C6/9",
        ] {
            assert_eq!(named(name).name(), name);
        }
        assert_eq!(Chord::parse("H7"), None);
        assert_eq!(Chord::parse("C7alt"), None);
    }

    #[test]
    fn notes_of_chords() {
        assert_eq!(named("G7").classes(), [7, 11, 2, 5]);
        assert_eq!(named("Cdim7").classes(), [0, 3, 6, 9]);
        assert_eq!(named("C/G").classes(), [0, 4, 7]);
        assert_eq!(named("D/F#").classes(), [2, 6, 9]);
        let mut half_dim = named("Am7(b5)").classes();
        half_dim.sort_unstable();
        assert_eq!(half_dim, [0, 3, 7, 9]);
    }

    #[test]
    fn diagrams_are_recognised() {
        let pitches = |frets: [Option<u8>; 6]| -> Vec<u8> {
            frets
                .iter()
                .zip(E)
                .filter_map(|(f, o)| f.map(|f| o + f))
                .collect()
        };
        // x 0 2 2 1 0, low to high: A minor.
        let am = [Some(0), Some(1), Some(2), Some(2), Some(0), None];
        assert_eq!(Chord::recognise(&pitches(am)).unwrap().name(), "Am");
        // 3 2 0 0 0 3: G.
        let g = [Some(3), Some(0), Some(0), Some(0), Some(2), Some(3)];
        assert_eq!(Chord::recognise(&pitches(g)).unwrap().name(), "G");
        // x x 0 2 1 2: D7.
        let d7 = [Some(2), Some(1), Some(2), Some(0), None, None];
        assert_eq!(Chord::recognise(&pitches(d7)).unwrap().name(), "D7");
        // A power chord: 5 7 on the two lowest strings.
        let a5 = [None, None, None, None, Some(7), Some(5)];
        assert_eq!(Chord::recognise(&pitches(a5)).unwrap().name(), "A5");
        // C over its fifth: 3 3 2 0 1 0.
        let cg = [Some(0), Some(1), Some(0), Some(2), Some(3), Some(3)];
        assert_eq!(Chord::recognise(&pitches(cg)).unwrap().name(), "C/G");
        assert_eq!(Chord::recognise(&[60]), None);
    }

    #[test]
    fn voicings_on_six_strings() {
        let v = named("C").voicings(&E, 24, 12);
        // The open C (x 3 2 0 1 0) comes first.
        assert_eq!(v[0], [Some(0), Some(1), Some(0), Some(2), Some(3), None]);
        // Every one sounds C, E and G, C lowest.
        for shape in &v {
            let mut p: Vec<u8> = shape
                .iter()
                .zip(E)
                .filter_map(|(f, o)| f.map(|f| o + f))
                .collect();
            p.sort_unstable();
            assert_eq!(p[0] % 12, 0, "{shape:?}");
            for c in [0, 4, 7] {
                assert!(p.iter().any(|x| x % 12 == c), "{shape:?}");
            }
        }
        assert!(v.len() >= 6);
        let e = named("Em").voicings(&E, 24, 3);
        assert_eq!(e[0], [Some(0), Some(0), Some(0), Some(2), Some(2), Some(0)]);
        // Drop D tuning gives D5 on the lowest strings.
        let drop: [u8; 6] = [64, 59, 55, 50, 45, 38];
        let d5 = named("D5").voicings(&drop, 24, 3);
        assert!(d5.iter().any(|s| s[5] == Some(0) && s[4] == Some(0)));
    }
}
