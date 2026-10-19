// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The caret: where the next edit goes, and how the keys move it.
//!
//! The caret names a beat by its place in the model (track, staff, voice,
//! bar, beat index) and a string, so it needs no layout to exist and stays
//! meaningful after any engraving. Moves follow Guitar Pro 8's cursor (its
//! user guide, "Navigating a Score" and the shortcut list, checked 19 Oct
//! 2026): left and right go beat by beat across bar lines, up and down go
//! string by string, Home goes to the bar's first beat, Ctrl + Home and
//! Ctrl + End to the first and last bars, Tab to the next staff, Ctrl + Up
//! and Ctrl + Down to the previous and next track.
//!
//! A voice with no beats in a bar still has one place for the caret, beat
//! 0, where entering a note will start the voice. Voices go from 0 to 3
//! (EDIT-3) whether or not a bar has them yet.

use oxiguitar_model::{Ratio, Score, Voice};

/// The most voices a staff has (EDIT-3).
pub const VOICES: usize = 4;

/// One bar of one staff: what the bar check judges and the window marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StaffBar {
    /// The track, from 0.
    pub track: usize,
    /// The staff in the track, from 0.
    pub staff: usize,
    /// The bar, from 0.
    pub bar: usize,
}

/// Where the next edit goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Caret {
    /// The track, from 0.
    pub track: usize,
    /// The staff in the track, from 0.
    pub staff: usize,
    /// The voice, 0 to 3.
    pub voice: usize,
    /// The bar, from 0.
    pub bar: usize,
    /// The beat in the voice, from 0. In a voice with no beats, 0.
    pub beat: usize,
    /// The string, from 1 (the highest). 1 on staves without strings.
    pub string: u8,
}

/// The start of each beat in `voice`, from the start of the bar. A grace
/// beat takes no time: it starts where the next main beat does. A beat
/// whose duration can't be measured counts as taking no time.
pub fn onsets(voice: &Voice) -> Vec<Ratio> {
    let mut out = Vec::with_capacity(voice.beats.len());
    let mut at = Ratio::ZERO;
    for b in &voice.beats {
        out.push(at);
        if b.grace.is_none() {
            at = at
                .checked_add(b.duration.length().unwrap_or(Ratio::ZERO))
                .unwrap_or(at);
        }
    }
    out
}

/// The beat of `voice` sounding at `onset`: the last main beat starting at
/// or before it, or 0. Grace beats are never chosen.
pub fn beat_at(voice: &Voice, onset: Ratio) -> usize {
    let starts = onsets(voice);
    let mut found = 0;
    for (i, (b, s)) in voice.beats.iter().zip(&starts).enumerate() {
        if b.grace.is_some() {
            continue;
        }
        if *s <= onset {
            found = i;
        } else {
            break;
        }
    }
    found
}

impl StaffBar {
    /// The bar of the staff after this one, if the score has it.
    pub fn next(self, score: &Score) -> Option<StaffBar> {
        (self.bar + 1 < score.bar_count()).then_some(StaffBar {
            bar: self.bar + 1,
            ..self
        })
    }
}

impl Caret {
    /// The start of `track`: its first staff, voice 1, bar 1, first beat,
    /// top string. `None` if the score has no such track or no bars.
    pub fn at_start(score: &Score, track: usize) -> Option<Caret> {
        let t = score.tracks.get(track)?;
        if t.staves.is_empty() || score.bar_count() == 0 {
            return None;
        }
        Some(Caret {
            track,
            staff: 0,
            voice: 0,
            bar: 0,
            beat: 0,
            string: 1,
        })
    }

    /// The bar the caret is in.
    pub fn staff_bar(self) -> StaffBar {
        StaffBar {
            track: self.track,
            staff: self.staff,
            bar: self.bar,
        }
    }

    /// The voice the caret is in, if the bar has it.
    pub fn voice_in(self, score: &Score) -> Option<&Voice> {
        score
            .tracks
            .get(self.track)?
            .staves
            .get(self.staff)?
            .bars
            .get(self.bar)?
            .voices
            .get(self.voice)
    }

    /// How many places the caret has in its bar and voice: one per beat,
    /// and one in a voice with no beats.
    fn places(self, score: &Score) -> usize {
        self.voice_in(score).map_or(0, |v| v.beats.len()).max(1)
    }

