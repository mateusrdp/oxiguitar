// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Our own plugins, built as the `.oxp` files we ship, through the host:
//! the contract, each plugin reading its own format and nothing else, every
//! public Guitar Pro file (and the private corpus, when `corpus/files/` is
//! there) read exactly as the reader reads it in-process, and the refusals
//! when no plugin recognises a file (Mateus, 18 Oct 2026).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use oxiguitar_host::api::Blob;
use oxiguitar_host::api::message::{Answer, ErrorKind, LogEntry, Request};
use oxiguitar_host::{Plugins, SharedSettings, Unheard, Unrecognised};
use oxiguitar_plugin_test::{
    OUR_DEVICES, OUR_EFFECTS, OUR_EXPORTERS, OUR_INSTRUMENTS, OURS, build_devices, build_ours,
    check, check_with,
};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Our plugins, built once for every test here (debug builds).
fn ours() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| build_ours(&workspace(), false).expect("our plugins build"))
}

/// One host over our plugins for every test that loads them: a system
/// loads a library file once per process.
fn plugins() -> &'static Plugins {
    static PLUGINS: OnceLock<Plugins> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        Plugins::discover(
            &[ours().to_owned()],
            SharedSettings::default(),
            oxiguitar_host::display_system(),
            Arc::new(Unheard),
        )
    })
}

fn fixture(ext: &str) -> PathBuf {
    workspace()
        .join("fixtures/gp")
        .join(format!("oxiguitar-test-song.{ext}"))
}

#[test]
fn our_plugins_pass_the_contract() {
    for (_, name) in OURS {
        let report = check(&ours().join(format!("{name}.oxp")));
        assert!(report.ok(), "{name}.oxp: {:#?}", report.failed);
        assert!(report.passed.len() >= 6, "{name}.oxp: {:#?}", report.passed);
    }
}

#[test]
fn our_instruments_pass_the_contract() {
    for (_, name) in OUR_INSTRUMENTS {
        let report = check(&ours().join(format!("{name}.oxp")));
        assert!(report.ok(), "{name}.oxp: {:#?}", report.failed);
        assert!(
            report
                .passed
                .iter()
                .any(|p| p.contains("notes start within its resolution")),
            "{name}.oxp: {:#?}",
            report.passed
        );
    }
}

/// The NAM plugin with a model in its `nam` folder: a host over a copy of
/// it (a library loads once per process), and that folder.
fn nam_plugins(tag: &str) -> (Plugins, PathBuf) {
    nam_plugins_of(
        &[
            (ours().join("nam.oxp"), format!("nam-{tag}.oxp")),
            (ours().join("midi.oxp"), "midi.oxp".into()),
        ],
        tag,
    )
}

