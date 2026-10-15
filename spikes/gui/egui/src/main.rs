// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! GUI spike in egui (eframe 0.36): palette on the left, track list along
//! the bottom, the score canvas in the middle.

use std::sync::Arc;

use eframe::egui::{
    self, Button, CentralPanel, Color32, ColorImage, FontData, FontDefinitions, FontFamily, FontId,
    Panel, RichText, ScrollArea, Slider, TextEdit, TextureHandle, TextureOptions, Vec2,
};
use spike_common::{BRAVURA_OTF, Canvas, Face, GROUPS, NOTO_SANS_REGULAR, PaletteState, TOOLS};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("OxiGuitar GUI spike — egui"),
        ..Default::default()
    };
    eframe::run_native(
        "oxiguitar-spike-egui",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::default()))
        }),
    )
}

/// Noto Sans for the interface, Bravura as a named family for symbols.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        Arc::new(FontData::from_static(NOTO_SANS_REGULAR)),
    );
    fonts.font_data.insert(
        "bravura".into(),
        Arc::new(FontData::from_static(BRAVURA_OTF)),
    );
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "noto".into());
    }
    fonts
        .families
        .insert(FontFamily::Name("bravura".into()), vec!["bravura".into()]);
    ctx.set_fonts(fonts);
}

struct App {
    palette: PaletteState,
    tracks: Vec<spike_common::Track>,
    search: String,
    zoom: f32,
    dark: bool,
    canvas: Canvas,
    texture: Option<TextureHandle>,
    /// What the texture holds: x, y, width, height, zoom, dpr, dark.
    drawn: Option<(i32, i32, u32, u32, u32, u32, bool)>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            palette: PaletteState::default(),
            tracks: spike_common::tracks(),
            search: String::new(),
            zoom: 1.0,
            dark: false,
            canvas: Canvas::new(),
            texture: None,
            drawn: None,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);

        Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("OxiGuitar");
                ui.separator();
                if ui.button("−").on_hover_text("Zoom out (Ctrl+−)").clicked() {
                    self.zoom = spike_common::step_zoom(self.zoom, -1);
                }
                ui.label(format!("{:.0} %", self.zoom * 100.0));
                if ui.button("+").on_hover_text("Zoom in (Ctrl++)").clicked() {
                    self.zoom = spike_common::step_zoom(self.zoom, 1);
                }
                ui.checkbox(&mut self.dark, "Dark page");
                ui.separator();
                ui.add(
                    TextEdit::singleline(&mut self.search)
                        .hint_text("Search commands…")
                        .desired_width(220.0),
                );
                ui.separator();
                ui.label(format!("Tool: {}", self.palette.summary()));
            });
        });

        Panel::left("palette")
            .resizable(true)
            .default_size(190.0)
            .show(ui, |ui| {
                if self.search.trim().is_empty() {
                    for group in GROUPS {
                        ui.label(RichText::new(*group).small().strong());
                        ui.horizontal_wrapped(|ui| {
                            for (i, tool) in
                                TOOLS.iter().enumerate().filter(|(_, t)| t.group == *group)
                            {
                                self.tool_button(ui, i, tool);
                            }
                        });
                        ui.add_space(8.0);
                    }
                } else {
                    ui.label(RichText::new("Commands").small().strong());
                    for i in spike_common::search(&self.search) {
                        let tool = &TOOLS[i];
                        let label = format!("{}   [{}]", tool.name, tool.key.to_ascii_uppercase());
                        if ui
                            .add(Button::selectable(self.palette.is_on(i), label))
                            .clicked()
                        {
                            self.palette.toggle(i);
                        }
                    }
                }
            });

        Panel::bottom("tracks")
            .resizable(true)
            .default_size(170.0)
            .show(ui, |ui| {
                ui.label(RichText::new("Tracks").small().strong());
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("track-grid")
                        .striped(true)
                        .num_columns(6)
                        .show(ui, |ui| {
                            for h in ["#", "Name", "Instrument", "Mute", "Solo", "Volume"] {
                                ui.label(RichText::new(h).small());
                            }
                            ui.end_row();
                            for (i, t) in self.tracks.iter_mut().enumerate() {
                                ui.label((i + 1).to_string());
                                ui.add_sized([220.0, 20.0], TextEdit::singleline(&mut t.name));
                                ui.label(t.instrument);
                                ui.toggle_value(&mut t.mute, "M").on_hover_text("Mute");
                                ui.toggle_value(&mut t.solo, "S").on_hover_text("Solo");
                                ui.add(Slider::new(&mut t.volume, 0.0..=1.0).show_value(false));
                                ui.end_row();
                            }
                        });
                });
            });

        CentralPanel::default().show(ui, |ui| self.score(ui));
    }
}

