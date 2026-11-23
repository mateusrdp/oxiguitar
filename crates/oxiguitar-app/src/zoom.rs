// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The canvas: the score's tiles for what is in view, retired when the zoom or the scale changes, and the score's zoom.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// The window's score area in points, for the Vertical layout.
pub(crate) fn screen_width(ui: &AppWindow, zoom: f32) -> f32 {
    ((ui.get_view_width() - CANVAS_PAD) / (PX_PER_PT * zoom)).max(150.0)
}

pub(crate) fn set_content_size(app: &Shared, ui: &AppWindow) {
    let s = app.borrow();
    let (w, h) = s.sheets.size();
    let px = PX_PER_PT * s.zoom;
    ui.set_content_width(w * px);
    ui.set_content_height(h * px);
}

/// The view moved or changed size: ask for the tiles it lacks, drop the
/// ones far away.
pub(crate) fn canvas_changed(app: &Shared) {
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
pub(crate) const MAX_OLD_TILES: usize = 96;

/// The tiles on screen no longer match the score (zoom, engraving, page
/// colour, scale): they move to the layer underneath, scaled by `k` around
/// the canvas origin, and new tiles are drawn over them.
pub(crate) fn retire_tiles(s: &mut App, k: f32) {
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
pub(crate) fn drop_old_tiles_when_covered(s: &App) {
    if s.old_tile_model.row_count() > 0 && s.visible.iter().all(|k| s.tiles.contains_key(k)) {
        s.old_tile_model.clear();
    }
}

/// The score-zoom slider moved: keep the middle of the view where it is.
pub(crate) fn zoom_moved(app: &Shared, v: f32) {
    // Snap to 100 % near the middle of the slider.
    let v = if v.abs() < 0.06 { 0.0 } else { v };
    set_zoom(app, v, None);
}

/// The mouse wheel turned over the page by `notches` (fractions from a
/// touchpad): zoom, keeping the point under the pointer where it is.
/// `(x, y)` is the pointer in the page's viewport, in logical pixels.
pub(crate) fn wheel_zoom(app: &Shared, notches: f32, x: f32, y: f32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let v = (ui.get_zoom_log() + notches * WHEEL_ZOOM_STEP).clamp(-2.0, 2.0);
    set_zoom(app, v, Some((x, y)));
}

/// Sets the score zoom to 2^`v`, keeping the canvas point at `anchor` (in
/// the viewport, logical pixels; the middle of the view if `None`) where it
/// is on screen.
pub(crate) fn set_zoom(app: &Shared, v: f32, anchor: Option<(f32, f32)>) {
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
        save_zooms(&mut s);
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
