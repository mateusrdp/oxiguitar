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

/// The host over our plugins, built on first use.
pub(crate) fn test_plugins() -> &'static Plugins {
    static PLUGINS: OnceLock<Plugins> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let folder = oxiguitar_plugin_test::build_ours(&workspace, !cfg!(debug_assertions))
            .expect("our plugins build");
        Plugins::discover(
            &[folder],
            SharedSettings::default(),
            oxiguitar_host::display_system(),
            Arc::new(Unheard),
        )
    })
}
