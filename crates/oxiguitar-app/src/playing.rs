// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The player in the window (M4 slice 4): the Player tab's controls, the
//! keys, the mixer's way to the sound, the frames drawn while the sound
//! runs, and what the player's thread says. The rules are in
//! [`crate::player`] and the thread in [`crate::sound`]; this joins them
//! to the window.

use std::time::Duration;

use oxiguitar_host::Level;
use slint::{ModelRc, SharedString, VecModel};

use crate::player::{Do, Place, State, span_of};
use crate::sound::{FromSound, ToSound};
use crate::{Shared, message, sync_footer};

/// The manual's page on getting better sounds.
pub(crate) const BETTER_SOUNDS: &str = "https://mateusrdp.github.io/oxiguitar/better-sounds.html";

/// How long the output stays open after the sound stops, for the last
/// notes to ring out, in seconds.
const RING_OUT: f64 = 2.5;

/// The key settings.json keeps the player's output under.
const KEY: &str = "player";

/// The player's thread said something.
pub(crate) fn from_sound(app: &Shared, m: FromSound) {
    let mut dos = Vec::new();
    match m {
        FromSound::Ready(ready) => {
            let silent = ready.silent.clone();
            dos = app.borrow_mut().player.ready(*ready);
            for s in silent {
                tell_once(app, Level::Warning, s, None);
            }
        }
        FromSound::Unplayable(why) => {
            dos = app.borrow_mut().player.unplayable_now(why.clone());
            let link = why
                .contains("sound set")
                .then_some(("Better sounds", BETTER_SOUNDS));
            tell_once(app, Level::Warning, why, link);
        }
        FromSound::NewEngine(control) => app.borrow_mut().player.new_engine(control),
        FromSound::Opened => dos = app.borrow_mut().player.opened(),
        FromSound::OpenFailed(why) => {
            app.borrow_mut().player.closed();
            message(app, Level::Error, format!("No sound: {why}."));
        }
        FromSound::Closed => app.borrow_mut().player.closed(),
        FromSound::Devices(devices) => outputs_found(app, devices),
    }
    act(app, dos);
    sync(app);
}

/// Says `text` in the footer, once a session.
fn tell_once(app: &Shared, level: Level, text: String, link: Option<(&str, &str)>) {
    {
        let mut s = app.borrow_mut();
        if !s.player_told.insert(text.clone()) {
            return;
        }
        match link {
            Some((t, url)) => s.footer.push_with_link(level, text, t, url),
            None => s.footer.push(level, text),
        }
    }
    sync_footer(app);
}

/// The outputs the audio-device plugin lists (`Err`: there is none).
fn outputs_found(app: &Shared, devices: Result<Vec<String>, String>) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let mut s = app.borrow_mut();
    match devices {
        Ok(names) => {
            let mut all = vec!["System default".to_owned()];
            all.extend(names);
            let wanted = s
                .stored
                .get(KEY)
                .and_then(|v| v.get("output").and_then(|o| o.as_str()).map(str::to_owned))
                .unwrap_or_default();
            let index = all.iter().position(|n| *n == wanted).unwrap_or(0);
            s.output = if index == 0 { String::new() } else { wanted };
            ui.set_outputs(ModelRc::new(VecModel::from(
                all.iter().map(SharedString::from).collect::<Vec<_>>(),
            )));
            ui.set_output_index(index as i32);
            ui.set_outputs_available(true);
            s.outputs = all;
            s.outputs_available = true;
        }
        Err(why) => {
            s.outputs_available = false;
            ui.set_outputs_available(false);
            ui.set_outputs(ModelRc::new(VecModel::from(vec![SharedString::from(
                "No audio output plugin",
            )])));
            drop(s);
            tell_once(app, Level::Warning, why, None);
        }
    }
}

/// Does what the player asked of the window.
pub(crate) fn act(app: &Shared, dos: Vec<Do>) {
    for d in dos {
        match d {
            Do::Open => {
                let s = app.borrow();
                s.sound.send(ToSound::Open {
                    device: s.output.clone(),
                });
            }
            Do::Close => app.borrow().sound.send(ToSound::Close),
            Do::CloseAfter(n) => {
                let a = app.clone();
                slint::Timer::single_shot(Duration::from_secs_f64(RING_OUT), move || {
                    let dos = a.borrow_mut().player.close_if(n);
                    act(&a, dos);
                });
            }
        }
    }
}

