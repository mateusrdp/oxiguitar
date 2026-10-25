// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin contract suite (docs/design/plugins.md, "Testing").
//!
//! [`build`] builds a plugin crate as an `.oxp` file, and [`check`] loads
//! one through OxiGuitar's own plugin host and checks it against the
//! interface:
//!
//! - its manifest is read without loading the plugin;
//! - it loads and starts;
//! - a request for a contribution it doesn't have is answered with
//!   "not found", not a crash;
//! - every format it reads answers damaged input (nothing, zeros, its own
//!   signature alone) with an error or a score, never a crash;
//! - every format it writes answers an empty score;
//! - every service it declares answers its probe;
//! - every tab it adds (interface 1.1) shows contents the core can draw,
//!   and a plausible use of each widget (a click, a tick, the slider's
//!   lowest value, an empty field, the first option, a press, drag and
//!   release in the middle of a canvas) answers with contents it can draw;
//! - every kind of event it subscribes to is delivered without an error;
//! - every tool it adds (interface 1.2) runs with no score open;
//! - its settings window and its sections of the track dialog (interface
//!   1.3) show contents the core can draw, before and after a plausible use
//!   of each widget, and a section hands back its track's settings as a
//!   JSON object.
//!
//! - every instrument and effect it offers (interface 1.3) prepares; an
//!   instrument starts a note within its declared resolution of the frame
//!   asked and is silent before; neither allocates while rendering (the
//!   SDK's allocator counts), and on Linux neither makes a system call
//!   while rendering: no lock that waits, no sleep, no file, no memory
//!   mapped. A lock nobody else holds makes no system call, so that one
//!   can't be seen.
//!
//! The `oxiguitar-plugin-test` program runs the same checks from the
//! command line, for plugin authors.

#[cfg(target_os = "linux")]
pub mod syscalls;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use oxiguitar_host::api::Blob;
use oxiguitar_host::api::class::InstrumentClass;
use oxiguitar_host::api::display::Point;
use oxiguitar_host::api::event::{Event, EventKind, Playback, Position};
use oxiguitar_host::api::manifest::SharedFolder;
use oxiguitar_host::api::message::{
    Answer, Error, Prepare, PrepareEffect, PreparedTrack, Request, TrackInfo,
};
use oxiguitar_host::api::realtime::{RtEvent, RtEventKind, RtNote};
use oxiguitar_host::api::score::Score;
use oxiguitar_host::api::tab::{Pointer, PointerButton, PointerKind, TabContent, TabEvent, Widget};
use oxiguitar_host::realtime::Instance;
use oxiguitar_host::{Core, Folders, Level, Plugins, SharedSettings, State, System};

/// Builds the plugin crate `package` of the workspace at `workspace` as
/// `NAME.oxp` (with its `oxp` feature, as a `cdylib`), in the workspace's
/// `target/oxp/` folder (its own, so a build started from a test doesn't
/// wait for the test's own build folder). Returns the file's path.
pub fn build(
    workspace: &Path,
    package: &str,
    name: &str,
    release: bool,
) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = workspace.join("target").join("oxp");
    let mut command = std::process::Command::new(cargo);
    command
        .current_dir(workspace)
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "rustc",
            "--quiet",
            "-p",
            package,
            "--features",
            "oxp",
            "--crate-type",
            "cdylib",
        ]);
    if release {
        command.arg("--release");
    } else {
        // Without debug information a test build is a tenth of the size;
        // contract tests copy it around.
        command
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_DEV_STRIP", "debuginfo");
    }
    let status = command
        .status()
        .map_err(|e| format!("cargo didn't run ({e})"))?;
    if !status.success() {
        return Err(format!("cargo couldn't build {package} ({status})"));
    }
    let library = target
        .join(if release { "release" } else { "debug" })
        .join(format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            package.replace('-', "_"),
            std::env::consts::DLL_SUFFIX
        ));
    let oxp = target.join(format!("{name}.oxp"));
    // A new file, not the old one overwritten: hard links to the old one
    // (tests make them) and a copy a running program has loaded keep theirs.
    let _ = std::fs::remove_file(&oxp);
    std::fs::copy(&library, &oxp).map_err(|e| {
        format!(
            "cannot copy {} to {}: {e}",
            library.display(),
            oxp.display()
        )
    })?;
    Ok(oxp)
}