    fn strings(self, score: &Score) -> u8 {
        let n = score
            .tracks
            .get(self.track)
            .and_then(|t| t.staves.get(self.staff))
            .map_or(0, |s| s.string_count());
        u8::try_from(n).unwrap_or(u8::MAX).max(1)
    }

    /// The caret moved to a valid place of `score`, as near as it can be:
    /// after an edit or an undo the bar, beat or string it pointed at may
    /// have gone. `None` if the score has no track to put it in.
    pub fn clamp(self, score: &Score) -> Option<Caret> {
        let track = self.track.min(score.tracks.len().checked_sub(1)?);
        let t = score.tracks.get(track)?;
        let staff = self.staff.min(t.staves.len().checked_sub(1)?);
        let bar = self.bar.min(score.bar_count().checked_sub(1)?);
        let mut c = Caret {
            track,
            staff,
            voice: self.voice.min(VOICES - 1),
            bar,
            beat: self.beat,
            string: self.string,
        };
        c.beat = c.beat.min(c.places(score) - 1);
        c.string = c.string.clamp(1, c.strings(score));
        Some(c)
    }

    /// Where the caret's beat starts in its bar; zero in a voice with no
    /// beats.
    pub fn onset(self, score: &Score) -> Ratio {
        self.voice_in(score)
            .and_then(|v| onsets(v).get(self.beat).copied())
            .unwrap_or(Ratio::ZERO)
    }

    /// Right arrow: the next beat, or the next bar's first beat. Stays put
    /// at the end of the score.
    pub fn next_beat(self, score: &Score) -> Caret {
        if self.beat + 1 < self.places(score) {
            return Caret {
                beat: self.beat + 1,
                ..self
            };
        }
        if self.bar + 1 < score.bar_count() {
            return Caret {
                bar: self.bar + 1,
                beat: 0,
                ..self
            };
        }
        self
    }

    /// Left arrow: the previous beat, or the previous bar's last beat.
    /// Stays put at the start of the score.
    pub fn prev_beat(self, score: &Score) -> Caret {
        if self.beat > 0 {
            return Caret {
                beat: self.beat - 1,
                ..self
            };
        }
        if self.bar > 0 {
            let c = Caret {
                bar: self.bar - 1,
                ..self
            };
            return Caret {
                beat: c.places(score) - 1,
                ..c
            };
        }
        self
    }

    /// Home: the first beat of the bar.
    pub fn bar_start(self) -> Caret {
        Caret { beat: 0, ..self }
    }

    /// End: the last beat of the bar.
    pub fn bar_end(self, score: &Score) -> Caret {
        Caret {
            beat: self.places(score) - 1,
            ..self
        }
    }

    /// The first beat of bar `bar` (kept within the score).
    pub fn to_bar(self, score: &Score, bar: usize) -> Caret {
        Caret {
            bar: bar.min(score.bar_count().saturating_sub(1)),
            beat: 0,
            ..self
        }
    }

    /// Ctrl + Home: the first beat of the first bar.
    pub fn first_bar(self, score: &Score) -> Caret {
        self.to_bar(score, 0)
    }

    /// Ctrl + End: the first beat of the last bar.
    pub fn last_bar(self, score: &Score) -> Caret {
        self.to_bar(score, usize::MAX)
    }

    /// Up arrow: the string above, if any.
    pub fn string_up(self) -> Caret {
        Caret {
            string: self.string.saturating_sub(1).max(1),
            ..self
        }
    }

    /// Down arrow: the string below, if any.
    pub fn string_down(self, score: &Score) -> Caret {
        Caret {
            string: self.string.saturating_add(1).min(self.strings(score)),
            ..self
        }
    }

    /// The same moment of the bar in another voice: the beat sounding where
    /// the caret's beat starts (Edit Voice 1 to 4).
    pub fn with_voice(self, score: &Score, voice: usize) -> Caret {
        let onset = self.onset(score);
        let c = Caret {
            voice: voice.min(VOICES - 1),
            ..self
        };
        Caret {
            beat: c.voice_in(score).map_or(0, |v| beat_at(v, onset)),
            ..c
        }
    }

    /// The same moment of the bar on another staff: `track`'s staff `staff`,
    /// in the same voice, on the nearest string it has.
    pub fn to_staff(self, score: &Score, track: usize, staff: usize) -> Caret {
        let onset = self.onset(score);
        let c = Caret {
            track,
            staff,
            ..self
        };
        let c = Caret {
            beat: c.voice_in(score).map_or(0, |v| beat_at(v, onset)),
            ..c
        };
        Caret {
            string: c.string.clamp(1, c.strings(score)),
            ..c
        }
    }

