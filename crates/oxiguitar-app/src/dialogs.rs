// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Questions over the whole window (unsaved changes) and the editing dialogs: lyrics, chords, bends and bars.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Goes on to `then`, asking first whether to save the changes when the
/// score differs from its file.
pub(crate) fn unless_unsaved(app: &Shared, then: Then) {
    let name = {
        let s = app.borrow();
        if s.question.is_some() {
            return;
        }
        s.modified.then(|| s.doc_name.clone().unwrap_or_default())
    };
    match name {
        Some(name) => ask(
            app,
            Asked::Unsaved(then),
            &format!("Save the changes to {name}?"),
            &["They are lost if you don't.".to_owned()],
            &["Save", "Don't Save", "Cancel"],
        ),
        None => go_on(app, then),
    }
}

/// Does what waited on the unsaved-changes question.
pub(crate) fn go_on(app: &Shared, then: Then) {
    match then {
        Then::New => app.borrow().worker.send(Request::New),
        Then::OpenDialog => open_dialog(app),
        Then::Open(path) => app.borrow().worker.send(Request::Open(path)),
        Then::Quit => {
            let _ = slint::quit_event_loop();
        }
    }
}

/// Shows the question over the window.
pub(crate) fn ask(app: &Shared, asked: Asked, heading: &str, lines: &[String], buttons: &[&str]) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    app.borrow_mut().question = Some(asked);
    ui.set_question_heading(heading.into());
    ui.set_question_lines(ModelRc::new(VecModel::from(
        lines
            .iter()
            .map(|l| SharedString::from(l.as_str()))
            .collect::<Vec<_>>(),
    )));
    ui.set_question_buttons(ModelRc::new(VecModel::from(
        buttons
            .iter()
            .map(|b| SharedString::from(*b))
            .collect::<Vec<_>>(),
    )));
    ui.set_question_shown(true);
}

/// A list of strings for the window.
pub(crate) fn strings(list: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        list.into_iter().map(SharedString::from).collect::<Vec<_>>(),
    ))
}

/// Opens the dialog the keys or the palette ask for at the caret; the
/// Bar's is the Bar tab.
pub(crate) fn open_edit_dialog(app: &Shared, dialog: editing::Dialog) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() {
        return;
    }
    match dialog {
        // The Bar dialog's fields are the Bar tab's.
        editing::Dialog::Bar => show_tab(app, "bar"),
        editing::Dialog::Tuplet | editing::Dialog::NestedTuplet => {
            if app.borrow().editing.bar_state().is_none() {
                return;
            }
            let nested = dialog == editing::Dialog::NestedTuplet;
            let five = oxiguitar_model::Tuplet {
                notes: 5,
                in_time_of: 4,
            };
            let t = if nested {
                app.borrow()
                    .editing
                    .inner_tuplet()
                    .unwrap_or(oxiguitar_model::Tuplet::TRIPLET)
            } else {
                app.borrow().editing.beat_tuplet().unwrap_or(five)
            };
            ui.set_tuplet_nested(nested);
            ui.set_tuplet_notes(i32::from(t.notes));
            ui.set_tuplet_in_time_of(i32::from(t.in_time_of));
            ui.set_tuplet_dialog_shown(true);
        }
        editing::Dialog::Bend | editing::Dialog::Whammy => {
            let whammy = dialog == editing::Dialog::Whammy;
            let here = {
                let s = app.borrow();
                if whammy {
                    s.editing.whammy_here()
                } else {
                    s.editing.bend_here()
                }
            };
            let Some(curve) = here else {
                return;
            };
            let form = bend_form::BendForm::open(curve.as_ref(), whammy);
            ui.set_bend_heading(if whammy { "Whammy Bar" } else { "Bend" }.into());
            sync_bend(&ui, &form);
            app.borrow_mut().bend_dialog = Some(form);
            ui.set_bend_dialog_shown(true);
        }
        editing::Dialog::Chord => {
            let (styles, prefs) = chord_settings(app);
            let style = prefs.style_in(&styles);
            let form = app
                .borrow()
                .editing
                .chord_form(styles, style, prefs.diagram);
            let Some(form) = form else {
                return;
            };
            ui.set_chord_styles(strings(form.style_names()));
            ui.set_chord_last_fret(i32::from(form.frets_max()));
            sync_chord(&ui, &form);
            app.borrow_mut().chord_dialog = Some(form);
            ui.set_chord_dialog_shown(true);
        }
        editing::Dialog::Text => {
            let Some(text) = app.borrow().editing.beat_text() else {
                return;
            };
            ui.set_text_dialog_text(text.into());
            ui.set_text_dialog_shown(true);
        }
        editing::Dialog::Lyrics => {
            let started = app.borrow_mut().editing.start_lyrics();
            if let Some((_, text)) = started {
                show_lyric(app, &ui, text);
            }
        }
    }
}

