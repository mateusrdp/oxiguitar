// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The SDK for OxiGuitar plugins (docs/design/plugins.md).
//!
//! A plugin is a Rust library crate that:
//!
//! 1. writes its manifest as `manifest.json` beside its `Cargo.toml`
//!    ([`api::manifest`] describes the keys);
//! 2. implements [`Plugin`], handing out a [`FormatReader`],
//!    [`FormatWriter`], [`Service`] or [`Tab`] for each contribution its
//!    manifest declares, and handling the [`Event`]s it subscribes to;
//! 3. calls [`export_plugin!`] once, in its `lib.rs`;
//! 4. has a Cargo feature named `oxp` (`oxp = []`), which the `.oxp` build
//!    turns on.
//!
//! ```ignore
//! use oxiguitar_plugin::{Error, FormatReader, Host, Plugin, Read};
//!
//! pub struct Gp5;
//!
//! impl Plugin for Gp5 {
//!     fn start(_host: Host) -> Result<Gp5, Error> {
//!         Ok(Gp5)
//!     }
//!     fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
//!         (format == "gp5").then_some(self as &dyn FormatReader)
//!     }
//! }
//!
//! impl FormatReader for Gp5 {
//!     fn read(&self, data: &[u8]) -> Result<Read, Error> {
//!         todo!()
//!     }
//! }
//!
//! oxiguitar_plugin::export_plugin!(Gp5);
//! ```
//!
//! Build the `.oxp` file with
//! `cargo rustc --release -p NAME --features oxp --crate-type cdylib`, then
//! copy the library (`libNAME.so`, `libNAME.dylib` or `NAME.dll`) to
//! `NAME.oxp`. The `oxiguitar-plugin-test` tool does both and checks the
//! result against the interface.
//!
//! Plugin authors write no `unsafe` code. `export_plugin!` expands to the
//! entry symbol and the manifest section (two `unsafe` attributes), and the
//! SDK's export glue does the rest; both are reviewed line by line. Without
//! the `oxp` feature the crate is an ordinary library: tests and apps that
//! build plugins in (the mobile apps) call [`Plugin`] directly, or through
//! [`dispatch`] with messages.
//!
//! Every call may come from any thread, several at once, so a plugin is
//! `Send + Sync`. A panic inside a call is caught and reported as an
//! error; the plugin stays loaded and usable (build with the default
//! `panic = "unwind"`).

pub use oxiguitar_plugin_api as api;
pub use oxiguitar_plugin_api::display;
pub use oxiguitar_plugin_api::event::{Event, Playback, Position, Selection};
pub use oxiguitar_plugin_api::message::{Availability, Error, ErrorKind, Level, LogEntry};
pub use oxiguitar_plugin_api::score;
pub use oxiguitar_plugin_api::tab::{
    Canvas, Pointer, PointerButton, PointerKind, Row, TabContent, TabEvent, Widget,
};

use std::sync::Arc;

use oxiguitar_plugin_api::Blob;
use oxiguitar_plugin_api::manifest::Manifest;
use oxiguitar_plugin_api::message::{Answer, HostAnswer, HostRequest, Request};

#[doc(hidden)]
pub mod glue;

/// A plugin: the one value an `.oxp` file holds. Built once when the host
/// first needs one of its contributions, and kept until the program ends.
pub trait Plugin: Send + Sync + 'static {
    /// Builds the plugin. `host` reaches the core's services; keep it if the
    /// plugin needs them later. An error here means the plugin can't work;
    /// the host says so in the footer and never calls it again.
    fn start(host: Host) -> Result<Self, Error>
    where
        Self: Sized;

    /// The reader for the format `format` (an id from the manifest).
    fn reader(&self, format: &str) -> Option<&dyn FormatReader> {
        let _ = format;
        None
    }

    /// The writer for the format `format`.
    fn writer(&self, format: &str) -> Option<&dyn FormatWriter> {
        let _ = format;
        None
    }

    /// The service `id` of the interface `interface`.
    fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
        let _ = (interface, id);
        None
    }

    /// The tab `id` (an id from the manifest's `tabs`).
    fn tab(&self, id: &str) -> Option<&dyn Tab> {
        let _ = id;
        None
    }

    /// Something the manifest's `events` subscribe to happened. Called on
    /// any thread, as it happens; keep it short, and call
    /// [`Host::redraw`] for a tab that should show it. The default does
    /// nothing.
    fn event(&self, event: &Event) {
        let _ = event;
    }
}

