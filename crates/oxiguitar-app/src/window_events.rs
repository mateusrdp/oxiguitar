// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The system's window: winit events (dropped files, the system's scale, Ctrl + wheel), the interface zoom applied on top of the system's scale, and the zooms kept in settings.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// The window's raw events, before any widget sees them:
///
/// - Ctrl + wheel and Ctrl + + / − / 0 scale the whole interface, wherever
///   the pointer or the keyboard focus is, so the score never scrolls by
///   accident.
/// - The wheel over the page scrolls it (along the line when a Horizontal
///   score fits the window's height); with Shift it zooms the page around
///   the pointer (Mateus, 18 Oct 2026). Elsewhere (a list in a side area)
///   the wheel scrolls as usual.
/// - A file dropped on the window opens like File → Open.
///
/// **Drops don't work on Wayland** (ADR 0006; CONTRIBUTING.md, "Known
/// platform gaps"). Upstream (winit 0.30): `DroppedFile` comes on Windows,
/// macOS and X11 only: its Wayland backend has no drag and drop.
/// Upstream (slint 1.19): Slint passes no operating-system drops on. Listening ourselves would need our
/// own `wl_data_device` on winit's connection, through an `unsafe` raw
/// pointer and with no way to wake on its events but polling, which we
/// rejected. Drops will work on Wayland with no change here once the winit
/// in our Slint version emits `DroppedFile` on Wayland; or switch to Slint's
/// own event if Slint starts forwarding OS drops. Check either with
/// `scripts/wayland-dnd-test/`.
pub(crate) fn install_window_events(ui: &AppWindow, app: &Shared) {
    let a = app.clone();
    ui.window().on_winit_window_event(move |_, event| {
        let mut s = a.borrow_mut();
        let step = match event {
            WinitEvent::DroppedFile(path) => {
                // Several files dropped at once arrive one by one: open the
                // first, as File → Open opens one file.
                if !s.drop_pending {
                    s.drop_pending = true;
                    // Once this event is done with the app: unsaved
                    // changes are asked about first. winit sends a drop's
                    // files one after another in one turn of the event
                    // loop, and timers run at the start of the next turn:
                    // by then the drop's other files have been ignored.
                    let (a, path) = (a.clone(), path.clone());
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        a.borrow_mut().drop_pending = false;
                        unless_unsaved(&a, Then::Open(path));
                    });
                }
                return EventResult::PreventDefault;
            }
            WinitEvent::ModifiersChanged(m) => {
                // ⌘ on macOS, as Slint maps the menus' Control shortcuts.
                s.ctrl = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
                s.shift = m.state().shift_key();
                return EventResult::Propagate;
            }
            WinitEvent::CursorMoved { position, .. } => {
                s.cursor = Some((position.x, position.y));
                return EventResult::Propagate;
            }
            WinitEvent::CursorLeft { .. } => {
                s.cursor = None;
                return EventResult::Propagate;
            }
            WinitEvent::ScaleFactorChanged { scale_factor, .. } => {
                // The system scale changed (another monitor, a new setting):
                // keep our interface zoom on top of it.
                s.system_scale = *scale_factor as f32;
                0
            }
            // The wheel over the page: scroll it, or zoom it with Shift.
            WinitEvent::MouseWheel { delta, .. } if !s.ctrl => {
                let (Some(ui), Some((px, py))) = (s.ui.upgrade(), s.cursor) else {
                    return EventResult::Propagate;
                };
                let scale = f64::from(ui.window().scale_factor());
                let (x, y) = (
                    (px / scale) as f32 - ui.get_canvas_left(),
                    (py / scale) as f32 - ui.get_canvas_top(),
                );
                let over_page = (0.0..ui.get_canvas_width()).contains(&x)
                    && (0.0..ui.get_canvas_height()).contains(&y);
                if !over_page {
                    return EventResult::Propagate;
                }
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(p) => (
                        (p.x / PIXELS_PER_STEP) as f32,
                        (p.y / PIXELS_PER_STEP) as f32,
                    ),
                };
                let shift = s.shift;
                let layout = s.settings.layout;
                drop(s);
                if shift {
                    // macOS (and some Linux setups) turn Shift + wheel into a
                    // sideways scroll: zoom by whichever way it turned.
                    let notches = if dy != 0.0 { dy } else { dx };
                    if notches != 0.0 {
                        wheel_zoom(&a, notches, x, y);
                    }
                } else {
                    // A score on one line that fits the window's height
                    // scrolls along the line.
                    let along = layout == Layout::Horizontal
                        && dx == 0.0
                        && ui.get_content_height() + CANVAS_PAD <= ui.get_view_height();
                    if along {
                        ui.invoke_scroll_by(-dy * WHEEL_SCROLL, 0.0);
                    } else {
                        ui.invoke_scroll_by(-dx * WHEEL_SCROLL, -dy * WHEEL_SCROLL);
                    }
                }
                return EventResult::PreventDefault;
            }
            WinitEvent::MouseWheel { delta, .. } if s.ctrl => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(*y) * PIXELS_PER_STEP,
                    MouseScrollDelta::PixelDelta(p) => p.y,
                };
                s.wheel_pixels += dy;
                let steps = (s.wheel_pixels / PIXELS_PER_STEP).trunc();
                s.wheel_pixels -= steps * PIXELS_PER_STEP;
                if steps == 0.0 {
                    return EventResult::PreventDefault;
                }
                steps as i32
            }
            WinitEvent::KeyboardInput { event, .. }
                if s.ctrl && event.state == ElementState::Pressed =>
            {
                let by_char = match &event.logical_key {
                    Key::Character(c) => match c.as_str() {
                        "+" | "=" => Some(1),
                        "-" | "_" => Some(-1),
                        "0" => Some(i32::MIN),
                        _ => None,
                    },
                    _ => None,
                };
                let by_key = match event.physical_key {
                    PhysicalKey::Code(KeyCode::NumpadAdd) => Some(1),
                    PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(-1),
                    PhysicalKey::Code(KeyCode::Numpad0) => Some(i32::MIN),
                    _ => None,
                };
                match by_char.or(by_key) {
                    Some(step) => step,
                    None => return EventResult::Propagate,
                }
            }
            _ => return EventResult::Propagate,
        };
        s.ui_zoom = match step {
            i32::MIN => 1.0,
            0 => s.ui_zoom,
            n => step_ui_zoom(s.ui_zoom, n),
        };
        save_zooms(&mut s);
        drop(s);
        // Apply after winit's own handling of this event has finished.
        let a = a.clone();
        slint::Timer::single_shot(Duration::ZERO, move || apply_interface_scale(&a));
        if matches!(event, WinitEvent::ScaleFactorChanged { .. }) {
            EventResult::Propagate
        } else {
            EventResult::PreventDefault
        }
    });
}

