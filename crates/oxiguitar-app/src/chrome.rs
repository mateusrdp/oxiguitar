// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What surrounds the page: the menus and the toolbar (OxiGuitar's tools and plugins'), the footer, the shell's areas, and the Keyboard and Mouse list.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

pub(crate) fn lines(items: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    ))
}

#[derive(Clone, Default)]
pub(crate) struct ShortcutData {
    pub(crate) navigation: Vec<(String, String)>,
    pub(crate) editing: Vec<(String, String)>,
    pub(crate) note_entry: Vec<(String, String)>,
    pub(crate) effects: Vec<(String, String)>,
    /// Plugins' tools' keys (interface 1.8).
    pub(crate) plugins: Vec<(String, String)>,
}

/// The shortcuts in three columns; the window makes each as wide as its
/// longest line.
pub(crate) fn shortcuts_to_columns(data: &ShortcutData) -> [ModelRc<ShortcutLine>; 3] {
    // Collect all shortcut lines in section order, then distribute
    // sequentially: every 3rd line goes to the next column.
    let mut all: Vec<ShortcutLine> = Vec::new();
    for entries in [
        &data.navigation,
        &data.editing,
        &data.note_entry,
        &data.effects,
        &data.plugins,
    ] {
        for (key, desc) in entries.iter() {
            all.push(ShortcutLine {
                text: format!("{}   →   {}", key, desc).into(),
            });
        }
    }

    let mut cols: [Vec<ShortcutLine>; 3] = Default::default();
    for (idx, line) in all.into_iter().enumerate() {
        cols[idx % 3].push(line);
    }
    cols.map(|c| ModelRc::new(VecModel::from(c)))
}

/// The Tools menu before any score is open: every tool, greyed out.
pub(crate) fn default_tools() -> Vec<ToolEntry> {
    oxiguitar_tools::all()
        .iter()
        .map(|t| ToolEntry {
            name: t.name().to_owned(),
            unavailable: None,
        })
        .collect()
}

/// Menu rows for the tools. Slint menus have no tooltips, so a tool that
/// cannot run carries its reason in its title, greyed out.
pub(crate) fn tool_rows(tools: &[ToolEntry]) -> Vec<ToolRow> {
    tools
        .iter()
        .map(|t| match &t.unavailable {
            None => ToolRow {
                title: t.name.as_str().into(),
                enabled: true,
                edits: true,
            },
            Some(why) => ToolRow {
                title: format!("{} ({})", t.name, reason_phrase(why)).into(),
                enabled: false,
                edits: true,
            },
        })
        .collect()
}

