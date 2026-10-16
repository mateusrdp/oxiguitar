// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! GUI spike in Slint 1.18. The screen is declared in `ui/app.slint`; this
//! file feeds it the palette icons, the tracks and the rendered score, and
//! handles interface zoom (Ctrl + wheel, Ctrl + + / − / 0) for the whole window.

use std::cell::RefCell;
use std::rc::Rc;

use oxiguitar_display::Theme;
use slint::winit_030::winit::event::{ElementState, MouseScrollDelta, WindowEvent as WinitEvent};
use slint::winit_030::winit::keyboard::{Key, KeyCode, PhysicalKey};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};
use spike_common::{Canvas, PaletteState, TOOLS};

slint::include_modules!();

/// Interface zoom steps, as in web browsers.
const UI_ZOOMS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
/// Size of a palette button, in logical pixels.
const BUTTON: f32 = 32.0;
/// Pixels of smooth (touchpad) scrolling per interface-zoom step.
const PIXELS_PER_STEP: f64 = 60.0;

struct State {
    palette: PaletteState,
    canvas: Canvas,
    search: String,
    /// Score zoom, 0.25 to 4.
    zoom: f32,
    /// Scale factor the system gives the window.
    system_scale: f32,
    /// Interface zoom chosen with Ctrl + wheel or Ctrl + + / − / 0.
    ui_zoom: f32,
    ctrl: bool,
    wheel_pixels: f64,
    /// Palette icons at the current scale, and the scale they were drawn for.
    icons: Vec<Image>,
    icons_scale: f32,
}

type Shared = Rc<RefCell<State>>;

fn main() -> Result<(), slint::PlatformError> {
    // The winit backend gives us the window's raw events for interface zoom.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;
    let state: Shared = Rc::new(RefCell::new(State {
        palette: PaletteState::default(),
        canvas: Canvas::new(),
        search: String::new(),
        zoom: 1.0,
        system_scale: 1.0,
        ui_zoom: 1.0,
        ctrl: false,
        wheel_pixels: 0.0,
        icons: Vec::new(),
        icons_scale: 0.0,
    }));

    let tracks: Vec<TrackItem> = spike_common::tracks()
        .into_iter()
        .map(|t| TrackItem {
            name: t.name.into(),
            instrument: t.instrument.into(),
            mute: t.mute,
            solo: t.solo,
            volume: t.volume,
        })
        .collect();
    ui.set_tracks(ModelRc::new(VecModel::from(tracks)));
    ui.set_shortcut_lines(lines(&[
        "Ctrl + mouse wheel: zoom the interface",
        "Ctrl + +  /  Ctrl + −  /  Ctrl + 0: zoom the interface in, out, back to 100 %",
        "Score zoom: the slider at the bottom right of the score; click its percentage for 100 %",
        "1 to 6: note values, whole to thirty-second;  .  dotted;  R  rest",
        "H hammer-on, S slide, B bend, V vibrato, P palm mute, L let ring, X dead note, G ghost note, A accent, T staccato",
        "Ctrl + K: find a tool;  F7 / F8: show or hide the palette / tracks",
        "Click a palette heading to collapse or expand it.",
    ]));
    ui.set_about_lines(lines(&[
        "Guitar tablature and score viewer, editor and player. GUI-toolkit spike, Slint 1.18.",
        "Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. GPL-3.0.",
        "Music font: Bravura (Steinberg, SIL OFL 1.1). Text font: Noto Sans (SIL OFL 1.1).",
    ]));
    set_page_size(&ui, &state.borrow());

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_tool_clicked(move |i| {
        let Some(ui) = weak.upgrade() else { return };
        st.borrow_mut().palette.toggle(i as usize);
        refresh_palette(&ui, &mut st.borrow_mut());
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_shortcut(move |text| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        let mut chars = text.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return false;
        };
        let hit = st.borrow_mut().palette.press(c).is_some();
        if hit {
            refresh_palette(&ui, &mut st.borrow_mut());
        }
        hit
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_search_edited(move |text| {
        let Some(ui) = weak.upgrade() else { return };
        st.borrow_mut().search = text.to_string();
        refresh_palette(&ui, &mut st.borrow_mut());
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_zoom_moved(move |v| {
        let Some(ui) = weak.upgrade() else { return };
        // Snap to 100 % near the middle of the slider.
        let v = if v.abs() < 0.06 { 0.0 } else { v };
        ui.set_zoom_log(v);
        st.borrow_mut().zoom = 2f32.powf(v).clamp(0.25, 4.0);
        set_page_size(&ui, &st.borrow());
        render(&ui, &mut st.borrow_mut());
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_canvas_changed(move || {
        if let Some(ui) = weak.upgrade() {
            render(&ui, &mut st.borrow_mut());
        }
    });

    ui.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    install_interface_zoom(&ui, &state);

    // First frame, once the window has its size and scale.
    let weak = ui.as_weak();
    let st = state.clone();
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        if let Some(ui) = weak.upgrade() {
            let mut s = st.borrow_mut();
            s.system_scale = ui.window().scale_factor();
            refresh_palette(&ui, &mut s);
            render(&ui, &mut s);
        }
    });

    ui.run()
}

fn lines(items: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    ))
}

/// Ctrl + wheel and Ctrl + + / − / 0 scale the whole interface, wherever the
/// pointer or the keyboard focus is. The winit event filter sees them before
/// any widget, so the score never scrolls by accident.
fn install_interface_zoom(ui: &AppWindow, state: &Shared) {
    let weak = ui.as_weak();
    let st = state.clone();
    ui.window().on_winit_window_event(move |_, event| {
        let mut s = st.borrow_mut();
        let step = match event {
            WinitEvent::ModifiersChanged(m) => {
                s.ctrl = m.state().control_key();
                return EventResult::Propagate;
            }
            WinitEvent::ScaleFactorChanged { scale_factor, .. } => {
                // The system scale changed (another monitor, a new setting):
                // keep our interface zoom on top of it.
                s.system_scale = *scale_factor as f32;
                0
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
        let scale = s.system_scale * s.ui_zoom;
        if std::env::var_os("OXIGUITAR_TRACE_ZOOM").is_some() {
            println!(
                "interface zoom {:.0} % (scale {scale:.2})",
                s.ui_zoom * 100.0
            );
        }
        drop(s);
        // Apply after winit's own handling of this event has finished.
        let weak = weak.clone();
        let st = st.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            if let Some(ui) = weak.upgrade() {
                let window = ui.window();
                window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                });
                // The window keeps its size on screen; its content gets fewer,
                // larger logical pixels.
                let physical = window.size();
                window.dispatch_event(slint::platform::WindowEvent::Resized {
                    size: slint::LogicalSize::new(
                        physical.width as f32 / scale,
                        physical.height as f32 / scale,
                    ),
                });
                let mut s = st.borrow_mut();
                refresh_palette(&ui, &mut s);
                render(&ui, &mut s);
            }
        });
        if matches!(event, WinitEvent::ScaleFactorChanged { .. }) {
            EventResult::Propagate
        } else {
            EventResult::PreventDefault
        }
    });
}

