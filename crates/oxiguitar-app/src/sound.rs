// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The player's thread (M4 slice 4): what takes time or waits on a plugin,
//! kept off the window's thread. It prepares the instruments for the open
//! score's tracks (loading their sound sets: a SoundFont 3 set is decoded
//! the first time, which takes seconds), builds the timeline and the
//! schedule after every change, and opens and closes the audio device.
//!
//! It sleeps until asked (no polling). Of several changes waiting, only
//! the newest is built. The engine's window half ([`Control`]) stays with
//! the window; the audio half ([`Engine`]) lives here between streams and
//! inside the device's callback while a stream is open, and comes back
//! here when the stream closes (see [`Keeper`]), with the instruments it
//! holds, so closing the device never costs preparing them again.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use oxiguitar_host::Plugins;
use oxiguitar_host::api::audio_device;
use oxiguitar_model::Score;
use oxiguitar_play::plugin_source::{self, Plan};
use oxiguitar_play::{Control, Engine, Route, Schedule, Sources, Timeline, engine};

/// The engine's largest piece, in frames.
pub(crate) const CAPACITY: usize = 512;

/// What the window asks.
pub(crate) enum ToSound {
    /// The score as it now is (opened or changed), counted by `revision`.
    Score { score: Arc<Score>, revision: u64 },
    /// Open the output named `device` (empty: the system's default).
    Open { device: String },
    /// Close the output.
    Close,
    /// The outputs' names, for the Output drop-down.
    Devices,
    /// The window is closing.
    Quit,
}

/// What the timeline and the instruments became for one revision.
pub(crate) struct Ready {
    pub(crate) revision: u64,
    pub(crate) timeline: Arc<Timeline>,
    pub(crate) schedule: Arc<Schedule>,
    /// New instruments, when the tracks' sounds changed (else the engine
    /// keeps the ones it has).
    pub(crate) sources: Option<Sources>,
    /// Tracks nothing plays, with why, each said once.
    pub(crate) silent: Vec<String>,
}

/// What the thread tells the window.
pub(crate) enum FromSound {
    Ready(Box<Ready>),
    /// Nothing can play this score: why, in a sentence for the footer.
    Unplayable(String),
    /// The device runs at another rate: this engine, at its rate, takes
    /// the old one's place. Everything the window had told the old one is
    /// to be told again; the instruments and the schedule follow.
    NewEngine(Control),
    /// The output opened.
    Opened,
    /// The output couldn't open: why.
    OpenFailed(String),
    Closed,
    /// The outputs' names, or why there is no audio output.
    Devices(Result<Vec<String>, String>),
}

/// The engine while a stream has it: it plays in the device's callback,
/// and goes back to `home` when the stream drops it, which the host does
/// on the thread closing the stream, never on the audio thread.
struct Keeper {
    engine: Option<Engine>,
    home: Arc<Mutex<Option<Engine>>>,
}

impl Drop for Keeper {
    fn drop(&mut self) {
        if let Some(e) = self.engine.take() {
            *self.home.lock().unwrap_or_else(PoisonError::into_inner) = Some(e);
        }
    }
}

/// The player's thread.
pub(crate) struct Sound {
    to: Sender<ToSound>,
    thread: Option<JoinHandle<()>>,
}

impl Sound {
    /// Starts the thread, with the engine at `rate`; `tell` hands what it
    /// says to the window (from this thread). Returns the engine's window
    /// half.
    pub(crate) fn start(
        plugins: Plugins,
        tell: impl Fn(FromSound) + Send + 'static,
    ) -> (Sound, Control) {
        let rate = oxiguitar_play::engine::RATE;
        let (eng, control) = engine(rate, CAPACITY);
        let (to, from) = channel();
        let thread = std::thread::Builder::new()
            .name("oxiguitar-sound".into())
            .spawn(move || {
                Thread {
                    plugins,
                    tell: Box::new(tell),
                    rate,
                    engine: Some(eng),
                    home: Arc::new(Mutex::new(None)),
                    stream: None,
                    plan: None,
                    routes: Vec::new(),
                    sources: 0,
                    score: None,
                }
                .run(&from);
            })
            .ok();
        (Sound { to, thread }, control)
    }

