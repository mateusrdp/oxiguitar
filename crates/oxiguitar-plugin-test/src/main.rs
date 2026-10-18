// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-plugin-test`: checks plugin files against the interface.
//!
//! ```text
//! oxiguitar-plugin-test FILE.oxp...
//! oxiguitar-plugin-test --build PACKAGE NAME [--release]
//! ```
//!
//! The second form builds the plugin crate `PACKAGE` of the workspace in
//! the current folder as `target/oxp/NAME.oxp`, then checks it.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<PathBuf> = match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => {
            println!("usage: oxiguitar-plugin-test FILE.oxp...");
            println!("       oxiguitar-plugin-test --build PACKAGE NAME [--release]");
            return ExitCode::SUCCESS;
        }
        Some("--build") => {
            let (Some(package), Some(name)) = (args.get(1), args.get(2)) else {
                eprintln!("--build needs a package and a plugin name");
                return ExitCode::from(2);
            };
            let release = args.iter().any(|a| a == "--release");
            let here = std::env::current_dir().unwrap_or_default();
            match oxiguitar_plugin_test::build(&here, package, name, release) {
                Ok(path) => vec![path],
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Some(_) => args.iter().map(PathBuf::from).collect(),
    };
    let mut ok = true;
    for file in files {
        println!("{}", file.display());
        let report = oxiguitar_plugin_test::check(&file);
        for p in &report.passed {
            println!("  ok    {p}");
        }
        for f in &report.failed {
            println!("  FAIL  {f}");
        }
        for m in &report.messages {
            println!("  said  {m}");
        }
        println!(
            "  manifest read in {:.2} ms",
            report.manifest_time.as_secs_f64() * 1000.0
        );
        ok &= report.ok();
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
