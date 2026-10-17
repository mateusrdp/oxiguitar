// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The window's shape: the toolbar's sections and the four tabbed areas
//! around the page (docs/design/ui.md, "The window"). No window code here,
//! so the rules are unit-tested.
//!
//! The core's own tabs and toolbar sections are set up here. Plugins add
//! theirs to the same lists once they can be loaded (docs/design/plugins.md,
//! "What plugins contribute"), so nothing in the window assumes a fixed set.

/// The four areas around the page. Top and bottom span the window's width;
/// left and right sit beside the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub(crate) const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

    /// The side for the window's area number (0 top, 1 bottom, 2 left, 3 right).
    pub(crate) fn from_index(i: i32) -> Option<Side> {
        Side::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A tab in one of the areas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tab {
    /// What the window shows inside it: the core's tabs have their own ids
    /// ("player", "tracks", …); a plugin's tab will carry the plugin's id.
    pub(crate) id: String,
    pub(crate) title: String,
}

impl Tab {
    fn new(id: &str, title: &str) -> Tab {
        Tab {
            id: id.to_owned(),
            title: title.to_owned(),
        }
    }
}

/// One area: its tabs, which one is showing, and whether the area is
/// collapsed to its tab strip or hidden altogether.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Area {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) collapsed: bool,
    pub(crate) shown: bool,
}

impl Area {
    fn new(tabs: Vec<Tab>, collapsed: bool) -> Area {
        Area {
            tabs,
            active: 0,
            collapsed,
            shown: true,
        }
    }

    /// A click on tab `i`'s name: another tab opens (expanding the area if
    /// it was collapsed); the open tab's own name collapses or expands it.
    pub(crate) fn click(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if i == self.active {
            self.collapsed = !self.collapsed;
        } else {
            self.active = i;
            self.collapsed = false;
        }
    }

    /// Shows tab `id` if the area has it, expanding the area.
    pub(crate) fn open(&mut self, id: &str) -> bool {
        match self.tabs.iter().position(|t| t.id == id) {
            Some(i) => {
                self.active = i;
                self.collapsed = false;
                self.shown = true;
                true
            }
            None => false,
        }
    }

    /// The id of the tab showing, or "" when the area has none.
    pub(crate) fn active_id(&self) -> &str {
        self.tabs.get(self.active).map_or("", |t| t.id.as_str())
    }
}

/// A button in a toolbar section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolbarItem {
    /// What a click asks for, as the window's `toolbar-activated` passes it.
    pub(crate) id: String,
    /// The icon's name in `assets/icons/ui/` (core items) or the plugin's
    /// manifest (plugin items).
    pub(crate) icon: String,
    /// The tooltip: the item's name, and its shortcut when it has one.
    pub(crate) tip: String,
    /// Greyed out when false.
    pub(crate) enabled: bool,
    /// Toggles (such as Dark Page) show their state.
    pub(crate) checkable: bool,
    pub(crate) checked: bool,
}

impl ToolbarItem {
    fn new(id: &str, icon: &str, tip: &str, enabled: bool) -> ToolbarItem {
        ToolbarItem {
            id: id.to_owned(),
            icon: icon.to_owned(),
            tip: tip.to_owned(),
            enabled,
            checkable: false,
            checked: false,
        }
    }

    fn toggle(mut self) -> ToolbarItem {
        self.checkable = true;
        self
    }
}

/// A titled group of toolbar buttons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolbarSection {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) items: Vec<ToolbarItem>,
}

/// The whole shell: toolbar and areas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shell {
    pub(crate) toolbar_shown: bool,
    pub(crate) toolbar: Vec<ToolbarSection>,
    /// Indexed by [`Side::index`].
    pub(crate) areas: [Area; 4],
}

/// The ids of the core's toolbar items, as `toolbar-activated` passes them.
pub(crate) mod item {
    pub(crate) const OPEN: &str = "open";
    pub(crate) const SAVE: &str = "save";
    pub(crate) const EXPORT_PDF: &str = "export-pdf";
    pub(crate) const UNDO: &str = "undo";
    pub(crate) const REDO: &str = "redo";
    pub(crate) const DARK_PAGE: &str = "dark-page";
}

