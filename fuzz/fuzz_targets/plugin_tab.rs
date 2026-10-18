// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Any answer a plugin could give with a tab's contents (interface 1.1), as
//! the core takes it: decoded, checked for what can't be shown, and each
//! canvas turned into a display list and drawn, small, as a narrow tab
//! would. A plugin's answer is untrusted bytes: refusing it is fine,
//! crashing the window is not. Seed: `fixtures/plugin/panel.answer`.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use oxiguitar_display::{FontSet, Theme};
use oxiguitar_plugin::api::message::Answer;
use oxiguitar_plugin::api::tab::Widget;
use oxiguitar_render::{Raster, View};

fn fonts() -> &'static FontSet {
    static FONTS: OnceLock<FontSet> = OnceLock::new();
    FONTS.get_or_init(FontSet::bundled)
}

fuzz_target!(|data: &[u8]| {
    let Ok(Answer::Tab(content)) = oxiguitar_plugin::api::decode::<Answer<'_>>(data) else {
        return;
    };
    // The host refuses contents with problems; everything else is drawn.
    if !content.problems().is_empty() {
        return;
    }
    let _ = content.widget("canvas");
    for widget in content.rows.iter().flat_map(|r| &r.widgets) {
        if let Widget::Canvas(c) = widget {
            let list = oxiguitar_display::wire::canvas(c.width, c.height, &c.items, fonts());
            // At most 64 pixels a side: the drawing, not its size, is tested.
            let scale = 64.0 / c.width.max(c.height);
            let view = View::page(&list, scale);
            let _ = Raster::new().render(&list, fonts(), &Theme::DARK, &view);
        }
    }
});
