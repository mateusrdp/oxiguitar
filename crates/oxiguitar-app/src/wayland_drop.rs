// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! File drops on Wayland: a **stopgap** until winit or Slint report them
//! (ADR 0006; listed under "Unsafe code and stopgaps" in CONTRIBUTING.md).
//!
//! winit 0.30 reports a file dropped on the window as
//! `WindowEvent::DroppedFile` on Windows, macOS and X11 (handled in
//! `main.rs`), but its Wayland backend has no drag and drop, and Slint 1.18
//! passes no operating-system drops on. So on Wayland this module listens
//! for drops itself: a `wl_data_device` on the window's own Wayland
//! connection, accepting `text/uri-list` (what file managers offer) and
//! opening the first local file.
//!
//! It must use winit's connection, because the compositor only tells a
//! connection about the surfaces it created. winit lends that connection
//! only as a raw `wl_display` pointer, and wrapping it is the one `unsafe`
//! call in the app (see [`WaylandDrops::new`]).
//!
//! Its events are read from the socket by winit's event loop, which wakes
//! whenever the socket has data; [`WaylandDrops::dispatch`] handles the
//! ones queued for us and runs on a 100 ms Slint timer (`main.rs`).
//!
//! **To replace it:** once the winit (or Slint) version we use reports
//! drops on Wayland, delete this file, its `mod` line and timer in
//! `main.rs`, the `wayland-*` dependencies in the app's Cargo.toml, and mark
//! ADR 0006 superseded. Nothing else depends on it.

// The only unsafe code in the app: one call, explained at its use.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use wayland_backend::client::Backend;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};

/// The type file managers offer for files.
const URI_LIST: &str = "text/uri-list";
/// The most of a uri-list we read: thousands of paths.
const MAX_LIST: u64 = 1 << 20;

/// Drops on the window's Wayland connection.
///
/// **Drop it before Slint's event loop and window go away** (`main.rs` does
/// so right after `run` returns): it borrows winit's `wl_display`.
pub(crate) struct WaylandDrops {
    // Field order is drop order: our objects before the connection.
    state: State,
    queue: EventQueue<State>,
    conn: Connection,
    /// The connection failed (the compositor went away): do nothing more.
    failed: bool,
}

/// What the event handlers keep between events.
struct State {
    device: WlDataDevice,
    /// The offer of the drag over the window and the serial of its Enter.
    current: Option<(WlDataOffer, u32)>,
    /// Dropped offers whose list is being read, by number.
    reading: HashMap<u64, WlDataOffer>,
    next: u64,
    /// Lists read by the reader threads: (number, bytes).
    lists: Receiver<(u64, Vec<u8>)>,
    lists_tx: Sender<(u64, Vec<u8>)>,
    /// Called with the file to open.
    on_file: Box<dyn Fn(PathBuf)>,
}

/// The MIME types an offer says it can give.
#[derive(Default)]
struct OfferTypes(Mutex<Vec<String>>);

impl WaylandDrops {
    /// Starts listening for drops on `window`, if it is a Wayland window.
    /// Returns `None` on other systems, before the window exists, or if the
    /// compositor has no data device manager.
    pub(crate) fn new(window: &slint::Window, on_file: Box<dyn Fn(PathBuf)>) -> Option<Self> {
        let display = window
            .with_winit_window(|w| match w.display_handle().ok()?.as_raw() {
                RawDisplayHandle::Wayland(h) => Some(h.display),
                _ => None,
            })
            .flatten()?;
        // SAFETY: `display` is the `wl_display` of winit's own connection,
        // which Slint's winit backend keeps open until the process ends.
        // `from_foreign_display` asks that it outlive this backend and every
        // clone of it: the backend lives only in this `WaylandDrops`
        // (`Connection` and the queue hold its clones; the reader threads
        // get none), and `main.rs` drops it as soon as the event loop
        // returns, while the display is still open. It also never
        // disconnects the display: winit owns it.
        let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, queue) = registry_queue_init::<State>(&conn).ok()?;
        let qh = queue.handle();
        let seat: WlSeat = globals.bind(&qh, 1..=1, ()).ok()?;
        let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).ok()?;
        let device = manager.get_data_device(&seat, &qh, ());
        let (lists_tx, lists) = channel();
        conn.flush().ok()?;
        Some(WaylandDrops {
            state: State {
                device,
                current: None,
                reading: HashMap::new(),
                next: 0,
                lists,
                lists_tx,
                on_file,
            },
            queue,
            conn,
            failed: false,
        })
    }

    /// Handles the drag-and-drop events winit's event loop has read for us,
    /// and the lists the reader threads have finished. Call it often (the
    /// app: every 100 ms). After a connection error it does nothing.
    pub(crate) fn dispatch(&mut self) {
        if self.failed || self.queue.dispatch_pending(&mut self.state).is_err() {
            self.failed = true;
            return;
        }
        while let Ok((n, bytes)) = self.state.lists.try_recv() {
            if let Some(offer) = self.state.reading.remove(&n) {
                if offer.version() >= 3 {
                    offer.finish();
                }
                offer.destroy();
            }
            if let Some(path) = paths_from_uri_list(&bytes).into_iter().next() {
                (self.state.on_file)(path);
            }
        }
        self.failed = self.conn.flush().is_err();
    }
}

