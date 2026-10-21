// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli tools`: list the tools of the app's Tools menu: the
//! core's, then those the installed plugins add (interface 1.2).

use std::process::ExitCode;

pub(crate) fn list() -> ExitCode {
    for tool in oxiguitar_tools::all() {
        println!("{:<24} {}", tool.id(), tool.name());
        println!("{:<24} {}", "", tool.description());
    }
    for tool in crate::plugins::plugins().tools() {
        let id = format!(
            "{}/{}",
            tool.plugin.manifest.as_ref().map_or("", |m| m.id.as_str()),
            tool.spec.id
        );
        println!("{id:<24} {} ({})", tool.spec.name, tool.plugin.name());
        if !tool.spec.description.is_empty() {
            println!("{:<24} {}", "", tool.spec.description);
        }
    }
    ExitCode::SUCCESS
}
