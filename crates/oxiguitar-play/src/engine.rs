// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The engine: the scheduler, the mixer and the audio clock
//! (docs/design/player.md, "The chain" and "Staying in sync").
//!
//! [`engine`] makes two halves. The [`Engine`] lives on the audio thread:
//! the device's callback calls [`Engine::render`], which reads the
//! [`Schedule`] directly (no thread fills queues ahead, so nothing needs
//! waking), splits the callback at every event, has each [`Source`] render
//! the piece, mixes the tracks (volume, pan, mute, solo), adds the
//! metronome and the count-in, and writes the device's frames. It
//! allocates nothing, takes no lock and makes no system call: everything
//! it needs arrives through lock-free single-producer queues ([`rtrb`]),
//! and what it lets go of (an old schedule, old sources) goes back the
//! same way, to be dropped off the audio thread.
//!
//! The [`Control`] lives with the window: play, pause, stop, seek, loop,
//! speed, the mixer, the metronome and count-in, and a new schedule or new
//! sources while nothing plays (no editing while playing: Mateus, 25 Oct
//! 2026). Its [`Control::heard`] says which time of the score is being
//! heard now: the frames handed to the device, minus the device's delay to
//! the speaker and the sources' declared latency, through the pieces of
//! time the engine played (so a loop or a seek moves the cursor exactly
//! when the sound moves).
//!
//! Time: the engine keeps an anchor (a device frame, the score's time
//! there, and the score's seconds per frame, the speed over the rate),
//! reset only on play, seek, loop and speed. A frame's time is the anchor's
//! plus the frames since times the step, so nothing accumulates and nothing
//! drifts, however long the song. An event at time `t` happens on the first
//! frame whose time is at or after `t`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use oxiguitar_plugin_api::realtime::{PitchPoint, RtEvent, RtEventKind, RtNote};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::Click;
use crate::schedule::Schedule;

/// The engine's sample rate when the device takes it: NAM's.
pub const RATE: u32 = 48_000;

/// Something that renders tracks: an instrument plugin's instance, or a
/// test's own. Called on the audio thread only.
pub trait Source: Send {
    /// Renders `out.frames()` frames of its tracks into `out` (zeroed
    /// before the call; add into it). `events` happen on the first frame;
    /// `pitch` holds the notes' pitch curves; `time` is the score's time
    /// at the first frame and `step` its seconds per frame. `out` may have
    /// no frames, only events.
    fn render(
        &mut self,
        events: &[RtEvent],
        pitch: &[PitchPoint],
        time: f64,
        step: f64,
        out: &mut Buffers,
    );

    /// The frames its sound comes late by (a resampler's, an oversampler's).
    /// The clock moves the cursor back by the largest source's. (Lining
    /// sources of different latencies up with each other comes with the
    /// first one that has any: ours have none.)
    fn latency(&self) -> u32 {
        0
    }
}

/// A source's buffers: a left and a right per track, at most `capacity`
/// frames, allocated once.
#[derive(Debug)]
pub struct Buffers {
    data: Vec<Vec<f32>>,
    frames: usize,
}

impl Buffers {
    fn new(tracks: usize, capacity: usize) -> Buffers {
        Buffers {
            data: (0..tracks * 2).map(|_| vec![0.0; capacity]).collect(),
            frames: 0,
        }
    }

    /// The frames to render.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The number of tracks.
    pub fn tracks(&self) -> usize {
        self.data.len() / 2
    }

    /// Track `slot`'s left and right buffers, `frames()` long.
    pub fn track(&mut self, slot: usize) -> Option<(&mut [f32], &mut [f32])> {
        let n = self.frames;
        let (left, right) = self.data.get_mut(slot * 2..slot * 2 + 2)?.split_at_mut(1);
        Some((&mut left[0][..n], &mut right[0][..n]))
    }

    /// Every buffer, left then right per track, `frames()` long: for
    /// handing to a plugin.
    pub fn all(&mut self) -> impl Iterator<Item = &mut [f32]> {
        let n = self.frames;
        self.data.iter_mut().map(move |b| &mut b[..n])
    }

