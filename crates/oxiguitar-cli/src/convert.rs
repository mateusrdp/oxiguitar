// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `convert`: reads a file and writes it in another format, through the
//! format plugins, naming what the new file can't hold (FMT-3).

use std::path::Path;
use std::process::ExitCode;

use oxiguitar_host::api::message::Level;

use crate::plugins::{level_word, open, plugins};

/// `convert <in> <out>`: the format to write comes from the extension of
/// `out`, among the formats the plugins write.
pub(crate) fn run(args: &[String]) -> ExitCode {
    let [input, output] = args else {
        return crate::usage_error("convert takes a file to read and a file to write");
    };
    let Some(ext) = Path::new(output)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return crate::usage_error("the file to write needs an extension, such as .gp");
    };
    let writers = plugins().writers();
    let Some(format) = writers
        .iter()
        .find(|f| {
            f.spec
                .extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&ext))
        })
        .copied()
    else {
        let known: Vec<String> = writers
            .iter()
            .flat_map(|f| f.spec.extensions.iter().map(|e| format!(".{e}")))
            .collect();
        eprintln!(
            "oxiguitar-cli: no installed plugin writes .{ext} files{}",
            if known.is_empty() {
                String::new()
            } else {
                format!(" (they write {})", known.join(", "))
            }
        );
        return ExitCode::FAILURE;
    };

    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("oxiguitar-cli: {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let opened = match open(input, &data) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("oxiguitar-cli: {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let score = oxiguitar_model::wire::to_wire2(&opened.score);
    let written = match plugins().write(format, &score) {
        Ok(w) => w,
        Err(e) => {
            eprintln!(
                "oxiguitar-cli: {input}: the {} plugin could not write it: {e}",
                format.plugin.name()
            );
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(output, &written.data) {
        eprintln!("oxiguitar-cli: {output}: {e}");
        return ExitCode::FAILURE;
    }
    // The tracks' plugin settings, which no format holds, go beside the new
    // file as they were beside the old (FILE.oxg).
    use oxiguitar_host::sidecar::Sidecar;
    match Sidecar::read_for(Path::new(input))
        .and_then(|s| s.map_or(Ok(()), |s| s.write_for(Path::new(output))))
    {
        Ok(()) => {}
        Err(e) => eprintln!("oxiguitar-cli: {e}"),
    }
    let lost = written
        .losses
        .iter()
        .filter(|l| l.level != Level::Info)
        .count();
    println!(
        "{input} ({}) → {output} ({}){}",
        opened.format,
        format.spec.name,
        match lost {
            0 => String::new(),
            1 => ", 1 loss".to_owned(),
            n => format!(", {n} losses"),
        }
    );
    for l in &written.losses {
        let count = if l.count > 1 {
            format!(", {} times", l.count)
        } else {
            String::new()
        };
        println!(
            "  {}: {} ({}{count})",
            level_word(l.level),
            l.message,
            l.place
        );
    }
    ExitCode::SUCCESS
}