impl Default for Shell {
    /// The core's toolbar and tabs. The player and the track list start
    /// open; the side areas, which hold placeholders until the editor
    /// (M3), start collapsed so the page keeps its room.
    fn default() -> Shell {
        let section = |id: &str, title: &str, items: Vec<ToolbarItem>| ToolbarSection {
            id: id.to_owned(),
            title: title.to_owned(),
            items,
        };
        Shell {
            toolbar_shown: true,
            toolbar: vec![
                section(
                    "file",
                    "File",
                    vec![
                        ToolbarItem::new(item::OPEN, "open", "Open… (Ctrl + O)", true),
                        ToolbarItem::new(item::SAVE, "save", "Save (comes with the editor)", false),
                        ToolbarItem::new(item::EXPORT_PDF, "export", "Export PDF…", false),
                    ],
                ),
                section(
                    "edit",
                    "Edit",
                    vec![
                        ToolbarItem::new(item::UNDO, "undo", "Undo (comes with the editor)", false),
                        ToolbarItem::new(item::REDO, "redo", "Redo (comes with the editor)", false),
                    ],
                ),
                section(
                    "view",
                    "View",
                    vec![
                        ToolbarItem::new(item::DARK_PAGE, "dark-page", "Dark Page", true).toggle(),
                    ],
                ),
            ],
            areas: [
                Area::new(vec![Tab::new("player", "Player")], false),
                Area::new(
                    vec![Tab::new("tracks", "Tracks"), Tab::new("mixer", "Mixer")],
                    false,
                ),
                Area::new(vec![Tab::new("palette", "Palette")], true),
                Area::new(vec![Tab::new("inspector", "Inspector")], true),
            ],
        }
    }
}

impl Shell {
    pub(crate) fn area(&self, side: Side) -> &Area {
        &self.areas[side.index()]
    }

    pub(crate) fn area_mut(&mut self, side: Side) -> &mut Area {
        &mut self.areas[side.index()]
    }

    /// Sets a toolbar item's enabled state or tick, wherever it is.
    pub(crate) fn set_item(&mut self, id: &str, enabled: Option<bool>, checked: Option<bool>) {
        for item in self.toolbar.iter_mut().flat_map(|s| s.items.iter_mut()) {
            if item.id == id {
                if let Some(e) = enabled {
                    item.enabled = e;
                }
                if let Some(c) = checked {
                    item.checked = c;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_tabs_and_sections() {
        let shell = Shell::default();
        assert_eq!(shell.area(Side::Top).active_id(), "player");
        assert_eq!(shell.area(Side::Bottom).active_id(), "tracks");
        assert!(shell.area(Side::Left).collapsed && shell.area(Side::Right).collapsed);
        assert!(!shell.area(Side::Top).collapsed);
        let ids: Vec<&str> = shell.toolbar.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["file", "edit", "view"]);
        assert_eq!(Side::from_index(3), Some(Side::Right));
        assert_eq!(Side::from_index(4), None);
        assert_eq!(Side::from_index(-1), None);
    }

    #[test]
    fn clicking_tabs_switches_and_collapses() {
        let mut area = Shell::default().area(Side::Bottom).clone();
        area.click(1);
        assert_eq!((area.active_id(), area.collapsed), ("mixer", false));
        area.click(1);
        assert!(area.collapsed, "the open tab's name collapses the area");
        area.click(0);
        assert_eq!((area.active_id(), area.collapsed), ("tracks", false));
        area.click(7);
        assert_eq!(area.active_id(), "tracks");
        area.shown = false;
        assert!(area.open("mixer") && area.shown && !area.collapsed);
        assert!(!area.open("nothing"));
    }

    #[test]
    fn toolbar_items_change_in_place() {
        let mut shell = Shell::default();
        shell.set_item(item::EXPORT_PDF, Some(true), None);
        shell.set_item(item::DARK_PAGE, None, Some(true));
        let all: Vec<&ToolbarItem> = shell.toolbar.iter().flat_map(|s| &s.items).collect();
        assert!(all.iter().any(|i| i.id == item::EXPORT_PDF && i.enabled));
        assert!(
            all.iter()
                .any(|i| i.id == item::DARK_PAGE && i.checked && i.checkable)
        );
        assert!(all.iter().any(|i| i.id == item::UNDO && !i.enabled));
    }
}
