// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The right-hand panel and the Tracks list: the song, the inspector at the caret, the track's settings and plugins' sections in it.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Shows the right area's tab `id` ("bar", "note"), expanding the area and
/// showing it if it was collapsed or hidden. The keys, the palette and a
/// double-click on a time signature come here.
pub(crate) fn show_tab(app: &Shared, id: &str) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    app.borrow_mut().shell.area_mut(Side::Right).open(id);
    sync_areas(app);
    ui.set_right_shown(true);
}

/// The track list's rows.
pub(crate) fn track_rows(app: &Shared, tracks: &[TrackInfo]) -> Vec<TrackRow> {
    let s = app.borrow();
    tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let row = s.tracks.row(i);
            let mix = s.mixer.get(i);
            TrackRow {
                name: t.name.as_str().into(),
                shown: row.shown,
                standard: row.standard,
                tablature: row.tablature,
                can_tab: t.can_tab,
                volume: mix.volume,
                pan: mix.pan,
                mute: mix.mute,
                solo: mix.solo,
                wave: tracks::waveform(&t.activity).into(),
                bars: i32::try_from(t.activity.len()).unwrap_or(i32::MAX),
            }
        })
        .collect()
}

/// An edit added, deleted, moved, renamed or restrung tracks: the track
/// list and the page follow, keeping what they can (`Tracks::refit`).
pub(crate) fn track_list_changed(app: &Shared, tracks: &[TrackInfo], was: &[Option<usize>]) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    {
        let mut s = app.borrow_mut();
        let caret = s.editing.track().unwrap_or(0);
        s.track_count = tracks.len();
        s.tracks = s.tracks.refit(
            tracks.iter().map(|t| t.default_notation).collect(),
            tracks.iter().map(|t| t.can_tab).collect(),
            was,
            caret,
        );
        // The caret left on a hidden track (its own was deleted): to the
        // nearest shown one.
        if let Some(to) = s.tracks.nearest_shown(caret)
            && !s.tracks.shown().contains(&caret)
        {
            s.editing.select_track(to);
        }
        s.mixer = Mixer::new(tracks.iter().map(|t| t.mix).collect());
        s.settings.tracks = s.tracks.shown();
        s.settings.notation = s.tracks.notation();
        let settings = s.settings.clone();
        s.worker.send(Request::Engrave(settings));
    }
    ui.set_tracks(ModelRc::new(VecModel::from(track_rows(app, tracks))));
    sync_tracks(app);
}

/// The Bar tab shows the caret's bar: each voice's beats against the
/// time signature.
/// The Song tab: each field changed in place, so one being typed in keeps
/// its place and cursor.
pub(crate) fn sync_song(app: &Shared, ui: &AppWindow) {
    let fields = app
        .borrow()
        .editing
        .score()
        .map(|s| song::fields(&s.info))
        .unwrap_or_default();
    let rows: Vec<SongField> = fields
        .into_iter()
        .map(|f| SongField {
            id: f.id.into(),
            label: f.label.into(),
            text: f.text.into(),
            multiline: f.multiline,
        })
        .collect();
    let model = ui.get_song_fields();
    if model.row_count() != rows.len() {
        ui.set_song_fields(ModelRc::new(VecModel::from(rows)));
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).is_some_and(|r| r.text != row.text) {
            model.set_row_data(i, row);
        }
    }
}

/// The palette's search: the tools whose names hold the words typed, as
/// the palette shows them (enabled, lit).
pub(crate) fn sync_found(ui: &AppWindow) {
    let query = ui.get_palette_query().to_lowercase();
    if query.trim().is_empty() {
        ui.set_palette_found(ModelRc::default());
        return;
    }
    let groups = ui.get_palette_groups();
    let mut found = Vec::new();
    for g in 0..groups.row_count() {
        let Some(row) = groups.row_data(g) else {
            continue;
        };
        for i in 0..row.tools.row_count() {
            if let Some(tool) = row.tools.row_data(i)
                && palette::matches(tool.name.as_str(), &query)
            {
                found.push(tool);
            }
        }
    }
    ui.set_palette_found(ModelRc::new(VecModel::from(found)));
}

