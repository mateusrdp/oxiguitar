// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Test drag source for Wayland file drops (ADR 0006): a 200x150 window;
//! pressing the mouse in it starts a drag offering `text/uri-list` for the
//! file given on the command line, as file managers do. It prints each
//! step of the protocol it sees. See README.md.
use std::io::Write;
use std::os::fd::AsFd;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_source, wl_pointer,
    wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

struct S {
    uri: String,
    surface: wl_surface::WlSurface,
    device: wl_data_device::WlDataDevice,
    manager: wl_data_device_manager::WlDataDeviceManager,
    shm: wl_shm::WlShm,
    started: bool,
    done: bool,
    source: Option<wl_data_source::WlDataSource>,
}

macro_rules! ignore {
    ($t:ty, $e:ty) => {
        impl Dispatch<$t, ()> for S {
            fn event(_: &mut Self, _: &$t, _: $e, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    };
}
ignore!(wl_compositor::WlCompositor, wl_compositor::Event);
ignore!(wl_surface::WlSurface, wl_surface::Event);
ignore!(wl_shm::WlShm, wl_shm::Event);
ignore!(wl_shm_pool::WlShmPool, wl_shm_pool::Event);
ignore!(wl_buffer::WlBuffer, wl_buffer::Event);
ignore!(wl_seat::WlSeat, wl_seat::Event);
ignore!(
    wl_data_device_manager::WlDataDeviceManager,
    wl_data_device_manager::Event
);
ignore!(
    wayland_client::protocol::wl_data_offer::WlDataOffer,
    wayland_client::protocol::wl_data_offer::Event
);
impl Dispatch<wl_data_device::WlDataDevice, ()> for S {
    fn event(
        _: &mut Self,
        _: &wl_data_device::WlDataDevice,
        e: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_device::Event::DataOffer { id } = e {
            id.destroy();
        }
    }
    wayland_client::event_created_child!(S, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wayland_client::protocol::wl_data_offer::WlDataOffer, ()),
    ]);
}
ignore!(xdg_toplevel::XdgToplevel, xdg_toplevel::Event);

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for S {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for S {
    fn event(
        _: &mut Self,
        b: &xdg_wm_base::XdgWmBase,
        e: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = e {
            b.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for S {
    fn event(
        s: &mut Self,
        x: &xdg_surface::XdgSurface,
        e: xdg_surface::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = e {
            x.ack_configure(serial);
            let (w, h) = (200, 150);
            let path = std::env::temp_dir().join(format!("dragsrc-{}", std::process::id()));
            let mut f = std::fs::File::options()
                .create(true)
                .read(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            f.write_all(&vec![0x40u8; (w * h * 4) as usize]).unwrap();
            let _ = std::fs::remove_file(&path);
            let pool = s.shm.create_pool(f.as_fd(), w * h * 4, qh, ());
            let buf = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Xrgb8888, qh, ());
            s.surface.attach(Some(&buf), 0, 0);
            s.surface.commit();
            eprintln!("source: mapped");
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for S {
    fn event(
        s: &mut Self,
        _: &wl_pointer::WlPointer,
        e: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            state: WEnum::Value(wl_pointer::ButtonState::Pressed),
            ..
        } = e
        {
            if s.started {
                return;
            }
            s.started = true;
            let src = s.manager.create_data_source(qh, ());
            src.offer("text/uri-list".into());
            src.offer("text/plain;charset=utf-8".into());
            if src.version() >= 3 {
                src.set_actions(
                    wl_data_device_manager::DndAction::Copy
                        | wl_data_device_manager::DndAction::Move,
                );
            }
            s.device.start_drag(Some(&src), &s.surface, None, serial);
            s.source = Some(src);
            eprintln!("source: drag started");
        }
    }
}
impl Dispatch<wl_data_source::WlDataSource, ()> for S {
    fn event(
        s: &mut Self,
        _: &wl_data_source::WlDataSource,
        e: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_data_source::Event::Target { mime_type } => {
                eprintln!("source: target accepts {mime_type:?}")
            }
            wl_data_source::Event::Action { dnd_action } => {
                eprintln!("source: action {dnd_action:?}")
            }
            wl_data_source::Event::Send { mime_type, fd } => {
                eprintln!("source: send {mime_type}");
                let mut f = std::fs::File::from(fd);
                let _ = f.write_all(s.uri.as_bytes());
            }
            wl_data_source::Event::DndDropPerformed => eprintln!("source: drop performed"),
            wl_data_source::Event::DndFinished => {
                eprintln!("source: finished");
                s.done = true;
            }
            wl_data_source::Event::Cancelled => {
                eprintln!("source: cancelled");
                s.done = true;
            }
            _ => {}
        }
    }
}
use wayland_client::Proxy;

fn main() {
    let path = std::fs::canonicalize(std::env::args().nth(1).unwrap()).unwrap();
    let enc: String = path
        .to_str()
        .unwrap()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut q) = registry_queue_init::<S>(&conn).unwrap();
    let qh = q.handle();
    let comp: wl_compositor::WlCompositor = globals.bind(&qh, 1..=4, ()).unwrap();
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
    let wm: xdg_wm_base::XdgWmBase = globals.bind(&qh, 1..=1, ()).unwrap();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=1, ()).unwrap();
    let manager: wl_data_device_manager::WlDataDeviceManager =
        globals.bind(&qh, 1..=3, ()).unwrap();
    let device = manager.get_data_device(&seat, &qh, ());
    let _pointer = seat.get_pointer(&qh, ());
    let surface = comp.create_surface(&qh, ());
    let xs = wm.get_xdg_surface(&surface, &qh, ());
    let top = xs.get_toplevel(&qh, ());
    top.set_title("dragsrc".into());
    surface.commit();
    let mut s = S {
        uri: format!("# test\r\nfile://{enc}\r\n"),
        surface,
        device,
        manager,
        shm,
        started: false,
        done: false,
        source: None,
    };
    while !s.done {
        q.blocking_dispatch(&mut s).unwrap();
    }
}
