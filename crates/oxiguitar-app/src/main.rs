// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar desktop window (M1: the viewer).
//!
//! The screen is declared in `ui/app.slint` (Slint, ADR 0004) and follows
//! `docs/design/ui.md`. This file connects it to the worker thread
//! ([`worker`]), which reads, engraves and draws the score as tiles
//! ([`canvas`]), and handles interface zoom for the whole window.
//!
//! Usage: `oxiguitar [FILE]`. Nothing here touches the network.

#![windows_subsystem = "windows"]

mod canvas;
mod document;
mod worker;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use oxiguitar_display::Rect;
use oxiguitar_layout::{Layout, Paper};
use slint::winit_030::winit::event::{ElementState, MouseScrollDelta, WindowEvent as WinitEvent};
use slint::winit_030::winit::keyboard::{Key, KeyCode, PhysicalKey};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
    Weak,
};

use crate::canvas::{Sheets, TILE};
use crate::document::{Settings, ToolEntry};
use crate::worker::{Event, Request, Worker};

/// The code Slint generates from `ui/app.slint`.
mod ui {
    #![allow(missing_docs, unreachable_pub, clippy::all, clippy::todo)]
    slint::include_modules!();
}
use ui::{AppWindow, TileItem, ToolRow, TrackRow};

/// Interface zoom steps, as in web browsers.
const UI_ZOOMS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
/// Pixels of smooth (touchpad) scrolling per interface-zoom step.
const PIXELS_PER_STEP: f64 = 60.0;
/// Logical pixels per point at 100 % score zoom (96 dpi).
const PX_PER_PT: f32 = 96.0 / 72.0;
/// Room around the sheets on the canvas, in logical pixels (both sides).
const CANVAS_PAD: f32 = 48.0;
/// How long a status message stays.
const STATUS_TIME: Duration = Duration::from_secs(10);

struct App {
    ui: Weak<AppWindow>,
    worker: Worker,
    settings: Settings,
    track_count: usize,
    /// The newest engraving: its id and where its sheets sit, in points.
    engraving: u64,
    sheets: Sheets,
    /// Bumped whenever tiles must be drawn again (zoom, scale, dark page).
    paint: u64,
    tiles: HashMap<(u32, u32), Image>,
    tile_model: Rc<VecModel<TileItem>>,
    /// Score zoom, 0.25 to 4.
    zoom: f32,
    /// Scale factor the system gives the window, and our interface zoom.
    system_scale: f32,
    ui_zoom: f32,
    ctrl: bool,
    wheel_pixels: f64,
    pdf_name: String,
    folder: Option<PathBuf>,
    status_serial: u64,
}

type Shared = Rc<RefCell<App>>;