    fn clear(&mut self, frames: usize) {
        self.frames = frames;
        for b in &mut self.data {
            b[..frames].fill(0.0);
        }
    }
}

/// A source and its buffers, as the engine holds them.
pub struct Slot {
    source: Box<dyn Source>,
    buffers: Buffers,
    events: Vec<RtEvent>,
    /// The score's track of each of the source's tracks.
    tracks: Vec<usize>,
}

/// The events a source's list holds at least.
const EVENTS: usize = 64;

impl Slot {
    /// `source`, playing the score's `tracks` (in its own order), in
    /// pieces of at most `capacity` frames.
    pub fn new(source: Box<dyn Source>, tracks: Vec<usize>, capacity: usize) -> Slot {
        Slot {
            buffers: Buffers::new(tracks.len(), capacity),
            events: Vec::with_capacity(EVENTS),
            source,
            tracks,
        }
    }
}

/// The sources, handed to the engine while nothing plays.
pub struct Sources {
    slots: Vec<Slot>,
}

impl Sources {
    /// `slots`, in the order routes name them.
    pub fn new(slots: Vec<Slot>) -> Sources {
        Sources { slots }
    }

    /// Makes room in each source's event list for the most events
    /// `schedule` hands it at one instant, so the audio thread never grows
    /// one. Off the audio thread, before [`Control::set_sources`]. (Events
    /// beyond it, a frame apart in time, are handed over in a call of their
    /// own without frames.)
    pub fn fit(&mut self, schedule: &Schedule) {
        for (slot, &most) in self.slots.iter_mut().zip(&schedule.most_at_once) {
            slot.events.reserve(most.saturating_sub(slot.events.len()));
        }
    }

    fn latency(&self) -> u32 {
        self.slots
            .iter()
            .map(|s| s.source.latency())
            .max()
            .unwrap_or(0)
    }
}

/// A track's mixer settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackMix {
    /// 0 to 1.
    pub volume: f32,
    /// −1 (left) to 1 (right).
    pub pan: f32,
    /// Muted.
    pub mute: bool,
    /// Soloed: when any track is, only soloed tracks sound.
    pub solo: bool,
}

impl Default for TrackMix {
    fn default() -> Self {
        TrackMix {
            volume: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
        }
    }
}

/// The most tracks a score's mixer holds.
pub const MAX_TRACKS: usize = 256;

/// What the window tells the engine.
enum Command {
    Play { from: f64, count_in: bool },
    Pause,
    Seek(f64),
    Loop(Option<(f64, f64)>),
    Speed(f64),
    Mix(usize, TrackMix),
    Master(f32),
    Metronome(bool),
    Schedule(Arc<Schedule>),
    Sources(Box<Sources>),
}

/// What the engine hands back, to be dropped off the audio thread or told.
#[allow(dead_code)] // The schedule and sources are held only to be dropped.
enum Returned {
    Schedule(Arc<Schedule>),
    Sources(Box<Sources>),
    /// The song played to its end and stopped.
    Ended,
    /// A source panicked: it is silent from now on.
    Failed(usize),
}

/// A piece of played time, for the clock: from device frame `frame`, the
/// score's time `time`, advancing `step` seconds a frame (0 while the
/// count-in or silence plays: `time` is then where the music will start).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Piece {
    frame: u64,
    time: f64,
    step: f64,
    playing: bool,
    /// Counts pieces, so of two starting on one frame the later wins.
    n: u64,
}

/// The clock: what the engine has handed the device, shared with the
/// window through a sequence lock (the audio thread never waits for it).
struct Clock {
    seq: AtomicU64,
    /// Device frames handed over, at the start of the latest callback.
    frames: AtomicU64,
    /// When that callback started, in nanoseconds since `origin`.
    at: AtomicU64,
    /// Its delay to the speaker, in frames, plus the sources' latency.
    delay: AtomicU64,
    rate: AtomicU64,
    /// The latest pieces of played time, oldest first, as bits.
    pieces: [[AtomicU64; 5]; PIECES],
    origin: Instant,
}

