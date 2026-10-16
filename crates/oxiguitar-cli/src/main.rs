// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli`: inspect, convert and test score files without a window.

mod info;
mod manifest;
mod render;
mod sha256;
mod specimen;
mod tools;

use std::process::ExitCode;

const USAGE: &str = "\
Usage: oxiguitar-cli <command> [options]

Commands:
  info [--bars] <file>...    Print the structure of each file: format, header,
                             bars, tracks and the import log. --bars adds a
                             line per bar and track.
  detect <file>...           Print the format of each file, read from its
                             contents rather than its extension
  manifest <dir>...          Print a TOML manifest (name, format, size,
                             SHA-256) of every file in the folders
  render <file> [--png PREFIX] [--pdf FILE] [--scale N] [--dark]
         [--layout page|vertical|horizontal|parchment] [--width PT]
         [--paper a4|letter] [--tracks 1,2|all] [--staff-space PT]
         [--tab 1,2] [--no-tab 1,2] [--no-dynamics] [--tool ID]...
         [--quiet]
                             Engrave a score: PNG pages PREFIX-1.png, … (N
                             pixels per point, default 2) and/or a PDF.
                             --layout picks the staff layout (default page;
                             long sheets are cut into PNGs of at most 8192
                             pixels); --width is the window width for the
                             vertical layout; the PDF is always in pages;
                             --tracks picks tracks (default: the first);
                             --tab / --no-tab show or hide those tracks'
                             tablature (default: fretted instruments only);
                             --no-dynamics hides dynamics markings;
                             --tool runs a tool on the score first, as the
                             app's Tools menu does; --quiet only reports
                             errors
  tools                      List the tools (the app's Tools menu)
  specimen [--png FILE] [--pdf FILE] [--scale N] [--dark]
                             Draw the render spike's test page into a PNG
                             (N pixels per point, default 2) and/or a PDF
  version                    Print the version and exit
  help                       Print this help

Exit status: 0 if every file was read, 1 if any could not be, 2 on bad usage.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("info") => info::run(rest),
        Some("detect") => info::detect(rest),
        Some("manifest") => manifest::run(rest),
        Some("render") => render::run(rest),
        Some("specimen") => specimen::run(rest),
        Some("tools") => tools::list(),
        Some("version" | "--version" | "-V") => {
            println!("oxiguitar-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("help" | "--help" | "-h") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => usage_error(&format!("unknown command '{other}'")),
    }
}

pub(crate) fn usage_error(msg: &str) -> ExitCode {
    eprintln!("oxiguitar-cli: {msg}\n\n{USAGE}");
    ExitCode::from(2)
}
