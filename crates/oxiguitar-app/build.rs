// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Compiles the window's Slint files.

fn main() {
    // The Fluent style on every system, so the app looks the same everywhere
    // and screenshots compare like with like.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    if let Err(e) = slint_build::compile_with_config("ui/app.slint", config) {
        panic!("ui/app.slint does not compile: {e}");
    }
}