/// Pieces kept: enough for the delay to the speaker to span loops.
const PIECES: usize = 16;

/// The engine's two halves, for `sample rate` and callbacks of at most
/// `capacity` frames.
pub fn engine(rate: u32, capacity: usize) -> (Engine, Control) {
    let (commands, from_window) = RingBuffer::new(256);
    let (to_window, returned) = RingBuffer::new(256);
    let clock = Arc::new(Clock {
        seq: AtomicU64::new(0),
        frames: AtomicU64::new(0),
        at: AtomicU64::new(0),
        delay: AtomicU64::new(0),
        rate: AtomicU64::new(u64::from(rate)),
        pieces: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
        origin: Instant::now(),
    });
    let engine = Engine {
        commands: from_window,
        returned: to_window,
        clock: clock.clone(),
        rate,
        capacity,
        schedule: Arc::new(Schedule::default()),
        sources: Box::new(Sources::new(Vec::new())),
        mix: [TrackMix::default(); MAX_TRACKS],
        // Not yet mixed: the first piece takes its gains at once.
        gains: [(f32::NAN, f32::NAN); MAX_TRACKS],
        master: 1.0,
        metronome: false,
        speed: 1.0,
        looped: None,
        state: State::Stopped,
        anchor: Piece::default(),
        device_frame: 0,
        next_start: 0,
        next_end: 0,
        next_click: 0,
        sounding: 0,
        clicks: Voice::default(),
        mixed: (vec![0.0; capacity], vec![0.0; capacity]),
        latest: 0,
        pieces: [Piece::default(); PIECES],
        count_end: 0.0,
    };
    let control = Control {
        commands,
        returned,
        clock,
        playing: false,
    };
    (engine, control)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Stopped,
    /// The count-in plays: the music starts at `count_end` score seconds…
    CountIn,
    Playing,
}

/// A click sounding: its frames left and its phase.
#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    left: u32,
    length: u32,
    phase: f32,
    pitch: f32,
    level: f32,
}

/// The audio thread's half.
pub struct Engine {
    commands: Consumer<Command>,
    returned: Producer<Returned>,
    clock: Arc<Clock>,
    rate: u32,
    capacity: usize,
    schedule: Arc<Schedule>,
    sources: Box<Sources>,
    mix: [TrackMix; MAX_TRACKS],
    /// The gains each track was mixed with at the end of the last piece,
    /// for ramps.
    gains: [(f32, f32); MAX_TRACKS],
    master: f32,
    metronome: bool,
    speed: f64,
    looped: Option<(f64, f64)>,
    state: State,
    anchor: Piece,
    device_frame: u64,
    /// The next note (by start) to start, by end to end, click to sound.
    next_start: usize,
    next_end: usize,
    next_click: usize,
    /// Whether notes may be sounding (an "all off" is due on a jump).
    sounding: usize,
    clicks: Voice,
    mixed: (Vec<f32>, Vec<f32>),
    latest: usize,
    pieces: [Piece; PIECES],
    /// Where the music starts after the count-in.
    count_end: f64,
}

