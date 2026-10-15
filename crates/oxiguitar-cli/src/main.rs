// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli`: inspect, convert and test score files without a window.

use std::process::ExitCode;

const USAGE: &str = "\
Usage: oxiguitar-cli <command>

Commands:
  version    Print the version and exit
  help       Print this help";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version" | "--version" | "-V") => {
            println!("oxiguitar-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("help" | "--help" | "-h") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("oxiguitar-cli: unknown command '{other}'\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