/// "The score already has key signatures." → "the score already has key
/// signatures", to follow a menu entry in brackets.
pub(crate) fn reason_phrase(why: &str) -> String {
    let why = why.trim().trim_end_matches('.');
    let mut chars = why.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The entries plugins add to menus other than Tools (interface 1.8).
#[derive(Default)]
pub(crate) struct PluginMenus {
    pub(crate) file: Vec<PluginMenuItem>,
    pub(crate) edit: Vec<PluginMenuItem>,
    pub(crate) view: Vec<PluginMenuItem>,
    pub(crate) help: Vec<PluginMenuItem>,
    pub(crate) page: Vec<PluginMenuItem>,
    pub(crate) track: Vec<PluginMenuItem>,
    /// Menus of the plugins' own, by the name the first tool gave each.
    pub(crate) own: Vec<(String, Vec<PluginMenuItem>)>,
}

impl PluginMenus {
    /// Adds `item` to the menu `key` names (lower case), or to the
    /// plugins' own menu called `name`, as its manifest wrote it.
    pub(crate) fn add(&mut self, key: &str, name: &str, item: PluginMenuItem) {
        let list = match key {
            "file" => &mut self.file,
            "edit" => &mut self.edit,
            "view" => &mut self.view,
            "help" => &mut self.help,
            "page" => &mut self.page,
            "track" => &mut self.track,
            _ => {
                let at = match self
                    .own
                    .iter()
                    .position(|(title, _)| title.to_lowercase() == key)
                {
                    Some(at) => at,
                    None => {
                        self.own.push((name.to_owned(), Vec::new()));
                        self.own.len() - 1
                    }
                };
                &mut self.own[at].1
            }
        };
        list.push(item);
    }

    pub(crate) fn show(self, ui: &AppWindow) {
        let model = |items: Vec<PluginMenuItem>| ModelRc::new(VecModel::from(items));
        ui.set_plugin_items_file(model(self.file));
        ui.set_plugin_items_edit(model(self.edit));
        ui.set_plugin_items_view(model(self.view));
        ui.set_plugin_items_help(model(self.help));
        ui.set_plugin_items_page(model(self.page));
        ui.set_plugin_items_track(model(self.track));
        ui.set_plugin_menus(ModelRc::new(VecModel::from(
            self.own
                .into_iter()
                .map(|(title, items)| PluginMenu {
                    title: title.into(),
                    items: model(items),
                })
                .collect::<Vec<_>>(),
        )));
    }
}

/// The Tools menu: the core's tools, then the plugins' (interface 1.2).
pub(crate) fn set_tools(ui: &AppWindow, rows: &[ToolRow]) {
    let mut rows = rows.to_vec();
    PLUGIN_TOOLS.with(|t| {
        rows.extend(t.borrow().iter().map(|(_, _, name, edits)| ToolRow {
            title: name.as_str().into(),
            enabled: true,
            edits: *edits,
        }));
    });
    ui.set_tools(ModelRc::new(VecModel::from(rows)));
}

/// A message for the footer.
/// A plugin's link (interface 1.4) opens in the browser when clicked, in
/// whichever window shows it; `links::open` takes only `https://`.
pub(crate) fn install_plugin_links(links: &PluginLinks<'_>, app: &Shared) {
    let a = app.clone();
    links.on_open(move |url| {
        if let Err(e) = links::open(&url) {
            message(&a, Level::Error, e);
        }
    });
}

pub(crate) fn message(app: &Shared, level: Level, text: impl Into<String>) {
    app.borrow_mut().footer.push(level, text);
    sync_footer(app);
}

/// Shows the footer's latest message and, for the expanded footer, all of
/// them, newest first.
pub(crate) fn sync_footer(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let latest = s.footer.latest();
    let (level, text) = latest.map_or((0, ""), |n| (level_code(n.level), n.text.as_str()));
    let (link_text, link_url) = latest
        .and_then(|n| n.link.as_ref())
        .map_or(("", ""), |(t, u)| (t.as_str(), u.as_str()));
    ui.set_footer_level(level);
    ui.set_footer_text(text.into());
    ui.set_footer_link_text(link_text.into());
    ui.set_footer_link_url(link_url.into());
    ui.set_footer_count(s.footer.len() as i32);
    let rows: Vec<FooterRow> = s
        .footer
        .newest_first()
        .map(|n| {
            let (link_text, link_url) = n
                .link
                .as_ref()
                .map_or(("", ""), |(t, u)| (t.as_str(), u.as_str()));
            FooterRow {
                level: level_code(n.level),
                text: n.text.as_str().into(),
                link_text: link_text.into(),
                link_url: link_url.into(),
            }
        })
        .collect();
    ui.set_footer_rows(ModelRc::new(VecModel::from(rows)));
}

/// The toolbar's icons, our own SVGs (assets/icons/ui/), tinted by the
/// window. Plugins' icons will come from their manifests.
pub(crate) fn ui_icon(name: &str) -> Image {
    let svg: &[u8] = match name {
        "open" => include_bytes!("../../../assets/icons/ui/open.svg"),
        "save" => include_bytes!("../../../assets/icons/ui/save.svg"),
        "export" => include_bytes!("../../../assets/icons/ui/export.svg"),
        "undo" => include_bytes!("../../../assets/icons/ui/undo.svg"),
        "redo" => include_bytes!("../../../assets/icons/ui/redo.svg"),
        _ => return Image::default(),
    };
    Image::load_from_svg_data(svg).unwrap_or_default()
}

/// Shows the toolbar and the areas as the shell says.
pub(crate) fn sync_shell(app: &Shared) {
    sync_toolbar(app);
    sync_areas(app);
}

pub(crate) fn sync_toolbar(app: &Shared) {
    let mut s = app.borrow_mut();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let names: Vec<String> = s
        .shell
        .toolbar
        .iter()
        .flat_map(|sec| sec.items.iter().map(|i| i.icon.clone()))
        .collect();
    for name in names {
        s.icons
            .entry(name.clone())
            .or_insert_with(|| ui_icon(&name));
    }
    let sections: Vec<ToolbarSectionRow> = s
        .shell
        .toolbar
        .iter()
        .map(|sec| ToolbarSectionRow {
            title: sec.title.as_str().into(),
            items: ModelRc::new(VecModel::from(
                sec.items
                    .iter()
                    .map(|i| ToolbarItemRow {
                        id: i.id.as_str().into(),
                        icon: s.icons.get(&i.icon).cloned().unwrap_or_default(),
                        tip: i.tip.as_str().into(),
                        enabled: i.enabled,
                        checkable: i.checkable,
                        checked: i.checked,
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_toolbar(ModelRc::new(VecModel::from(sections)));
    ui.set_toolbar_shown(s.shell.toolbar_shown);
}

pub(crate) fn sync_areas(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let state = |side: Side| {
        let area = s.shell.area(side);
        AreaState {
            titles: ModelRc::new(VecModel::from(
                area.tabs
                    .iter()
                    .map(|t| SharedString::from(t.title.as_str()))
                    .collect::<Vec<_>>(),
            )),
            active: area.active as i32,
            active_id: area.active_id().into(),
            collapsed: area.collapsed,
            plugin: plugin_tabs::tab_of(area.active_id()).is_some(),
        }
    };
    ui.set_top_area(state(Side::Top));
    ui.set_bottom_area(state(Side::Bottom));
    ui.set_left_area(state(Side::Left));
    ui.set_right_area(state(Side::Right));
    ui.set_top_shown(s.shell.area(Side::Top).shown);
    ui.set_bottom_shown(s.shell.area(Side::Bottom).shown);
    ui.set_left_shown(s.shell.area(Side::Left).shown);
    ui.set_right_shown(s.shell.area(Side::Right).shown);
    drop(s);
    for side in Side::ALL {
        show_plugin_tab(app, side);
    }
}