impl Engine {
    /// The device's callback: fills `out`, `frames` frames of `channels`
    /// interleaved channels, which reach the speaker `delay_frames` frames
    /// from now.
    pub fn render(&mut self, out: &mut [f32], channels: usize, delay_frames: u64) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let started = Instant::now();
        let first = self.device_frame;
        self.take_commands();
        let mut done = 0;
        while done < frames {
            let n = self.piece(frames - done);
            for i in 0..n {
                let (l, r) = (self.mixed.0[i], self.mixed.1[i]);
                let frame = &mut out[(done + i) * channels..(done + i + 1) * channels];
                match channels {
                    1 => frame[0] = 0.5 * (l + r),
                    _ => {
                        frame[0] = l;
                        frame[1] = r;
                        frame[2..].fill(0.0);
                    }
                }
            }
            done += n;
        }
        self.publish(
            first,
            started,
            delay_frames + u64::from(self.sources.latency()),
        );
    }

    fn take_commands(&mut self) {
        while let Ok(c) = self.commands.pop() {
            match c {
                Command::Play { from, count_in } => self.play(from, count_in),
                Command::Pause => self.stop(),
                Command::Seek(t) => self.seek(t),
                Command::Loop(l) => self.looped = l.filter(|(a, b)| b > a),
                Command::Speed(s) => {
                    let time = self.time_at(self.device_frame);
                    self.speed = s.clamp(0.25, 2.0);
                    if self.state == State::Playing {
                        self.anchor_at(time, true);
                    }
                }
                Command::Mix(t, m) => {
                    if let Some(slot) = self.mix.get_mut(t) {
                        *slot = m;
                    }
                }
                Command::Master(m) => self.master = m.clamp(0.0, 2.0),
                Command::Metronome(on) => self.metronome = on,
                Command::Schedule(s) => {
                    if self.state == State::Stopped {
                        let old = std::mem::replace(&mut self.schedule, s);
                        self.give_back(Returned::Schedule(old));
                    } else {
                        self.give_back(Returned::Schedule(s));
                    }
                }
                Command::Sources(s) => {
                    if self.state == State::Stopped {
                        let old = std::mem::replace(&mut self.sources, s);
                        self.give_back(Returned::Sources(old));
                    } else {
                        self.give_back(Returned::Sources(s));
                    }
                }
            }
        }
    }

    fn give_back(&mut self, r: Returned) {
        // The window empties this queue whenever it acts and on every frame
        // it draws while playing; should it ever be full, the item is
        // dropped here, which only a test could cause.
        let _ = self.returned.push(r);
    }

    fn play(&mut self, from: f64, count_in: bool) {
        self.all_off();
        let from = from.clamp(0.0, self.schedule.length);
        self.position_at(from);
        if count_in && !self.schedule.count_in.is_empty() {
            self.state = State::CountIn;
            self.count_end = from;
            self.next_click = 0;
            let start = from - self.schedule.count_in_length;
            self.anchor_at(start, true);
        } else {
            self.state = State::Playing;
            self.anchor_at(from, true);
        }
    }

    fn stop(&mut self) {
        self.all_off();
        let time = self.time_at(self.device_frame);
        self.state = State::Stopped;
        self.anchor_at(time.max(0.0), false);
    }

    fn seek(&mut self, time: f64) {
        let time = time.clamp(0.0, self.schedule.length);
        self.all_off();
        self.position_at(time);
        if self.state == State::CountIn {
            self.state = State::Playing;
        }
        let playing = self.state == State::Playing;
        self.anchor_at(time, playing);
    }

    /// Puts the cursors on `time`: the next note to start is the first at or
    /// after it, and nothing is sounding.
    fn position_at(&mut self, time: f64) {
        self.next_start = self.schedule.first_start_from(time);
        self.next_end = self.schedule.first_end_after(time);
        self.next_click = self.schedule.clicks.partition_point(|c| c.time < time);
    }

    /// Every sounding note stops, now.
    fn all_off(&mut self) {
        if self.sounding == 0 {
            return;
        }
        for slot in &mut self.sources.slots {
            slot.events.clear();
            if slot.events.capacity() > 0 {
                slot.events.push(RtEvent {
                    kind: RtEventKind::ALL_OFF,
                    note: RtNote::default(),
                });
            }
        }
        self.sounding = 0;
        self.render_sources(0, 0.0, 0.0);
    }

    fn anchor_at(&mut self, time: f64, playing: bool) {
        self.anchor = Piece {
            frame: self.device_frame,
            time,
            step: if playing {
                self.speed / f64::from(self.rate)
            } else {
                0.0
            },
            playing,
            n: self.anchor.n + 1,
        };
        self.latest = (self.latest + 1) % PIECES;
        self.pieces[self.latest] = self.anchor;
    }

    /// The score's time at device frame `frame`.
    fn time_at(&self, frame: u64) -> f64 {
        self.anchor.time + (frame.saturating_sub(self.anchor.frame)) as f64 * self.anchor.step
    }

    /// The first device frame (at or after `self.device_frame`) whose time
    /// is at or after `time`; `u64::MAX` while time stands still.
    fn frame_of(&self, time: f64) -> u64 {
        if self.anchor.step <= 0.0 {
            return u64::MAX;
        }
        let frames = (time - self.anchor.time) / self.anchor.step;
        // An event exactly on a frame's time happens on that frame; the
        // margin absorbs the rounding of times that are exact in theory.
        let f = (frames - 1e-6).ceil().max(0.0);
        (self.anchor.frame + f as u64).max(self.device_frame)
    }

    /// Plays one piece: up to `room` frames, ending before the next event;
    /// the events due now are handed out first. Returns its frames, mixed
    /// into `self.mixed`.
    fn piece(&mut self, room: usize) -> usize {
        let room = room.min(self.capacity);
        // What happens now, then where the next thing happens.
        self.due_now();
        let next = self.next_event_frame();
        let n = (next.saturating_sub(self.device_frame) as usize).clamp(1, room);
        let time = self.time_at(self.device_frame);
        let step = self.anchor.step;
        self.mixed.0[..n].fill(0.0);
        self.mixed.1[..n].fill(0.0);
        // The sources render even while nothing plays (stopped, or the
        // count-in): a note let go of fades out as its sound set says,
        // rather than being cut off.
        self.render_sources(n, time, step);
        self.mix_tracks(n);
        self.render_clicks(n);
        let master = self.master;
        for v in self.mixed.0[..n]
            .iter_mut()
            .chain(self.mixed.1[..n].iter_mut())
        {
            *v *= master;
        }
        // The device frame advances as the callback's frames are written;
        // the piece's own frames are counted here.
        self.device_frame += n as u64;
        n
    }

    /// Hands out every event due on the current frame.
    fn due_now(&mut self) {
        let now = self.device_frame;
        loop {
            match self.state {
                State::CountIn => {
                    let s = &self.schedule;
                    let start = self.count_end - s.count_in_length;
                    if let Some(c) = s.count_in.get(self.next_click)
                        && self.frame_of(start + c.time) <= now
                    {
                        let c = *c;
                        self.next_click += 1;
                        self.click(c);
                        continue;
                    }
                    if self.frame_of(self.count_end) <= now {
                        self.state = State::Playing;
                        self.position_at(self.count_end);
                        self.anchor_at(self.count_end, true);
                        continue;
                    }
                    return;
                }
                State::Stopped => return,
                State::Playing => {}
            }
            // The loop's end, or the song's: a jump, or the end.
            let (end, jump) = match self.looped {
                Some((a, b)) => (b.min(self.schedule.length), Some(a)),
                None => (self.schedule.length, None),
            };
            if self.frame_of(end) <= now {
                match jump {
                    Some(a) => {
                        self.all_off();
                        self.position_at(a);
                        self.anchor_at(a, true);
                        continue;
                    }
                    None => {
                        self.stop();
                        self.give_back(Returned::Ended);
                        return;
                    }
                }
            }
            break;
        }
        let s = self.schedule.clone();
        for slot in &mut self.sources.slots {
            slot.events.clear();
        }
        // Ends before starts, so a note repeated on the same string ends
        // before its next one starts.
        while let Some(&i) = s.ends.get(self.next_end) {
            let r = &s.notes[i as usize];
            if self.frame_of(r.note.end) > now {
                break;
            }
            self.next_end += 1;
            // A note that started before the current position (a seek
            // passed its start) never sounded: no end for it.
            if (i as usize) < self.next_start {
                self.push(r.source, RtEventKind::NOTE_OFF, r.note);
            }
        }
        while let Some(r) = s.notes.get(self.next_start) {
            if self.frame_of(r.note.start) > now {
                break;
            }
            self.next_start += 1;
            self.sounding += 1;
            self.push(r.source, RtEventKind::NOTE_ON, r.note);
        }
        while let Some(c) = s.clicks.get(self.next_click) {
            if self.frame_of(c.time) > now {
                break;
            }
            let c = *c;
            self.next_click += 1;
            if self.metronome {
                self.click(c);
            }
        }
    }

    fn push(&mut self, source: usize, kind: u32, note: RtNote) {
        let Some(slot) = self.sources.slots.get_mut(source) else {
            return;
        };
        if slot.events.len() == slot.events.capacity() {
            // More at once than the schedule counted (events a frame apart
            // in time, on the same frame): hand these over now, in a call
            // without frames, and go on.
            let time = self.anchor.time;
            render_slot(slot, &self.schedule.pitch, 0, time, self.anchor.step);
            slot.events.clear();
        }
        if slot.events.capacity() > 0 {
            slot.events.push(RtEvent { kind, note });
        }
    }

    /// The frame of the next event, from the current one on.
    fn next_event_frame(&self) -> u64 {
        let s = &self.schedule;
        let mut next = u64::MAX;
        let mut at = |t: f64| next = next.min(self.frame_of(t));
        match self.state {
            State::CountIn => {
                let start = self.count_end - s.count_in_length;
                if let Some(c) = s.count_in.get(self.next_click) {
                    at(start + c.time);
                }
                at(self.count_end);
            }
            State::Playing => {
                if let Some(r) = s.notes.get(self.next_start) {
                    at(r.note.start);
                }
                if let Some(&i) = s.ends.get(self.next_end) {
                    at(s.notes[i as usize].note.end);
                }
                if self.metronome
                    && let Some(c) = s.clicks.get(self.next_click)
                {
                    at(c.time);
                }
                at(match self.looped {
                    Some((_, b)) => b.min(s.length),
                    None => s.length,
                });
            }
            State::Stopped => {}
        }
        next
    }

    fn render_sources(&mut self, frames: usize, time: f64, step: f64) {
        let pitch = &self.schedule.pitch;
        for (i, slot) in self.sources.slots.iter_mut().enumerate() {
            if !render_slot(slot, pitch, frames, time, step) {
                let _ = self.returned.push(Returned::Failed(i));
            }
            slot.events.clear();
        }
    }

    /// Adds every track into the mix, with its gains ramped from the last
    /// piece's to this one's so a change never clicks.
    fn mix_tracks(&mut self, n: usize) {
        let solo = self.mix.iter().any(|m| m.solo);
        for slot in &mut self.sources.slots {
            for (k, &track) in slot.tracks.iter().enumerate() {
                let Some(m) = self.mix.get(track) else {
                    continue;
                };
                let silent = m.mute || (solo && !m.solo);
                let v = if silent { 0.0 } else { m.volume };
                // Constant power.
                let angle = (m.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
                let target = (v * angle.cos(), v * angle.sin());
                let from = match self.gains[track] {
                    (l, _) if l.is_nan() => target,
                    g => g,
                };
                self.gains[track] = target;
                let Some((l, r)) = slot.buffers.track(k) else {
                    continue;
                };
                let inv = 1.0 / n as f32;
                for i in 0..n {
                    let w = i as f32 * inv;
                    let gl = from.0 + (target.0 - from.0) * w;
                    let gr = from.1 + (target.1 - from.1) * w;
                    // A track's left and right, each panned.
                    self.mixed.0[i] += l[i] * gl;
                    self.mixed.1[i] += r[i] * gr;
                }
            }
        }
    }

    /// Starts a click: the first beat of a bar higher.
    fn click(&mut self, c: Click) {
        let length = self.rate / 25; // 40 ms
        self.clicks = Voice {
            left: length,
            length,
            phase: 0.0,
            pitch: if c.strong { 1760.0 } else { 1320.0 },
            level: if c.strong { 0.5 } else { 0.35 },
        };
    }

    fn render_clicks(&mut self, n: usize) {
        let v = &mut self.clicks;
        let inc = v.pitch / self.rate as f32 * std::f32::consts::TAU;
        for i in 0..n {
            if v.left == 0 {
                break;
            }
            let envelope = v.left as f32 / v.length.max(1) as f32;
            let s = v.phase.sin() * v.level * envelope * envelope;
            v.phase = (v.phase + inc) % std::f32::consts::TAU;
            v.left -= 1;
            self.mixed.0[i] += s;
            self.mixed.1[i] += s;
        }
    }

    /// Tells the clock where the callback starting at the current frame
    /// stands.
    fn publish(&mut self, first: u64, started: Instant, delay: u64) {
        let c = &self.clock;
        let seq = c.seq.load(Ordering::Relaxed);
        c.seq.store(seq + 1, Ordering::Release);
        std::sync::atomic::fence(Ordering::Release);
        c.frames.store(first, Ordering::Relaxed);
        c.at.store(
            started.duration_since(c.origin).as_nanos() as u64,
            Ordering::Relaxed,
        );
        c.delay.store(delay, Ordering::Relaxed);
        for (k, p) in self.pieces.iter().enumerate() {
            let slot = &c.pieces[k];
            slot[0].store(p.frame, Ordering::Relaxed);
            slot[1].store(p.time.to_bits(), Ordering::Relaxed);
            slot[2].store(p.step.to_bits(), Ordering::Relaxed);
            slot[3].store(u64::from(p.playing), Ordering::Relaxed);
            slot[4].store(p.n, Ordering::Relaxed);
        }
        c.seq.store(seq + 2, Ordering::Release);
    }
}

