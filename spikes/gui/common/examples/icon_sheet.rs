// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Writes every palette icon into one PNG, each on a button-coloured square
//! with a centre cross, to check sizing and centring by eye.
//! Usage: cargo run -p spike-common --example icon_sheet -- out.png [px]

use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "icons.png".into());
    let px: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(72);
    let icons = spike_common::icons::tool_icons(px);
    let gap = px / 6;
    let cols = 9u32;
    let rows = (icons.len() as u32).div_ceil(cols);
    let mut sheet = Pixmap::new(cols * (px + gap) + gap, rows * (px + gap) + gap).unwrap();
    sheet.fill(Color::from_rgba8(240, 240, 240, 255));
    for (i, icon) in icons.iter().enumerate() {
        let (c, r) = (i as u32 % cols, i as u32 / cols);
        let (x, y) = ((gap + c * (px + gap)) as f32, (gap + r * (px + gap)) as f32);
        let mut bg = Paint::default();
        bg.set_color_rgba8(60, 60, 64, 255);
        sheet.fill_rect(
            Rect::from_xywh(x, y, px as f32, px as f32).unwrap(),
            &bg,
            Transform::identity(),
            None,
        );
        let img = Pixmap::from_vec(
            icon.rgba.clone(),
            tiny_skia::IntSize::from_wh(px, px).unwrap(),
        )
        .unwrap();
        sheet.draw_pixmap(
            x as i32,
            y as i32,
            img.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        let mut red = Paint::default();
        red.set_color_rgba8(230, 60, 60, 160);
        let mut pb = PathBuilder::new();
        let h = px as f32 / 2.0;
        pb.move_to(x + h, y);
        pb.line_to(x + h, y + px as f32);
        pb.move_to(x, y + h);
        pb.line_to(x + px as f32, y + h);
        sheet.stroke_path(
            &pb.finish().unwrap(),
            &red,
            &Stroke {
                width: 1.0,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }
    sheet.save_png(&out).unwrap();
    println!("wrote {out}");
}