impl App {
    fn tool_button(&mut self, ui: &mut egui::Ui, i: usize, tool: &spike_common::Tool) {
        // A small custom widget: egui sizes text rows by the font's line
        // metrics, and Bravura's are tall, so we centre the glyph ourselves.
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::click());
        let on = self.palette.is_on(i);
        let visuals = ui.style().interact_selectable(&resp, on);
        ui.painter().rect(
            rect,
            4.0,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let font = match tool.face {
            Face::Symbol(_) => FontId::new(20.0, FontFamily::Name("bravura".into())),
            Face::Text(_) => FontId::proportional(13.0),
        };
        let text = match tool.face {
            Face::Symbol(c) => c.to_string(),
            Face::Text(s) => s.to_owned(),
        };
        let galley = ui
            .painter()
            .layout_no_wrap(text, font, visuals.text_color());
        let ink = galley.mesh_bounds;
        let pos = rect.center() - ink.center().to_vec2();
        ui.painter().galley(pos, galley, visuals.text_color());
        let resp = resp.on_hover_text(format!("{} ({})", tool.name, tool.key.to_ascii_uppercase()));
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, tool.name)
        });
        if resp.clicked() {
            self.palette.toggle(i);
        }
    }

    /// Single-key tool shortcuts when no text field has focus; Ctrl +/− zoom.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Text(t) if !typing => {
                    for c in t.chars() {
                        self.palette.press(c);
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => match key {
                    egui::Key::Plus | egui::Key::Equals => {
                        self.zoom = spike_common::step_zoom(self.zoom, 1)
                    }
                    egui::Key::Minus => self.zoom = spike_common::step_zoom(self.zoom, -1),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    /// The score canvas: only the visible part is drawn, at the screen's
    /// pixel density.
    fn score(&mut self, ui: &mut egui::Ui) {
        let (pw, ph) = self.canvas.page_pixels(self.zoom);
        let bg = if self.dark {
            Color32::from_rgb(0x2a, 0x2b, 0x2e)
        } else {
            Color32::from_gray(0xd8)
        };
        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
        ScrollArea::both()
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let margin = 24.0;
                ui.set_min_size(Vec2::new(pw + 2.0 * margin, ph + 2.0 * margin));
                let origin = ui.min_rect().min + Vec2::splat(margin);
                let page_rect = egui::Rect::from_min_size(origin, Vec2::new(pw, ph));
                let visible = page_rect.intersect(egui::Rect::from_min_size(
                    ui.min_rect().min + viewport.min.to_vec2(),
                    viewport.size(),
                ));
                if visible.width() < 1.0 || visible.height() < 1.0 {
                    return;
                }
                let dpr = ui.ctx().pixels_per_point();
                let pt_per_px = 72.0 / 96.0 / self.zoom;
                let x = (visible.min.x - origin.x) * pt_per_px;
                let y = (visible.min.y - origin.y) * pt_per_px;
                let w = (visible.width() * dpr).round() as u32;
                let h = (visible.height() * dpr).round() as u32;
                let key = (
                    x.round() as i32,
                    y.round() as i32,
                    w,
                    h,
                    (self.zoom * 100.0) as u32,
                    (dpr * 100.0) as u32,
                    self.dark,
                );
                if self.drawn != Some(key) {
                    let f = self.canvas.frame(x, y, w, h, self.zoom, dpr, self.dark);
                    let img = ColorImage::from_rgba_premultiplied(
                        [f.width as usize, f.height as usize],
                        &f.rgba,
                    );
                    match &mut self.texture {
                        Some(t) => t.set(img, TextureOptions::NEAREST),
                        None => {
                            self.texture =
                                Some(ui.ctx().load_texture("score", img, TextureOptions::NEAREST))
                        }
                    }
                    self.drawn = Some(key);
                }
                if let Some(t) = &self.texture {
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(t.id(), visible, uv, Color32::WHITE);
                }
            });
    }
}