/// Renders one source; false once it has failed.
fn render_slot(slot: &mut Slot, pitch: &[PitchPoint], frames: usize, time: f64, step: f64) -> bool {
    slot.buffers.clear(frames);
    slot.source
        .render(&slot.events, pitch, time, step, &mut slot.buffers);
    true
}

/// The window's half.
pub struct Control {
    commands: Producer<Command>,
    returned: Consumer<Returned>,
    clock: Arc<Clock>,
    playing: bool,
}

/// What [`Control::collect`] found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Collected {
    /// The song played to its end.
    pub ended: bool,
    /// Sources that failed, by index.
    pub failed: Vec<usize>,
}

impl Control {
    fn send(&mut self, c: Command) {
        // 256 commands waiting means the audio thread has stopped calling:
        // nothing would hear them anyway.
        let _ = self.commands.push(c);
    }

    /// Plays from `time` (score seconds at 100 % speed), after a bar of
    /// count-in if `count_in`.
    pub fn play(&mut self, time: f64, count_in: bool) {
        self.playing = true;
        self.send(Command::Play {
            from: time,
            count_in,
        });
    }

    /// Stops the sound where it is.
    pub fn pause(&mut self) {
        self.playing = false;
        self.send(Command::Pause);
    }

    /// Moves to `time`.
    pub fn seek(&mut self, time: f64) {
        self.send(Command::Seek(time));
    }

