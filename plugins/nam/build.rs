// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Sets `cfg(x86_64_v3)` when the feature `x86-64-v3` is on and the build
//! can use NeuralAmpModeler-rs: Linux (glibc) on x86_64, with the eight
//! features of the x86-64-v3 level enabled (RUSTFLAGS="-C
//! target-cpu=x86-64-v3"). Cargo.toml brings the library in on Linux
//! x86_64 with the feature; without those processor features its own build
//! script stops the build, so this never sees the feature on without them
//! there.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(x86_64_v3)");
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    let features = var("CARGO_CFG_TARGET_FEATURE");
    let has = |f: &str| features.split(',').any(|g| g == f);
    let level = [
        "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "lzcnt", "movbe",
    ];
    if std::env::var_os("CARGO_FEATURE_X86_64_V3").is_some()
        && var("CARGO_CFG_TARGET_OS") == "linux"
        && var("CARGO_CFG_TARGET_ENV") == "gnu"
        && var("CARGO_CFG_TARGET_ARCH") == "x86_64"
        && level.iter().all(|f| has(f))
    {
        println!("cargo::rustc-cfg=x86_64_v3");
    }
}
