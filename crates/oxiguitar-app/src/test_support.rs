// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the app's tests share: the released Guitar Pro plugins (#165: the
//! plugins live in repositories of their own), found by one host. A system loads a library file once per process, so every test
//! uses this one.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use oxiguitar_host::{Plugins, SharedSettings, Unheard};

/// The folder holding the released Guitar Pro plugins' `.oxp` files, had
/// once for the whole test program.
fn ours() -> &'static Path {
    static FOLDER: OnceLock<std::path::PathBuf> = OnceLock::new();
    FOLDER.get_or_init(|| oxiguitar_plugin_test::released::folder(&["guitarpro"]))
}

/// The host over our plugins, built on first use.
pub(crate) fn test_plugins() -> &'static Plugins {
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

/// A second host for the plugin tab and event tests: copies (so they load
/// apart from [`test_plugins`]'s) of our Guitar Pro 7–8 plugin and of the
/// test plugin, which has tabs and hears every event. It hands plugins the
/// score the worker keeps, and keeps the redraws they ask for.
pub(crate) struct TabHost {
    pub(crate) plugins: Plugins,
    pub(crate) open: crate::document::OpenScore,
    pub(crate) redraws: Arc<std::sync::Mutex<Vec<oxiguitar_host::TabId>>>,
}

struct Recorder {
    open: crate::document::OpenScore,
    redraws: Arc<std::sync::Mutex<Vec<oxiguitar_host::TabId>>>,
}

impl oxiguitar_host::Core for Recorder {
    fn notify(&self, _level: oxiguitar_host::Level, _text: String) {}

    fn redraw(&self, tab: &oxiguitar_host::TabId) {
        if let Ok(mut r) = self.redraws.lock() {
            r.push(tab.clone());
        }
    }

    fn score(&self) -> Option<oxiguitar_host::api::score::Score> {
        self.open.wire()
    }

    fn score2(&self) -> Option<(u64, oxiguitar_host::api::score2::Score2)> {
        self.open.wire2()
    }

    fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: oxiguitar_host::api::score2::Score2,
    ) -> Result<(), String> {
        self.open.apply(plugin, revision, name, score)
    }
}

/// The test plugin's `.oxp` file, built once for the whole test program:
/// building writes the file afresh, so tests building it at the same time
/// would link or copy a file half written.
pub(crate) fn test_oxp() -> &'static Path {
    static BUILT: OnceLock<std::path::PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        oxiguitar_plugin_test::build(
            &workspace,
            "oxiguitar-test-plugin",
            "test",
            !cfg!(debug_assertions),
        )
        .expect("the test plugin builds")
    })
}

/// The [`TabHost`], built on first use.
pub(crate) fn tab_host() -> &'static TabHost {
    static HOST: OnceLock<TabHost> = OnceLock::new();
    HOST.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ours = ours();
        let test = test_oxp();
        let folder = workspace.join("target").join("oxp").join("app-tabs");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("a folder for the copies");
        // One Guitar Pro plugin is enough for the tab tests: the 7-8 one
        // (released as gp-VERSION-SYSTEM-ARCH.oxp).
        let gp = std::fs::read_dir(ours)
            .expect("the plugin folder reads")
            .map(|e| e.expect("an entry").path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("gp-") && n.ends_with(".oxp"))
            })
            .expect("the released Guitar Pro 7-8 plugin");
        std::fs::copy(&gp, folder.join("gp.oxp")).expect("gp.oxp copied");
        std::fs::copy(test, folder.join("test.oxp")).expect("test.oxp copied");
        let open = crate::document::OpenScore::default();
        let redraws = Arc::default();
        let plugins = Plugins::discover(
            &[folder],
            SharedSettings::default(),
            oxiguitar_host::display_system(),
            Arc::new(Recorder {
                open: open.clone(),
                redraws: Arc::clone(&redraws),
            }),
        );
        TabHost {
            plugins,
            open,
            redraws,
        }
    })
}

/// The test plugin's tab `panel`.
pub(crate) fn panel() -> oxiguitar_host::TabId {
    oxiguitar_host::TabId {
        plugin: "oxiguitar.test".into(),
        tab: "panel".into(),
    }
}
