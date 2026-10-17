// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar desktop window.
//!
//! The screen is declared in `ui/app.slint` (Slint, ADR 0004) and follows
//! `docs/design/ui.md`: the toolbar and the four tabbed areas come from
//! [`shell`], the footer's messages from [`footer`]. This file connects the
//! window to the worker thread ([`worker`]), which reads, engraves and draws
//! the score as tiles ([`canvas`]), and handles both zooms: the mouse wheel
//! over the page zooms the page, Ctrl + wheel the whole interface.
//!
//! At start, the plugin host (`oxiguitar-host`) makes sure the user folder,
//! settings.json and the plugin folder exist, and the footer says what is
//! missing (docs/design/plugins.md).
//!
//! Usage: `oxiguitar [FILE]`. Nothing here touches the network.

#![windows_subsystem = "windows"]

mod canvas;
mod document;
mod footer;
mod icons;
mod shell;
#[cfg(test)]
mod speed;
mod tracks;
mod worker;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use oxiguitar_display::Rect;
use oxiguitar_host::Level;
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
use crate::footer::{Footer, level_code};
use crate::shell::{Shell, Side, item};
use crate::tracks::Tracks;
use crate::worker::{Event, Request, Worker};

/// The code Slint generates from `ui/app.slint`.
mod ui {
    #![allow(missing_docs, unreachable_pub, clippy::all, clippy::todo)]
    slint::include_modules!();
}
use ui::{
    AppWindow, AreaState, FooterRow, TileItem, ToolRow, ToolbarItemRow, ToolbarSectionRow, TrackRow,
};

/// The app id: the desktop entry's name and the Wayland app id.
const APP_ID: &str = "io.github.mateusrdp.OxiGuitar";
/// Interface zoom steps, as in web browsers.
const UI_ZOOMS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
/// Size of the notation buttons' icons, in logical pixels.
const NOTATION_ICON: f32 = 24.0;
/// Pixels of smooth (touchpad) scrolling per wheel notch, for both zooms.
const PIXELS_PER_STEP: f64 = 60.0;
/// Score zoom per wheel notch, in the slider's log2 units: 2^(1/8), about 9 %.
const WHEEL_ZOOM_STEP: f32 = 0.125;
/// Logical pixels scrolled per wheel notch with Shift held.
const WHEEL_SCROLL: f32 = 40.0;
/// Logical pixels per point at 100 % score zoom (96 dpi).
const PX_PER_PT: f32 = 96.0 / 72.0;
/// Room around the sheets on the canvas, in logical pixels (both sides).
const CANVAS_PAD: f32 = 48.0;

