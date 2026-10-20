// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugins' tabs in the window (docs/design/plugins.md, "Tabs and events").
//! No window code here, so it is unit-tested.
//!
//! The window never calls a plugin itself: a thread of its own asks the
//! plugins for their tabs' contents, tells them what the user did, and
//! draws their canvases, so a slow plugin (or one loading for the first
//! time) never holds the window up. The thread sleeps until the window
//! asks for something. Requests already waiting when it wakes are merged:
//! a canvas dragged on faster than its plugin answers gets the latest
//! position, and each canvas is drawn once, at its latest size.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use oxiguitar_display::{FontSet, Rgba, Theme};
use oxiguitar_host::api::tab::{Area, PointerKind, TabContent, TabEvent, Widget};
use oxiguitar_host::{Plugins, TabId};
use oxiguitar_render::{Raster, View};

use crate::shell::{Shell, Side, Tab};

/// The largest side, in pixels, a canvas is drawn at.
pub(crate) const MAX_CANVAS_PIXELS: u32 = 4096;

/// The shell's id for a plugin's tab.
pub(crate) fn shell_id(tab: &TabId) -> String {
    format!("plugin:{}/{}", tab.plugin, tab.tab)
}

/// The plugin tab a shell id names, if it names one.
pub(crate) fn tab_of(shell_id: &str) -> Option<TabId> {
    let (plugin, tab) = shell_id.strip_prefix("plugin:")?.split_once('/')?;
    Some(TabId {
        plugin: plugin.to_owned(),
        tab: tab.to_owned(),
    })
}

/// The window's side for a manifest's area.
pub(crate) fn side_of(area: Area) -> Side {
    match area {
        Area::Top => Side::Top,
        Area::Bottom => Side::Bottom,
        Area::Left => Side::Left,
        Area::Right => Side::Right,
    }
}

/// Adds every tab the usable plugins declare to its area, after the core's
/// own, in plugin order. Nothing is loaded: the names come from the
/// manifests.
pub(crate) fn add_to(shell: &mut Shell, plugins: &Plugins) {
    for tab in plugins.tabs() {
        shell.area_mut(side_of(tab.spec.area)).tabs.push(Tab {
            id: shell_id(&tab.id()),
            title: tab.spec.title.clone(),
        });
    }
}

/// Takes away every tab of the plugin `id`, which was withdrawn: as if it
/// had never been installed. Whether any went.
pub(crate) fn remove_from(shell: &mut Shell, id: &str) -> bool {
    let mut any = false;
    for side in Side::ALL {
        any |= shell
            .area_mut(side)
            .remove(|tab| tab_of(tab).is_some_and(|t| t.plugin == id));
    }
    any
}

/// The colours a canvas is drawn in: the score's ink, accent and muted
/// colours, on the panels' own colour, so a canvas follows Dark Interface.
pub(crate) fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            page: Rgba::rgb(0x2c, 0x2c, 0x2c),
            ..Theme::DARK
        }
    } else {
        Theme {
            page: Rgba::rgb(0xf3, 0xf3, 0xf3),
            ..Theme::LIGHT
        }
    }
}

/// The size, in whole pixels, that a canvas `width` × `height` units large
/// is drawn at to fill `room` (pixels) with its proportions: never empty,
/// never more than [`MAX_CANVAS_PIXELS`] a side.
pub(crate) fn fitted(width: f32, height: f32, room: (u32, u32)) -> (u32, u32, f32) {
    let room_w = room.0.clamp(1, MAX_CANVAS_PIXELS) as f32;
    let room_h = room.1.clamp(1, MAX_CANVAS_PIXELS) as f32;
    let scale = (room_w / width).min(room_h / height);
    let px = |units: f32| ((units * scale).round() as u32).clamp(1, MAX_CANVAS_PIXELS);
    (px(width), px(height), scale)
}

/// Draws the canvas `widget` of `content` at `room` pixels (see
/// [`fitted`]), as premultiplied RGBA on a transparent ground.
pub(crate) fn draw(
    content: &TabContent,
    widget: &str,
    room: (u32, u32),
    dark: bool,
    raster: &mut Raster,
    fonts: &FontSet,
) -> Option<Drawn> {
    let Some(Widget::Canvas(canvas)) = content.widget(widget) else {
        return None;
    };
    let list = oxiguitar_display::wire::canvas(canvas.width, canvas.height, &canvas.items, fonts);
    let (width, height, scale) = fitted(canvas.width, canvas.height, room);
    let view = View {
        scale,
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    raster.render_into(&mut pixmap, &list, fonts, &theme(dark), &view);
    Some(Drawn {
        widget: widget.to_owned(),
        width,
        height,
        rgba: pixmap.take(),
    })
}

/// What the window asks of the tab thread.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Request {
    /// The tab's contents.
    Content(TabId),
    /// The user used the tab's widget `widget`.
    Event {
        tab: TabId,
        widget: String,
        event: TabEvent,
    },
    /// The tab's canvas `widget` has `room` pixels to fill.
    Size {
        tab: TabId,
        widget: String,
        room: (u32, u32),
    },
    /// The interface became dark (`true`) or light: canvases are drawn
    /// again.
    Dark(bool),
}

