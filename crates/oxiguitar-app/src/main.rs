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

// The windowing backend comes from one of these features (Cargo.toml).
#[cfg(not(any(feature = "wayland", feature = "x11")))]
compile_error!("build oxiguitar-app with the `wayland` feature (the default) or `x11`");

mod bar_form;
mod bend_form;
mod canvas;
mod chord_form;
mod cursor;
mod document;
mod editing;
mod effect_form;
mod footer;
mod icons;
mod inspector;
mod links;
mod manager;
mod palette;
mod player;
mod playing;
mod plugin_tabs;
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
mod track_form;
mod track_sections;
mod tracks;
mod worker;

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
use oxiguitar_host::api::tab::{Pointer, PointerButton, PointerKind, TabContent, TabEvent, Widget};
use oxiguitar_host::{Level, TabId};
use oxiguitar_layout::{Layout, Paper};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
    Weak,
};

use crate::canvas::{Sheets, TILE};
use crate::document::{FULLEST, OpenScore, SaveFormat, Settings, ToolEntry, TrackInfo};
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
    AltBox, AppWindow, AreaState, BarFields, BoxItem, FooterRow, GridPoint, InspectorField,
    InspectorVoice, ManagerRow, PaletteGroupRow, PaletteToolRow, PluginManagerWindow, PluginRow,
    PluginSection, PluginSettingsWindow, PluginWidget, SongField, TileItem, ToolRow,
    ToolbarItemRow, ToolbarSectionRow, TrackFields, TrackRow,
};

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
    /// A dropped file is being opened; the rest of the same drop is ignored.
    drop_pending: bool,
    /// The thread that asks plugins for their tabs and draws their
    /// canvases; what it last answered for each tab; what each area shows.
    tab_thread: Option<TabThread>,
    plugin_tabs: HashMap<TabId, PluginTabState>,
    plugin_views: [PluginView; 4],
    /// Every plugin file found, for the plugin manager, and its window
    /// once opened (hidden, not dropped, when closed).
    plugins: oxiguitar_host::Plugins,
    manager: Option<PluginManagerWindow>,
    /// A plugin's settings window (interface 1.3): the window, once made,
    /// and the plugin it shows.
    settings_window: Option<(PluginSettingsWindow, String)>,
    /// The plugins' sections of the open track dialog, and how many times
    /// the dialog has opened (so late answers for an earlier opening are
    /// dropped).
    sections: Option<track_sections::TrackSections>,
    dialogs_opened: u64,
    /// The editor: the caret and the selection over the page.
    editing: editing::Editing,
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
    /// The bar the open Bar dialog was opened on, as it was.
    bar_dialog: Option<oxiguitar_edit::BarState>,
    /// The bend or whammy-bar editor, while it is open.
    bend_dialog: Option<bend_form::BendForm>,
    /// The chord dialog, while it is open.
    chord_dialog: Option<chord_form::ChordForm>,
    /// What the open Transpose dialog applies to.
    transpose_scope: Option<oxiguitar_edit::Scope>,
    /// What to do once the score is saved (after "Save the changes…?").
    after_save: Option<Then>,
    /// The player (M4): its controls and the engine's window half, its
    /// thread, and the score's revisions as it was told them.
    player: player::Player,
    sound: sound::Sound,
    score_revision: u64,
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
}

/// What the question over the window asks.
#[derive(Clone, Debug, PartialEq)]
enum Asked {
    /// Write the file despite what it loses, or under another name? With
    /// `elsewhere`, the buttons are Save, Save As… and Cancel.
    Save { elsewhere: bool },
    /// Save the changes before going on to this?
    Unsaved(Then),
    /// Apply this edit, which leaves notes out?
    Apply(editing::Typed),
    /// Delete this track?
    DeleteTrack(editing::Typed),
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
    /// tool id, menu entry).
    static PLUGIN_TOOLS: RefCell<Vec<(String, String, String)>> = const { RefCell::new(Vec::new()) };
}

/// What the window does for plugins: shows their footer messages, shows
/// their tabs again when they ask, and hands them the open score. Requests
/// may come from any thread (a plugin runs on the thread that called it),
/// so the first two are handed to the event loop.
struct WindowCore {
    open: OpenScore,
}

impl oxiguitar_host::Core for WindowCore {
    fn notify(&self, level: Level, text: String) {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                app.borrow_mut().footer.push(level, text);
                sync_footer(&app);
            }
        });
    }

    fn notify_with_link(
        &self,
        level: Level,
        text: String,
        link: oxiguitar_host::api::message::Link,
    ) {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                app.borrow_mut()
                    .footer
                    .push_with_link(level, text, link.text, link.url);
                sync_footer(&app);
            }
        });
    }

    fn redraw(&self, tab: &TabId) {
        let tab = tab.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                plugin_tab_changed(&app, tab);
            }
        });
    }

    fn score(&self) -> Option<oxiguitar_host::api::score::Score> {
        self.open.wire()
    }

    fn score2(&self) -> Option<(u64, oxiguitar_host::api::score2::Score2)> {
        self.open.wire2()
    }

    fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: oxiguitar_host::api::score2::Score2,
    ) -> Result<(), String> {
        self.open.apply(plugin, revision, name, score)
    }

    fn withdrawn(&self, id: &str) {
        let id = id.to_owned();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                plugin_withdrawn(&app, &id);
            }
        });
    }
}

/// The plugin `id` failed and was withdrawn: its tabs go, as if it had
/// never been installed, and the plugin manager shows why.
fn plugin_withdrawn(app: &Shared, id: &str) {
    let gone = {
        let mut s = app.borrow_mut();
        s.plugin_tabs.retain(|tab, _| tab.plugin != id);
        plugin_tabs::remove_from(&mut s.shell, id)
    };
    if gone {
        sync_shell(app);
        for side in Side::ALL {
            show_plugin_tab(app, side);
        }
    }
    sync_manager(app);
}

/// The display system this build draws with, as plugins' conditions name
/// it. Each Linux build has its own (docs/design/plugins.md); a build with
/// both features, for development, follows the host's rule.
fn display_system() -> &'static str {
    let linux = cfg!(not(any(target_os = "windows", target_vendor = "apple")));
    match (linux, cfg!(feature = "wayland"), cfg!(feature = "x11")) {
        (true, true, false) => "wayland",
        (true, false, true) => "x11",
        _ => oxiguitar_host::display_system(),
    }
}

