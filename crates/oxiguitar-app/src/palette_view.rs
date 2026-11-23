// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The palette and the icons: OxiGuitar's groups and plugins' tools, drawn for the window's scale, and the state of their buttons at the caret.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Draws the track rows' and the palette's icons for the window's current
/// scale.
pub(crate) fn refresh_icons(app: &Shared, ui: &AppWindow) {
    let scale = ui.window().scale_factor();
    if (scale - app.borrow().icons_scale).abs() < 1e-3 {
        return;
    }
    let px = (TOGGLE_ICON * scale).round().max(8.0) as u32;
    let images: Vec<Image> = app.borrow().toggles.draw(px).iter().map(image_of).collect();
    if let [std, tab, mute, solo] = images.as_slice() {
        ui.set_standard_icon(std.clone());
        ui.set_tablature_icon(tab.clone());
        ui.set_mute_icon(mute.clone());
        ui.set_solo_icon(solo.clone());
    }
    app.borrow_mut().icons_scale = scale;
    set_palette(app, ui, true);
}

/// The buttons the installed plugins add to the toolbar's Plugins section
/// (interface 1.8): each one's id and name, and its icon under that id.
/// Drawn once, larger than shown, so any scale stays sharp.
pub(crate) fn plugin_toolbar_tools(
    plugins: &oxiguitar_host::Plugins,
) -> (Vec<(String, String)>, HashMap<String, Image>) {
    let mut tools = Vec::new();
    let mut faces = Vec::new();
    for t in plugins.toolbar_tools() {
        let (Some(face), Some(manifest)) = (&t.spec.toolbar, &t.plugin.manifest) else {
            continue;
        };
        tools.push((
            palette::plugin_tool_id(&manifest.id, &t.spec.id),
            t.spec.name.clone(),
        ));
        faces.push(palette::plugin_face(
            face.symbol_char(),
            face.text.as_deref().unwrap_or_default(),
        ));
    }
    let icons = if faces.is_empty() {
        HashMap::new()
    } else {
        icons::IconSet::new(&faces, icons::Fill::TOGGLES)
            .draw(54)
            .iter()
            .zip(&tools)
            .map(|(icon, (id, _))| (id.clone(), image_of(icon)))
            .collect()
    };
    (tools, icons)
}

/// The tools the installed plugins add to the palette (interface 1.8).
pub(crate) fn plugin_palette_tools(plugins: &oxiguitar_host::Plugins) -> Vec<palette::PluginTool> {
    plugins
        .palette_tools()
        .iter()
        .filter_map(|t| {
            let at = t.spec.palette.as_ref()?;
            // The tooltip names its keys too, when it has some.
            let keys = t
                .spec
                .shortcut
                .as_deref()
                .and_then(oxiguitar_host::api::manifest::Shortcut::parse);
            Some(palette::PluginTool {
                plugin: t.plugin.manifest.as_ref()?.id.clone(),
                tool: t.spec.id.clone(),
                name: match keys {
                    Some(k) => format!("{} ({})", t.spec.name, k.label()),
                    None => t.spec.name.clone(),
                },
                section: at.section.clone(),
                symbol: at.symbol_char(),
                text: at.text.clone().unwrap_or_default(),
            })
        })
        .collect()
}

/// Measures the palette's icons on a thread of their own, so the window
/// never waits for them, and shows the palette once they are ready.
pub(crate) fn measure_palette(shown: Arc<Vec<palette::ShownGroup>>) {
    let spawned = std::thread::Builder::new()
        .name("palette icons".to_owned())
        .spawn(move || {
            let faces: Vec<(icons::Face, icons::Fit)> = shown
                .iter()
                .flat_map(|g| g.tools.iter().map(|t| (t.face, t.fit)))
                .collect();
            let set = Arc::new(icons::IconSet::new(&faces, icons::Fill::PALETTE));
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = APP.with(|a| a.borrow().clone()) else {
                    return;
                };
                app.borrow_mut().palette_icons = Some(set);
                if let Some(ui) = app.borrow().ui.upgrade() {
                    set_palette(&app, &ui, true);
                }
            });
        });
    if let Err(e) = spawned {
        eprintln!("oxiguitar: the palette's icons can't be drawn: {e}");
    }
}

pub(crate) fn image_of(icon: &icons::Icon) -> Image {
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&icon.rgba, icon.size, icon.size);
    Image::from_rgba8_premultiplied(buf)
}

/// The palette's groups for the window: every tool greyed out until the
/// editor (M3), with its icon once the icons are measured. `redraw` draws
/// the icons again (a new scale); otherwise only the groups' collapsed
/// state changes and the icons already shown are kept.
pub(crate) fn set_palette(app: &Shared, ui: &AppWindow, redraw: bool) {
    let s = app.borrow();
    let Some(set) = s.palette_icons.clone() else {
        return;
    };
    let model = ui.get_palette_groups();
    let shown = s.palette_shown.clone();
    let fresh = redraw || model.row_count() != shown.len();
    let images: Vec<Image> = if fresh {
        let px = (PALETTE_BUTTON * ui.window().scale_factor())
            .round()
            .max(8.0) as u32;
        set.draw(px).iter().map(image_of).collect()
    } else {
        Vec::new()
    };
    let mut images = images.into_iter();
    let groups: Vec<PaletteGroupRow> = shown
        .iter()
        .enumerate()
        .map(|(g, group)| {
            let tools: ModelRc<PaletteToolRow> = if fresh {
                let rows: Vec<PaletteToolRow> = group
                    .tools
                    .iter()
                    .map(|t| PaletteToolRow {
                        id: t.id.as_str().into(),
                        name: t.name.as_str().into(),
                        icon: images.next().unwrap_or_default(),
                        tip: palette::tool_tip(&t.id, &t.name).into(),
                        enabled: false,
                        checked: false,
                    })
                    .collect();
                ModelRc::new(VecModel::from(rows))
            } else {
                model.row_data(g).map(|r| r.tools).unwrap_or_default()
            };
            PaletteGroupRow {
                title: group.title.as_str().into(),
                collapsed: s.palette.collapsed(g),
                tools,
            }
        })
        .collect();
    let at = s.editing.at_caret();
    drop(s);
    if fresh {
        ui.set_palette_groups(ModelRc::new(VecModel::from(groups)));
    } else {
        for (g, row) in groups.into_iter().enumerate() {
            model.set_row_data(g, row);
        }
    }
    sync_palette_state(ui, at);
}

/// The palette's working tools: usable with a score open, highlighted as
/// the caret's beat is (its voice, value, dots, rest, tie).
pub(crate) fn sync_palette_state(ui: &AppWindow, at: Option<editing::AtCaret>) {
    sync_palette_rows(ui, at);
    if !ui.get_palette_query().is_empty() {
        sync_found(ui);
    }
}

pub(crate) fn sync_palette_rows(ui: &AppWindow, at: Option<editing::AtCaret>) {
    let groups = ui.get_palette_groups();
    for g in 0..groups.row_count() {
        let Some(group) = groups.row_data(g) else {
            continue;
        };
        for i in 0..group.tools.row_count() {
            let Some(mut row) = group.tools.row_data(i) else {
                continue;
            };
            if palette::working(row.id.as_str()).is_none() {
                continue;
            }
            let (enabled, checked) = (
                at.is_some(),
                at.is_some_and(|at| palette::checked(row.id.as_str(), &at)),
            );
            if (row.enabled, row.checked) != (enabled, checked) {
                row.enabled = enabled;
                row.checked = checked;
                group.tools.set_row_data(i, row);
            }
        }
    }
}
