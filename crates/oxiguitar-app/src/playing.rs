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
use crate::ui::PlayFrame;
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
    ui.set_visual_on(s.visual);
    // Paused, the cursor and the overview's playhead stay where the sound
    // stopped; stopped, nothing shows.
    let still = match p.state() {
        State::Paused => p.heard().map(|t| shown_at(&s, t, false)),
        // Stopped, the playhead shows where Play starts: the caret.
        _ => match (s.editing.score(), s.editing.caret(), p.timeline()) {
            (Some(score), Some(c), Some(tl)) if tl.length() > 0.0 => {
                crate::player::time_of(tl, Place::of_caret(c, &score), 0).map(|(_, t)| PlayFrame {
                    song: (t / tl.length()).clamp(0.0, 1.0) as f32,
                    ..Default::default()
                })
            }
            _ => None,
        },
    };
    ui.set_still_frame(still.unwrap_or_default());
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
        "visual-metronome" => {
            let mut s = app.borrow_mut();
            s.visual = !s.visual;
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
    s.followed = None;
    s.player.play(Place::of_caret(caret, &score), span)
}

fn pause(app: &Shared) {
    let moved = {
        let mut s = app.borrow_mut();
        let at = s.player.pause();
        let moved = at.is_some_and(|p| s.editing.put_caret_at(p.bar, p.offset));
        // Where the pause leaves it, before anyone hears it moved.
        s.paused_at = s.editing.caret();
        moved
    };
    if moved {
        crate::caret_moved(app);
    }
}

/// The caret moved: moved away from where Pause left it, the pause ends
/// (Play starts from the caret, so the paused cursor would show a place
/// nothing will play from).
pub(crate) fn caret_moved(app: &Shared) {
    let dos = {
        let mut s = app.borrow_mut();
        if s.player.state() != State::Paused || s.editing.caret() == s.paused_at {
            return;
        }
        s.paused_at = None;
        s.began = None;
        s.player.stop().1
    };
    act(app, dos);
    sync(app);
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
/// changes the window (the page turning, the song's end) waits until the
/// frame is done.
pub(crate) fn frame(app: &Shared) -> PlayFrame {
    let Ok(mut s) = app.try_borrow_mut() else {
        return PlayFrame::default();
    };
    let (ended, dos) = s.player.frame();
    let out = match s.player.heard() {
        Some(t) => shown_at(&s, t, true),
        None => PlayFrame::default(),
    };
    // A new line or system: turn the page if the cursor nears its edge.
    let turn = out.shown && (s.followed != Some((out.top as i32, out.x as i32 / 64)));
    if turn {
        s.followed = Some((out.top as i32, out.x as i32 / 64));
    }
    drop(s);
    if turn {
        let a = app.clone();
        let f = out.clone();
        slint::Timer::single_shot(Duration::ZERO, move || follow(&a, f));
    }
    if ended {
        let back = app.borrow_mut().began.take();
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
    out
}

/// What shows of playback at score time `t`: the overview's playhead, the
/// cursor on the page and, while the sound runs (`running`), the visual
/// metronome's flash.
fn shown_at(s: &crate::App, t: f64, running: bool) -> PlayFrame {
    let Some(tl) = s.player.timeline() else {
        return PlayFrame::default();
    };
    let length = tl.length();
    let song = if length > 0.0 {
        (t / length).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let mut out = PlayFrame {
        song,
        ..Default::default()
    };
    if let (Some(score), Some((v, offset))) = (s.editing.score(), tl.find(t)) {
        let bar = tl.visits()[v].visit.bar;
        let bar_length = tl.map.bar_length(bar).to_f64();
        if let Some(c) = crate::cursor::at(s.editing.hits(), &score, bar, offset, bar_length)
            && let Some(o) = s.sheets.rects.get(c.sheet)
        {
            out.shown = true;
            out.x = o.x + c.x;
            out.top = o.y + c.top;
            out.bottom = o.y + c.bottom;
            out.beat = crate::ui::BoxItem {
                x: o.x + c.beat.x,
                y: o.y + c.beat.y,
                w: c.beat.width,
                h: c.beat.height,
            };
        }
    }
    if running && s.visual {
        // The latest click: bright for a moment, then dark.
        let i = tl.clicks.partition_point(|c| c.time <= t);
        if let Some(c) = i.checked_sub(1).and_then(|i| tl.clicks.get(i)) {
            let age = (t - c.time).max(0.0);
            let strength = if c.strong { 1.0 } else { 0.55 };
            out.flash = (strength * (1.0 - age / FLASH).max(0.0)) as f32;
        }
    }
    out
}

/// How long a flash of the visual metronome lasts, in score seconds.
const FLASH: f64 = 0.12;

/// Turns the page to keep the cursor in sight.
fn follow(app: &Shared, f: PlayFrame) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let px = crate::PX_PER_PT * app.borrow().zoom;
    let (ox, oy) = (ui.get_origin_x(), ui.get_origin_y());
    let view = (
        ui.get_view_x(),
        ui.get_view_y(),
        ui.get_view_width(),
        ui.get_view_height(),
    );
    if let Some((x, y)) =
        crate::cursor::follow(view, ox + f.x * px, oy + f.top * px, oy + f.bottom * px)
    {
        ui.invoke_scroll_to(x, y);
    }
}

/// A click on a track's overview, at `fraction` of the song: the sound
/// goes there while it plays, else the caret does.
pub(crate) fn overview_clicked(app: &Shared, fraction: f32) {
    let place = {
        let mut s = app.borrow_mut();
        let Some(tl) = s.player.timeline().cloned() else {
            return;
        };
        let t = f64::from(fraction.clamp(0.0, 1.0)) * tl.length();
        if s.player.seek(t) {
            return;
        }
        crate::player::place_at(&tl, t.min(tl.length() - 1e-6).max(0.0))
    };
    if let Some(p) = place {
        put_caret_at(app, p);
    }
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