pub(crate) fn sync_inspector(app: &Shared, ui: &AppWindow) {
    sync_song(app, ui);
    let (note, bar, scope) = {
        let s = app.borrow();
        (
            s.editing.effect_fields(),
            s.editing.bar_fields(),
            s.editing.bar_scope(),
        )
    };
    ui.set_bar_scope(scope.into());
    sync_track(app, ui);
    show_rows(app, 0, note, ui.get_note_fields(), |m| {
        ui.set_note_fields(m)
    });
    show_rows(app, 1, bar, ui.get_bar_fields(), |m| ui.set_bar_fields(m));
    let view = app.borrow().editing.bar_view();
    let Some(v) = view else {
        ui.set_inspector_heading(SharedString::new());
        ui.set_inspector_voices(ModelRc::default());
        return;
    };
    ui.set_inspector_heading(v.heading.into());
    ui.set_inspector_unit(v.unit.into());
    ui.set_inspector_caret(v.caret.into());
    let rows: Vec<InspectorVoice> = v
        .voices
        .into_iter()
        .map(|r| InspectorVoice {
            name: r.name.into(),
            count: r.count.into(),
            state: r.state.into(),
            fill: match r.fill {
                inspector::Fill::Full => 0,
                inspector::Fill::Short => 1,
                inspector::Fill::Long => 2,
                inspector::Fill::Unknown => 3,
            },
            entered: i32::try_from(r.entered).unwrap_or(i32::MAX),
            current: r.current,
        })
        .collect();
    ui.set_inspector_voices(ModelRc::new(VecModel::from(rows)));
}

/// Shows `rows` in a tab's list (`which` 0 the Note tab, 1 the Bar tab).
/// Nothing is sent when they are what the window has. When only values
/// differ the rows change where they are, so a field being typed in keeps
/// its cursor and its focus; otherwise the list is replaced.
pub(crate) fn show_rows(
    app: &Shared,
    which: usize,
    rows: Vec<effect_form::Field>,
    model: ModelRc<InspectorField>,
    replace: impl FnOnce(ModelRc<InspectorField>),
) {
    let shown = std::mem::take(&mut app.borrow_mut().shown_rows[which]);
    if shown == rows {
        app.borrow_mut().shown_rows[which] = shown;
        return;
    }
    let row = |f: &effect_form::Field| InspectorField {
        id: f.id.into(),
        label: f.label.as_str().into(),
        kind: match f.kind {
            effect_form::Kind::Heading => 0,
            effect_form::Kind::Switch => 1,
            effect_form::Kind::Choice => 2,
            effect_form::Kind::Number => 3,
            effect_form::Kind::Button => 4,
            effect_form::Kind::Text => 5,
            effect_form::Kind::Ticks => 6,
            effect_form::Kind::Info => 7,
            effect_form::Kind::Swatches => 8,
        },
        value: f.value,
        options: strings(f.options.clone()),
        min: f.min,
        max: f.max,
        text: f.text.as_str().into(),
        global: f.kind == effect_form::Kind::Swatches && app.borrow().editing.bar_colour_global(),
    };
    let same_shape = shown.len() == rows.len()
        && model.row_count() == rows.len()
        && shown
            .iter()
            .zip(&rows)
            .all(|(a, b)| a.id == b.id && a.kind == b.kind && a.label == b.label);
    if same_shape {
        for (i, (old, new)) in shown.iter().zip(&rows).enumerate() {
            if old != new {
                model.set_row_data(i, row(new));
            }
        }
    } else {
        replace(ModelRc::new(VecModel::from(
            rows.iter().map(row).collect::<Vec<_>>(),
        )));
    }
    app.borrow_mut().shown_rows[which] = rows;
}

/// The Track tab: the caret's track as rows, and the plugins' sections for
/// it. The sections are asked again when the caret goes to another track or
/// the track's plugin settings changed by anything but the tab itself
/// (Undo, Redo).
pub(crate) fn sync_track(app: &Shared, ui: &AppWindow) {
    let (score, index) = {
        let s = app.borrow();
        (s.editing.score(), s.editing.track())
    };
    let track = match (&score, index) {
        (Some(sc), Some(t)) => sc.tracks.get(t).map(|tr| (t, tr)),
        _ => None,
    };
    let Some((t, track)) = track else {
        ui.set_track_heading(SharedString::new());
        ui.set_track_sections(ModelRc::default());
        let mut s = app.borrow_mut();
        s.sections = None;
        s.shown_rows[2].clear();
        ui.set_track_fields(ModelRc::default());
        return;
    };
    let (reopen, rows) = {
        let mut s = app.borrow_mut();
        let moved = s
            .sections
            .as_ref()
            .is_none_or(|x| x.track.index as usize != t);
        if moved {
            s.track_error = None;
        }
        let mut f = track_form::TrackForm::of(track);
        f.keep_pitches = s.track_keep_pitches;
        let (message, error) = match &s.track_error {
            Some(e) => (e.clone(), true),
            None => (f.preview(), false),
        };
        // The sections: asked again for another track, or when the
        // settings moved without the tab's doing.
        let seen = (track.plugins.clone(), track.effects.clone());
        let reopen = moved
            || s.sections.as_ref().is_some_and(|x| {
                seen != s.sections_seen
                    && (&track.plugins, &track.effects) != (&x.settings, &x.order)
            });
        s.sections_seen = seen;
        if reopen {
            s.dialogs_opened += 1;
            let sections =
                track_sections::TrackSections::open(&s.plugins, s.dialogs_opened, t, track);
            s.sections = Some(sections);
            s.sections_touched = false;
        }
        (
            reopen,
            track_form::fields(
                &f,
                &message,
                error,
                t,
                &score
                    .as_ref()
                    .unwrap()
                    .tracks
                    .iter()
                    .map(|tr| tr.name.clone())
                    .collect::<Vec<_>>(),
            ),
        )
    };
    ui.set_track_heading(format!("Track {}: {}", t + 1, track.name).into());
    if reopen {
        sync_track_sections(app);
    }
    show_rows(app, 2, rows, ui.get_track_fields(), |m| {
        ui.set_track_fields(m);
    });
}