/// Saves the score and interface zooms with the other View settings, so
/// they survive restarts (ADR 0010).
pub(crate) fn save_zooms(s: &mut App) {
    let prefs = ViewPrefs {
        score_zoom: s.zoom,
        ui_zoom: s.ui_zoom,
        ..s.prefs
    };
    if prefs != s.prefs {
        s.prefs = prefs;
        let value = prefs.to_value(s.stored.get(prefs::KEY).as_ref());
        if let Err(e) = s.stored.set(prefs::KEY, value) {
            s.footer.push(Level::Error, e);
        }
    }
}

/// Scales the window by the system's scale times our interface zoom, and
/// draws what depends on the scale again.
///
/// Upstream (slint 1.19): Slint has no call to change a window's scale
/// while it runs (`SLINT_SCALE_FACTOR` is read once, at the start), so we
/// tell it what winit would on a scale change: `ScaleFactorChanged`, then
/// `Resized` to the same physical size in fewer logical pixels. Anything
/// that reads `window().scale_factor()` afterwards gets the system's scale
/// times the zoom; `App::system_scale` keeps the system's part. On update:
/// if Slint gains such a call, use it here and keep the rest.
pub(crate) fn apply_interface_scale(a: &Shared) {
    let Some(ui) = a.borrow().ui.upgrade() else {
        return;
    };
    let scale = {
        let s = a.borrow();
        s.system_scale * s.ui_zoom
    };
    let window = ui.window();
    window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
        scale_factor: scale,
    });
    // The window keeps its size on screen; its content gets fewer, larger
    // logical pixels.
    let physical = window.size();
    window.dispatch_event(slint::platform::WindowEvent::Resized {
        size: slint::LogicalSize::new(
            physical.width as f32 / scale,
            physical.height as f32 / scale,
        ),
    });
    // Tiles and plugins' canvases are drawn for device pixels: draw them
    // again.
    resend_canvas_rooms(a, scale);
    {
        let mut s = a.borrow_mut();
        s.paint += 1;
        // Tiles sit in logical pixels, which still fit the score.
        retire_tiles(&mut s, 1.0);
    }
    canvas_changed(a);
}

pub(crate) fn step_ui_zoom(zoom: f32, steps: i32) -> f32 {
    let i = UI_ZOOMS
        .iter()
        .position(|z| (*z - zoom).abs() < 1e-3)
        .unwrap_or(5) as i32;
    UI_ZOOMS
        .get((i + steps).clamp(0, UI_ZOOMS.len() as i32 - 1) as usize)
        .copied()
        .unwrap_or(1.0)
}
