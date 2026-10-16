// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli render`: engrave a score file into PNG pages and/or a PDF.

use std::process::ExitCode;
use std::time::Instant;

use oxiguitar_display::{FontSet, Theme, smufl::Metadata};
use oxiguitar_layout::{Layout, Options, Paper, engrave};
use oxiguitar_render::{Raster, View, to_pdf};

pub(crate) fn run(args: &[String]) -> ExitCode {
    let mut file = None;
    let mut png = None;
    let mut pdf = None;
    let mut scale = 2.0_f32;
    let mut theme = Theme::LIGHT;
    let mut options = Options::default();
    let mut quiet = false;
    let mut tools: Vec<&'static dyn oxiguitar_tools::Tool> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--png" => png = it.next().cloned(),
            "--pdf" => pdf = it.next().cloned(),
            "--scale" => match it.next().and_then(|s| s.parse::<f32>().ok()) {
                Some(s) if (0.1..=16.0).contains(&s) => scale = s,
                _ => return crate::usage_error("--scale takes a number from 0.1 to 16"),
            },
            "--tracks" => {
                let parsed: Option<Vec<usize>> = it.next().and_then(|list| {
                    if list == "all" {
                        return Some((0..=u8::MAX.into()).collect());
                    }
                    list.split(',')
                        .map(|t| t.trim().parse::<usize>().ok()?.checked_sub(1))
                        .collect()
                });
                match parsed {
                    Some(t) if !t.is_empty() => options.tracks = t,
                    _ => {
                        return crate::usage_error(
                            "--tracks takes track numbers from 1, such as 1,3, or all",
                        );
                    }
                }
            }
            "--staff-space" => match it.next().and_then(|s| s.parse::<f32>().ok()) {
                Some(s) if (2.0..=20.0).contains(&s) => options.staff_space = s,
                _ => return crate::usage_error("--staff-space takes points from 2 to 20"),
            },
            "--layout" => match it.next().and_then(|n| Layout::from_name(n)) {
                Some(l) => options.layout = l,
                None => {
                    return crate::usage_error(
                        "--layout takes page, vertical, horizontal or parchment",
                    );
                }
            },
            "--paper" => match it.next().and_then(|n| Paper::from_name(n)) {
                Some(p) => options = options.with_paper(p),
                None => return crate::usage_error("--paper takes a4 or letter"),
            },
            "--width" => match it.next().and_then(|s| s.parse::<f32>().ok()) {
                Some(w) if (100.0..=20000.0).contains(&w) => options.screen_width = w,
                _ => return crate::usage_error("--width takes points from 100 to 20000"),
            },
            "--tab" | "--no-tab" => {
                let on = arg == "--tab";
                let parsed: Option<Vec<usize>> = it.next().and_then(|list| {
                    list.split(',')
                        .map(|t| t.trim().parse::<usize>().ok()?.checked_sub(1))
                        .collect()
                });
                match parsed {
                    Some(t) if !t.is_empty() => {
                        for track in t {
                            options.tablature.insert(track, on);
                        }
                    }
                    _ => return crate::usage_error("--tab and --no-tab take track numbers from 1"),
                }
            }
            "--dark" => theme = Theme::DARK,
            "--no-dynamics" => options.dynamics = false,
            "--tool" => match it.next().and_then(|id| oxiguitar_tools::find(id)) {
                Some(t) => tools.push(t),
                None => {
                    return crate::usage_error(
                        "--tool takes a tool id; `oxiguitar-cli tools` lists them",
                    );
                }
            },
            "--quiet" => quiet = true,
            other if other.starts_with("--") => {
                return crate::usage_error(&format!("render: unknown option '{other}'"));
            }
            other if file.is_none() => file = Some(other.to_owned()),
            other => return crate::usage_error(&format!("render: unexpected '{other}'")),
        }
    }
    let Some(file) = file else {
        return crate::usage_error("render needs a score file");
    };
    if png.is_none() && pdf.is_none() && !quiet {
        return crate::usage_error("render needs --png PREFIX, --pdf FILE or both");
    }

    let data = match std::fs::read(&file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{file}: cannot open: {e}");
            return ExitCode::FAILURE;
        }
    };
    let started = Instant::now();
    let imported = match oxiguitar_formats::read(&data) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{file}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let read_ms = ms(started);
    let mut score = imported.score;
    for tool in tools {
        let outcome = tool.apply(&score);
        if !quiet {
            println!("{file}: {}: {}", tool.name(), outcome.report);
        }
        if let Some(changed) = outcome.score {
            score = changed;
        }
    }
    let fonts = FontSet::bundled();
    let meta = match Metadata::bravura() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("oxiguitar-cli: Bravura metadata: {e}");
            return ExitCode::FAILURE;
        }
    };
    let started = Instant::now();
    let engraved = engrave(&score, &fonts, &meta, &options);
    let items: usize = engraved.pages.iter().map(|p| p.items.len()).sum();
    if !quiet {
        println!(
            "{file}: read in {read_ms:.1} ms, engraved in {:.1} ms: {} page(s), {} system(s), {items} items",
            ms(started),
            engraved.pages.len(),
            engraved.systems,
        );
    }

    if let Some(prefix) = png {
        let mut raster = Raster::new();
        let mut n = 0;
        for (p, page) in engraved.pages.iter().enumerate() {
            // A long sheet (Horizontal, Vertical, Parchment) is cut into
            // pieces of at most PNG_MAX pixels along its length.
            for view in png_pieces(page, scale) {
                let Some(pixmap) = raster.render(page, &fonts, &theme, &view) else {
                    eprintln!("oxiguitar-cli: sheet {} is empty", p + 1);
                    return ExitCode::FAILURE;
                };
                n += 1;
                let path = format!("{prefix}-{n}.png");
                let written = pixmap
                    .encode_png()
                    .map_err(|e| e.to_string())
                    .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
                if let Err(e) = written {
                    eprintln!("oxiguitar-cli: {path}: {e}");
                    return ExitCode::FAILURE;
                }
                if !quiet {
                    println!("wrote {path}");
                }
            }
        }
    }

    if let Some(path) = pdf {
        // PDF export is always the Page layout, on paper (EXP-1).
        let paged;
        let engraved = if options.layout == Layout::Page {
            &engraved
        } else {
            paged = engrave(
                &score,
                &fonts,
                &meta,
                &Options {
                    layout: Layout::Page,
                    ..options.clone()
                },
            );
            &paged
        };
        let pages: Vec<_> = engraved.pages.iter().collect();
        let title = score.info.title.trim();
        let title = if title.is_empty() { "OxiGuitar" } else { title };
        match to_pdf(&pages, &fonts, &theme, title) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&path, bytes) {
                    eprintln!("oxiguitar-cli: {path}: {e}");
                    return ExitCode::FAILURE;
                }
                if !quiet {
                    println!("wrote {path}");
                }
            }
            Err(e) => {
                eprintln!("oxiguitar-cli: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// The longest side of one PNG, in pixels.
const PNG_MAX: u32 = 8192;

/// Views covering `page` at `scale`, cut across its longer side into pieces
/// no longer than [`PNG_MAX`] pixels.
fn png_pieces(page: &oxiguitar_display::DisplayList, scale: f32) -> Vec<View> {
    let whole = View::page(page, scale);
    let wide = whole.width >= whole.height;
    let length = if wide { whole.width } else { whole.height };
    let count = length.div_ceil(PNG_MAX).max(1);
    let piece = length.div_ceil(count);
    (0..count)
        .map(|i| {
            let start = i * piece;
            let len = piece.min(length - start);
            let offset = start as f32 / scale;
            if wide {
                View {
                    x: offset,
                    width: len,
                    ..whole
                }
            } else {
                View {
                    y: offset,
                    height: len,
                    ..whole
                }
            }
        })
        .collect()
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}
