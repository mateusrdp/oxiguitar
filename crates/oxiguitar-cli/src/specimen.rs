// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli specimen`: the render spike's test page as PNG and PDF.

use std::process::ExitCode;
use std::time::Instant;

use oxiguitar_display::{FontSet, Theme, smufl::Metadata, specimen};
use oxiguitar_render::{Raster, View, to_pdf};

pub(crate) fn run(args: &[String]) -> ExitCode {
    let mut png = None;
    let mut pdf = None;
    let mut scale = 2.0_f32;
    let mut theme = Theme::LIGHT;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--png" => png = it.next().cloned(),
            "--pdf" => pdf = it.next().cloned(),
            "--scale" => match it.next().and_then(|s| s.parse::<f32>().ok()) {
                Some(s) if (0.1..=16.0).contains(&s) => scale = s,
                _ => return crate::usage_error("--scale takes a number from 0.1 to 16"),
            },
            "--dark" => theme = Theme::DARK,
            other => return crate::usage_error(&format!("specimen: unexpected '{other}'")),
        }
    }
    if png.is_none() && pdf.is_none() {
        return crate::usage_error("specimen needs --png FILE, --pdf FILE or both");
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
    let page = specimen::guitar_excerpt(&fonts, &meta);
    println!(
        "display list: {} items, built in {:.1} ms",
        page.items.len(),
        ms(started)
    );

    if let Some(path) = png {
        let mut raster = Raster::new();
        let view = View::page(&page, scale);
        let started = Instant::now();
        let Some(pixmap) = raster.render(&page, &fonts, &theme, &view) else {
            eprintln!("oxiguitar-cli: empty image");
            return ExitCode::FAILURE;
        };
        let cold = ms(started);
        let started = Instant::now();
        let _ = raster.render(&page, &fonts, &theme, &view);
        let warm = ms(started);
        println!(
            "raster: {}×{} px at {scale}× in {cold:.1} ms (first), {warm:.1} ms (glyphs cached, {} outlines)",
            view.width,
            view.height,
            raster.cached_glyphs()
        );
        match pixmap.encode_png() {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&path, bytes) {
                    eprintln!("oxiguitar-cli: {path}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("wrote {path}");
            }
            Err(e) => {
                eprintln!("oxiguitar-cli: PNG encoding failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    if let Some(path) = pdf {
        let started = Instant::now();
        match to_pdf(&[&page], &fonts, &theme, "OxiGuitar render spike") {
            Ok(bytes) => {
                println!("pdf: {} bytes in {:.1} ms", bytes.len(), ms(started));
                if let Err(e) = std::fs::write(&path, bytes) {
                    eprintln!("oxiguitar-cli: {path}: {e}");
                    return ExitCode::FAILURE;
                }
                println!("wrote {path}");
            }
            Err(e) => {
                eprintln!("oxiguitar-cli: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}