/// The sound set the suite puts in the `soundfonts` folder of every plugin
/// whose manifest lists that folder: a SoundFont 2 file holding a looped
/// 480 Hz sine with no envelope delay or attack, as a melodic preset (bank 0,
/// program 0) and a drum kit (bank 128, program 0). Every note of it is
/// heard from its first frame, so an instrument that plays from a sound set
/// can show it keeps time. The SoundFont plugin's tests make it again and
/// check this copy (`OXIGUITAR_BLESS=1 cargo test -p
/// oxiguitar-soundfont-plugin` rewrites it).
pub const TEST_SET: &[u8] = include_bytes!("test-sine.sf2");

/// Our own plugins, shipped with OxiGuitar: each crate's package name and
/// its file's name (`NAME.oxp`). Each lives in `plugins/NAME/` as
/// `oxiguitar-NAME-plugin`; `plugins/test` is never shipped.
pub const OURS: &[(&str, &str)] = &[
    ("oxiguitar-gp3-plugin", "gp3"),
    ("oxiguitar-gp4-plugin", "gp4"),
    ("oxiguitar-gp5-plugin", "gp5"),
    ("oxiguitar-gpx-plugin", "gpx"),
    ("oxiguitar-gp-plugin", "gp"),
];

/// Our instrument plugins (interface 1.3), shipped with the format plugins:
/// each crate's package name and its file's name. [`build_ours`] builds
/// them too.
pub const OUR_INSTRUMENTS: &[(&str, &str)] = &[("oxiguitar-soundfont-plugin", "soundfont")];

/// Our audio-device plugins (interface 1.3), shipped with the format
/// plugins: each crate's package name, its file's name and the system it
/// runs on (its manifest's `os`). Elsewhere each builds as a plugin whose
/// service is never available, and isn't shipped.
pub const OUR_DEVICES: &[(&str, &str, &str)] = &[
    ("oxiguitar-audio-pipewire-plugin", "audio-pipewire", "linux"),
    ("oxiguitar-audio-alsa-plugin", "audio-alsa", "linux"),
    ("oxiguitar-audio-wasapi-plugin", "audio-wasapi", "windows"),
    (
        "oxiguitar-audio-coreaudio-plugin",
        "audio-coreaudio",
        "macos",
    ),
];

