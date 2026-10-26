// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The player's controls (M4 slice 4), without window code: what Play,
//! Play Selection, Pause, Stop, Rewind, Loop, the count-in, the metronome,
//! the speed, the mixer and the master level do to the engine, and what
//! they need from the player's thread ([`crate::sound`]).
//!
//! Playing starts from the caret's beat (Play Selection from the
//! selection's first beat, and it stops at its end); Loop repeats the
//! selection, or the whole song without one. Pause leaves the caret on
//! the beat heard; Stop puts it back where playing began; Rewind goes to
//! the song's start, or to the loop's while looping. No editing while the
//! sound runs: edits are refused while playing and allowed when paused
//! (Mateus, 25 Oct 2026), so the engine never swaps a schedule under
//! sounding notes. The mixer and the speed last for the session; nothing
//! is written to the file.
//!
//! The output opens on Play (the device isn't held while nothing plays),
//! stays open while paused, and closes a moment after Stop or the song's
//! end, once the last notes have rung out.

use std::sync::Arc;

use oxiguitar_edit::{Caret, Selection, onsets};
use oxiguitar_model::{Mix, Ratio, Score};
use oxiguitar_play::{Control, Schedule, Sources, Timeline, TrackMix};

use crate::sound::Ready;

/// The speeds the player takes, in percent.
pub(crate) const SPEEDS: (u32, u32) = (25, 200);

/// The master level's control, 0 to 1, as it starts: about +6 dB
/// ([`master_gain`] of it is 1.96). Measured over the 104 corpus songs
/// through the SoundFont plugin with FluidR3 at the engine's unity gain,
/// their peaks sit at a median of −9.4 dBFS (a tenth below −14.6, a tenth
/// above −5.8; one at 0) and their loudest second at a median of −23 dBFS
/// RMS; one guitar alone at mf peaks near −27. So +6 dB brings a typical
/// song's peaks near −3 dBFS, and the engine's limiter (−1 dBFS) holds
/// down the few louder ones instead of letting them clip.
pub(crate) const MASTER: f32 = 0.7;

/// The engine's master gain for the master control `v` (0 to 1): a square
/// law, as a mixer's fader (half way is −6 dB from the top), times
/// [`HEADROOM`].
pub(crate) fn master_gain(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v * HEADROOM
}

/// The gain at the top of the master control: +12 dB, for quiet songs.
pub(crate) const HEADROOM: f32 = 4.0;

/// A place in the score: a bar and a position in it, in whole notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) bar: usize,
    pub(crate) offset: Ratio,
}

impl Place {
    /// Where the caret's beat starts.
    pub(crate) fn of_caret(caret: Caret, score: &Score) -> Place {
        Place {
            bar: caret.bar,
            offset: caret.onset(score),
        }
    }
}

/// The start and the end of what `selection` covers in `score`: from its
/// first beat to the end of its last, or its bars whole.
pub(crate) fn span_of(selection: Selection, score: &Score) -> Option<(Place, Place)> {
    match selection {
        Selection::Beats {
            track,
            staff,
            voice,
            from,
            to,
        } => {
            let voice_in = |bar: usize| {
                score
                    .tracks
                    .get(track)?
                    .staves
                    .get(staff)?
                    .bars
                    .get(bar)?
                    .voices
                    .get(voice)
            };
            let start = onsets(voice_in(from.0)?).get(from.1).copied()?;
            let last = voice_in(to.0)?;
            let starts = onsets(last);
            let end = match starts.get(to.1 + 1) {
                Some(&e) => e,
                None => last.length().unwrap_or(Ratio::ZERO),
            };
            Some((
                Place {
                    bar: from.0,
                    offset: start,
                },
                Place {
                    bar: to.0,
                    offset: end,
                },
            ))
        }
        Selection::Bars { bars, .. } => Some((
            Place {
                bar: bars.0,
                offset: Ratio::ZERO,
            },
            Place {
                bar: bars.1,
                offset: Ratio::from_integer(i64::MAX / 4),
            },
        )),
    }
}