thread_local! {
    /// The app, for events arriving from the worker thread.
    static APP: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

fn main() -> Result<(), slint::PlatformError> {
    // The winit backend gives us the window's raw events for interface zoom.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;
    let tile_model = Rc::new(VecModel::<TileItem>::default());
    ui.set_tiles(ModelRc::from(tile_model.clone()));

    let worker = Worker::start(|event| {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                handle(&app, event);
            }
        });
    });
    let app: Shared = Rc::new(RefCell::new(App {
        ui: ui.as_weak(),
        worker,
        settings: Settings::default(),
        track_count: 0,
        engraving: 0,
        sheets: Sheets::default(),
        paint: 0,
        tiles: HashMap::new(),
        tile_model,
        zoom: 1.0,
        system_scale: 1.0,
        ui_zoom: 1.0,
        ctrl: false,
        wheel_pixels: 0.0,
        pdf_name: "score.pdf".to_owned(),
        folder: None,
        status_serial: 0,
    }));
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));

    ui.set_shortcut_lines(lines(&[
        "Ctrl + O: open a file;  Ctrl + Q: quit;  F8: show or hide the track list",
        "Mouse wheel, arrow keys, Page Up / Page Down, Home / End: scroll the score",
        "Score zoom: the slider at the bottom right of the score; click its percentage for 100 %",
        "Ctrl + mouse wheel: zoom the interface",
        "Ctrl + +  /  Ctrl + −  /  Ctrl + 0: zoom the interface in, out, back to 100 %",
        "Track list: tick tracks to show them together; click a name to show that track alone",
    ]));
    ui.set_about_lines(lines(&[
        &format!(
            "Guitar tablature and score viewer, editor and player. Version {}.",
            env!("CARGO_PKG_VERSION")
        ),
        "Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. Licensed under the GNU GPL, version 3 or later.",
        "Music font: Bravura (Steinberg, SIL OFL 1.1). Text font: Noto Sans (SIL OFL 1.1). GUI: Slint (GPL-3.0).",
        "OxiGuitar never connects to the internet on its own and sends nothing anywhere.",
    ]));
    set_tools(&ui, &tool_rows(&default_tools()));

    let a = app.clone();
    ui.on_open_file(move || open_dialog(&a));
    let a = app.clone();
    ui.on_export_pdf(move || export_dialog(&a));
    ui.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    let a = app.clone();
    ui.on_settings_changed(move || settings_changed(&a));
    let a = app.clone();
    ui.on_canvas_changed(move || canvas_changed(&a));
    let a = app.clone();
    ui.on_zoom_moved(move |v| zoom_moved(&a, v));
    let a = app.clone();
    ui.on_track_toggled(move |i| {
        let mut tracks = a.borrow().settings.tracks.clone();
        let i = i as usize;
        if let Some(pos) = tracks.iter().position(|&t| t == i) {
            if tracks.len() > 1 {
                tracks.remove(pos);
            }
        } else {
            tracks.push(i);
            tracks.sort_unstable();
        }
        set_tracks(&a, tracks);
    });
    let a = app.clone();
    ui.on_track_solo(move |i| set_tracks(&a, vec![i as usize]));
    let a = app.clone();
    ui.on_multitrack_toggled(move || {
        let (count, tracks) = {
            let s = a.borrow();
            (s.track_count, s.settings.tracks.clone())
        };
        let all: Vec<usize> = (0..count).collect();
        let next = if tracks == all {
            tracks.into_iter().take(1).collect()
        } else {
            all
        };
        set_tracks(&a, next);
    });
    let a = app.clone();
    ui.on_tool_picked(move |i| {
        a.borrow().worker.send(Request::ApplyTool(i as usize));
    });
    let a = app.clone();
    ui.on_status_dismissed(move || {
        if let Some(ui) = a.borrow().ui.upgrade() {
            ui.set_status(SharedString::new());
        }
    });

    install_interface_zoom(&ui, &app);

    // A file named on the command line (or by the system's file manager).
    if let Some(path) = std::env::args_os().nth(1) {
        app.borrow().worker.send(Request::Open(PathBuf::from(path)));
    }

    // Once the window has its size and scale.
    let a = app.clone();
    slint::Timer::single_shot(Duration::from_millis(50), move || {
        let scale = a.borrow().ui.upgrade().map(|ui| ui.window().scale_factor());
        if let Some(scale) = scale {
            a.borrow_mut().system_scale = scale;
            canvas_changed(&a);
        }
    });

    ui.run()?;
    APP.with(|a| a.borrow_mut().take());
    Ok(())
}

fn lines(items: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    ))
}

/// The Tools menu before any score is open: every tool, greyed out.
fn default_tools() -> Vec<ToolEntry> {
    oxiguitar_tools::all()
        .iter()
        .map(|t| ToolEntry {
            name: t.name().to_owned(),
            unavailable: None,
        })
        .collect()
}

/// Menu rows for the tools. Slint menus have no tooltips, so a tool that
/// cannot run carries its reason in its title, greyed out.
fn tool_rows(tools: &[ToolEntry]) -> Vec<ToolRow> {
    tools
        .iter()
        .map(|t| match &t.unavailable {
            None => ToolRow {
                title: t.name.as_str().into(),
                enabled: true,
            },
            Some(why) => ToolRow {
                title: format!("{} ({})", t.name, reason_phrase(why)).into(),
                enabled: false,
            },
        })
        .collect()
}

