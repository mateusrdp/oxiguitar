// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The OxiGuitar desktop window.
//!
//! The screen is declared in `ui/app.slint` (Slint, ADR 0004) and follows
//! `docs/design/ui.md`: the toolbar and the four tabbed areas come from
//! [`shell`], the footer's messages from [`footer`]. This file connects the
//! window to the worker thread ([`worker`]), which reads, engraves and draws
//! the score as tiles ([`canvas`]), and handles the wheel: over the page it
//! scrolls the page and, with Shift, zooms it; Ctrl + wheel zooms the whole
//! interface.
//!
//! At start, the plugin host (`oxiguitar-host`) makes sure the user folder,
//! settings.json and the plugin folder exist, and the footer says what is
//! missing (docs/design/plugins.md). Plugins' tabs join the four areas
//! ([`plugin_tabs`]): a thread of their own asks the plugins for their
//! contents and draws their canvases, and the worker tells the plugins
//! that subscribe when a score is opened or changed.
//!
//! Tools → Plugins opens the plugin manager ([`manager`]), and
//! `--safe-mode` starts with no plugins at all; Help → About lists the
//! plugin folders.
//!
//! Usage: `oxiguitar [--safe-mode] [FILE]`. Nothing here touches the
//! network.

#![windows_subsystem = "windows"]

// The windowing backend comes from one of these features (Cargo.toml), and
// only one: each build knows its display system when it is built, and never
// looks at the session to choose (ADR 0011).
#[cfg(not(any(feature = "wayland", feature = "x11")))]
compile_error!("build oxiguitar-app with the `wayland` feature (the default) or `x11`");
#[cfg(all(feature = "wayland", feature = "x11"))]
compile_error!("build oxiguitar-app with one of the `wayland` and `x11` features, not both");

mod advice;
mod audio_export;
mod automation;
mod bar_form;
mod bend_form;
mod canvas;
mod chord_form;
mod chord_prefs;
mod cursor;
mod document;
mod drawings;
mod editing;
mod effect_form;
mod engraving;
mod footer;
mod icons;
mod inspector;
mod links;
mod manager;
mod palette;
mod player;
mod playing;
mod plugin_tabs;
mod plugin_ui;
mod prefs;
mod shell;
mod song;
mod sound;
#[cfg(test)]
mod speed;
#[cfg(test)]
mod test_support;
#[cfg(test)]
use test_support::test_plugins;
mod chrome;
mod dialogs;
mod files;
mod host_core;
mod inspector_view;
mod manager_view;
mod page;
mod palette_view;
mod plugin_view;
mod tool_dialogs;
mod track_form;
mod track_sections;
mod tracks;
mod window_events;
mod worker;
mod zoom;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use i_slint_backend_winit::winit::event::{
    ElementState, MouseScrollDelta, WindowEvent as WinitEvent,
};
use i_slint_backend_winit::winit::keyboard::{Key, KeyCode, PhysicalKey};
use i_slint_backend_winit::{EventResult, WinitWindowAccessor};
use oxiguitar_display::Rect;
use oxiguitar_host::api::display::Point as CanvasPoint;
use oxiguitar_host::api::tab::{
    Cursor, Pointer, PointerButton, PointerKind, TabContent, TabEvent, Widget,
};
use oxiguitar_host::{Level, TabId};
use oxiguitar_layout::{Layout, Paper};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
    Weak,
};

use crate::canvas::{Sheets, TILE};
use crate::document::{OpenScore, SaveFormat, Settings, ToolEntry, TrackInfo};
use crate::footer::{Footer, level_code};
use crate::plugin_tabs::TabThread;
use crate::prefs::ViewPrefs;
use crate::shell::{Shell, Side, item};
use crate::tracks::{Mixer, Tracks};
use crate::worker::{Event, Request, Worker};

/// The code Slint generates from `ui/app.slint`.
mod ui {
    #![allow(missing_docs, unreachable_pub, clippy::all, clippy::todo)]
    slint::include_modules!();
}
use ui::{
    AdviceLink, AppWindow, AreaState, BoxItem, FooterRow, GridPoint, InspectorField,
    InspectorVoice, ManagerRow, PaletteGroupRow, PaletteToolRow, PluginLinks, PluginManagerWindow,
    PluginMenu, PluginMenuItem, PluginRow, PluginSection, PluginSurfaces, PluginWidget,
    ShortcutLine, SongField, TileItem, ToolRow, ToolbarItemRow, ToolbarSectionRow, TrackRow,
};

// The window's modules split out of this file (23 Nov 2026), each with its
// own part of the window; they use this file's items through `super::*`.
use chrome::*;
use dialogs::*;
use files::*;
use host_core::*;
use inspector_view::*;
use manager_view::*;
use page::*;
use palette_view::*;
use plugin_view::*;
use tool_dialogs::*;
use window_events::*;
use zoom::*;

/// The app id: the desktop entry's name and the Wayland app id.
const APP_ID: &str = "io.github.mateusrdp.OxiGuitar";
/// Interface zoom steps, as in web browsers.
const UI_ZOOMS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
/// Size of the track rows' toggle icons (notation, mute, solo), in
/// logical pixels.
const TOGGLE_ICON: f32 = 24.0;
/// Size of a palette button and its icon, in logical pixels.
const PALETTE_BUTTON: f32 = 32.0;
/// Pixels of smooth (touchpad) scrolling per wheel notch, for both zooms.
const PIXELS_PER_STEP: f64 = 60.0;
/// Score zoom per wheel notch, in the slider's log2 units: 2^(1/8), about 9 %.
const WHEEL_ZOOM_STEP: f32 = 0.125;
/// Logical pixels the page scrolls per wheel notch.
const WHEEL_SCROLL: f32 = 40.0;
/// Logical pixels per point at 100 % score zoom (96 dpi).
const PX_PER_PT: f32 = 96.0 / 72.0;
/// Room around the sheets on the canvas, in logical pixels (both sides).
const CANVAS_PAD: f32 = 48.0;