    /// Ctrl + Down: the first staff of the next track among `shown` (the
    /// tracks the window shows, in order). Stays put on the last.
    pub fn next_track(self, score: &Score, shown: &[usize]) -> Caret {
        let at = shown.iter().position(|&t| t == self.track);
        let next = match at {
            Some(i) => shown.get(i + 1),
            None => shown.iter().find(|&&t| t > self.track),
        };
        next.map_or(self, |&t| self.to_staff(score, t, 0))
    }

    /// Ctrl + Up: the first staff of the previous track among `shown`.
    /// Stays put on the first.
    pub fn prev_track(self, score: &Score, shown: &[usize]) -> Caret {
        let at = shown.iter().position(|&t| t == self.track);
        let prev = match at {
            Some(i) => i.checked_sub(1).and_then(|i| shown.get(i)),
            None => shown.iter().rev().find(|&&t| t < self.track),
        };
        prev.map_or(self, |&t| self.to_staff(score, t, 0))
    }

    /// Tab: the next staff of the track, or the first staff of the next
    /// track among `shown`.
    pub fn next_staff(self, score: &Score, shown: &[usize]) -> Caret {
        let staves = score.tracks.get(self.track).map_or(0, |t| t.staves.len());
        if self.staff + 1 < staves {
            return self.to_staff(score, self.track, self.staff + 1);
        }
        self.next_track(score, shown)
    }

    /// Shift + Tab: the previous staff of the track, or the last staff of
    /// the previous track among `shown`.
    pub fn prev_staff(self, score: &Score, shown: &[usize]) -> Caret {
        if self.staff > 0 {
            return self.to_staff(score, self.track, self.staff - 1);
        }
        let c = self.prev_track(score, shown);
        if c.track == self.track {
            return self;
        }
        let last = score
            .tracks
            .get(c.track)
            .map_or(0, |t| t.staves.len().saturating_sub(1));
        self.to_staff(score, c.track, last)
    }

