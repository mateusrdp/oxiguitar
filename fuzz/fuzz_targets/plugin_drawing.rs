// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Any answer a plugin could give with what it draws on the page
//! (interface 1.8), as the core takes it: decoded, checked as the host
//! checks it, and each drawing turned into a display list and drawn,
//! small. A plugin's answer is untrusted bytes: refusing it is fine,
//! crashing the window is not. Seeds: `fixtures/plugin/*.answer`.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use oxiguitar_display::{FontSet, Theme};
use oxiguitar_plugin::api::message::Answer;
use oxiguitar_render::{Raster, View};

fn fonts() -> &'static FontSet {
    static FONTS: OnceLock<FontSet> = OnceLock::new();
    FONTS.get_or_init(FontSet::bundled)
}

fuzz_target!(|data: &[u8]| {
    let Ok(Answer::Drawn(drawn)) = oxiguitar_plugin::api::decode::<Answer<'_>>(data) else {
        return;
    };
    for d in drawn {
        // The host refuses a drawing with a problem; the rest is drawn.
        if d.problem().is_some() {
            continue;
        }
        let list = oxiguitar_display::wire::canvas(d.width, d.height, &d.items, fonts());
        // At most 64 pixels a side: the drawing, not its size, is tested.
        let scale = 64.0 / d.width.max(d.height).max(f32::MIN_POSITIVE);
        let view = View::page(&list, scale);
        let _ = Raster::new().render(&list, fonts(), &Theme::DARK, &view);
    }
});