/// A canvas drawn, premultiplied RGBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Drawn {
    pub(crate) widget: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// What the tab thread tells the window.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Answer {
    /// The tab's contents, or why it can't be shown, with its canvases
    /// drawn at their last known sizes.
    Content {
        tab: TabId,
        content: Result<TabContent, String>,
        canvases: Vec<Drawn>,
    },
    /// One canvas drawn again (a new size, or the theme changed).
    Canvas { tab: TabId, drawn: Drawn },
}

/// Merges the requests waiting: a drag on a canvas replaces the drag just
/// before it on the same canvas, a size replaces an earlier size of the
/// same canvas, and a tab's contents asked for twice in a row are asked for
/// once.
pub(crate) fn merge(requests: Vec<Request>) -> Vec<Request> {
    let mut out: Vec<Request> = Vec::with_capacity(requests.len());
    let drag = |r: &Request| match r {
        Request::Event {
            tab,
            widget,
            event: TabEvent::Pointer(p),
        } if p.kind == PointerKind::Drag => Some((tab.clone(), widget.clone())),
        _ => None,
    };
    for request in requests {
        if let Some(key) = drag(&request)
            && out.last().and_then(drag).as_ref() == Some(&key)
        {
            *out.last_mut().unwrap_or_else(|| unreachable!()) = request;
            continue;
        }
        if let Request::Size { tab, widget, .. } = &request {
            out.retain(
                |r| !matches!(r, Request::Size { tab: t, widget: w, .. } if t == tab && w == widget),
            );
        }
        if matches!(&request, Request::Content(_)) && out.last() == Some(&request) {
            continue;
        }
        out.push(request);
    }
    out
}

/// The tab thread and the way to reach it.
pub(crate) struct TabThread {
    requests: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}

impl TabThread {
    /// Starts the thread over `plugins`; `sink` receives its answers, on
    /// the thread.
    pub(crate) fn start(
        plugins: Plugins,
        dark: bool,
        sink: impl Fn(Answer) + Send + 'static,
    ) -> TabThread {
        let (requests, rx) = channel();
        let thread = std::thread::Builder::new()
            .name("oxiguitar-plugin-tabs".to_owned())
            .spawn(move || run(&rx, &plugins, dark, &sink))
            .ok();
        TabThread {
            requests: Some(requests),
            thread,
        }
    }

    /// Queues a request. Never blocks.
    pub(crate) fn send(&self, request: Request) {
        if let Some(r) = &self.requests {
            let _ = r.send(request);
        }
    }
}