struct App {
    /// The plugins' tools' keys (interface 1.8): keys, plugin and tool.
    plugin_keys: Vec<(oxiguitar_host::api::manifest::Shortcut, String, String)>,
    ui: Weak<AppWindow>,
    worker: Worker,
    settings: Settings,
    track_count: usize,
    /// The track list: shown tracks and their notation.
    tracks: Tracks,
    /// Each track's mixer.
    mixer: Mixer,
    /// The track rows' toggles (notation, mute, solo), measured at start.
    toggles: icons::IconSet,
    /// The palette's icons, measured on a thread of their own at start so
    /// the window never waits for them; `None` until they are.
    palette_icons: Option<Arc<icons::IconSet>>,
    /// The palette as shown: the core's groups with the plugins' tools
    /// (interface 1.8).
    palette_shown: Arc<Vec<palette::ShownGroup>>,
    /// Which palette groups are collapsed.
    palette: palette::Palette,
    /// The scale the icons were drawn for.
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
    /// The extensions of the formats the installed plugins read, for
    /// File → Open's filter.
    open_extensions: Vec<String>,
    /// The toolbar and the four areas, and the toolbar's icons by name.
    shell: Shell,
    icons: HashMap<String, Image>,
    /// This session's messages.
    footer: Footer,
    /// settings.json, shared with the plugins (saved only when it could be
    /// read at start), and the View settings last written to it.
    stored: oxiguitar_host::SharedSettings,
    prefs: ViewPrefs,
    /// Whether the interface is dark now: the user's choice, or the
    /// desktop's while following it. The other windows take this.
    dark_interface: bool,
    /// A dropped file is being opened; the rest of the same drop is ignored.
    drop_pending: bool,
    /// The thread that asks plugins for their tabs and draws their
    /// canvases; what it last answered for each tab; what each area shows.
    tab_thread: Option<TabThread>,
    plugin_tabs: HashMap<TabId, PluginTabState>,
    plugin_views: [PluginView; 6],
    /// The plugin dialog that is open (a tab whose area is `dialog`,
    /// interface 1.8).
    plugin_dialog: Option<TabId>,
    /// Every plugin file found, for the plugin manager, and its window
    /// once opened (hidden, not dropped, when closed).
    plugins: oxiguitar_host::Plugins,
    manager: Option<PluginManagerWindow>,
    /// The plugins' sections of the Track tab, and how many times
    /// the dialog has opened (so late answers for an earlier opening are
    /// dropped).
    sections: Option<track_sections::TrackSections>,
    dialogs_opened: u64,
    /// The editor: the caret and the selection over the page.
    editing: editing::Editing,
    /// The rows the Note and Bar tabs show, as last sent to the window.
    shown_rows: [Vec<effect_form::Field>; 3],
    /// Keep the notes' pitches (else their frets) when the Track tab
    /// changes the strings.
    track_keep_pitches: bool,
    /// Why the Track tab's last change couldn't be applied.
    track_error: Option<String>,
    /// The track's plugin settings and order of effects as the Track tab
    /// last saw them.
    sections_seen: (std::collections::BTreeMap<String, String>, Vec<String>),
    /// A widget of a plugin's section was used since the sections were
    /// asked: only then do their settings go into the track.
    sections_touched: bool,
    /// The bars that don't add up, every track's.
    bad: Vec<oxiguitar_edit::StaffBar>,
    /// The open file's name, and whether the score differs from it.
    doc_name: Option<String>,
    modified: bool,
    /// The score was made with File → New and has no file yet.
    untitled: bool,
    /// The formats the installed plugins write, for File → Save As.
    save_formats: Vec<SaveFormat>,
    /// The question over the window, if one shows.
    question: Option<Asked>,
    /// The advice dialog over the window, if one shows (interface 1.4).
    advice: Option<oxiguitar_host::advice::Asked>,
    /// The bend or whammy-bar editor, while it is open.
    bend_dialog: Option<bend_form::BendForm>,
    /// The chord dialog, while it is open.
    chord_dialog: Option<chord_form::ChordForm>,
    /// What the open Transpose dialog applies to.
    transpose_scope: Option<oxiguitar_edit::Scope>,
    /// Fit Bars to Time Signature… is open: for the whole song (Tools) or
    /// the selected bars (the page's menu).
    fit_whole_song: Option<bool>,
    /// Harmonize… is open.
    harmonize_shown: bool,
    /// Merge Into… is open (#312): the track merged, and the tracks its
    /// list offers, in its order.
    merge_dialog: Option<(usize, Vec<usize>)>,
    /// Copy, Cut or Paste Bars' dialog is open, for which.
    bars_dialog: Option<editing::ClipKey>,
    /// What to do once the score is saved (after "Save the changes…?").
    after_save: Option<Then>,
    /// The player (M4): its controls and the engine's window half, its
    /// thread, and the score's revisions as it was told them.
    player: player::Player,
    sound: sound::Sound,
    score_revision: u64,
    /// The score changed while the song played (#232): it is sent to the
    /// player's thread again once the song stops.
    changed_while_playing: bool,
    /// Where the caret was when playing began, for Stop.
    began: Option<oxiguitar_edit::Caret>,
    /// What the player has said in the footer this session.
    player_told: std::collections::HashSet<String>,
    /// The Output drop-down's entries (the first is the system's default),
    /// the one chosen (empty: the default), and whether any audio output
    /// works here.
    outputs: Vec<String>,
    output: String,
    outputs_available: bool,
    /// The score as plugins and the worker share it (whether it plays).
    open_score: OpenScore,
    /// The visual metronome is on; the cursor's line and system when the
    /// page was last checked for turning.
    visual: bool,
    followed: Option<(i32, i32)>,
    /// Where Pause put the caret: moved from there, the pause ends.
    paused_at: Option<oxiguitar_edit::Caret>,
    /// File → Export Audio…'s exporters; the one picked while its dialog
    /// is open, and its options shown there (interface 1.11); the export
    /// waiting for its options to be kept before its file is asked for;
    /// the stop switch of the export under way.
    exporters: Vec<audio_export::Target>,
    export_with: Option<usize>,
    export_options: Option<TabId>,
    export_waiting: Option<audio_export::Waiting>,
    export_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}

/// What the question over the window asks.
#[derive(Clone, Debug, PartialEq)]
enum Asked {
    /// Write the file despite what it loses? With `can_keep`, the buttons
    /// are Keep Them in the .oxg File, Save Without Them and Cancel.
    Save { can_keep: bool },
    /// Save the changes before going on to this?
    Unsaved(Then),
    /// Apply this edit, which leaves notes out?
    Apply(editing::Typed),
    /// Delete this track?
    DeleteTrack(editing::Typed),
    /// Nothing to answer: it only tells (its one button is OK).
    Told,
}

/// What waits on the answer to "Save the changes…?".
#[derive(Clone, Debug, PartialEq)]
enum Then {
    /// File → New.
    New,
    /// File → Open's dialog.
    OpenDialog,
    /// This file (dropped on the window).
    Open(PathBuf),
    /// Quit.
    Quit,
}

/// A plugin tab's last contents (or why it can't be shown) and its
/// canvases as drawn.
#[derive(Default)]
struct PluginTabState {
    content: Option<Result<TabContent, String>>,
    canvases: HashMap<String, Image>,
    /// The plugin asked for the tab to be redrawn while it was hidden.
    stale: bool,
    /// The last error put in the footer, so it is said once.
    told: Option<String>,
    /// Each canvas's room, in logical pixels, for when the scale changes.
    rooms: HashMap<String, (f32, f32)>,
}

/// What an area shows of a plugin tab: the tab, the contents its models
/// hold, and the models (kept, and changed in place where the rows keep
/// their shape, so a canvas being dragged on is never rebuilt).
#[derive(Default)]
struct PluginView {
    tab: Option<TabId>,
    content: Option<TabContent>,
    rows: Rc<VecModel<PluginRow>>,
    row_models: Vec<Rc<VecModel<PluginWidget>>>,
}

type Shared = Rc<RefCell<App>>;

thread_local! {
    /// The app, for events arriving from the worker thread.
    static APP: RefCell<Option<Shared>> = const { RefCell::new(None) };
    /// The plugins' tools for the Tools menu, after the core's: (plugin id,
    /// tool id, menu entry, whether it edits the score).
    static PLUGIN_TOOLS: RefCell<Vec<(String, String, String, bool)>> = const { RefCell::new(Vec::new()) };
}

/// The display system this build draws with, as plugins' conditions name
/// it: fixed when building (ADR 0011). Each Linux build has its own
/// (docs/design/plugins.md).
pub(crate) const DISPLAY_SYSTEM: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_vendor = "apple") {
    "macos"
} else if cfg!(feature = "wayland") {
    "wayland"
} else {
    "x11"
};