/// "The score already has key signatures." → "the score already has key
/// signatures", to follow a menu entry in brackets.
fn reason_phrase(why: &str) -> String {
    let why = why.trim().trim_end_matches('.');
    let mut chars = why.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn set_tools(ui: &AppWindow, rows: &[ToolRow]) {
    ui.set_tools(ModelRc::new(VecModel::from(rows.to_vec())));
}

fn status(app: &Shared, message: String) {
    let serial = {
        let mut s = app.borrow_mut();
        s.status_serial += 1;
        s.status_serial
    };
    if let Some(ui) = app.borrow().ui.upgrade() {
        ui.set_status(message.into());
    }
    let a = app.clone();
    slint::Timer::single_shot(STATUS_TIME, move || {
        let s = a.borrow();
        if s.status_serial == serial
            && let Some(ui) = s.ui.upgrade()
        {
            ui.set_status(SharedString::new());
        }
    });
}

/// Events from the worker, on the UI thread.
fn handle(app: &Shared, event: Event) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    match event {
        Event::Opened {
            tracks,
            tools,
            pdf_name,
            folder,
        } => {
            {
                let mut s = app.borrow_mut();
                s.track_count = tracks.len();
                s.settings.tracks = vec![0];
                s.pdf_name = pdf_name;
                s.folder = folder;
            }
            let rows: Vec<TrackRow> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| TrackRow {
                    name: t.name.as_str().into(),
                    instrument: t.instrument.as_str().into(),
                    shown: i == 0,
                })
                .collect();
            ui.set_tracks(ModelRc::new(VecModel::from(rows)));
            ui.set_multitrack(false);
            // Room for the tracks, up to five rows; the splitter changes it.
            ui.set_tracks_height(28.0 * tracks.len().clamp(1, 5) as f32 + 12.0);
            set_tools(&ui, &tool_rows(&tools));
            ui.set_has_score(true);
            ui.invoke_scroll_to(0.0, 0.0);
            settings_changed(app);
        }
        Event::Engraved { engraving, sheets } => {
            {
                let mut s = app.borrow_mut();
                s.engraving = engraving;
                s.sheets = Sheets {
                    lists: Vec::new(),
                    rects: sheets,
                };
                s.tiles.clear();
                s.tile_model.clear();
            }
            set_content_size(app, &ui);
            canvas_changed(app);
        }
        Event::Tile {
            engraving,
            paint,
            col,
            row,
            rgba,
        } => {
            let mut s = app.borrow_mut();
            let stale = engraving != s.engraving || paint != s.paint;
            if stale
                || s.tiles.contains_key(&(col, row))
                || rgba.len() != (TILE * TILE * 4) as usize
            {
                return;
            }
            let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, TILE, TILE);
            s.tiles
                .insert((col, row), Image::from_rgba8_premultiplied(buf));
            let scale = ui.window().scale_factor();
            let size = TILE as f32 / scale;
            s.tile_model.push(TileItem {
                image: s.tiles[&(col, row)].clone(),
                x: col as f32 * size,
                y: row as f32 * size,
                size,
            });
        }
        Event::Tools(tools) => set_tools(&ui, &tool_rows(&tools)),
        Event::Status(message) => status(app, message),
    }
}

/// The settings from the window's View menu and track list.
fn settings_changed(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let mut s = app.borrow_mut();
    let before = (s.settings.layout, s.settings.dynamics, s.settings.paper);
    s.settings.layout = Layout::ALL
        .get(ui.get_layout() as usize)
        .copied()
        .unwrap_or_default();
    s.settings.paper = Paper::ALL
        .get(ui.get_paper() as usize)
        .copied()
        .unwrap_or_default();
    s.settings.dynamics = ui.get_dynamics();
    s.settings.screen_width = screen_width(&ui, s.zoom);
    if before.0 != s.settings.layout {
        ui.invoke_scroll_to(0.0, 0.0);
    }
    // Dark Page only needs new tiles; everything else a new engraving.
    s.paint += 1;
    s.tiles.clear();
    s.tile_model.clear();
    if s.track_count > 0 {
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
}

fn set_tracks(app: &Shared, tracks: Vec<usize>) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let count = app.borrow().track_count;
    let model = ui.get_tracks();
    for i in 0..model.row_count() {
        if let Some(mut row) = model.row_data(i) {
            row.shown = tracks.contains(&i);
            model.set_row_data(i, row);
        }
    }
    ui.set_multitrack(count > 1 && tracks.len() == count);
    app.borrow_mut().settings.tracks = tracks;
    settings_changed(app);
}

/// The window's score area in points, for the Vertical layout.
fn screen_width(ui: &AppWindow, zoom: f32) -> f32 {
    ((ui.get_view_width() - CANVAS_PAD) / (PX_PER_PT * zoom)).max(150.0)
}