impl Drop for TabThread {
    fn drop(&mut self) {
        // Closing the channel ends the thread once it has answered what it
        // was asked.
        self.requests = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(rx: &Receiver<Request>, plugins: &Plugins, mut dark: bool, sink: &dyn Fn(Answer)) {
    let fonts = FontSet::bundled();
    let mut raster = Raster::new();
    let mut contents: HashMap<TabId, TabContent> = HashMap::new();
    let mut rooms: HashMap<(TabId, String), (u32, u32)> = HashMap::new();
    // Sleeps until the window asks, then takes whatever else is waiting.
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        batch.extend(rx.try_iter());
        for request in merge(batch) {
            match request {
                Request::Content(tab) => {
                    let answer = plugins.tab_content(&tab);
                    answered(
                        &tab,
                        answer,
                        &mut contents,
                        &rooms,
                        dark,
                        &mut raster,
                        &fonts,
                        sink,
                    );
                }
                Request::Event { tab, widget, event } => {
                    let answer = plugins.tab_event(&tab, &widget, event);
                    answered(
                        &tab,
                        answer,
                        &mut contents,
                        &rooms,
                        dark,
                        &mut raster,
                        &fonts,
                        sink,
                    );
                }
                Request::Size { tab, widget, room } => {
                    rooms.insert((tab.clone(), widget.clone()), room);
                    if let Some(drawn) = contents
                        .get(&tab)
                        .and_then(|c| draw(c, &widget, room, dark, &mut raster, &fonts))
                    {
                        sink(Answer::Canvas { tab, drawn });
                    }
                }
                Request::Dark(now) if now != dark => {
                    dark = now;
                    for ((tab, widget), room) in &rooms {
                        if let Some(drawn) = contents
                            .get(tab)
                            .and_then(|c| draw(c, widget, *room, dark, &mut raster, &fonts))
                        {
                            sink(Answer::Canvas {
                                tab: tab.clone(),
                                drawn,
                            });
                        }
                    }
                }
                Request::Dark(_) => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn answered(
    tab: &TabId,
    answer: Result<TabContent, oxiguitar_host::api::message::Error>,
    contents: &mut HashMap<TabId, TabContent>,
    rooms: &HashMap<(TabId, String), (u32, u32)>,
    dark: bool,
    raster: &mut Raster,
    fonts: &FontSet,
    sink: &dyn Fn(Answer),
) {
    match answer {
        Ok(content) => {
            let canvases = rooms
                .iter()
                .filter(|((t, _), _)| t == tab)
                .filter_map(|((_, widget), room)| {
                    draw(&content, widget, *room, dark, raster, fonts)
                })
                .collect();
            contents.insert(tab.clone(), content.clone());
            sink(Answer::Content {
                tab: tab.clone(),
                content: Ok(content),
                canvases,
            });
        }
        Err(e) => sink(Answer::Content {
            tab: tab.clone(),
            content: Err(e.to_string()),
            canvases: Vec::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxiguitar_host::api::display::{Item, Paint, Point};
    use oxiguitar_host::api::tab::{Canvas, Pointer, PointerButton, Row};

    fn tab(name: &str) -> TabId {
        TabId {
            plugin: "oxiguitar.test".into(),
            tab: name.into(),
        }
    }

    #[test]
    fn shell_ids_name_the_plugin_and_the_tab() {
        let t = tab("panel");
        assert_eq!(shell_id(&t), "plugin:oxiguitar.test/panel");
        assert_eq!(tab_of(&shell_id(&t)), Some(t));
        assert_eq!(tab_of("tracks"), None);
        assert_eq!(tab_of("plugin:nothing"), None);
        assert_eq!(side_of(Area::Left), Side::Left);
    }

    #[test]
    fn a_withdrawn_plugin_s_tabs_go_and_the_rest_stay() {
        let mut shell = Shell::default();
        let add = |shell: &mut Shell, side: Side, plugin: &str, tab: &str| {
            shell.area_mut(side).tabs.push(Tab {
                id: shell_id(&TabId {
                    plugin: plugin.into(),
                    tab: tab.into(),
                }),
                title: tab.into(),
            });
        };
        add(&mut shell, Side::Bottom, "a.one", "x");
        add(&mut shell, Side::Bottom, "b.two", "y");
        add(&mut shell, Side::Right, "a.one", "z");
        let bottom = shell.area(Side::Bottom).tabs.len();
        // The other plugin's tab showing in the bottom area, the withdrawn
        // one's in the right area.
        shell.area_mut(Side::Bottom).active = bottom - 1;
        let right = shell.area(Side::Right).tabs.len();
        shell.area_mut(Side::Right).active = right - 1;
        assert!(remove_from(&mut shell, "a.one"));
        let ids =
            |side| -> Vec<String> { shell.area(side).tabs.iter().map(|t| t.id.clone()).collect() };
        assert_eq!(ids(Side::Bottom).len(), bottom - 1);
        assert_eq!(shell.area(Side::Bottom).active_id(), "plugin:b.two/y");
        assert!(ids(Side::Right).iter().all(|id| tab_of(id).is_none()));
        assert!(shell.area(Side::Right).active < right - 1);
        assert!(!remove_from(&mut shell, "a.one"));
    }

    #[test]
    fn canvases_fill_their_room_with_their_proportions() {
        assert_eq!(fitted(100.0, 50.0, (400, 400)), (400, 200, 4.0));
        assert_eq!(fitted(100.0, 50.0, (400, 100)), (200, 100, 2.0));
        let (w, h, _) = fitted(1.0, 100_000.0, (0, 0));
        assert_eq!((w, h), (1, 1));
        let (w, h, _) = fitted(10.0, 10.0, (100_000, 100_000));
        assert_eq!((w, h), (MAX_CANVAS_PIXELS, MAX_CANVAS_PIXELS));
    }

    fn drag(widget: &str, x: f32) -> Request {
        Request::Event {
            tab: tab("panel"),
            widget: widget.into(),
            event: TabEvent::Pointer(Pointer {
                kind: PointerKind::Drag,
                at: Point::new(x, 0.0),
                button: PointerButton::Primary,
            }),
        }
    }

    #[test]
    fn waiting_drags_and_sizes_merge_and_everything_else_keeps_its_order() {
        let size = |w| Request::Size {
            tab: tab("panel"),
            widget: "canvas".into(),
            room: (w, 10),
        };
        let content = Request::Content(tab("panel"));
        let click = Request::Event {
            tab: tab("panel"),
            widget: "click".into(),
            event: TabEvent::Clicked,
        };
        let merged = merge(vec![
            content.clone(),
            content.clone(),
            size(10),
            drag("canvas", 1.0),
            drag("canvas", 2.0),
            drag("other", 3.0),
            drag("canvas", 4.0),
            click.clone(),
            click.clone(),
            size(20),
        ]);
        assert_eq!(
            merged,
            [
                content,
                drag("canvas", 2.0),
                drag("other", 3.0),
                drag("canvas", 4.0),
                click.clone(),
                click,
                size(20),
            ]
        );
    }

    /// The tab thread over the test plugin: contents with the canvas drawn
    /// at its room, a burst of drags ending where the pointer let go, the
    /// canvas drawn again for a light interface.
    #[test]
    fn the_thread_asks_the_plugin_and_draws_its_canvas() {
        use std::time::Duration;
        let host = crate::test_support::tab_host();
        let (tx, rx) = channel();
        let thread = TabThread::start(host.plugins.clone(), true, move |a| {
            let _ = tx.send(a);
        });
        let panel = crate::test_support::panel();
        let next = || rx.recv_timeout(Duration::from_secs(30)).unwrap();
        thread.send(Request::Size {
            tab: panel.clone(),
            widget: "canvas".into(),
            room: (300, 300),
        });
        thread.send(Request::Content(panel.clone()));
        let Answer::Content {
            content, canvases, ..
        } = next()
        else {
            panic!("contents first");
        };
        assert!(content.unwrap().widget("canvas").is_some());
        // The test plugin's canvas is 100 × 50 units: 300 × 150 pixels.
        assert_eq!(
            canvases
                .iter()
                .map(|d| (d.widget.as_str(), d.width, d.height))
                .collect::<Vec<_>>(),
            [("canvas", 300, 150)]
        );

        let pointer = |kind, x| Request::Event {
            tab: panel.clone(),
            widget: "canvas".into(),
            event: TabEvent::Pointer(Pointer {
                kind,
                at: Point::new(x, 10.0),
                button: PointerButton::Primary,
            }),
        };
        thread.send(pointer(PointerKind::Down, 1.0));
        for x in 2..60 {
            thread.send(pointer(PointerKind::Drag, x as f32));
        }
        thread.send(pointer(PointerKind::Up, 60.0));
        let last = loop {
            let Answer::Content { content, .. } = next() else {
                continue;
            };
            let content = content.unwrap();
            let Some(Widget::Canvas(c)) = content.widget("canvas") else {
                panic!("no canvas");
            };
            if c.items
                .iter()
                .any(|i| matches!(i, Item::Text { text, .. } if text == "Up"))
            {
                break c.clone();
            }
        };
        assert!(last.items.contains(&Item::Rect {
            at: Point::new(58.0, 8.0),
            width: 4.0,
            height: 4.0,
            paint: Paint::Accent,
        }));

        thread.send(Request::Dark(false));
        assert!(matches!(next(), Answer::Canvas { drawn, .. } if drawn.width == 300));
    }

    #[test]
    fn a_canvas_is_drawn_in_the_interface_s_colours() {
        let content = TabContent {
            rows: vec![Row {
                widgets: vec![Widget::Canvas(Canvas {
                    id: "c".into(),
                    width: 10.0,
                    height: 5.0,
                    items: vec![Item::Rect {
                        at: Point::new(0.0, 0.0),
                        width: 5.0,
                        height: 5.0,
                        paint: Paint::Ink,
                    }],
                })],
            }],
        };
        let fonts = FontSet::bundled();
        let mut raster = Raster::new();
        let drawn = draw(&content, "c", (20, 20), true, &mut raster, &fonts).unwrap();
        assert_eq!((drawn.width, drawn.height), (20, 10));
        let px = |x: usize, y: usize| &drawn.rgba[(y * 20 + x) * 4..][..4];
        assert_eq!(
            px(2, 2),
            [0xe6, 0xe6, 0xe6, 0xff],
            "dark interface: light ink"
        );
        assert_eq!(px(15, 5), [0, 0, 0, 0], "a transparent ground");
        let light = draw(&content, "c", (20, 20), false, &mut raster, &fonts).unwrap();
        assert_eq!(&light.rgba[..4], [0x1a, 0x1a, 0x1a, 0xff]);
        assert!(draw(&content, "nothing", (20, 20), true, &mut raster, &fonts).is_none());
    }
}