/// A tab in one of the window's areas (docs/design/plugins.md, "What
/// plugins contribute"): standard widgets the core draws, and a canvas the
/// plugin draws.
pub trait Tab: Send + Sync {
    /// What the tab shows now. The core asks when the tab is shown, after
    /// every [`Tab::on`], and after [`Host::redraw`].
    fn content(&self) -> TabContent;

    /// The user used the widget `widget` (its id). The tab's contents are
    /// asked for again afterwards. The default does nothing.
    fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
        let _ = (widget, event);
        Ok(())
    }
}

/// Reads one file format into a score.
pub trait FormatReader: Send + Sync {
    /// Reads `data`, a whole file. Treat it as untrusted: a damaged file
    /// gives an error saying where, never a panic (FMT-1).
    fn read(&self, data: &[u8]) -> Result<Read, Error>;
}

/// What a [`FormatReader`] read.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    /// The score.
    pub score: score::Score,
    /// What couldn't be carried over faithfully (FMT-2).
    pub log: Vec<LogEntry>,
}

/// Writes a score in one file format.
pub trait FormatWriter: Send + Sync {
    /// Writes `score` as a whole file.
    fn write(&self, score: &score::Score) -> Result<Written, Error>;
}

/// What a [`FormatWriter`] wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    /// The file's contents.
    pub data: Vec<u8>,
    /// What the format can't hold and was left out (FMT-3).
    pub losses: Vec<LogEntry>,
}

/// One implementation of a service interface.
pub trait Service: Send + Sync {
    /// Whether the service can work here. The host asks once, before the
    /// first use; the default says yes.
    fn probe(&self) -> Availability {
        Availability::Available
    }

    /// Calls the method `method` with `input`; the interface defines both,
    /// and the output.
    fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error>;
}

/// Answers one request with `plugin`: what the export glue does for every
/// call through the interface, and what a host that builds plugins in
/// calls directly.
pub fn dispatch(plugin: &dyn Plugin, request: Request<'_>) -> Result<Answer<'static>, Error> {
    match request {
        Request::Read { format, data } => {
            let reader = plugin
                .reader(&format)
                .ok_or_else(|| Error::not_found(format!("this plugin doesn't read {format:?}")))?;
            let Read { score, log } = reader.read(data.as_slice())?;
            Ok(Answer::Read {
                score: Box::new(score),
                log,
            })
        }
        Request::Write { format, score } => {
            let writer = plugin
                .writer(&format)
                .ok_or_else(|| Error::not_found(format!("this plugin doesn't write {format:?}")))?;
            let Written { data, losses } = writer.write(&score)?;
            Ok(Answer::Written {
                data: Blob::owned(data),
                losses,
            })
        }
        Request::Probe { interface, id } => {
            let service = find_service(plugin, &interface, &id)?;
            Ok(Answer::Probe(service.probe()))
        }
        Request::Service {
            interface,
            id,
            method,
            input,
        } => {
            let service = find_service(plugin, &interface, &id)?;
            Ok(Answer::Service(Blob::owned(
                service.call(&method, input.as_slice())?,
            )))
        }
        Request::Tab { tab } => Ok(Answer::Tab(Box::new(find_tab(plugin, &tab)?.content()))),
        Request::TabEvent { tab, widget, event } => {
            let tab = find_tab(plugin, &tab)?;
            tab.on(&widget, event)?;
            Ok(Answer::Tab(Box::new(tab.content())))
        }
        Request::Event(event) => {
            plugin.event(&event);
            Ok(Answer::Done)
        }
    }
}

fn find_tab<'p>(plugin: &'p dyn Plugin, id: &str) -> Result<&'p dyn Tab, Error> {
    plugin
        .tab(id)
        .ok_or_else(|| Error::not_found(format!("this plugin has no tab {id:?}")))
}