/// What to tell someone whose session this build can't draw in: the
/// Wayland build without a Wayland compositor, the X11 build without an X
/// server. `None` for builds that try both, and outside Linux.
fn wrong_build_hint(display: &str) -> Option<&'static str> {
    if !cfg!(not(any(target_os = "windows", target_vendor = "apple")))
        || (cfg!(feature = "wayland") && cfg!(feature = "x11"))
    {
        return None;
    }
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
        if let Some(hint) = wrong_build_hint(display_system()) {
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
    let core = Arc::new(WindowCore {
        open: open_score.clone(),
    });
    let startup = if args.safe_mode {
        oxiguitar_host::start_in_safe_mode(oxiguitar_host::Folders::here(), display_system(), core)
    } else {
        oxiguitar_host::start(oxiguitar_host::Folders::here(), display_system(), core)
    };
    let open_extensions = startup.plugins.read_extensions();
    let save_formats = SaveFormat::writable(&startup.plugins);
    let mut shell = Shell::default();
    plugin_tabs::add_to(&mut shell, &startup.plugins);
    // Only when a plugin has tabs, a settings window or track sections.
    let uses_tab_thread = !startup.plugins.tabs().is_empty()
        || !startup.plugins.with_settings_window().is_empty()
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
        icons: HashMap::new(),
        footer: Footer::default(),
        stored: oxiguitar_host::SharedSettings::default(),
        prefs: ViewPrefs::default(),
        drop_pending: false,
        tab_thread,
        plugin_tabs: HashMap::new(),
        plugin_views: Default::default(),
        plugins: startup.plugins.clone(),
        manager: None,
        settings_window: None,
        sections: None,
        dialogs_opened: 0,
        editing: editing::Editing::default(),
        bad: Vec::new(),
        doc_name: None,
        modified: false,
        untitled: false,
        save_formats,
        question: None,
        bar_dialog: None,
        bend_dialog: None,
        chord_dialog: None,
        transpose_scope: None,
        after_save: None,
        player: player::Player::new(control),
        sound,
        score_revision: 0,
        began: None,
        player_told: std::collections::HashSet::new(),
        outputs: Vec::new(),
        output: String::new(),
        outputs_available: false,
        open_score,
        visual: false,
        followed: None,
        paused_at: None,
    }));
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));
    measure_palette();

    ui.set_shortcut_lines(lines(&[
        "Ctrl + N: a new score;  Ctrl + O: open a file;  Ctrl + S: save it;  Ctrl + Shift + S: save it as another file;  Ctrl + Q: quit;  F7 / F8: show or hide the left / bottom panel",
        "Escape or a click: close this note",
        "Space: play from the caret, or pause;  Ctrl + Left / Ctrl + Right: a bar back / on (the sound while it plays, else the caret)",
        "Mouse wheel over the page: scroll it;  Shift + wheel: zoom the page around the pointer",
        "Page Up / Page Down, the scroll bars: scroll the page;  with no score open, the arrow keys too",
        "A click on the page puts the caret there; Shift + click selects up to it",
        "Left / Right: the previous / next beat;  Up / Down: the string above / below;  Home / End: the bar's first / last beat",
        "Ctrl + Home / Ctrl + End: the first / last bar;  Tab / Shift + Tab: the next / previous staff;  Ctrl + Up / Ctrl + Down: the previous / next track",
        "Shift + the arrows, Home, End: select;  Ctrl + Shift + Left / Right: select by bar;  Ctrl + A: select all;  Escape: drop the selection",
        "Ctrl + 1 to Ctrl + 4: voice 1 to 4;  Ctrl + Z / Ctrl + Y: undo / redo;  F4: the next bar that doesn't add up (shown in red)",
        "0 to 9: a fret on the caret's string (two digits in a row: 10 to 24);  R: rest;  − / +: longer / shorter;  Shift + . / Ctrl + .: dot / double dot",
        "L / Shift + L: tie the note / the beat;  Backspace: delete the note;  Shift + Delete: delete the beat;  Ctrl + Insert: insert a beat",
        "Alt + Shift + Up / Down: a semitone up / down;  Alt + Up / Down: the same note on the string above / below",
        "On the standard staff: click a line or space, Up / Down to move it, Enter to put a note there;  Alt + 1 to 4: move the beat to that voice;  Ctrl + Shift + Delete: clear the beat;  Right at the end: add a bar",
        "/: triplet;  Insert: insert a bar;  Ctrl + Delete: delete the bar;  [ and ]: repeat open and close;  |: free time;  Ctrl + K, Ctrl + /, D: the Bar dialog;  A: a chord;  Shift + T: text above the beat",
        "Ctrl + T: type lyrics under the beat (Space or − the next note, Backspace back, Up / Down another verse, Enter done)",
        "Ctrl + F: search the palette (Escape empties it, Enter goes back to the score)",
        "Effects: X dead, O ghost, ; and : accent and heavy accent, ! staccato, H hammer-on, S / Alt + S / Shift + S legato / shift slide / slide out down, V vibrato, P / Shift + P palm mute (note / beat), I let ring",
        "Y natural harmonic, Ctrl + Alt + Y artificial, Alt + P pinch, Alt + T tapped;  N trill;  \": tremolo picking;  T or ) tapping;  $ slap;  ( left-hand tapping;  < fade in;  G / Ctrl + Alt + G grace note",
        "Ctrl + D / Ctrl + U: brush down / up;  Shift + D / Shift + U: pick stroke down / up;  Ctrl + Alt + W: wide vibrato;  B: bend;  Ctrl + Alt + V: whammy bar",
        "Ctrl + C, Ctrl + X, Ctrl + V: copy, cut, paste;  Ctrl + Shift + V: Special Paste (several times, or replacing)",
        "Score zoom also: the slider at the bottom right of the page; click its percentage for 100 %",
        "Ctrl + mouse wheel: zoom the interface",
        "Ctrl + +  /  Ctrl + −  /  Ctrl + 0: zoom the interface in, out, back to 100 %",
        "Panels: click a tab's name to open it, the open tab's name (or the chevron) to collapse its panel",
        "Footer: click it to see every message; the chevron folds it back",
        "Track list: the clef and TAB buttons show a track's notation, both off hides it; click a name to select the track",
        "Tools → Plugins: every plugin file, and a tickbox to enable or disable each from the next start;  oxiguitar --safe-mode starts without any plugin",
    ]));
    let mut about = vec![
        format!(
            "Guitar tablature and score viewer, editor and player. Version {}.",
            env!("CARGO_PKG_VERSION")
        ),
        "Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors. Licensed under the GNU GPL, version 3 or later.".to_owned(),
        "Music font: Bravura (Steinberg, SIL OFL 1.1). Text font: Noto Sans (SIL OFL 1.1). GUI: Slint (GPL-3.0).".to_owned(),
        "OxiGuitar never connects to the internet on its own and sends nothing anywhere.".to_owned(),
    ];
    about.extend(about_folders(&startup.inventory.folders, args.safe_mode));
    ui.set_about_lines(lines(&about.iter().map(String::as_str).collect::<Vec<_>>()));
    PLUGIN_TOOLS.with(|t| {
        *t.borrow_mut() = startup
            .plugins
            .tools()
            .iter()
            .filter_map(|t| {
                let plugin = t.plugin.manifest.as_ref()?.id.clone();
                Some((plugin, t.spec.id.clone(), t.spec.name.clone()))
            })
            .collect();
    });
    set_tools(&ui, &tool_rows(&default_tools()));

    // The outputs, asked of the audio-device plugin on the player's thread
    // (finding one loads it).
    app.borrow().sound.send(sound::ToSound::Devices);
    // The View menu as the user left it (a dark interface and a white page
    // the first time).
    let prefs = ViewPrefs::from_value(startup.settings.get(prefs::KEY).as_ref());
    apply_prefs(&ui, prefs);
    {
        let mut s = app.borrow_mut();
        s.footer.extend(startup.notices);
        s.stored = startup.settings;
        s.prefs = prefs;
    }
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
            "bar-dialog" => return open_edit_dialog(&a, editing::Dialog::Bar),
            "tuplet-dialog" => return open_edit_dialog(&a, editing::Dialog::Tuplet),
            "chord-dialog" => return open_edit_dialog(&a, editing::Dialog::Chord),
            "text-dialog" => return open_edit_dialog(&a, editing::Dialog::Text),
            "lyrics" => return open_edit_dialog(&a, editing::Dialog::Lyrics),
            "cut" => return clipboard(&a, editing::ClipKey::Cut),
            "copy" => return clipboard(&a, editing::ClipKey::Copy),
            "paste" => return clipboard(&a, editing::ClipKey::Paste),
            "special-paste" => return clipboard(&a, editing::ClipKey::Special),
            "track-properties" => return open_track_dialog(&a),
            "add-track" => return track_action(&a, "add"),
            "delete-track" => return track_action(&a, "delete"),
            "transpose-selection" => return open_transpose(&a, oxiguitar_edit::Scope::Selection),
            "transpose-all" => return open_transpose(&a, oxiguitar_edit::Scope::All),
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
    ui.on_bar_dialog_done(move |ok| bar_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_tuplet_dialog_done(move |ok| tuplet_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_paste_dialog_done(move |ok| paste_dialog_done(&a, ok));
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
    ui.set_chord_roots(strings(chord_form::note_names()));
    ui.set_chord_triads(strings(chord_form::triad_names()));
    ui.set_chord_extensions(strings(chord_form::extension_names()));
    ui.set_chord_basses(strings(chord_form::bass_names()));
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
    ui.on_inspector_changed(move |id, value| {
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
    ui.set_track_instrument_names(strings(
        track_form::INSTRUMENTS
            .iter()
            .map(|n| (*n).to_owned())
            .collect(),
    ));
    ui.set_track_string_counts(strings(
        track_form::STRING_COUNTS
            .iter()
            .map(|n| {
                if *n == 0 {
                    "None".to_owned()
                } else {
                    n.to_string()
                }
            })
            .collect(),
    ));
    ui.set_track_key_names(strings(
        (0..12u8)
            .map(|k| oxiguitar_model::tuning::key_name(k).to_owned())
            .collect(),
    ));
    ui.set_track_pattern_names(strings(
        oxiguitar_model::tuning::Pattern::ALL
            .iter()
            .map(|p| p.label().to_owned())
            .collect(),
    ));
    let a = app.clone();
    ui.on_track_form_changed(move || track_form_changed(&a));
    let a = app.clone();
    ui.on_track_dialog_done(move |ok| track_dialog_done(&a, ok));
    let a = app.clone();
    ui.on_track_section_event(move |i, widget, kind, on, value, text, chosen| {
        track_section_event(&a, i, &widget, widget_event(kind, on, value, &text, chosen));
    });
    let a = app.clone();
    ui.on_transpose_dialog_done(move |ok| transpose_dialog_done(&a, ok));
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
        "bar" => open_edit_dialog(&a, editing::Dialog::Bar),
        "track" => open_track_dialog(&a),
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
    ui.set_bar_key_names(strings(bar_form::key_names()));
    ui.set_bar_clef_names(strings(
        bar_form::CLEFS
            .iter()
            .map(|(n, _)| (*n).to_owned())
            .collect(),
    ));
    ui.set_bar_feel_names(strings(
        bar_form::FEELS
            .iter()
            .map(|(n, _)| (*n).to_owned())
            .collect(),
    ));
    ui.set_bar_direction_names(strings(
        bar_form::DIRECTIONS
            .iter()
            .map(|(n, _)| (*n).to_owned())
            .collect(),
    ));
    let a = app.clone();
    ui.on_export_pdf(move || export_dialog(&a));
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
    ui.on_track_selected(move |i| {
        let moved = usize::try_from(i).is_ok_and(|i| a.borrow_mut().editing.select_track(i));
        if moved {
            caret_moved(&a);
        }
    });
    let a = app.clone();
    ui.on_page_pressed(move |x, y, shift| page_pressed(&a, x, y, shift));
    let a = app.clone();
    ui.on_page_double_clicked(move |x, y| page_double_clicked(&a, x, y));
    let a = app.clone();
    ui.on_page_menu_pressed(move |x, y| page_menu_pressed(&a, x, y));
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
            None => false,
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
        sync_mix(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_pan(move |i, v| {
        let m = a.borrow_mut().mixer.set_pan(i as usize, v);
        sync_mix(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_mute(move |i| {
        let m = a.borrow_mut().mixer.toggle_mute(i as usize);
        sync_mix(&a, i as usize, m);
    });
    let a = app.clone();
    ui.on_mixer_solo(move |i| {
        let m = a.borrow_mut().mixer.toggle_solo(i as usize);
        sync_mix(&a, i as usize, m);
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
    ui.on_speed_edited(move |v| {
        a.borrow_mut()
            .player
            .set_speed(u32::try_from(v).unwrap_or(100));
        playing::sync(&a);
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
                Some((plugin, tool, _)) => Request::PluginTool(plugin, tool),
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
    let a = app.clone();
    ui.on_open_plugin_manager(move || open_plugin_manager(&a));
    let a = app.clone();
    ui.on_open_link(move |url| {
        if let Err(e) = links::open(&url) {
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

/// The Tools menu: the core's tools, then the plugins' (interface 1.2).
fn set_tools(ui: &AppWindow, rows: &[ToolRow]) {
    let mut rows = rows.to_vec();
    PLUGIN_TOOLS.with(|t| {
        rows.extend(t.borrow().iter().map(|(_, _, name)| ToolRow {
            title: name.as_str().into(),
            enabled: true,
        }));
    });
    ui.set_tools(ModelRc::new(VecModel::from(rows)));
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
    let latest = s.footer.latest();
    let (level, text) = latest.map_or((0, ""), |n| (level_code(n.level), n.text.as_str()));
    let (link_text, link_url) = latest
        .and_then(|n| n.link.as_ref())
        .map_or(("", ""), |(t, u)| (t.as_str(), u.as_str()));
    ui.set_footer_level(level);
    ui.set_footer_text(text.into());
    ui.set_footer_link_text(link_text.into());
    ui.set_footer_link_url(link_url.into());
    ui.set_footer_count(s.footer.len() as i32);
    let rows: Vec<FooterRow> = s
        .footer
        .newest_first()
        .map(|n| {
            let (link_text, link_url) = n
                .link
                .as_ref()
                .map_or(("", ""), |(t, u)| (t.as_str(), u.as_str()));
            FooterRow {
                level: level_code(n.level),
                text: n.text.as_str().into(),
                link_text: link_text.into(),
                link_url: link_url.into(),
            }
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
            plugin: plugin_tabs::tab_of(area.active_id()).is_some(),
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
    drop(s);
    for side in Side::ALL {
        show_plugin_tab(app, side);
    }
}

/// The plugin tab area `side` shows, if it shows one: its tab is open and
/// the area is neither collapsed nor hidden.
fn plugin_tab_on_screen(s: &App, side: Side) -> Option<TabId> {
    let area = s.shell.area(side);
    if !area.shown || area.collapsed {
        return None;
    }
    plugin_tabs::tab_of(area.active_id())
}

/// Shows area `side`'s plugin tab, asking its plugin for the contents when
/// there are none yet or the plugin asked for a redraw while it was hidden.
fn show_plugin_tab(app: &Shared, side: Side) {
    let Some(tab) = plugin_tab_on_screen(&app.borrow(), side) else {
        let mut s = app.borrow_mut();
        let view = &mut s.plugin_views[side.index()];
        view.tab = None;
        view.content = None;
        return;
    };
    {
        let mut s = app.borrow_mut();
        let state = s.plugin_tabs.entry(tab.clone()).or_default();
        let ask = state.content.is_none() || state.stale;
        state.stale = false;
        if ask && let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Content(tab.clone()));
        }
    }
    refresh_plugin_view(app, side, &tab);
}

/// The plugin asked for its tab to be shown again.
fn plugin_tab_changed(app: &Shared, tab: TabId) {
    let on_screen = Side::ALL
        .iter()
        .any(|&side| plugin_tab_on_screen(&app.borrow(), side).as_ref() == Some(&tab));
    let mut s = app.borrow_mut();
    if on_screen {
        if let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Content(tab));
        }
    } else {
        s.plugin_tabs.entry(tab).or_default().stale = true;
    }
}

/// What the tab thread answered.
fn plugin_answer(app: &Shared, answer: plugin_tabs::Answer) {
    let tab = match answer {
        plugin_tabs::Answer::Content {
            tab,
            content,
            canvases,
        } => {
            let mut told = None;
            {
                let mut s = app.borrow_mut();
                let state = s.plugin_tabs.entry(tab.clone()).or_default();
                for drawn in canvases {
                    state
                        .canvases
                        .insert(drawn.widget.clone(), drawn_image(&drawn));
                }
                if let Err(why) = &content
                    && state.told.as_ref() != Some(why)
                {
                    state.told = Some(why.clone());
                    told = Some(why.clone());
                }
                state.content = Some(content);
            }
            if let Some(why) = told {
                message(app, Level::Warning, why);
            }
            tab
        }
        plugin_tabs::Answer::Canvas { tab, drawn } => {
            let mut s = app.borrow_mut();
            s.plugin_tabs
                .entry(tab.clone())
                .or_default()
                .canvases
                .insert(drawn.widget.clone(), drawn_image(&drawn));
            tab
        }
        plugin_tabs::Answer::Settings { plugin, content } => {
            settings_window_answer(app, &plugin, content);
            return;
        }
        plugin_tabs::Answer::Section {
            dialog,
            plugin,
            section,
            answer,
        } => {
            let changed = app
                .borrow_mut()
                .sections
                .as_mut()
                .is_some_and(|s| s.answered(dialog, &plugin, &section, answer));
            if changed {
                sync_track_sections(app);
            }
            return;
        }
    };
    for side in Side::ALL {
        if plugin_tab_on_screen(&app.borrow(), side).as_ref() == Some(&tab) {
            refresh_plugin_view(app, side, &tab);
        }
    }
}

fn drawn_image(drawn: &plugin_tabs::Drawn) -> Image {
    let buf =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&drawn.rgba, drawn.width, drawn.height);
    Image::from_rgba8_premultiplied(buf)
}

/// One plugin widget, as the window's `PluginTab` shows it.
fn plugin_widget(w: &Widget, canvases: &HashMap<String, Image>) -> PluginWidget {
    let mut out = PluginWidget {
        enabled: true,
        ..PluginWidget::default()
    };
    match w {
        Widget::Label { text, muted } => {
            out.kind = "label".into();
            out.text = text.as_str().into();
            out.muted = *muted;
        }
        Widget::Button { id, text, enabled } => {
            out.kind = "button".into();
            out.id = id.as_str().into();
            out.text = text.as_str().into();
            out.enabled = *enabled;
        }
        Widget::Toggle {
            id,
            text,
            on,
            enabled,
        } => {
            out.kind = "toggle".into();
            out.id = id.as_str().into();
            out.text = text.as_str().into();
            out.on = *on;
            out.enabled = *enabled;
        }
        Widget::Slider {
            id,
            value,
            min,
            max,
            enabled,
        } => {
            out.kind = "slider".into();
            out.id = id.as_str().into();
            out.value = *value;
            out.minimum = *min;
            out.maximum = *max;
            out.enabled = *enabled;
        }
        Widget::Field {
            id,
            text,
            placeholder,
            enabled,
        } => {
            out.kind = "field".into();
            out.id = id.as_str().into();
            out.text = text.as_str().into();
            out.text2 = placeholder.as_str().into();
            out.enabled = *enabled;
        }
        Widget::Choice {
            id,
            options,
            selected,
            enabled,
        } => {
            out.kind = "choice".into();
            out.id = id.as_str().into();
            out.options = ModelRc::new(VecModel::from(
                options
                    .iter()
                    .map(|o| SharedString::from(o.as_str()))
                    .collect::<Vec<_>>(),
            ));
            out.selected = *selected as i32;
            out.enabled = *enabled;
        }
        Widget::List {
            id,
            items,
            selected,
        } => {
            out.kind = "list".into();
            out.id = id.as_str().into();
            out.items = ModelRc::new(VecModel::from(
                items
                    .iter()
                    .map(|i| slint::StandardListViewItem::from(i.as_str()))
                    .collect::<Vec<_>>(),
            ));
            out.selected = selected.map_or(-1, |s| s as i32);
        }
        Widget::Canvas(c) => {
            out.kind = "canvas".into();
            out.id = c.id.as_str().into();
            out.canvas = canvases.get(&c.id).cloned().unwrap_or_default();
            out.canvas_width = c.width;
            out.canvas_height = c.height;
        }
    }
    out
}

/// The kind and id of each widget of a row: rows of the same shape are
/// changed in place.
fn row_shape(
    row: &oxiguitar_host::api::tab::Row,
) -> Vec<(std::mem::Discriminant<Widget>, Option<&str>)> {
    row.widgets
        .iter()
        .map(|w| (std::mem::discriminant(w), w.id()))
        .collect()
}

/// Puts tab `tab`'s contents in area `side`. Rows of the same shape as
/// before are changed in place (a row holding a canvas widget by widget,
/// so a drag on it carries on); other rows are made anew, so every widget
/// shows exactly what the plugin says.
fn refresh_plugin_view(app: &Shared, side: Side, tab: &TabId) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let mut guard = app.borrow_mut();
    let s = &mut *guard;
    let state = s.plugin_tabs.entry(tab.clone()).or_default();
    let view = &mut s.plugin_views[side.index()];
    let content = match &state.content {
        None => {
            set_plugin_area(&ui, side, None, "Asking the plugin…");
            view.tab = None;
            view.content = None;
            return;
        }
        Some(Err(why)) => {
            set_plugin_area(&ui, side, None, why);
            view.tab = None;
            view.content = None;
            return;
        }
        Some(Ok(content)) => content,
    };
    let canvases = &state.canvases;
    let widgets = |row: &oxiguitar_host::api::tab::Row| -> Vec<PluginWidget> {
        row.widgets
            .iter()
            .map(|w| plugin_widget(w, canvases))
            .collect()
    };
    let stretch = |row: &oxiguitar_host::api::tab::Row| {
        row.widgets
            .iter()
            .any(|w| matches!(w, Widget::Canvas(_) | Widget::List { .. }))
    };
    let same_shape = view.tab.as_ref() == Some(tab)
        && view.content.as_ref().is_some_and(|old| {
            old.rows.len() == content.rows.len()
                && old
                    .rows
                    .iter()
                    .zip(&content.rows)
                    .all(|(a, b)| row_shape(a) == row_shape(b))
        });
    if same_shape {
        let old = view.content.take().unwrap_or_default();
        for (i, (before, now)) in old.rows.iter().zip(&content.rows).enumerate() {
            let has_canvas = now.widgets.iter().any(|w| matches!(w, Widget::Canvas(_)));
            if has_canvas {
                // Widget by widget: the canvas's image may be new even when
                // its contents aren't.
                let model = &view.row_models[i];
                for (j, w) in widgets(now).into_iter().enumerate() {
                    if model.row_data(j).as_ref() != Some(&w) {
                        model.set_row_data(j, w);
                    }
                }
            } else if before != now {
                let model = Rc::new(VecModel::from(widgets(now)));
                view.rows.set_row_data(
                    i,
                    PluginRow {
                        widgets: ModelRc::from(model.clone()),
                        stretch: stretch(now),
                    },
                );
                view.row_models[i] = model;
            }
        }
    } else {
        view.row_models = content
            .rows
            .iter()
            .map(|r| Rc::new(VecModel::from(widgets(r))))
            .collect();
        view.rows = Rc::new(VecModel::from(
            content
                .rows
                .iter()
                .zip(&view.row_models)
                .map(|(r, m)| PluginRow {
                    widgets: ModelRc::from(m.clone()),
                    stretch: stretch(r),
                })
                .collect::<Vec<_>>(),
        ));
        set_plugin_area(&ui, side, Some(ModelRc::from(view.rows.clone())), "");
    }
    view.tab = Some(tab.clone());
    view.content = Some(content.clone());
}

/// Area `side`'s plugin tab: its rows, or a message instead.
fn set_plugin_area(ui: &AppWindow, side: Side, rows: Option<ModelRc<PluginRow>>, message: &str) {
    let rows = rows.unwrap_or_default();
    let message = SharedString::from(message);
    match side {
        Side::Top => {
            ui.set_top_plugin_rows(rows);
            ui.set_top_plugin_message(message);
        }
        Side::Bottom => {
            ui.set_bottom_plugin_rows(rows);
            ui.set_bottom_plugin_message(message);
        }
        Side::Left => {
            ui.set_left_plugin_rows(rows);
            ui.set_left_plugin_message(message);
        }
        Side::Right => {
            ui.set_right_plugin_rows(rows);
            ui.set_right_plugin_message(message);
        }
    }
}

/// Sends what the user did to the plugin tab area `area` (the window's
/// area number) shows.
fn plugin_event(app: &Shared, area: i32, widget: &str, event: TabEvent) {
    let s = app.borrow();
    let Some(tab) = Side::from_index(area).and_then(|side| plugin_tab_on_screen(&s, side)) else {
        return;
    };
    if let Some(thread) = &s.tab_thread {
        thread.send(plugin_tabs::Request::Event {
            tab,
            widget: widget.to_owned(),
            event,
        });
    }
}

/// A canvas's room changed, in logical pixels: it is drawn again for the
/// device pixels it now covers.
fn canvas_room(app: &Shared, area: i32, widget: &str, width: f32, height: f32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let scale = ui.window().scale_factor();
    let mut s = app.borrow_mut();
    let Some(tab) = Side::from_index(area).and_then(|side| plugin_tab_on_screen(&s, side)) else {
        return;
    };
    s.plugin_tabs
        .entry(tab.clone())
        .or_default()
        .rooms
        .insert(widget.to_owned(), (width, height));
    if let Some(thread) = &s.tab_thread {
        thread.send(plugin_tabs::Request::Size {
            tab,
            widget: widget.to_owned(),
            room: device_room(width, height, scale),
        });
    }
}

fn device_room(width: f32, height: f32, scale: f32) -> (u32, u32) {
    (
        (width * scale).round().max(1.0) as u32,
        (height * scale).round().max(1.0) as u32,
    )
}

/// The window's scale changed: every canvas is drawn again for its device
/// pixels.
fn resend_canvas_rooms(app: &Shared, scale: f32) {
    let s = app.borrow();
    let Some(thread) = &s.tab_thread else {
        return;
    };
    for (tab, state) in &s.plugin_tabs {
        for (widget, (w, h)) in &state.rooms {
            thread.send(plugin_tabs::Request::Size {
                tab: tab.clone(),
                widget: widget.clone(),
                room: device_room(*w, *h, scale),
            });
        }
    }
}

/// The callbacks of every area's `PluginTab`.
fn install_plugin_tab_callbacks(ui: &AppWindow, app: &Shared) {
    let a = app.clone();
    ui.on_plugin_clicked(move |area, id| plugin_event(&a, area, &id, TabEvent::Clicked));
    let a = app.clone();
    ui.on_plugin_toggled(move |area, id, on| plugin_event(&a, area, &id, TabEvent::Toggled(on)));
    let a = app.clone();
    ui.on_plugin_slid(move |area, id, v| plugin_event(&a, area, &id, TabEvent::Slid(v)));
    let a = app.clone();
    ui.on_plugin_edited(move |area, id, text| {
        plugin_event(&a, area, &id, TabEvent::Edited(text.to_string()));
    });
    let a = app.clone();
    ui.on_plugin_chosen(move |area, id, i| {
        if let Ok(i) = u32::try_from(i) {
            plugin_event(&a, area, &id, TabEvent::Chosen(i));
        }
    });
    let a = app.clone();
    ui.on_plugin_pointer(move |area, id, kind, x, y, button| {
        let kind = match kind {
            0 => PointerKind::Down,
            1 => PointerKind::Up,
            _ => PointerKind::Drag,
        };
        let button = match button {
            1 => PointerButton::Secondary,
            2 => PointerButton::Middle,
            _ => PointerButton::Primary,
        };
        let pointer = Pointer {
            kind,
            at: CanvasPoint::new(x, y),
            button,
        };
        plugin_event(&a, area, &id, TabEvent::Pointer(pointer));
    });
    let a = app.clone();
    ui.on_plugin_canvas_room(move |area, id, w, h| canvas_room(&a, area, &id, w, h));
}

/// The command line: `oxiguitar [--safe-mode] [FILE]`.
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

/// Tools → Plugins: the plugin manager's window, made the first time and
/// shown again (in front) after that. It doesn't block the main window.
fn open_plugin_manager(app: &Shared) {
    let existing = app
        .borrow()
        .manager
        .as_ref()
        .map(ComponentHandle::clone_strong);
    let window = match existing {
        Some(w) => w,
        None => {
            let Ok(w) = PluginManagerWindow::new() else {
                message(
                    app,
                    Level::Error,
                    "The plugin manager's window couldn't be opened.",
                );
                return;
            };
            let a = app.clone();
            w.on_toggled(move |i, on| manager_toggled(&a, i, on));
            let a = app.clone();
            w.on_settings_clicked(move |i| {
                if let Some(id) = app_rows_settings(&a, i) {
                    open_settings_window(&a, id);
                }
            });
            let weak = w.as_weak();
            w.on_close_clicked(move || {
                if let Some(w) = weak.upgrade() {
                    let _ = w.hide();
                }
            });
            app.borrow_mut().manager = Some(w.clone_strong());
            w
        }
    };
    {
        let s = app.borrow();
        window.set_dark_interface(s.prefs.dark_interface);
        window.set_safe_mode(s.plugins.safe_mode());
        let can_save = s.stored.path().is_some();
        window.set_can_save(can_save);
        window.set_problem(if can_save {
            SharedString::new()
        } else {
            SharedString::from(format!(
                "{} couldn't be read when OxiGuitar started, so plugins can't be enabled or disabled now: correct it (or remove it) and start OxiGuitar again.",
                oxiguitar_host::SETTINGS_FILE
            ))
        });
    }
    sync_manager(app);
    let _ = window.show();
}

/// The manager's rows, as settings.json stands now.
fn sync_manager(app: &Shared) {
    let s = app.borrow();
    let Some(window) = &s.manager else {
        return;
    };
    let rows = manager::rows(&s.plugins, &manager::disabled(&s.stored));
    window.set_restart_needed(manager::restart_needed(&rows));
    let rows: Vec<ManagerRow> = rows
        .into_iter()
        .map(|r| ManagerRow {
            name: r.name.into(),
            version: r.version.into(),
            file: r.file.into(),
            folder: r.folder.into(),
            state: r.state.into(),
            can_toggle: r.id.is_some(),
            enabled: r.enabled,
            has_settings: r.settings.is_some(),
        })
        .collect();
    window.set_rows(ModelRc::new(VecModel::from(rows)));
}

/// A tickbox in the manager: enable or disable that plugin from the next
/// start. A loaded plugin can't be unloaded, so unticking one also says in
/// the footer that it stays in use until OxiGuitar restarts.
fn manager_toggled(app: &Shared, index: i32, on: bool) {
    let (result, row) = {
        let s = app.borrow();
        let row = usize::try_from(index)
            .ok()
            .and_then(|i| manager::rows(&s.plugins, &[]).into_iter().nth(i));
        let result = row
            .as_ref()
            .and_then(|r| r.id.as_ref())
            .map(|id| manager::set_enabled(&s.stored, id, on));
        (result, row)
    };
    match result {
        Some(Err(e)) => {
            if let Some(w) = &app.borrow().manager {
                w.set_problem(e.into());
            }
        }
        Some(Ok(())) if !on && row.as_ref().is_some_and(|r| r.loaded) => {
            let name = row.map(|r| r.name).unwrap_or_default();
            message(
                app,
                Level::Info,
                format!(
                    "{name} stays in use until you restart OxiGuitar: a loaded plugin can't be unloaded while it runs."
                ),
            );
        }
        _ => {}
    }
    sync_manager(app);
}

/// Puts the View menu's settings on the window.
fn apply_prefs(ui: &AppWindow, p: ViewPrefs) {
    ui.set_dark_interface(p.dark_interface);
    ui.set_dark_page(p.dark_page);
    ui.set_dynamics(p.dynamics);
    let index = |found: Option<usize>| found.unwrap_or(0) as i32;
    ui.invoke_pick_layout(index(Layout::ALL.iter().position(|l| *l == p.layout)));
    ui.invoke_pick_paper(index(Paper::ALL.iter().position(|q| *q == p.paper)));
}

/// A toolbar button was clicked.
fn toolbar_activated(app: &Shared, id: &str) {
    match id {
        item::OPEN => unless_unsaved(app, Then::OpenDialog),
        item::SAVE => save(app),
        item::EXPORT_PDF => export_dialog(app),
        item::UNDO => app.borrow().worker.send(Request::Undo),
        item::REDO => app.borrow().worker.send(Request::Redo),
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
            settings_changed(app);
            playing::score_changed(app, true);
        }
        Event::AskSave {
            heading,
            lines,
            elsewhere,
        } => {
            let buttons: &[&str] = if elsewhere {
                &["Save", "Save As…", "Cancel"]
            } else {
                &["Save", "Cancel"]
            };
            ask(app, Asked::Save { elsewhere }, &heading, &lines, buttons);
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
        } => {
            {
                let mut s = app.borrow_mut();
                s.editing.changed(score);
                if let Some(c) = caret {
                    s.editing.edited(c);
                }
                s.bad = bad;
            }
            if let Some((tracks, was)) = tracks {
                track_list_changed(app, &tracks, &was);
            }
            sync_sections(app);
            sync_edit_state(app, &edit);
            if caret.is_some() {
                caret_moved(app);
            } else {
                sync_caret(app, false);
            }
            playing::score_changed(app, false);
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
                    lists: Vec::new(),
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

/// A press on the page, at (`x`, `y`) in the viewport's logical pixels:
/// the caret goes there (with Shift, the selection grows to it).
fn page_pressed(app: &Shared, x: f32, y: f32, shift: bool) {
    // A click elsewhere ends typing lyrics, keeping what was typed.
    let typing = app
        .borrow()
        .ui
        .upgrade()
        .map(|ui| (ui.get_lyric_shown(), ui.get_lyric_text().to_string()));
    if let Some((true, text)) = typing {
        lyric_key(app, "done", &text);
    }
    let moved = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.press_at(i, sx, sy, shift));
    if moved {
        caret_moved(app);
    }
}

/// A right-click on the page, before its menu: the caret goes there unless
/// the click is inside the selection.
fn page_menu_pressed(app: &Shared, x: f32, y: f32) {
    let moved = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.menu_press_at(i, sx, sy));
    if moved {
        caret_moved(app);
    }
}

/// A double-click on the page: on a time signature, the caret goes to its
/// bar and the Bar dialog opens there (Mateus, 20 Oct 2026).
fn page_double_clicked(app: &Shared, x: f32, y: f32) {
    let on_time = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.time_signature_at(i, sx, sy));
    if on_time {
        caret_moved(app);
        open_edit_dialog(app, editing::Dialog::Bar);
    }
}

/// A point of the canvas, in logical pixels, as the sheet under it and the
/// point on that sheet, in points.
fn on_sheet(app: &Shared, x: f32, y: f32) -> Option<(usize, f32, f32)> {
    let ui = app.borrow().ui.upgrade()?;
    let s = app.borrow();
    let px = PX_PER_PT * s.zoom;
    let (cx, cy) = ((x - ui.get_origin_x()) / px, (y - ui.get_origin_y()) / px);
    s.sheets
        .rects
        .iter()
        .enumerate()
        .find(|(_, r)| cx >= r.x && cx < r.right() && cy >= r.y && cy < r.bottom())
        .map(|(i, r)| (i, cx - r.x, cy - r.y))
}

/// The caret or the selection moved: draw them, keep the caret in view,
/// and tell the plugins that listen.
/// Sends a command typed at the caret to the worker.
fn send_typed(app: &Shared, t: editing::Typed) {
    app.borrow().worker.send(Request::Edit {
        command: t.command,
        caret: t.caret,
        selection: t.selection,
    });
}

fn caret_moved(app: &Shared) {
    playing::caret_moved(app);
    playing::sync(app);
    sync_caret(app, true);
    follow_voice(app);
    let s = app.borrow();
    s.worker.send(Request::Selection(
        s.editing.for_plugins(),
        s.editing.for_plugins2(),
    ));
}

/// The page greys out the voices the caret isn't in: a new voice is a new
/// engraving (Mateus, 20 Oct 2026).
fn follow_voice(app: &Shared) {
    let mut s = app.borrow_mut();
    let Some(voice) = s.editing.at_caret().map(|a| a.voice) else {
        return;
    };
    if s.settings.active_voice != Some(voice) && s.track_count > 0 {
        s.settings.active_voice = Some(voice);
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
}

/// Draws the caret and the selection over the page, highlights the
/// caret's track, and, when `scroll`, scrolls the caret into view.
fn sync_caret(app: &Shared, scroll: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    sync_inspector(app, &ui);
    let s = app.borrow();
    ui.set_has_selection(s.editing.selection().is_some());
    // A box on sheet `sheet`, in canvas points.
    let place = |sheet: usize, r: Rect| -> Option<BoxItem> {
        let o = s.sheets.rects.get(sheet)?;
        Some(BoxItem {
            x: o.x + r.x,
            y: o.y + r.y,
            w: r.width,
            h: r.height,
        })
    };
    let found = s.editing.caret_box();
    let column = found.and_then(|f| place(f.sheet, f.column));
    // On the standard staff, the caret's line or space; else its string.
    let string = found.and_then(|f| f.step.or(f.string).and_then(|r| place(f.sheet, r)));
    ui.set_caret_shown(column.is_some());
    if let Some(b) = &column {
        ui.set_caret(b.clone());
    }
    ui.set_string_shown(string.is_some());
    if let Some(b) = string {
        ui.set_caret_string(b);
    }
    let boxes: Vec<BoxItem> = s
        .editing
        .selection_boxes()
        .into_iter()
        .filter_map(|(sheet, r)| place(sheet, r))
        .collect();
    ui.set_selection_boxes(ModelRc::new(VecModel::from(boxes)));
    ui.set_selected_track(
        s.editing
            .track()
            .and_then(|t| i32::try_from(t).ok())
            .unwrap_or(-1),
    );
    ui.set_bad_bars(s.editing.any_shown(&s.bad));
    sync_palette_state(&ui, s.editing.at_caret());
    if let (true, Some(b)) = (scroll, column) {
        let px = PX_PER_PT * s.zoom;
        let x0 = ui.get_origin_x() + b.x * px;
        let y0 = ui.get_origin_y() + b.y * px;
        let (x1, y1) = (x0 + b.w * px, y0 + b.h * px);
        let (vx, vy) = (ui.get_view_x(), ui.get_view_y());
        let (vw, vh) = (ui.get_view_width(), ui.get_view_height());
        let margin = 24.0;
        let nx = if x0 < vx + margin {
            x0 - margin
        } else if x1 > vx + vw - margin {
            (x1 - vw + margin).min(x0 - margin)
        } else {
            vx
        };
        // Off screen (a jump such as F4): the caret's staves go to the
        // middle of the view. Near an edge: just enough to show them.
        let ny = if y1 < vy || y0 > vy + vh {
            y0 - (vh - (y1 - y0)).max(0.0) / 2.0
        } else if y0 < vy + margin {
            y0 - margin
        } else if y1 > vy + vh - margin {
            (y1 - vh + margin).min(y0 - margin)
        } else {
            vy
        };
        if (nx - vx).abs() > 0.5 || (ny - vy).abs() > 0.5 {
            drop(s);
            ui.invoke_scroll_to(nx, ny);
        }
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
    // Remember the View menu for next time, when it changed.
    let prefs = ViewPrefs {
        dark_interface: ui.get_dark_interface(),
        dark_page: ui.get_dark_page(),
        dynamics: s.settings.dynamics,
        layout: s.settings.layout,
        paper: s.settings.paper,
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
        tabs.send(plugin_tabs::Request::Dark(prefs.dark_interface));
    }
    if let Some(m) = &s.manager {
        m.set_dark_interface(prefs.dark_interface);
    }
    if let Some((w, _)) = &s.settings_window {
        w.set_dark_interface(prefs.dark_interface);
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

/// Draws the track rows' and the palette's icons for the window's current
/// scale.
fn refresh_icons(app: &Shared, ui: &AppWindow) {
    let scale = ui.window().scale_factor();
    if (scale - app.borrow().icons_scale).abs() < 1e-3 {
        return;
    }
    let px = (TOGGLE_ICON * scale).round().max(8.0) as u32;
    let images: Vec<Image> = app.borrow().toggles.draw(px).iter().map(image_of).collect();
    if let [std, tab, mute, solo] = images.as_slice() {
        ui.set_standard_icon(std.clone());
        ui.set_tablature_icon(tab.clone());
        ui.set_mute_icon(mute.clone());
        ui.set_solo_icon(solo.clone());
    }
    app.borrow_mut().icons_scale = scale;
    set_palette(app, ui, true);
}

/// Measures the palette's icons on a thread of their own, so the window
/// never waits for them, and shows the palette once they are ready.
fn measure_palette() {
    let spawned = std::thread::Builder::new()
        .name("palette icons".to_owned())
        .spawn(|| {
            let faces: Vec<(icons::Face, icons::Fit)> =
                palette::tools().map(|(_, t)| (t.face, t.fit)).collect();
            let set = Arc::new(icons::IconSet::new(&faces, icons::Fill::PALETTE));
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = APP.with(|a| a.borrow().clone()) else {
                    return;
                };
                app.borrow_mut().palette_icons = Some(set);
                if let Some(ui) = app.borrow().ui.upgrade() {
                    set_palette(&app, &ui, true);
                }
            });
        });
    if let Err(e) = spawned {
        eprintln!("oxiguitar: the palette's icons can't be drawn: {e}");
    }
}

fn image_of(icon: &icons::Icon) -> Image {
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&icon.rgba, icon.size, icon.size);
    Image::from_rgba8_premultiplied(buf)
}

/// The palette's groups for the window: every tool greyed out until the
/// editor (M3), with its icon once the icons are measured. `redraw` draws
/// the icons again (a new scale); otherwise only the groups' collapsed
/// state changes and the icons already shown are kept.
fn set_palette(app: &Shared, ui: &AppWindow, redraw: bool) {
    let s = app.borrow();
    let Some(set) = s.palette_icons.clone() else {
        return;
    };
    let model = ui.get_palette_groups();
    let fresh = redraw || model.row_count() != palette::GROUPS.len();
    let images: Vec<Image> = if fresh {
        let px = (PALETTE_BUTTON * ui.window().scale_factor())
            .round()
            .max(8.0) as u32;
        set.draw(px).iter().map(image_of).collect()
    } else {
        Vec::new()
    };
    let mut images = images.into_iter();
    let groups: Vec<PaletteGroupRow> = palette::GROUPS
        .iter()
        .enumerate()
        .map(|(g, group)| {
            let tools: ModelRc<PaletteToolRow> = if fresh {
                let rows: Vec<PaletteToolRow> = group
                    .tools
                    .iter()
                    .map(|t| PaletteToolRow {
                        id: t.id.into(),
                        icon: images.next().unwrap_or_default(),
                        tip: palette::tool_tip(t.id, t.name).into(),
                        enabled: false,
                        checked: false,
                    })
                    .collect();
                ModelRc::new(VecModel::from(rows))
            } else {
                model.row_data(g).map(|r| r.tools).unwrap_or_default()
            };
            PaletteGroupRow {
                title: group.title.into(),
                collapsed: s.palette.collapsed(g),
                tools,
            }
        })
        .collect();
    let at = s.editing.at_caret();
    drop(s);
    if fresh {
        ui.set_palette_groups(ModelRc::new(VecModel::from(groups)));
    } else {
        for (g, row) in groups.into_iter().enumerate() {
            model.set_row_data(g, row);
        }
    }
    sync_palette_state(ui, at);
}

/// The palette's working tools: usable with a score open, highlighted as
/// the caret's beat is (its voice, value, dots, rest, tie).
fn sync_palette_state(ui: &AppWindow, at: Option<editing::AtCaret>) {
    sync_palette_rows(ui, at);
    if !ui.get_palette_query().is_empty() {
        sync_found(ui);
    }
}

fn sync_palette_rows(ui: &AppWindow, at: Option<editing::AtCaret>) {
    let groups = ui.get_palette_groups();
    for g in 0..groups.row_count() {
        let Some(group) = groups.row_data(g) else {
            continue;
        };
        for i in 0..group.tools.row_count() {
            let Some(mut row) = group.tools.row_data(i) else {
                continue;
            };
            if palette::working(row.id.as_str()).is_none() {
                continue;
            }
            let (enabled, checked) = (
                at.is_some(),
                at.is_some_and(|at| palette::checked(row.id.as_str(), &at)),
            );
            if (row.enabled, row.checked) != (enabled, checked) {
                row.enabled = enabled;
                row.checked = checked;
                group.tools.set_row_data(i, row);
            }
        }
    }
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
    ui.set_pt_scale(PX_PER_PT * zoom);
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

/// Goes on to `then`, asking first whether to save the changes when the
/// score differs from its file.
fn unless_unsaved(app: &Shared, then: Then) {
    let name = {
        let s = app.borrow();
        if s.question.is_some() {
            return;
        }
        s.modified.then(|| s.doc_name.clone().unwrap_or_default())
    };
    match name {
        Some(name) => ask(
            app,
            Asked::Unsaved(then),
            &format!("Save the changes to {name}?"),
            &["They are lost if you don't.".to_owned()],
            &["Save", "Don't Save", "Cancel"],
        ),
        None => go_on(app, then),
    }
}

/// Does what waited on the unsaved-changes question.
fn go_on(app: &Shared, then: Then) {
    match then {
        Then::New => app.borrow().worker.send(Request::New),
        Then::OpenDialog => open_dialog(app),
        Then::Open(path) => app.borrow().worker.send(Request::Open(path)),
        Then::Quit => {
            let _ = slint::quit_event_loop();
        }
    }
}

/// Shows the question over the window.
fn ask(app: &Shared, asked: Asked, heading: &str, lines: &[String], buttons: &[&str]) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    app.borrow_mut().question = Some(asked);
    ui.set_question_heading(heading.into());
    ui.set_question_lines(ModelRc::new(VecModel::from(
        lines
            .iter()
            .map(|l| SharedString::from(l.as_str()))
            .collect::<Vec<_>>(),
    )));
    ui.set_question_buttons(ModelRc::new(VecModel::from(
        buttons
            .iter()
            .map(|b| SharedString::from(*b))
            .collect::<Vec<_>>(),
    )));
    ui.set_question_shown(true);
}

/// A list of strings for the window.
fn strings(list: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        list.into_iter().map(SharedString::from).collect::<Vec<_>>(),
    ))
}

/// Opens the Bar dialog or Custom N-tuplet at the caret.
fn open_edit_dialog(app: &Shared, dialog: editing::Dialog) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() {
        return;
    }
    match dialog {
        editing::Dialog::Bar => {
            let Some((state, heading)) = app.borrow().editing.bar_state() else {
                return;
            };
            let f = bar_form::BarForm::of(&state);
            ui.set_bar_fields(BarFields {
                numerator: f.numerator,
                denominator: f.denominator,
                key: f.key,
                minor: f.minor,
                clef: f.clef,
                tempo: f.tempo,
                feel: f.feel,
                repeat_start: f.repeat_start,
                repeat_count: f.repeat_count,
                letter: f.letter.into(),
                section: f.section.into(),
                double_bar: f.double_bar,
                free_time: f.free_time,
            });
            ui.set_bar_endings(ModelRc::new(VecModel::from(f.endings)));
            ui.set_bar_directions(ModelRc::new(VecModel::from(f.directions)));
            ui.set_bar_heading(heading.into());
            app.borrow_mut().bar_dialog = Some(state);
            ui.set_bar_dialog_shown(true);
        }
        editing::Dialog::Tuplet => {
            if app.borrow().editing.bar_state().is_none() {
                return;
            }
            let t = app
                .borrow()
                .editing
                .beat_tuplet()
                .unwrap_or(oxiguitar_model::Tuplet {
                    notes: 5,
                    in_time_of: 4,
                });
            ui.set_tuplet_notes(i32::from(t.notes));
            ui.set_tuplet_in_time_of(i32::from(t.in_time_of));
            ui.set_tuplet_dialog_shown(true);
        }
        editing::Dialog::Bend | editing::Dialog::Whammy => {
            let whammy = dialog == editing::Dialog::Whammy;
            let here = {
                let s = app.borrow();
                if whammy {
                    s.editing.whammy_here()
                } else {
                    s.editing.bend_here()
                }
            };
            let Some(curve) = here else {
                return;
            };
            let form = bend_form::BendForm::open(curve.as_ref(), whammy);
            ui.set_bend_heading(if whammy { "Whammy Bar" } else { "Bend" }.into());
            sync_bend(&ui, &form);
            app.borrow_mut().bend_dialog = Some(form);
            ui.set_bend_dialog_shown(true);
        }
        editing::Dialog::Chord => {
            let Some(form) = app.borrow().editing.chord_form() else {
                return;
            };
            ui.set_chord_last_fret(i32::from(form.frets_max()));
            sync_chord(&ui, &form);
            app.borrow_mut().chord_dialog = Some(form);
            ui.set_chord_dialog_shown(true);
        }
        editing::Dialog::Text => {
            let Some(text) = app.borrow().editing.beat_text() else {
                return;
            };
            ui.set_text_dialog_text(text.into());
            ui.set_text_dialog_shown(true);
        }
        editing::Dialog::Lyrics => {
            let started = app.borrow_mut().editing.start_lyrics();
            if let Some((_, text)) = started {
                show_lyric(app, &ui, text);
            }
        }
    }
}

/// Ctrl + F: the palette shows, and its search field takes the keyboard.
fn search_palette(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    {
        let mut s = app.borrow_mut();
        let area = s.shell.area_mut(Side::Left);
        if let Some(i) = area.tabs.iter().position(|t| t.id == "palette") {
            area.active = i;
        }
        area.collapsed = false;
        area.shown = true;
    }
    sync_areas(app);
    ui.set_left_shown(true);
    ui.set_palette_search_requests(ui.get_palette_search_requests() + 1);
}

/// Shows the field where lyrics are typed, holding `text`, under the beat
/// it types at; hidden when nothing is typed there.
fn show_lyric(app: &Shared, ui: &AppWindow, text: String) {
    let spot = {
        let s = app.borrow();
        s.editing.lyric_spot().and_then(|(sheet, x, y, gap)| {
            let o = s.sheets.rects.get(sheet)?;
            Some((o.x + x, o.y + y, gap))
        })
    };
    match spot {
        Some((x, y, gap)) => {
            ui.set_lyric_x(x);
            ui.set_lyric_y(y);
            // The engraver's lyric size: 1.9 staff spaces, its lines 2.4
            // apart.
            ui.set_lyric_size(gap * 1.9 / 2.4);
            ui.set_lyric_text(text.into());
            ui.set_lyric_shown(true);
        }
        None => ui.set_lyric_shown(false),
    }
    caret_moved(app);
}

/// A key that ends a syllable typed on the page.
fn lyric_key(app: &Shared, key: &str, text: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let key = match key {
        "space" => editing::LyricKey::Space,
        "hyphen" => editing::LyricKey::Hyphen,
        "back" => editing::LyricKey::Back,
        "up" => editing::LyricKey::Up,
        "down" => editing::LyricKey::Down,
        _ => editing::LyricKey::Done,
    };
    let (edit, next) = app.borrow_mut().editing.lyric_key(key, text);
    if let Some(t) = edit {
        send_typed(app, t);
    }
    match next {
        Some((_, text)) => show_lyric(app, &ui, text),
        None => {
            ui.set_lyric_shown(false);
            ui.invoke_focus_page();
        }
    }
}

/// An edit at the caret (or on the selection), as a tool makes it.
fn edit_at_caret(app: &Shared, command: oxiguitar_edit::Command) {
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// Shows the chord dialog's state.
fn sync_chord(ui: &AppWindow, f: &chord_form::ChordForm) {
    use oxiguitar_edit::harmony::{Extension, Triad, alt};
    ui.set_chord_name(f.name.as_str().into());
    ui.set_chord_library(strings(f.library.iter().map(|d| d.name.clone()).collect()));
    let c = f.chord;
    let index = |i: Option<usize>| i.and_then(|i| i32::try_from(i).ok()).unwrap_or(-1);
    ui.set_chord_root(index(c.map(|c| c.root)));
    ui.set_chord_triad(index(
        c.and_then(|c| Triad::ALL.iter().position(|t| *t == c.triad)),
    ));
    ui.set_chord_extension(index(
        c.and_then(|c| Extension::ALL.iter().position(|e| *e == c.extension)),
    ));
    ui.set_chord_bass(index(c.map(|c| c.bass.map_or(0, |b| b + 1))));
    let bits = c.map_or(0, |c| c.alterations);
    ui.set_chord_alterations(ModelRc::new(VecModel::from(
        alt::NAMES
            .iter()
            .map(|(bit, name)| AltBox {
                label: name.replace('b', "♭").replace('#', "♯").into(),
                on: bits & bit != 0,
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_chord_strings(i32::try_from(f.strings()).unwrap_or(0));
    ui.set_chord_dots(ModelRc::new(VecModel::from(f.dots())));
    ui.set_chord_first_fret(i32::from(f.first_fret));
    ui.set_chord_voicings(strings(
        f.voicings
            .iter()
            .map(|v| chord_form::ChordForm::shape_text(v))
            .collect(),
    ));
    ui.set_chord_show_diagram(f.show_diagram);
}

/// A change in the chord dialog: `kind` names what, `value` and `text`
/// carry it.
fn chord_changed(app: &Shared, kind: &str, value: i32, text: &str) {
    use oxiguitar_edit::harmony::{Extension, Triad, alt};
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if kind == "remove" {
        ui.set_chord_dialog_shown(false);
        ui.invoke_focus_page();
        app.borrow_mut().chord_dialog = None;
        edit_at_caret(app, oxiguitar_edit::Command::Chord(None));
        return;
    }
    let i = usize::try_from(value).unwrap_or(0);
    let form = {
        let mut s = app.borrow_mut();
        let Some(f) = s.chord_dialog.as_mut() else {
            return;
        };
        let bit = alt::NAMES.get(i).map_or(0, |(b, _)| *b);
        match kind {
            "name" => f.type_name(text),
            "library" => f.pick_library(i),
            "root" => f.build(|c| c.root = i),
            "triad" => f.build(|c| c.triad = Triad::ALL.get(i).copied().unwrap_or(c.triad)),
            "extension" => {
                f.build(|c| c.extension = Extension::ALL.get(i).copied().unwrap_or(c.extension))
            }
            "bass" => f.build(|c| c.bass = i.checked_sub(1)),
            "alt-on" => f.build(|c| c.alterations |= bit),
            "alt-off" => f.build(|c| c.alterations &= !bit),
            "grid" => {
                let strings = f.strings();
                let low = i / 100;
                if let Some(high_first) = strings.checked_sub(low + 1) {
                    f.click_grid(high_first, u8::try_from(i % 100).unwrap_or(0));
                }
            }
            "head" => {
                if let Some(high_first) = f.strings().checked_sub(i + 1) {
                    f.click_head(high_first);
                }
            }
            "first" => f.scroll(u8::try_from(i).unwrap_or(1)),
            "voicing" => f.pick_voicing(i),
            "show" => f.show_diagram = value != 0,
            _ => {}
        }
        f.clone()
    };
    sync_chord(&ui, &form);
}

/// The chord dialog closed: on OK, its diagram on the caret's beat.
fn chord_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_chord_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(form) = app.borrow_mut().chord_dialog.take() else {
        return;
    };
    if ok {
        edit_at_caret(app, oxiguitar_edit::Command::Chord(form.diagram()));
    }
}

/// Shows the bend editor's state.
fn sync_bend(ui: &AppWindow, f: &bend_form::BendForm) {
    let types: &[&str] = if f.whammy {
        &bend_form::WHAMMY_TYPES
    } else {
        &bend_form::BEND_TYPES
    };
    // "Custom" last, always, so the list never changes under the box.
    let mut names: Vec<String> = types.iter().map(|t| (*t).to_owned()).collect();
    names.push("Custom".to_owned());
    let kind = f.kind.unwrap_or(types.len());
    if ui.get_bend_types().row_count() != names.len() {
        ui.set_bend_types(strings(names));
    }
    ui.set_bend_kind(i32::try_from(kind).unwrap_or(0));
    ui.set_bend_amount(f.amount - 1);
    let (low, high) = f.rows();
    ui.set_bend_low(low);
    ui.set_bend_high(high);
    ui.set_bend_points(ModelRc::new(VecModel::from(
        f.points
            .iter()
            .map(|p| GridPoint {
                col: p.col,
                row: p.row,
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_bend_path(f.path().into());
}

/// A change in the bend editor.
fn bend_edit(app: &Shared, edit: impl FnOnce(&mut bend_form::BendForm)) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let form = {
        let mut s = app.borrow_mut();
        let Some(f) = s.bend_dialog.as_mut() else {
            return;
        };
        edit(f);
        f.clone()
    };
    sync_bend(&ui, &form);
}

/// The bend editor closed: on OK, a changed curve is one change at the
/// caret (or on the selection).
fn bend_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_bend_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(form) = app.borrow_mut().bend_dialog.take() else {
        return;
    };
    if !ok || !form.changed {
        return;
    }
    let command = if form.whammy {
        oxiguitar_edit::Command::Whammy(form.bend())
    } else {
        oxiguitar_edit::Command::Bend(form.bend())
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The Bar dialog closed: on OK, what it sets is one change at the caret.
fn bar_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_bar_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(was) = app.borrow_mut().bar_dialog.take() else {
        return;
    };
    if !ok {
        return;
    }
    let f = ui.get_bar_fields();
    let form = bar_form::BarForm {
        numerator: f.numerator,
        denominator: f.denominator,
        key: f.key,
        minor: f.minor,
        clef: f.clef,
        tempo: f.tempo,
        feel: f.feel,
        repeat_start: f.repeat_start,
        repeat_count: f.repeat_count,
        letter: f.letter.to_string(),
        section: f.section.to_string(),
        double_bar: f.double_bar,
        free_time: f.free_time,
        endings: ui.get_bar_endings().iter().collect(),
        directions: ui.get_bar_directions().iter().collect(),
    };
    let state = form.to_state(&was);
    if state == was {
        return;
    }
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Bar(Box::new(state)));
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The track list's rows.
fn track_rows(app: &Shared, tracks: &[TrackInfo]) -> Vec<TrackRow> {
    let s = app.borrow();
    tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let row = s.tracks.row(i);
            let mix = s.mixer.get(i);
            TrackRow {
                name: t.name.as_str().into(),
                shown: row.shown,
                standard: row.standard,
                tablature: row.tablature,
                can_tab: t.can_tab,
                volume: mix.volume,
                pan: mix.pan,
                mute: mix.mute,
                solo: mix.solo,
                wave: tracks::waveform(&t.activity).into(),
                bars: i32::try_from(t.activity.len()).unwrap_or(i32::MAX),
            }
        })
        .collect()
}

/// An edit added, deleted, moved, renamed or restrung tracks: the track
/// list and the page follow, keeping what they can (`Tracks::refit`).
fn track_list_changed(app: &Shared, tracks: &[TrackInfo], was: &[Option<usize>]) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    {
        let mut s = app.borrow_mut();
        let caret = s.editing.track().unwrap_or(0);
        s.track_count = tracks.len();
        s.tracks = s.tracks.refit(
            tracks.iter().map(|t| t.default_notation).collect(),
            tracks.iter().map(|t| t.can_tab).collect(),
            was,
            caret,
        );
        // The caret left on a hidden track (its own was deleted): to the
        // nearest shown one.
        if let Some(to) = s.tracks.nearest_shown(caret)
            && !s.tracks.shown().contains(&caret)
        {
            s.editing.select_track(to);
        }
        s.mixer = Mixer::new(tracks.iter().map(|t| t.mix).collect());
        s.settings.tracks = s.tracks.shown();
        s.settings.notation = s.tracks.notation();
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
    ui.set_tracks(ModelRc::new(VecModel::from(track_rows(app, tracks))));
    sync_tracks(app);
}

/// The Inspector shows the caret's bar: each voice's beats against the
/// time signature.
/// The Song tab: each field changed in place, so one being typed in keeps
/// its place and cursor.
fn sync_song(app: &Shared, ui: &AppWindow) {
    let fields = app
        .borrow()
        .editing
        .score()
        .map(|s| song::fields(&s.info))
        .unwrap_or_default();
    let rows: Vec<SongField> = fields
        .into_iter()
        .map(|f| SongField {
            id: f.id.into(),
            label: f.label.into(),
            text: f.text.into(),
            multiline: f.multiline,
        })
        .collect();
    let model = ui.get_song_fields();
    if model.row_count() != rows.len() {
        ui.set_song_fields(ModelRc::new(VecModel::from(rows)));
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).is_some_and(|r| r.text != row.text) {
            model.set_row_data(i, row);
        }
    }
}

/// The palette's search: the tools whose names hold the words typed, as
/// the palette shows them (enabled, lit).
fn sync_found(ui: &AppWindow) {
    let query = ui.get_palette_query().to_lowercase();
    if query.trim().is_empty() {
        ui.set_palette_found(ModelRc::default());
        return;
    }
    let groups = ui.get_palette_groups();
    let mut found = Vec::new();
    for (g, group) in palette::GROUPS.iter().enumerate() {
        let Some(row) = groups.row_data(g) else {
            continue;
        };
        for (i, t) in group.tools.iter().enumerate() {
            if palette::matches(t.name, &query)
                && let Some(tool) = row.tools.row_data(i)
            {
                found.push(tool);
            }
        }
    }
    ui.set_palette_found(ModelRc::new(VecModel::from(found)));
}

fn sync_inspector(app: &Shared, ui: &AppWindow) {
    sync_song(app, ui);
    let fields = app.borrow().editing.effect_fields();
    ui.set_inspector_fields(ModelRc::new(VecModel::from(
        fields
            .into_iter()
            .map(|f| InspectorField {
                id: f.id.into(),
                label: f.label.into(),
                kind: match f.kind {
                    effect_form::Kind::Heading => 0,
                    effect_form::Kind::Switch => 1,
                    effect_form::Kind::Choice => 2,
                    effect_form::Kind::Number => 3,
                    effect_form::Kind::Button => 4,
                },
                value: f.value,
                options: strings(f.options),
                min: f.min,
                max: f.max,
            })
            .collect::<Vec<_>>(),
    )));
    let view = app.borrow().editing.bar_view();
    let Some(v) = view else {
        ui.set_inspector_heading(SharedString::new());
        ui.set_inspector_voices(ModelRc::default());
        return;
    };
    ui.set_inspector_heading(v.heading.into());
    ui.set_inspector_unit(v.unit.into());
    ui.set_inspector_caret(v.caret.into());
    let rows: Vec<InspectorVoice> = v
        .voices
        .into_iter()
        .map(|r| InspectorVoice {
            name: r.name.into(),
            count: r.count.into(),
            state: r.state.into(),
            fill: match r.fill {
                inspector::Fill::Full => 0,
                inspector::Fill::Short => 1,
                inspector::Fill::Long => 2,
                inspector::Fill::Unknown => 3,
            },
            entered: i32::try_from(r.entered).unwrap_or(i32::MAX),
            current: r.current,
        })
        .collect();
    ui.set_inspector_voices(ModelRc::new(VecModel::from(rows)));
}

/// Edit → Go to Section lists the score's sections.
fn sync_sections(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let names = app
        .borrow()
        .editing
        .sections()
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    ui.set_sections(strings(names));
}

/// A track command from its row's menu (or the Edit menu), on the caret's
/// track.
fn track_action(app: &Shared, action: &str) {
    use oxiguitar_edit::Command;
    let command = match action {
        "properties" => return open_track_dialog(app),
        "transpose" => return open_transpose(app, oxiguitar_edit::Scope::Track),
        "add" => Command::AddTrack,
        "duplicate" => Command::DuplicateTrack,
        "up" => Command::MoveTrackUp,
        "down" => Command::MoveTrackDown,
        "delete" => {
            let typed = app.borrow_mut().editing.tool(Command::DeleteTrack);
            let name = {
                let s = app.borrow();
                let score = s.editing.score();
                let t = s.editing.track().unwrap_or(0);
                score
                    .and_then(|sc| sc.tracks.get(t).map(|t| t.name.clone()))
                    .unwrap_or_default()
            };
            if let Some(t) = typed {
                ask(
                    app,
                    Asked::DeleteTrack(t),
                    &format!("Delete the track {name}?"),
                    &["Its notes go with it. Undo brings it back.".to_owned()],
                    &["Delete", "Cancel"],
                );
            }
            return;
        }
        _ => return,
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// Opens the track dialog on the caret's track.
fn open_track_dialog(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() {
        return;
    }
    let (score, track) = {
        let s = app.borrow();
        (s.editing.score(), s.editing.track())
    };
    let (Some(score), Some(t)) = (score, track) else {
        return;
    };
    let Some(track) = score.tracks.get(t) else {
        return;
    };
    let f = track_form::TrackForm::of(track);
    ui.set_track_heading(format!("Track {}: {}", t + 1, track.name).into());
    ui.set_track_fields(track_fields(&f));
    ui.set_track_preview(f.preview().into());
    ui.set_track_invalid(false);
    {
        let mut s = app.borrow_mut();
        s.dialogs_opened += 1;
        let sections = track_sections::TrackSections::open(&s.plugins, s.dialogs_opened, t, track);
        if let Some(thread) = &s.tab_thread {
            for ask in sections.asks() {
                thread.send(ask);
            }
        }
        s.sections = Some(sections);
    }
    sync_track_sections(app);
    ui.set_track_dialog_shown(true);
}

/// The track dialog's plugin sections, as they stand.
fn sync_track_sections(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let sections: Vec<PluginSection> = s
        .sections
        .iter()
        .flat_map(|t| &t.sections)
        .map(|shown| {
            let (rows, message) = match &shown.content {
                None => (ModelRc::default(), "…".to_owned()),
                Some(Ok(content)) => (plain_rows(content), String::new()),
                Some(Err(why)) => (ModelRc::default(), why.clone()),
            };
            PluginSection {
                title: shown.title.as_str().into(),
                rows,
                message: message.into(),
            }
        })
        .collect();
    ui.set_track_sections(ModelRc::new(VecModel::from(sections)));
}

/// A plugin's rows without canvases, as settings windows and track
/// sections show them: rebuilt whole each time (they are small, and their
/// widgets answer only when let go).
fn plain_rows(content: &TabContent) -> ModelRc<PluginRow> {
    let none = HashMap::new();
    ModelRc::new(VecModel::from(
        content
            .rows
            .iter()
            .map(|r| PluginRow {
                widgets: ModelRc::new(VecModel::from(
                    r.widgets
                        .iter()
                        .map(|w| plugin_widget(w, &none))
                        .collect::<Vec<_>>(),
                )),
                stretch: r.widgets.iter().any(|w| matches!(w, Widget::List { .. })),
            })
            .collect::<Vec<_>>(),
    ))
}

/// What a use of a widget in the window's plugin callbacks comes to: the
/// kind (0 clicked, 1 toggled, 2 slid, 3 edited, 4 chosen) and its value.
fn widget_event(kind: i32, on: bool, value: f32, text: &str, chosen: i32) -> TabEvent {
    match kind {
        1 => TabEvent::Toggled(on),
        2 => TabEvent::Slid(value),
        3 => TabEvent::Edited(text.to_owned()),
        4 => TabEvent::Chosen(u32::try_from(chosen).unwrap_or(0)),
        _ => TabEvent::Clicked,
    }
}

/// A widget of the track dialog's section `index` was used.
fn track_section_event(app: &Shared, index: i32, widget: &str, event: TabEvent) {
    let s = app.borrow();
    let (Some(sections), Some(thread)) = (&s.sections, &s.tab_thread) else {
        return;
    };
    if let Some(ask) = usize::try_from(index)
        .ok()
        .and_then(|i| sections.ask(i, Some((widget.to_owned(), event))))
    {
        thread.send(ask);
    }
}

/// The plugin id of the manager's row `index`, when it has a settings
/// window.
fn app_rows_settings(app: &Shared, index: i32) -> Option<String> {
    let s = app.borrow();
    usize::try_from(index)
        .ok()
        .and_then(|i| {
            manager::rows(&s.plugins, &manager::disabled(&s.stored))
                .into_iter()
                .nth(i)
        })
        .and_then(|r| r.settings)
}

/// Opens the settings window of the plugin `id` (interface 1.3), made the
/// first time; it shows one plugin at a time.
fn open_settings_window(app: &Shared, id: String) {
    let existing = app
        .borrow()
        .settings_window
        .as_ref()
        .map(|(w, _)| w.clone_strong());
    let window = match existing {
        Some(w) => w,
        None => {
            let Ok(w) = PluginSettingsWindow::new() else {
                message(
                    app,
                    Level::Error,
                    "The plugin's settings window couldn't be opened.",
                );
                return;
            };
            let a = app.clone();
            w.on_clicked(move |widget| settings_event(&a, &widget, TabEvent::Clicked));
            let a = app.clone();
            w.on_toggled(move |widget, on| settings_event(&a, &widget, TabEvent::Toggled(on)));
            let a = app.clone();
            w.on_slid(move |widget, v| settings_event(&a, &widget, TabEvent::Slid(v)));
            let a = app.clone();
            w.on_edited(move |widget, t| {
                settings_event(&a, &widget, TabEvent::Edited(t.to_string()));
            });
            let a = app.clone();
            w.on_chosen(move |widget, n| {
                settings_event(&a, &widget, TabEvent::Chosen(u32::try_from(n).unwrap_or(0)));
            });
            let weak = w.as_weak();
            w.on_close_clicked(move || {
                if let Some(w) = weak.upgrade() {
                    let _ = w.hide();
                }
            });
            w
        }
    };
    let name = {
        let s = app.borrow();
        s.plugins
            .found()
            .iter()
            .find(|f| f.manifest.as_ref().is_some_and(|m| m.id == id))
            .map(oxiguitar_host::Found::name)
            .unwrap_or_else(|| id.clone())
    };
    window.set_window_title(format!("{name} Settings").into());
    window.set_dark_interface(app.borrow().prefs.dark_interface);
    window.set_rows(ModelRc::default());
    window.set_message("…".into());
    {
        let mut s = app.borrow_mut();
        if let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Settings {
                plugin: id.clone(),
                event: None,
            });
        }
        s.settings_window = Some((window.clone_strong(), id));
    }
    let _ = window.show();
}

/// A widget of the settings window was used.
fn settings_event(app: &Shared, widget: &str, event: TabEvent) {
    let s = app.borrow();
    if let (Some((_, plugin)), Some(thread)) = (&s.settings_window, &s.tab_thread) {
        thread.send(plugin_tabs::Request::Settings {
            plugin: plugin.clone(),
            event: Some((widget.to_owned(), event)),
        });
    }
}

/// The tab thread answered for a settings window.
fn settings_window_answer(app: &Shared, plugin: &str, content: Result<TabContent, String>) {
    let s = app.borrow();
    let Some((window, shown)) = &s.settings_window else {
        return;
    };
    if shown != plugin {
        return;
    }
    match content {
        Ok(content) => {
            window.set_rows(plain_rows(&content));
            window.set_message(SharedString::new());
        }
        Err(why) => {
            window.set_rows(ModelRc::default());
            window.set_message(why.into());
        }
    }
}

fn track_fields(f: &track_form::TrackForm) -> TrackFields {
    TrackFields {
        name: f.name.as_str().into(),
        short_name: f.short_name.as_str().into(),
        instrument: f.instrument,
        strings: f.strings,
        key: f.key,
        pattern: f.pattern,
        notes: f.notes.as_str().into(),
        capo: f.capo,
        frets: f.frets,
        keep_pitches: f.keep_pitches,
    }
}

fn track_form(f: &TrackFields) -> track_form::TrackForm {
    track_form::TrackForm {
        plugins: std::collections::BTreeMap::new(),
        name: f.name.to_string(),
        short_name: f.short_name.to_string(),
        instrument: f.instrument,
        strings: f.strings,
        key: f.key,
        pattern: f.pattern,
        notes: f.notes.to_string(),
        capo: f.capo,
        frets: f.frets,
        keep_pitches: f.keep_pitches,
    }
}

/// A field of the track dialog changed: what the tuning comes to.
fn track_form_changed(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let f = track_form(&ui.get_track_fields());
    ui.set_track_invalid(f.to_settings().is_err());
    ui.set_track_preview(
        f.to_settings()
            .err()
            .filter(|e| e.starts_with("A track"))
            .unwrap_or_else(|| f.preview())
            .into(),
    );
}

/// The track dialog closed: on OK, its settings as one change, after a
/// question when notes can't be kept.
fn track_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if !ok {
        app.borrow_mut().sections = None;
        ui.set_track_dialog_shown(false);
        ui.invoke_focus_page();
        return;
    }
    let mut form = track_form(&ui.get_track_fields());
    // The plugins' sections as the dialog holds them; without any, the
    // track's own stay.
    form.plugins = {
        let s = app.borrow();
        match (&s.sections, s.editing.score(), s.editing.track()) {
            (Some(sections), _, _) => sections.settings.clone(),
            (None, Some(score), Some(t)) => score
                .tracks
                .get(t)
                .map(|tr| tr.plugins.clone())
                .unwrap_or_default(),
            _ => Default::default(),
        }
    };
    let settings = match form.to_settings() {
        Ok(s) => s,
        Err(e) => {
            // The dialog stays open, saying why.
            ui.set_track_preview(e.into());
            ui.set_track_invalid(true);
            return;
        }
    };
    ui.set_track_dialog_shown(false);
    ui.invoke_focus_page();
    app.borrow_mut().sections = None;
    let (score, track) = {
        let s = app.borrow();
        (s.editing.score(), s.editing.track())
    };
    let lost = match (&score, track) {
        (Some(sc), Some(t)) => oxiguitar_edit::notes_lost(sc, t, &settings),
        _ => 0,
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::TrackProperties(Box::new(settings)));
    let Some(t) = typed else {
        return;
    };
    if lost > 0 {
        ask(
            app,
            Asked::Apply(t),
            &format!(
                "{lost} {} can't be kept",
                if lost == 1 { "note" } else { "notes" }
            ),
            &["No string can play them with these strings and capo, so they are left out. Undo brings them back.".to_owned()],
            &["Apply", "Cancel"],
        );
    } else {
        send_typed(app, t);
    }
}

/// Opens Transpose… for the caret's track, the selection or every track.
fn open_transpose(app: &Shared, scope: oxiguitar_edit::Scope) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() || app.borrow().editing.track().is_none() {
        return;
    }
    let heading = match scope {
        oxiguitar_edit::Scope::Track => {
            let s = app.borrow();
            let name = s
                .editing
                .score()
                .zip(s.editing.track())
                .and_then(|(sc, t)| sc.tracks.get(t).map(|t| t.name.clone()))
                .unwrap_or_default();
            format!("Transpose {name}")
        }
        oxiguitar_edit::Scope::Selection => {
            if app.borrow().editing.selection().is_some() {
                "Transpose the selection".to_owned()
            } else {
                "Transpose the beat".to_owned()
            }
        }
        oxiguitar_edit::Scope::All => "Transpose every track".to_owned(),
    };
    app.borrow_mut().transpose_scope = Some(scope);
    ui.set_transpose_heading(heading.into());
    ui.set_transpose_amount(0);
    ui.set_transpose_dialog_shown(true);
}

/// Transpose… closed: on OK, the transposition as one change, after a
/// question when notes can't be kept.
fn transpose_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_transpose_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(scope) = app.borrow_mut().transpose_scope.take() else {
        return;
    };
    if !ok {
        return;
    }
    let Some(t) = track_form::transposition(
        ui.get_transpose_amount(),
        ui.get_transpose_steps(),
        ui.get_transpose_keep(),
        scope,
    ) else {
        return;
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Transpose(t));
    let Some(typed) = typed else {
        return;
    };
    let lost = app.borrow().editing.score().map_or(0, |sc| {
        oxiguitar_edit::notes_lost_transposing(&sc, typed.caret, typed.selection, &t)
    });
    if lost > 0 {
        ask(
            app,
            Asked::Apply(typed),
            &format!(
                "{lost} {} can't be kept",
                if lost == 1 { "note" } else { "notes" }
            ),
            &["No string can play them at their new pitch, so they are left out. Undo brings them back.".to_owned()],
            &["Apply", "Cancel"],
        );
    } else {
        send_typed(app, typed);
    }
}

/// Copy, cut, paste, or Special Paste's dialog.
fn clipboard(app: &Shared, key: editing::ClipKey) {
    let typed = match key {
        editing::ClipKey::Copy => {
            app.borrow_mut().editing.copy();
            return;
        }
        editing::ClipKey::Cut => app.borrow_mut().editing.cut(),
        editing::ClipKey::Paste => app.borrow_mut().editing.paste(1, false),
        editing::ClipKey::Special => {
            let Some(ui) = app.borrow().ui.upgrade() else {
                return;
            };
            if app.borrow().editing.has_clip() && app.borrow().question.is_none() {
                ui.set_paste_dialog_shown(true);
            }
            return;
        }
    };
    if let Some(t) = typed {
        send_typed(app, t);
        sync_caret(app, false);
    }
}

/// Special Paste closed: on OK, the clipboard pasted as asked.
fn paste_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_paste_dialog_shown(false);
    ui.invoke_focus_page();
    if !ok {
        return;
    }
    let times = u8::try_from(ui.get_paste_times().clamp(1, 99)).unwrap_or(1);
    let typed = app
        .borrow_mut()
        .editing
        .paste(times, ui.get_paste_replace());
    if let Some(t) = typed {
        send_typed(app, t);
        sync_caret(app, false);
    }
}

/// Custom N-tuplet closed: on OK, the beats go into that tuplet.
fn tuplet_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_tuplet_dialog_shown(false);
    ui.invoke_focus_page();
    if !ok {
        return;
    }
    let Some(t) = bar_form::tuplet(ui.get_tuplet_notes(), ui.get_tuplet_in_time_of()) else {
        return;
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Tuplet(t));
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The user answered the question: button `i`, the last being Cancel.
fn question_answered(app: &Shared, i: i32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_question_shown(false);
    ui.invoke_focus_page();
    let Some(asked) = app.borrow_mut().question.take() else {
        return;
    };
    match (asked, i) {
        (Asked::Save { .. }, 0) => app.borrow().worker.send(Request::ConfirmSave),
        (Asked::Save { elsewhere: true }, 1) => {
            // What waits on the save keeps waiting, for the file chosen.
            app.borrow().worker.send(Request::CancelSave);
            save_as_dialog(app);
        }
        (Asked::Save { .. }, _) => {
            let mut s = app.borrow_mut();
            s.after_save = None;
            s.worker.send(Request::CancelSave);
        }
        (Asked::Unsaved(then), 0) => {
            app.borrow_mut().after_save = Some(then);
            save(app);
        }
        (Asked::Unsaved(then), 1) => go_on(app, then),
        (Asked::Unsaved(_), _) => {}
        (Asked::Apply(t) | Asked::DeleteTrack(t), 0) => send_typed(app, t),
        (Asked::Apply(_) | Asked::DeleteTrack(_), _) => {}
    }
}

/// File → Save: the file the score came from, in its own format unless
/// that loses part of the score (the worker asks then).
fn save(app: &Shared) {
    // A new score has no file yet: Save asks for a name, as Save As.
    if app.borrow().untitled {
        save_as_dialog(app);
        return;
    }
    app.borrow().worker.send(Request::Save(None));
}

/// File → Save As…: any name, in a format the plugins write, chosen by the
/// name's extension (Guitar Pro 7–8's .gp when it has none).
fn save_as_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        app.borrow_mut().after_save = None;
        message(app, Level::Warning, problem);
        return;
    }
    let (folder, stem, formats, sender) = {
        let s = app.borrow();
        let stem = s
            .doc_name
            .as_deref()
            .map(|n| {
                std::path::Path::new(n)
                    .file_stem()
                    .map_or_else(|| n.to_owned(), |x| x.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "score".to_owned());
        (
            s.folder.clone(),
            stem,
            s.save_formats.clone(),
            s.worker.sender(),
        )
    };
    if formats.is_empty() {
        app.borrow_mut().after_save = None;
        message(
            app,
            Level::Error,
            "No installed plugin writes files, so nothing can be saved.".to_owned(),
        );
        return;
    }
    // The fullest format first, as the default.
    let mut formats = formats;
    formats.sort_by_key(|f| f.id != FULLEST);
    let pick = move || {
        let first = formats.first().and_then(|f| f.extensions.first()).cloned();
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save As")
            .set_file_name(format!("{stem}.{}", first.as_deref().unwrap_or("gp")));
        for f in &formats {
            dialog = dialog.add_filter(&f.name, &f.extensions);
        }
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        let Some(mut path) = dialog.save_file() else {
            // Nothing waits on a save that won't happen (quitting after
            // "Save the changes…?", say).
            let _ = slint::invoke_from_event_loop(|| {
                if let Some(app) = APP.with(|a| a.borrow().clone()) {
                    app.borrow_mut().after_save = None;
                }
            });
            return;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let format = ext
            .as_deref()
            .and_then(|e| {
                formats
                    .iter()
                    .find(|f| f.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
            })
            .or(formats.first());
        let Some(format) = format else {
            return;
        };
        if !format
            .extensions
            .iter()
            .any(|x| ext.as_deref().is_some_and(|e| x.eq_ignore_ascii_case(e)))
            && let Some(e) = format.extensions.first()
        {
            path.set_extension(e);
        }
        let _ = sender.send(Request::Save(Some((path, format.id.clone()))));
    };
    run_dialog(pick);
}

/// File → Open: the system's dialog, off the UI thread where the system
/// allows it, so the window keeps drawing.
fn open_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        message(app, Level::Warning, problem);
        return;
    }
    let folder = app.borrow().folder.clone();
    let extensions = app.borrow().open_extensions.clone();
    let sender = app.borrow().worker.sender();
    let pick = move || {
        let mut dialog = rfd::FileDialog::new().set_title("Open");
        // The formats the installed plugins read; with none, the dialog
        // still opens, and the footer says why nothing can be read.
        if !extensions.is_empty() {
            dialog = dialog.add_filter("Scores", &extensions);
        }
        dialog = dialog.add_filter("All files", &["*"]);
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
/// - The wheel over the page scrolls it (along the line when a Horizontal
///   score fits the window's height); with Shift it zooms the page around
///   the pointer (Mateus, 18 Oct 2026). Elsewhere (a list in a side area)
///   the wheel scrolls as usual.
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
                    // Once this event is done with the app: unsaved
                    // changes are asked about first.
                    let (a2, path) = (a.clone(), path.clone());
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        unless_unsaved(&a2, Then::Open(path));
                    });
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
            // Tiles and plugins' canvases are drawn for device pixels: draw
            // them again.
            resend_canvas_rooms(&a, scale);
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
    fn each_linux_build_names_its_own_display_system() {
        let linux = cfg!(not(any(target_os = "windows", target_vendor = "apple")));
        let one = cfg!(feature = "wayland") != cfg!(feature = "x11");
        if linux && one {
            let own = if cfg!(feature = "wayland") {
                "wayland"
            } else {
                "x11"
            };
            assert_eq!(display_system(), own);
            assert!(wrong_build_hint(own).unwrap().contains(&format!(
                "OxiGuitar's {} build",
                if own == "x11" { "X11" } else { "Wayland" }
            )));
        } else {
            assert_eq!(display_system(), oxiguitar_host::display_system());
            assert!(wrong_build_hint("wayland").is_none());
        }
    }

    #[test]
    fn interface_zoom_steps() {
        assert_eq!(step_ui_zoom(1.0, 1), 1.1);
        assert_eq!(step_ui_zoom(1.0, -2), 0.8);
        assert_eq!(step_ui_zoom(2.0, 5), 2.0);
        assert_eq!(step_ui_zoom(0.5, -1), 0.5);
    }
}