fn set_content_size(app: &Shared, ui: &AppWindow) {
    let s = app.borrow();
    let (w, h) = s.sheets.size();
    let px = PX_PER_PT * s.zoom;
    ui.set_content_width(w * px);
    ui.set_content_height(h * px);
}

/// The view moved or changed size: ask for the tiles it lacks, drop the
/// ones far away.
fn canvas_changed(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let mut s = app.borrow_mut();
    if s.track_count == 0 {
        return;
    }
    // In the Vertical layout the lines follow the window's width.
    if s.settings.layout == Layout::Vertical {
        let w = screen_width(&ui, s.zoom);
        if (w - s.settings.screen_width).abs() > 4.0 {
            s.settings.screen_width = w;
            let settings = s.settings.clone();
            s.worker.send(Request::Engrave(settings));
        }
    }
    let scale = ui.window().scale_factor();
    let px = PX_PER_PT * s.zoom; // logical pixels per point
    let device = px * scale;
    let view = Rect::new(
        (ui.get_view_x() - ui.get_origin_x()) / px,
        (ui.get_view_y() - ui.get_origin_y()) / px,
        ui.get_view_width() / px,
        ui.get_view_height() / px,
    );
    // Draw a little beyond the view, keep more than that.
    let near = view.inflate(0.25 * view.width.max(view.height));
    let keep = view.inflate(1.0 * view.width.max(view.height));
    let wanted = s.sheets.tiles_for(near, device);
    let kept = s.sheets.tiles_for(keep, device);
    let before = s.tiles.len();
    s.tiles.retain(|k, _| kept.contains(k));
    if s.tiles.len() != before {
        let size = TILE as f32 / scale;
        let items: Vec<TileItem> = s
            .tiles
            .iter()
            .map(|(&(c, r), img)| TileItem {
                image: img.clone(),
                x: c as f32 * size,
                y: r as f32 * size,
                size,
            })
            .collect();
        s.tile_model.set_vec(items);
    }
    let missing: Vec<(u32, u32)> = wanted
        .into_iter()
        .filter(|k| !s.tiles.contains_key(k))
        .collect();
    if !missing.is_empty() {
        let request = Request::Tiles {
            engraving: s.engraving,
            paint: s.paint,
            scale: device,
            dark: ui.get_dark_page(),
            tiles: missing,
        };
        s.worker.send(request);
    }
}

/// The score-zoom slider moved: keep the middle of the view where it is.
fn zoom_moved(app: &Shared, v: f32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    // Snap to 100 % near the middle of the slider.
    let v = if v.abs() < 0.06 { 0.0 } else { v };
    ui.set_zoom_log(v);
    let zoom = 2f32.powf(v).clamp(0.25, 4.0);
    let (old, layout) = {
        let s = app.borrow();
        (s.zoom, s.settings.layout)
    };
    if (zoom - old).abs() < 1e-4 {
        return;
    }
    ui.set_zoom_text(format!("{:.0} %", zoom * 100.0).into());
    let (cx, cy) = (
        ui.get_view_x() + ui.get_view_width() / 2.0 - ui.get_origin_x(),
        ui.get_view_y() + ui.get_view_height() / 2.0 - ui.get_origin_y(),
    );
    {
        let mut s = app.borrow_mut();
        s.zoom = zoom;
        s.paint += 1;
        s.tiles.clear();
        s.tile_model.clear();
    }
    set_content_size(app, &ui);
    let k = zoom / old;
    ui.invoke_scroll_to(
        cx * k + ui.get_origin_x() - ui.get_view_width() / 2.0,
        cy * k + ui.get_origin_y() - ui.get_view_height() / 2.0,
    );
    if layout == Layout::Vertical {
        // Zooming a window-wide layout reflows it.
        settings_changed(app);
    } else {
        canvas_changed(app);
    }
}

/// File → Open: the system's dialog, off the UI thread where the system
/// allows it, so the window keeps drawing.
fn open_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        status(app, problem);
        return;
    }
    let folder = app.borrow().folder.clone();
    let sender = app.borrow().worker.sender();
    let pick = move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open")
            .add_filter("Guitar Pro", &["gp", "gpx", "gp5", "gp4", "gp3", "gtp"])
            .add_filter("All files", &["*"]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        if let Some(path) = dialog.pick_file() {
            let _ = sender.send(Request::Open(path));
        }
    };
    run_dialog(pick);
}

