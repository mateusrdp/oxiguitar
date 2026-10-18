// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What the app's tests share: our format plugins, built as `.oxp` files
//! (debug builds for `cargo test`, release builds for `cargo test
//! --release`, so the speed budget includes the real plugin) and found by
//! one host. A system loads a library file once per process, so every test
//! uses this one.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use oxiguitar_host::{Plugins, SharedSettings, Unheard};

/// The folder holding our plugins' `.oxp` files, built once for the whole
/// test program: building replaces the files, which Windows refuses while
/// a host has them loaded.
fn ours() -> &'static Path {
    static BUILT: OnceLock<std::path::PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        oxiguitar_plugin_test::build_ours(&workspace, !cfg!(debug_assertions))
            .expect("our plugins build")
    })
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
        std::fs::copy(ours.join("gp.oxp"), folder.join("gp.oxp")).expect("gp.oxp copied");
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