fn find_service<'p>(
    plugin: &'p dyn Plugin,
    interface: &str,
    id: &str,
) -> Result<&'p dyn Service, Error> {
    plugin.service(interface, id).ok_or_else(|| {
        Error::not_found(format!(
            "this plugin has no service {id:?} of {interface:?}"
        ))
    })
}

/// The core's services, as a plugin reaches them.
#[derive(Clone)]
pub struct Host(Arc<dyn HostServices>);

/// Something that answers a plugin's requests: the core, through the
/// interface, or a test.
pub trait HostServices: Send + Sync {
    /// Answers one request.
    fn request(&self, request: HostRequest<'_>) -> Result<HostAnswer<'static>, Error>;
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Host")
    }
}

impl Host {
    /// A host answering through `services`.
    pub fn new(services: Arc<dyn HostServices>) -> Host {
        Host(services)
    }

    /// A host that shows no messages, keeps no settings and has no
    /// services: for tests that don't care.
    pub fn detached() -> Host {
        Host(Arc::new(Detached))
    }

    /// Shows a message in the footer. A message that can't be shown is
    /// dropped.
    pub fn notify(&self, level: Level, text: impl Into<String>) {
        let _ = self.0.request(HostRequest::Notify {
            level,
            text: text.into(),
        });
    }

    /// The plugin's own settings: the object under its id in settings.json
    /// (empty when there is none).
    pub fn settings(&self) -> Result<serde_json::Map<String, serde_json::Value>, Error> {
        match self.0.request(HostRequest::Settings)? {
            HostAnswer::Settings(json) => match serde_json::from_str(&json) {
                Ok(serde_json::Value::Object(map)) => Ok(map),
                _ => Err(Error::new(
                    ErrorKind::Invalid,
                    "the host's settings are not a JSON object",
                )),
            },
            other => Err(unexpected(&other)),
        }
    }