/// When `place` is first played at or after played bar `from`: the played
/// bar and the time, in seconds at 100 %. A place past its bar's end is
/// that bar's end.
pub(crate) fn time_of(timeline: &Timeline, place: Place, from: usize) -> Option<(usize, f64)> {
    let visits = timeline.visits();
    let v = (from..visits.len()).find(|&v| visits[v].visit.bar == place.bar)?;
    let length = timeline.map.bar_length(place.bar);
    if place.offset >= length {
        return Some((v, visits[v].end));
    }
    Some((v, timeline.map.time(v, place.offset)))
}

/// The times from the start of `span` to its end, in playing order.
pub(crate) fn times_of(timeline: &Timeline, span: (Place, Place)) -> Option<(f64, f64)> {
    let (v, a) = time_of(timeline, span.0, 0)?;
    let (_, b) = time_of(timeline, span.1, v)?;
    (b > a).then_some((a, b))
}

/// Where the music is at `time`: the bar and the position in it.
pub(crate) fn place_at(timeline: &Timeline, time: f64) -> Option<Place> {
    let (v, offset) = timeline.find(time)?;
    let bar = timeline.visits()[v].visit.bar;
    // Positions as played, in whole notes, to the nearest 1920th of a
    // whole note (the finest beat a file holds is a 256th in a tuplet).
    let n = (offset * 1920.0 + 1e-6).floor().max(0.0) as i64;
    Some(Place {
        bar,
        offset: Ratio::new(n, 1920).unwrap_or(Ratio::ZERO),
    })
}

/// What the transport is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Stopped,
    /// Play was asked for; waiting for the output to open or the score's
    /// schedule to be built.
    Starting,
    Playing,
    Paused,
}

/// What the window does for the player.
#[derive(Debug, PartialEq)]
pub(crate) enum Do {
    /// Open the output.
    Open,
    /// Close the output once this many seconds have passed, unless the
    /// player has started again since (the number counts stops).
    CloseAfter(u64),
    /// Close the output now.
    Close,
}

/// What Play was asked for, kept until it can start.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Asked {
    from: Place,
    /// Play Selection's span, which stops at its end.
    until: Option<(Place, Place)>,
}

/// The transport, the mixer and the engine's window half.
pub(crate) struct Player {
    control: Control,
    state: State,
    /// The output is open.
    open: bool,
    /// The score's newest revision, and the timeline and schedule the
    /// engine has (or is about to have), with their revision.
    revision: u64,
    ready: Option<(u64, Arc<Timeline>, Arc<Schedule>)>,
    /// Instruments and a schedule for the engine, kept while it plays.
    waiting: Option<(Option<Sources>, Arc<Schedule>)>,
    /// Why nothing can play, if so.
    unplayable: Option<String>,
    asked: Option<Asked>,
    /// Where the current playing began, for Stop.
    began: Option<Place>,
    /// The loop's span in the score, when Loop is on (`None`: the song).
    looping: bool,
    loop_span: Option<(Place, Place)>,
    count_in: bool,
    metronome: bool,
    speed: u32,
    master: f32,
    mix: Vec<TrackMix>,
    stops: u64,
}

impl Player {
    pub(crate) fn new(control: Control) -> Player {
        Player {
            control,
            state: State::Stopped,
            open: false,
            revision: 0,
            ready: None,
            waiting: None,
            unplayable: None,
            asked: None,
            began: None,
            looping: false,
            loop_span: None,
            count_in: false,
            metronome: false,
            speed: 100,
            master: MASTER,
            mix: Vec::new(),
            stops: 0,
        }
    }

    pub(crate) fn state(&self) -> State {
        self.state
    }

    /// The sound runs (or is about to): edits are refused.
    pub(crate) fn refuses_edits(&self) -> bool {
        matches!(self.state, State::Starting | State::Playing)
    }

    /// Whether Play can do anything: a timeline was built and something
    /// plays it.
    pub(crate) fn can_play(&self) -> bool {
        self.ready.is_some() && self.unplayable.is_none()
    }

