// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! GUI spike in iced 0.14: palette on the left, track list along the
//! bottom, the score canvas in the middle.

use std::cell::RefCell;

use iced::keyboard::{self, Key};
use iced::widget::{
    button, checkbox, column, container, image, responsive, row, scrollable, slider, text,
    text_input, tooltip,
};
use iced::{Element, Font, Length, Padding, Size, Subscription, Theme};
use spike_common::{BRAVURA_OTF, Canvas, Face, GROUPS, NOTO_SANS_REGULAR, PaletteState, TOOLS};

const MARGIN: f32 = 24.0;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("OxiGuitar GUI spike — iced")
        .font(NOTO_SANS_REGULAR)
        .font(BRAVURA_OTF)
        .default_font(Font::with_name("Noto Sans"))
        .window_size(Size::new(1280.0, 800.0))
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    Tool(usize),
    Key(keyboard::Event),
    Zoom(i32),
    Dark(bool),
    Search(String),
    Scrolled(scrollable::Viewport),
    TrackName(usize, String),
    Mute(usize, bool),
    Solo(usize, bool),
    Volume(usize, f32),
}

struct App {
    palette: PaletteState,
    tracks: Vec<spike_common::Track>,
    search: String,
    zoom: f32,
    dark: bool,
    offset: (f32, f32),
    /// iced's `view` takes `&self`; the canvas renders lazily and caches.
    canvas: RefCell<CanvasCache>,
}

struct CanvasCache {
    canvas: Canvas,
    key: Option<(i32, i32, u32, u32, u32, bool)>,
    image: Option<(image::Handle, f32, f32, f32, f32)>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            palette: PaletteState::default(),
            tracks: spike_common::tracks(),
            search: String::new(),
            zoom: 1.0,
            dark: false,
            offset: (0.0, 0.0),
            canvas: RefCell::new(CanvasCache {
                canvas: Canvas::new(),
                key: None,
                image: None,
            }),
        }
    }
}