    /// Plays from `a` to `b` over and over, or straight on (`None`).
    pub fn set_loop(&mut self, l: Option<(f64, f64)>) {
        self.send(Command::Loop(l));
    }

    /// The speed, 0.25 to 2.0.
    pub fn set_speed(&mut self, speed: f64) {
        self.send(Command::Speed(speed));
    }

    /// Track `track`'s mixer settings.
    pub fn set_mix(&mut self, track: usize, mix: TrackMix) {
        self.send(Command::Mix(track, mix));
    }

    /// The master level.
    pub fn set_master(&mut self, level: f32) {
        self.send(Command::Master(level));
    }

    /// The metronome on or off.
    pub fn set_metronome(&mut self, on: bool) {
        self.send(Command::Metronome(on));
    }

    /// A new schedule. Taken only while nothing plays (no editing while
    /// playing); otherwise handed back.
    pub fn set_schedule(&mut self, s: Arc<Schedule>) {
        self.send(Command::Schedule(s));
    }

    /// New sources (after a schedule's routes changed). Taken only while
    /// nothing plays.
    pub fn set_sources(&mut self, s: Sources) {
        self.send(Command::Sources(Box::new(s)));
    }

    /// Whether play was asked for last (rather than pause), or the song has
    /// ended since.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Drops what the engine handed back and says what happened. Call it
    /// whenever the window acts, and on every frame it draws while playing.
    pub fn collect(&mut self) -> Collected {
        let mut out = Collected::default();
        while let Ok(r) = self.returned.pop() {
            match r {
                Returned::Ended => {
                    self.playing = false;
                    out.ended = true;
                }
                Returned::Failed(i) => out.failed.push(i),
                Returned::Schedule(_) | Returned::Sources(_) => {}
            }
        }
        out
    }