/// Ctrl + F: the palette shows, and its search field takes the keyboard.
pub(crate) fn search_palette(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    {
        let mut s = app.borrow_mut();
        let area = s.shell.area_mut(Side::Left);
        if let Some(i) = area.tabs.iter().position(|t| t.id == "palette") {
            area.active = i;
        }
        area.collapsed = false;
        area.shown = true;
    }
    sync_areas(app);
    ui.set_left_shown(true);
    ui.set_palette_search_requests(ui.get_palette_search_requests() + 1);
}

/// Shows the field where lyrics are typed, holding `text`, under the beat
/// it types at; hidden when nothing is typed there.
pub(crate) fn show_lyric(app: &Shared, ui: &AppWindow, text: String) {
    let spot = {
        let s = app.borrow();
        s.editing.lyric_spot().and_then(|(sheet, x, y, gap)| {
            let o = s.sheets.rects.get(sheet)?;
            Some((o.x + x, o.y + y, gap))
        })
    };
    match spot {
        Some((x, y, gap)) => {
            ui.set_lyric_x(x);
            ui.set_lyric_y(y);
            // The engraver's lyric size: 1.9 staff spaces, its lines 2.4
            // apart.
            ui.set_lyric_size(gap * 1.9 / 2.4);
            ui.set_lyric_text(text.into());
            ui.set_lyric_shown(true);
        }
        None => ui.set_lyric_shown(false),
    }
    caret_moved(app);
}

/// A key that ends a syllable typed on the page.
pub(crate) fn lyric_key(app: &Shared, key: &str, text: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let key = match key {
        "space" => editing::LyricKey::Space,
        "hyphen" => editing::LyricKey::Hyphen,
        "back" => editing::LyricKey::Back,
        "up" => editing::LyricKey::Up,
        "down" => editing::LyricKey::Down,
        _ => editing::LyricKey::Done,
    };
    let (edit, next) = app.borrow_mut().editing.lyric_key(key, text);
    if let Some(t) = edit {
        send_typed(app, t);
    }
    match next {
        Some((_, text)) => show_lyric(app, &ui, text),
        None => {
            ui.set_lyric_shown(false);
            ui.invoke_focus_page();
        }
    }
}

/// An edit at the caret (or on the selection), as a tool makes it.
pub(crate) fn edit_at_caret(app: &Shared, command: oxiguitar_edit::Command) {
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The chord notation styles and the chord dialog's lasting choices, from
/// settings.json. A settings.json without styles is given the first ones
/// (`chord_prefs`), so they are there to change.
pub(crate) fn chord_settings(
    app: &Shared,
) -> (oxiguitar_edit::chord_names::Styles, chord_prefs::ChordPrefs) {
    let mut s = app.borrow_mut();
    let stored = s.stored.get(chord_prefs::STYLES_KEY).unwrap_or_else(|| {
        let first = chord_prefs::first_styles();
        if let Err(e) = s.stored.set(chord_prefs::STYLES_KEY, first.clone()) {
            s.footer.push(Level::Error, e);
        }
        first
    });
    let prefs = chord_prefs::ChordPrefs::from_value(s.stored.get(chord_prefs::KEY).as_ref());
    (chord_prefs::styles(&stored), prefs)
}

/// Keeps the chord dialog's style and its "Add diagram" choice in
/// settings.json, for the next chord.
pub(crate) fn keep_chord_prefs(app: &Shared, form: &chord_form::ChordForm, diagram: Option<bool>) {
    let mut s = app.borrow_mut();
    let old = s.stored.get(chord_prefs::KEY);
    let mut prefs = chord_prefs::ChordPrefs::from_value(old.as_ref());
    if let Some(style) = form.style_name() {
        prefs.style = style;
    }
    if let Some(diagram) = diagram {
        prefs.diagram = diagram;
    }
    if Some(prefs.to_value(old.as_ref())) != old
        && let Err(e) = s.stored.set(chord_prefs::KEY, prefs.to_value(old.as_ref()))
    {
        s.footer.push(Level::Error, e);
    }
}

/// Shows the chord dialog's state.
pub(crate) fn sync_chord(ui: &AppWindow, f: &chord_form::ChordForm) {
    ui.set_chord_name(f.name.as_str().into());
    ui.set_chord_names(strings(f.names.clone()));
    ui.set_chord_voicing(f.voicing.as_str().into());
    ui.set_chord_voicings(strings(
        f.voicings
            .iter()
            .map(|v| chord_form::shape_text(v))
            .collect(),
    ));
    ui.set_chord_style(i32::try_from(f.style).unwrap_or(0));
    ui.set_chord_strings(i32::try_from(f.strings()).unwrap_or(0));
    ui.set_chord_dots(ModelRc::new(VecModel::from(f.dots())));
    ui.set_chord_first_fret(i32::from(f.first_fret));
    ui.set_chord_add_diagram(f.add_diagram);
}

/// The chord dialog's style applied to every chord of the song, as one
/// step for Undo: each name rewritten by the style, the ones it has no
/// alias for left as they are.
pub(crate) fn apply_chord_style(app: &Shared, form: &chord_form::ChordForm) {
    let Some(score) = app.borrow().editing.score() else {
        return;
    };
    let names: Vec<Vec<String>> = score
        .tracks
        .iter()
        .map(|t| t.chords.iter().map(|d| form.in_style(&d.name)).collect())
        .collect();
    edit_at_caret(app, oxiguitar_edit::Command::RenameChords(names));
}

/// A change in the chord dialog: `kind` names what, `value` and `text`
/// carry it.
pub(crate) fn chord_changed(app: &Shared, kind: &str, value: i32, text: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let i = usize::try_from(value).unwrap_or(0);
    let form = {
        let mut s = app.borrow_mut();
        let Some(f) = s.chord_dialog.as_mut() else {
            return;
        };
        match kind {
            "name" => f.type_name(text),
            "name-picked" => f.pick_name(i),
            "voicing" => f.type_voicing(text),
            "voicing-picked" => f.pick_voicing(i),
            "style" => f.pick_style(i),
            "apply" => f.song_restyled(),
            "grid" => {
                let strings = f.strings();
                let low = i / 100;
                if let Some(high_first) = strings.checked_sub(low + 1) {
                    f.click_grid(high_first, u8::try_from(i % 100).unwrap_or(0));
                }
            }
            "head" => {
                if let Some(high_first) = f.strings().checked_sub(i + 1) {
                    f.click_head(high_first);
                }
            }
            "first" => f.scroll(u8::try_from(i).unwrap_or(1)),
            "diagram" => f.add_diagram = value != 0,
            _ => {}
        }
        f.clone()
    };
    match kind {
        "style" => keep_chord_prefs(app, &form, None),
        "diagram" => keep_chord_prefs(app, &form, Some(form.add_diagram)),
        "apply" => apply_chord_style(app, &form),
        _ => {}
    }
    sync_chord(&ui, &form);
}

/// The chord dialog closed: on OK, its diagram on the caret's beat.
pub(crate) fn chord_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_chord_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(form) = app.borrow_mut().chord_dialog.take() else {
        return;
    };
    if ok {
        edit_at_caret(app, oxiguitar_edit::Command::Chord(form.diagram()));
    }
}