impl Drop for WaylandDrops {
    fn drop(&mut self) {
        if let Some((offer, _)) = self.state.current.take() {
            offer.destroy();
        }
        for (_, offer) in self.state.reading.drain() {
            offer.destroy();
        }
        if self.state.device.version() >= 2 {
            self.state.device.release();
        }
        let _ = self.conn.flush();
    }
}

impl State {
    /// The drop happened: ask the source for its uri-list through a pipe,
    /// read on a thread of its own (the source may be slow).
    fn receive(&mut self, offer: WlDataOffer) {
        let Ok((mut reader, writer)) = std::io::pipe() else {
            offer.destroy();
            return;
        };
        offer.receive(URI_LIST.to_owned(), writer.as_fd());
        drop(writer);
        let n = self.next;
        self.next += 1;
        self.reading.insert(n, offer);
        let tx = self.lists_tx.clone();
        let _ = std::thread::Builder::new()
            .name("oxiguitar-drop".to_owned())
            .spawn(move || {
                let mut bytes = Vec::new();
                let _ = (&mut reader).take(MAX_LIST).read_to_end(&mut bytes);
                let _ = tx.send((n, bytes));
            });
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter { serial, id, .. } => {
                if let Some((old, _)) = state.current.take() {
                    old.destroy();
                }
                let Some(offer) = id else { return };
                let files = offer
                    .data::<OfferTypes>()
                    .and_then(|t| t.0.lock().ok().map(|t| t.iter().any(|m| m == URI_LIST)))
                    .unwrap_or(false);
                if files {
                    offer.accept(serial, Some(URI_LIST.to_owned()));
                    if offer.version() >= 3 {
                        offer.set_actions(DndAction::Copy, DndAction::Copy);
                    }
                    state.current = Some((offer, serial));
                } else {
                    offer.accept(serial, None);
                    offer.destroy();
                }
            }
            wl_data_device::Event::Leave => {
                if let Some((offer, _)) = state.current.take() {
                    offer.destroy();
                }
            }
            wl_data_device::Event::Drop => {
                if let Some((offer, _)) = state.current.take() {
                    state.receive(offer);
                }
            }
            // The clipboard's offers: Slint has its own data device for
            // copy and paste, so ours are let go at once.
            wl_data_device::Event::Selection { id: Some(offer) } => offer.destroy(),
            _ => {}
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, OfferTypes::default()),
    ]);
}

impl Dispatch<WlDataOffer, OfferTypes> for State {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        types: &OfferTypes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event
            && let Ok(mut t) = types.0.lock()
        {
            t.push(mime_type);
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wayland_client::protocol::wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: wayland_client::protocol::wl_data_device_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// The local files in a `text/uri-list` (RFC 2483): one URI per line,
/// lines starting with `#` are comments. `file://` URIs with an empty host
/// or `localhost` become paths; anything else is skipped.
fn paths_from_uri_list(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with(b"#"))
        .filter_map(|line| {
            let rest = line.strip_prefix(b"file://")?;
            let path = rest.strip_prefix(b"localhost").unwrap_or(rest);
            path.starts_with(b"/").then(|| percent_decode(path))
        })
        .map(|raw| PathBuf::from(OsString::from_vec(raw)))
        .collect()
}

/// `%XX` escapes to bytes; anything malformed is kept as it is.
fn percent_decode(s: &[u8]) -> Vec<u8> {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match (
            s.get(i),
            s.get(i + 1).copied().and_then(hex),
            s.get(i + 2).copied().and_then(hex),
        ) {
            (Some(b'%'), Some(h), Some(l)) => {
                out.push(h << 4 | l);
                i += 3;
            }
            (Some(&b), _, _) => {
                out.push(b);
                i += 1;
            }
            _ => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_become_paths() {
        let list = b"# from a file manager\r\n\
            file:///home/m/Tarde%20em%20Itapu%C3%A3.gp\r\n\
            file://localhost/tmp/a.gp5\r\n\
            https://example.com/b.gp\r\n\
            file://otherhost/c.gp\r\n\
            file:///bad%zz.gp\n";
        assert_eq!(
            paths_from_uri_list(list),
            vec![
                PathBuf::from("/home/m/Tarde em Itapuã.gp"),
                PathBuf::from("/tmp/a.gp5"),
                PathBuf::from("/bad%zz.gp"),
            ]
        );
        assert!(paths_from_uri_list(b"").is_empty());
    }
}