    pub(crate) fn unplayable(&self) -> Option<&str> {
        self.unplayable.as_deref()
    }

    /// The timeline of the score the engine plays.
    pub(crate) fn timeline(&self) -> Option<&Arc<Timeline>> {
        self.ready.as_ref().map(|(_, t, _)| t)
    }

    /// The score changed (opened or edited): its revision.
    pub(crate) fn score_changed(&mut self, revision: u64) {
        self.revision = revision;
    }

    /// The player's thread built a revision's timeline (and instruments).
    pub(crate) fn ready(&mut self, ready: Ready) -> Vec<Do> {
        if ready.revision != self.revision {
            // Older than the score: a newer one is on its way. Its
            // instruments are still the newest prepared, though.
            if let Some(s) = ready.sources {
                self.hand_over(Some(s), ready.schedule.clone());
            }
            return Vec::new();
        }
        self.unplayable = None;
        self.ready = Some((ready.revision, ready.timeline, ready.schedule.clone()));
        self.hand_over(ready.sources, ready.schedule);
        self.go()
    }

    /// Nothing can play the score.
    pub(crate) fn unplayable_now(&mut self, why: String) -> Vec<Do> {
        self.unplayable = Some(why);
        self.ready = None;
        self.asked = None;
        let mut out = Vec::new();
        if self.state != State::Stopped {
            out.extend(self.stop_sound());
        }
        out
    }

    /// The instruments and schedule to the engine: now if the output is
    /// open and nothing plays, else when it opens or the sound stops (so
    /// nothing piles up in the engine's queue while no device empties it).
    fn hand_over(&mut self, sources: Option<Sources>, schedule: Arc<Schedule>) {
        let sources = match self.waiting.take() {
            Some((old, _)) => sources.or(old),
            None => sources,
        };
        self.waiting = Some((sources, schedule));
        if self.open && self.state != State::Playing {
            self.take_waiting();
        }
    }

    fn take_waiting(&mut self) {
        if let Some((sources, schedule)) = self.waiting.take() {
            if let Some(s) = sources {
                self.control.set_sources(s);
            }
            self.control.set_schedule(schedule);
        }
    }

    /// Everything the engine is to know, once the output is open (nothing
    /// is sent while it is closed, so nothing piles up).
    fn tell_all(&mut self) {
        self.take_waiting();
        self.control.set_master(master_gain(self.master));
        self.control.set_speed(f64::from(self.speed) / 100.0);
        self.control.set_metronome(self.metronome);
        for (t, m) in self.mix.clone().into_iter().enumerate() {
            self.control.set_mix(t, m);
        }
        if let Some(tl) = self.timeline().cloned() {
            let l = self.loop_times(&tl, 0);
            self.control.set_loop(l);
        }
    }

    /// The engine was made again at the device's rate: everything it was
    /// told is told again.
    pub(crate) fn new_engine(&mut self, control: Control) {
        self.control = control;
        self.waiting = None;
        // Told when its output opens. The schedule comes again with the instruments prepared at the
        // new rate.
        self.ready = None;
    }

    /// Play: from `from`, or Play Selection: `span`, stopping at its end.
    /// The loop's span is given whenever Loop may need it.
    pub(crate) fn play(&mut self, from: Place, until: Option<(Place, Place)>) -> Vec<Do> {
        if self.unplayable.is_some() {
            return Vec::new();
        }
        let from = until.map_or(from, |s| s.0);
        self.asked = Some(Asked { from, until });
        self.began = Some(from);
        if self.state == State::Playing {
            // Playing already: start again from there.
            self.state = State::Starting;
        } else if self.state != State::Starting {
            self.state = State::Starting;
            self.stops += 1;
        }
        let mut out = Vec::new();
        if !self.open {
            out.push(Do::Open);
        }
        out.extend(self.go());
        out
    }

    /// The output opened.
    pub(crate) fn opened(&mut self) -> Vec<Do> {
        self.open = true;
        self.tell_all();
        if self.state == State::Stopped {
            // Stopped while it opened.
            return vec![Do::Close];
        }
        self.go()
    }