/// File → Export PDF…: the shown tracks, in pages, on the chosen paper.
fn export_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        status(app, problem);
        return;
    }
    let (folder, name, settings, sender) = {
        let s = app.borrow();
        (
            s.folder.clone(),
            s.pdf_name.clone(),
            s.settings.clone(),
            s.worker.sender(),
        )
    };
    let pick = move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export PDF")
            .set_file_name(&name)
            .add_filter("PDF", &["pdf"]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        if let Some(mut path) = dialog.save_file() {
            if path.extension().is_none() {
                path.set_extension("pdf");
            }
            let _ = sender.send(Request::ExportPdf(path, settings));
        }
    };
    run_dialog(pick);
}

/// On Linux the file dialogs come from the XDG desktop portal over the
/// session's D-Bus. Without a session bus there is no dialog to show, so say
/// so rather than doing nothing.
fn no_dialogs() -> Option<String> {
    if !cfg!(target_os = "linux") || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return None;
    }
    let bus = std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("bus"));
    if bus.is_some_and(|b| b.exists()) {
        return None;
    }
    Some(
        "No file dialog: this session has no D-Bus for the desktop portal. \
         Start OxiGuitar with a file instead: oxiguitar FILE"
            .to_owned(),
    )
}

/// macOS wants dialogs on the main thread; elsewhere they run on their own
/// thread so the window stays responsive.
fn run_dialog(pick: impl FnOnce() + Send + 'static) {
    if cfg!(target_os = "macos") {
        pick();
    } else {
        let _ = std::thread::Builder::new()
            .name("oxiguitar-dialog".to_owned())
            .spawn(pick);
    }
}

/// Ctrl + wheel and Ctrl + + / − / 0 scale the whole interface, wherever the
/// pointer or the keyboard focus is. The winit event filter sees them before
/// any widget, so the score never scrolls by accident.
fn install_interface_zoom(ui: &AppWindow, app: &Shared) {
    let a = app.clone();
    ui.window().on_winit_window_event(move |_, event| {
        let mut s = a.borrow_mut();
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
            // A score on one line that fits the window's height: the wheel
            // scrolls along the line.
            WinitEvent::MouseWheel { delta, .. }
                if !s.ctrl && s.settings.layout == Layout::Horizontal =>
            {
                let Some(ui) = s.ui.upgrade() else {
                    return EventResult::Propagate;
                };
                let fits = ui.get_content_height() + CANVAS_PAD <= ui.get_view_height();
                let dy = match delta {
                    MouseScrollDelta::LineDelta(0.0, y) => *y * 40.0,
                    MouseScrollDelta::PixelDelta(p) if p.x == 0.0 => p.y as f32,
                    _ => return EventResult::Propagate,
                };
                if !fits {
                    return EventResult::Propagate;
                }
                drop(s);
                ui.invoke_scroll_by(-dy, 0.0);
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
        let scale = s.system_scale * s.ui_zoom;
        drop(s);
        // Apply after winit's own handling of this event has finished.
        let a = a.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            let Some(ui) = a.borrow().ui.upgrade() else {
                return;
            };
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
            // Tiles are drawn for device pixels: draw them again.
            {
                let mut s = a.borrow_mut();
                s.paint += 1;
                s.tiles.clear();
                s.tile_model.clear();
            }
            canvas_changed(&a);
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
    UI_ZOOMS
        .get((i + steps).clamp(0, UI_ZOOMS.len() as i32 - 1) as usize)
        .copied()
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_follow_the_tool_name() {
        assert_eq!(
            reason_phrase("The score already has key signatures."),
            "the score already has key signatures"
        );
        let rows = tool_rows(&[ToolEntry {
            name: "Infer Key Signatures".to_owned(),
            unavailable: Some("The score has no bars.".to_owned()),
        }]);
        assert_eq!(
            rows[0].title,
            "Infer Key Signatures (the score has no bars)"
        );
        assert!(!rows[0].enabled);
    }

    #[test]
    fn interface_zoom_steps() {
        assert_eq!(step_ui_zoom(1.0, 1), 1.1);
        assert_eq!(step_ui_zoom(1.0, -2), 0.8);
        assert_eq!(step_ui_zoom(2.0, 5), 2.0);
        assert_eq!(step_ui_zoom(0.5, -1), 0.5);
    }
}