    /// The caret's beat counted in the bar's first voice, as plugins see a
    /// position (interface 1.1's `Position`): the first voice's beat
    /// sounding when the caret's beat starts.
    pub fn beat_in_first_voice(self, score: &Score) -> usize {
        let onset = self.onset(score);
        Caret { voice: 0, ..self }
            .voice_in(score)
            .map_or(0, |v| beat_at(v, onset))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use oxiguitar_model::{
        Bar, Beat, BeatKind, Duration, GraceKind, MasterBar, Note, NoteValue, Staff, Track,
    };

    pub(crate) fn beat(value: NoteValue) -> Beat {
        Beat {
            duration: Duration::new(value),
            notes: vec![Note::fretted(1, 0)],
            ..Beat::default()
        }
    }

    pub(crate) fn bar(values: &[NoteValue]) -> Bar {
        Bar {
            voices: vec![Voice {
                beats: values.iter().map(|&v| beat(v)).collect(),
            }],
            ..Bar::default()
        }
    }

    /// `tracks` six-string tracks of `bars` bars, each bar four quarters.
    pub(crate) fn score(tracks: usize, bars: usize) -> Score {
        let q = NoteValue::Quarter;
        Score {
            master_bars: vec![MasterBar::default(); bars],
            tracks: (0..tracks)
                .map(|_| Track {
                    staves: vec![Staff {
                        tuning: Staff::standard_guitar_tuning(),
                        bars: vec![bar(&[q, q, q, q]); bars],
                        ..Staff::default()
                    }],
                    ..Track::default()
                })
                .collect(),
            ..Score::new()
        }
    }

    #[test]
    fn arrows_cross_bar_lines_and_stop_at_the_ends() {
        let s = score(1, 2);
        let c = Caret::at_start(&s, 0).unwrap();
        assert_eq!(c.prev_beat(&s), c, "nothing before the first beat");
        let mut d = c;
        for _ in 0..4 {
            d = d.next_beat(&s);
        }
        assert_eq!((d.bar, d.beat), (1, 0));
        assert_eq!(d.prev_beat(&s).bar, 0);
        assert_eq!(d.prev_beat(&s).beat, 3);
        let end = d.bar_end(&s);
        assert_eq!(end.beat, 3);
        assert_eq!(end.next_beat(&s), end, "nothing after the last beat");
        assert_eq!(end.bar_start().beat, 0);
        assert_eq!(end.first_bar(&s).bar, 0);
        assert_eq!(c.last_bar(&s), Caret { bar: 1, ..c });
    }

    #[test]
    fn strings_stay_on_the_instrument() {
        let s = score(1, 1);
        let c = Caret::at_start(&s, 0).unwrap();
        assert_eq!(c.string_up().string, 1);
        let mut d = c;
        for _ in 0..10 {
            d = d.string_down(&s);
        }
        assert_eq!(d.string, 6);
    }

    #[test]
    fn an_empty_voice_has_one_place() {
        let s = score(1, 2);
        let c = Caret::at_start(&s, 0).unwrap().with_voice(&s, 1);
        assert_eq!((c.voice, c.beat), (1, 0));
        let d = c.next_beat(&s);
        assert_eq!((d.bar, d.beat), (1, 0), "one place per bar");
        assert_eq!(d.prev_beat(&s).beat, 0);
    }

    #[test]
    fn voices_and_tracks_keep_the_moment() {
        let mut s = score(3, 1);
        // Track 2: an eighth, then quarters.
        let e = NoteValue::Eighth;
        let q = NoteValue::Quarter;
        s.tracks[1].staves[0].bars[0] = bar(&[e, e, q, q, q]);
        let c = Caret {
            beat: 2,
            ..Caret::at_start(&s, 0).unwrap()
        };
        // The third quarter starts at a half: the fourth beat of track 2.
        let shown = [0, 1, 2];
        let d = c.next_track(&s, &shown);
        assert_eq!((d.track, d.beat), (1, 3));
        assert_eq!(d.prev_track(&s, &shown).beat, 2);
        // Hidden tracks are skipped.
        assert_eq!(c.next_track(&s, &[0, 2]).track, 2);
        assert_eq!(c.prev_track(&s, &shown), c);
        assert_eq!(d.beat_in_first_voice(&s), 3);
    }

    #[test]
    fn tab_goes_through_every_staff() {
        let mut s = score(2, 1);
        let second = s.tracks[0].staves[0].clone();
        s.tracks[0].staves.push(second);
        let c = Caret::at_start(&s, 0).unwrap();
        let shown = [0, 1];
        let d = c.next_staff(&s, &shown);
        assert_eq!((d.track, d.staff), (0, 1));
        let e = d.next_staff(&s, &shown);
        assert_eq!((e.track, e.staff), (1, 0));
        assert_eq!(e.next_staff(&s, &shown), e);
        assert_eq!(e.prev_staff(&s, &shown), d);
        assert_eq!(d.prev_staff(&s, &shown), c);
    }

    #[test]
    fn grace_beats_take_no_time_and_are_never_landed_on_by_time() {
        let q = NoteValue::Quarter;
        let mut v = bar(&[q, q, q]).voices.remove(0);
        v.beats.insert(
            1,
            Beat {
                grace: Some(GraceKind::BeforeBeat),
                ..beat(NoteValue::ThirtySecond)
            },
        );
        let quarter = NoteValue::Quarter.length();
        assert_eq!(
            onsets(&v),
            vec![Ratio::ZERO, quarter, quarter, quarter + quarter]
        );
        assert_eq!(beat_at(&v, quarter), 2);
        assert_eq!(beat_at(&v, Ratio::ZERO), 0);
    }

    #[test]
    fn clamping_after_the_score_shrank() {
        let s = score(1, 2);
        let far = Caret {
            track: 5,
            staff: 3,
            voice: 9,
            bar: 40,
            beat: 99,
            string: 20,
        };
        let c = far.clamp(&s).unwrap();
        assert_eq!(
            c,
            Caret {
                track: 0,
                staff: 0,
                voice: 3,
                bar: 1,
                beat: 0,
                string: 6
            }
        );
        assert!(far.clamp(&Score::new()).is_none());
        // A rest still counts as a place.
        let mut r = s.clone();
        r.tracks[0].staves[0].bars[1].voices[0].beats[0].kind = BeatKind::Rest;
        assert_eq!(
            Caret {
                voice: 0,
                beat: 3,
                ..c
            }
            .clamp(&r)
            .unwrap()
            .beat,
            3
        );
    }
}
