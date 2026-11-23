// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Tools menu's dialogs: Transpose, Find Fingering, Fit Bars, Harmonize, Merge Tracks, and the clipboard's bar ranges and tuplets.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// Opens Transpose… for the caret's track, the selection or every track.
pub(crate) fn open_transpose(app: &Shared, scope: oxiguitar_edit::Scope) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() || app.borrow().editing.track().is_none() {
        return;
    }
    let heading = match scope {
        oxiguitar_edit::Scope::Track => {
            let s = app.borrow();
            let name = s
                .editing
                .score()
                .zip(s.editing.track())
                .and_then(|(sc, t)| sc.tracks.get(t).map(|t| t.name.clone()))
                .unwrap_or_default();
            format!("Transpose {name}")
        }
        oxiguitar_edit::Scope::Selection => {
            if app.borrow().editing.selection().is_some() {
                "Transpose the selection".to_owned()
            } else {
                "Transpose the beat".to_owned()
            }
        }
        oxiguitar_edit::Scope::All => "Transpose every track".to_owned(),
    };
    app.borrow_mut().transpose_scope = Some(scope);
    ui.set_transpose_heading(heading.into());
    ui.set_transpose_amount(0);
    ui.set_transpose_dialog_shown(true);
}

/// Find Fingering (#157) for the caret's track, the selection or every
/// track: one change, no dialog. Beats that stay hard to play get a text
/// saying why.
pub(crate) fn find_fingering(app: &Shared, scope: oxiguitar_edit::Scope) {
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Fingering(scope));
    if let Some(typed) = typed {
        send_typed(app, typed);
    }
}

/// Opens Fit Bars to Time Signature… (#222) for the whole song or the
/// selected bars (the caret's bar when nothing is selected).
pub(crate) fn open_fit(app: &Shared, whole_song: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() || app.borrow().editing.track().is_none() {
        return;
    }
    let heading = if whole_song {
        "Fit every bar to its time signature"
    } else if app.borrow().editing.selection().is_some() {
        "Fit the selected bars to their time signature"
    } else {
        "Fit this bar to its time signature"
    };
    app.borrow_mut().fit_whole_song = Some(whole_song);
    ui.set_fit_heading(heading.into());
    ui.set_fit_dialog_shown(true);
}

/// Fit Bars to Time Signature… closed: on OK, the fit as one change.
pub(crate) fn fit_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_fit_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(whole_song) = app.borrow_mut().fit_whole_song.take() else {
        return;
    };
    if !ok {
        return;
    }
    let how = if ui.get_fit_stretch() {
        oxiguitar_edit::FitHow::Stretch
    } else {
        oxiguitar_edit::FitHow::Rebar
    };
    let fit = oxiguitar_edit::Fit { how, whole_song };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::FitBars(fit));
    if let Some(typed) = typed {
        send_typed(app, typed);
    }
}

/// Opens Harmonize… (#235). Its tickbox starts on the whole song from
/// Tools and on the selected bars (the caret's bar when nothing is
/// selected) from the page's right-click menu; the user can change it.
pub(crate) fn open_harmonize(app: &Shared, whole_song: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() {
        return;
    }
    let name = {
        let s = app.borrow();
        let Some(track) = s.editing.track() else {
            return;
        };
        let score = s.editing.score();
        score
            .as_ref()
            .and_then(|sc| sc.tracks.get(track))
            .map_or_else(String::new, |t| t.name.clone())
    };
    let part = if app.borrow().editing.selection().is_some() {
        "the selected bars"
    } else {
        "this bar"
    };
    app.borrow_mut().harmonize_shown = true;
    ui.set_harmonize_heading(format!("Find chords for {name}").into());
    ui.set_harmonize_part(part.into());
    ui.set_harmonize_whole_song(whole_song);
    ui.set_harmonize_dialog_shown(true);
}

