// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Compiles the window's Slint files and, for Windows, the program's icon
//! and version information.

#[path = "build/winres.rs"]
mod winres;

use std::path::PathBuf;

fn main() {
    // The Fluent style on every system, so the app looks the same everywhere
    // and screenshots compare like with like.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    if let Err(e) = slint_build::compile_with_config("ui/app.slint", config) {
        panic!("ui/app.slint does not compile: {e}");
    }

    // Windows takes a program's icon (in Explorer, the taskbar and the Start
    // menu) and its name in Task Manager from resources linked into the .exe.
    // We write the resource file ourselves and hand it to the MSVC linker,
    // which accepts .res files directly: no resource compiler is needed.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os == "windows" && env == "msvc" {
        let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"));
        let png = root.join("../../assets/icon/png");
        let mut icons = Vec::new();
        for size in winres::ICON_SIZES {
            let path = png.join(format!("oxiguitar-{size}.png"));
            println!("cargo:rerun-if-changed={}", path.display());
            let data = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            icons.push((size, data));
        }
        let version = [
            "CARGO_PKG_VERSION_MAJOR",
            "CARGO_PKG_VERSION_MINOR",
            "CARGO_PKG_VERSION_PATCH",
        ]
        .map(|k| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
        });
        let res = winres::resource_file(
            &icons,
            &winres::VersionInfo {
                version: [version[0], version[1], version[2], 0],
                version_text: std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
                description: "OxiGuitar",
                product: "OxiGuitar",
                copyright: "Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors",
                original_filename: "oxiguitar.exe",
            },
        );
        let out =
            PathBuf::from(std::env::var("OUT_DIR").expect("set by Cargo")).join("oxiguitar.res");
        std::fs::write(&out, res).unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
        println!("cargo:rustc-link-arg-bin=oxiguitar={}", out.display());
    }
    println!("cargo:rerun-if-changed=build/winres.rs");
}