    /// The output couldn't open, or closed.
    pub(crate) fn closed(&mut self) {
        self.open = false;
        if self.state != State::Stopped {
            self.state = State::Stopped;
            self.asked = None;
        }
    }

    /// Starts what was asked once everything it needs is there.
    fn go(&mut self) -> Vec<Do> {
        if self.state != State::Starting || !self.open {
            return Vec::new();
        }
        let (Some(asked), Some((revision, timeline, _))) = (self.asked, &self.ready) else {
            return Vec::new();
        };
        if *revision != self.revision {
            return Vec::new();
        }
        let timeline = timeline.clone();
        self.take_waiting();
        let Some((visit, from)) = time_of(&timeline, asked.from, 0) else {
            self.asked = None;
            self.state = State::Stopped;
            return vec![Do::CloseAfter(self.stops)];
        };
        let until = asked
            .until
            .and_then(|s| times_of(&timeline, s))
            .map(|(_, b)| b);
        self.control.set_loop(self.loop_times(&timeline, visit));
        self.control.set_speed(f64::from(self.speed) / 100.0);
        self.control.set_metronome(self.metronome);
        self.control.play_until(from, until, self.count_in);
        self.asked = None;
        self.state = State::Playing;
        Vec::new()
    }

    /// The loop's times, when Loop is on: the selection's span, or the
    /// whole song.
    fn loop_times(&self, timeline: &Timeline, _visit: usize) -> Option<(f64, f64)> {
        if !self.looping {
            return None;
        }
        match self.loop_span {
            Some(s) => times_of(timeline, s),
            None => Some((0.0, timeline.length())),
        }
    }

    /// Pause: the sound stops where it is; the output stays open. Returns
    /// where it stopped, for the caret.
    pub(crate) fn pause(&mut self) -> Option<Place> {
        match self.state {
            State::Playing => {
                let heard = self.control.heard();
                self.control.pause();
                self.state = State::Paused;
                self.take_waiting();
                heard.and_then(|t| self.timeline().and_then(|tl| place_at(tl, t)))
            }
            State::Starting => {
                self.asked = None;
                self.state = State::Paused;
                None
            }
            _ => None,
        }
    }

    /// Stop: the sound stops, and the caret goes back where playing began
    /// (returned).
    pub(crate) fn stop(&mut self) -> (Option<Place>, Vec<Do>) {
        if self.state == State::Stopped {
            return (None, Vec::new());
        }
        let back = self.began.take();
        (back, self.stop_sound())
    }

    fn stop_sound(&mut self) -> Vec<Do> {
        self.control.pause();
        self.asked = None;
        self.state = State::Stopped;
        self.stops += 1;
        if self.open {
            self.take_waiting();
        }
        if self.open {
            vec![Do::CloseAfter(self.stops)]
        } else {
            Vec::new()
        }
    }

    /// The delay to close the output after a stop has passed: close it if
    /// nothing played since.
    pub(crate) fn close_if(&mut self, stops: u64) -> Vec<Do> {
        if self.open && self.state == State::Stopped && stops == self.stops {
            self.open = false;
            return vec![Do::Close];
        }
        Vec::new()
    }

    /// Rewind: to the song's start, or to the loop's while looping. While
    /// the sound runs it moves there; otherwise the caret does (returned).
    pub(crate) fn rewind(&mut self) -> Option<Place> {
        let start = match (self.looping, self.loop_span) {
            (true, Some((a, _))) => a,
            _ => Place {
                bar: 0,
                offset: Ratio::ZERO,
            },
        };
        if self.state == State::Playing {
            if let Some(tl) = self.timeline().cloned()
                && let Some((_, t)) = time_of(&tl, start, 0)
            {
                self.control.seek(t);
            }
            return None;
        }
        Some(start)
    }

    /// While playing: to the start of the bar played `by` bars away.
    pub(crate) fn skip_bars(&mut self, by: isize) -> bool {
        if self.state != State::Playing {
            return false;
        }
        let Some(tl) = self.timeline().cloned() else {
            return false;
        };
        let Some((v, _)) = self.control.heard().and_then(|t| tl.find(t)) else {
            return false;
        };
        let to = v
            .saturating_add_signed(by)
            .min(tl.visits().len().saturating_sub(1));
        self.control.seek(tl.visits()[to].start);
        true
    }