fn step_ui_zoom(zoom: f32, steps: i32) -> f32 {
    let i = UI_ZOOMS
        .iter()
        .position(|z| (*z - zoom).abs() < 1e-3)
        .unwrap_or(5) as i32;
    UI_ZOOMS[(i + steps).clamp(0, UI_ZOOMS.len() as i32 - 1) as usize]
}

fn tool_item(s: &State, i: usize) -> ToolItem {
    let t = &TOOLS[i];
    ToolItem {
        index: i as i32,
        name: t.name.into(),
        key: t.key.to_ascii_uppercase().to_string().into(),
        icon: s.icons.get(i).cloned().unwrap_or_default(),
        on: s.palette.is_on(i),
    }
}

fn refresh_palette(ui: &AppWindow, s: &mut State) {
    // Icons are drawn for the window's physical pixels; redraw them when the
    // scale changes.
    let scale = ui.window().scale_factor();
    if (scale - s.icons_scale).abs() > 1e-3 {
        let px = (BUTTON * scale).round() as u32;
        s.icons = spike_common::icons::tool_icons(px)
            .into_iter()
            .map(|icon| {
                let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    &icon.rgba, icon.size, icon.size,
                );
                Image::from_rgba8_premultiplied(buf)
            })
            .collect();
        s.icons_scale = scale;
    }
    let group = |name: &str| -> ModelRc<ToolItem> {
        let v: Vec<ToolItem> = (0..TOOLS.len())
            .filter(|&i| TOOLS[i].group == name)
            .map(|i| tool_item(s, i))
            .collect();
        ModelRc::new(VecModel::from(v))
    };
    ui.set_values(group("Note values"));
    ui.set_effects(group("Effects"));
    let results: Vec<ToolItem> = if s.search.trim().is_empty() {
        Vec::new()
    } else {
        spike_common::search(&s.search)
            .into_iter()
            .map(|i| tool_item(s, i))
            .collect()
    };
    ui.set_results(ModelRc::new(VecModel::from(results)));
}

fn set_page_size(ui: &AppWindow, s: &State) {
    let (w, h) = s.canvas.page_pixels(s.zoom);
    ui.set_page_width(w);
    ui.set_page_height(h);
    ui.set_zoom_text(format!("{:.0} %", s.zoom * 100.0).into());
}

fn theme(ui: &AppWindow) -> Theme {
    if ui.get_dark_page() {
        Theme::DARK
    } else {
        Theme::LIGHT
    }
}

/// Render the part of the page the Flickable shows, at the window's scale.
fn render(ui: &AppWindow, s: &mut State) {
    const MARGIN: f32 = 24.0;
    let (pw, ph) = s.canvas.page_pixels(s.zoom);
    let (vx, vy, vw, vh) = (
        ui.get_view_x(),
        ui.get_view_y(),
        ui.get_view_width(),
        ui.get_view_height(),
    );
    // Visible part of the page, in logical pixels of the Flickable's content.
    let x0 = vx.max(MARGIN);
    let y0 = vy.max(MARGIN);
    let x1 = (vx + vw).min(MARGIN + pw);
    let y1 = (vy + vh).min(MARGIN + ph);
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        ui.set_score_width(0.0);
        return;
    }
    let dpr = ui.window().scale_factor();
    let pt_per_px = 72.0 / 96.0 / s.zoom;
    let w = ((x1 - x0) * dpr).round() as u32;
    let h = ((y1 - y0) * dpr).round() as u32;
    let f = s.canvas.frame_themed(
        (x0 - MARGIN) * pt_per_px,
        (y0 - MARGIN) * pt_per_px,
        w,
        h,
        s.zoom,
        dpr,
        &theme(ui),
    );
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&f.rgba, f.width, f.height);
    ui.set_score(Image::from_rgba8_premultiplied(buf));
    ui.set_score_x(x0);
    ui.set_score_y(y0);
    ui.set_score_width(x1 - x0);
    ui.set_score_height(y1 - y0);
}
