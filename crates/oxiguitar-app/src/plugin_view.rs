// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugins' panels on screen: a panel is a plugin's own view in Slint
//! placed in one of the window's areas (plugin_ui.rs makes it); here, which
//! one each area shows and bringing one to the front.
//!
//! Split out of main.rs (23 Nov 2026): it uses main.rs's items and imports
//! through `use super::*`.

use super::*;

/// Shows area `side`'s panel, if its open tab is one.
pub(crate) fn show_plugin_tab(app: &Shared, side: Side) {
    show_plugin_panel(app, side);
}

/// Puts area `side`'s panel, a plugin's own view in Slint, in the area's
/// container, when the area shows one.
fn show_plugin_panel(app: &Shared, side: Side) {
    let (view, ui) = {
        let s = app.borrow();
        let area = s.shell.area(side);
        let view = (area.shown && !area.collapsed)
            .then(|| plugin_ui::view_of(area.active_id()))
            .flatten();
        (view, s.ui.clone())
    };
    let Some(id) = view else {
        plugin_ui::no_panel(side.index());
        return;
    };
    let set: Rc<dyn Fn(slint::ComponentFactory)> = Rc::new(move |f| {
        if let Some(ui) = ui.upgrade() {
            match side {
                Side::Top => ui.set_top_view(f),
                Side::Bottom => ui.set_bottom_view(f),
                Side::Left => ui.set_left_view(f),
                Side::Right => ui.set_right_view(f),
            }
        }
    });
    plugin_ui::panel(side.index(), &id, set);
}

/// A plugin asked for its panel (a view in an area, interface 1.15) to be
/// brought to the front.
pub(crate) fn plugin_panel_wanted(app: &Shared, view: &oxiguitar_host::ViewId) {
    bring_forward(app, &plugin_ui::shell_id(view));
}

/// Opens the tab `id` in whichever area has it, showing the area.
fn bring_forward(app: &Shared, id: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let Some(side) = Side::ALL
        .iter()
        .copied()
        .find(|&side| app.borrow_mut().shell.area_mut(side).open(id))
    else {
        return;
    };
    match side {
        Side::Top => ui.set_top_shown(true),
        Side::Bottom => ui.set_bottom_shown(true),
        Side::Left => ui.set_left_shown(true),
        Side::Right => ui.set_right_shown(true),
    }
    sync_areas(app);
}