    /// Called on every frame the window draws while the sound runs (and
    /// whenever the window acts): what the engine handed back is dropped,
    /// and the song's end noticed. Returns whether it ended.
    pub(crate) fn frame(&mut self) -> (bool, Vec<Do>) {
        let c = self.control.collect();
        if c.ended && self.state == State::Playing {
            self.began = None;
            return (true, self.stop_sound());
        }
        (false, Vec::new())
    }

    /// The time heard now, while playing.
    pub(crate) fn heard(&self) -> Option<f64> {
        matches!(self.state, State::Playing | State::Paused)
            .then(|| self.control.heard())
            .flatten()
    }

    pub(crate) fn looping(&self) -> bool {
        self.looping
    }

    /// Loop on or off, over `span` (the selection) or the whole song.
    pub(crate) fn set_loop(&mut self, on: bool, span: Option<(Place, Place)>) {
        self.looping = on;
        self.loop_span = span;
        if self.open
            && let Some(tl) = self.timeline().cloned()
        {
            let l = self.loop_times(&tl, 0);
            self.control.set_loop(l);
        }
    }

    /// The overview was clicked at `time`: the sound goes there while it
    /// plays (true); otherwise the caret should (false).
    pub(crate) fn seek(&mut self, time: f64) -> bool {
        if self.state == State::Playing {
            self.control.seek(time);
            return true;
        }
        false
    }

    pub(crate) fn count_in(&self) -> bool {
        self.count_in
    }

    pub(crate) fn set_count_in(&mut self, on: bool) {
        self.count_in = on;
    }

    pub(crate) fn metronome(&self) -> bool {
        self.metronome
    }

    pub(crate) fn set_metronome(&mut self, on: bool) {
        self.metronome = on;
        if self.open {
            self.control.set_metronome(on);
        }
    }

    pub(crate) fn speed(&self) -> u32 {
        self.speed
    }

    /// The speed, in percent (25 to 200).
    pub(crate) fn set_speed(&mut self, percent: u32) {
        self.speed = percent.clamp(SPEEDS.0, SPEEDS.1);
        if self.open {
            self.control.set_speed(f64::from(self.speed) / 100.0);
        }
    }

    pub(crate) fn master(&self) -> f32 {
        self.master
    }

    pub(crate) fn set_master(&mut self, v: f32) {
        self.master = v.clamp(0.0, 1.0);
        if self.open {
            self.control.set_master(master_gain(self.master));
        }
    }

    /// Every track's mixer, as the track rows show it.
    pub(crate) fn set_mixes(&mut self, mixes: &[Mix]) {
        self.mix = mixes.iter().map(|m| track_mix(*m)).collect();
        if self.open {
            for (t, m) in self.mix.clone().into_iter().enumerate() {
                self.control.set_mix(t, m);
            }
        }
    }

    /// One track's mixer.
    pub(crate) fn set_mix(&mut self, track: usize, mix: Mix) {
        let m = track_mix(mix);
        if let Some(slot) = self.mix.get_mut(track) {
            *slot = m;
        }
        if self.open {
            self.control.set_mix(track, m);
        }
    }
}