    pub(crate) fn send(&self, m: ToSound) {
        let _ = self.to.send(m);
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        let _ = self.to.send(ToSound::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Thread {
    plugins: Plugins,
    tell: Box<dyn Fn(FromSound) + Send>,
    /// The rate the engine and the instruments run at.
    rate: u32,
    /// The engine, while no stream has it.
    engine: Option<Engine>,
    home: Arc<Mutex<Option<Engine>>>,
    stream: Option<oxiguitar_host::realtime::Stream>,
    /// What the instruments were prepared for, and the routes then.
    plan: Option<Plan>,
    routes: Vec<Option<Route>>,
    sources: usize,
    /// The latest score, for preparing again at another rate.
    score: Option<(Arc<Score>, u64)>,
}

impl Thread {
    fn run(mut self, from: &Receiver<ToSound>) {
        while let Ok(first) = from.recv() {
            // Of several scores waiting, only the newest is built.
            let mut next = Some(first);
            while let Some(m) = next.take() {
                let m = match m {
                    ToSound::Score { .. } => {
                        let mut newest = m;
                        loop {
                            match from.try_recv() {
                                Ok(s @ ToSound::Score { .. }) => newest = s,
                                Ok(other) => {
                                    next = Some(other);
                                    break;
                                }
                                Err(_) => break,
                            }
                        }
                        newest
                    }
                    other => other,
                };
                match m {
                    ToSound::Score { score, revision } => self.score(score, revision, false),
                    ToSound::Open { device } => self.open(&device),
                    ToSound::Close => self.close(),
                    ToSound::Devices => (self.tell)(FromSound::Devices(self.devices())),
                    ToSound::Quit => {
                        self.close();
                        return;
                    }
                }
            }
        }
        self.close();
    }

    /// The timeline and schedule for `score`, and its instruments when its
    /// tracks' sounds differ from what they were prepared for (or `fresh`:
    /// at a new rate).
    fn score(&mut self, score: Arc<Score>, revision: u64, fresh: bool) {
        self.score = Some((score.clone(), revision));
        let timeline = Arc::new(Timeline::new(&score));
        let plan = plugin_source::plan(&self.plugins, &score);
        let mut sources = None;
        let mut silent = Vec::new();
        if fresh || self.plan.as_ref() != Some(&plan) {
            if plan.is_empty() {
                self.plan = Some(plan);
                self.routes = vec![None; score.tracks.len()];
                self.sources = 0;
                (self.tell)(FromSound::Unplayable(nothing_plays(&self.plugins)));
                return;
            }
            match plugin_source::prepare(&self.plugins, &score, &plan, self.rate, CAPACITY) {
                Ok(p) => {
                    self.routes = p.routes;
                    self.sources = p.count;
                    silent = p.silent;
                    sources = Some(p.sources);
                    self.plan = Some(plan);
                }
                Err(why) => {
                    self.plan = None;
                    self.routes = vec![None; score.tracks.len()];
                    self.sources = 0;
                    (self.tell)(FromSound::Unplayable(why));
                    return;
                }
            }
        }
        let routes: Vec<Option<Route>> = (0..score.tracks.len())
            .map(|t| self.routes.get(t).copied().flatten())
            .collect();
        let schedule = Schedule::new(&timeline, &routes, self.sources);
        // The instruments the engine already has were fitted to an earlier
        // schedule; more events at once than they have room for are handed
        // over in a call of their own (engine.rs), never by growing a list.
        if let Some(s) = &mut sources {
            s.fit(&schedule);
        }
        (self.tell)(FromSound::Ready(Box::new(Ready {
            revision,
            timeline,
            schedule: Arc::new(schedule),
            sources,
            silent,
        })));
    }

    fn open(&mut self, device: &str) {
        if self.stream.is_some() {
            return;
        }
        let Some(eng) = self.engine.take() else {
            (self.tell)(FromSound::OpenFailed("the player isn't ready".into()));
            return;
        };
        let rate = self.rate;
        let mut keeper = Keeper {
            engine: Some(eng),
            home: self.home.clone(),
        };
        let opened = self
            .plugins
            .open_stream(rate, device, move |out, channels, delay_ns| {
                if let Some(e) = keeper.engine.as_mut() {
                    let delay = delay_ns.saturating_mul(u64::from(rate)) / 1_000_000_000;
                    e.render(out, channels, delay);
                }
            });
        match opened {
            Ok(stream) => {
                let got = stream.opened().rate;
                if got == rate {
                    (self.tell)(FromSound::Opened);
                    self.stream = Some(stream);
                    return;
                }
                // The device runs at another rate: the engine and the
                // instruments again, at its rate, then the stream again.
                drop(stream);
                self.take_home();
                self.rate = got;
                let (eng, control) = engine(got, CAPACITY);
                self.engine = Some(eng);
                (self.tell)(FromSound::NewEngine(control));
                if let Some((score, revision)) = self.score.clone() {
                    self.score(score, revision, true);
                }
                let Some(eng) = self.engine.take() else {
                    return;
                };
                let mut keeper = Keeper {
                    engine: Some(eng),
                    home: self.home.clone(),
                };
                match self
                    .plugins
                    .open_stream(got, device, move |out, channels, delay_ns| {
                        if let Some(e) = keeper.engine.as_mut() {
                            let delay = delay_ns.saturating_mul(u64::from(got)) / 1_000_000_000;
                            e.render(out, channels, delay);
                        }
                    }) {
                    Ok(stream) => {
                        (self.tell)(FromSound::Opened);
                        self.stream = Some(stream);
                    }
                    Err(e) => {
                        self.take_home();
                        (self.tell)(FromSound::OpenFailed(e.to_string()));
                    }
                }
            }
            Err(e) => {
                self.take_home();
                (self.tell)(FromSound::OpenFailed(e.to_string()));
            }
        }
    }

    /// The engine back from a stream that dropped it.
    fn take_home(&mut self) {
        if let Some(e) = self
            .home
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            self.engine = Some(e);
        }
    }

    fn close(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
            self.take_home();
            (self.tell)(FromSound::Closed);
        }
    }

    /// The outputs' names; an error when no audio-device plugin works
    /// here (probing loads it, which is why it is asked here).
    fn devices(&self) -> Result<Vec<String>, String> {
        if let Err(why) = self.plugins.service(audio_device::INTERFACE) {
            return Err(format!(
                "No audio output plugin works here, so nothing can be heard ({why}). The basic plugins include one for each system."
            ));
        }
        Ok(self
            .plugins
            .call_service(audio_device::INTERFACE, audio_device::DEVICES, &[])
            .ok()
            .and_then(|a| oxiguitar_host::api::decode::<Vec<String>>(&a).ok())
            .unwrap_or_default())
    }
}

/// Why nothing plays a score: no instrument for any of its tracks.
fn nothing_plays(plugins: &Plugins) -> String {
    if plugins.instruments().is_empty() {
        "No instrument plugin is installed, so nothing can play. The basic plugins include the SoundFont plugin.".to_owned()
    } else {
        "No installed instrument plays this score's tracks.".to_owned()
    }
}