/// Harmonize… closed: on OK, the chords as one change. The footer says
/// when nothing was found, and when the notes fit another key better
/// than the key signature's.
pub(crate) fn harmonize_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_harmonize_dialog_shown(false);
    ui.invoke_focus_page();
    if !std::mem::take(&mut app.borrow_mut().harmonize_shown) || !ok {
        return;
    }
    let harmonize = oxiguitar_edit::Harmonize {
        sevenths: ui.get_harmonize_sevenths(),
        inside_bars: ui.get_harmonize_inside_bars(),
        keep: ui.get_harmonize_keep(),
        all_tracks: ui.get_harmonize_all_tracks(),
        whole_song: ui.get_harmonize_whole_song(),
    };
    harmonize_run(app, harmonize);
}

/// Harmonize as one change, as its dialog's OK does (or a plugin, interface
/// 1.14). The footer says when nothing was found, and when the notes fit
/// another key better than the key signature's.
pub(crate) fn harmonize_run(app: &Shared, harmonize: oxiguitar_edit::Harmonize) {
    let report = {
        let s = app.borrow();
        match (s.editing.score(), s.editing.caret()) {
            (Some(score), Some(caret)) => {
                oxiguitar_edit::harmonize_report(&score, caret, s.editing.selection(), &harmonize)
            }
            _ => return,
        }
    };
    let Some(report) = report else {
        message(
            app,
            Level::Warning,
            if harmonize.all_tracks {
                "No chords found: these bars have no notes, or the track is a drum kit."
            } else {
                "No chords found: this track has no notes in these bars, or it is a drum kit."
            },
        );
        return;
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Harmonize(harmonize));
    match typed {
        Some(typed) => {
            send_typed(app, typed);
            if let Some(other_key) = report.other_key {
                message(app, Level::Warning, other_key);
            }
        }
        None => message(
            app,
            Level::Warning,
            "No new chords: the chords already there are the ones Harmonize finds, or cover every bar.",
        ),
    }
}

/// Opens Merge Into… (#312) for the caret's track, offering every other
/// track of its kind (a drum track merges only into a drum track). The
/// voices that hold notes are ticked.
pub(crate) fn open_merge(app: &Shared) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    if app.borrow().question.is_some() {
        return;
    }
    let found = {
        let s = app.borrow();
        let (Some(score), Some(t)) = (s.editing.score(), s.editing.track()) else {
            return;
        };
        let Some(track) = score.tracks.get(t) else {
            return;
        };
        let drums = track.instrument.percussion;
        let (choices, names): (Vec<usize>, Vec<String>) = score
            .tracks
            .iter()
            .enumerate()
            .filter(|&(i, other)| i != t && other.instrument.percussion == drums)
            .map(|(i, other)| (i, other.name.clone()))
            .unzip();
        let mut voices = [false; oxiguitar_edit::VOICES];
        for bar in track.staves.iter().flat_map(|s| &s.bars) {
            for (v, voice) in bar.voices.iter().enumerate() {
                if let Some(on) = voices.get_mut(v) {
                    *on |= voice
                        .beats
                        .iter()
                        .any(|b| b.kind == oxiguitar_model::BeatKind::Notes);
                }
            }
        }
        if !voices.contains(&true) {
            voices[0] = true;
        }
        (t, track.name.clone(), drums, choices, names, voices)
    };
    let (t, name, drums, choices, names, voices) = found;
    if choices.is_empty() {
        message(
            app,
            Level::Warning,
            if drums {
                "No track to merge into: a drum track merges only into another drum track."
            } else {
                "No track to merge into: a track merges only into another track that isn't a drum kit."
            },
        );
        return;
    }
    app.borrow_mut().merge_dialog = Some((t, choices));
    ui.set_merge_heading(format!("Merge {name} into").into());
    ui.set_merge_tracks(strings(names));
    ui.set_merge_into(0);
    ui.set_merge_voice1(voices[0]);
    ui.set_merge_voice2(voices[1]);
    ui.set_merge_voice3(voices[2]);
    ui.set_merge_voice4(voices[3]);
    ui.set_merge_to_voice(0);
    ui.set_merge_dialog_shown(true);
}