/// Shows the bend editor's state.
pub(crate) fn sync_bend(ui: &AppWindow, f: &bend_form::BendForm) {
    let types: &[&str] = if f.whammy {
        &bend_form::WHAMMY_TYPES
    } else {
        &bend_form::BEND_TYPES
    };
    // "Custom" last, always, so the list never changes under the box.
    let mut names: Vec<String> = types.iter().map(|t| (*t).to_owned()).collect();
    names.push("Custom".to_owned());
    let kind = f.kind.unwrap_or(types.len());
    if ui.get_bend_types().row_count() != names.len() {
        ui.set_bend_types(strings(names));
    }
    ui.set_bend_kind(i32::try_from(kind).unwrap_or(0));
    ui.set_bend_amount(f.amount - 1);
    let (low, high) = f.rows();
    ui.set_bend_low(low);
    ui.set_bend_high(high);
    ui.set_bend_points(ModelRc::new(VecModel::from(
        f.points
            .iter()
            .map(|p| GridPoint {
                col: p.col,
                row: p.row,
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_bend_path(f.path().into());
}

/// A change in the bend editor.
pub(crate) fn bend_edit(app: &Shared, edit: impl FnOnce(&mut bend_form::BendForm)) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let form = {
        let mut s = app.borrow_mut();
        let Some(f) = s.bend_dialog.as_mut() else {
            return;
        };
        edit(f);
        f.clone()
    };
    sync_bend(&ui, &form);
}

/// The bend editor closed: on OK, a changed curve is one change at the
/// caret (or on the selection).
pub(crate) fn bend_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_bend_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(form) = app.borrow_mut().bend_dialog.take() else {
        return;
    };
    if !ok || !form.changed {
        return;
    }
    let command = if form.whammy {
        oxiguitar_edit::Command::Whammy(form.bend())
    } else {
        oxiguitar_edit::Command::Bend(form.bend())
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// A row of the Bar tab was used: what it sets is one change at the caret
/// (or the selection's bars), one step to undo. `value` is the row's number,
/// index or bits; `text` a line of text.
pub(crate) fn bar_changed(app: &Shared, id: &str, value: i32, text: &str) {
    let command = app.borrow().editing.bar_changed(id, value, text);
    let Some(command) = command else {
        return;
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The Global checkbox for bar colour swatches was toggled.
pub(crate) fn bar_global_changed(app: &Shared, id: &str, global: bool) {
    if id != "bar-colour" {
        return;
    }
    // When toggling global off while a colour is set, apply it to the
    // current track only so the colour moves from all-tracks scope
    // to the current track. When toggling on, just store the setting.
    let command = app.borrow_mut().editing.set_bar_colour_global(global);
    if let Some(command) = command {
        let typed = app.borrow_mut().editing.tool(command);
        if let Some(t) = typed {
            send_typed(app, t);
        }
    }
}