/// Like [`nam_plugins`], with the plugin files `files` (each copied under
/// its new name) in the folder.
fn nam_plugins_of(files: &[(PathBuf, String)], tag: &str) -> (Plugins, PathBuf) {
    let scratch = std::env::temp_dir().join(format!("oxiguitar-nam-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let folder = scratch.join("plugins");
    std::fs::create_dir_all(&folder).unwrap();
    for (file, name) in files {
        std::fs::copy(file, folder.join(name)).unwrap();
    }
    let home = scratch.join("home");
    std::fs::create_dir_all(home.join("nam/Amps")).unwrap();
    // The MIDI plugin NAM needs is checked too, and plays from a set.
    std::fs::create_dir_all(home.join("soundfonts")).unwrap();
    std::fs::write(
        home.join("soundfonts/test-sine.sf2"),
        oxiguitar_plugin_test::TEST_SET,
    )
    .unwrap();
    std::fs::copy(
        workspace().join("fixtures/nam/wavenet.nam"),
        home.join("nam/Amps/wavenet.nam"),
    )
    .unwrap();
    let plugins = Plugins::discover(
        &[folder],
        SharedSettings::default(),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let folders = oxiguitar_host::Folders::resolve(
        oxiguitar_host::System::this(),
        |name| (name == oxiguitar_host::HOME_VARIABLE).then(|| home.clone().into()),
        None,
    )
    .unwrap();
    plugins.use_folders(folders);
    (plugins, scratch)
}

/// The NAM plugin passes the contract, and plays a model and a built-in
/// cabinet through the suite's real-time checks: nothing allocated and
/// (on Linux) no system call made while it runs (M4 slice 6).
#[test]
fn our_nam_plugin_runs_a_model_in_real_time() {
    for (_, name) in OUR_EFFECTS {
        let report = check_with(
            &ours().join(format!("{name}.oxp")),
            &[&ours().join("midi.oxp")],
        );
        assert!(report.ok(), "{name}.oxp: {:#?}", report.failed);
    }
    let (plugins, scratch) = nam_plugins("realtime");
    let settings =
        r#"{"on": true, "model": "Amps/wavenet.nam", "cabinet": "built-in:evh-5150-iii"}"#;
    let report = oxiguitar_plugin_test::check_realtime(&plugins, settings);
    assert!(report.ok(), "{:#?}", report.failed);
    for wanted in [
        "effect nam: it prepares",
        "effect nam: it allocates nothing",
    ] {
        assert!(
            report.passed.iter().any(|p| p.contains(wanted)),
            "{wanted}: {:#?}",
            report.passed
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// At a rate other than the model's, the track goes to 48 kHz and back:
/// it says its latency, makes sound, and allocates nothing while running;
/// a model that isn't there is refused with its name.
#[test]
fn our_nam_plugin_converts_the_rate_for_its_model() {
    use oxiguitar_host::api::class::InstrumentClass;
    use oxiguitar_host::api::message::{PrepareEffect, PreparedTrack, TrackInfo};

    let (plugins, scratch) = nam_plugins("rate");
    let effect = plugins.effects_for(InstrumentClass::Fretted)[0];
    let prepare = |settings: &str| PrepareEffect {
        effect: "nam".into(),
        rate: 44_100,
        frames: 512,
        track: PreparedTrack {
            info: TrackInfo {
                index: 0,
                name: "Lead".into(),
                class: InstrumentClass::Fretted,
            },
            program: 29,
            percussion: false,
            tuning: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
            settings: settings.into(),
        },
    };
    let mut amp = plugins
        .prepare_effect(
            effect,
            prepare(r#"{"on": true, "model": "Amps/wavenet.nam"}"#),
        )
        .unwrap();
    assert!(
        amp.latency() > 16,
        "the conversion's latency: {}",
        amp.latency()
    );
    let mut peak = 0.0f32;
    let mut allocated = 0;
    for k in 0..200 {
        let mut l: Vec<f32> = (0..300)
            .map(|n| (((k * 300 + n) as f32) * 0.06).sin() * 0.3)
            .collect();
        let mut r = l.clone();
        let before = amp.allocations();
        assert!(amp.process(&mut l, &mut r));
        if k > 0 {
            allocated += amp.allocations().saturating_sub(before);
        }
        peak = l.iter().fold(peak, |p, v| p.max(v.abs()));
        assert_eq!(l, r, "the same on both sides");
    }
    assert!(peak > 0.01, "it makes sound: {peak}");
    assert_eq!(allocated, 0, "nothing allocated while running");
    let off = plugins
        .prepare_effect(
            effect,
            prepare(r#"{"on": false, "model": "Amps/wavenet.nam"}"#),
        )
        .unwrap();
    assert_eq!(off.latency(), 0);
    let missing = plugins
        .prepare_effect(effect, prepare(r#"{"on": true, "model": "gone.nam"}"#))
        .unwrap_err();
    assert!(missing.to_string().contains("gone.nam"), "{missing}");
    let escaping = plugins
        .prepare_effect(effect, prepare(r#"{"on": true, "model": "../../x.nam"}"#))
        .unwrap_err();
    assert!(
        escaping.to_string().contains("isn't in the nam folder"),
        "{escaping}"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A mono 16-bit WAV file of a 440 Hz sine at half scale, `seconds`
/// long, at 48 kHz.
fn sine_wav(seconds: f64) -> Vec<u8> {
    let rate = 48_000u32;
    let frames = (seconds * f64::from(rate)) as usize;
    let mut data = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = (std::f64::consts::TAU * 440.0 * i as f64 / f64::from(rate)).sin() * 16_384.0;
        data.extend_from_slice(&(v as i16).to_le_bytes());
    }
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * 2).to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let mut body = b"WAVE".to_vec();
    for (id, chunk) in [(b"fmt ", fmt), (b"data", data)] {
        body.extend_from_slice(id);
        body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        body.extend_from_slice(&chunk);
    }
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    file
}

/// The SoundFont plugin plays an SFZ instrument picked in its settings
/// through the contract suite's real-time checks: notes start on time,
/// nothing is allocated and (on Linux) no system call made while it
/// renders (M4 slice 5).
#[test]
fn our_midi_plugin_plays_sfz_in_real_time() {
    let scratch =
        std::env::temp_dir().join(format!("oxiguitar-sfz-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let folder = scratch.join("plugins");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(ours().join("midi.oxp"), folder.join("midi.oxp")).unwrap();
    let home = scratch.join("home");
    let set = home.join("soundfonts").join("Test Sine");
    std::fs::create_dir_all(set.join("Samples")).unwrap();
    std::fs::write(set.join("Samples/sine.wav"), sine_wav(0.5)).unwrap();
    std::fs::write(
        set.join("sine.sfz"),
        "<control> default_path=Samples/\n\
         <group> ampeg_release=0.01 loop_mode=loop_continuous loop_start=0 loop_end=23999\n\
         <region> sample=sine.wav pitch_keycenter=69 hivel=80\n\
         <region> sample=sine.wav pitch_keycenter=69 lovel=81 seq_length=2 seq_position=1\n\
         <region> sample=sine.wav pitch_keycenter=69 lovel=81 seq_length=2 seq_position=2 volume=-1\n",
    )
    .unwrap();
    let pick = std::fs::canonicalize(set.join("sine.sfz"))
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let settings = SharedSettings::default();
    settings
        .set(
            "oxiguitar.midi",
            serde_json::json!({"guitars": pick, "basses": pick, "drums": pick, "other": pick}),
        )
        .unwrap();
    let plugins = Plugins::discover(
        &[folder],
        settings,
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let folders = oxiguitar_host::Folders::resolve(
        oxiguitar_host::System::this(),
        |name| (name == oxiguitar_host::HOME_VARIABLE).then(|| home.clone().into()),
        None,
    )
    .unwrap();
    plugins.use_folders(folders);
    let report = oxiguitar_plugin_test::check_realtime(&plugins, "{}");
    assert!(report.ok(), "{:#?}", report.failed);
    for wanted in ["notes start within", "allocates nothing"] {
        assert!(
            report.passed.iter().any(|p| p.contains(wanted)),
            "{wanted}: {:#?}",
            report.passed
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn the_list_of_our_plugins_is_the_plugins_folder() {
    let mut folders: Vec<String> = std::fs::read_dir(workspace().join("plugins"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        // The test plugins, never shipped.
        .filter(|n| n != "test" && !n.starts_with("test-"))
        .collect();
    folders.sort();
    let mut listed: Vec<String> = OURS
        .iter()
        .map(|(_, n)| *n)
        .chain(OUR_INSTRUMENTS.iter().map(|(_, n)| *n))
        .chain(OUR_EFFECTS.iter().map(|(_, n)| *n))
        .chain(OUR_EXPORTERS.iter().map(|(_, n)| *n))
        .chain(OUR_DEVICES.iter().map(|(_, n, _)| *n))
        .map(str::to_owned)
        .collect();
    listed.sort();
    assert_eq!(folders, listed);
    for (package, name) in OURS
        .iter()
        .chain(OUR_INSTRUMENTS)
        .chain(OUR_EFFECTS)
        .chain(OUR_EXPORTERS)
    {
        assert_eq!(*package, format!("oxiguitar-{name}-plugin"));
    }
    for (package, name, os) in OUR_DEVICES {
        assert_eq!(*package, format!("oxiguitar-{name}-plugin"));
        let manifest =
            std::fs::read_to_string(workspace().join("plugins").join(name).join("manifest.json"))
                .unwrap();
        assert!(
            manifest.contains(&format!("\"{os}\"")),
            "{name} runs on {os}"
        );
    }
}

/// The audio devices that run here pass the contract. Their probes answer
/// whether or not this machine has a sound card (CI's have none).
#[test]
fn our_audio_devices_pass_the_contract() {
    let (folder, names) = build_devices(&workspace(), false).expect("our audio devices build");
    for name in names {
        let report = check(&folder.join(&name));
        assert!(report.ok(), "{name}: {:#?}", report.failed);
    }
}

#[test]
fn each_plugin_recognises_and_reads_its_own_format_and_nothing_else() {
    let plugins = plugins();
    for (_, name) in OURS {
        let data = std::fs::read(fixture(name)).unwrap();
        let format = plugins
            .recognise(&data, Some(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            format.plugin.manifest.as_ref().unwrap().id,
            format!("oxiguitar.{name}")
        );
        let read = plugins.read(format, &data).unwrap();
        assert_eq!(read.score.info.title, "OxiGuitar Test Song", "{name}");
        // Every other plugin, asked directly, refuses the file: it holds
        // only its own reader.
        for (_, other) in OURS.iter().filter(|(_, o)| o != name) {
            let found = plugins
                .found()
                .iter()
                .find(|f| {
                    f.manifest
                        .as_ref()
                        .is_some_and(|m| m.id == format!("oxiguitar.{other}"))
                })
                .unwrap();
            let answer = plugins.send(
                found,
                &Request::Read {
                    format: (*other).to_owned(),
                    data: Blob::borrowed(&data),
                },
            );
            match answer {
                Err(e) => assert_eq!(e.kind, ErrorKind::Unsupported, "{other} on {name}: {e}"),
                Ok(Answer::Read { .. }) => panic!("{other}.oxp read a .{name} file"),
                Ok(other) => panic!("{other:?}"),
            }
        }
    }
    assert_eq!(
        plugins.read_extensions(),
        ["gp", "gp3", "gp4", "gp5", "gpx"],
        "File → Open's filter, in plugin order"
    );
}

/// What a reader made of a file: the score and its log, or the error's
/// sentence.
type Outcome = Result<(oxiguitar_model::Score, Vec<LogEntry>), String>;

fn in_process(data: &[u8]) -> Outcome {
    oxiguitar_formats::read(data)
        .map(|i| {
            let log = oxiguitar_formats::plugin::log(&i.log);
            (i.score, log)
        })
        .map_err(|e| e.to_string())
}

fn through_plugins(data: &[u8]) -> Outcome {
    let plugins = plugins();
    let format = plugins.recognise(data, None).map_err(|e| e.to_string())?;
    // The host prefixes a plugin's error with the plugin's name.
    let prefix = format!("{}: ", format.plugin.name());
    let read = plugins.read(format, data).map_err(|e| {
        let text = e.to_string();
        text.strip_prefix(&prefix)
            .map_or(text.clone(), str::to_owned)
    })?;
    let score = oxiguitar_model::wire::from_wire2(read.score)?;
    Ok((score, read.log))
}

fn guitar_pro_files() -> Vec<PathBuf> {
    let root = workspace();
    let mut files = Vec::new();
    for dir in [
        "fixtures/gp",
        "fixtures/reference",
        "corpus/files/tabs",
        "corpus/files/scores",
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in entries {
            let path = e.unwrap().path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase);
            if matches!(ext.as_deref(), Some("gp" | "gpx" | "gp3" | "gp4" | "gp5")) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn the_plugins_read_every_file_exactly_as_the_readers_do() {
    let files = guitar_pro_files();
    assert!(
        files.len() >= 55,
        "the public fixtures and references are there"
    );
    let mut later_formats = 0;
    let mut later_opened = 0;
    for path in &files {
        let data = std::fs::read(path).unwrap();
        let direct = in_process(&data);
        let plugged = through_plugins(&data);
        assert!(
            direct == plugged,
            "{}: the plugin path differs",
            path.display()
        );
        let opened = plugged.is_ok();
        // M1's gate: Guitar Pro 5 and later, from the file's contents.
        if data.starts_with(b"\x18FICHIER GUITAR PRO v5")
            || data.starts_with(b"BCF")
            || data.starts_with(b"PK")
        {
            if path.starts_with(workspace().join("corpus")) {
                later_formats += 1;
                later_opened += usize::from(opened);
            }
        } else if path.starts_with(workspace().join("corpus")) {
            assert!(opened, "{}: {:?}", path.display(), plugged.err());
        }
    }
    if later_formats > 0 {
        println!(
            "corpus, Guitar Pro 5 and later: {later_opened} of {later_formats} open through the plugins"
        );
        assert_eq!((later_opened, later_formats), (42, 42));
    }
}

#[test]
fn a_file_no_plugin_recognises_is_refused_with_the_right_report() {
    let plugins = plugins();
    // Nothing declares .txt: a feature request.
    let e = plugins.recognise(b"hello", Some("txt")).unwrap_err();
    assert_eq!(e, Unrecognised::Unknown);
    assert!(e.to_string().contains("feature request"), "{e}");
    assert!(e.to_string().contains(oxiguitar_host::ISSUES_URL), "{e}");
    // Guitar Pro 1–2, which no plugin reads, the same way.
    let mut gtp = vec![24u8];
    gtp.extend_from_slice(b"FICHIER GUITARE PRO v1.04");
    gtp.resize(64, 0);
    assert_eq!(
        plugins.recognise(&gtp, Some("gtp")).unwrap_err(),
        Unrecognised::Unknown
    );
    // The Guitar Pro 5 plugin declares .gp5 but doesn't recognise this
    // one: a bug report.
    let e = plugins.recognise(b"hello", Some("GP5")).unwrap_err();
    assert_eq!(
        e,
        Unrecognised::NotRecognised {
            plugin: "Guitar Pro 5".into(),
            format: "Guitar Pro 5".into(),
            extension: "gp5".into(),
        }
    );
    assert!(e.to_string().contains("report a bug"), "{e}");
    // The extension never decides: a Guitar Pro 5 file named .gp3 is
    // read as Guitar Pro 5 (four such files are in the corpus).
    let gp5 = std::fs::read(fixture("gp5")).unwrap();
    let format = plugins.recognise(&gp5, Some("gp3")).unwrap();
    assert_eq!(format.spec.id, "gp5");
}

#[test]
fn a_disabled_plugin_and_no_plugin_at_all() {
    // Only manifests are read here, so the files can be shared.
    let mut settings = oxiguitar_host::Settings::default();
    settings.set("disabled", serde_json::json!(["oxiguitar.gp5"]));
    let disabled = Plugins::discover(
        &[ours().to_owned()],
        SharedSettings::new(settings, None),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let e = disabled.recognise(b"hello", Some("gp5")).unwrap_err();
    assert!(
        matches!(&e, Unrecognised::NotUsable { plugin, .. } if plugin == "Guitar Pro 5"),
        "{e:?}"
    );
    assert!(e.to_string().contains("disabled"), "{e}");
    // A real .gp5 file isn't opened by the disabled plugin either.
    let gp5 = std::fs::read(fixture("gp5")).unwrap();
    assert!(disabled.recognise(&gp5, Some("gp5")).is_err());

    let empty = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ours-no-plugins");
    std::fs::create_dir_all(&empty).unwrap();
    let none = Plugins::discover(
        &[empty],
        SharedSettings::default(),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let e = none.recognise(&gp5, Some("gp5")).unwrap_err();
    assert_eq!(e, Unrecognised::NoReaders);
    assert!(e.to_string().contains(oxiguitar_host::PLUGINS_URL), "{e}");
    assert!(none.read_extensions().is_empty());
}

#[test]
fn format_plugins_add_no_tabs_and_hear_no_events() {
    use oxiguitar_host::api::event::{Event, EventKind};
    let plugins = plugins();
    assert!(plugins.tabs().is_empty());
    for kind in [EventKind::Score, EventKind::Selection, EventKind::Playback] {
        assert!(!plugins.wants(kind));
    }
    // Pushed anyway: nobody is told, and nothing waits.
    plugins.push(Event::ScoreClosed);
    plugins.wait_for_events();
}

/// The NAM plugin's build for x86-64-v3 processors (nam-x86-64-v3.oxp, made
/// by scripts/build-plugins.sh on Linux x86_64 with Rust 1.99), when
/// `OXIGUITAR_TEST_NAM_X86_64_V3` names it (CI's "NAM for x86-64-v3" job):
/// it passes the contract and the real-time checks as nam.oxp does, comes a
/// cabinet's partition (64 frames) later than it, and the host uses it
/// rather than nam.oxp, whatever the order of their names (the
/// processor-features step of the choice: docs/design/plugins.md). Without
/// the variable there is nothing to test.
#[test]
fn the_nam_build_for_x86_64_v3_processors_runs_in_real_time_and_is_chosen() {
    use oxiguitar_host::State;
    use oxiguitar_host::api::class::InstrumentClass;
    use oxiguitar_host::api::message::{PrepareEffect, PreparedTrack, TrackInfo};

    let Some(v3) = std::env::var_os("OXIGUITAR_TEST_NAM_X86_64_V3") else {
        println!("OXIGUITAR_TEST_NAM_X86_64_V3 isn't set: nothing to test");
        return;
    };
    let v3 = PathBuf::from(v3);
    let report = check_with(&v3, &[&ours().join("midi.oxp")]);
    assert!(report.ok(), "{}: {:#?}", v3.display(), report.failed);

    // Named to come after nam.oxp: the host must still take it.
    let (plugins, scratch) = nam_plugins_of(
        &[
            (ours().join("nam.oxp"), "a-nam.oxp".into()),
            (v3.clone(), "b-nam.oxp".into()),
            (ours().join("midi.oxp"), "midi.oxp".into()),
        ],
        "v3",
    );
    let states: Vec<(String, &State)> = plugins
        .found()
        .iter()
        .filter(|f| f.path.file_name().is_some_and(|n| n != "midi.oxp"))
        .map(|f| {
            (
                f.path.file_name().unwrap().to_string_lossy().into_owned(),
                &f.state,
            )
        })
        .collect();
    assert_eq!(states.len(), 2, "{states:#?}");
    assert!(
        matches!(states[0].1, State::Replaced(p) if p.ends_with("b-nam.oxp")),
        "{states:#?}"
    );
    assert_eq!(states[1].1, &State::Ready, "{states:#?}");

    let settings =
        r#"{"on": true, "model": "Amps/wavenet.nam", "cabinet": "built-in:evh-5150-iii"}"#;
    let report = oxiguitar_plugin_test::check_realtime(&plugins, settings);
    assert!(report.ok(), "{:#?}", report.failed);
    assert!(
        report
            .passed
            .iter()
            .any(|p| p.contains("effect nam: it allocates nothing")),
        "{:#?}",
        report.passed
    );

    let effect = plugins.effects_for(InstrumentClass::Fretted)[0];
    let prepare = PrepareEffect {
        effect: "nam".into(),
        rate: 48_000,
        frames: 512,
        track: PreparedTrack {
            info: TrackInfo {
                index: 0,
                name: "Lead".into(),
                class: InstrumentClass::Fretted,
            },
            program: 29,
            percussion: false,
            tuning: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
            settings: settings.into(),
        },
    };
    let amp = plugins.prepare_effect(effect, prepare).unwrap();
    assert_eq!(amp.latency(), 64, "the cabinet's partition");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The WAV plugin passes the contract (its exporter through the suite's
/// exports and its settings window), and the song exported through it is a
/// WAV file of the default format, 24 bits at 48 kHz, as long as the song
/// and its tail, that sounds; at 16 bits and 44.1 kHz once its settings
/// say so (M4 slice 7).
#[test]
fn our_wav_plugin_exports_the_song() {
    use oxiguitar_play::export::{self, Plan};

    for (_, name) in OUR_EXPORTERS {
        let report = check(&ours().join(format!("{name}.oxp")));
        assert!(report.ok(), "{name}.oxp: {:#?}", report.failed);
        assert!(
            report.passed.iter().any(|p| p.contains("exporter wav")),
            "{:#?}",
            report.passed
        );
    }
    let scratch = std::env::temp_dir().join(format!("oxiguitar-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let folder = scratch.join("plugins");
    std::fs::create_dir_all(&folder).unwrap();
    for name in ["gp5", "midi", "wav"] {
        let file = format!("{name}.oxp");
        std::fs::copy(ours().join(&file), folder.join(&file)).unwrap();
    }
    let home = scratch.join("home");
    std::fs::create_dir_all(home.join("soundfonts")).unwrap();
    std::fs::write(
        home.join("soundfonts").join("test-sine.sf2"),
        oxiguitar_plugin_test::TEST_SET,
    )
    .unwrap();
    let settings = SharedSettings::default();
    let plugins = Plugins::discover(
        &[folder],
        settings.clone(),
        oxiguitar_host::display_system(),
        Arc::new(Unheard),
    );
    let folders = oxiguitar_host::Folders::resolve(
        oxiguitar_host::System::this(),
        |name| (name == oxiguitar_host::HOME_VARIABLE).then(|| home.clone().into()),
        None,
    )
    .unwrap();
    plugins.use_folders(folders);
    let song = workspace().join("fixtures/gp/oxiguitar-test-song.gp5");
    let score = oxiguitar_formats::read(&std::fs::read(&song).unwrap())
        .unwrap()
        .score;
    let plan = Plan::whole(&score);
    let length = export::length(&oxiguitar_play::Timeline::new(&score), &plan);
    let exporter = plugins.exporters()[0];
    assert_eq!(exporter.spec.extension, "wav");

    let read = |path: &Path| {
        let wav = std::fs::read(path).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize,
            wav.len() - 8
        );
        let rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
        let bits = u16::from_le_bytes([wav[34], wav[35]]);
        let data = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data, wav.len() - 44);
        (rate, bits, wav)
    };

    let out = scratch.join("song.wav");
    export::export(&plugins, exporter, &score, &plan, &out, &mut |_| true).unwrap();
    let (rate, bits, wav) = read(&out);
    assert_eq!((rate, bits), (48_000, 24));
    let frames = (wav.len() - 44) / 6;
    assert!(
        (frames as f64 / 48_000.0 - length).abs() < 0.1,
        "{frames} frames for {length} s"
    );
    let peak = wav[44..]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8).unsigned_abs())
        .max()
        .unwrap();
    assert!(peak > 80_000, "it sounds: {peak}");

    // The settings window's choices: 16 bits, 44.1 kHz.
    let wav_plugin = plugins
        .found()
        .iter()
        .find(|f| f.path.ends_with("wav.oxp"))
        .unwrap();
    for (widget, choice) in [("depth", 0), ("rate", 0)] {
        plugins
            .send(
                wav_plugin,
                &oxiguitar_host::api::message::Request::SettingsEvent {
                    widget: widget.into(),
                    event: oxiguitar_host::api::tab::TabEvent::Chosen(choice),
                },
            )
            .unwrap();
    }
    let out = scratch.join("song-cd.wav");
    export::export(&plugins, exporter, &score, &plan, &out, &mut |_| true).unwrap();
    let (rate, bits, wav) = read(&out);
    assert_eq!((rate, bits), (44_100, 16));
    let frames = (wav.len() - 44) / 4;
    assert!((frames as f64 / 44_100.0 - length).abs() < 0.1);
    let _ = std::fs::remove_dir_all(&scratch);
}