    /// Replaces the plugin's own settings, and saves settings.json.
    pub fn set_settings(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        let json = serde_json::to_string(settings)
            .map_err(|e| Error::new(ErrorKind::Invalid, e.to_string()))?;
        match self.0.request(HostRequest::SetSettings { json })? {
            HostAnswer::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// Calls `method` of the service the host chose for `interface`, which
    /// the manifest lists under `requires`.
    pub fn call_service(
        &self,
        interface: &str,
        method: &str,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        match self.0.request(HostRequest::Service {
            interface: interface.into(),
            method: method.into(),
            input: Blob::borrowed(input),
        })? {
            HostAnswer::Service(output) => Ok(output.into_vec()),
            other => Err(unexpected(&other)),
        }
    }

    /// Asks the core to show the tab `tab` (an id from the manifest) again:
    /// it asks for [`Tab::content`] when the tab is next on screen. A
    /// request that can't be made is dropped.
    pub fn redraw(&self, tab: &str) {
        let _ = self.0.request(HostRequest::Redraw { tab: tab.into() });
    }

    /// The open score, or `None` when none is open: what a plugin asks for
    /// after [`Event::ScoreOpened`] or [`Event::ScoreChanged`].
    pub fn score(&self) -> Result<Option<score::Score>, Error> {
        match self.0.request(HostRequest::Score)? {
            HostAnswer::Score(score) => Ok(score.map(|s| *s)),
            other => Err(unexpected(&other)),
        }
    }
}

fn unexpected(answer: &HostAnswer<'_>) -> Error {
    Error::new(
        ErrorKind::Failed,
        format!("the host answered unexpectedly ({answer:?})"),
    )
}

struct Detached;

impl HostServices for Detached {
    fn request(&self, request: HostRequest<'_>) -> Result<HostAnswer<'static>, Error> {
        match request {
            HostRequest::Notify { .. }
            | HostRequest::SetSettings { .. }
            | HostRequest::Redraw { .. } => Ok(HostAnswer::Done),
            HostRequest::Settings => Ok(HostAnswer::Settings("{}".into())),
            HostRequest::Score => Ok(HostAnswer::Score(None)),
            HostRequest::Service { interface, .. } => Err(Error::not_found(format!(
                "no service of {interface:?} (a detached host has none)"
            ))),
        }
    }
}

/// Everything wrong with a plugin's manifest and the contributions it
/// declares, checked without the interface: what `export_plugin!`'s test
/// runs under `cargo test`, so mistakes show up before an `.oxp` is built.
///
/// `manifest_json` is the manifest's text and `crate_version` the crate's
/// version, which the manifest's version must equal.
pub fn check<P: Plugin>(manifest_json: &str, crate_version: &str) -> Vec<String> {
    let manifest = match Manifest::parse(manifest_json.as_bytes()) {
        Ok(m) => m,
        Err(e) => return vec![e],
    };
    let mut out = manifest.problems();
    out.extend(unknown_keys(manifest_json, &manifest));
    let (major, minor) = oxiguitar_plugin_api::INTERFACE;
    if manifest.interface.major != major || manifest.interface.minor > minor {
        out.push(format!(
            "the manifest asks for interface {}, but this SDK is {major}.{minor}",
            manifest.interface
        ));
    }
    if manifest.version.to_string() != crate_version {
        out.push(format!(
            "the manifest's version is {}, but the crate's is {crate_version}",
            manifest.version
        ));
    }
    let plugin = match P::start(Host::detached()) {
        Ok(p) => p,
        Err(e) => {
            out.push(format!("the plugin doesn't start: {e}"));
            return out;
        }
    };
    for f in &manifest.formats {
        if f.read && plugin.reader(&f.id).is_none() {
            out.push(format!(
                "format {:?} is declared as read, but has no reader",
                f.id
            ));
        }
        if f.write && plugin.writer(&f.id).is_none() {
            out.push(format!(
                "format {:?} is declared as written, but has no writer",
                f.id
            ));
        }
    }
    for s in &manifest.services {
        if plugin.service(&s.interface, &s.id).is_none() {
            out.push(format!(
                "service {:?} of {:?} is declared, but not implemented",
                s.id, s.interface
            ));
        }
    }
    for t in &manifest.tabs {
        match plugin.tab(&t.id) {
            None => out.push(format!("tab {:?} is declared, but not implemented", t.id)),
            Some(tab) => {
                for p in tab.content().problems() {
                    out.push(format!("tab {:?}: {p}", t.id));
                }
            }
        }
    }
    out
}

/// Keys in the manifest's text that the manifest types don't know: typing
/// mistakes, or keys from a newer interface than this SDK.
fn unknown_keys(text: &str, manifest: &Manifest) -> Vec<String> {
    let (Ok(written), Ok(known)) = (
        serde_json::from_str::<serde_json::Value>(text),
        serde_json::to_value(manifest),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    compare(&written, &known, "", &mut out);
    out
}

fn compare(
    written: &serde_json::Value,
    known: &serde_json::Value,
    at: &str,
    out: &mut Vec<String>,
) {
    use serde_json::Value;
    match (written, known) {
        (Value::Object(w), Value::Object(k)) => {
            for (key, value) in w {
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                match k.get(key) {
                    Some(known) => compare(value, known, &path, out),
                    None => out.push(format!("the manifest key {path:?} is unknown")),
                }
            }
        }
        (Value::Array(w), Value::Array(k)) => {
            for (i, (w, k)) in w.iter().zip(k).enumerate() {
                compare(w, k, &format!("{at}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// Exports a plugin: its entry symbol and its manifest section, when the
/// crate's `oxp` feature is on, and always a constant `MANIFEST_JSON` with
/// the manifest's text and a test (under `cargo test`) running [`check`].
///
/// Call it once, in the crate's `lib.rs`, with the type implementing
/// [`Plugin`]. The manifest is the file `manifest.json` beside the crate's
/// `Cargo.toml`.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        /// The plugin's manifest, `manifest.json`.
        pub const MANIFEST_JSON: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json"));

        // The section and the entry symbol need `unsafe` attributes, which
        // the plugin crate's `unsafe_code = "deny"` allows here only.
        #[cfg(feature = "oxp")]
        #[allow(unsafe_code)]
        const _: () = {
            // The manifest's bytes, in the section the host reads without
            // loading the plugin (oxiguitar_plugin_api::MANIFEST_SECTION).
            #[cfg_attr(target_os = "windows", unsafe(link_section = "oxgmanif"))]
            #[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,oxg_manifest"))]
            #[cfg_attr(
                not(any(target_os = "windows", target_vendor = "apple")),
                unsafe(link_section = "oxg_manifest")
            )]
            #[used]
            static MANIFEST: [u8; MANIFEST_JSON.len()] =
                *include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json"));

            static TABLE: $crate::glue::Table = $crate::glue::table::<$plugin>(&MANIFEST);

            /// The plugin's entry point (oxiguitar_plugin_api::ENTRY_SYMBOL).
            #[unsafe(no_mangle)]
            pub extern "C" fn oxiguitar_plugin_v1() -> *const $crate::api::abi::PluginTableV1 {
                TABLE.get()
            }
        };

        #[cfg(test)]
        mod oxiguitar_plugin_checks {
            #[allow(unused_imports)]
            use super::*;

            #[test]
            fn the_manifest_matches_the_plugin() {
                let problems =
                    $crate::check::<$plugin>(super::MANIFEST_JSON, env!("CARGO_PKG_VERSION"));
                assert!(
                    problems.is_empty(),
                    "manifest.json: {}",
                    problems.join("; ")
                );
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Plugin for Echo {
        fn start(_host: Host) -> Result<Echo, Error> {
            Ok(Echo)
        }

        fn service(&self, interface: &str, id: &str) -> Option<&dyn Service> {
            (interface == "echo" && id == "echo").then_some(self as &dyn Service)
        }
    }

    impl Service for Echo {
        fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
            match method {
                "echo" => Ok(input.to_vec()),
                _ => Err(Error::not_found(method)),
            }
        }
    }

    const ECHO: &str = r#"{
        "interface": "1.0", "id": "test.echo", "name": "Echo", "version": "0.1.0",
        "services": [{"interface": "echo", "id": "echo"}]
    }"#;

    #[test]
    fn dispatch_reaches_the_contribution_or_says_it_is_missing() {
        let echo = Echo;
        let answer = dispatch(
            &echo,
            Request::Service {
                interface: "echo".into(),
                id: "echo".into(),
                method: "echo".into(),
                input: Blob::borrowed(b"hi"),
            },
        );
        assert_eq!(answer, Ok(Answer::Service(Blob::owned(b"hi".to_vec()))));
        let missing = dispatch(
            &echo,
            Request::Read {
                format: "gp5".into(),
                data: Blob::borrowed(b""),
            },
        )
        .unwrap_err();
        assert_eq!(missing.kind, ErrorKind::NotFound);
        let probe = dispatch(
            &echo,
            Request::Probe {
                interface: "echo".into(),
                id: "echo".into(),
            },
        );
        assert_eq!(probe, Ok(Answer::Probe(Availability::Available)));
    }

    #[test]
    fn check_finds_mistakes() {
        assert_eq!(check::<Echo>(ECHO, "0.1.0"), Vec::<String>::new());
        assert_eq!(
            check::<Echo>(ECHO, "0.2.0"),
            vec!["the manifest's version is 0.1.0, but the crate's is 0.2.0"]
        );
        let typo = ECHO.replace("\"services\"", "\"description\": \"\", \"servces\"");
        assert_eq!(
            check::<Echo>(&typo, "0.1.0"),
            vec!["the manifest key \"servces\" is unknown"]
        );
        let nested = ECHO.replace("\"id\": \"echo\"}", "\"id\": \"echo\", \"prio\": 1}");
        assert_eq!(
            check::<Echo>(&nested, "0.1.0"),
            vec!["the manifest key \"services[0].prio\" is unknown"]
        );
        let missing = ECHO.replace("\"echo\"}]", "\"other\"}]");
        assert_eq!(
            check::<Echo>(&missing, "0.1.0"),
            vec!["service \"other\" of \"echo\" is declared, but not implemented"]
        );
        let later = ECHO.replace("\"1.0\"", "\"1.9\"");
        assert!(check::<Echo>(&later, "0.1.0")[0].contains("this SDK is 1.1"));
    }

    /// A tab counting its button's clicks, and events.
    struct Counter(std::sync::atomic::AtomicU32);

    impl Plugin for Counter {
        fn start(_host: Host) -> Result<Counter, Error> {
            Ok(Counter(0.into()))
        }

        fn tab(&self, id: &str) -> Option<&dyn Tab> {
            (id == "count").then_some(self as &dyn Tab)
        }

        fn event(&self, _event: &Event) {
            self.0.fetch_add(10, std::sync::atomic::Ordering::Relaxed);
        }
    }

    impl Tab for Counter {
        fn content(&self) -> TabContent {
            let n = self.0.load(std::sync::atomic::Ordering::Relaxed);
            TabContent {
                rows: vec![Row {
                    widgets: vec![Widget::Button {
                        id: "more".into(),
                        text: n.to_string(),
                        enabled: true,
                    }],
                }],
            }
        }

        fn on(&self, widget: &str, event: TabEvent) -> Result<(), Error> {
            match (widget, event) {
                ("more", TabEvent::Clicked) => {
                    self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(())
                }
                _ => Err(Error::not_found(widget)),
            }
        }
    }

    fn text_of(answer: Answer<'_>) -> String {
        match answer {
            Answer::Tab(t) => match &t.rows[0].widgets[0] {
                Widget::Button { text, .. } => text.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tabs_answer_with_their_contents_and_events_reach_the_plugin() {
        let counter = Counter::start(Host::detached()).unwrap();
        let tab = |tab: &str| dispatch(&counter, Request::Tab { tab: tab.into() });
        assert_eq!(text_of(tab("count").unwrap()), "0");
        assert_eq!(tab("other").unwrap_err().kind, ErrorKind::NotFound);
        let click = Request::TabEvent {
            tab: "count".into(),
            widget: "more".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(text_of(dispatch(&counter, click).unwrap()), "1");
        let wrong = Request::TabEvent {
            tab: "count".into(),
            widget: "less".into(),
            event: TabEvent::Clicked,
        };
        assert_eq!(
            dispatch(&counter, wrong).unwrap_err().kind,
            ErrorKind::NotFound
        );
        let event = Request::Event(Event::ScoreOpened { revision: 1 });
        assert_eq!(dispatch(&counter, event), Ok(Answer::Done));
        assert_eq!(text_of(tab("count").unwrap()), "11");

        let manifest = r#"{
            "interface": "1.1", "id": "test.counter", "name": "Counter", "version": "0.1.0",
            "tabs": [{"id": "count", "title": "Count", "area": "bottom"}],
            "events": ["score"]
        }"#;
        assert_eq!(check::<Counter>(manifest, "0.1.0"), Vec::<String>::new());
        let missing = manifest.replace("\"id\": \"count\"", "\"id\": \"other\"");
        assert_eq!(
            check::<Counter>(&missing, "0.1.0"),
            vec!["tab \"other\" is declared, but not implemented"]
        );
    }

    #[test]
    fn a_detached_host_answers_plainly() {
        let host = Host::detached();
        host.notify(Level::Info, "nobody hears this");
        assert!(host.settings().unwrap().is_empty());
        assert!(host.set_settings(&serde_json::Map::new()).is_ok());
        let e = host.call_service("audio-device", "open", b"").unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
        host.redraw("count");
        assert_eq!(host.score(), Ok(None));
    }
}