/// What to tell someone whose session this build can't draw in: the
/// Wayland build without a Wayland compositor, the X11 build without an X
/// server. `None` outside Linux.
fn wrong_build_hint(display: &str) -> Option<&'static str> {
    match display {
        "wayland" => Some(
            "This is OxiGuitar's Wayland build, and it found no Wayland session (is WAYLAND_DISPLAY set?). On an X11 desktop, install the X11 build instead (the package oxiguitar-x11), or use the AppImage, which holds both builds.",
        ),
        "x11" => Some(
            "This is OxiGuitar's X11 build, and it found no X server (is DISPLAY set?). On a Wayland desktop, install the Wayland build instead (the package oxiguitar), or use the AppImage, which holds both builds.",
        ),
        _ => None,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // The winit backend gives us the window's raw events for interface zoom.
    // It draws with this build's display system only.
    if let Err(e) = slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()
    {
        if let Some(hint) = wrong_build_hint(DISPLAY_SYSTEM) {
            eprintln!("{hint}");
        }
        return Err(e);
    }
    // On Wayland the compositor shows the icon of the desktop entry with
    // this name (assets/linux/, installed by the packages or by
    // scripts/install-desktop-entry.sh); on X11 it is the window's class.
    slint::set_xdg_app_id(APP_ID)?;
    let ui = AppWindow::new()?;
    let tile_model = Rc::new(VecModel::<TileItem>::default());
    ui.set_tiles(ModelRc::from(tile_model.clone()));
    let old_tile_model = Rc::new(VecModel::<TileItem>::default());
    ui.set_old_tiles(ModelRc::from(old_tile_model.clone()));

    // The user folder, settings.json and the plugin folders (created when
    // missing), every plugin's manifest (no plugin is loaded), and what the
    // footer should say about them. A few small file operations, done
    // before the window first shows. Files are opened only through the
    // plugins found here.
    let args = Args::parse(std::env::args_os().skip(1));
    let open_score = OpenScore::default();
    let _ = UI_THREAD.set(std::thread::current().id());
    let core = Arc::new(WindowCore {
        open: open_score.clone(),
        fonts: Arc::new(oxiguitar_display::FontSet::bundled()),
    });
    let startup = if args.safe_mode {
        oxiguitar_host::start_in_safe_mode(oxiguitar_host::Folders::here(), DISPLAY_SYSTEM, core)
    } else {
        oxiguitar_host::start(oxiguitar_host::Folders::here(), DISPLAY_SYSTEM, core)
    };
    let open_extensions = startup.plugins.read_extensions();
    let save_formats = SaveFormat::writable(&startup.plugins);
    let mut shell = Shell::default();
    plugin_tabs::add_to(&mut shell, &startup.plugins);
    plugin_tabs::add_views_to(&mut shell, &startup.plugins);
    let (toolbar_tools, toolbar_icons) = plugin_toolbar_tools(&startup.plugins);
    let (plugin_keys, key_lines, key_clashes) = {
        let (tools, clashes) = startup.plugins.shortcut_tools();
        let lines: Vec<(String, String)> = tools
            .iter()
            .map(|(k, t)| (k.label(), t.spec.name.clone()))
            .collect();
        let keys: Vec<(oxiguitar_host::api::manifest::Shortcut, String, String)> = tools
            .into_iter()
            .filter_map(|(k, t)| {
                Some((k, t.plugin.manifest.as_ref()?.id.clone(), t.spec.id.clone()))
            })
            .collect();
        (keys, lines, clashes)
    };
    shell.set_plugin_tools(&toolbar_tools);
    // Only when a plugin has tabs or track sections.
    let uses_tab_thread = !startup.plugins.tabs().is_empty()
        || oxiguitar_host::api::class::InstrumentClass::ALL
            .iter()
            .any(|&c| !startup.plugins.track_sections(c).is_empty());
    let tab_thread = uses_tab_thread.then(|| {
        TabThread::start(startup.plugins.clone(), true, |answer| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = APP.with(|a| a.borrow().clone()) {
                    plugin_answer(&app, answer);
                }
            });
        })
    });

    let (sound, control) = sound::Sound::start(startup.plugins.clone(), |m| {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                playing::from_sound(&app, m);
            }
        });
    });
    let worker = Worker::start(startup.plugins.clone(), open_score.clone(), |event| {
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
        mixer: Mixer::default(),
        toggles: icons::IconSet::new(&icons::toggles(), icons::Fill::TOGGLES),
        palette_icons: None,
        palette_shown: Arc::new(palette::shown(&plugin_palette_tools(&startup.plugins))),
        palette: palette::Palette::default(),
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
        open_extensions,
        shell,
        icons: toolbar_icons,
        plugin_keys: plugin_keys.clone(),
        footer: Footer::default(),
        stored: oxiguitar_host::SharedSettings::default(),
        prefs: ViewPrefs::default(),
        dark_interface: false,
        drop_pending: false,
        tab_thread,
        plugin_tabs: HashMap::new(),
        plugin_views: Default::default(),
        plugin_dialog: None,
        plugins: startup.plugins.clone(),
        manager: None,
        sections: None,
        dialogs_opened: 0,
        editing: editing::Editing::default(),
        shown_rows: Default::default(),
        track_keep_pitches: true,
        track_error: None,
        sections_seen: Default::default(),
        sections_touched: false,
        bad: Vec::new(),
        doc_name: None,
        modified: false,
        untitled: false,
        save_formats,
        question: None,
        advice: None,
        bend_dialog: None,
        chord_dialog: None,
        transpose_scope: None,
        fit_whole_song: None,
        harmonize_shown: false,
        merge_dialog: None,
        bars_dialog: None,
        after_save: None,
        player: player::Player::new(control),
        sound,
        score_revision: 0,
        changed_while_playing: false,
        began: None,
        player_told: std::collections::HashSet::new(),
        outputs: Vec::new(),
        output: String::new(),
        outputs_available: false,
        open_score,
        visual: false,
        followed: None,
        paused_at: None,
        exporters: audio_export::targets(&startup.plugins),
        export_with: None,
        export_options: None,
        export_waiting: None,
        export_cancel: None,
    }));
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));
    measure_palette(app.borrow().palette_shown.clone());

    let shortcuts = ShortcutData {
        navigation: vec![
            ("Ctrl + Alt + ←".into(), "rewind to start".into()),
            ("Ctrl + ← / →".into(), "bar back / on".into()),
            ("Ctrl + Home / End".into(), "first / last bar".into()),
            ("Tab / Shift + Tab".into(), "next / prev staff".into()),
            ("Ctrl + ↑ / ↓".into(), "prev / next track".into()),
        ],
        editing: vec![
            ("Ctrl + 1 to 4".into(), "voice 1 to 4".into()),
            ("F4".into(), "next out-of-tune bar".into()),
        ],
        note_entry: vec![
            ("R".into(), "rest".into()),
            ("− / +".into(), "longer / shorter".into()),
            ("Shift + . / Ctrl + .".into(), "dot / double dot".into()),
            ("L / Shift + L".into(), "tie note / beat".into()),
            ("Shift + Delete".into(), "delete beat".into()),
            ("Ctrl + Insert".into(), "insert beat".into()),
            ("Alt + Shift + ↑ / ↓".into(), "semitone up / down".into()),
            ("Alt + ↑ / ↓".into(), "same note, adjacent string".into()),
            ("Ctrl + Shift + Delete".into(), "clear beat".into()),
            ("/".into(), "triplet".into()),
            ("Insert".into(), "insert bar".into()),
            ("Ctrl + Delete".into(), "delete bar".into()),
            ("[ / ]".into(), "repeat open / close".into()),
            ("|".into(), "free time".into()),
            ("A".into(), "chord".into()),
            ("Shift + T".into(), "text above beat".into()),
            ("Ctrl + T".into(), "lyrics".into()),
        ],
        effects: vec![
            ("X".into(), "dead note".into()),
            ("O".into(), "ghost note".into()),
            ("; / :".into(), "accent / heavy accent".into()),
            ("!".into(), "staccato".into()),
            ("H".into(), "hammer-on".into()),
            ("S".into(), "legato".into()),
            ("Alt + S".into(), "shift slide".into()),
            ("Shift + S".into(), "slide out".into()),
            ("V".into(), "vibrato".into()),
            ("P / Shift + P".into(), "palm mute note / beat".into()),
            ("I".into(), "let ring".into()),
            ("Y".into(), "natural harmonic".into()),
            ("Ctrl + Alt + Y".into(), "artificial harmonic".into()),
            ("Alt + P".into(), "pinch harmonic".into()),
            ("Alt + T".into(), "tapped harmonic".into()),
            ("N".into(), "trill".into()),
            ("\"".into(), "tremolo picking".into()),
            ("T / )".into(), "tapping".into()),
            ("$".into(), "slap".into()),
            ("(".into(), "left-hand tap".into()),
            ("<".into(), "fade in".into()),
            ("G".into(), "normal grace note".into()),
            ("Ctrl + Alt + G".into(), "grace note".into()),
            ("B".into(), "bend".into()),
            ("Ctrl + Alt + V".into(), "whammy bar".into()),
            ("Ctrl + D / U".into(), "brush down / up".into()),
            ("Shift + D".into(), "pick stroke down".into()),
            ("Shift + U".into(), "pick stroke up".into()),
            ("Ctrl + Alt + W".into(), "wide vibrato".into()),
        ],
        plugins: key_lines,
    };
    let [left, center, right] = shortcuts_to_columns(&shortcuts);
    ui.set_shortcut_left_column(left);
    ui.set_shortcut_center_column(center);
    ui.set_shortcut_right_column(right);
    let mut about = vec![
        format!(
            "Guitar tablature and score viewer, editor and player. Version {}.",
            env!("CARGO_PKG_VERSION")
        ),
        "Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. Licensed under the GNU GPL, version 3 or later.".to_owned(),
        "OxiGuitar never connects to the internet on its own and sends nothing anywhere.".to_owned(),
    ];
    about.extend(about_folders(&startup.inventory.folders, args.safe_mode));
    ui.set_about_lines(lines(&about.iter().map(String::as_str).collect::<Vec<_>>()));
    // The plugins' menu entries (interface 1.8): the Tools menu's after
    // the core's tools, the others in the menus their manifests name.
    let mut menus = PluginMenus::default();
    let mut tools_menu = Vec::new();
    for (menu, t) in startup.plugins.menu_tools() {
        let Some(plugin) = t.plugin.manifest.as_ref().map(|m| m.id.clone()) else {
            continue;
        };
        if menu == "tools" {
            tools_menu.push((plugin, t.spec.id.clone(), t.spec.name.clone(), t.spec.edits));
            continue;
        }
        let item = PluginMenuItem {
            id: palette::plugin_tool_id(&plugin, &t.spec.id).into(),
            title: t.spec.name.as_str().into(),
            edits: t.spec.edits,
        };
        // Without a menu of its own choosing, the one the plugins share.
        let own = t
            .spec
            .menu
            .as_deref()
            .map_or(oxiguitar_host::api::manifest::PLUGINS_PLACE, str::trim);
        menus.add(&menu, own, item);
    }
    PLUGIN_TOOLS.with(|t| *t.borrow_mut() = tools_menu);
    menus.show(&ui);
    let a = app.clone();
    ui.on_plugin_tool_picked(move |id| {
        if let Some((plugin, tool)) = palette::plugin_tool_of(id.as_str()) {
            a.borrow()
                .worker
                .send(Request::PluginTool(plugin.to_owned(), tool.to_owned()));
        }
    });
    set_tools(&ui, &tool_rows(&default_tools()));

    // The outputs, asked of the audio-device plugin on the player's thread
    // (finding one loads it).
    app.borrow().sound.send(sound::ToSound::Devices);
    // The View menu as the user left it (the first time, the interface
    // follows the desktop, and the page is white).
    let prefs = ViewPrefs::from_value(startup.settings.get(prefs::KEY).as_ref());
    apply_prefs(&ui, prefs);
    {
        let mut s = app.borrow_mut();
        s.footer.extend(startup.notices);
        for clash in &key_clashes {
            s.footer.push(Level::Warning, clash.clone());
        }
        // What the core can't do, as advice for the window's first moments
        // (the host left out what the user silenced, and all of it in safe
        // mode).
        for asked in startup.advice {
            advice::ask(asked);
        }
        s.stored = startup.settings;
        s.prefs = prefs;
        // s.zoom and s.ui_zoom default to 1.0; restore the persisted
        // values so settings_changed() does not overwrite them with defaults.
        s.zoom = prefs.score_zoom;
        s.ui_zoom = prefs.ui_zoom;
        s.dark_interface = ui.get_dark_interface();
        if let Some(tabs) = &s.tab_thread {
            tabs.send(plugin_tabs::Request::Dark(s.dark_interface));
        }
        plugin_ui::init(s.plugins.clone(), s.dark_interface);
        // Settings: the count-in and metronome a session starts with.
        s.player.set_count_in(prefs.count_in);
        s.player.set_metronome(prefs.metronome);
    }
    // Establish the scrollable area dimensions for the restored zoom.
    set_content_size(&app, &ui);
    settings_changed(&app);
    sync_footer(&app);
    sync_shell(&app);

    let a = app.clone();
    ui.on_open_file(move || unless_unsaved(&a, Then::OpenDialog));
    let a = app.clone();
    ui.on_new_file(move || unless_unsaved(&a, Then::New));
    let a = app.clone();
    ui.on_edit_command(move |name| {
        match name.as_str() {
            "cut" => return clipboard(&a, editing::ClipKey::Cut),
            "copy" => return clipboard(&a, editing::ClipKey::Copy),
            "paste" => return clipboard(&a, editing::ClipKey::Paste),
            "copy-bars" => return clipboard(&a, editing::ClipKey::CopyBars),
            "cut-bars" => return clipboard(&a, editing::ClipKey::CutBars),
            "paste-bars" => return clipboard(&a, editing::ClipKey::PasteBars),
            "add-track" => return track_action(&a, "add"),
            "delete-track" => return track_action(&a, "delete"),
            "merge-track" => return track_action(&a, "merge"),
            "transpose-selection" => return open_transpose(&a, oxiguitar_edit::Scope::Selection),
            "transpose-all" => return open_transpose(&a, oxiguitar_edit::Scope::All),
            "fingering-selection" => return find_fingering(&a, oxiguitar_edit::Scope::Selection),
            "fingering-all" => return find_fingering(&a, oxiguitar_edit::Scope::All),
            "fit-all" => return open_fit(&a, true),
            "harmonize-all" => return open_harmonize(&a, true),
            _ => {}
        }
        let Some(command) = editing::menu_command(name.as_str()) else {
            return;
        };
        let typed = a.borrow_mut().editing.tool(command);
        if let Some(t) = typed {
            send_typed(&a, t);
        }
    });
    let a = app.clone();
    ui.on_save(move || save(&a));
    let a = app.clone();
    ui.on_save_as(move || save_as_dialog(&a));
    let a = app.clone();
    ui.on_question_answered(move |i| question_answered(&a, i));
    let a = app.clone();
    ui.on_advice_done(move || advice_done(&a));
    let a = app.clone();
    ui.on_tuplet_dialog_done(move |ok| tuplet_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_bars_dialog_done(move |ok| bars_dialog_done(&a, ok));
    ui.set_bend_amounts(strings(bend_form::amounts()));
    let a = app.clone();
    ui.on_bend_kind_picked(move |i| {
        bend_edit(&a, |f| {
            f.pick_kind(usize::try_from(i).unwrap_or(usize::MAX))
        })
    });
    let a = app.clone();
    ui.on_bend_amount_picked(move |i| bend_edit(&a, |f| f.pick_amount(i + 1)));
    let a = app.clone();
    ui.on_bend_pressed(move |c, r| bend_edit(&a, |f| f.press(c, r)));
    let a = app.clone();
    ui.on_bend_dragged(move |c, r| bend_edit(&a, |f| f.drag(c, r)));
    let a = app.clone();
    ui.on_bend_released(move || bend_edit(&a, bend_form::BendForm::release));
    let a = app.clone();
    ui.on_bend_dialog_done(move |ok| bend_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_chord_changed(move |kind, value, text| {
        chord_changed(&a, kind.as_str(), value, text.as_str())
    });
    let a = app.clone();
    ui.on_chord_dialog_done(move |ok| chord_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_text_dialog_done(move |ok| {
        let Some(ui) = a.borrow().ui.upgrade() else {
            return;
        };
        ui.set_text_dialog_shown(false);
        ui.invoke_focus_page();
        if ok {
            let text = ui.get_text_dialog_text().to_string();
            edit_at_caret(&a, oxiguitar_edit::Command::Text(text));
        }
    });
    let a = app.clone();
    ui.on_song_edited(move |id, text| {
        let info = a.borrow().editing.score().map(|s| s.info.clone());
        let Some(mut info) = info else {
            return;
        };
        if song::set(&mut info, id.as_str(), text.as_str()) {
            edit_at_caret(&a, oxiguitar_edit::Command::Info(Box::new(info)));
        }
    });
    let a = app.clone();
    ui.on_palette_searched(move |_| {
        if let Some(ui) = a.borrow().ui.upgrade() {
            sync_found(&ui);
        }
    });
    let a = app.clone();
    ui.on_lyric_key(move |key, text| lyric_key(&a, key.as_str(), text.as_str()));
    let a = app.clone();
    ui.on_note_changed(move |id, value| {
        let action = a.borrow().editing.field_changed(id.as_str(), value);
        match action {
            Some(effect_form::Action::Edit(command)) => {
                let typed = a.borrow_mut().editing.tool(command);
                if let Some(t) = typed {
                    send_typed(&a, t);
                }
            }
            Some(effect_form::Action::Bend) => open_edit_dialog(&a, editing::Dialog::Bend),
            Some(effect_form::Action::Whammy) => open_edit_dialog(&a, editing::Dialog::Whammy),
            Some(effect_form::Action::Chord) => open_edit_dialog(&a, editing::Dialog::Chord),
            Some(effect_form::Action::Text) => open_edit_dialog(&a, editing::Dialog::Text),
            None => {}
        }
    });
    let a = app.clone();
    ui.on_bar_changed(move |id, value| bar_changed(&a, id.as_str(), value, ""));
    let a = app.clone();
    ui.on_bar_edited(move |id, text| bar_changed(&a, id.as_str(), 0, text.as_str()));
    let a = app.clone();
    ui.on_bar_global_changed(move |id, global| bar_global_changed(&a, id.as_str(), global));
    let a = app.clone();
    ui.on_track_changed(move |id, value| track_changed(&a, id.as_str(), value, ""));
    let a = app.clone();
    ui.on_track_edited(move |id, text| track_changed(&a, id.as_str(), 0, text.as_str()));
    let a = app.clone();
    ui.on_track_section_event(move |i, widget, kind, on, value, text, chosen| {
        track_section_event(&a, i, &widget, widget_event(kind, on, value, &text, chosen));
    });
    let a = app.clone();
    ui.on_track_section_moved(move |i, up| track_section_moved(&a, i, up));
    let a = app.clone();
    ui.on_transpose_dialog_done(move |ok| transpose_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_fit_dialog_done(move |ok| fit_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_harmonize_dialog_done(move |ok| harmonize_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_merge_dialog_done(move |ok| merge_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_track_menu(move |i, action| {
        let i = usize::try_from(i).unwrap_or(0);
        let a = a.clone();
        // After the menu has closed: the command rebuilds the track rows,
        // which hold the menu, and on Windows that crashed the app while
        // the menu's own callback was still running (issue #102).
        slint::Timer::single_shot(Duration::ZERO, move || {
            let moved = a.borrow_mut().editing.select_track(i);
            if moved {
                caret_moved(&a);
            }
            track_action(&a, action.as_str());
        });
    });
    let a = app.clone();
    ui.on_page_menu(move |action| match action.as_str() {
        "transpose" => open_transpose(&a, oxiguitar_edit::Scope::Selection),
        "fingering" => find_fingering(&a, oxiguitar_edit::Scope::Selection),
        "fit" => open_fit(&a, false),
        "harmonize" => open_harmonize(&a, false),
        "design-more" => design_layout(&a, Some(true)),
        "design-less" => design_layout(&a, Some(false)),
        "design-auto" => design_layout(&a, None),
        "cut" => clipboard(&a, editing::ClipKey::Cut),
        "copy" => clipboard(&a, editing::ClipKey::Copy),
        "paste" => clipboard(&a, editing::ClipKey::Paste),
        _ => {}
    });
    let a = app.clone();
    ui.on_go_to_section(move |i| {
        let bar = {
            let s = a.borrow();
            s.editing
                .sections()
                .get(usize::try_from(i).unwrap_or(usize::MAX))
                .map(|(b, _)| *b)
        };
        let moved = bar.is_some_and(|b| a.borrow_mut().editing.go_to_bar(b));
        if moved {
            caret_moved(&a);
        }
    });
    ui.on_tuplet_usual(|n| i32::from(bar_form::usual_in_time_of(u8::try_from(n).unwrap_or(3))));
    let a = app.clone();
    ui.on_export_pdf(move || export_dialog(&a));
    ui.set_exporters(strings(
        app.borrow()
            .exporters
            .iter()
            .map(|t| t.name.clone())
            .collect(),
    ));
    let a = app.clone();
    ui.on_export_audio(move || export_audio_dialog(&a));
    let a = app.clone();
    ui.on_export_format_picked(move |i| export_format_picked(&a, i));
    let a = app.clone();
    ui.on_export_dialog_done(move |ok| export_audio_chosen(&a, ok));
    let a = app.clone();
    ui.on_export_cancel(move || {
        if let Some(c) = &a.borrow().export_cancel {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    let a = app.clone();
    ui.on_export_finished(move |level, text| {
        let ui = a.borrow().ui.upgrade();
        if let Some(ui) = ui {
            ui.set_export_running(false);
        }
        a.borrow_mut().export_cancel = None;
        let level = match level {
            0 => Level::Info,
            1 => Level::Warning,
            _ => Level::Error,
        };
        // An empty sentence: the file dialog was cancelled.
        if !text.is_empty() {
            message(&a, level, text.to_string());
        }
    });
    let a = app.clone();
    ui.on_quit(move || unless_unsaved(&a, Then::Quit));
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
    ui.on_track_selected(move |i, shift| {
        let track = match usize::try_from(i) {
            Ok(t) => t,
            Err(_) => return,
        };
        let handled = a.borrow_mut().tracks.select_track(track, shift);
        if handled {
            sync_tracks(&a);
        }
        let moved = a.borrow_mut().editing.select_track(track);
        if moved {
            caret_moved(&a);
        }
    });
    let a = app.clone();
    ui.on_track_double_clicked(move |i| {
        let Ok(track) = usize::try_from(i) else {
            return;
        };
        // Select the track and show its tab.
        let moved = a.borrow_mut().editing.select_track(track);
        if moved {
            caret_moved(&a);
        }
        show_tab(&a, "track");
    });
    let a = app.clone();
    ui.on_page_pressed(move |x, y, shift| page_pressed(&a, x, y, shift));
    let a = app.clone();
    ui.on_page_double_clicked(move |x, y| page_double_clicked(&a, x, y));
    let a = app.clone();
    ui.on_page_menu_pressed(move |x, y| page_menu_pressed(&a, x, y));
    let a = app.clone();
    ui.on_page_dragged(move |x, y| {
        let spot = on_sheet(&a, x, y).and_then(|(sheet, sx, sy)| {
            let spot = a.borrow().editing.hits().hit(sheet, sx, sy, 0)?;
            Some((sheet, spot))
        });
        let moved = spot.is_some_and(|(_sheet, spot)| a.borrow_mut().editing.drag_select(spot));
        if moved {
            caret_moved(&a);
        }
    });
    let a = app.clone();
    ui.on_page_drag_started(move |x, y| page_drag_started(&a, x, y));
    let a = app.clone();
    ui.on_page_drag_ended(move |x, y| page_drag_ended(&a, x, y));
    let a = app.clone();
    ui.on_caret_key(move |name, shift, ctrl| {
        let Some(key) = editing::Key::from_name(name.as_str()) else {
            return false;
        };
        // With one track on the page, Ctrl + Up and Down show the previous
        // or next track in its place, as Guitar Pro's single-track view.
        if ctrl && !shift && matches!(key, editing::Key::Up | editing::Key::Down) {
            let target = {
                let s = a.borrow();
                let shown = s.tracks.shown();
                match (shown.as_slice(), s.editing.track()) {
                    ([only], Some(t)) if *only == t => {
                        if key == editing::Key::Down {
                            (t + 1 < s.tracks.len()).then_some(t + 1)
                        } else {
                            t.checked_sub(1)
                        }
                    }
                    _ => None,
                }
            };
            if let Some(t) = target {
                {
                    let mut s = a.borrow_mut();
                    s.tracks.switch_to(t);
                    s.editing.select_track(t);
                }
                sync_tracks(&a);
                caret_moved(&a);
                return true;
            }
        }
        // Right at the very end of the score adds a bar, as Guitar Pro.
        if key == editing::Key::Right && !shift && !ctrl {
            let typed = a.borrow_mut().editing.at_the_end();
            if let Some(t) = typed {
                send_typed(&a, t);
                return true;
            }
        }
        let taken = a.borrow_mut().editing.key(key, shift, ctrl);
        if taken {
            caret_moved(&a);
        }
        taken
    });
    let a = app.clone();
    ui.on_edit_key(move |name, shift, ctrl, alt| {
        if ctrl && !alt && !shift && matches!(name.as_str(), "f" | "F") {
            search_palette(&a);
            return true;
        }
        if ctrl && !alt && !shift && matches!(name.as_str(), "m" | "M") {
            toggle_multivoice(&a);
            return true;
        }
        if let Some(k) = editing::clip_key(name.as_str(), shift, ctrl, alt) {
            clipboard(&a, k);
            return true;
        }
        if let Some(d) = editing::dialog_key(name.as_str(), shift, ctrl, alt) {
            open_edit_dialog(&a, d);
            return true;
        }
        let typed = a
            .borrow_mut()
            .editing
            .typed(name.as_str(), shift, ctrl, alt, Instant::now());
        match typed {
            Some(t) => {
                send_typed(&a, t);
                true
            }
            // Last, plugins' tools' keys (interface 1.8): OxiGuitar's own
            // keys come first.
            None => {
                let tool = a
                    .borrow()
                    .plugin_keys
                    .iter()
                    .find(|(k, _, _)| k.is(name.as_str(), shift, ctrl, alt))
                    .map(|(_, plugin, tool)| (plugin.clone(), tool.clone()));
                match tool {
                    Some((plugin, tool)) => {
                        a.borrow().worker.send(Request::PluginTool(plugin, tool));
                        true
                    }
                    None => false,
                }
            }
        }
    });
    let a = app.clone();
    ui.on_undo(move || a.borrow().worker.send(Request::Undo));
    let a = app.clone();
    ui.on_redo(move || a.borrow().worker.send(Request::Redo));
    let a = app.clone();
    ui.on_next_bad_bar(move || {
        let moved = {
            let mut s = a.borrow_mut();
            let next = s.editing.next_bad(&s.bad);
            next.is_some_and(|b| s.editing.go_to(b))
        };
        if moved {
            caret_moved(&a);
        }
    });
    let a = app.clone();
    ui.on_mixer_volume(move |i, v| {
        let m = a.borrow_mut().mixer.set_volume(i as usize, v);
        mixer_moved(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_pan(move |i, v| {
        let m = a.borrow_mut().mixer.set_pan(i as usize, v);
        mixer_moved(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_mute(move |i| {
        let m = a.borrow_mut().mixer.toggle_mute(i as usize);
        mixer_moved(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_solo(move |i| {
        let m = a.borrow_mut().mixer.toggle_solo(i as usize);
        mixer_moved(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_player_action(move |name| playing::action(&a, name.as_str()));
    let a = app.clone();
    ui.on_player_key(move |name, shift, ctrl, alt| {
        playing::key(&a, name.as_str(), shift, ctrl, alt)
    });
    let a = app.clone();
    ui.on_player_frame(move |_| playing::frame(&a));
    let a = app.clone();
    ui.on_overview_clicked(move |f| playing::overview_clicked(&a, f));
    let a = app.clone();
    ui.on_speed_slider_moved(move |v| {
        let speed = (v * 175.0 + 25.0).round() as u32;
        a.borrow_mut().player.set_speed(speed.clamp(25, 200));
        playing::sync(&a);
    });
    let a = app.clone();
    ui.on_speed_text_edited(move |v| {
        if let Ok(n) = v.parse::<u32>() {
            let speed = n.clamp(25, 200);
            a.borrow_mut().player.set_speed(speed);
            playing::sync(&a);
        }
    });
    let a = app.clone();
    ui.on_master_moved(move |v| {
        a.borrow_mut().player.set_master(v);
    });
    let a = app.clone();
    ui.on_output_chosen(move |i| playing::output_chosen(&a, usize::try_from(i).unwrap_or(0)));
    let a = app.clone();
    ui.on_palette_group_clicked(move |g| {
        a.borrow_mut().palette.toggle(g as usize);
        if let Some(ui) = a.borrow().ui.upgrade() {
            set_palette(&a, &ui, false);
        }
    });
    let a = app.clone();
    ui.on_palette_tool_clicked(move |id| {
        match id.as_str() {
            "multivoice" => {
                toggle_multivoice(&a);
                return;
            }
            "design-mode" => {
                if let Some(ui) = a.borrow().ui.upgrade() {
                    ui.set_design_mode(!ui.get_design_mode());
                }
                settings_changed(&a);
                return;
            }
            _ => {}
        }
        // A plugin's tool (interface 1.8) runs as from the Tools menu.
        if let Some((plugin, tool)) = palette::plugin_tool_of(id.as_str()) {
            a.borrow()
                .worker
                .send(Request::PluginTool(plugin.to_owned(), tool.to_owned()));
            return;
        }
        if let Some(d) = palette::dialog_of(id.as_str()) {
            open_edit_dialog(&a, d);
            return;
        }
        if let Some(command) = palette::command_of(id.as_str()) {
            let typed = a.borrow_mut().editing.tool(command);
            if let Some(t) = typed {
                send_typed(&a, t);
            }
            return;
        }
        let Some(v) = palette::voice_of(id.as_str()) else {
            return;
        };
        let moved = a
            .borrow_mut()
            .editing
            .key(editing::Key::Voice(v), false, true);
        if moved {
            caret_moved(&a);
        }
    });
    let a = app.clone();
    ui.on_tool_picked(move |i| {
        let i = usize::try_from(i).unwrap_or(usize::MAX);
        let ours = oxiguitar_tools::all().len();
        let request = match i.checked_sub(ours) {
            None => Request::ApplyTool(i),
            Some(j) => match PLUGIN_TOOLS.with(|t| t.borrow().get(j).cloned()) {
                Some((plugin, tool, _, _)) => Request::PluginTool(plugin, tool),
                None => return,
            },
        };
        a.borrow().worker.send(request);
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

    install_plugin_tab_callbacks(&ui, &app);
    install_plugin_links(&ui.global::<PluginLinks<'_>>(), &app);
    let a = app.clone();
    ui.on_open_plugin_manager(move || open_plugin_manager(&a));
    let a = app.clone();
    ui.on_open_link(move |url| {
        if let Err(e) = links::open(&url) {
            message(&a, Level::Error, e);
        }
    });
    let a = app.clone();
    ui.on_open_credits(move || {
        if let Err(e) = links::open(links::CREDITS) {
            message(&a, Level::Error, e);
        }
    });
    let a = app.clone();
    ui.on_open_manual(move || {
        if let Err(e) = links::open(links::MANUAL) {
            message(&a, Level::Error, e);
        }
    });
    let a = app.clone();
    ui.on_open_donation(move || {
        if let Err(e) = links::open(links::DONATE) {
            message(&a, Level::Error, e);
        }
    });
    // Closing the main window ends the program, even with the plugin
    // manager's window open (the event loop would otherwise wait for the
    // last window).
    // With unsaved changes, the window asks first and stays.
    let a = app.clone();
    ui.window().on_close_requested(move || {
        if a.borrow().modified {
            unless_unsaved(&a, Then::Quit);
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    install_window_events(&ui, &app);

    // A file named on the command line (or by the system's file manager).
    if let Some(path) = args.file {
        app.borrow().worker.send(Request::Open(path));
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

    // Once the system's window exists, Slint knows its size and scale: keep
    // the system's scale and put the saved interface zoom on top of it
    // (ADR 0010). Upstream (i-slint-backend-winit 1.19): `winit_window()`
    // is a future that wakes when the window has been made, after Slint has
    // set its scale; before 23 Nov 2026 this waited 50 ms and hoped.
    let (a, weak) = (app.clone(), ui.as_weak());
    let made = slint::spawn_local(async move {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.window().winit_window().await.is_err() {
            return;
        }
        let zoomed = {
            let mut s = a.borrow_mut();
            s.system_scale = ui.window().scale_factor();
            s.ui_zoom != 1.0
        };
        if zoomed {
            apply_interface_scale(&a);
        } else {
            canvas_changed(&a);
        }
    });
    if let Err(e) = made {
        app.borrow_mut().footer.push(Level::Error, e.to_string());
    }

    // Advice plugins asked for while the window was still being made.
    let a = app.clone();
    slint::Timer::single_shot(Duration::ZERO, move || show_advice(&a));

    ui.run()?;
    APP.with(|a| a.borrow_mut().take());
    Ok(())
}

/// The command line: `oxiguitar [--safe-mode] [FILE]`.
///
/// The only way a file manager hands us a file on macOS today.
/// Upstream (winit 0.30): Finder's double-click, Open With and drops on the
/// Dock icon arrive as Apple Events to the application delegate, which
/// winit (and Slint 1.19 through it) doesn't report; a delegate of our own
/// would need `unsafe` Objective-C glue, which we rejected (ADR 0007). On
/// update: if winit or Slint reports opened files, open them like `FILE`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    /// Start with no plugins at all.
    safe_mode: bool,
    /// The file to open.
    file: Option<PathBuf>,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = std::ffi::OsString>) -> Args {
        let mut out = Args::default();
        for arg in args {
            if arg == "--safe-mode" {
                out.safe_mode = true;
            } else if out.file.is_none() {
                out.file = Some(PathBuf::from(arg));
            }
        }
        out
    }
}

/// Help → About's lines about the plugin folders, so a user can find a
/// plugin file even when no plugin manager can be opened.
fn about_folders(folders: &[PathBuf], safe_mode: bool) -> Vec<String> {
    let mut out = vec![if folders.is_empty() {
        "Plugin folders: none.".to_owned()
    } else {
        "Plugin folders, searched in this order:".to_owned()
    }];
    out.extend(folders.iter().map(|f| format!("    {}", f.display())));
    if safe_mode {
        out.push("Started in safe mode: no plugin is used.".to_owned());
    }
    out
}

/// Puts the View menu's settings on the window.
fn apply_prefs(ui: &AppWindow, p: ViewPrefs) {
    // The choice first: leaving follow-desktop sets the window's colours
    // from it.
    if let Some(dark) = p.dark_interface {
        ui.set_chosen_dark(dark);
    }
    ui.set_follow_desktop(p.dark_interface.is_none());
    ui.set_dark_page(p.dark_page);
    ui.set_dynamics(p.dynamics);
    ui.set_play_engraved_beat(p.play_engraved_beat);
    ui.set_count_in_default(p.count_in);
    ui.set_metronome_default(p.metronome);
    let index = |found: Option<usize>| found.unwrap_or(0) as i32;
    ui.invoke_pick_layout(index(Layout::ALL.iter().position(|l| *l == p.layout)));
    ui.invoke_pick_paper(index(Paper::ALL.iter().position(|q| *q == p.paper)));
    // Restore the score zoom the user left it at.
    let zoom = p.score_zoom.clamp(0.25, 4.0);
    ui.set_zoom_log(zoom.log2());
    ui.set_zoom_text(format!("{:.0} %", zoom * 100.0).into());
    // Match the canvas rendering scale to the zoom level.
    ui.set_pt_scale(PX_PER_PT * zoom);
}

/// A toolbar button was clicked.
fn toolbar_activated(app: &Shared, id: &str) {
    match id {
        item::OPEN => unless_unsaved(app, Then::OpenDialog),
        item::SAVE => save(app),
        item::EXPORT_PDF => export_dialog(app),
        item::UNDO => app.borrow().worker.send(Request::Undo),
        item::REDO => app.borrow().worker.send(Request::Redo),
        // A plugin's button in the Plugins section runs its tool.
        _ => {
            if let Some((plugin, tool)) = palette::plugin_tool_of(id) {
                app.borrow()
                    .worker
                    .send(Request::PluginTool(plugin.to_owned(), tool.to_owned()));
            }
        }
    }
}

/// Events from the worker, on the UI thread.
fn handle(app: &Shared, event: Event) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    match event {
        Event::Opened {
            score,
            edit,
            bad,
            tracks,
            tools,
            pdf_name,
            folder,
            name,
            untitled,
        } => {
            {
                let mut s = app.borrow_mut();
                s.doc_name = Some(name);
                s.untitled = untitled;
                s.after_save = None;
                s.track_count = tracks.len();
                s.tracks = Tracks::new(
                    tracks.iter().map(|t| t.default_notation).collect(),
                    tracks.iter().map(|t| t.can_tab).collect(),
                );
                s.mixer = Mixer::new(tracks.iter().map(|t| t.mix).collect());
                let first = s.tracks.shown().first().copied().unwrap_or(0);
                s.editing.opened(score, first);
                // Plugins hear where the caret starts, as they do when it
                // moves.
                s.worker.send(Request::Selection(
                    s.editing.for_plugins(),
                    s.editing.for_plugins2(),
                ));
                ui.set_lyric_shown(false);
                s.bad = bad;
                s.settings.tracks = s.tracks.shown();
                s.settings.notation = s.tracks.notation();
                s.pdf_name = pdf_name;
                s.folder = folder;
                // Another song: nothing of the last one stays on screen.
                s.tiles.clear();
                s.tile_model.clear();
                s.old_tile_model.clear();
            }
            ui.set_tracks(ModelRc::new(VecModel::from(track_rows(app, &tracks))));
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
            sync_sections(app);
            {
                let mut s = app.borrow_mut();
                s.shell.set_item(item::EXPORT_PDF, Some(true), None);
                s.shell.set_item(item::SAVE, Some(true), None);
            }
            sync_toolbar(app);
            sync_edit_state(app, &edit);
            ui.invoke_scroll_to(0.0, 0.0);
            // Set the scrollable area to match the score dimensions at the
            // restored zoom level.
            set_content_size(app, &ui);
            settings_changed(app);
            playing::score_changed(app, true);
        }
        Event::AskSave {
            heading,
            lines,
            can_keep,
        } => {
            let buttons: &[&str] = if can_keep {
                &["Keep Them in the .oxg File", "Save Without Them", "Cancel"]
            } else {
                &["Save", "Cancel"]
            };
            ask(app, Asked::Save { can_keep }, &heading, &lines, buttons);
        }
        Event::Saved {
            name,
            edit,
            pdf_name,
            folder,
        } => {
            let then = {
                let mut s = app.borrow_mut();
                s.doc_name = Some(name);
                s.untitled = false;
                s.pdf_name = pdf_name;
                s.folder = folder;
                s.after_save.take()
            };
            sync_edit_state(app, &edit);
            if let Some(then) = then {
                go_on(app, then);
            }
        }
        Event::SaveFailed => app.borrow_mut().after_save = None,
        Event::Changed {
            score,
            edit,
            bad,
            caret,
            tracks,
            note_edit,
            mixer,
        } => {
            if mixer {
                // The window's own mixer moves: the rows and the sound
                // have them already, and nothing else changed.
                app.borrow_mut().editing.changed(score);
                sync_edit_state(app, &edit);
                return;
            }
            let play = {
                let mut s = app.borrow_mut();
                s.editing.changed(score.clone());
                if let Some(c) = caret {
                    s.editing.edited(c);
                }
                s.bad = bad;
                s.prefs.play_engraved_beat
            };
            // A preview still sounding plays the score as it was: it ends
            // before anything follows the new one (#314).
            playing::end_preview(app);
            if let Some((tracks, was)) = tracks {
                track_list_changed(app, &tracks, &was);
            } else {
                mixer_from_score(app, &score);
            }
            sync_sections(app);
            sync_edit_state(app, &edit);
            if caret.is_some() {
                caret_moved(app);
            } else {
                sync_caret(app, false);
            }
            playing::score_changed(app, false);
            // Play a 1-beat preview after a note edit when the toggle is on
            // and the change was a note insertion or deletion (#246).
            if play && note_edit {
                let dos = playing::play_beats(app, 1);
                playing::act(app, dos);
                playing::sync(app);
            }
        }
        Event::Refused => {
            let text = "Editing waits while the score plays: pause it (Space) to edit.".to_owned();
            let new = app.borrow_mut().player_told.insert(text.clone());
            if new {
                message(app, Level::Warning, text);
            }
        }
        Event::Engraved {
            engraving,
            sheets,
            hits,
        } => {
            {
                let mut s = app.borrow_mut();
                s.engraving = engraving;
                s.sheets = Sheets {
                    lists: Arc::default(),
                    rects: sheets,
                };
                s.editing.engraved(hits);
                retire_tiles(&mut s, 1.0);
            }
            set_content_size(app, &ui);
            sync_caret(app, false);
            canvas_changed(app);
            // The field for lyrics follows its beat to where it is now.
            if ui.get_lyric_shown() {
                show_lyric(app, &ui, ui.get_lyric_text().to_string());
            }
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
        Event::Report(text) => {
            app.borrow_mut().footer.push_report(text);
            sync_footer(app);
        }
    }
}

/// What the Edit menu and the toolbar say about undo and redo.
fn sync_edit_state(app: &Shared, edit: &document::EditState) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let title = |verb: &str, name: &Option<String>| match name {
        Some(n) => format!("{verb} {n}"),
        None => verb.to_owned(),
    };
    ui.set_can_undo(edit.undo.is_some());
    ui.set_can_redo(edit.redo.is_some());
    ui.set_undo_title(title("Undo", &edit.undo).into());
    ui.set_redo_title(title("Redo", &edit.redo).into());
    let window_title = {
        let mut s = app.borrow_mut();
        s.modified = edit.modified;
        match &s.doc_name {
            Some(name) if edit.modified => format!("• {name} — OxiGuitar"),
            Some(name) => format!("{name} — OxiGuitar"),
            None => "OxiGuitar".to_owned(),
        }
    };
    ui.set_window_title(window_title.into());
    {
        let mut s = app.borrow_mut();
        s.shell
            .set_item(item::UNDO, Some(edit.undo.is_some()), None);
        s.shell
            .set_item(item::REDO, Some(edit.redo.is_some()), None);
    }
    sync_toolbar(app);
}

/// The settings from the window's View menu, Settings dialog and track list.
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
    s.settings.design = ui.get_design_mode();
    s.settings.screen_width = screen_width(&ui, s.zoom);
    // Remember the View menu for next time, when it changed.
    let dark = ui.get_dark_interface();
    s.dark_interface = dark;
    let prefs = ViewPrefs {
        dark_interface: (!ui.get_follow_desktop()).then_some(dark),
        dark_page: ui.get_dark_page(),
        dynamics: s.settings.dynamics,
        layout: s.settings.layout,
        paper: s.settings.paper,
        score_zoom: s.zoom,
        ui_zoom: s.ui_zoom,
        play_engraved_beat: ui.get_play_engraved_beat(),
        count_in: ui.get_count_in_default(),
        metronome: ui.get_metronome_default(),
    };
    if prefs != s.prefs {
        s.prefs = prefs;
        let value = prefs.to_value(s.stored.get(prefs::KEY).as_ref());
        if let Err(e) = s.stored.set(prefs::KEY, value) {
            s.footer.push(Level::Error, e);
            drop(s);
            sync_footer(app);
            s = app.borrow_mut();
        }
    }
    if before.0 != s.settings.layout {
        ui.invoke_scroll_to(0.0, 0.0);
    }
    if let Some(tabs) = &s.tab_thread {
        tabs.send(plugin_tabs::Request::Dark(dark));
    }
    plugin_ui::dark(dark);
    if let Some(m) = &s.manager {
        m.set_dark_interface(dark);
    }
    // Dark Page only needs new tiles; everything else a new engraving.
    s.paint += 1;
    retire_tiles(&mut s, 1.0);
    if s.track_count > 0 {
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
}

/// A track's mixer changed: show it in its row, and let the sound follow.
/// A track's mixer control moved: heard at once, and kept in the score as
/// one more change, so saving writes it (each format as far as it holds
/// volume, pan, mute and solo) and Undo takes it back.
fn mixer_moved(app: &Shared, i: usize, mix: oxiguitar_model::Mix) {
    sync_mix(app, i, mix);
    let s = app.borrow();
    if let Some(caret) = s.editing.caret() {
        s.worker.send(Request::Edit {
            command: oxiguitar_edit::Command::Mix { track: i, mix },
            caret,
            selection: None,
        });
    }
}

/// The score's mixer, after a change that wasn't the window's own moves
/// (Undo and Redo, a plugin's change): the rows and the sound follow it.
fn mixer_from_score(app: &Shared, score: &oxiguitar_model::Score) {
    let changed: Vec<_> = {
        let s = app.borrow();
        score
            .tracks
            .iter()
            .enumerate()
            .filter(|(i, t)| s.mixer.get(*i) != t.mix)
            .map(|(i, t)| (i, t.mix))
            .collect()
    };
    if changed.is_empty() {
        return;
    }
    app.borrow_mut().mixer = Mixer::new(score.tracks.iter().map(|t| t.mix).collect());
    for (i, mix) in changed {
        sync_mix(app, i, mix);
    }
}

fn sync_mix(app: &Shared, i: usize, mix: oxiguitar_model::Mix) {
    app.borrow_mut().player.set_mix(i, mix);
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let model = ui.get_tracks();
    if let Some(mut row) = model.row_data(i) {
        row.volume = mix.volume;
        row.pan = mix.pan;
        row.mute = mix.mute;
        row.solo = mix.solo;
        model.set_row_data(i, row);
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
    fn the_command_line_takes_safe_mode_and_a_file_in_any_order() {
        let parse = |a: &[&str]| Args::parse(a.iter().map(std::ffi::OsString::from));
        assert_eq!(parse(&[]), Args::default());
        assert_eq!(
            parse(&["--safe-mode", "a.gp"]),
            Args {
                safe_mode: true,
                file: Some("a.gp".into())
            }
        );
        assert_eq!(
            parse(&["a.gp", "--safe-mode", "b.gp"]).file,
            Some("a.gp".into())
        );
        assert!(parse(&["a.gp", "--safe-mode"]).safe_mode);
    }

    #[test]
    fn about_lists_the_plugin_folders() {
        let folders = [
            PathBuf::from("/home/me/.local/share/oxiguitar/plugins"),
            PathBuf::from("/usr/lib/oxiguitar/plugins"),
        ];
        let lines = about_folders(&folders, false);
        assert_eq!(lines[0], "Plugin folders, searched in this order:");
        assert!(lines[2].ends_with("/usr/lib/oxiguitar/plugins"));
        assert_eq!(lines.len(), 3);
        assert_eq!(
            about_folders(&[], true),
            [
                "Plugin folders: none.",
                "Started in safe mode: no plugin is used."
            ]
        );
    }

    #[test]
    fn each_build_names_its_own_display_system() {
        let own = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_vendor = "apple") {
            "macos"
        } else if cfg!(feature = "wayland") {
            "wayland"
        } else {
            "x11"
        };
        assert_eq!(DISPLAY_SYSTEM, own);
        match own {
            "wayland" | "x11" => assert!(wrong_build_hint(own).unwrap().contains(&format!(
                "OxiGuitar's {} build",
                if own == "x11" { "X11" } else { "Wayland" }
            ))),
            _ => assert!(wrong_build_hint(own).is_none()),
        }
    }

    #[test]
    fn interface_zoom_steps() {
        assert_eq!(step_ui_zoom(1.0, 1), 1.1);
        assert_eq!(step_ui_zoom(1.0, -2), 0.8);
        assert_eq!(step_ui_zoom(2.0, 5), 2.0);
        assert_eq!(step_ui_zoom(0.5, -1), 0.5);
    }

    #[test]
    fn shortcuts_fill_three_columns_in_turn() {
        let pair = |k: &str| (k.to_owned(), "x".to_owned());
        let data = ShortcutData {
            navigation: vec![pair("A"), pair("B")],
            editing: vec![pair("C"), pair("D")],
            plugins: vec![pair("E")],
            ..ShortcutData::default()
        };
        let texts = |m: &ModelRc<ShortcutLine>| -> Vec<String> {
            m.iter().map(|l| l.text.to_string()).collect()
        };
        let [left, center, right] = shortcuts_to_columns(&data);
        assert_eq!(texts(&left), ["A   →   x", "D   →   x"]);
        assert_eq!(texts(&center), ["B   →   x", "E   →   x"]);
        assert_eq!(texts(&right), ["C   →   x"]);
    }
}