/// A row of the Track tab was used: the track's settings as one change,
/// after a question when notes can't be kept. A tuning that can't be says
/// why in the tab and changes nothing.
pub(crate) fn track_changed(app: &Shared, id: &str, value: i32, text: &str) {
    // "chords-from" is not a TrackSettings field: use a ChordsFrom command.
    if id == "chords-from" {
        let (score, t, ui) = {
            let s = app.borrow();
            (s.editing.score(), s.editing.track(), s.ui.upgrade())
        };
        let (Some(_score), Some(_t)) = (score, t) else {
            return;
        };
        let Some(ui) = ui else {
            return;
        };
        let chords_from = if value == 0 {
            usize::MAX
        } else {
            usize::try_from(value - 1).unwrap_or(usize::MAX)
        };
        let typed = app
            .borrow_mut()
            .editing
            .tool(oxiguitar_edit::Command::ChordsFrom(chords_from));
        if let Some(typed) = typed {
            send_typed(app, typed);
        }
        sync_inspector(app, &ui);
        return;
    }
    let (score, index, keep) = {
        let s = app.borrow();
        (s.editing.score(), s.editing.track(), s.track_keep_pitches)
    };
    let (Some(score), Some(t)) = (score, index) else {
        return;
    };
    let Some(track) = score.tracks.get(t) else {
        return;
    };
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if id == "keep" {
        app.borrow_mut().track_keep_pitches = value == 0;
        sync_inspector(app, &ui);
        return;
    }
    let mut form = track_form::TrackForm::of(track);
    form.keep_pitches = keep;
    track_form::changed(&mut form, id, value, text);
    let settings = match form.to_settings() {
        Ok(s) => s,
        Err(e) => {
            app.borrow_mut().track_error = Some(e);
            sync_inspector(app, &ui);
            return;
        }
    };
    let was = oxiguitar_edit::TrackSettings {
        keep_pitches: keep,
        ..oxiguitar_edit::TrackSettings::of(track)
    };
    app.borrow_mut().track_error = None;
    if settings == was {
        sync_inspector(app, &ui);
        return;
    }
    let lost = oxiguitar_edit::notes_lost(&score, t, &settings);
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::TrackProperties(Box::new(settings)));
    let Some(typed) = typed else {
        return;
    };
    if lost > 0 {
        // Until it is answered the tab shows the track as it is.
        app.borrow_mut().shown_rows[2].clear();
        sync_inspector(app, &ui);
        ask(
            app,
            Asked::Apply(typed),
            &format!(
                "{lost} {} can't be kept",
                if lost == 1 { "note" } else { "notes" }
            ),
            &["No string can play them with these strings and capo, so they are left out. Undo brings them back.".to_owned()],
            &["Apply", "Cancel"],
        );
    } else {
        send_typed(app, typed);
    }
}

/// A plugin's own track-section view changed the track's settings for it
/// (interface 1.15): as a section's widget would, they go into the track
/// as one change. The error is a sentence for the plugin.
pub(crate) fn view_set_track_settings(
    app: &Shared,
    id: &oxiguitar_host::ViewId,
    settings: String,
) -> Result<(), String> {
    {
        let mut s = app.borrow_mut();
        let Some(sections) = s.sections.as_mut() else {
            return Err("no track's sections are shown".to_owned());
        };
        if !sections
            .sections
            .iter()
            .any(|x| x.plugin == id.plugin && x.section == id.view)
        {
            return Err(format!("the Track tab doesn't show the view {:?}", id.view));
        }
        if settings == sections.settings_of(&id.plugin) {
            return Ok(());
        }
        sections.settings.insert(id.plugin.clone(), settings);
        s.sections_touched = true;
    }
    apply_section_settings(app);
    Ok(())
}