struct App {
    ui: Weak<AppWindow>,
    worker: Worker,
    settings: Settings,
    track_count: usize,
    /// The track list: shown tracks and their notation.
    tracks: Tracks,
    /// The notation buttons' icons, and the scale they were drawn for.
    icons_scale: f32,
    /// The newest engraving: its id and where its sheets sit, in points.
    engraving: u64,
    sheets: Sheets,
    /// Bumped whenever tiles must be drawn again (zoom, scale, dark page).
    paint: u64,
    tiles: HashMap<(u32, u32), Image>,
    tile_model: Rc<VecModel<TileItem>>,
    /// Tiles from before the last zoom, engraving or repaint, scaled to the
    /// current zoom and drawn under the new ones until those cover the
    /// view, so the canvas never goes blank in between.
    old_tile_model: Rc<VecModel<TileItem>>,
    /// The tiles the view itself shows (without the margin drawn ahead).
    visible: Vec<(u32, u32)>,
    /// Score zoom, 0.25 to 4.
    zoom: f32,
    /// Scale factor the system gives the window, and our interface zoom.
    system_scale: f32,
    ui_zoom: f32,
    ctrl: bool,
    shift: bool,
    wheel_pixels: f64,
    /// Where the pointer is, in the window's physical pixels, for the wheel.
    cursor: Option<(f64, f64)>,
    pdf_name: String,
    folder: Option<PathBuf>,
    /// The toolbar and the four areas, and the toolbar's icons by name.
    shell: Shell,
    icons: HashMap<String, Image>,
    /// This session's messages.
    footer: Footer,
    /// A dropped file is being opened; the rest of the same drop is ignored.
    drop_pending: bool,
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
    // On Wayland the compositor shows the icon of the desktop entry with
    // this name (assets/linux/, installed by the packages or by
    // scripts/install-desktop-entry.sh); on X11 it is the window's class.
    slint::set_xdg_app_id(APP_ID)?;
    let ui = AppWindow::new()?;
    let tile_model = Rc::new(VecModel::<TileItem>::default());
    ui.set_tiles(ModelRc::from(tile_model.clone()));
    let old_tile_model = Rc::new(VecModel::<TileItem>::default());
    ui.set_old_tiles(ModelRc::from(old_tile_model.clone()));

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
        tracks: Tracks::default(),
        icons_scale: 0.0,
        engraving: 0,
        sheets: Sheets::default(),
        paint: 0,
        tiles: HashMap::new(),
        tile_model,
        old_tile_model,
        visible: Vec::new(),
        zoom: 1.0,
        system_scale: 1.0,
        ui_zoom: 1.0,
        ctrl: false,
        shift: false,
        wheel_pixels: 0.0,
        cursor: None,
        pdf_name: "score.pdf".to_owned(),
        folder: None,
        shell: Shell::default(),
        icons: HashMap::new(),
        footer: Footer::default(),
        drop_pending: false,
    }));
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));

    ui.set_shortcut_lines(lines(&[
        "Ctrl + O: open a file;  Ctrl + Q: quit;  F7 / F8: show or hide the left / bottom panel",
        "Escape or a click: close this note",
        "Mouse wheel over the page: zoom the page;  Shift + wheel: scroll it",
        "Arrow keys, Page Up / Page Down, Home / End, the scroll bars: scroll the page",
        "Score zoom also: the slider at the bottom right of the page; click its percentage for 100 %",
        "Ctrl + mouse wheel: zoom the interface",
        "Ctrl + +  /  Ctrl + −  /  Ctrl + 0: zoom the interface in, out, back to 100 %",
        "Panels: click a tab's name to open it, the open tab's name (or the chevron) to collapse its panel",
        "Footer: click it to see every message; the chevron folds it back",
        "Track list: the clef and TAB buttons show a track's notation, both off hides it; click a name to show that track alone",
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

    // The user folder, settings.json and the plugin folders (created when
    // missing), and what the footer should say about them. A few small
    // file operations, done before the window first shows.
    let startup = oxiguitar_host::start(
        oxiguitar_host::Folders::here(),
        document::BUILT_IN_READERS
            .iter()
            .map(|r| (*r).to_owned())
            .collect(),
        Vec::new(),
    );
    ui.set_player_available(!startup.inventory.instruments.is_empty());
    if !startup.inventory.instruments.is_empty() {
        ui.set_outputs(ModelRc::new(VecModel::from(
            startup
                .inventory
                .instruments
                .iter()
                .map(|n| SharedString::from(n.as_str()))
                .collect::<Vec<_>>(),
        )));
    }
    app.borrow_mut().footer.extend(startup.notices);
    sync_footer(&app);
    sync_shell(&app);

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
    ui.on_standard_clicked(move |i| {
        a.borrow_mut().tracks.toggle_standard(i as usize);
        sync_tracks(&a);
    });
    let a = app.clone();
    ui.on_tablature_clicked(move |i| {
        a.borrow_mut().tracks.toggle_tablature(i as usize);
        sync_tracks(&a);
    });
    let a = app.clone();
    ui.on_track_solo(move |i| {
        a.borrow_mut().tracks.solo(i as usize);
        sync_tracks(&a);
    });
    let a = app.clone();
    ui.on_tool_picked(move |i| {
        a.borrow().worker.send(Request::ApplyTool(i as usize));
    });
    let a = app.clone();
    ui.on_toolbar_activated(move |id| toolbar_activated(&a, id.as_str()));
    let a = app.clone();
    ui.on_tab_clicked(move |area, i| {
        if let (Some(side), Ok(i)) = (Side::from_index(area), usize::try_from(i)) {
            a.borrow_mut().shell.area_mut(side).click(i);
            sync_areas(&a);
        }
    });
    let a = app.clone();
    ui.on_area_toggled(move |area| {
        if let Some(side) = Side::from_index(area) {
            {
                let mut s = a.borrow_mut();
                let area = s.shell.area_mut(side);
                area.collapsed = !area.collapsed;
            }
            sync_areas(&a);
        }
    });
    let a = app.clone();
    ui.on_panels_changed(move || {
        let Some(ui) = a.borrow().ui.upgrade() else {
            return;
        };
        let shown = [
            ui.get_top_shown(),
            ui.get_bottom_shown(),
            ui.get_left_shown(),
            ui.get_right_shown(),
        ];
        let mut s = a.borrow_mut();
        for (side, shown) in Side::ALL.into_iter().zip(shown) {
            s.shell.area_mut(side).shown = shown;
        }
    });

    install_window_events(&ui, &app);

    // A file named on the command line (or by the system's file manager).
    if let Some(path) = std::env::args_os().nth(1) {
        app.borrow().worker.send(Request::Open(PathBuf::from(path)));
    }

    // For automated checks (scripts/no-network.sh): quit after this many
    // milliseconds.
    if let Some(ms) = std::env::var("OXIGUITAR_EXIT_AFTER_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        slint::Timer::single_shot(Duration::from_millis(ms), || {
            let _ = slint::quit_event_loop();
        });
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

/// A message for the footer.
fn message(app: &Shared, level: Level, text: impl Into<String>) {
    app.borrow_mut().footer.push(level, text);
    sync_footer(app);
}

/// Shows the footer's latest message and, for the expanded footer, all of
/// them, newest first.
fn sync_footer(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let (level, text) = s
        .footer
        .latest()
        .map_or((0, ""), |n| (level_code(n.level), n.text.as_str()));
    ui.set_footer_level(level);
    ui.set_footer_text(text.into());
    ui.set_footer_count(s.footer.len() as i32);
    let rows: Vec<FooterRow> = s
        .footer
        .newest_first()
        .map(|n| FooterRow {
            level: level_code(n.level),
            text: n.text.as_str().into(),
        })
        .collect();
    ui.set_footer_rows(ModelRc::new(VecModel::from(rows)));
}

/// The toolbar's icons, our own SVGs (assets/icons/ui/), tinted by the
/// window. Plugins' icons will come from their manifests.
fn ui_icon(name: &str) -> Image {
    let svg: &[u8] = match name {
        "open" => include_bytes!("../../../assets/icons/ui/open.svg"),
        "save" => include_bytes!("../../../assets/icons/ui/save.svg"),
        "export" => include_bytes!("../../../assets/icons/ui/export.svg"),
        "undo" => include_bytes!("../../../assets/icons/ui/undo.svg"),
        "redo" => include_bytes!("../../../assets/icons/ui/redo.svg"),
        "dark-page" => include_bytes!("../../../assets/icons/ui/dark-page.svg"),
        _ => return Image::default(),
    };
    Image::load_from_svg_data(svg).unwrap_or_default()
}

/// Shows the toolbar and the areas as the shell says.
fn sync_shell(app: &Shared) {
    sync_toolbar(app);
    sync_areas(app);
}

fn sync_toolbar(app: &Shared) {
    let mut s = app.borrow_mut();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let names: Vec<String> = s
        .shell
        .toolbar
        .iter()
        .flat_map(|sec| sec.items.iter().map(|i| i.icon.clone()))
        .collect();
    for name in names {
        s.icons
            .entry(name.clone())
            .or_insert_with(|| ui_icon(&name));
    }
    let sections: Vec<ToolbarSectionRow> = s
        .shell
        .toolbar
        .iter()
        .map(|sec| ToolbarSectionRow {
            title: sec.title.as_str().into(),
            items: ModelRc::new(VecModel::from(
                sec.items
                    .iter()
                    .map(|i| ToolbarItemRow {
                        id: i.id.as_str().into(),
                        icon: s.icons.get(&i.icon).cloned().unwrap_or_default(),
                        tip: i.tip.as_str().into(),
                        enabled: i.enabled,
                        checkable: i.checkable,
                        checked: i.checked,
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_toolbar(ModelRc::new(VecModel::from(sections)));
    ui.set_toolbar_shown(s.shell.toolbar_shown);
}

fn sync_areas(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let state = |side: Side| {
        let area = s.shell.area(side);
        AreaState {
            titles: ModelRc::new(VecModel::from(
                area.tabs
                    .iter()
                    .map(|t| SharedString::from(t.title.as_str()))
                    .collect::<Vec<_>>(),
            )),
            active: area.active as i32,
            active_id: area.active_id().into(),
            collapsed: area.collapsed,
        }
    };
    ui.set_top_area(state(Side::Top));
    ui.set_bottom_area(state(Side::Bottom));
    ui.set_left_area(state(Side::Left));
    ui.set_right_area(state(Side::Right));
    ui.set_top_shown(s.shell.area(Side::Top).shown);
    ui.set_bottom_shown(s.shell.area(Side::Bottom).shown);
    ui.set_left_shown(s.shell.area(Side::Left).shown);
    ui.set_right_shown(s.shell.area(Side::Right).shown);
}

/// A toolbar button was clicked.
fn toolbar_activated(app: &Shared, id: &str) {
    match id {
        item::OPEN => open_dialog(app),
        item::EXPORT_PDF => export_dialog(app),
        item::DARK_PAGE => {
            if let Some(ui) = app.borrow().ui.upgrade() {
                ui.set_dark_page(!ui.get_dark_page());
            }
            settings_changed(app);
        }
        // Save, Undo and Redo come with the editor; they are greyed out.
        _ => {}
    }
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
                s.tracks = Tracks::new(
                    tracks.iter().map(|t| t.default_notation).collect(),
                    tracks.iter().map(|t| t.can_tab).collect(),
                );
                s.settings.tracks = s.tracks.shown();
                s.settings.notation = s.tracks.notation();
                s.pdf_name = pdf_name;
                s.folder = folder;
                // Another song: nothing of the last one stays on screen.
                s.tiles.clear();
                s.tile_model.clear();
                s.old_tile_model.clear();
            }
            let rows: Vec<TrackRow> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let row = app.borrow().tracks.row(i);
                    TrackRow {
                        name: t.name.as_str().into(),
                        instrument: t.instrument.as_str().into(),
                        shown: row.shown,
                        standard: row.standard,
                        tablature: row.tablature,
                        can_tab: t.can_tab,
                    }
                })
                .collect();
            ui.set_tracks(ModelRc::new(VecModel::from(rows)));
            // Room for the tracks, up to five rows; the splitter changes it.
            ui.set_bottom_height(28.0 * tracks.len().clamp(1, 5) as f32 + 12.0);
            {
                let mut s = app.borrow_mut();
                let bottom = s.shell.area_mut(Side::Bottom);
                if bottom.active_id() != "tracks" {
                    bottom.open("tracks");
                }
            }
            sync_areas(app);
            set_tools(&ui, &tool_rows(&tools));
            ui.set_has_score(true);
            app.borrow_mut()
                .shell
                .set_item(item::EXPORT_PDF, Some(true), None);
            sync_toolbar(app);
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
                retire_tiles(&mut s, 1.0);
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
            drop_old_tiles_when_covered(&s);
        }
        Event::Tools(tools) => set_tools(&ui, &tool_rows(&tools)),
        Event::Message(level, text) => message(app, level, text),
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
    // The toolbar's Dark Page button follows the menu's tick, and the
    // other way round.
    let dark = ui.get_dark_page();
    let ticked = s
        .shell
        .toolbar
        .iter()
        .flat_map(|sec| &sec.items)
        .any(|i| i.id == item::DARK_PAGE && i.checked);
    if ticked != dark {
        s.shell.set_item(item::DARK_PAGE, None, Some(dark));
        drop(s);
        sync_toolbar(app);
        s = app.borrow_mut();
    }
    if before.0 != s.settings.layout {
        ui.invoke_scroll_to(0.0, 0.0);
    }
    // Dark Page only needs new tiles; everything else a new engraving.
    s.paint += 1;
    retire_tiles(&mut s, 1.0);
    if s.track_count > 0 {
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
}

/// The track list changed: show its state in every row, and engrave again.
fn sync_tracks(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let model = ui.get_tracks();
    {
        let s = app.borrow();
        for i in 0..model.row_count() {
            if let Some(mut row) = model.row_data(i) {
                let r = s.tracks.row(i);
                row.shown = r.shown;
                row.standard = r.standard;
                row.tablature = r.tablature;
                model.set_row_data(i, row);
            }
        }
    }
    {
        let mut s = app.borrow_mut();
        s.settings.tracks = s.tracks.shown();
        s.settings.notation = s.tracks.notation();
    }
    settings_changed(app);
}

/// Draws the notation buttons' icons for the window's current scale.
fn refresh_icons(app: &Shared, ui: &AppWindow) {
    let scale = ui.window().scale_factor();
    if (scale - app.borrow().icons_scale).abs() < 1e-3 {
        return;
    }
    let px = (NOTATION_ICON * scale).round().max(8.0) as u32;
    let images: Vec<Image> = icons::symbol_icons(&[icons::G_CLEF, icons::TAB_CLEF], px)
        .into_iter()
        .map(|icon| {
            let buf =
                SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&icon.rgba, icon.size, icon.size);
            Image::from_rgba8_premultiplied(buf)
        })
        .collect();
    if let [std, tab] = images.as_slice() {
        ui.set_standard_icon(std.clone());
        ui.set_tablature_icon(tab.clone());
    }
    app.borrow_mut().icons_scale = scale;
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
    // The interface zoom or the screen changed the scale: redraw the icons.
    refresh_icons(app, &ui);
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
    let (near, keep) = Sheets::around(view);
    s.visible = s.sheets.tiles_for(view, device);
    drop_old_tiles_when_covered(&s);
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

/// Most old tiles kept at once: enough for a few screens, so dragging the
/// zoom slider cannot pile them up.
const MAX_OLD_TILES: usize = 96;

/// The tiles on screen no longer match the score (zoom, engraving, page
/// colour, scale): they move to the layer underneath, scaled by `k` around
/// the canvas origin, and new tiles are drawn over them.
fn retire_tiles(s: &mut App, k: f32) {
    let scaled = |t: TileItem| TileItem {
        x: t.x * k,
        y: t.y * k,
        size: t.size * k,
        ..t
    };
    let mut items: Vec<TileItem> = s.old_tile_model.iter().map(scaled).collect();
    items.extend(s.tile_model.iter().map(scaled));
    let extra = items.len().saturating_sub(MAX_OLD_TILES);
    items.drain(..extra);
    s.old_tile_model.set_vec(items);
    s.tiles.clear();
    s.tile_model.clear();
}

/// Once every tile the view shows has arrived, the old layer goes.
fn drop_old_tiles_when_covered(s: &App) {
    if s.old_tile_model.row_count() > 0 && s.visible.iter().all(|k| s.tiles.contains_key(k)) {
        s.old_tile_model.clear();
    }
}

/// The score-zoom slider moved: keep the middle of the view where it is.
fn zoom_moved(app: &Shared, v: f32) {
    // Snap to 100 % near the middle of the slider.
    let v = if v.abs() < 0.06 { 0.0 } else { v };
    set_zoom(app, v, None);
}

/// The mouse wheel turned over the page by `notches` (fractions from a
/// touchpad): zoom, keeping the point under the pointer where it is.
/// `(x, y)` is the pointer in the page's viewport, in logical pixels.
fn wheel_zoom(app: &Shared, notches: f32, x: f32, y: f32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let v = (ui.get_zoom_log() + notches * WHEEL_ZOOM_STEP).clamp(-2.0, 2.0);
    set_zoom(app, v, Some((x, y)));
}

/// Sets the score zoom to 2^`v`, keeping the canvas point at `anchor` (in
/// the viewport, logical pixels; the middle of the view if `None`) where it
/// is on screen.
fn set_zoom(app: &Shared, v: f32, anchor: Option<(f32, f32)>) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
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
    let (ax, ay) = anchor.unwrap_or((ui.get_view_width() / 2.0, ui.get_view_height() / 2.0));
    let (cx, cy) = (
        ui.get_view_x() + ax - ui.get_origin_x(),
        ui.get_view_y() + ay - ui.get_origin_y(),
    );
    {
        let mut s = app.borrow_mut();
        s.zoom = zoom;
        s.paint += 1;
        retire_tiles(&mut s, zoom / old);
    }
    set_content_size(app, &ui);
    let k = zoom / old;
    ui.invoke_scroll_to(
        cx * k + ui.get_origin_x() - ax,
        cy * k + ui.get_origin_y() - ay,
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
        message(app, Level::Warning, problem);
        return;
    }
    let folder = app.borrow().folder.clone();
    let sender = app.borrow().worker.sender();
    let pick = move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open")
            .add_filter("Guitar Pro", document::OPEN_EXTENSIONS)
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
        message(app, Level::Warning, problem);
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

/// The window's raw events, before any widget sees them:
///
/// - Ctrl + wheel and Ctrl + + / − / 0 scale the whole interface, wherever
///   the pointer or the keyboard focus is, so the score never scrolls by
///   accident.
/// - The wheel over the page zooms the page, around the pointer; with Shift
///   it scrolls the page instead. Elsewhere (a list in a side area) the
///   wheel scrolls as usual.
/// - A file dropped on the window opens like File → Open.
///
/// **Drops don't work on Wayland** (ADR 0006; CONTRIBUTING.md, "Known
/// platform gaps"). winit 0.30 sends `DroppedFile` on Windows, macOS and
/// X11 only: its Wayland backend has no drag and drop, and Slint 1.18
/// passes no operating-system drops on. Listening ourselves would need our
/// own `wl_data_device` on winit's connection, through an `unsafe` raw
/// pointer and with no way to wake on its events but polling, which we
/// rejected. Drops will work on Wayland with no change here once the winit
/// in our Slint version emits `DroppedFile` on Wayland; or switch to Slint's
/// own event if Slint starts forwarding OS drops. Check either with
/// `scripts/wayland-dnd-test/`.
fn install_window_events(ui: &AppWindow, app: &Shared) {
    let a = app.clone();
    ui.window().on_winit_window_event(move |_, event| {
        let mut s = a.borrow_mut();
        let step = match event {
            WinitEvent::DroppedFile(path) => {
                // Several files dropped at once arrive one by one: open the
                // first, as File → Open opens one file.
                if !s.drop_pending {
                    s.drop_pending = true;
                    s.worker.send(Request::Open(path.clone()));
                    let a = a.clone();
                    slint::Timer::single_shot(Duration::from_millis(100), move || {
                        a.borrow_mut().drop_pending = false;
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
            // The wheel over the page: zoom it, or scroll it with Shift.
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
                } else if dy != 0.0 {
                    wheel_zoom(&a, dy, x, y);
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
                // Tiles sit in logical pixels, which still fit the score.
                retire_tiles(&mut s, 1.0);
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
