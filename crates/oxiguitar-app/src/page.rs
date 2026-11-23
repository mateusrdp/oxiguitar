// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The page under the mouse and the caret: clicks, drags and double-clicks on the score, typing, and keeping the caret in view.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// A press on the page, at (`x`, `y`) in the viewport's logical pixels:
/// when playing, skip to the clicked note; when stopped, put the caret there.
pub(crate) fn page_pressed(app: &Shared, x: f32, y: f32, shift: bool) {
    // A click elsewhere ends typing lyrics, keeping what was typed.
    let typing = app
        .borrow()
        .ui
        .upgrade()
        .map(|ui| (ui.get_lyric_shown(), ui.get_lyric_text().to_string()));
    if let Some((true, text)) = typing {
        lyric_key(app, "done", &text);
    }
    // When playing: click on notation to skip to that note's time.
    let state = app.borrow().player.state();
    if matches!(
        state,
        crate::player::State::Playing | crate::player::State::Starting
    ) && let Some((sheet, sx, sy)) = on_sheet(app, x, y)
    {
        playing::notation_clicked(app, sheet, sx, sy);
        return;
    }
    // When stopped/paused: existing editing behavior (caret placement, selection).
    let moved = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.press_at(i, sx, sy, shift));
    if moved {
        caret_moved(app);
    }
    // On what a plugin drew: the plugin is told, after the caret moved.
    if !shift {
        drawing_clicked(app, x, y, false);
    }
}

/// Tells a plugin the user clicked what it drew at (`x`, `y`) of the page
/// view, if anything (interface 1.8). Returns whether there was a drawing.
pub(crate) fn drawing_clicked(app: &Shared, x: f32, y: f32, double: bool) -> bool {
    let Some((sheet, sx, sy)) = on_sheet(app, x, y) else {
        return false;
    };
    let Some(spot) = app.borrow().editing.hits().drawn_at(sheet, sx, sy) else {
        return false;
    };
    app.borrow().worker.send(Request::DrawingClicked {
        tag: spot.tag,
        at: spot.at,
        x: spot.x,
        y: spot.y,
        double,
    });
    true
}

/// A drag-start on the page: anchors the drag at the current caret bar.
pub(crate) fn page_drag_started(app: &Shared, _x: f32, _y: f32) {
    app.borrow_mut().editing.start_drag();
}

/// A drag-end on the page: no-op (selection is already set by drag_select).
pub(crate) fn page_drag_ended(_app: &Shared, _x: f32, _y: f32) {
    // Selection is already committed by the last drag_select call.
}

/// A right-click on the page, before its menu: the caret goes there unless
/// the click is inside the selection.
pub(crate) fn page_menu_pressed(app: &Shared, x: f32, y: f32) {
    let moved = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.menu_press_at(i, sx, sy));
    if moved {
        caret_moved(app);
    }
}

/// A double-click on the page: on a time signature, the caret goes to its
/// bar and the Bar tab shows (Mateus, 20 Oct 2026; the dialog became the
/// tab in issue #177).
pub(crate) fn page_double_clicked(app: &Shared, x: f32, y: f32) {
    if drawing_clicked(app, x, y, true) {
        return;
    }
    let on_time = on_sheet(app, x, y)
        .is_some_and(|(i, sx, sy)| app.borrow_mut().editing.time_signature_at(i, sx, sy));
    if on_time {
        caret_moved(app);
        show_tab(app, "bar");
    }
}

/// A point of the canvas, in logical pixels, as the sheet under it and the
/// point on that sheet, in points.
pub(crate) fn on_sheet(app: &Shared, x: f32, y: f32) -> Option<(usize, f32, f32)> {
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
pub(crate) fn send_typed(app: &Shared, t: editing::Typed) {
    app.borrow().worker.send(Request::Edit {
        command: t.command,
        caret: t.caret,
        selection: t.selection,
    });
}

pub(crate) fn caret_moved(app: &Shared) {
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

/// Multivoice on or off (Ctrl + M, the palette's button): every voice in
/// ink and the caret walking all of them, or back to the caret's voice.
pub(crate) fn toggle_multivoice(app: &Shared) {
    app.borrow_mut().editing.toggle_multivoice();
    caret_moved(app);
    if let Some(ui) = app.borrow().ui.upgrade() {
        set_palette(app, &ui, false);
    }
}

/// Design Mode's page menu: one bar more or less on the caret's system, or
/// the automatic layout from it on, for the shown track alone (its own
/// layout) or the shown tracks together (the score's).
pub(crate) fn design_layout(app: &Shared, more: Option<bool>) {
    let typed = {
        let mut s = app.borrow_mut();
        let Some(layout) = s.editing.system_layout(more) else {
            return;
        };
        let track = match s.settings.tracks.as_slice() {
            [one] => Some(*one),
            _ => None,
        };
        s.editing
            .tool(oxiguitar_edit::Command::SystemLayout { track, layout })
    };
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The page greys out the voices the caret isn't in: a new voice is a new
/// engraving (Mateus, 20 Oct 2026). In Multivoice every voice is in ink.
pub(crate) fn follow_voice(app: &Shared) {
    let mut s = app.borrow_mut();
    let Some(voice) = s.editing.at_caret().map(|a| a.voice) else {
        return;
    };
    let voice = (!s.editing.multivoice()).then_some(voice);
    if s.settings.active_voice != voice && s.track_count > 0 {
        s.settings.active_voice = voice;
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
}

/// Draws the caret and the selection over the page, highlights the
/// caret's track, and, when `scroll`, scrolls the caret into view.
pub(crate) fn sync_caret(app: &Shared, scroll: bool) {
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