/// A plugin's section changed the track's settings for it, or a section
/// was moved: they go into the track as one change.
pub(crate) fn apply_section_settings(app: &Shared) {
    let (score, index, plugins, effects, section_track) = {
        let s = app.borrow();
        let Some(sections) = s.sections.as_ref().filter(|_| s.sections_touched) else {
            return;
        };
        (
            s.editing.score(),
            s.editing.track(),
            sections.settings.clone(),
            sections.order.clone(),
            sections.track.index as usize,
        )
    };
    let (Some(score), Some(t)) = (score, index) else {
        return;
    };
    let Some(track) = score.tracks.get(t) else {
        return;
    };
    if t != section_track || (track.plugins == plugins && track.effects == effects) {
        return;
    }
    let mut form = track_form::TrackForm::of(track);
    form.plugins = plugins;
    form.effects = effects;
    let Ok(settings) = form.to_settings() else {
        return;
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::TrackProperties(Box::new(settings)));
    if let Some(typed) = typed {
        send_typed(app, typed);
    }
}

/// The Song tab's structure lists the score's sections.
pub(crate) fn sync_sections(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    let names = app
        .borrow()
        .editing
        .sections()
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    ui.set_sections(strings(names));
}

/// A track command from its row's menu (or the Edit menu), on the caret's
/// track.
pub(crate) fn track_action(app: &Shared, action: &str) {
    use oxiguitar_edit::Command;
    let command = match action {
        "transpose" => return open_transpose(app, oxiguitar_edit::Scope::Track),
        "fingering" => return find_fingering(app, oxiguitar_edit::Scope::Track),
        "merge" => return open_merge(app),
        "add" => Command::AddTrack,
        "duplicate" => Command::DuplicateTrack,
        "up" => Command::MoveTrackUp,
        "down" => Command::MoveTrackDown,
        "delete" => {
            let typed = app.borrow_mut().editing.tool(Command::DeleteTrack);
            let name = {
                let s = app.borrow();
                let score = s.editing.score();
                let t = s.editing.track().unwrap_or(0);
                score
                    .and_then(|sc| sc.tracks.get(t).map(|t| t.name.clone()))
                    .unwrap_or_default()
            };
            if let Some(t) = typed {
                ask(
                    app,
                    Asked::DeleteTrack(t),
                    &format!("Delete the track {name}?"),
                    &["Its notes go with it. Undo brings it back.".to_owned()],
                    &["Delete", "Cancel"],
                );
            }
            return;
        }
        _ => return,
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}

/// The Track tab's plugin sections, as they stand.
pub(crate) fn sync_track_sections(app: &Shared) {
    let s = app.borrow();
    let Some(ui) = s.ui.upgrade() else {
        return;
    };
    let sections: Vec<PluginSection> = s
        .sections
        .iter()
        .flat_map(|t| {
            t.sections
                .iter()
                .enumerate()
                .map(move |(i, x)| (t.moves(i), x))
        })
        .map(|((up, down), shown)| {
            // The plugin's own view, made once for this opening of the
            // sections, with the track and its settings.
            let factory = s
                .sections
                .as_ref()
                .map(|t| {
                    let id = oxiguitar_host::ViewId {
                        plugin: shown.plugin.clone(),
                        view: shown.section.clone(),
                    };
                    let track = oxiguitar_host::api::view::TrackView {
                        track: t.track.clone(),
                        settings: t.settings_of(&shown.plugin).to_owned(),
                    };
                    plugin_ui::section(&id, t.dialog, track, || {
                        let _ = slint::invoke_from_event_loop(|| {
                            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                                sync_track_sections(&app);
                            }
                        });
                    })
                })
                .unwrap_or_default();
            PluginSection {
                title: shown.title.as_str().into(),
                up,
                down,
                factory,
            }
        })
        .collect();
    ui.set_track_sections(ModelRc::new(VecModel::from(sections)));
}

/// The Track tab's section `index` was moved up or down: the order of the
/// track's effects changes, as one change of the track (#152). Not while
/// the song plays: the effects would have to be prepared again, and
/// editing waits (only the sections' settings change while playing, #232).
pub(crate) fn track_section_moved(app: &Shared, index: i32, up: bool) {
    if app.borrow().player.refuses_edits() {
        handle(app, Event::Refused);
        return;
    }
    let moved = {
        let mut s = app.borrow_mut();
        let moved = usize::try_from(index)
            .ok()
            .zip(s.sections.as_mut())
            .is_some_and(|(i, x)| x.move_section(i, up));
        if moved {
            s.sections_touched = true;
        }
        moved
    };
    if moved {
        sync_track_sections(app);
        apply_section_settings(app);
    }
}
