// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugins' tabs on screen: which tab each area shows, their widgets, canvases and surfaces drawn for device pixels, and plugin dialogs.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// The plugin tab area `side` shows, if it shows one: its tab is open and
/// the area is neither collapsed nor hidden.
pub(crate) fn plugin_tab_on_screen(s: &App, side: Side) -> Option<TabId> {
    let area = s.shell.area(side);
    if !area.shown || area.collapsed {
        return None;
    }
    plugin_tabs::tab_of(area.active_id())
}

/// Where a plugin's tab shows: one of the window's areas, the plugin
/// dialog over the window (interface 1.8), or the export dialog's options
/// (interface 1.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    Area(Side),
    Dialog,
    Export,
}

impl View {
    const ALL: [View; 6] = [
        View::Area(Side::Top),
        View::Area(Side::Bottom),
        View::Area(Side::Left),
        View::Area(Side::Right),
        View::Dialog,
        View::Export,
    ];

    /// The view for the window's number: an area's, 4 for the dialog, 5
    /// for the export dialog's options.
    fn from_index(i: i32) -> Option<View> {
        View::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub(crate) fn index(self) -> usize {
        match self {
            View::Area(side) => side.index(),
            View::Dialog => 4,
            View::Export => 5,
        }
    }
}

/// The plugin tab `view` shows now.
pub(crate) fn plugin_tab_at(s: &App, view: View) -> Option<TabId> {
    match view {
        View::Area(side) => plugin_tab_on_screen(s, side),
        View::Dialog => s.plugin_dialog.clone(),
        View::Export => s.export_options.clone(),
    }
}

/// Shows area `side`'s plugin tab, asking its plugin for the contents when
/// there are none yet or the plugin asked for a redraw while it was hidden.
pub(crate) fn show_plugin_tab(app: &Shared, side: Side) {
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
    refresh_plugin_view(app, View::Area(side), &tab);
}

/// The plugin asked for its tab to be shown again.
pub(crate) fn plugin_tab_changed(app: &Shared, tab: TabId) {
    let on_screen = View::ALL
        .iter()
        .any(|&view| plugin_tab_at(&app.borrow(), view).as_ref() == Some(&tab));
    let mut s = app.borrow_mut();
    if on_screen {
        if let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Content(tab));
        }
    } else {
        s.plugin_tabs.entry(tab).or_default().stale = true;
    }
}

/// Whether a plugin dialog is open: asked from the plugins' threads.
pub(crate) static DIALOG_OPEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Opens a plugin's dialog (a tab whose area is `dialog`, interface 1.8)
/// over the window and asks the plugin what it shows.
pub(crate) fn open_plugin_dialog(app: &Shared, tab: TabId, title: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
        return;
    };
    {
        let mut s = app.borrow_mut();
        // What an earlier opening showed is not what this one shows.
        s.plugin_tabs.remove(&tab);
        s.plugin_dialog = Some(tab.clone());
        if let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Content(tab.clone()));
        }
    }
    ui.set_plugin_dialog_title(title.into());
    refresh_plugin_view(app, View::Dialog, &tab);
    ui.set_plugin_dialog_shown(true);
}

/// The user closed the plugin dialog, with OK or not: its plugin is told.
pub(crate) fn plugin_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_plugin_dialog_shown(false);
    ui.invoke_focus_page();
    let mut s = app.borrow_mut();
    let view = &mut s.plugin_views[View::Dialog.index()];
    view.tab = None;
    view.content = None;
    if let Some(tab) = s.plugin_dialog.take() {
        s.plugin_tabs.remove(&tab);
        if let Some(thread) = &s.tab_thread {
            thread.send(plugin_tabs::Request::Closed { tab, ok });
        }
    }
    DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// A plugin asks for its tab at the front of its area (interface 1.8): the
/// area opens if it was closed.
pub(crate) fn plugin_tab_wanted(app: &Shared, tab: &TabId) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let id = plugin_tabs::shell_id(tab);
    let Some(side) = Side::ALL
        .iter()
        .copied()
        .find(|&side| app.borrow_mut().shell.area_mut(side).open(&id))
    else {
        return;
    };
    match side {
        Side::Top => ui.set_top_shown(true),
        Side::Bottom => ui.set_bottom_shown(true),
        Side::Left => ui.set_left_shown(true),
        Side::Right => ui.set_right_shown(true),
    }
    sync_areas(app);
}