    /// The score's time being heard now (seconds at 100 % speed), or where
    /// the music will start while the count-in plays; `None` before
    /// anything was played.
    pub fn heard(&self) -> Option<f64> {
        self.heard_at(Instant::now())
    }

    /// The score's time heard at `now`.
    pub fn heard_at(&self, now: Instant) -> Option<f64> {
        let (frames, at, delay, rate, pieces) = self.read_clock()?;
        // The device frame being written now, then the one heard.
        let since = now
            .checked_duration_since(self.clock.origin)
            .map_or(0, |d| d.as_nanos() as u64)
            .saturating_sub(at);
        let ahead = (since.saturating_mul(rate) / 1_000_000_000).min(rate);
        let heard = (frames + ahead).saturating_sub(delay);
        Some(time_of(&pieces, heard))
    }

    /// The score's time the engine played at device frame `frame` (as far
    /// back as the pieces it keeps reach): the clock without the delay to
    /// the speaker, for tests.
    pub fn played_at(&self, frame: u64) -> Option<f64> {
        let (.., pieces) = self.read_clock()?;
        Some(time_of(&pieces, frame))
    }

    /// The clock as the engine last published it: device frames at the
    /// latest callback's start, when it started, the delay, the rate and
    /// the pieces. `None` before the first callback.
    #[allow(clippy::type_complexity)]
    fn read_clock(&self) -> Option<(u64, u64, u64, u64, [Piece; PIECES])> {
        let c = &self.clock;
        loop {
            let seq = c.seq.load(Ordering::Acquire);
            if seq % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let frames = c.frames.load(Ordering::Relaxed);
            let at = c.at.load(Ordering::Relaxed);
            let delay = c.delay.load(Ordering::Relaxed);
            let rate = c.rate.load(Ordering::Relaxed).max(1);
            let pieces: [Piece; PIECES] = std::array::from_fn(|k| {
                let p = &c.pieces[k];
                Piece {
                    frame: p[0].load(Ordering::Relaxed),
                    time: f64::from_bits(p[1].load(Ordering::Relaxed)),
                    step: f64::from_bits(p[2].load(Ordering::Relaxed)),
                    playing: p[3].load(Ordering::Relaxed) != 0,
                    n: p[4].load(Ordering::Relaxed),
                }
            });
            std::sync::atomic::fence(Ordering::Acquire);
            if c.seq.load(Ordering::Relaxed) != seq {
                continue;
            }
            if seq == 0 {
                return None;
            }
            return Some((frames, at, delay, rate, pieces));
        }
    }
}

/// The score's time at device frame `frame`: the latest piece starting at
/// or before it, carried on.
fn time_of(pieces: &[Piece], frame: u64) -> f64 {
    let piece = pieces
        .iter()
        .filter(|p| p.n > 0 && p.frame <= frame)
        .max_by_key(|p| (p.frame, p.n))
        .copied()
        .unwrap_or_default();
    piece.time + (frame - piece.frame) as f64 * piece.step
}