impl App {
    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        // Only keys no widget used, so typing in a text field never fires
        // a palette shortcut.
        keyboard::listen().map(Message::Key)
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Tool(i) => self.palette.toggle(i),
            Message::Key(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => {
                if modifiers.command() {
                    match key.as_ref() {
                        Key::Character("+" | "=") => {
                            self.zoom = spike_common::step_zoom(self.zoom, 1)
                        }
                        Key::Character("-") => self.zoom = spike_common::step_zoom(self.zoom, -1),
                        _ => {}
                    }
                } else if let Some(c) = text.and_then(|t| t.chars().next()) {
                    self.palette.press(c);
                }
            }
            Message::Key(_) => {}
            Message::Zoom(d) => self.zoom = spike_common::step_zoom(self.zoom, d),
            Message::Dark(d) => self.dark = d,
            Message::Search(s) => self.search = s,
            Message::Scrolled(v) => {
                let o = v.absolute_offset();
                self.offset = (o.x, o.y);
            }
            Message::TrackName(i, s) => self.tracks[i].name = s,
            Message::Mute(i, b) => self.tracks[i].mute = b,
            Message::Solo(i, b) => self.tracks[i].solo = b,
            Message::Volume(i, v) => self.tracks[i].volume = v,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let toolbar = row![
            text("OxiGuitar").font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::with_name("Noto Sans")
            }),
            button("−").on_press(Message::Zoom(-1)),
            text(format!("{:.0} %", self.zoom * 100.0)).width(48),
            button("+").on_press(Message::Zoom(1)),
            checkbox(self.dark)
                .label("Dark page")
                .on_toggle(Message::Dark),
            text_input("Search commands…", &self.search)
                .on_input(Message::Search)
                .width(220),
            text(format!("Tool: {}", self.palette.summary())),
        ]
        .spacing(10)
        .padding(6)
        .align_y(iced::Alignment::Center);

        let palette = container(self.palette_view())
            .width(190)
            .height(Length::Fill)
            .padding(8);
        let canvas = responsive(move |size| self.score_view(size));
        let middle = row![palette, canvas].height(Length::Fill);

        column![toolbar, middle, self.tracks_view()].into()
    }

    fn tool_button(&self, i: usize) -> Element<'_, Message> {
        let tool = &TOOLS[i];
        let face: Element<'_, Message> = match tool.face {
            Face::Symbol(s) => text(s).font(Font::with_name("Bravura")).size(20).into(),
            Face::Text(s) => text(s).size(12).into(),
        };
        let b = button(container(face).center(36))
            .padding(0)
            .width(36)
            .height(36)
            .style(if self.palette.is_on(i) {
                button::primary
            } else {
                button::secondary
            })
            .on_press(Message::Tool(i));
        tooltip(
            b,
            text(format!("{} ({})", tool.name, tool.key.to_ascii_uppercase())).size(12),
            tooltip::Position::Right,
        )
        .style(container::rounded_box)
        .into()
    }

    fn palette_view(&self) -> Element<'_, Message> {
        let mut col = column![].spacing(4);
        let groups: Vec<(&str, Vec<usize>)> = if self.search.trim().is_empty() {
            GROUPS
                .iter()
                .map(|g| {
                    (
                        *g,
                        (0..TOOLS.len()).filter(|&i| TOOLS[i].group == *g).collect(),
                    )
                })
                .collect()
        } else {
            vec![("Commands", spike_common::search(&self.search))]
        };
        for (title, tools) in groups {
            col = col.push(text(title).size(11));
            // iced's row has no wrapping by itself here: four per row.
            for chunk in tools.chunks(4) {
                col = col.push(row(chunk.iter().map(|&i| self.tool_button(i))).spacing(4));
            }
            col = col.push(iced::widget::space().height(8));
        }
        col.into()
    }

    fn tracks_view(&self) -> Element<'_, Message> {
        let mut col = column![text("Tracks").size(11)].spacing(4);
        for (i, t) in self.tracks.iter().enumerate() {
            col = col.push(
                row![
                    text((i + 1).to_string()).width(20),
                    text_input("", &t.name)
                        .on_input(move |s| Message::TrackName(i, s))
                        .width(220),
                    text(t.instrument).width(130),
                    checkbox(t.mute)
                        .label("M")
                        .on_toggle(move |b| Message::Mute(i, b)),
                    checkbox(t.solo)
                        .label("S")
                        .on_toggle(move |b| Message::Solo(i, b)),
                    slider(0.0..=1.0, t.volume, move |v| Message::Volume(i, v))
                        .step(0.01_f32)
                        .width(120),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            );
        }
        container(scrollable(col)).height(170).padding(6).into()
    }

    /// The score canvas: render the visible part of the page and place it
    /// inside a scrollable of the page's full size.
    fn score_view(&self, size: Size) -> Element<'_, Message> {
        let mut cache = self.canvas.borrow_mut();
        let (pw, ph) = cache.canvas.page_pixels(self.zoom);
        let (vx, vy) = self.offset;
        let x0 = vx.max(MARGIN);
        let y0 = vy.max(MARGIN);
        let x1 = (vx + size.width).min(MARGIN + pw);
        let y1 = (vy + size.height).min(MARGIN + ph);
        // iced 0.14 offers no simple way to read the scale factor in `view`;
        // the spike renders at one pixel per logical pixel.
        let dpr = 1.0;
        if x1 - x0 >= 1.0 && y1 - y0 >= 1.0 {
            let pt_per_px = 72.0 / 96.0 / self.zoom;
            let w = ((x1 - x0) * dpr).round() as u32;
            let h = ((y1 - y0) * dpr).round() as u32;
            let key = (
                x0.round() as i32,
                y0.round() as i32,
                w,
                h,
                (self.zoom * 100.0) as u32,
                self.dark,
            );
            if cache.key != Some(key) {
                let f = cache.canvas.frame(
                    (x0 - MARGIN) * pt_per_px,
                    (y0 - MARGIN) * pt_per_px,
                    w,
                    h,
                    self.zoom,
                    dpr,
                    self.dark,
                );
                let handle = image::Handle::from_rgba(f.width, f.height, f.rgba);
                cache.image = Some((handle, x0, y0, x1 - x0, y1 - y0));
                cache.key = Some(key);
            }
        }
        let content: Element<'_, Message> = match &cache.image {
            Some((handle, x, y, w, h)) => container(
                image(handle.clone())
                    .width(*w)
                    .height(*h)
                    .filter_method(image::FilterMethod::Nearest),
            )
            .padding(Padding {
                top: *y,
                left: *x,
                right: 0.0,
                bottom: 0.0,
            })
            .width(pw + 2.0 * MARGIN)
            .height(ph + 2.0 * MARGIN)
            .into(),
            None => iced::widget::space()
                .width(pw + 2.0 * MARGIN)
                .height(ph + 2.0 * MARGIN)
                .into(),
        };
        let bg = if self.dark {
            iced::Color::from_rgb8(0x2a, 0x2b, 0x2e)
        } else {
            iced::Color::from_rgb8(0xd8, 0xd8, 0xd8)
        };
        container(
            scrollable(content)
                .direction(scrollable::Direction::Both {
                    vertical: Default::default(),
                    horizontal: Default::default(),
                })
                .on_scroll(Message::Scrolled),
        )
        .style(move |_| container::Style {
            background: Some(bg.into()),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