/// Shows the player's state in the window, and tells the worker and the
/// plugins whether edits are refused.
pub(crate) fn sync(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let s = app.borrow();
    let p = &s.player;
    let has_score = s.track_count > 0;
    let available = has_score && s.outputs_available && p.can_play();
    let why = if !has_score {
        " (open a score first)".to_owned()
    } else if !s.outputs_available {
        " (no audio output plugin: see the footer)".to_owned()
    } else if p.unplayable().is_some() {
        " (nothing can play this score: see the footer)".to_owned()
    } else if !p.can_play() {
        " (getting the sounds ready)".to_owned()
    } else {
        String::new()
    };
    ui.set_player_available(available);
    ui.set_player_why(why.into());
    ui.set_player_state(match p.state() {
        State::Stopped => 0,
        State::Starting => 1,
        State::Playing => 2,
        State::Paused => 3,
    });
    ui.set_count_in_on(p.count_in());
    ui.set_loop_on(p.looping());
    ui.set_metronome_on(p.metronome());
    ui.set_player_speed(p.speed() as i32);
    ui.set_master_level(p.master());
    ui.set_player_running(p.state() == State::Playing);
    // Paused, the overview's playhead stays where the sound stopped;
    // stopped, at the start.
    let song = match (p.state(), p.heard(), p.timeline()) {
        (State::Paused, Some(t), Some(tl)) if tl.length() > 0.0 => {
            (t / tl.length()).clamp(0.0, 1.0) as f32
        }
        _ => 0.0,
    };
    ui.set_song_position(song);
    s.open_score.set_playing(p.refuses_edits());
}

/// The score was opened or changed: the player's thread builds it again.
pub(crate) fn score_changed(app: &Shared, opened: bool) {
    let dos = {
        let mut s = app.borrow_mut();
        let Some(score) = s.editing.score() else {
            return;
        };
        s.score_revision += 1;
        let revision = s.score_revision;
        let mut dos = Vec::new();
        if opened {
            // Another song: the last one stops.
            dos = s.player.stop().1;
            s.began = None;
        }
        s.player.score_changed(revision);
        let mixes: Vec<_> = (0..s.track_count).map(|i| s.mixer.get(i)).collect();
        s.player.set_mixes(&mixes);
        s.sound.send(ToSound::Score { score, revision });
        dos
    };
    act(app, dos);
    sync(app);
}

/// A Player tab control was used.
pub(crate) fn action(app: &Shared, name: &str) {
    let dos = match name {
        "play" => play(app, false),
        "play-selection" => play(app, true),
        "pause" => {
            pause(app);
            Vec::new()
        }
        "stop" => stop(app),
        "rewind" => {
            let to = app.borrow_mut().player.rewind();
            if let Some(p) = to {
                put_caret_at(app, p);
            }
            Vec::new()
        }
        "count-in" => {
            let mut s = app.borrow_mut();
            let on = !s.player.count_in();
            s.player.set_count_in(on);
            Vec::new()
        }
        "metronome" => {
            let mut s = app.borrow_mut();
            let on = !s.player.metronome();
            s.player.set_metronome(on);
            Vec::new()
        }
        "loop" => {
            let mut s = app.borrow_mut();
            let on = !s.player.looping();
            let span = loop_span(&s);
            s.player.set_loop(on, span);
            Vec::new()
        }
        _ => Vec::new(),
    };
    act(app, dos);
    sync(app);
}

/// The selection's span, for Loop.
fn loop_span(s: &crate::App) -> Option<(Place, Place)> {
    let score = s.editing.score()?;
    span_of(s.editing.selection()?, &score)
}

fn play(app: &Shared, selection: bool) -> Vec<Do> {
    let mut s = app.borrow_mut();
    let (Some(score), Some(caret)) = (s.editing.score(), s.editing.caret()) else {
        return Vec::new();
    };
    let span = if selection {
        match s.editing.selection().and_then(|sel| span_of(sel, &score)) {
            Some(sp) => Some(sp),
            None => return Vec::new(),
        }
    } else {
        None
    };
    // The loop follows the selection as it is now.
    if s.player.looping() {
        let l = loop_span(&s);
        s.player.set_loop(true, l);
    }
    if s.player.state() != State::Playing {
        s.began = Some(caret);
    }
    s.player.play(Place::of_caret(caret, &score), span)
}

