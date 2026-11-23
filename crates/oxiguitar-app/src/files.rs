// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Files: the startup advice, Open, Save, Save As, Export and audio export, and the system's file dialogs (on threads of their own, through rfd's portal).
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Shows the next advice that waits, unless one shows already: one dialog
/// at a time.
pub(crate) fn show_advice(app: &Shared) {
    if app.borrow().advice.is_some() {
        return;
    }
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let Some(asked) = advice::next() else {
        return;
    };
    ui.set_advice_heading(asked.advice.title.as_str().into());
    ui.set_advice_source(advice::source(&asked).into());
    ui.set_advice_lines(strings(advice::lines(&asked.advice.text)));
    ui.set_advice_links(ModelRc::new(VecModel::from(
        asked
            .advice
            .links
            .iter()
            .map(|l| AdviceLink {
                text: l.text.as_str().into(),
                url: l.url.as_str().into(),
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_advice_hide(false);
    ui.set_advice_shown(true);
    app.borrow_mut().advice = Some(asked);
}

/// The user closed the advice dialog: keep what the box says (silenced for
/// good, in settings.json), then show the next advice that waits.
pub(crate) fn advice_done(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let hide = ui.get_advice_hide();
    ui.set_advice_shown(false);
    ui.invoke_focus_page();
    let asked = app.borrow_mut().advice.take();
    if let (true, Some(asked)) = (hide, asked) {
        let saved =
            oxiguitar_host::advice::hide(&app.borrow().stored, &asked.plugin, &asked.advice.id);
        if let Err(why) = saved {
            app.borrow_mut().footer.push(Level::Warning, why);
            sync_footer(app);
        }
    }
    show_advice(app);
}

/// The user answered the question: button `i`, the last being Cancel.
pub(crate) fn question_answered(app: &Shared, i: i32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_question_shown(false);
    ui.invoke_focus_page();
    let Some(asked) = app.borrow_mut().question.take() else {
        return;
    };
    match (asked, i) {
        (Asked::Save { can_keep }, 0) => app
            .borrow()
            .worker
            .send(Request::ConfirmSave { keep: can_keep }),
        (Asked::Save { can_keep: true }, 1) => app
            .borrow()
            .worker
            .send(Request::ConfirmSave { keep: false }),
        (Asked::Save { .. }, _) => {
            let mut s = app.borrow_mut();
            s.after_save = None;
            s.worker.send(Request::CancelSave);
        }
        (Asked::Unsaved(then), 0) => {
            app.borrow_mut().after_save = Some(then);
            save(app);
        }
        (Asked::Unsaved(then), 1) => go_on(app, then),
        (Asked::Unsaved(_), _) => {}
        (Asked::Apply(t) | Asked::DeleteTrack(t), 0) => send_typed(app, t),
        (Asked::Apply(_) | Asked::DeleteTrack(_) | Asked::Told, _) => {}
    }
}

/// File → Save: the file the score came from, in its own format (the
/// worker asks first when that loses part of the score).
pub(crate) fn save(app: &Shared) {
    // A new score has no file yet: Save asks for a name, as Save As.
    if app.borrow().untitled {
        save_as_dialog(app);
        return;
    }
    app.borrow().worker.send(Request::Save(None));
}

/// File → Save As…: any name, in a format the plugins write, chosen by the
/// name's extension (the first listed when it has none).
pub(crate) fn save_as_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        app.borrow_mut().after_save = None;
        message(app, Level::Warning, problem);
        return;
    }
    let (folder, stem, formats, sender) = {
        let s = app.borrow();
        let stem = s
            .doc_name
            .as_deref()
            .map(|n| {
                std::path::Path::new(n)
                    .file_stem()
                    .map_or_else(|| n.to_owned(), |x| x.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "score".to_owned());
        (
            s.folder.clone(),
            stem,
            s.save_formats.clone(),
            s.worker.sender(),
        )
    };
    if formats.is_empty() {
        app.borrow_mut().after_save = None;
        message(
            app,
            Level::Error,
            "No installed plugin writes files, so nothing can be saved.".to_owned(),
        );
        return;
    }
    // The score's own format first, as the default: the one its file's
    // extension names.
    let own = app.borrow().doc_name.as_deref().and_then(|n| {
        std::path::Path::new(n)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
    });
    let mut formats = formats;
    formats.sort_by_key(|f| !own.as_ref().is_some_and(|e| f.extensions.contains(e)));
    let pick = move || {
        let name = match formats.first().and_then(|f| f.extensions.first()) {
            Some(extension) => format!("{stem}.{extension}"),
            None => stem,
        };
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save As")
            .set_file_name(name);
        for f in &formats {
            dialog = dialog.add_filter(&f.name, &f.extensions);
        }
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        let Some(mut path) = dialog.save_file() else {
            // Nothing waits on a save that won't happen (quitting after
            // "Save the changes…?", say).
            let _ = slint::invoke_from_event_loop(|| {
                if let Some(app) = APP.with(|a| a.borrow().clone()) {
                    app.borrow_mut().after_save = None;
                }
            });
            return;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let format = ext
            .as_deref()
            .and_then(|e| {
                formats
                    .iter()
                    .find(|f| f.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
            })
            .or(formats.first());
        let Some(format) = format else {
            return;
        };
        if !format
            .extensions
            .iter()
            .any(|x| ext.as_deref().is_some_and(|e| x.eq_ignore_ascii_case(e)))
            && let Some(e) = format.extensions.first()
        {
            path.set_extension(e);
        }
        let _ = sender.send(Request::Save(Some((path, format.id.clone()))));
    };
    run_dialog(pick);
}

/// File → Open: the system's dialog, off the UI thread where the system
/// allows it, so the window keeps drawing.
pub(crate) fn open_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        message(app, Level::Warning, problem);
        return;
    }
    let folder = app.borrow().folder.clone();
    let extensions = app.borrow().open_extensions.clone();
    let sender = app.borrow().worker.sender();
    let pick = move || {
        let mut dialog = rfd::FileDialog::new().set_title("Open");
        // The formats the installed plugins read; with none, the dialog
        // still opens, and the footer says why nothing can be read.
        if !extensions.is_empty() {
            dialog = dialog.add_filter("Scores", &extensions);
        }
        dialog = dialog.add_filter("All files", &["*"]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        if let Some(path) = dialog.pick_file() {
            let _ = sender.send(Request::Open(path));
        }
    };
    run_dialog(pick);
}

/// File → Export PDF…: the shown tracks, in pages, on the chosen paper.
pub(crate) fn export_dialog(app: &Shared) {
    if let Some(problem) = no_dialogs() {
        message(app, Level::Warning, problem);
        return;
    }
    let (folder, name, settings, sender) = {
        let s = app.borrow();
        (
            s.folder.clone(),
            s.pdf_name.clone(),
            s.settings.clone(),
            s.worker.sender(),
        )
    };
    let pick = move || {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export PDF")
            .set_file_name(&name)
            .add_filter("PDF", &["pdf"]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        if let Some(mut path) = dialog.save_file() {
            if path.extension().is_none() {
                path.set_extension("pdf");
            }
            let _ = sender.send(Request::ExportPdf(path, settings));
        }
    };
    run_dialog(pick);
}

/// File → Export Audio…: the dialog of the format and what to render. It
/// starts with the format picked last; a plugin's dialog can't open over
/// it (they share the window's one dialog at a time).
pub(crate) fn export_audio_dialog(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let i = {
        let s = app.borrow();
        if s.exporters.is_empty() || s.export_cancel.is_some() || s.export_waiting.is_some() {
            return;
        }
        audio_export::remembered(
            &s.exporters,
            s.stored.get(audio_export::FORMAT_KEY).as_ref(),
        )
    };
    if DIALOG_OPEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    {
        let s = app.borrow();
        ui.set_export_has_selection(s.editing.selection().is_some());
        ui.set_export_practice_speed(s.player.speed() as i32);
    }
    ui.set_export_selection(false);
    ui.set_export_at_speed(false);
    ui.set_export_metronome(false);
    ui.set_export_format(i32::try_from(i).unwrap_or(0));
    export_format_picked(app, i32::try_from(i).unwrap_or(0));
    ui.set_export_dialog_shown(true);
}

/// The export dialog's format is `i`: its options show, and the ones shown
/// before are closed with Cancel (what was picked in them is forgotten).
pub(crate) fn export_format_picked(app: &Shared, i: i32) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let options = {
        let mut s = app.borrow_mut();
        let Some(target) = usize::try_from(i)
            .ok()
            .and_then(|i| s.exporters.get(i).cloned())
        else {
            return;
        };
        s.export_with = usize::try_from(i).ok();
        if s.export_options == target.options && s.export_options.is_some() {
            return;
        }
        close_export_options(&mut s, false);
        s.export_options = target.options.clone();
        if let Some(tab) = &target.options {
            // What an earlier opening showed is not what this one shows.
            s.plugin_tabs.remove(tab);
            if let Some(thread) = &s.tab_thread {
                thread.send(plugin_tabs::Request::Content(tab.clone()));
            }
        }
        target.options
    };
    ui.set_export_has_options(options.is_some());
    match options {
        Some(tab) => refresh_plugin_view(app, View::Export, &tab),
        None => set_plugin_area(&ui, View::Export, None, ""),
    }
}

/// Closes the export dialog's options, if it shows some, telling their
/// plugin whether the user pressed OK; which ones they were.
pub(crate) fn close_export_options(s: &mut App, ok: bool) -> Option<TabId> {
    let tab = s.export_options.take()?;
    let view = &mut s.plugin_views[View::Export.index()];
    view.tab = None;
    view.content = None;
    s.plugin_tabs.remove(&tab);
    if let Some(thread) = &s.tab_thread {
        thread.send(plugin_tabs::Request::Closed {
            tab: tab.clone(),
            ok,
        });
    }
    Some(tab)
}

/// The export dialog closed: with OK, once the plugin has kept the
/// options, the file dialog, then the export on a thread of its own while
/// the window shows how far it has got.
pub(crate) fn export_audio_chosen(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_export_dialog_shown(false);
    DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
    let (i, options) = {
        let mut s = app.borrow_mut();
        let options = close_export_options(&mut s, ok);
        (s.export_with.take(), options)
    };
    set_plugin_area(&ui, View::Export, None, "");
    let Some(i) = i else {
        return;
    };
    if !ok {
        return;
    }
    // The format is kept with its options, whatever comes of the export.
    {
        let mut s = app.borrow_mut();
        let Some(key) = s.exporters.get(i).map(audio_export::Target::key) else {
            return;
        };
        if s.stored
            .get(audio_export::FORMAT_KEY)
            .as_ref()
            .and_then(|v| v.as_str())
            != Some(key.as_str())
            && let Err(e) = s.stored.set(audio_export::FORMAT_KEY, key.into())
        {
            s.footer.push(Level::Error, e);
        }
    }
    if let Some(problem) = no_dialogs() {
        message(app, Level::Warning, problem);
        return;
    }
    let (job, name, folder) = {
        let mut s = app.borrow_mut();
        let Some(target) = s.exporters.get(i).cloned() else {
            return;
        };
        let Some(score) = s.open_score.score() else {
            return;
        };
        let timeline = oxiguitar_play::Timeline::new(&score);
        let span = s
            .editing
            .selection()
            .filter(|_| ui.get_export_selection())
            .and_then(|sel| player::span_of(sel, &score))
            .and_then(|span| player::times_of(&timeline, span));
        let plan = oxiguitar_play::export::Plan {
            from: span.map_or(0.0, |(a, _)| a),
            until: span.map(|(_, b)| b),
            speed: if ui.get_export_at_speed() {
                f64::from(s.player.speed()) / 100.0
            } else {
                1.0
            },
            metronome: ui.get_export_metronome(),
            mixes: s.player.mixes(),
            master: player::master_gain(s.player.master()),
        };
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        s.export_cancel = Some(cancel.clone());
        let name = audio_export::file_name(&s.pdf_name, &target.extension);
        let job = audio_export::Job {
            plugins: s.plugins.clone(),
            target,
            score,
            plan,
            cancel,
        };
        (job, name, s.folder.clone())
    };
    match options {
        // The tab thread tells the plugin before anything else asked of it
        // after; its answer (`Answer::Closed`) lets the export go on.
        Some(options) => {
            app.borrow_mut().export_waiting = Some(audio_export::Waiting {
                options,
                job,
                name,
                folder,
            });
        }
        None => pick_export_file(&ui, job, name, folder),
    }
}

/// The options `tab` were closed: the export waiting for them asks for its
/// file.
pub(crate) fn export_options_closed(app: &Shared, tab: &TabId) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let waiting = {
        let mut s = app.borrow_mut();
        match &s.export_waiting {
            Some(w) if w.options == *tab => s.export_waiting.take(),
            _ => None,
        }
    };
    if let Some(w) = waiting {
        pick_export_file(&ui, w.job, w.name, w.folder);
    }
}

/// The file dialog for `job`, then the export on a thread of its own.
pub(crate) fn pick_export_file(
    ui: &AppWindow,
    job: audio_export::Job,
    name: String,
    folder: Option<PathBuf>,
) {
    let weak = ui.as_weak();
    let pick = move || {
        let extension = job.target.extension.clone();
        let mut dialog = rfd::FileDialog::new()
            .set_title(format!("Export {}", job.target.name))
            .set_file_name(&name)
            .add_filter(&job.target.name, &[&extension]);
        if let Some(f) = folder {
            dialog = dialog.set_directory(f);
        }
        let Some(path) = dialog.save_file() else {
            let _ = weak.upgrade_in_event_loop(|ui| {
                ui.invoke_export_finished(0, SharedString::default());
            });
            return;
        };
        let path = audio_export::with_extension(path, &extension);
        let status = format!(
            "Exporting {}…",
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        let shown = weak.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_export_status(status.into());
            ui.set_export_progress(0.0);
            ui.set_export_running(true);
        });
        let done = weak.clone();
        let _ = std::thread::Builder::new()
            .name("oxiguitar-export".to_owned())
            .spawn(move || {
                let (level, text) = job.run(&path, &mut |p| {
                    let _ = shown.upgrade_in_event_loop(move |ui| ui.set_export_progress(p));
                });
                let _ = done.upgrade_in_event_loop(move |ui| {
                    ui.invoke_export_finished(level_code(level), text.into());
                });
            });
    };
    run_dialog(pick);
}

/// On Linux the file dialogs come from the XDG desktop portal over the
/// session's D-Bus. Without a session bus there is no dialog to show, so say
/// so rather than doing nothing.
pub(crate) fn no_dialogs() -> Option<String> {
    if !cfg!(target_os = "linux") || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return None;
    }
    let bus = std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("bus"));
    if bus.is_some_and(|b| b.exists()) {
        return None;
    }
    Some(
        "No file dialog: this session has no D-Bus for the desktop portal. \
         Start OxiGuitar with a file instead: oxiguitar FILE"
            .to_owned(),
    )
}

/// macOS wants dialogs on the main thread; elsewhere they run on their own
/// thread so the window stays responsive.
pub(crate) fn run_dialog(pick: impl FnOnce() + Send + 'static) {
    if cfg!(target_os = "macos") {
        pick();
    } else {
        let _ = std::thread::Builder::new()
            .name("oxiguitar-dialog".to_owned())
            .spawn(pick);
    }
}