/// Builds the plugins of [`OUR_DEVICES`] that run on this system into the
/// workspace's `target/oxp/devices-debug/` or `devices-release/`, as
/// [`build_ours`] does. Returns that folder and the files' names.
pub fn build_devices(workspace: &Path, release: bool) -> Result<(PathBuf, Vec<String>), String> {
    let folder = workspace.join("target").join("oxp").join(if release {
        "devices-release"
    } else {
        "devices-debug"
    });
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut names = Vec::new();
    for (package, name, os) in OUR_DEVICES {
        if *os != std::env::consts::OS {
            continue;
        }
        let built = build(workspace, package, name, release)?;
        let file = format!("{name}.oxp");
        let to = folder.join(&file);
        let _ = std::fs::remove_file(&to);
        std::fs::copy(&built, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        names.push(file);
    }
    Ok((folder, names))
}

/// Builds every plugin of [`OURS`] and [`OUR_INSTRUMENTS`], each on its own so it carries only its
/// own code (built together, Cargo would give each the features of all),
/// and puts the files in the workspace's `target/oxp/ours-debug/` or
/// `ours-release/`, replacing what was there. Returns that folder, a plugin
/// folder for tests.
pub fn build_ours(workspace: &Path, release: bool) -> Result<PathBuf, String> {
    let folder = workspace.join("target").join("oxp").join(if release {
        "ours-release"
    } else {
        "ours-debug"
    });
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for (package, name) in OURS.iter().chain(OUR_INSTRUMENTS) {
        let built = build(workspace, package, name, release)?;
        let to = folder.join(format!("{name}.oxp"));
        // A new file: a program that loaded the old one keeps its own.
        let _ = std::fs::remove_file(&to);
        std::fs::copy(&built, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    Ok(folder)
}

/// The outcome of [`check`]: each check's description, passed or with
/// what went wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The checks that passed.
    pub passed: Vec<String>,
    /// The checks that failed, with why.
    pub failed: Vec<String>,
    /// Footer messages the plugin sent while being checked.
    pub messages: Vec<String>,
    /// How long reading the manifest took.
    pub manifest_time: Duration,
}

impl Report {
    /// Whether every check passed.
    pub fn ok(&self) -> bool {
        self.failed.is_empty()
    }

    fn check(&mut self, what: impl Into<String>, result: Result<(), String>) {
        let what = what.into();
        match result {
            Ok(()) => self.passed.push(what),
            Err(why) => self.failed.push(format!("{what}: {why}")),
        }
    }
}

/// Footer messages, kept for the report.
#[derive(Default)]
struct Kept(Mutex<Vec<String>>);

impl Core for Kept {
    fn notify(&self, level: Level, text: String) {
        if let Ok(mut kept) = self.0.lock() {
            kept.push(format!("{level:?}: {text}"));
        }
    }
}

/// Checks the plugin file `oxp` against the interface (see the crate's
/// documentation). The file is copied into a folder of its own first, so
/// no other plugin is found beside it; the copy stays loaded until the
/// program ends.
pub fn check(oxp: &Path) -> Report {
    let mut report = Report::default();
    let folder = std::env::temp_dir().join(format!(
        "oxiguitar-plugin-test-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&folder);
    let copy = folder.join(oxp.file_name().unwrap_or("plugin.oxp".as_ref()));
    if let Err(e) =
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::copy(oxp, &copy).map(drop))
    {
        report.check("the plugin file can be read", Err(e.to_string()));
        return report;
    }
    let kept = Arc::new(Kept::default());
    let started = Instant::now();
    let plugins = Plugins::discover(
        std::slice::from_ref(&folder),
        SharedSettings::default(),
        oxiguitar_host::display_system(),
        kept.clone(),
    );
    report.manifest_time = started.elapsed();
    // The plugin's own folders and the shared ones, in the scratch folder.
    let home = folder.join("home");
    if let Some(folders) = Folders::resolve(
        System::this(),
        |name| (name == oxiguitar_host::HOME_VARIABLE).then(|| home.clone().into()),
        None,
    ) {
        let shared = plugins.shared_folders();
        let _ = folders.ensure_shared(&shared);
        if shared.contains(&SharedFolder::Soundfonts) {
            // A sound set for instruments that play from one: a looped sine,
            // starting on its frame (TEST_SET).
            let _ = std::fs::write(
                folders
                    .shared(SharedFolder::Soundfonts)
                    .join("test-sine.sf2"),
                TEST_SET,
            );
        }
        plugins.use_folders(folders);
    }
    let Some(found) = plugins.found().first() else {
        report.check(
            "the manifest reads without loading the plugin",
            Err("the file wasn't found".into()),
        );
        return report;
    };
    report.check(
        "the manifest reads without loading the plugin",
        match (&found.state, found.is_loaded()) {
            (State::Ready, false) => Ok(()),
            (State::Ready, true) => Err("reading it loaded the plugin".into()),
            (state, _) => Err(format!("{state:?}")),
        },
    );
    if found.state != State::Ready {
        return report;
    }
    let manifest = found
        .manifest
        .clone()
        .unwrap_or_else(|| unreachable!("a ready plugin has a manifest"));
    let loading = plugins.load_all();
    report.check(
        "it loads and starts",
        if loading.is_empty() && found.is_loaded() {
            Ok(())
        } else {
            Err(loading.join("; "))
        },
    );
    if !found.is_loaded() {
        return report;
    }

    let missing = plugins.send(
        found,
        &Request::Read {
            format: "oxiguitar-contract-no-such-format".into(),
            data: Blob::borrowed(b""),
        },
    );
    report.check(
        "a contribution it doesn't have is \"not found\"",
        match missing {
            Err(e) if e.kind == oxiguitar_host::api::message::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("answered {:?}: {}", e.kind, e.message)),
            Ok(_) => Err("it answered as if it had it".into()),
        },
    );

    for format in manifest.formats.iter().filter(|f| f.read) {
        let mut inputs: Vec<(&str, Vec<u8>)> =
            vec![("nothing", Vec::new()), ("1 KiB of zeros", vec![0; 1024])];
        if let Some(sig) = format.signatures.first() {
            let mut bytes = vec![0; usize::try_from(sig.at).unwrap_or(0).min(1 << 16)];
            if let Some(text) = &sig.text {
                bytes.extend_from_slice(text.as_bytes());
            }
            inputs.push(("its signature alone", bytes));
        }
        for (what, data) in inputs {
            let answer = plugins.send(
                found,
                &Request::Read {
                    format: format.id.clone(),
                    data: Blob::borrowed(&data),
                },
            );
            report.check(
                format!("format {}: reading {what} doesn't crash", format.id),
                match answer {
                    Ok(Answer::Read { .. }) => Ok(()),
                    Ok(other) => Err(format!("answered {other:?}")),
                    Err(e) => not_a_crash(&e),
                },
            );
        }
    }
    for format in manifest.formats.iter().filter(|f| f.write) {
        // Version 2 of the score for a plugin built for interface 1.2.
        let request = if manifest.knows_minor(2) {
            Request::Write2 {
                format: format.id.clone(),
                score: Box::default(),
            }
        } else {
            Request::Write {
                format: format.id.clone(),
                score: Box::new(Score::default()),
            }
        };
        let answer = plugins.send(found, &request);
        report.check(
            format!("format {}: writing an empty score answers", format.id),
            match answer {
                Ok(Answer::Written { .. }) => Ok(()),
                Ok(other) => Err(format!("answered {other:?}")),
                Err(e) => not_a_crash(&e),
            },
        );
    }
    for service in &manifest.services {
        let answer = plugins.send(
            found,
            &Request::Probe {
                interface: service.interface.clone(),
                id: service.id.clone(),
            },
        );
        report.check(
            format!(
                "service {} of {}: the probe answers",
                service.id, service.interface
            ),
            match answer {
                Ok(Answer::Probe(_)) => Ok(()),
                Ok(other) => Err(format!("answered {other:?}")),
                Err(e) => Err(e.message),
            },
        );
    }
    for tab in plugins.tabs() {
        let id = tab.id();
        let content = plugins.tab_content(&id);
        report.check(
            format!("tab {}: its contents can be shown", id.tab),
            content.as_ref().map(drop).map_err(|e| e.message.clone()),
        );
        let Ok(content) = content else { continue };
        for (widget, event) in plausible_events(&content) {
            let answer = plugins.tab_event(&id, &widget, event.clone());
            report.check(
                format!(
                    "tab {}: {event:?} on {widget} answers with contents it can show",
                    id.tab
                ),
                match answer {
                    Ok(_) => Ok(()),
                    Err(e) if e.message.contains("can't be shown") => Err(e.message),
                    Err(e) => not_a_crash(&e),
                },
            );
        }
    }
    let events = [
        (EventKind::Score, Event::ScoreOpened { revision: 1 }),
        (EventKind::Selection, Event::Selection(None)),
        (EventKind::Selection, Event::Selection2(None)),
        (
            EventKind::Playback,
            Event::Playback(Playback {
                playing: false,
                at: Position::default(),
                seconds: 0.0,
            }),
        ),
    ];
    for (kind, event) in events.into_iter().filter(|(k, _)| manifest.wants(*k)) {
        let warnings = |kept: &Kept| {
            kept.0
                .lock()
                .map(|m| m.iter().filter(|t| !t.starts_with("Info")).count())
                .unwrap_or(0)
        };
        let before = warnings(&kept);
        plugins.push(event);
        plugins.wait_for_events();
        report.check(
            format!("events of kind {kind:?} are taken"),
            if warnings(&kept) == before {
                Ok(())
            } else {
                Err(kept
                    .0
                    .lock()
                    .ok()
                    .and_then(|m| m.last().cloned())
                    .unwrap_or_default())
            },
        );
    }
    // Tools (interface 1.2), with no score open.
    for tool in plugins.tools() {
        let answer = plugins.run_tool(tool);
        report.check(
            format!("tool {}: it runs with no score open", tool.spec.id),
            match answer {
                Ok(_) => Ok(()),
                Err(e) => not_a_crash(&e),
            },
        );
    }
    // The settings window and the track dialog's sections (interface 1.3).
    if let Some(found) = plugins.with_settings_window().first() {
        let content = plugins.settings_window(found);
        report.check(
            "the settings window: its contents can be shown",
            content.as_ref().map(drop).map_err(|e| e.message.clone()),
        );
        if let Ok(content) = content {
            for (widget, event) in plausible_events(&content) {
                let answer = plugins.settings_event(found, &widget, event.clone());
                report.check(
                    format!(
                        "the settings window: {event:?} on {widget} answers with contents it can show"
                    ),
                    match answer {
                        Ok(_) => Ok(()),
                        Err(e) if e.message.contains("can't be shown") => Err(e.message),
                        Err(e) => not_a_crash(&e),
                    },
                );
            }
        }
    }
    for class in InstrumentClass::ALL {
        for section in plugins.track_sections(class) {
            // Each section once, for the first class it shows for.
            if InstrumentClass::ALL
                .iter()
                .take_while(|&&c| c != class)
                .any(|&c| section.spec.shows_for(c))
            {
                continue;
            }
            let track = TrackInfo {
                index: 0,
                name: "Track 1".into(),
                class,
            };
            let id = &section.spec.id;
            let answer = plugins.track_section(section, &track, "{}");
            report.check(
                format!("track section {id}: its contents can be shown"),
                answer.as_ref().map(drop).map_err(|e| e.message.clone()),
            );
            let Ok((content, settings)) = answer else {
                continue;
            };
            for (widget, event) in plausible_events(&content) {
                let answer =
                    plugins.track_section_event(section, &track, &settings, &widget, event.clone());
                report.check(
                    format!(
                        "track section {id}: {event:?} on {widget} answers with contents it can show and settings"
                    ),
                    match answer {
                        Ok(_) => Ok(()),
                        Err(e) if e.message.contains("can't be shown") => Err(e.message),
                        Err(e) => not_a_crash(&e),
                    },
                );
            }
        }
    }
    realtime_checks(&plugins, &mut report, "{}");
    report.messages = kept.0.lock().map(|m| m.clone()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&folder);
    report
}

/// The sample rate and the largest piece the real-time checks use.
const RATE: u32 = 48_000;
const FRAMES: usize = 256;

/// The real-time checks alone, of every instrument and effect `plugins`
/// offer, prepared for tracks whose settings are `settings` (a JSON
/// object): for tests of the checks themselves.
pub fn check_realtime(plugins: &Plugins, settings: &str) -> Report {
    let mut report = Report::default();
    realtime_checks(plugins, &mut report, settings);
    report
}

/// A track to prepare an instrument or effect for.
fn checked_track(index: u32, class: InstrumentClass, settings: &str) -> PreparedTrack {
    PreparedTrack {
        info: TrackInfo {
            index,
            name: format!("Track {}", index + 1),
            class,
        },
        program: if class == InstrumentClass::Drums {
            0
        } else {
            29
        },
        percussion: class == InstrumentClass::Drums,
        tuning: if class == InstrumentClass::Fretted {
            vec![64, 59, 55, 50, 45, 40]
        } else {
            Vec::new()
        },
        capo: 0,
        settings: settings.into(),
    }
}

/// Buffers for two tracks.
struct Pieces {
    buffers: Vec<Vec<f32>>,
    events: Vec<RtEvent>,
}

impl Pieces {
    fn new() -> Pieces {
        Pieces {
            buffers: (0..4).map(|_| vec![0.0; FRAMES]).collect(),
            events: Vec::with_capacity(8),
        }
    }

    /// One render call of `frames` frames with `events`; returns whether
    /// the plugin answered without failing.
    fn render(&mut self, instance: &mut Instance, events: &[RtEvent], frames: usize) -> bool {
        self.events.clear();
        self.events.extend_from_slice(events);
        for b in &mut self.buffers {
            b[..frames].fill(0.0);
        }
        let step = 1.0 / f64::from(RATE);
        instance.render(
            &self.events,
            &[],
            0.0,
            step,
            frames,
            self.buffers.iter_mut().map(|b| &mut b[..frames]),
        )
    }

    /// The first frame of track `slot` that isn't silent.
    fn first_sound(&self, slot: usize, frames: usize) -> Option<usize> {
        let (l, r) = (&self.buffers[slot * 2], &self.buffers[slot * 2 + 1]);
        (0..frames).find(|&i| l[i].abs() > 1e-4 || r[i].abs() > 1e-4)
    }
}

fn note(id: u32, track: u32, key: u8) -> RtNote {
    RtNote {
        id,
        track,
        key,
        velocity: 100,
        start: 0.0,
        end: 1.0,
        ..RtNote::default()
    }
}

fn event(kind: u32, note: RtNote) -> RtEvent {
    RtEvent { kind, note }
}

/// The script every instrument plays: silence, a note at the start of a
/// piece, a second note handed over in a call without frames, then
/// everything off (whatever happened before). Returns why it failed, if it
/// did.
fn script(instance: &mut Instance, pieces: &mut Pieces, resolution: usize) -> Result<(), String> {
    let played = notes(instance, pieces, resolution);
    let off = [event(RtEventKind::ALL_OFF, RtNote::default())];
    let stopped = pieces.render(instance, &off, FRAMES);
    match (played, stopped) {
        (Err(e), _) => Err(e),
        (Ok(()), false) => Err("it failed stopping its notes".into()),
        (Ok(()), true) => Ok(()),
    }
}

fn notes(instance: &mut Instance, pieces: &mut Pieces, resolution: usize) -> Result<(), String> {
    let a = note(1, 0, 64);
    let b = note(2, 1, 67);
    if !pieces.render(instance, &[], 128) {
        return Err("it failed rendering silence".into());
    }
    if pieces.first_sound(0, 128).is_some() || pieces.first_sound(1, 128).is_some() {
        return Err("it sounds before any note".into());
    }
    if !pieces.render(instance, &[event(RtEventKind::NOTE_ON, a)], 128) {
        return Err("it failed starting a note".into());
    }
    match pieces.first_sound(0, 128) {
        Some(i) if i <= resolution => {}
        Some(i) => {
            return Err(format!(
                "a note asked for on frame 0 started on frame {i}, later than its resolution of {resolution}"
            ));
        }
        None => return Err("a note asked for on frame 0 wasn't heard in 128 frames".into()),
    }
    if !pieces.render(instance, &[event(RtEventKind::NOTE_ON, b)], 0)
        || !pieces.render(instance, &[], 128)
    {
        return Err("it failed taking a note in a call without frames".into());
    }
    match pieces.first_sound(1, 128) {
        Some(i) if i <= resolution => {}
        _ => return Err("a note handed over in a call without frames wasn't heard on time".into()),
    }
    let off = [
        event(RtEventKind::NOTE_OFF, a),
        event(RtEventKind::NOTE_OFF, b),
    ];
    if !pieces.render(instance, &off, FRAMES) {
        return Err("it failed ending its notes".into());
    }
    Ok(())
}

/// The real-time checks of every instrument and effect (interface 1.3).
fn realtime_checks(plugins: &Plugins, report: &mut Report, settings: &str) {
    for class in InstrumentClass::ALL {
        for instrument in plugins.playable(class) {
            // Each instrument once, for the first class it plays.
            if InstrumentClass::ALL
                .iter()
                .take_while(|&&c| c != class)
                .any(|&c| instrument.spec.plays(c))
            {
                continue;
            }
            let id = instrument.spec.id.clone();
            let prepare = Prepare {
                instrument: id.clone(),
                rate: RATE,
                frames: FRAMES as u32,
                tracks: vec![
                    checked_track(0, class, settings),
                    checked_track(1, class, settings),
                ],
            };
            let mut instance = match plugins.prepare(instrument, prepare) {
                Ok(i) => {
                    report.check(
                        format!("instrument {id}: it prepares for two tracks"),
                        Ok(()),
                    );
                    i
                }
                Err(e) => {
                    report.check(
                        format!("instrument {id}: it prepares for two tracks"),
                        Err(e.message),
                    );
                    continue;
                }
            };
            let resolution = instance.resolution() as usize;
            let mut pieces = Pieces::new();
            // A first pass warms it up (what it makes on first use); the
            // second is counted.
            let _ = script(&mut instance, &mut pieces, resolution);
            let before = instance.allocations();
            let played = script(&mut instance, &mut pieces, resolution);
            let after = instance.allocations();
            report.check(
                format!("instrument {id}: notes start within its resolution ({resolution} frames)"),
                played,
            );
            report.check(
                format!("instrument {id}: it allocates nothing while rendering"),
                allocations(before, after),
            );
            report.check(
                format!("instrument {id}: it makes no system call while rendering"),
                watched(|| {
                    let _ = script(&mut instance, &mut pieces, resolution);
                }),
            );
        }
        for effect in plugins.effects_for(class) {
            if InstrumentClass::ALL
                .iter()
                .take_while(|&&c| c != class)
                .any(|&c| effect.spec.plays(c))
            {
                continue;
            }
            let id = effect.spec.id.clone();
            let prepare = PrepareEffect {
                effect: id.clone(),
                rate: RATE,
                frames: FRAMES as u32,
                track: checked_track(0, class, settings),
            };
            let mut instance = match plugins.prepare_effect(effect, prepare) {
                Ok(i) => {
                    report.check(format!("effect {id}: it prepares"), Ok(()));
                    i
                }
                Err(e) => {
                    report.check(format!("effect {id}: it prepares"), Err(e.message));
                    continue;
                }
            };
            let (mut l, mut r) = (vec![0.5f32; FRAMES], vec![-0.5f32; FRAMES]);
            let _ = instance.process(&mut l, &mut r);
            let before = instance.allocations();
            let ran = instance.process(&mut l, &mut r);
            let after = instance.allocations();
            report.check(
                format!("effect {id}: it runs"),
                if ran { Ok(()) } else { Err("it failed".into()) },
            );
            report.check(
                format!("effect {id}: it allocates nothing while running"),
                allocations(before, after),
            );
            report.check(
                format!("effect {id}: it makes no system call while running"),
                watched(|| {
                    let _ = instance.process(&mut l, &mut r);
                }),
            );
        }
    }
}

fn allocations(before: u64, after: u64) -> Result<(), String> {
    match (before, after) {
        (u64::MAX, _) | (_, u64::MAX) => {
            // Built without the SDK's allocator: nothing to count with.
            Ok(())
        }
        (b, a) if a == b => Ok(()),
        (b, a) => Err(format!(
            "{} allocations while rendering",
            a.saturating_sub(b)
        )),
    }
}

/// Runs `f` under the system-call watch where there is one (Linux).
fn watched(f: impl FnOnce()) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        match syscalls::watch(f) {
            syscalls::Watched::Clean => Ok(()),
            syscalls::Watched::Called(nr) => Err(format!("system call number {nr}")),
            syscalls::Watched::Stuck => Err("it never finished: it waited for something".into()),
            // A machine that can't set up the watch (no seccomp) checks
            // nothing here.
            syscalls::Watched::Unwatched(_) => Ok(()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = f;
        Ok(())
    }
}

/// One thing a user could do to each widget of `content`: a click, a tick,
/// the slider's lowest value, an empty field, the first option, and a
/// press, drag and release in the middle of a canvas.
fn plausible_events(content: &TabContent) -> Vec<(String, TabEvent)> {
    let mut out = Vec::new();
    for widget in content.rows.iter().flat_map(|r| &r.widgets) {
        let Some(id) = widget.id().map(str::to_owned) else {
            continue;
        };
        match widget {
            Widget::Button { .. } => out.push((id, TabEvent::Clicked)),
            Widget::Toggle { on, .. } => out.push((id, TabEvent::Toggled(!on))),
            Widget::Slider { min, .. } => out.push((id, TabEvent::Slid(*min))),
            Widget::Field { .. } => out.push((id, TabEvent::Edited(String::new()))),
            Widget::Choice { options, .. } if !options.is_empty() => {
                out.push((id, TabEvent::Chosen(0)));
            }
            Widget::List { items, .. } if !items.is_empty() => {
                out.push((id, TabEvent::Chosen(0)));
            }
            Widget::Canvas(c) => {
                let at = Point::new(c.width / 2.0, c.height / 2.0);
                for kind in [PointerKind::Down, PointerKind::Drag, PointerKind::Up] {
                    out.push((
                        id.clone(),
                        TabEvent::Pointer(Pointer {
                            kind,
                            at,
                            button: PointerButton::Primary,
                        }),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// An error from a plugin is fine; a crash or a request it couldn't decode
/// breaks the contract.
fn not_a_crash(e: &Error) -> Result<(), String> {
    if e.message.contains("it crashed") || e.message.contains("couldn't read the request") {
        Err(e.message.clone())
    } else {
        Ok(())
    }
}