/// What the tab thread answered.
pub(crate) fn plugin_answer(app: &Shared, answer: plugin_tabs::Answer) {
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
        plugin_tabs::Answer::Closed { tab } => {
            export_options_closed(app, &tab);
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
                apply_section_settings(app);
            }
            return;
        }
    };
    for view in View::ALL {
        if plugin_tab_at(&app.borrow(), view).as_ref() == Some(&tab) {
            refresh_plugin_view(app, view, &tab);
        }
    }
}

pub(crate) fn drawn_image(drawn: &plugin_tabs::Drawn) -> Image {
    let buf =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&drawn.rgba, drawn.width, drawn.height);
    Image::from_rgba8_premultiplied(buf)
}

/// How wide a plugin's drop-down is: room for its longest option at the
/// window's 12–13 px text (about 7 px a character) and the arrow, within
/// reason.
pub(crate) fn choice_width(options: &[String]) -> f32 {
    let longest = options.iter().map(|o| o.chars().count()).max().unwrap_or(0);
    (longest as f32 * 7.0 + 44.0).clamp(100.0, 440.0)
}

/// One plugin widget, as the window's `PluginTab` shows it.
pub(crate) fn plugin_widget(w: &Widget, canvases: &HashMap<String, Image>) -> PluginWidget {
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
            out.wide = choice_width(options);
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
        Widget::Link { text, url } => {
            out.kind = "link".into();
            out.text = text.as_str().into();
            out.text2 = url.as_str().into();
        }
        Widget::Surface(surface) => {
            out.kind = "surface".into();
            out.id = surface.id.as_str().into();
            out.canvas = canvases.get(&surface.id).cloned().unwrap_or_default();
            out.surface_height = surface.height;
            out.keys = surface.keys;
            out.cursor = match surface.cursor {
                Cursor::Default => 0,
                Cursor::Pointer => 1,
                Cursor::Text => 2,
                Cursor::Grab => 3,
                Cursor::Grabbing => 4,
                Cursor::NotAllowed => 5,
                Cursor::Wait => 6,
            };
        }
    }
    out
}

/// The kind and id of each widget of a row: rows of the same shape are
/// changed in place.
pub(crate) fn row_shape(
    row: &oxiguitar_host::api::tab::Row,
) -> Vec<(std::mem::Discriminant<Widget>, Option<&str>)> {
    row.widgets
        .iter()
        .map(|w| (std::mem::discriminant(w), w.id()))
        .collect()
}

