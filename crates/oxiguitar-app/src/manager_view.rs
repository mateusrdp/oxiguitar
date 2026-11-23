// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The plugin manager's window (Tools > Plugins): its list, and what a tick changes at the next start.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Tools → Plugins: the plugin manager's window, made the first time and
/// shown again (in front) after that. It doesn't block the main window.
pub(crate) fn open_plugin_manager(app: &Shared) {
    let existing = app
        .borrow()
        .manager
        .as_ref()
        .map(ComponentHandle::clone_strong);
    let window = match existing {
        Some(w) => w,
        None => {
            let Ok(w) = PluginManagerWindow::new() else {
                message(
                    app,
                    Level::Error,
                    "The plugin manager's window couldn't be opened.",
                );
                return;
            };
            install_plugin_links(&w.global::<PluginLinks<'_>>(), app);
            let a = app.clone();
            w.on_toggled(move |i, on| manager_toggled(&a, i, on));
            let weak = w.as_weak();
            w.on_close_clicked(move || {
                if let Some(w) = weak.upgrade() {
                    let _ = w.hide();
                }
            });
            app.borrow_mut().manager = Some(w.clone_strong());
            w
        }
    };
    {
        let s = app.borrow();
        window.set_dark_interface(s.dark_interface);
        window.set_safe_mode(s.plugins.safe_mode());
        let can_save = s.stored.path().is_some();
        window.set_can_save(can_save);
        window.set_problem(if can_save {
            SharedString::new()
        } else {
            SharedString::from(format!(
                "{} couldn't be read when OxiGuitar started, so plugins can't be enabled or disabled now: correct it (or remove it) and start OxiGuitar again.",
                oxiguitar_host::SETTINGS_FILE
            ))
        });
    }
    sync_manager(app);
    let _ = window.show();
}

/// The manager's rows, as settings.json stands now.
pub(crate) fn sync_manager(app: &Shared) {
    let s = app.borrow();
    let Some(window) = &s.manager else {
        return;
    };
    let rows = manager::rows(&s.plugins, &manager::disabled(&s.stored));
    window.set_restart_needed(manager::restart_needed(&rows));
    let rows: Vec<ManagerRow> = rows
        .into_iter()
        .map(|r| ManagerRow {
            name: r.name.into(),
            version: r.version.into(),
            file: r.file.into(),
            folder: r.folder.into(),
            state: r.state.into(),
            can_toggle: r.id.is_some(),
            enabled: r.enabled,
        })
        .collect();
    window.set_rows(ModelRc::new(VecModel::from(rows)));
}

/// A tickbox in the manager: enable or disable that plugin from the next
/// start. A loaded plugin can't be unloaded, so unticking one also says in
/// the footer that it stays in use until OxiGuitar restarts.
pub(crate) fn manager_toggled(app: &Shared, index: i32, on: bool) {
    let (result, row) = {
        let s = app.borrow();
        let row = usize::try_from(index)
            .ok()
            .and_then(|i| manager::rows(&s.plugins, &[]).into_iter().nth(i));
        let result = row
            .as_ref()
            .and_then(|r| r.id.as_ref())
            .map(|id| manager::set_enabled(&s.stored, id, on));
        (result, row)
    };
    match result {
        Some(Err(e)) => {
            if let Some(w) = &app.borrow().manager {
                w.set_problem(e.into());
            }
        }
        Some(Ok(())) if !on && row.as_ref().is_some_and(|r| r.loaded) => {
            let name = row.map(|r| r.name).unwrap_or_default();
            message(
                app,
                Level::Info,
                format!(
                    "{name} stays in use until you restart OxiGuitar: a loaded plugin can't be unloaded while it runs."
                ),
            );
        }
        _ => {}
    }
    sync_manager(app);
}