/// Merge Into… closed: on OK, the merge as one change, after a question
/// when notes can't be kept; a bar whose rhythms can't be written together
/// stops it, with a warning.
pub(crate) fn merge_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_merge_dialog_shown(false);
    ui.invoke_focus_page();
    let Some((from, choices)) = app.borrow_mut().merge_dialog.take() else {
        return;
    };
    let picked = usize::try_from(ui.get_merge_into()).unwrap_or(0);
    let (Some(&into), true) = (choices.get(picked), ok) else {
        return;
    };
    let ticked: Vec<usize> = [
        ui.get_merge_voice1(),
        ui.get_merge_voice2(),
        ui.get_merge_voice3(),
        ui.get_merge_voice4(),
    ]
    .into_iter()
    .enumerate()
    .filter_map(|(v, on)| on.then_some(v))
    .collect();
    let voices = match ticked.as_slice() {
        [] => return,
        [one] => {
            let to = usize::try_from(ui.get_merge_to_voice()).unwrap_or(0);
            vec![(*one, to.min(oxiguitar_edit::VOICES - 1))]
        }
        many => many.iter().map(|&v| (v, v)).collect(),
    };
    let merge = oxiguitar_edit::Merge {
        into,
        voices,
        discard_unisons: ui.get_merge_discard_unisons(),
    };
    let (report, donor, receiver) = {
        let s = app.borrow();
        let Some(score) = s.editing.score() else {
            return;
        };
        if s.editing.track() != Some(from) {
            return;
        }
        let name = |t: usize| {
            score
                .tracks
                .get(t)
                .map(|t| t.name.clone())
                .unwrap_or_default()
        };
        (
            oxiguitar_edit::merge_lost(&score, from, &merge),
            name(from),
            name(into),
        )
    };
    let lost = match report {
        Ok(lost) => lost,
        Err(oxiguitar_edit::MergeError::Ranges {
            notes,
            lowest,
            highest,
        }) => {
            // The question says no; the footer says why and what to do.
            message(
                app,
                Level::Warning,
                format!(
                    "{donor} and {receiver} have incompatible ranges: {notes} {} of {donor} can't be played on {receiver}, which plays {} to {}. Consider transposing {donor} first.",
                    if notes == 1 { "note" } else { "notes" },
                    lowest.name(),
                    highest.name(),
                ),
            );
            ask(app, Asked::Told, "Incompatible tracks", &[], &["OK"]);
            return;
        }
        Err(oxiguitar_edit::MergeError::Rhythms { bar }) => {
            message(
                app,
                Level::Warning,
                format!(
                    "Not merged: in bar {}, the two tracks' rhythms can't be written in one voice.",
                    bar + 1
                ),
            );
            return;
        }
        Err(_) => return,
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::MergeTracks(merge));
    let Some(typed) = typed else {
        return;
    };
    if lost > 0 {
        ask(
            app,
            Asked::Apply(typed),
            &format!(
                "{lost} {} can't be kept",
                if lost == 1 { "note" } else { "notes" }
            ),
            &[format!(
                "No string of {receiver} is free to play them at their pitch, so they are left out. Undo brings them back."
            )],
            &["Merge", "Cancel"],
        );
    } else {
        send_typed(app, typed);
    }
}

/// Transpose… closed: on OK, the transposition as one change, after a
/// question when notes can't be kept.
pub(crate) fn transpose_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_transpose_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(scope) = app.borrow_mut().transpose_scope.take() else {
        return;
    };
    if !ok {
        return;
    }
    let Some(t) = track_form::transposition(
        ui.get_transpose_amount(),
        ui.get_transpose_steps(),
        ui.get_transpose_keep(),
        scope,
    ) else {
        return;
    };
    let typed = app
        .borrow_mut()
        .editing
        .tool(oxiguitar_edit::Command::Transpose(t));
    let Some(typed) = typed else {
        return;
    };
    let lost = app.borrow().editing.score().map_or(0, |sc| {
        oxiguitar_edit::notes_lost_transposing(&sc, typed.caret, typed.selection, &t)
    });
    if lost > 0 {
        ask(
            app,
            Asked::Apply(typed),
            &format!(
                "{lost} {} can't be kept",
                if lost == 1 { "note" } else { "notes" }
            ),
            &["No string can play them at their new pitch, so they are left out. Undo brings them back.".to_owned()],
            &["Apply", "Cancel"],
        );
    } else {
        send_typed(app, typed);
    }
}

