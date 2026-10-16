// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli tools`: list the tools of the app's Tools menu.

use std::process::ExitCode;

pub(crate) fn list() -> ExitCode {
    for tool in oxiguitar_tools::all() {
        println!("{:<24} {}", tool.id(), tool.name());
        println!("{:<24} {}", "", tool.description());
    }
    ExitCode::SUCCESS
}
