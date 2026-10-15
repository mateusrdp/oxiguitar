// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! GUI spike in Slint 1.18. The screen is declared in `ui/app.slint`; this
//! file feeds it the palette, the tracks and the rendered score.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use spike_common::{Canvas, Face, PaletteState, TOOLS};

slint::include_modules!();

struct State {
    palette: PaletteState,
    canvas: Canvas,
    zoom: f32,
    search: String,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let state = Rc::new(RefCell::new(State {
        palette: PaletteState::default(),
        canvas: Canvas::new(),
        zoom: 1.0,
        search: String::new(),
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
    refresh_palette(&ui, &state.borrow());
    set_page_size(&ui, &state.borrow());

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_tool_clicked(move |i| {
        let Some(ui) = weak.upgrade() else { return };
        st.borrow_mut().palette.toggle(i as usize);
        refresh_palette(&ui, &st.borrow());
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
            refresh_palette(&ui, &st.borrow());
        }
        hit
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_search_edited(move |text| {
        let Some(ui) = weak.upgrade() else { return };
        st.borrow_mut().search = text.to_string();
        refresh_palette(&ui, &st.borrow());
    });

    let weak = ui.as_weak();
    let st = state.clone();
    ui.on_zoom_step(move |dir| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut s = st.borrow_mut();
            s.zoom = spike_common::step_zoom(s.zoom, dir);
        }
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

    // First frame, once the window has its size.
    let weak = ui.as_weak();
    let st = state.clone();
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        if let Some(ui) = weak.upgrade() {
            render(&ui, &mut st.borrow_mut());
        }
    });

    ui.run()
}

fn tool_item(i: usize, on: bool) -> ToolItem {
    let t = &TOOLS[i];
    let (face, symbol) = match t.face {
        Face::Symbol(c) => (c.to_string(), true),
        Face::Text(s) => (s.to_owned(), false),
    };
    ToolItem {
        index: i as i32,
        name: t.name.into(),
        face: face.into(),
        symbol,
        key: t.key.to_ascii_uppercase().to_string().into(),
        group: t.group.into(),
        on,
    }
}

fn refresh_palette(ui: &AppWindow, s: &State) {
    let shown = spike_common::search(&s.search);
    let items = |group: Option<&str>| -> ModelRc<ToolItem> {
        let v: Vec<ToolItem> = shown
            .iter()
            .filter(|&&i| group.is_none_or(|g| TOOLS[i].group == g))
            .map(|&i| tool_item(i, s.palette.is_on(i)))
            .collect();
        ModelRc::new(VecModel::from(v))
    };
    if s.search.trim().is_empty() {
        ui.set_values(items(Some("Note values")));
        ui.set_effects(items(Some("Effects")));
    } else {
        ui.set_values(items(None));
        ui.set_effects(ModelRc::new(VecModel::<ToolItem>::default()));
    }
    ui.set_status(s.palette.summary().into());
}

fn set_page_size(ui: &AppWindow, s: &State) {
    let (w, h) = s.canvas.page_pixels(s.zoom);
    ui.set_page_width(w);
    ui.set_page_height(h);
    ui.set_zoom(s.zoom);
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
    let f = s.canvas.frame(
        (x0 - MARGIN) * pt_per_px,
        (y0 - MARGIN) * pt_per_px,
        w,
        h,
        s.zoom,
        dpr,
        ui.get_dark(),
    );
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&f.rgba, f.width, f.height);
    ui.set_score(Image::from_rgba8_premultiplied(buf));
    ui.set_score_x(x0);
    ui.set_score_y(y0);
    ui.set_score_width(x1 - x0);
    ui.set_score_height(y1 - y0);
}