fn pause(app: &Shared) {
    let at = app.borrow_mut().player.pause();
    if let Some(p) = at {
        put_caret_at(app, p);
    }
}

fn stop(app: &Shared) -> Vec<Do> {
    let (dos, back) = {
        let mut s = app.borrow_mut();
        let (_, dos) = s.player.stop();
        (dos, s.began.take())
    };
    if let Some(c) = back {
        let moved = app.borrow_mut().editing.put_caret(c);
        if moved {
            crate::caret_moved(app);
        }
    }
    dos
}

fn put_caret_at(app: &Shared, p: Place) {
    let moved = app.borrow_mut().editing.put_caret_at(p.bar, p.offset);
    if moved {
        crate::caret_moved(app);
    }
}

/// The player's keys, before the editor's: Space plays and pauses, as in
/// Guitar Pro 8; Ctrl + Left and Right move a bar back and on (the sound
/// while it runs, else the caret). Returns whether the key was taken.
pub(crate) fn key(app: &Shared, name: &str, shift: bool, ctrl: bool, alt: bool) -> bool {
    if name == " " && !shift && !ctrl && !alt {
        let state = app.borrow().player.state();
        if matches!(state, State::Playing | State::Starting) {
            action(app, "pause");
        } else if app.borrow().player.can_play() {
            action(app, "play");
        }
        return true;
    }
    if ctrl && !shift && !alt && (name == "left" || name == "right") {
        let by = if name == "left" { -1 } else { 1 };
        if app.borrow_mut().player.skip_bars(by) {
            return true;
        }
        // Not playing: the caret goes a bar back or on, as Guitar Pro's
        // Rewind and Fast Forward move its cursor.
        let moved = {
            let mut s = app.borrow_mut();
            let bars = s.editing.score().map_or(0, |sc| sc.bar_count());
            match s.editing.caret() {
                Some(c) => {
                    let to = c.bar.saturating_add_signed(by).min(bars.saturating_sub(1));
                    to != c.bar && s.editing.go_to_bar(to)
                }
                None => false,
            }
        };
        if moved {
            crate::caret_moved(app);
        }
        return true;
    }
    false
}

/// One frame while the sound runs: what the engine handed back is
/// dropped, the song's end noticed, and where the sound is, for what the
/// frame shows. Called by the window while drawing, so anything that
/// changes the window waits until the frame is done.
pub(crate) fn frame(app: &Shared) -> crate::ui::PlayFrame {
    let Ok(mut s) = app.try_borrow_mut() else {
        return crate::ui::PlayFrame { song: 0.0 };
    };
    let (ended, dos) = s.player.frame();
    let song = match (s.player.heard(), s.player.timeline()) {
        (Some(t), Some(tl)) if tl.length() > 0.0 => (t / tl.length()).clamp(0.0, 1.0) as f32,
        _ => 0.0,
    };
    if ended {
        let back = s.began.take();
        drop(s);
        let a = app.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(c) = back {
                let moved = a.borrow_mut().editing.put_caret(c);
                if moved {
                    crate::caret_moved(&a);
                }
            }
            act(&a, dos);
            sync(&a);
        });
    }
    crate::ui::PlayFrame { song }
}

/// The Output drop-down: `index` into the outputs listed, 0 the system's
/// default; kept in settings.json for next time.
pub(crate) fn output_chosen(app: &Shared, index: usize) {
    let mut s = app.borrow_mut();
    let Some(name) = s.outputs.get(index).cloned() else {
        return;
    };
    s.output = if index == 0 {
        String::new()
    } else {
        name.clone()
    };
    let value = serde_json::json!({ "output": s.output });
    if let Err(e) = s.stored.set(KEY, value) {
        s.footer.push(Level::Error, e);
        drop(s);
        sync_footer(app);
        return;
    }
    // A stream open now goes on with its output; the next one opens the
    // new one.
    if s.player.state() == State::Stopped {
        s.sound.send(ToSound::Close);
    }
}