/// Copy, cut or paste, or the dialog of Copy, Cut or Paste Bars.
pub(crate) fn clipboard(app: &Shared, key: editing::ClipKey) {
    let typed = match key {
        editing::ClipKey::Copy => {
            app.borrow_mut().editing.copy();
            return;
        }
        editing::ClipKey::Cut => app.borrow_mut().editing.cut(),
        editing::ClipKey::Paste => app.borrow_mut().editing.paste(),
        editing::ClipKey::CopyBars | editing::ClipKey::CutBars | editing::ClipKey::PasteBars => {
            open_bars_dialog(app, key);
            return;
        }
    };
    if let Some(t) = typed {
        send_typed(app, t);
        sync_caret(app, false);
    }
}

/// Copy, Cut or Paste Bars' dialog (#287); Paste Bars only once bars have
/// been copied.
pub(crate) fn open_bars_dialog(app: &Shared, key: editing::ClipKey) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    {
        let s = app.borrow();
        if s.question.is_some() || s.editing.caret().is_none() {
            return;
        }
    }
    let (heading, note) = match key {
        editing::ClipKey::CopyBars => ("Copy Bars", ""),
        editing::ClipKey::CutBars => (
            "Cut Bars",
            "The whole bars the selection touches are copied, then taken out of every track.",
        ),
        editing::ClipKey::PasteBars if app.borrow().editing.has_bars() => ("Paste Bars", ""),
        editing::ClipKey::PasteBars => {
            message(
                app,
                Level::Warning,
                "No bars to paste: copy some with Copy Bars (Ctrl + Shift + C) first.",
            );
            return;
        }
        _ => return,
    };
    app.borrow_mut().bars_dialog = Some(key);
    ui.set_bars_heading(heading.into());
    ui.set_bars_note(note.into());
    ui.set_bars_asks_place(key == editing::ClipKey::PasteBars);
    ui.set_bars_dialog_shown(true);
}

/// Copy, Cut or Paste Bars' dialog closed: on OK, done as asked.
pub(crate) fn bars_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_bars_dialog_shown(false);
    ui.invoke_focus_page();
    let Some(key) = app.borrow_mut().bars_dialog.take() else {
        return;
    };
    if !ok {
        return;
    }
    let all = ui.get_bars_all_tracks();
    let typed = match key {
        editing::ClipKey::CopyBars => {
            app.borrow_mut().editing.copy_bars(all);
            return;
        }
        editing::ClipKey::CutBars => app.borrow_mut().editing.cut_bars(all),
        editing::ClipKey::PasteBars => app
            .borrow_mut()
            .editing
            .paste_bars(ui.get_bars_after(), all),
        _ => None,
    };
    if let Some(t) = typed {
        send_typed(app, t);
        sync_caret(app, false);
    }
}

/// Custom (or Nested) N-tuplet closed: on OK, the beats go into that
/// tuplet (inside the one they have, for Nested).
pub(crate) fn tuplet_dialog_done(app: &Shared, ok: bool) {
    let Some(ui) = app.borrow().ui.upgrade() else {
        return;
    };
    ui.set_tuplet_dialog_shown(false);
    ui.invoke_focus_page();
    if !ok {
        return;
    }
    let Some(t) = bar_form::tuplet(ui.get_tuplet_notes(), ui.get_tuplet_in_time_of()) else {
        return;
    };
    let command = if ui.get_tuplet_nested() {
        oxiguitar_edit::Command::NestedTuplet(t)
    } else {
        oxiguitar_edit::Command::Tuplet(t)
    };
    let typed = app.borrow_mut().editing.tool(command);
    if let Some(t) = typed {
        send_typed(app, t);
    }
}
