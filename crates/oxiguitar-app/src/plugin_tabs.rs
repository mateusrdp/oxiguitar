// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugins' panels among the areas' tabs: a panel is a plugin's own view
//! in Slint placed in an area (plugin interface 2.0; plugin_ui.rs makes
//! it). No window code here, so it is unit-tested.

use oxiguitar_host::Plugins;
use oxiguitar_host::api::view::Place;

use crate::shell::{Shell, Side, Tab};

/// Adds every panel the usable plugins declare to its area, after the
/// core's own tabs, in plugin order. Nothing is loaded: the names come
/// from the manifests.
pub(crate) fn add_views_to(shell: &mut Shell, plugins: &Plugins) {
    for view in plugins.views() {
        let side = match view.spec.place {
            Place::Top => Side::Top,
            Place::Bottom => Side::Bottom,
            Place::Left => Side::Left,
            Place::Right => Side::Right,
            Place::Window | Place::Track | Place::Export => continue,
        };
        shell.area_mut(side).tabs.push(Tab {
            id: crate::plugin_ui::shell_id(&view.id()),
            title: view.spec.title.clone(),
        });
    }
}

/// Takes away every panel of the plugin `id`, which was withdrawn: as if
/// it had never been installed. Whether any went.
pub(crate) fn remove_from(shell: &mut Shell, id: &str) -> bool {
    let mut any = false;
    for side in Side::ALL {
        any |= shell
            .area_mut(side)
            .remove(|tab| crate::plugin_ui::view_of(tab).is_some_and(|v| v.plugin == id));
    }
    any
}

#[cfg(test)]
mod tests {
    use oxiguitar_host::ViewId;

    use super::*;

    #[test]
    fn a_withdrawn_plugin_s_panels_go_and_the_rest_stay() {
        let mut shell = Shell::default();
        let add = |shell: &mut Shell, side: Side, plugin: &str, view: &str| {
            shell.area_mut(side).tabs.push(Tab {
                id: crate::plugin_ui::shell_id(&ViewId {
                    plugin: plugin.into(),
                    view: view.into(),
                }),
                title: view.into(),
            });
        };
        add(&mut shell, Side::Bottom, "a.one", "x");
        add(&mut shell, Side::Bottom, "b.two", "y");
        add(&mut shell, Side::Right, "a.one", "z");
        let bottom = shell.area(Side::Bottom).tabs.len();
        // The other plugin's panel showing in the bottom area, the
        // withdrawn one's in the right area.
        shell.area_mut(Side::Bottom).active = bottom - 1;
        let right = shell.area(Side::Right).tabs.len();
        shell.area_mut(Side::Right).active = right - 1;
        assert!(remove_from(&mut shell, "a.one"));
        let ids =
            |side| -> Vec<String> { shell.area(side).tabs.iter().map(|t| t.id.clone()).collect() };
        assert_eq!(ids(Side::Bottom).len(), bottom - 1);
        assert_eq!(shell.area(Side::Bottom).active_id(), "view:b.two/y");
        assert!(
            ids(Side::Right)
                .iter()
                .all(|id| crate::plugin_ui::view_of(id).is_none())
        );
        assert!(shell.area(Side::Right).active < right - 1);
        assert!(!remove_from(&mut shell, "a.one"));
    }
}