fn track_mix(m: Mix) -> TrackMix {
    TrackMix {
        volume: m.volume,
        pan: m.pan,
        mute: m.mute,
        solo: m.solo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_model::{Bar, Beat, Duration, MasterBar, Note, NoteValue, Staff, Track, Voice};
    use oxiguitar_play::{Engine, Route, engine};

    /// Four bars of four quarter notes at 120 (two seconds a bar).
    fn score() -> Arc<Score> {
        let beat = Beat {
            duration: Duration::new(NoteValue::Quarter),
            notes: vec![Note::fretted(1, 0)],
            ..Beat::default()
        };
        let bar = Bar {
            voices: vec![Voice {
                beats: vec![beat; 4],
            }],
            ..Bar::default()
        };
        Arc::new(Score {
            master_bars: vec![MasterBar::default(); 4],
            tracks: vec![Track {
                staves: vec![Staff {
                    tuning: Staff::standard_guitar_tuning(),
                    bars: vec![bar; 4],
                    ..Staff::default()
                }],
                ..Track::default()
            }],
            initial_tempo: 120.0,
            ..Score::new()
        })
    }

    fn ready(score: &Score, revision: u64) -> Ready {
        let timeline = Arc::new(Timeline::new(score));
        let routes: Vec<Option<Route>> = vec![None];
        Ready {
            revision,
            schedule: Arc::new(Schedule::new(&timeline, &routes, 0)),
            timeline,
            sources: None,
            silent: Vec::new(),
        }
    }

    fn quarter(n: i64) -> Ratio {
        Ratio::new(n, 4).unwrap()
    }

    fn at(bar: usize, quarters: i64) -> Place {
        Place {
            bar,
            offset: quarter(quarters),
        }
    }

    /// A player told about `score` (revision 1), and the engine's audio
    /// half, which the tests call as the device would.
    fn rig() -> (Player, Engine, Arc<Score>) {
        let (engine, control) = engine(48_000, 512);
        let mut p = Player::new(control);
        let s = score();
        p.score_changed(1);
        assert!(p.ready(ready(&s, 1)).is_empty());
        (p, engine, s)
    }

    /// `seconds` of device callbacks of 480 frames.
    fn render(e: &mut Engine, seconds: f64) {
        let mut out = vec![0.0f32; 960];
        for _ in 0..(seconds * 100.0) as usize {
            e.render(&mut out, 2, 0);
        }
    }

    #[test]
    fn a_selection_spans_from_its_first_beat_to_the_end_of_its_last() {
        let s = score();
        let tl = Timeline::new(&s);
        let beats = Selection::Beats {
            track: 0,
            staff: 0,
            voice: 0,
            from: (1, 1),
            to: (2, 2),
        };
        let span = span_of(beats, &s).unwrap();
        assert_eq!(span, (at(1, 1), at(2, 3)));
        let (a, b) = times_of(&tl, span).unwrap();
        assert!((a - 2.5).abs() < 1e-9 && (b - 5.5).abs() < 1e-9, "{a} {b}");
        let bars = Selection::Bars {
            tracks: (0, 0),
            bars: (1, 2),
        };
        let (a, b) = times_of(&tl, span_of(bars, &s).unwrap()).unwrap();
        assert!((a - 2.0).abs() < 1e-9 && (b - 6.0).abs() < 1e-9, "{a} {b}");
        // And back from a time to a place.
        assert_eq!(place_at(&tl, 2.5), Some(at(1, 1)));
        assert_eq!(place_at(&tl, 7.99).map(|p| p.bar), Some(3));
    }

    #[test]
    fn play_opens_the_output_first_and_edits_wait_while_the_sound_runs() {
        let (mut p, mut e, _) = rig();
        assert!(p.can_play());
        assert!(!p.refuses_edits());
        assert_eq!(p.play(at(1, 0), None), vec![Do::Open]);
        assert_eq!(p.state(), State::Starting);
        assert!(
            p.refuses_edits(),
            "edits wait from the moment Play is pressed"
        );
        assert!(p.opened().is_empty());
        assert_eq!(p.state(), State::Playing);
        render(&mut e, 0.5);
        let heard = p.heard().unwrap();
        assert!((2.0..2.6).contains(&heard), "playing from bar 2: {heard}");
        // Pause: the sound stops where it is, the caret goes there, the
        // output stays open and edits are allowed again.
        let place = p.pause().unwrap();
        assert_eq!(place.bar, 1);
        assert_eq!(p.state(), State::Paused);
        assert!(!p.refuses_edits());
        // Playing again doesn't open the output again.
        assert!(p.play(place, None).is_empty());
        assert_eq!(p.state(), State::Playing);
    }

    #[test]
    fn stop_goes_back_where_playing_began_and_the_output_closes_after_the_ring_out() {
        let (mut p, mut e, _) = rig();
        p.play(at(2, 1), None);
        p.opened();
        render(&mut e, 0.3);
        let (back, dos) = p.stop();
        assert_eq!(back, Some(at(2, 1)));
        let Some(&Do::CloseAfter(n)) = dos.first() else {
            panic!("{dos:?}");
        };
        assert_eq!(p.state(), State::Stopped);
        // Played again before the delay ran out: the output stays.
        p.play(at(0, 0), None);
        assert!(p.close_if(n).is_empty());
        let (_, dos) = p.stop();
        let Some(&Do::CloseAfter(m)) = dos.first() else {
            panic!("{dos:?}");
        };
        assert_eq!(p.close_if(m), vec![Do::Close]);
        p.closed();
        // The next Play opens it again.
        assert_eq!(p.play(at(0, 0), None), vec![Do::Open]);
    }

    #[test]
    fn play_selection_stops_at_the_selections_end() {
        let (mut p, mut e, s) = rig();
        let span = span_of(
            Selection::Bars {
                tracks: (0, 0),
                bars: (1, 1),
            },
            &s,
        )
        .unwrap();
        p.play(at(3, 0), Some(span));
        p.opened();
        render(&mut e, 1.0);
        let heard = p.heard().unwrap();
        assert!(
            (2.0..3.1).contains(&heard),
            "from the selection's start: {heard}"
        );
        assert_eq!(p.frame(), (false, Vec::new()));
        render(&mut e, 1.5);
        let (ended, dos) = p.frame();
        assert!(ended, "the selection's end stops it");
        assert!(matches!(dos.as_slice(), [Do::CloseAfter(_)]), "{dos:?}");
        assert_eq!(p.state(), State::Stopped);
    }

    #[test]
    fn loop_repeats_the_selection_and_rewind_goes_to_its_start() {
        let (mut p, mut e, s) = rig();
        let span = span_of(
            Selection::Bars {
                tracks: (0, 0),
                bars: (2, 2),
            },
            &s,
        );
        p.set_loop(true, span);
        p.play(at(2, 0), None);
        p.opened();
        render(&mut e, 5.0);
        let heard = p.heard().unwrap();
        assert!((4.0..6.0).contains(&heard), "still in bar 3: {heard}");
        assert!(!p.frame().0, "a loop never ends");
        // Rewind while playing moves the sound; when stopped, the caret.
        assert_eq!(p.rewind(), None);
        p.stop();
        assert_eq!(p.rewind(), Some(at(2, 0)));
        p.set_loop(false, None);
        assert_eq!(p.rewind(), Some(at(0, 0)));
    }

    #[test]
    fn a_newer_score_waits_for_its_schedule_before_playing() {
        let (mut p, _, s) = rig();
        p.score_changed(2);
        assert_eq!(p.play(at(0, 0), None), vec![Do::Open]);
        assert!(p.opened().is_empty());
        assert_eq!(p.state(), State::Starting, "revision 1's schedule is old");
        p.ready(ready(&s, 2));
        assert_eq!(p.state(), State::Playing);
    }

    #[test]
    fn nothing_plays_an_unplayable_score() {
        let (mut p, _, _) = rig();
        p.unplayable_now("no sound set found".into());
        assert!(!p.can_play());
        assert!(p.play(at(0, 0), None).is_empty());
        assert_eq!(p.state(), State::Stopped);
    }

    #[test]
    fn the_master_control_is_a_square_law_up_to_the_headroom() {
        assert!((master_gain(MASTER) - oxiguitar_play::engine::MASTER_USUAL).abs() < 1e-6);
        assert_eq!(master_gain(1.0), HEADROOM);
        assert!((master_gain(0.5) - HEADROOM / 4.0).abs() < 1e-6);
        assert_eq!(master_gain(0.0), 0.0);
        assert_eq!(master_gain(2.0), HEADROOM, "clamped");
        const { assert!(HEADROOM <= oxiguitar_play::engine::MASTER_MOST) };
    }
}