/// Puts tab `tab`'s contents in area `side`. Contents of the same shape as
/// before are changed in place, widget by widget, so a drag on a canvas
/// carries on and an open list stays open; each widget follows its data
/// (plugin_tab.slint's `says`), so it shows exactly what the plugin says.
/// Contents of another shape are made anew.
pub(crate) fn refresh_plugin_view(app: &Shared, side: View, tab: &TabId) {
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
        row.widgets.iter().any(|w| match w {
            Widget::Canvas(_) | Widget::List { .. } => true,
            Widget::Surface(s) => s.height == 0.0,
            _ => false,
        })
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
        // Widget by widget, in the models the window already shows: only
        // what changed is set, so a list the user has open stays open and
        // keeps the keyboard (oxiguitar-midi#19), and a canvas's image may
        // be new even when its contents aren't. The shapes are the same, so
        // each row keeps its stretch.
        for (i, now) in content.rows.iter().enumerate() {
            let model = &view.row_models[i];
            for (j, w) in widgets(now).into_iter().enumerate() {
                if model.row_data(j).as_ref() != Some(&w) {
                    model.set_row_data(j, w);
                }
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

/// View `side`'s plugin tab: its rows, or a message instead.
pub(crate) fn set_plugin_area(
    ui: &AppWindow,
    side: View,
    rows: Option<ModelRc<PluginRow>>,
    message: &str,
) {
    let rows = rows.unwrap_or_default();
    let message = SharedString::from(message);
    let side = match side {
        View::Area(side) => side,
        View::Dialog => {
            ui.set_dialog_plugin_rows(rows);
            ui.set_dialog_plugin_message(message);
            return;
        }
        View::Export => {
            ui.set_export_plugin_rows(rows);
            ui.set_export_plugin_message(message);
            return;
        }
    };
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
pub(crate) fn plugin_event(app: &Shared, area: i32, widget: &str, event: TabEvent) {
    let s = app.borrow();
    let Some(tab) = View::from_index(area).and_then(|view| plugin_tab_at(&s, view)) else {
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
pub(crate) fn canvas_room(app: &Shared, area: i32, widget: &str, width: f32, height: f32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let scale = ui.window().scale_factor();
    let mut s = app.borrow_mut();
    let Some(tab) = View::from_index(area).and_then(|view| plugin_tab_at(&s, view)) else {
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
            scale,
        });
    }
}

/// A surface's room (interface 1.14), in logical pixels: drawn again for
/// the device pixels it covers and, when its size changed, its plugin
/// hears it to lay itself out again.
pub(crate) fn surface_room(app: &Shared, view: i32, widget: &str, width: f32, height: f32) {
    let Some(tab) = View::from_index(view).and_then(|v| plugin_tab_at(&app.borrow(), v)) else {
        return;
    };
    let before = app
        .borrow()
        .plugin_tabs
        .get(&tab)
        .and_then(|t| t.rooms.get(widget).copied());
    canvas_room(app, view, widget, width, height);
    if before != Some((width, height)) {
        plugin_event(app, view, widget, TabEvent::Resized { width, height });
    }
}

/// What the user did on a surface (interface 1.14), as PluginSurfaces
/// tells it.
pub(crate) fn install_surface_callbacks(ui: &AppWindow, app: &Shared) {
    let surfaces = ui.global::<PluginSurfaces<'_>>();
    let a = app.clone();
    surfaces.on_room(move |view, id, w, h| surface_room(&a, view, &id, w, h));
    let a = app.clone();
    surfaces.on_pointer(move |view, id, kind, x, y, button| {
        let kind = match kind {
            0 => PointerKind::Down,
            1 => PointerKind::Up,
            2 => PointerKind::Drag,
            3 => PointerKind::Move,
            _ => PointerKind::Leave,
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
        plugin_event(&a, view, &id, TabEvent::Pointer(pointer));
    });
    let a = app.clone();
    surfaces.on_wheel(move |view, id, x, y, dx, dy| {
        let at = CanvasPoint::new(x, y);
        plugin_event(&a, view, &id, TabEvent::Wheel { at, dx, dy });
    });
    let a = app.clone();
    surfaces.on_key(move |view, id, text, shift, ctrl, alt| {
        match plugin_tabs::surface_key(&text, shift, ctrl, alt) {
            Some(key) => {
                plugin_event(&a, view, &id, TabEvent::Key(key));
                true
            }
            None => false,
        }
    });
    let a = app.clone();
    surfaces.on_typed(move |view, id, text| {
        plugin_event(&a, view, &id, TabEvent::Typed(text.to_string()));
    });
    let a = app.clone();
    surfaces.on_focus(move |view, id, on| plugin_event(&a, view, &id, TabEvent::Focus(on)));
}

pub(crate) fn device_room(width: f32, height: f32, scale: f32) -> (u32, u32) {
    (
        (width * scale).round().max(1.0) as u32,
        (height * scale).round().max(1.0) as u32,
    )
}

/// The window's scale changed: every canvas is drawn again for its device
/// pixels.
pub(crate) fn resend_canvas_rooms(app: &Shared, scale: f32) {
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
                scale,
            });
        }
    }
}

/// The callbacks of every area's `PluginTab`.
pub(crate) fn install_plugin_tab_callbacks(ui: &AppWindow, app: &Shared) {
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
    let a = app.clone();
    ui.on_plugin_dialog_done(move |ok| plugin_dialog_done(&a, ok));
    install_surface_callbacks(ui, app);
}
